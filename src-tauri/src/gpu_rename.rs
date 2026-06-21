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

fn get_backup_path() -> Result<PathBuf, String> {
    let exe_dir = std::env::current_exe()
        .map_err(|e| format!("获取程序路径失败: {}", e))?;
    let parent_dir = exe_dir.parent().ok_or("无法获取父目录")?;
    Ok(parent_dir.join("gpu_rename_backup.json"))
}

fn save_backup(info: &GpuInfo) -> Result<(), String> {
    let path = get_backup_path()?;
    let json = serde_json::to_string_pretty(info)
        .map_err(|e| format!("序列化备份数据失败: {}", e))?;
    fs::write(&path, json)
        .map_err(|e| format!("写入备份文件失败: {}", e))?;
    Ok(())
}

fn load_backup() -> Result<Option<GpuInfo>, String> {
    let path = get_backup_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("读取备份文件失败: {}", e))?;
    let info: GpuInfo = serde_json::from_str(&content)
        .map_err(|e| format!("解析备份数据失败: {}", e))?;
    Ok(Some(info))
}

#[cfg(target_os = "windows")]
fn find_gpu_registry_keys() -> Result<Vec<(RegKey, String)>, String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let enum_key = hklm.open_subkey("SYSTEM\\CurrentControlSet\\Enum\\PCI")
        .map_err(|e| format!("打开注册表键失败: {}", e))?;
    
    let mut gpu_keys = Vec::new();
    
    for vendor_result in enum_key.enum_keys() {
        let vendor_key_name = vendor_result.map_err(|e| format!("枚举厂商键失败: {}", e))?;
        let vendor_key = enum_key.open_subkey(&vendor_key_name)
            .map_err(|e| format!("打开厂商键失败: {}", e))?;
        
        for device_result in vendor_key.enum_keys() {
            let device_key_name = device_result.map_err(|e| format!("枚举设备键失败: {}", e))?;
            let device_key = vendor_key.open_subkey(&device_key_name)
                .map_err(|e| format!("打开设备键失败: {}", e))?;
            let key_path = vendor_key_name.clone() + "\\" + &device_key_name;
            
            // 排除 USB 控制器等其他带 NVIDIA 标识的非显卡设备
            // 路径格式：VEN_10DE&DEV_XXXX...，10DE 是 NVIDIA 的 PCI 厂商 ID
            if !vendor_key_name.to_uppercase().contains("VEN_10DE") {
                continue;
            }
            
            // 排除关键词：USB控制器等非显卡设备即使带 NVIDIA 标识也要跳过
            let exclude_keywords = ["usb", "controller", "控制器", "host", "xhci", "ehci", "uhci"];
            
            let mut is_nvidia_gpu = false;
            let mut is_excluded = false;
            
            if let Ok(device_desc) = device_key.get_value::<String, _>("DeviceDesc") {
                let device_desc_lower = device_desc.to_lowercase();
                // 先检查是否命中排除词
                for &kw in &exclude_keywords {
                    if device_desc_lower.contains(kw) {
                        is_excluded = true;
                        break;
                    }
                }
                if !is_excluded && (device_desc_lower.contains("nvidia") || 
                   device_desc_lower.contains("geforce") || 
                   device_desc_lower.contains("gtx") ||
                   device_desc_lower.contains("rtx")) {
                    is_nvidia_gpu = true;
                }
            }
            
            if !is_nvidia_gpu && !is_excluded {
                if let Ok(friendly_name) = device_key.get_value::<String, _>("FriendlyName") {
                    let friendly_name_lower = friendly_name.to_lowercase();
                    for &kw in &exclude_keywords {
                        if friendly_name_lower.contains(kw) {
                            is_excluded = true;
                            break;
                        }
                    }
                    if !is_excluded && (friendly_name_lower.contains("nvidia") || 
                       friendly_name_lower.contains("geforce") || 
                       friendly_name_lower.contains("gtx") ||
                       friendly_name_lower.contains("rtx")) {
                        is_nvidia_gpu = true;
                    }
                }
            }
            
            if is_excluded {
                continue;
            }
            
            if is_nvidia_gpu {
                gpu_keys.push((device_key, key_path));
            }
        }
    }
    
    Ok(gpu_keys)
}

#[cfg(target_os = "windows")]
fn get_current_gpu_name() -> Result<String, String> {
    let gpu_keys = find_gpu_registry_keys()?;
    if gpu_keys.is_empty() {
        return Err("未找到显卡注册表信息".to_string());
    }
    
    let (key, _) = &gpu_keys[0];
    if let Ok(name) = key.get_value::<String, _>("FriendlyName") {
        return Ok(name);
    }
    if let Ok(name) = key.get_value::<String, _>("DeviceDesc") {
        let parts: Vec<&str> = name.split(';').collect();
        if parts.len() > 1 {
            return Ok(parts[1].to_string());
        }
        return Ok(name);
    }
    
    Err("无法获取显卡名称".to_string())
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
            # 只处理 NVIDIA 设备（VEN_10DE），排除 USB 控制器等其他带 NVIDIA 标识的设备
            if ($vendor.PSChildName -notmatch "VEN_10DE") {{
                continue
            }}
            $devices = Get-ChildItem $vendor.PSPath
            foreach ($device in $devices) {{
                $isNvidia = $false
                $isExcluded = $false
                $keyPath = $device.PSPath
                
                # 排除关键词：USB控制器等非显卡设备
                $excludeKeywords = @("usb", "controller", "host", "xhci", "ehci", "uhci")
                
                try {{
                    $deviceDesc = (Get-ItemProperty -Path $keyPath -Name "DeviceDesc" -ErrorAction SilentlyContinue).DeviceDesc
                    # 先检查排除词
                    if ($deviceDesc) {{
                        foreach ($kw in $excludeKeywords) {{
                            if ($deviceDesc -match [regex]::Escape($kw)) {{
                                $isExcluded = $true
                                break
                            }}
                        }}
                    }}
                    if (-not $isExcluded -and $deviceDesc -and ($deviceDesc -match "NVIDIA|GeForce|GTX|RTX")) {{
                        $isNvidia = $true
                        Write-Host "找到NVIDIA显卡(DeviceDesc): $($device.PSChildName)"
                    }}
                    
                    if (-not $isNvidia -and -not $isExcluded) {{
                        $friendlyName = (Get-ItemProperty -Path $keyPath -Name "FriendlyName" -ErrorAction SilentlyContinue).FriendlyName
                        if ($friendlyName) {{
                            foreach ($kw in $excludeKeywords) {{
                                if ($friendlyName -match [regex]::Escape($kw)) {{
                                    $isExcluded = $true
                                    break
                                }}
                            }}
                        }}
                        if (-not $isExcluded -and $friendlyName -and ($friendlyName -match "NVIDIA|GeForce|GTX|RTX")) {{
                            $isNvidia = $true
                            Write-Host "找到NVIDIA显卡(FriendlyName): $($device.PSChildName)"
                        }}
                    }}
                    
                    if ($isExcluded) {{
                        Write-Host "跳过非显卡设备: $($device.PSChildName)"
                        continue
                    }}
                    
                    if ($isNvidia) {{
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
                    if ($driverDesc -and ($driverDesc -match "NVIDIA|GeForce|GTX|RTX")) {{
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
                    if ($checkText -match "NVIDIA|GeForce|GTX|RTX") {{
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
        // 低端显卡
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
            id: "r7240".to_string(),
            name: "AMD Radeon R7 240".to_string(),
            category: "low-end".to_string(),
        },
        // 高端显卡
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
        GpuOption {
            id: "rx9060xt".to_string(),
            name: "AMD Radeon RX 9060 XT".to_string(),
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
            
            let backup_path = get_backup_path()?;
            let _ = fs::remove_file(backup_path);
            
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
