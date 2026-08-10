use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
use winreg::enums::*;
#[cfg(target_os = "windows")]
use winreg::RegKey;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GpuInfo {
    /// 显卡当前名称
    pub name: String,
    /// 是否为核显（集成显卡）
    pub is_integrated: bool,
    /// 备份的原始名称（已备份时才有）
    pub original_name: Option<String>,
    /// 是否有可恢复的备份
    pub is_backed_up: bool,
    /// 显卡在 Enum\PCI 下的相对路径（vendor\device 格式），用于精确改写指定显卡
    pub key_path: String,
}

/// 单张显卡的备份记录（按注册表 key_path 精确区分核显/独显）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GpuBackupEntry {
    pub key_path: String,
    pub original_name: String,
    pub is_integrated: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GpuRenameResult {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GpuOption {
    pub id: String,
    pub name: String,
    pub category: String,
}

fn get_appdata_backup_path() -> Result<PathBuf, String> {
    let appdata_dir = dirs::config_dir()
        .ok_or("无法获取 APPDATA 目录")?
        .join("NexBox");
    std::fs::create_dir_all(&appdata_dir)
        .map_err(|e| format!("创建数据目录失败: {}", e))?;
    Ok(appdata_dir.join("gpu_rename_backup.json"))
}

fn get_install_dir_backup_path() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(|p| p.join("gpu_rename_backup.json"))
}

/// 备份数据：新版为多显卡数组；旧版为单个对象（兼容迁移）
enum BackupData {
    Multi(Vec<GpuBackupEntry>),
    /// 旧版单条备份（仅记录一个原始名称）
    Legacy(String),
}

fn read_backup_from(path: &std::path::Path) -> Option<BackupData> {
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    // 新版：多显卡数组
    if let Ok(entries) = serde_json::from_str::<Vec<GpuBackupEntry>>(&content) {
        if !entries.is_empty() {
            return Some(BackupData::Multi(entries));
        }
    }
    // 旧版：单个对象，仅提取原始名称
    #[derive(serde::Deserialize)]
    struct LegacyGpuInfo {
        original_name: String,
    }
    if let Ok(info) = serde_json::from_str::<LegacyGpuInfo>(&content) {
        return Some(BackupData::Legacy(info.original_name));
    }
    None
}

fn save_backup(entries: &[GpuBackupEntry]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(entries)
        .map_err(|e| format!("序列化备份数据失败: {}", e))?;

    // 写入 %APPDATA%/NexBox/ — 持久保留
    let appdata_path = get_appdata_backup_path()?;
    fs::write(&appdata_path, &json)
        .map_err(|e| format!("写入备份文件失败: {}", e))?;

    // 同时也写一份到安装目录 — 方便用户直接查看
    if let Some(install_path) = get_install_dir_backup_path() {
        let _ = fs::write(&install_path, &json);
    }

    Ok(())
}

fn load_backup() -> Result<Option<BackupData>, String> {
    // 优先从 %APPDATA% 读取
    if let Ok(appdata_path) = get_appdata_backup_path() {
        if let Some(info) = read_backup_from(&appdata_path) {
            return Ok(Some(info));
        }
    }

    // 回退到安装目录（兼容旧版本）
    if let Some(install_path) = get_install_dir_backup_path() {
        if let Some(info) = read_backup_from(&install_path) {
            // 自动迁移到 %APPDATA%（保持原始内容，旧版格式无需转换）
            if let Ok(appdata_path) = get_appdata_backup_path() {
                if let Ok(content) = std::fs::read_to_string(&install_path) {
                    let _ = std::fs::write(&appdata_path, content);
                }
            }
            return Ok(Some(info));
        }
    }

    Ok(None)
}

#[cfg(target_os = "windows")]
fn find_gpu_registry_keys() -> Result<Vec<(RegKey, String, bool)>, String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let enum_key = hklm.open_subkey("SYSTEM\\CurrentControlSet\\Enum\\PCI")
        .map_err(|e| format!("打开注册表键失败: {}", e))?;
    
    let mut gpu_keys = Vec::new();
    
    // 支持的显卡厂商 PCI Vendor ID: NVIDIA(10DE)、AMD(1002)、Intel(8086)
    let supported_vendors = ["VEN_10DE", "VEN_1002", "VEN_8086"];
    // 排除关键词：USB控制器等非显卡设备、Microsoft 基础显示适配器（未安装驱动的占位设备）
    let exclude_keywords = ["usb", "controller", "控制器", "host", "xhci", "ehci", "uhci", "chipset", "smbus", "audio", "sound", "basic display"];
    // 显卡名称关键词（NVIDIA / AMD / Intel）
    let gpu_keywords = ["nvidia", "geforce", "gtx", "rtx", "amd", "radeon", "intel", "uhd graphics", "iris", "hd graphics"];
    
    for vendor_result in enum_key.enum_keys() {
        let vendor_key_name = match vendor_result {
            Ok(name) => name,
            Err(_) => continue,
        };
        let vendor_key = match enum_key.open_subkey(&vendor_key_name) {
            Ok(key) => key,
            Err(_) => continue,
        };
        
        // 只处理 NVIDIA / AMD 厂商
        let vendor_upper = vendor_key_name.to_uppercase();
        if !supported_vendors.iter().any(|v| vendor_upper.contains(v)) {
            continue;
        }
        
        for device_result in vendor_key.enum_keys() {
            let device_key_name = match device_result {
                Ok(name) => name,
                Err(_) => continue,
            };
            let device_key = match vendor_key.open_subkey(&device_key_name) {
                Ok(key) => key,
                Err(_) => continue,
            };
            let key_path = vendor_key_name.clone() + "\\" + &device_key_name;
            
            // 排除 USB 控制器等非显卡设备
            let mut is_excluded = false;
            if let Ok(device_desc) = device_key.get_value::<String, _>("DeviceDesc") {
                let lower = device_desc.to_lowercase();
                if exclude_keywords.iter().any(|kw| lower.contains(kw)) {
                    is_excluded = true;
                }
            }
            if !is_excluded {
                if let Ok(friendly_name) = device_key.get_value::<String, _>("FriendlyName") {
                    let lower = friendly_name.to_lowercase();
                    if exclude_keywords.iter().any(|kw| lower.contains(kw)) {
                        is_excluded = true;
                    }
                }
            }
            if is_excluded {
                continue;
            }
            
            // 判断是否为显卡：
            // 1. 优先按 ClassGUID（显卡类 {4d36e968-...}）判断，NVIDIA / AMD 通用
            // 2. 回退按名称关键词判断（AMD 的 DeviceDesc 通常是硬件路径，需靠 FriendlyName 兜底）
            let mut is_gpu = false;
            if let Ok(class_guid) = device_key.get_value::<String, _>("ClassGUID") {
                let normalized = class_guid
                    .trim()
                    .trim_start_matches('{')
                    .trim_end_matches('}')
                    .to_uppercase();
                if normalized == "4D36E968-E325-11CE-BFC1-08002BE10318" {
                    is_gpu = true;
                }
            }
            if !is_gpu {
                if let Ok(class) = device_key.get_value::<String, _>("Class") {
                    if class.eq_ignore_ascii_case("Display") {
                        is_gpu = true;
                    }
                }
            }
            if !is_gpu {
                if let Ok(device_desc) = device_key.get_value::<String, _>("DeviceDesc") {
                    let lower = device_desc.to_lowercase();
                    if gpu_keywords.iter().any(|kw| lower.contains(kw)) {
                        is_gpu = true;
                    }
                }
            }
            if !is_gpu {
                if let Ok(friendly_name) = device_key.get_value::<String, _>("FriendlyName") {
                    let lower = friendly_name.to_lowercase();
                    if gpu_keywords.iter().any(|kw| lower.contains(kw)) {
                        is_gpu = true;
                    }
                }
            }
            
            if is_gpu {
                // 通过 LocationInformation 判断是否为核显（集成显卡）
                // 核显的 LocationInformation 通常包含 "Internal Graphics" 或 "on board"
                let is_integrated = check_is_integrated(&device_key, &key_path);
                log::debug!(
                    "显卡注册表: {} is_integrated={}",
                    key_path, is_integrated
                );
                gpu_keys.push((device_key, key_path, is_integrated));
            }
        }
    }
    
    Ok(gpu_keys)
}

/// 根据显卡名称判断是否为核显（Intel 核显 / AMD APU 集成显卡）
#[cfg(target_os = "windows")]
fn is_integrated_by_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    // 明确的中文标记
    if lower.contains("核显") {
        return true;
    }
    // Intel 核显特征：UHD Graphics / HD Graphics / Iris，排除 Intel Arc 独显
    if lower.contains("uhd") || lower.contains("hd graphics") || lower.contains("iris") {
        return true;
    }
    if lower.contains("intel") && lower.contains("graphics") && !lower.contains("arc") {
        return true;
    }
    // AMD APU 核显：Radeon 系列中，含 Graphics 或 Vega 的是核显（APU），
    // 仅 RX 系列是独显。注意 Vega 在 APU 上是核显（如 Vega 8 Graphics），
    // 680M/780M 等带数字的也是 APU 核显——切勿把数字/Vega 当作独显特征。
    if lower.contains("radeon") {
        let is_rx_discrete = lower.contains("rx");
        if !is_rx_discrete && (lower.contains("graphics") || lower.contains("vega")) {
            return true;
        }
    }
    false
}

/// 检查 GPU 是否为核显（集成显卡）
/// 判断优先级：
/// 1. 名称特征判断（Intel UHD/HD/Iris、AMD APU Graphics、中文"核显"）
/// 2. LocationInformation 判断（核显通常标记为 "Internal Graphics" / "on board"）
/// 3. Vendor ID 兜底：若 vendor 为 Intel(8086) 且名称不含 "Arc"，则视为核显
///    （Intel 独显仅有 Arc 系列，其余 Intel 显卡均为核显。
///     此兜底用于应对核显被旧版改名工具改成独显名字后名称判断失效的情况。）
///
/// 注意：不能依据 LocationInformation 是否包含 "bus 0" 来判断 ——
/// 独立显卡的 LocationInformation 通常是 "PCI bus 0, device X, function Y"，同样包含 "bus 0"，
/// 此前该判断会把独显误判为核显，导致显卡列表中只剩核显。
#[cfg(target_os = "windows")]
fn check_is_integrated(device_key: &RegKey, key_path: &str) -> bool {
    let key_path_upper = key_path.to_uppercase();

    // 1. 名称特征判断（Intel UHD/HD/Iris、AMD APU Graphics、中文"核显"）
    let read_name = |key: &RegKey| -> Option<String> {
        if let Ok(name) = key.get_value::<String, _>("FriendlyName") {
            return Some(name);
        }
        key.get_value::<String, _>("DeviceDesc").ok()
    };
    if let Some(name) = read_name(device_key) {
        if is_integrated_by_name(&name) {
            log::debug!("检测到核显(名称): {} ({})", key_path, name);
            return true;
        }
    }

    // 2. LocationInformation 判断（核显通常标记为 "Internal Graphics" / "on board"）
    for instance_result in device_key.enum_keys() {
        let instance_name = match instance_result {
            Ok(name) => name,
            Err(_) => continue,
        };
        if let Ok(instance_key) = device_key.open_subkey(&instance_name) {
            if let Ok(location) = instance_key.get_value::<String, _>("LocationInformation") {
                let lower = location.to_lowercase();
                if lower.contains("internal graphics")
                    || lower.contains("on board")
                    || lower.contains("internal")
                {
                    log::debug!(
                        "检测到核显: {} LocationInformation={}",
                        key_path, location
                    );
                    return true;
                }
            }
        }
    }

    // 3. Vendor ID 兜底：Intel(8086) 非 Arc 独显即为核显
    //    用于应对核显被旧版改名工具改成独显名字后名称判断失效的情况
    if key_path_upper.contains("VEN_8086") {
        let is_arc_by_name = read_name(device_key)
            .map(|n| n.to_lowercase().contains("arc"))
            .unwrap_or(false);
        if !is_arc_by_name {
            log::debug!(
                "检测到核显(Vendor兜底): {} (Intel 非 Arc)",
                key_path
            );
            return true;
        }
    }

    false
}

/// 从注册表键读取显卡名称（FriendlyName 优先，回退 DeviceDesc）
#[cfg(target_os = "windows")]
fn read_gpu_name(key: &RegKey) -> Option<String> {
    if let Ok(name) = key.get_value::<String, _>("FriendlyName") {
        return Some(name);
    }
    if let Ok(name) = key.get_value::<String, _>("DeviceDesc") {
        let parts: Vec<&str> = name.split(';').collect();
        return Some(if parts.len() > 1 { parts[1].to_string() } else { name });
    }
    None
}

/// 列出全部显卡（核显 + 独显），独显排前，并关联备份的原始名称。
#[cfg(target_os = "windows")]
fn get_gpu_list_inner() -> Result<Vec<GpuInfo>, String> {
    let gpu_keys = find_gpu_registry_keys()?;
    if gpu_keys.is_empty() {
        return Err("未找到显卡注册表信息".to_string());
    }

    // 备份映射：key_path -> original_name
    let backup = load_backup()?;
    let mut backup_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if let Some(BackupData::Multi(entries)) = &backup {
        for e in entries {
            backup_map.insert(e.key_path.clone(), e.original_name.clone());
        }
    }

    let mut list: Vec<GpuInfo> = Vec::new();
    for (key, key_path, is_integrated) in &gpu_keys {
        let Some(name) = read_gpu_name(key) else {
            continue;
        };
        // 双保险：跳过 Microsoft 基础显示适配器（未安装驱动的占位设备）
        if name.to_lowercase().contains("basic display") {
            log::info!("跳过基础显示适配器(列表): {} ({})", name, key_path);
            continue;
        }
        let original_name = backup_map.get(key_path).cloned();
        list.push(GpuInfo {
            name,
            is_integrated: *is_integrated,
            original_name: original_name.clone(),
            is_backed_up: original_name.is_some(),
            key_path: key_path.clone(),
        });
    }

    // 旧版备份（Legacy）无法按 key_path 映射：视为整体已备份，仅主显卡可恢复
    if matches!(backup, Some(BackupData::Legacy(_))) {
        for g in list.iter_mut() {
            g.is_backed_up = true;
        }
    }

    // 独显在前，核显在后；同类型按原名
    list.sort_by(|a, b| {
        a.is_integrated
            .cmp(&b.is_integrated)
            .then_with(|| a.name.cmp(&b.name))
    });

    Ok(list)
}

#[cfg(not(target_os = "windows"))]
fn get_gpu_list_inner() -> Result<Vec<GpuInfo>, String> {
    Err("此功能仅支持 Windows 系统".to_string())
}

#[cfg(target_os = "windows")]
fn rename_gpu(new_name: &str, target_key_path: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    log::info!(
        "开始修改显卡 {} 的名称为: {}",
        target_key_path, new_name
    );

    // PowerShell 单引号字符串中只需转义单引号（' -> ''）
    let escaped_name = new_name.replace('\'', "''");
    let escaped_target = target_key_path.replace('\'', "''");

    // 策略：只改写 target_key_path 指定的显卡，避免影响其他显卡
    // 1. Enum\PCI：通过 vendor\device 精确匹配 target_key_path
    // 2. Class：通过 MatchingDeviceId 与目标显卡一致来匹配
    // 3. Video：通过当前名称与目标显卡当前名称一致来匹配
    let ps_script = format!(
        r#"
$ErrorActionPreference = 'Continue'
$modified = $false
$targetKeyPath = '{target}'
$newName = '{name}'

$targetEnumPath = "HKLM:\SYSTEM\CurrentControlSet\Enum\PCI\$targetKeyPath"
if (-not (Test-Path $targetEnumPath)) {{
    Write-Host "FAILED: 目标显卡路径不存在: $targetEnumPath"
    exit 1
}}

# 读取目标显卡的 MatchingDeviceId 和当前 FriendlyName，用于 Class/Video 精确匹配
$targetMatchingId = (Get-ItemProperty -Path $targetEnumPath -Name 'MatchingDeviceId' -ErrorAction SilentlyContinue).MatchingDeviceId
$targetCurrentName = (Get-ItemProperty -Path $targetEnumPath -Name 'FriendlyName' -ErrorAction SilentlyContinue).FriendlyName
if (-not $targetCurrentName) {{
    $desc = (Get-ItemProperty -Path $targetEnumPath -Name 'DeviceDesc' -ErrorAction SilentlyContinue).DeviceDesc
    if ($desc) {{
        $parts = $desc -split ';', 2
        $targetCurrentName = if ($parts.Count -gt 1) {{ $parts[1] }} else {{ $desc }}
    }}
}}
Write-Host "目标显卡: $targetKeyPath"
Write-Host "目标 MatchingDeviceId: $targetMatchingId"
Write-Host "目标当前名称: $targetCurrentName"

# 1. 修改 Enum\PCI 下目标显卡
try {{
    Set-ItemProperty -Path $targetEnumPath -Name 'FriendlyName' -Value $newName -ErrorAction Stop
    Write-Host "成功修改 FriendlyName"
    $modified = $true
}} catch {{
    Write-Host "修改 FriendlyName 失败: $_"
}}
try {{
    $deviceDesc = (Get-ItemProperty -Path $targetEnumPath -Name 'DeviceDesc' -ErrorAction SilentlyContinue).DeviceDesc
    if ($deviceDesc) {{
        $parts = $deviceDesc -split ';', 2
        if ($parts.Count -gt 1) {{
            $newDesc = "$($parts[0]);$newName"
            Set-ItemProperty -Path $targetEnumPath -Name 'DeviceDesc' -Value $newDesc -ErrorAction Stop
            Write-Host "成功修改 DeviceDesc"
            $modified = $true
        }}
    }}
}} catch {{
    Write-Host "修改 DeviceDesc 失败: $_"
}}

# 2. 修改 Class 下匹配的显卡键（通过 MatchingDeviceId 精确匹配）
try {{
    $classPath = "HKLM:\SYSTEM\CurrentControlSet\Control\Class\{{4d36e968-e325-11ce-bfc1-08002be10318}}"
    if (Test-Path $classPath) {{
        $subkeys = Get-ChildItem $classPath
        foreach ($subkey in $subkeys) {{
            if ($subkey.PSChildName -match "^00\d+") {{
                try {{
                    $mid = (Get-ItemProperty -Path $subkey.PSPath -Name 'MatchingDeviceId' -ErrorAction SilentlyContinue).MatchingDeviceId
                    if ($mid -and $targetMatchingId -and ($mid -eq $targetMatchingId)) {{
                        $driverDesc = (Get-ItemProperty -Path $subkey.PSPath -Name 'DriverDesc' -ErrorAction SilentlyContinue).DriverDesc
                        Write-Host "找到目标显卡 Class 键: $($subkey.PSChildName) DriverDesc: $driverDesc"
                        Set-ItemProperty -Path $subkey.PSPath -Name 'DriverDesc' -Value $newName -ErrorAction Stop
                        Write-Host "成功修改 DriverDesc"
                        $modified = $true
                    }}
                }} catch {{
                    Write-Host "处理 Class 键失败: $_"
                }}
            }}
        }}
    }}
}} catch {{
    Write-Host "Class 处理失败: $_"
}}

# 3. 修改 Control\Video 下匹配的显卡键（仅当某字段等于目标显卡当前名称时才改写）
try {{
    $videoPath = "HKLM:\SYSTEM\CurrentControlSet\Control\Video"
    if (Test-Path $videoPath) {{
        $videoKeys = Get-ChildItem $videoPath
        foreach ($videoKey in $videoKeys) {{
            $subkeys = Get-ChildItem $videoKey.PSPath -ErrorAction SilentlyContinue
            foreach ($subkey in $subkeys) {{
                try {{
                    $keyPath = $subkey.PSPath
                    $driverDesc = (Get-ItemProperty -Path $keyPath -Name 'DriverDesc' -ErrorAction SilentlyContinue).DriverDesc
                    $deviceDesc = (Get-ItemProperty -Path $keyPath -Name 'DeviceDesc' -ErrorAction SilentlyContinue).DeviceDesc
                    $description = (Get-ItemProperty -Path $keyPath -Name 'Description' -ErrorAction SilentlyContinue).Description
                    $friendlyName = (Get-ItemProperty -Path $keyPath -Name 'FriendlyName' -ErrorAction SilentlyContinue).FriendlyName

                    $isTarget = $false
                    foreach ($v in @($driverDesc, $deviceDesc, $description, $friendlyName)) {{
                        if ($v -and $targetCurrentName -and ($v -eq $targetCurrentName)) {{
                            $isTarget = $true
                            break
                        }}
                    }}
                    if ($isTarget) {{
                        Write-Host "找到目标显卡 Video 键: $($videoKey.PSChildName)\$($subkey.PSChildName)"
                        foreach ($n in @('DriverDesc', 'DeviceDesc', 'Description', 'FriendlyName')) {{
                            try {{
                                $cur = (Get-ItemProperty -Path $keyPath -Name $n -ErrorAction SilentlyContinue).$n
                                if ($cur) {{
                                    Set-ItemProperty -Path $keyPath -Name $n -Value $newName -ErrorAction Stop
                                    Write-Host "成功修改 $n"
                                    $modified = $true
                                }}
                            }} catch {{}}
                        }}
                    }}
                }} catch {{}}
            }}
        }}
    }}
}} catch {{
    Write-Host "Video 处理失败: $_"
}}

if ($modified) {{
    Write-Host "SUCCESS: 显卡名称修改完成！"
    exit 0
}} else {{
    Write-Host "FAILED: 未能修改任何显卡注册表键"
    exit 1
}}
"#,
        target = escaped_target,
        name = escaped_name
    );

    log::info!("执行PowerShell脚本修改注册表");

    let output = Command::new("powershell.exe")
        .args(&["-ExecutionPolicy", "Bypass", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("无法执行PowerShell: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    log::info!("PowerShell输出: {}", stdout);
    if !stderr.is_empty() {
        log::warn!("PowerShell错误: {}", stderr);
    }

    if output.status.success() && stdout.contains("SUCCESS") {
        log::info!("显卡名称修改成功！");
        Ok(())
    } else {
        Err(format!("修改失败: {}", if stderr.is_empty() { stdout } else { stderr }))
    }
}

#[cfg(not(target_os = "windows"))]
fn rename_gpu(_new_name: &str, _target_key_path: &str) -> Result<(), String> {
    Err("此功能仅支持 Windows 系统".to_string())
}

/// 获取全部显卡列表（核显 + 独显，独显在前）
#[tauri::command]
pub async fn get_gpu_list() -> Result<Vec<GpuInfo>, String> {
    get_gpu_list_inner()
}

/// 获取默认显卡（优先独显），用于兼容旧调用方
#[tauri::command]
pub async fn get_gpu_info() -> Result<GpuInfo, String> {
    let list = get_gpu_list_inner()?;
    list.iter()
        .find(|g| !g.is_integrated)
        .or_else(|| list.first())
        .cloned()
        .ok_or_else(|| "未找到显卡".to_string())
}

#[tauri::command]
pub async fn get_gpu_options() -> Result<Vec<GpuOption>, String> {
    Ok(vec![
        // 低端显卡（NVIDIA）
        GpuOption {
            id: "gtx650".to_string(),
            name: "NVIDIA GeForce GTX 650".to_string(),
            category: "low-end".to_string(),
        },
        GpuOption {
            id: "gtx750".to_string(),
            name: "NVIDIA GeForce GTX 750".to_string(),
            category: "low-end".to_string(),
        },
        GpuOption {
            id: "gtx750ti".to_string(),
            name: "NVIDIA GeForce GTX 750 Ti".to_string(),
            category: "low-end".to_string(),
        },
        GpuOption {
            id: "gtx1050".to_string(),
            name: "NVIDIA GeForce GTX 1050".to_string(),
            category: "low-end".to_string(),
        },
        // 低端显卡（AMD）
        GpuOption {
            id: "r7240".to_string(),
            name: "AMD Radeon R7 240".to_string(),
            category: "low-end".to_string(),
        },
        GpuOption {
            id: "rx460".to_string(),
            name: "AMD Radeon RX 460".to_string(),
            category: "low-end".to_string(),
        },
        GpuOption {
            id: "rx560".to_string(),
            name: "AMD Radeon RX 560".to_string(),
            category: "low-end".to_string(),
        },
        GpuOption {
            id: "rx550".to_string(),
            name: "AMD Radeon RX 550".to_string(),
            category: "low-end".to_string(),
        },
        GpuOption {
            id: "rx570".to_string(),
            name: "AMD Radeon RX 570".to_string(),
            category: "low-end".to_string(),
        },
        GpuOption {
            id: "rx580".to_string(),
            name: "AMD Radeon RX 580".to_string(),
            category: "low-end".to_string(),
        },
        GpuOption {
            id: "rx590".to_string(),
            name: "AMD Radeon RX 590".to_string(),
            category: "low-end".to_string(),
        },
        // 高端显卡（NVIDIA）
        GpuOption {
            id: "rtx4080".to_string(),
            name: "NVIDIA GeForce RTX 4080".to_string(),
            category: "high-end".to_string(),
        },
        GpuOption {
            id: "rtx4090".to_string(),
            name: "NVIDIA GeForce RTX 4090".to_string(),
            category: "high-end".to_string(),
        },
        GpuOption {
            id: "rtx5080".to_string(),
            name: "NVIDIA GeForce RTX 5080".to_string(),
            category: "high-end".to_string(),
        },
        GpuOption {
            id: "rtx5090".to_string(),
            name: "NVIDIA GeForce RTX 5090".to_string(),
            category: "high-end".to_string(),
        },
        // 高端显卡（AMD）
        GpuOption {
            id: "rx6700xt".to_string(),
            name: "AMD Radeon RX 6700 XT".to_string(),
            category: "high-end".to_string(),
        },
        GpuOption {
            id: "rx6750gre".to_string(),
            name: "AMD Radeon RX 6750 GRE".to_string(),
            category: "high-end".to_string(),
        },
        GpuOption {
            id: "rx6800".to_string(),
            name: "AMD Radeon RX 6800".to_string(),
            category: "high-end".to_string(),
        },
        GpuOption {
            id: "rx6800xt".to_string(),
            name: "AMD Radeon RX 6800 XT".to_string(),
            category: "high-end".to_string(),
        },
        GpuOption {
            id: "rx6900xt".to_string(),
            name: "AMD Radeon RX 6900 XT".to_string(),
            category: "high-end".to_string(),
        },
        GpuOption {
            id: "rx7600".to_string(),
            name: "AMD Radeon RX 7600".to_string(),
            category: "high-end".to_string(),
        },
        GpuOption {
            id: "rx7700xt".to_string(),
            name: "AMD Radeon RX 7700 XT".to_string(),
            category: "high-end".to_string(),
        },
        GpuOption {
            id: "rx7800xt".to_string(),
            name: "AMD Radeon RX 7800 XT".to_string(),
            category: "high-end".to_string(),
        },
        GpuOption {
            id: "rx7900gre".to_string(),
            name: "AMD Radeon RX 7900 GRE".to_string(),
            category: "high-end".to_string(),
        },
        GpuOption {
            id: "rx7900xt".to_string(),
            name: "AMD Radeon RX 7900 XT".to_string(),
            category: "high-end".to_string(),
        },
        GpuOption {
            id: "rx7900xtx".to_string(),
            name: "AMD Radeon RX 7900 XTX".to_string(),
            category: "high-end".to_string(),
        },
        GpuOption {
            id: "rx9060xt".to_string(),
            name: "AMD Radeon RX 9060 XT".to_string(),
            category: "high-end".to_string(),
        },
        GpuOption {
            id: "rx9070xt".to_string(),
            name: "AMD Radeon RX 9070 XT".to_string(),
            category: "high-end".to_string(),
        },
    ])
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub async fn apply_gpu_rename(
    new_name: String,
    target_key_path: String,
) -> Result<GpuRenameResult, String> {
    if target_key_path.trim().is_empty() {
        return Err("未指定要改写的显卡".to_string());
    }

    let backup = load_backup()?;

    // 首次应用时备份全部显卡的原始名称（按 key_path 区分核显/独显）
    if backup.is_none() {
        let gpu_keys = find_gpu_registry_keys()?;
        let entries: Vec<GpuBackupEntry> = gpu_keys
            .iter()
            .filter_map(|(key, key_path, is_integrated)| {
                read_gpu_name(key).map(|name| GpuBackupEntry {
                    key_path: key_path.clone(),
                    original_name: name,
                    is_integrated: *is_integrated,
                })
            })
            .collect();
        if entries.is_empty() {
            return Err("未找到可备份的显卡".to_string());
        }
        save_backup(&entries)?;
    }

    rename_gpu(&new_name, &target_key_path)?;

    Ok(GpuRenameResult {
        success: true,
        message: format!("显卡名称已更改为: {}", new_name),
    })
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub async fn apply_gpu_rename(
    new_name: String,
    target_key_path: String,
) -> Result<GpuRenameResult, String> {
    let _ = (new_name, target_key_path);
    Err("此功能仅支持 Windows 系统".to_string())
}

/// 按备份逐张恢复 Enum\PCI 下的显卡键，Class/Video 驱动键按 MatchingDeviceId 关联恢复。
#[cfg(target_os = "windows")]
fn restore_gpu_by_entries(entries: &[GpuBackupEntry]) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    // 恢复兜底名称：优先第一张独显的原始名，否则第一条记录（用于 Video 下无法精确匹配的场景）
    let fallback = entries
        .iter()
        .find(|e| !e.is_integrated)
        .or_else(|| entries.first())
        .map(|e| e.original_name.as_str())
        .unwrap_or("")
        .replace('\'', "''");

    let entries_json = serde_json::to_string(entries)
        .map_err(|e| format!("序列化恢复数据失败: {}", e))?;
    // 单引号字符串中只需转义单引号
    let entries_json_escaped = entries_json.replace('\'', "''");

    let ps_script = format!(
        r#"
$ErrorActionPreference = 'Continue'
$fallbackName = '{fallback}'
$entries = '{entries_json}' | ConvertFrom-Json

# 构建 key_path -> original_name 与 matching_id -> original_name 两个映射
$keyPathMap = @{{}}
$matchingIdMap = @{{}}
foreach ($e in $entries) {{
    $keyPathMap[$e.key_path] = $e.original_name
    $enumPath = "HKLM:\SYSTEM\CurrentControlSet\Enum\PCI\$($e.key_path)"
    if (Test-Path $enumPath) {{
        $mid = (Get-ItemProperty -Path $enumPath -Name 'MatchingDeviceId' -ErrorAction SilentlyContinue).MatchingDeviceId
        if ($mid) {{
            $matchingIdMap[$mid] = $e.original_name
        }}
    }}
}}

# 1. 按 key_path 精确恢复 Enum\PCI 下每张显卡
# 注意：$keyPath 必须拼接 vendor\device 两段，与 backup 中存储的 key_path 格式一致
$pciPath = 'HKLM:\SYSTEM\CurrentControlSet\Enum\PCI'
if (Test-Path $pciPath) {{
    foreach ($vendor in (Get-ChildItem $pciPath -ErrorAction SilentlyContinue)) {{
        if ($vendor.PSChildName -notmatch 'VEN_10DE|VEN_1002|VEN_8086') {{ continue }}
        foreach ($device in (Get-ChildItem $vendor.PSPath -ErrorAction SilentlyContinue)) {{
            $keyPath = $vendor.PSChildName + '\' + $device.PSChildName
            if ($keyPathMap.ContainsKey($keyPath)) {{
                $name = $keyPathMap[$keyPath]
                try {{
                    Set-ItemProperty -Path $device.PSPath -Name 'FriendlyName' -Value $name -ErrorAction Stop
                    Write-Host "恢复 FriendlyName: $keyPath -> $name"
                }} catch {{ Write-Host "恢复 FriendlyName 失败: $_" }}
                try {{
                    $desc = (Get-ItemProperty -Path $device.PSPath -Name 'DeviceDesc' -ErrorAction SilentlyContinue).DeviceDesc
                    if ($desc) {{
                        $parts = $desc -split ';', 2
                        if ($parts.Count -gt 1) {{
                            Set-ItemProperty -Path $device.PSPath -Name 'DeviceDesc' -Value "$($parts[0]);$name" -ErrorAction Stop
                        }}
                    }}
                }} catch {{ Write-Host "恢复 DeviceDesc 失败: $_" }}
            }}
        }}
    }}
}}

# 2. 通过 MatchingDeviceId 精确恢复 Class 下显卡驱动键
$classPath = 'HKLM:\SYSTEM\CurrentControlSet\Control\Class\{{4d36e968-e325-11ce-bfc1-08002be10318}}'
if (Test-Path $classPath) {{
    foreach ($subkey in (Get-ChildItem $classPath -ErrorAction SilentlyContinue)) {{
        if ($subkey.PSChildName -match '^00\d+') {{
            try {{
                $mid = (Get-ItemProperty -Path $subkey.PSPath -Name 'MatchingDeviceId' -ErrorAction SilentlyContinue).MatchingDeviceId
                if ($mid -and $matchingIdMap.ContainsKey($mid)) {{
                    $origName = $matchingIdMap[$mid]
                    Set-ItemProperty -Path $subkey.PSPath -Name 'DriverDesc' -Value $origName -ErrorAction Stop
                    Write-Host "恢复 Class DriverDesc: $($subkey.PSChildName) -> $origName"
                }}
            }} catch {{}}
        }}
    }}
}}

# 3. 恢复 Control\Video 下显卡键（Video 下无 MatchingDeviceId，使用 fallback 恢复所有 GPU 类子键）
$videoPath = 'HKLM:\SYSTEM\CurrentControlSet\Control\Video'
if (Test-Path $videoPath) {{
    foreach ($videoKey in (Get-ChildItem $videoPath -ErrorAction SilentlyContinue)) {{
        foreach ($subkey in (Get-ChildItem $videoKey.PSPath -ErrorAction SilentlyContinue)) {{
            try {{
                $d1 = (Get-ItemProperty -Path $subkey.PSPath -Name 'DriverDesc' -ErrorAction SilentlyContinue).DriverDesc
                $d2 = (Get-ItemProperty -Path $subkey.PSPath -Name 'DeviceDesc' -ErrorAction SilentlyContinue).DeviceDesc
                $d3 = (Get-ItemProperty -Path $subkey.PSPath -Name 'Description' -ErrorAction SilentlyContinue).Description
                $checkText = "$d1 $d2 $d3"
                if ($checkText -match 'NVIDIA|GeForce|GTX|RTX|AMD|Radeon|Intel|UHD Graphics|Iris|HD Graphics') {{
                    foreach ($n in @('DriverDesc','DeviceDesc','Description','FriendlyName')) {{
                        try {{
                            $cur = (Get-ItemProperty -Path $subkey.PSPath -Name $n -ErrorAction SilentlyContinue).$n
                            if ($cur) {{ Set-ItemProperty -Path $subkey.PSPath -Name $n -Value $fallbackName -ErrorAction SilentlyContinue }}
                        }} catch {{}}
                    }}
                }}
            }} catch {{}}
        }}
    }}
}}

Write-Host 'RESTORE_DONE'
"#,
        fallback = fallback,
        entries_json = entries_json_escaped,
    );

    log::info!("执行PowerShell脚本恢复显卡名称");
    let output = Command::new("powershell.exe")
        .args(&["-ExecutionPolicy", "Bypass", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("无法执行PowerShell: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    log::info!("PowerShell输出: {}", stdout);
    if !stderr.is_empty() {
        log::warn!("PowerShell错误: {}", stderr);
    }

    if output.status.success() && stdout.contains("RESTORE_DONE") {
        Ok(())
    } else {
        Err(format!("恢复失败: {}", if stderr.is_empty() { stdout } else { stderr }))
    }
}

#[cfg(not(target_os = "windows"))]
fn restore_gpu_by_entries(_entries: &[GpuBackupEntry]) -> Result<(), String> {
    Err("此功能仅支持 Windows 系统".to_string())
}

#[tauri::command]
pub async fn restore_gpu_name() -> Result<GpuRenameResult, String> {
    let backup = load_backup()?;

    let result = match backup {
        Some(BackupData::Multi(entries)) => {
            restore_gpu_by_entries(&entries)?;
            GpuRenameResult {
                success: true,
                message: format!("显卡名称已恢复为原始名称（共 {} 张显卡）", entries.len()),
            }
        }
        Some(BackupData::Legacy(original_name)) => {
            // Legacy 备份无 key_path 信息，只能恢复第一张 GPU（优先独显）
            #[cfg(target_os = "windows")]
            {
                let gpu_keys = find_gpu_registry_keys()?;
                let target_key_path = gpu_keys
                    .iter()
                    .find(|(_, _, is_integrated)| !is_integrated)
                    .or_else(|| gpu_keys.first())
                    .map(|(_, key_path, _)| key_path.clone())
                    .ok_or_else(|| "未找到可恢复的显卡".to_string())?;
                rename_gpu(&original_name, &target_key_path)?;
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = original_name;
                return Err("此功能仅支持 Windows 系统".to_string());
            }
            GpuRenameResult {
                success: true,
                message: format!("显卡名称已恢复为: {}", original_name),
            }
        }
        None => {
            return Ok(GpuRenameResult {
                success: false,
                message: "未找到备份文件，无法恢复".to_string(),
            })
        }
    };

    // 删除两处的备份文件
    if let Ok(appdata_path) = get_appdata_backup_path() {
        let _ = fs::remove_file(appdata_path);
    }
    if let Some(install_path) = get_install_dir_backup_path() {
        let _ = fs::remove_file(install_path);
    }

    Ok(result)
}
