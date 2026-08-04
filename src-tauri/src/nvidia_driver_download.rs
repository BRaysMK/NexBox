//! NVIDIA 驱动下载模块
//!
//! 通过 NVIDIA AjaxDriverService API 获取所有 GeForce 驱动版本：
//! - languageID=1（English US）避免返回每种语言的重复条目
//! - upCRD=0 排除 Studio（Creator Ready）驱动，仅保留 Game Ready（GRD）
//! - 合并台式机/笔记本两个通道（RTX 50 + GTX 10），按版本号去重，从最新到最旧
//! - 每个版本同时附带台式机与笔记本的详情页/下载地址
//! - 请求带有限流（并发 3）与自动重试，避免偶发握手失败/超时导致整页拿不到数据

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use regex::Regex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Semaphore;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// Windows 10/11 64-bit DCH 驱动的 osid
const OSID_WIN10_11_64BIT: u32 = 57;

/// 单次请求返回的最大驱动数量（超过该值 NVIDIA 后端会超时并返回 502）
const API_RESULT_LIMIT: u32 = 50;

/// 同时进行的请求数上限（共 4 个查询任务全部并行，以缩短总耗时；
/// 并发进一步加大 NVIDIA 后端会拒绝握手）
const MAX_CONCURRENCY: usize = 4;

/// 单个驱动版本在某个设备类型下的信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverClassInfo {
    /// 驱动唯一 ID
    pub id: String,
    /// 详情页 URL
    pub detail_url: String,
    /// 直接下载 URL（.exe）
    pub download_url: String,
}

/// 单个驱动版本（合并台式机/笔记本后的行数据）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverEntry {
    /// 驱动版本号，如 "610.88"
    pub version: String,
    /// 驱动分支类型，如 "GRD"
    pub branch: String,
    /// 发布日期，如 "2026-07-28"
    pub release_date: String,
    /// 驱动名称/描述
    pub name: String,
    /// 是否最新版
    pub is_latest_only: bool,
    /// 台式机驱动信息
    pub desktop: Option<DriverClassInfo>,
    /// 笔记本驱动信息
    pub laptop: Option<DriverClassInfo>,
}

/// 当前 GPU 检测结果
#[derive(Debug, Clone, Serialize)]
pub struct GpuDetection {
    /// GPU 名称
    pub gpu_name: String,
    /// 匹配到的系列名称
    pub series_name: String,
    /// 是否笔记本
    pub is_laptop: bool,
    /// 当前驱动版本
    pub driver_version: String,
}

/// NVIDIA AjaxDriverService API 返回的 JSON 结构。
/// 注意：查询不到驱动时响应中可能缺失大部分字段，全部用默认值兜底，
/// 避免出现 "missing field Name" 之类的反序列化错误。
#[derive(Debug, Deserialize)]
struct ApiDriverResponse {
    #[serde(rename = "IDS", default)]
    ids: Vec<ApiDriverItem>,
}

#[derive(Debug, Deserialize)]
struct ApiDriverItem {
    #[serde(rename = "downloadInfo")]
    download_info: ApiDownloadInfo,
}

#[derive(Debug, Deserialize, Default)]
struct ApiDownloadInfo {
    #[serde(rename = "ID", default)]
    id: String,
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "Version", default)]
    version: String,
    #[serde(rename = "ReleaseDateTime", default)]
    release_date_time: String,
    #[serde(rename = "IsCRD", default)]
    is_crd: String,
    #[serde(rename = "DownloadURL", default)]
    download_url: String,
}

/// 中间结果：单个驱动版本的单通道信息
struct ClassDriver {
    version: String,
    name: String,
    branch: String,
    release_date: String,
    class_info: DriverClassInfo,
}

/// GeForce 系列映射（来自 NVIDIA lookupValueSearch API 的当前值）
struct GpuSeriesRef {
    name: &'static str,
    desktop_psid: u32,
    desktop_pfid: u32,
    laptop_psid: u32,
    laptop_pfid: u32,
}

/// 全部系列（用于自动检测匹配系列名）
const GEFORCE_SERIES: &[GpuSeriesRef] = &[
    GpuSeriesRef {
        name: "RTX 50 系列",
        desktop_psid: 131,
        desktop_pfid: 1066,
        laptop_psid: 133,
        laptop_pfid: 1073,
    },
    GpuSeriesRef {
        name: "RTX 40 系列",
        desktop_psid: 127,
        desktop_pfid: 1015,
        laptop_psid: 129,
        laptop_pfid: 1007,
    },
    GpuSeriesRef {
        name: "RTX 30 系列",
        desktop_psid: 120,
        desktop_pfid: 930,
        laptop_psid: 123,
        laptop_pfid: 938,
    },
    GpuSeriesRef {
        name: "RTX 20 系列",
        desktop_psid: 107,
        desktop_pfid: 879,
        laptop_psid: 111,
        laptop_pfid: 890,
    },
    GpuSeriesRef {
        name: "GTX 16 系列",
        desktop_psid: 112,
        desktop_pfid: 911,
        laptop_psid: 115,
        laptop_pfid: 899,
    },
    GpuSeriesRef {
        name: "GTX 10 系列",
        desktop_psid: 101,
        desktop_pfid: 845,
        laptop_psid: 102,
        laptop_pfid: 819,
    },
];

/// 实际查询的系列（覆盖 2021 年至今全部 GRD 版本）：
/// GTX 10（Pascal）覆盖 2021~2025 旧版本区间，RTX 50 覆盖 2025 至今的新版本区间，
/// 两个系列合并即可覆盖完整时间线，减少请求数从而显著加快加载速度。
const SERIES_TO_FETCH: &[GpuSeriesRef] = &[
    GpuSeriesRef {
        name: "RTX 50 系列",
        desktop_psid: 131,
        desktop_pfid: 1066,
        laptop_psid: 133,
        laptop_pfid: 1073,
    },
    GpuSeriesRef {
        name: "GTX 10 系列",
        desktop_psid: 101,
        desktop_pfid: 845,
        laptop_psid: 102,
        laptop_pfid: 819,
    },
];

/// 驱动列表内存缓存（避免每次进入页面都重新扫描）
struct DriverCache {
    entries: Vec<DriverEntry>,
    fetched_at: SystemTime,
}

static DRIVER_CACHE: Mutex<Option<DriverCache>> = Mutex::new(None);

/// GPU 检测结果缓存（避免每次进入页面都重复走 NVAPI / WMI 检测）
static GPU_DETECT_CACHE: Mutex<Option<GpuDetection>> = Mutex::new(None);

/// 缓存有效期（2 小时，驱动不会更新得那么频繁）
const CACHE_TTL: Duration = Duration::from_secs(2 * 60 * 60);

/// 获取所有 GeForce 驱动版本（从最新到最旧，仅 Game Ready，不含 Studio）
///
/// 并行查询台式机与笔记本两个通道，合并后按版本号去重排序；
/// 单个系列查询失败会自动重试，最终只要有一个系列成功就返回结果。
///
/// 结果会缓存 2 小时（内存 + 磁盘两层）：
/// - 内存缓存：本次运行期间重复进入页面直接返回；
/// - 磁盘缓存：重启应用后首次进入也直接返回（无需等待网络）；
/// - 磁盘缓存过期时：先立即返回旧数据，同时在后台刷新，刷新完成
///   后通过 "nvidia-drivers-updated" 事件推送新数据，用户无需空等；
/// - 传入 force_refresh=true 时强制重新扫描（同步等待）。
#[tauri::command]
pub async fn fetch_nvidia_drivers(
    app: AppHandle,
    force_refresh: Option<bool>,
) -> Result<Vec<DriverEntry>, String> {
    let force = force_refresh == Some(true);

    // 1) 内存缓存（新鲜）直接返回
    if !force {
        if let Ok(guard) = DRIVER_CACHE.lock() {
            if let Some(cache) = guard.as_ref() {
                if cache
                    .fetched_at
                    .elapsed()
                    .map(|d| d < CACHE_TTL)
                    .unwrap_or(false)
                {
                    return Ok(cache.entries.clone());
                }
            }
        }
    }

    // 2) 磁盘缓存
    if !force {
        if let Some((fetched_at, entries)) = load_disk_cache(&app) {
            if fetched_at
                .elapsed()
                .map(|d| d < CACHE_TTL)
                .unwrap_or(false)
            {
                // 磁盘缓存仍新鲜：写入内存缓存后直接返回
                set_memory_cache(entries.clone());
                return Ok(entries);
            }
            // 磁盘缓存已过期：先返回旧数据，后台刷新完成后推送事件
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                match do_fetch_drivers(&app).await {
                    Ok(entries) => {
                        let _ = app.emit("nvidia-drivers-updated", &entries);
                    }
                    Err(e) => log::warn!("后台刷新驱动列表失败: {e}"),
                }
            });
            return Ok(entries);
        }
    }

    // 3) 无缓存或强制刷新：同步拉取
    do_fetch_drivers(&app).await
}

/// 实际执行网络拉取 + 合并排序 + 写入缓存（内存 + 磁盘）
async fn do_fetch_drivers(app: &AppHandle) -> Result<Vec<DriverEntry>, String> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENCY));

    let mut handles = Vec::new();
    for series in SERIES_TO_FETCH {
        for is_laptop in [false, true] {
            let (psid, pfid) = if is_laptop {
                (series.laptop_psid, series.laptop_pfid)
            } else {
                (series.desktop_psid, series.desktop_pfid)
            };
            let client = client.clone();
            let semaphore = semaphore.clone();
            handles.push(tokio::spawn(async move {
                let _permit = semaphore.acquire_owned().await;
                let result = fetch_series_drivers_with_retry(&client, psid, pfid).await;
                (is_laptop, result)
            }));
        }
    }

    let mut desktop_map: HashMap<String, DriverClassInfo> = HashMap::new();
    let mut laptop_map: HashMap<String, DriverClassInfo> = HashMap::new();
    let mut meta_map: HashMap<String, (String, String, String)> = HashMap::new();
    let mut any_success = false;

    for handle in handles {
        match handle.await {
            Ok((is_laptop, Ok(list))) => {
                any_success = true;
                for d in list {
                    let info = DriverClassInfo {
                        id: d.class_info.id,
                        detail_url: d.class_info.detail_url,
                        download_url: d.class_info.download_url,
                    };
                    let map = if is_laptop {
                        &mut laptop_map
                    } else {
                        &mut desktop_map
                    };
                    map.entry(d.version.clone()).or_insert(info);
                    meta_map
                        .entry(d.version.clone())
                        .or_insert((d.name, d.release_date, d.branch));
                }
            }
            Ok((_, Err(e))) => log::warn!("获取 GeForce 驱动列表失败: {}", e),
            Err(e) => log::warn!("驱动列表查询任务失败: {}", e),
        }
    }

    if !any_success {
        return Err("未能获取到任何驱动版本，NVIDIA 官网接口可能暂时不可用".into());
    }

    // 合并两个通道的版本号，从新到旧排序
    let mut versions: Vec<String> = desktop_map
        .keys()
        .chain(laptop_map.keys())
        .cloned()
        .collect();
    versions.sort_by(|a, b| version_cmp(&b, &a));
    versions.dedup();

    let mut entries: Vec<DriverEntry> = Vec::new();
    for v in versions {
        let (name, release_date, branch) = meta_map.get(&v).cloned().unwrap_or_default();
        // 只保留 Game Ready（Studio 在拉取阶段已过滤，这里再兜底一次）
        if branch != "GRD" {
            continue;
        }
        entries.push(DriverEntry {
            version: v.clone(),
            branch,
            release_date,
            name,
            is_latest_only: false,
            desktop: desktop_map.get(&v).cloned(),
            laptop: laptop_map.get(&v).cloned(),
        });
    }

    if entries.is_empty() {
        return Err("未能获取到任何驱动版本，NVIDIA 官网接口可能已变更".into());
    }

    entries[0].is_latest_only = true;

    // 写入缓存（内存 + 磁盘）
    set_memory_cache(entries.clone());
    save_disk_cache(app, &entries);

    Ok(entries)
}

/// 驱动列表磁盘缓存结构
#[derive(Serialize, Deserialize)]
struct DiskDriverCache {
    fetched_at_unix: u64,
    entries: Vec<DriverEntry>,
}

fn disk_cache_path(app: &AppHandle) -> std::path::PathBuf {
    app.path()
        .app_data_dir()
        .map(|d| d.join("nvidia_driver_cache.json"))
        .unwrap_or_else(|_| std::path::PathBuf::from("nvidia_driver_cache.json"))
}

/// 从磁盘读取驱动列表缓存，返回「获取时刻 + 列表」
fn load_disk_cache(app: &AppHandle) -> Option<(SystemTime, Vec<DriverEntry>)> {
    let path = disk_cache_path(app);
    let content = std::fs::read_to_string(&path).ok()?;
    let disk: DiskDriverCache = serde_json::from_str(&content).ok()?;
    let fetched_at = std::time::UNIX_EPOCH + Duration::from_secs(disk.fetched_at_unix);
    Some((fetched_at, disk.entries))
}

/// 写入磁盘缓存
fn save_disk_cache(app: &AppHandle, entries: &[DriverEntry]) {
    let path = disk_cache_path(app);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let disk = DiskDriverCache {
        fetched_at_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        entries: entries.to_vec(),
    };
    if let Ok(json) = serde_json::to_string(&disk) {
        let _ = std::fs::write(&path, json);
    }
}

/// 写入内存缓存
fn set_memory_cache(entries: Vec<DriverEntry>) {
    if let Ok(mut guard) = DRIVER_CACHE.lock() {
        *guard = Some(DriverCache {
            entries,
            fetched_at: SystemTime::now(),
        });
    }
}

/// 带重试的系列查询：最多尝试 3 次，间隔 1s / 2s
async fn fetch_series_drivers_with_retry(
    client: &reqwest::Client,
    psid: u32,
    pfid: u32,
) -> Result<Vec<ClassDriver>, String> {
    let mut last_err = String::new();
    for attempt in 0..3 {
        match fetch_series_drivers(client, psid, pfid).await {
            Ok(list) => return Ok(list),
            Err(e) => {
                last_err = e;
                let delay = std::time::Duration::from_millis(500 * (attempt as u64 + 1) * 2);
                tokio::time::sleep(delay).await;
            }
        }
    }
    Err(last_err)
}

/// 查询单个系列的最新驱动列表
async fn fetch_series_drivers(
    client: &reqwest::Client,
    psid: u32,
    pfid: u32,
) -> Result<Vec<ClassDriver>, String> {
    let url = format!(
        "https://gfwsl.geforce.com/services_toolkit/services/com/nvidia/services/AjaxDriverService.php?func=DriverManualLookup&psid={}&pfid={}&osID={}&languageID=1&dch=1&upCRD=0&qnf=0&sort1=0&txr=0&searchString=doNotPickle&numberOfResults={}&dltype=1",
        psid, pfid, OSID_WIN10_11_64BIT, API_RESULT_LIMIT
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("请求 NVIDIA API 失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("NVIDIA API 返回错误状态码: {}", resp.status()));
    }

    let api_data: ApiDriverResponse = resp
        .json()
        .await
        .map_err(|e| format!("解析 NVIDIA API 响应失败: {}", e))?;

    let mut drivers: Vec<ClassDriver> = Vec::new();
    for item in api_data.ids {
        let info = item.download_info;
        if info.id.is_empty() || info.version.is_empty() {
            continue;
        }

        let name = urlencoding::decode(&info.name)
            .map(|s| s.to_string())
            .unwrap_or_else(|_| info.name.clone());

        let branch = if info.is_crd == "1" || name.to_lowercase().contains("studio") {
            "Studio"
        } else {
            "GRD"
        }
        .to_string();

        let detail_url = format!(
            "https://www.nvidia.cn/Download/driverResults.aspx/{}/zh-cn",
            info.id
        );

        drivers.push(ClassDriver {
            version: info.version,
            name,
            branch,
            release_date: format_release_date(&info.release_date_time),
            class_info: DriverClassInfo {
                id: info.id,
                detail_url,
                download_url: info.download_url,
            },
        });
    }

    Ok(drivers)
}

/// 自动检测当前系统 NVIDIA 显卡
#[tauri::command]
pub async fn detect_current_nvidia_gpu() -> Result<Option<GpuDetection>, String> {
    // 命中缓存直接返回，避免重复检测
    if let Ok(guard) = GPU_DETECT_CACHE.lock() {
        if let Some(d) = guard.as_ref() {
            return Ok(Some(d.clone()));
        }
    }

    let (gpu_name, driver_version) = match crate::nvapi::get_nvidia_driver_version() {
        Ok(info) => {
            let name = if info.gpu_name.is_empty() || info.gpu_name == "NVIDIA GPU" {
                String::new()
            } else {
                info.gpu_name.clone()
            };
            let ver = if info.version > 0 {
                format!("{}.{:02}", info.version / 100, info.version % 100)
            } else {
                String::new()
            };
            (name, ver)
        }
        Err(_) => return Ok(None),
    };

    if gpu_name.is_empty() {
        return Ok(None);
    }

    let is_laptop = detect_is_laptop() || gpu_name.to_lowercase().contains("laptop");

    let gpu_lower = gpu_name.to_uppercase();
    let matched_series = GEFORCE_SERIES
        .iter()
        .find(|s| {
            let series_upper = s.name.to_uppercase();
            if let Some(num) = extract_series_number(&series_upper) {
                gpu_lower.contains(&format!("RTX {}", num))
                    || gpu_lower.contains(&format!("GTX {}", num))
            } else {
                false
            }
        })
        .map(|s| s.name.to_string());

    let detection = GpuDetection {
        gpu_name,
        series_name: matched_series.unwrap_or_default(),
        is_laptop,
        driver_version,
    };

    // 写入缓存
    if let Ok(mut guard) = GPU_DETECT_CACHE.lock() {
        *guard = Some(detection.clone());
    }

    Ok(Some(detection))
}

/// 从系列名中提取数字代号（如 "RTX 40 系列" → "40"）
fn extract_series_number(name: &str) -> Option<String> {
    let re = Regex::new(r"(?:RTX|GTX)\s*(\d+)").ok()?;
    re.captures(name)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// 将 "Tue Jul 28, 2026" 格式化为 "2026-07-28"
fn format_release_date(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(trimmed, "%a %b %d, %Y") {
        return date.format("%Y-%m-%d").to_string();
    }
    trimmed.to_string()
}

/// 版本号比较（如 610.88 > 581.42）
fn version_parts(v: &str) -> (u32, u32) {
    let mut parts = v.split('.');
    let major = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (major, minor)
}

fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    version_parts(a).cmp(&version_parts(b))
}

/// 检测当前设备是否为笔记本（快速 WMI 直调，避免启动 PowerShell 的开销）
fn detect_is_laptop() -> bool {
    // PCSystemType：0=未指定 1=台式机 2=移动设备(笔记本) 3=工作站
    // 8=Slate 9=Convertible 10=Detachable（平板/二合一同样按笔记本处理）
    const LAPTOP_CODES: [u16; 4] = [2, 8, 9, 10];
    if let Ok(rows) =
        crate::wmi_query::wmi_query("SELECT PCSystemType FROM Win32_ComputerSystem")
    {
        if let Some(row) = rows.first() {
            if let Some(code) = row
                .get("PCSystemType")
                .and_then(|v| crate::wmi_query::v_u16(v))
            {
                if LAPTOP_CODES.contains(&code) {
                    return true;
                }
                // 明确识别为台式机则直接返回
                if code == 1 {
                    return false;
                }
            }
        }
    }
    // 兜底：存在电池（Win32_Battery 非空）也视为笔记本
    crate::wmi_query::wmi_query("SELECT Name FROM Win32_Battery")
        .map(|rows| !rows.is_empty())
        .unwrap_or(false)
}
