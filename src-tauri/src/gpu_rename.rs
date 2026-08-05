use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
use winreg::enums::*;
#[cfg(target_os = "windows")]
use winreg::RegKey;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GpuInfo {
    pub original_name: String,
    pub current_name: String,
    pub is_backed_up: bool,
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

fn read_backup_from(path: &std::path::Path) -> Option<GpuInfo> {
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_backup(info: &GpuInfo) -> Result<(), String> {
    let json = serde_json::to_string_pretty(info)
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

fn load_backup() -> Result<Option<GpuInfo>, String> {
    // 优先从 %APPDATA% 读取
    if let Ok(appdata_path) = get_appdata_backup_path() {
        if let Some(info) = read_backup_from(&appdata_path) {
            return Ok(Some(info));
        }
    }

    // 回退到安装目录（兼容旧版本）
    if let Some(install_path) = get_install_dir_backup_path() {
        if let Some(info) = read_backup_from(&install_path) {
            // 自动迁移到 %APPDATA%
            let _ = save_backup(&info);
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
    
    // 支持的显卡厂商 PCI Vendor ID: NVIDIA(10DE)、AMD(1002)
    let supported_vendors = ["VEN_10DE", "VEN_1002"];
    // 排除关键词：USB控制器等非显卡设备
    let exclude_keywords = ["usb", "controller", "控制器", "host", "xhci", "ehci", "uhci", "chipset", "smbus", "audio", "sound"];
    // 显卡名称关键词（NVIDIA / AMD）
    let gpu_keywords = ["nvidia", "geforce", "gtx", "rtx", "amd", "radeon"];
    
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

/// 检查 GPU 是否为核显（集成显卡）
/// 通过读取设备实例子键下的 LocationInformation 注册表值判断
#[cfg(target_os = "windows")]
fn check_is_integrated(device_key: &RegKey, key_path: &str) -> bool {
    // 枚举设备实例子键（如 "4&38d6c93&0&0008"）
    for instance_result in device_key.enum_keys() {
        let instance_name = match instance_result {
            Ok(name) => name,
            Err(_) => continue,
        };
        if let Ok(instance_key) = device_key.open_subkey(&instance_name) {
            // LocationInformation 值包含显卡的物理位置信息
            // 核显通常是 "Internal Graphics" 或 "on board"
            if let Ok(location) = instance_key.get_value::<String, _>("LocationInformation") {
                let lower = location.to_lowercase();
                if lower.contains("internal") || lower.contains("on board") || lower.contains("bus 0") {
                    log::debug!(
                        "检测到核显: {} LocationInformation={}",
                        key_path, location
                    );
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(target_os = "windows")]
fn get_current_gpu_name() -> Result<String, String> {
    let gpu_keys = find_gpu_registry_keys()?;
    if gpu_keys.is_empty() {
        return Err("未找到显卡注册表信息".to_string());
    }
    
    // 从注册表键读取显卡名称（FriendlyName 优先，回退 DeviceDesc）
    let read_gpu_name = |key: &RegKey| -> Option<String> {
        if let Ok(name) = key.get_value::<String, _>("FriendlyName") {
            return Some(name);
        }
        if let Ok(name) = key.get_value::<String, _>("DeviceDesc") {
            let parts: Vec<&str> = name.split(';').collect();
            return Some(if parts.len() > 1 { parts[1].to_string() } else { name });
        }
        None
    };
    
    // 多显卡（如 AMD APU 核显 + 独立显卡）时，优先选择独立显卡：
    // 1. 通过 LocationInformation 注册表值判断核显（最可靠）
    // 2. NVIDIA 设备一定是独显
    // 3. 名称不含 "Graphics" 的 AMD 设备优先
    let mut fallback: Option<String> = None;
    for (key, _, is_integrated) in &gpu_keys {
        let Some(name) = read_gpu_name(key) else {
            continue;
        };
        if fallback.is_none() {
            fallback = Some(name.clone());
        }
        // 跳过 LocationInformation 标记为核显的设备
        if *is_integrated {
            log::info!("跳过核显(注册表标记): {}", name);
            continue;
        }
        // 跳过名称含 "Graphics" 或 "核显" 的 AMD 设备（核显常见后缀）
        let lower = name.to_lowercase();
        if lower.contains("graphics") || lower.contains("核显") {
            log::info!("跳过核显(名称标记): {}", name);
            continue;
        }
        // 到达此处的 GPU 为非核显，直接返回
        return Ok(name);
    }
    
    // 如果所有 GPU 都被标记为核显，回退到第一个 NVIDIA 设备（NVIDIA 不做核显）
    for (key, _, _) in &gpu_keys {
        let Some(name) = read_gpu_name(key) else {
            continue;
        };
        let lower = name.to_lowercase();
        if lower.contains("nvidia") || lower.contains("geforce") || lower.contains("gtx") || lower.contains("rtx") {
            return Ok(name);
        }
    }
    
    // 最后兜底返回第一个
    fallback.ok_or_else(|| "无法获取显卡名称".to_string())
}

#[cfg(not(target_os = "windows"))]
fn get_current_gpu_name() -> Result<String, String> {
    Err("此功能仅支持 Windows 系统".to_string())
}

#[cfg(target_os = "windows")]
fn rename_gpu(new_name: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    
    log::info!("开始尝试修改显卡名称为: {}", new_name);
    
    let escaped_name = new_name.replace('\"', "\"\"");
    
    // PowerShell 脚本，找到并修改所有相关显卡注册表键
    let ps_script = format!(
        r#"
$ErrorActionPreference = 'Stop'
$modified = $false

# 1. 修改 Enum\PCI 下的键
try {{
    $pciPath = "HKLM:\SYSTEM\CurrentControlSet\Enum\PCI"
    if (Test-Path $pciPath) {{
        $vendors = Get-ChildItem $pciPath
        foreach ($vendor in $vendors) {{
            # 只处理 NVIDIA(VEN_10DE) / AMD(VEN_1002) 设备，排除 USB 控制器等其他非显卡设备
            if ($vendor.PSChildName -notmatch "VEN_10DE|VEN_1002") {{
                continue
            }}
            $devices = Get-ChildItem $vendor.PSPath
            foreach ($device in $devices) {{
                $isGpu = $false
                $isExcluded = $false
                $keyPath = $device.PSPath
                
                # 排除关键词：USB控制器等非显卡设备
                $excludeKeywords = @("usb", "controller", "host", "xhci", "ehci", "uhci", "chipset", "smbus", "audio", "sound")
                
                try {{
                    $props = Get-ItemProperty -Path $keyPath -ErrorAction SilentlyContinue
                    $deviceDesc = $props.DeviceDesc
                    $friendlyName = $props.FriendlyName
                    $classGuid = $props.ClassGUID
                    
                    # 先检查排除词（DeviceDesc / FriendlyName）
                    if ($deviceDesc) {{
                        foreach ($kw in $excludeKeywords) {{
                            if ($deviceDesc -match [regex]::Escape($kw)) {{
                                $isExcluded = $true
                                break
                            }}
                        }}
                    }}
                    if (-not $isExcluded -and $friendlyName) {{
                        foreach ($kw in $excludeKeywords) {{
                            if ($friendlyName -match [regex]::Escape($kw)) {{
                                $isExcluded = $true
                                break
                            }}
                        }}
                    }}
                    
                    # 判断是否为显示设备：优先按显卡 ClassGUID（NVIDIA/AMD 通用）
                    $isDisplay = $false
                    if ($classGuid -and $classGuid -match "4d36e968-e325-11ce-bfc1-08002be10318") {{
                        $isDisplay = $true
                    }}
                    
                    if (-not $isExcluded) {{
                        $descText = "$deviceDesc $friendlyName"
                        if ($isDisplay -or $descText -match "NVIDIA|GeForce|GTX|RTX|AMD|Radeon") {{
                            $isGpu = $true
                            Write-Host "找到显卡: $($device.PSChildName)"
                        }}
                    }}
                    
                    if ($isExcluded) {{
                        Write-Host "跳过非显卡设备: $($device.PSChildName)"
                        continue
                    }}
                    
                    if ($isGpu) {{
                        Write-Host "正在修改: $keyPath"
                        
                        # 修改 FriendlyName
                        try {{
                            Set-ItemProperty -Path $keyPath -Name "FriendlyName" -Value "{}"
                            Write-Host "成功修改 FriendlyName"
                            $modified = $true
                        }} catch {{
                            Write-Host "修改 FriendlyName 失败: $_"
                        }}
                        
                        # 修改 DeviceDesc
                        try {{
                            $deviceDesc = (Get-ItemProperty -Path $keyPath -Name "DeviceDesc" -ErrorAction SilentlyContinue).DeviceDesc
                            if ($deviceDesc) {{
                                $parts = $deviceDesc -split ';', 2
                                if ($parts.Count -gt 1) {{
                                    $newDesc = "$($parts[0]);{}"
                                    Set-ItemProperty -Path $keyPath -Name "DeviceDesc" -Value $newDesc
                                    Write-Host "成功修改 DeviceDesc"
                                    $modified = $true
                                }}
                            }}
                        }} catch {{
                            Write-Host "修改 DeviceDesc 失败: $_"
                        }}
                    }}
                }} catch {{
                    Write-Host "处理设备失败: $_"
                }}
            }}
        }}
    }}
}} catch {{
    Write-Host "Enum\PCI 处理失败: $_"
}}

# 2. 修改 Class 下的显卡键
try {{
    $classPath = "HKLM:\SYSTEM\CurrentControlSet\Control\Class\{{4d36e968-e325-11ce-bfc1-08002be10318}}"
    if (Test-Path $classPath) {{
        $subkeys = Get-ChildItem $classPath
        foreach ($subkey in $subkeys) {{
            if ($subkey.PSChildName -match "^00\d+") {{
                $keyPath = $subkey.PSPath
                try {{
                    $driverDesc = (Get-ItemProperty -Path $keyPath -Name "DriverDesc" -ErrorAction SilentlyContinue).DriverDesc
                    if ($driverDesc -and ($driverDesc -match "NVIDIA|GeForce|GTX|RTX|AMD|Radeon")) {{
                        Write-Host "找到显卡Class键: $($subkey.PSChildName) DriverDesc: $driverDesc"
                        Set-ItemProperty -Path $keyPath -Name "DriverDesc" -Value "{}"
                        Write-Host "成功修改 DriverDesc"
                        $modified = $true
                    }}
                }} catch {{
                    Write-Host "处理Class键失败: $_"
                }}
            }}
        }}
    }}
}} catch {{
    Write-Host "Class 处理失败: $_"
}}

# 3. 额外检查其他可能的显卡位置
try {{
    $displayPath = "HKLM:\SYSTEM\CurrentControlSet\Control\Video"
    if (Test-Path $displayPath) {{
        $videoKeys = Get-ChildItem $displayPath
        foreach ($videoKey in $videoKeys) {{
            $subkeys = Get-ChildItem $videoKey.PSPath -ErrorAction SilentlyContinue
            foreach ($subkey in $subkeys) {{
                try {{
                    $keyPath = $subkey.PSPath
                    $driverDesc = (Get-ItemProperty -Path $keyPath -Name "DriverDesc" -ErrorAction SilentlyContinue).DriverDesc
                    $deviceDesc = (Get-ItemProperty -Path $keyPath -Name "DeviceDesc" -ErrorAction SilentlyContinue).DeviceDesc
                    $description = (Get-ItemProperty -Path $keyPath -Name "Description" -ErrorAction SilentlyContinue).Description
                    
                    $checkText = @($driverDesc, $deviceDesc, $description) -join " "
                    if ($checkText -match "NVIDIA|GeForce|GTX|RTX|AMD|Radeon") {{
                        Write-Host "找到Video键: $($videoKey.PSChildName)\$($subkey.PSChildName)"
                        
                        foreach ($name in @("DriverDesc", "DeviceDesc", "Description", "FriendlyName")) {{
                            try {{
                                $current = (Get-ItemProperty -Path $keyPath -Name $name -ErrorAction SilentlyContinue).$name
                                if ($current) {{
                                    Set-ItemProperty -Path $keyPath -Name $name -Value "{}"
                                    Write-Host "成功修改 $name"
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
    Write-Host "FAILED: 未能找到或修改任何显卡注册表键"
    exit 1
}}
"#,
        escaped_name, escaped_name, escaped_name, escaped_name
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
fn rename_gpu(_new_name: &str) -> Result<(), String> {
    Err("此功能仅支持 Windows 系统".to_string())
}

#[tauri::command]
pub async fn get_gpu_info() -> Result<GpuInfo, String> {
    let backup = load_backup()?;
    let current_name = get_current_gpu_name()?;
    
    match backup {
        Some(mut info) => {
            info.current_name = current_name;
            Ok(info)
        }
        None => {
            Ok(GpuInfo {
                original_name: current_name.clone(),
                current_name,
                is_backed_up: false,
            })
        }
    }
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
        GpuOption {
            id: "rx6400".to_string(),
            name: "AMD Radeon RX 6400".to_string(),
            category: "low-end".to_string(),
        },
        GpuOption {
            id: "rx6500xt".to_string(),
            name: "AMD Radeon RX 6500 XT".to_string(),
            category: "low-end".to_string(),
        },
        GpuOption {
            id: "rx6600".to_string(),
            name: "AMD Radeon RX 6600".to_string(),
            category: "low-end".to_string(),
        },
        GpuOption {
            id: "rx6650xt".to_string(),
            name: "AMD Radeon RX 6650 XT".to_string(),
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

#[tauri::command]
pub async fn apply_gpu_rename(new_name: String) -> Result<GpuRenameResult, String> {
    let backup = load_backup()?;
    
    let current_name = get_current_gpu_name()?;
    
    if backup.is_none() {
        let info = GpuInfo {
            original_name: current_name.clone(),
            current_name: new_name.clone(),
            is_backed_up: true,
        };
        save_backup(&info)?;
    }
    
    rename_gpu(&new_name)?;
    
    Ok(GpuRenameResult {
        success: true,
        message: format!("显卡名称已更改为: {}", new_name),
    })
}

#[tauri::command]
pub async fn restore_gpu_name() -> Result<GpuRenameResult, String> {
    let backup = load_backup()?;
    
    match backup {
        Some(info) => {
            rename_gpu(&info.original_name)?;

            // 删除两处的备份文件
            if let Ok(appdata_path) = get_appdata_backup_path() {
                let _ = fs::remove_file(appdata_path);
            }
            if let Some(install_path) = get_install_dir_backup_path() {
                let _ = fs::remove_file(install_path);
            }

            Ok(GpuRenameResult {
                success: true,
                message: format!("显卡名称已恢复为: {}", info.original_name),
            })
        }
        None => {
            Ok(GpuRenameResult {
                success: false,
                message: "未找到备份文件，无法恢复".to_string(),
            })
        }
    }
}
