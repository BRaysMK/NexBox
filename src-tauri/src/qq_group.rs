use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tauri::Manager;
use tokio::sync::RwLock;

/// 官方 QQ 群配置文件地址（gitee 仓库 muliuawa/nexbox，与 notice.json / sponsors.json 同目录）
const QQ_GROUPS_URL: &str = "https://gitee.com/muliuawa/nexbox/raw/master/qq_groups.json";
const CONNECT_TIMEOUT_SECS: u64 = 3;
const REQUEST_TIMEOUT_SECS: u64 = 6;
/// 内存缓存时长，避免每次打开弹窗都请求 gitee
const MEMORY_CACHE_TTL_SECS: u64 = 600;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QqGroup {
    /// 群名，如 "①群"
    pub name: String,
    /// QQ 群号
    pub number: String,
    /// 加群链接（qm.qq.com），为空时前端退化为复制群号
    pub link: String,
    /// 群图标 URL（可放 gitee 仓库），为空时前端使用默认 QQ 图标
    #[serde(default)]
    pub icon: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QqGroupResponse {
    pub update_time: String,
    pub groups: Vec<QqGroup>,
}

/// 内置兜底数据：gitee 拉取失败/为空时使用，保证功能可用
fn default_groups() -> Vec<QqGroup> {
    vec![
        QqGroup {
            name: "①群".to_string(),
            number: "526045683".to_string(),
            link: "https://qm.qq.com/q/atlGEA2tQk".to_string(),
            icon: String::new(),
        },
        QqGroup {
            name: "②群".to_string(),
            number: "957472962".to_string(),
            link: String::new(),
            icon: String::new(),
        },
    ]
}

struct MemoryCache {
    data: Option<Vec<QqGroup>>,
    fetched_at: Option<Instant>,
}

impl MemoryCache {
    fn new() -> Self {
        Self {
            data: None,
            fetched_at: None,
        }
    }

    fn get(&self) -> Option<Vec<QqGroup>> {
        if let (Some(data), Some(fetched_at)) = (&self.data, &self.fetched_at) {
            if fetched_at.elapsed() < Duration::from_secs(MEMORY_CACHE_TTL_SECS) {
                return Some(data.clone());
            }
        }
        None
    }

    fn set(&mut self, data: Vec<QqGroup>) {
        self.data = Some(data);
        self.fetched_at = Some(Instant::now());
    }
}

static MEMORY_CACHE: OnceLock<Arc<RwLock<MemoryCache>>> = OnceLock::new();

fn get_memory_cache() -> Arc<RwLock<MemoryCache>> {
    MEMORY_CACHE
        .get_or_init(|| Arc::new(RwLock::new(MemoryCache::new())))
        .clone()
}

async fn fetch_qq_groups() -> Result<Vec<QqGroup>, String> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(QQ_GROUPS_URL)
        .send()
        .await
        .map_err(|e| format!("Network request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()));
    }

    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    let data: QqGroupResponse =
        serde_json::from_str(&text).map_err(|e| format!("JSON parse error: {}", e))?;

    let groups = if data.groups.is_empty() {
        // gitee 文件为空时回退内置数据
        default_groups()
    } else {
        data.groups
    };

    let cache = get_memory_cache();
    cache.write().await.set(groups.clone());

    Ok(groups)
}

/// 获取官方 QQ 群列表：优先内存缓存，其次 gitee 配置，最后内置兜底
#[tauri::command]
pub async fn get_qq_groups() -> Vec<QqGroup> {
    {
        let cache = get_memory_cache();
        if let Some(data) = cache.read().await.get() {
            return data;
        };
    }

    match fetch_qq_groups().await {
        Ok(data) => data,
        Err(e) => {
            log::warn!("Failed to fetch QQ groups: {e}, using default groups");
            default_groups()
        }
    }
}

/// 后端下载群图标到应用缓存，返回本地文件路径（前端用 convertFileSrc 转换成可显示地址）。
/// 这样不依赖 WebView 直连 gitee（WebView 通常加载不了 gitee raw 图），图标仍来自 gitee（远程）。
#[tauri::command]
pub async fn get_qq_group_icon(app: tauri::AppHandle, url: String) -> Result<String, String> {
    if url.trim().is_empty() {
        return Ok(String::new());
    }

    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("qq_icons");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let file = dir.join(format!("{}.img", hasher.finish()));

    if !file.exists() {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(20))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 NexBox")
            .build()
            .map_err(|e| format!("client error: {e}"))?;

        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("network error: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("icon http {}", resp.status()));
        }
        let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
        std::fs::write(&file, &bytes).map_err(|e| e.to_string())?;
    }

    Ok(file.to_string_lossy().to_string())
}