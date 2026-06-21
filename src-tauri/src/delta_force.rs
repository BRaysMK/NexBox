use std::os::windows::process::CommandExt;
use std::process::Command;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct DeltaPasswordItem {
    pub name: String,
    pub password: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct DeltaPasswordResponse {
    pub status: String,
    pub data: Vec<DeltaPasswordItem>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct WeaponCode {
    pub id: String,
    pub name: String,
    pub code: String,
    pub category: String,
    pub description: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct DLSSModelPreset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub recommended: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct DLSSApplyResult {
    pub success: bool,
    pub message: String,
    pub preset: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct DLSSPresetStatus {
    pub preset: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct MapInfo {
    pub id: String,
    pub name: String,
    pub url: String,
    pub description: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct DLSSSettingsStatus {
    pub dlss_indicator_enabled: bool,
    pub dlss_lock_enabled: bool,
}

static DELTA_PASSWORD_CACHE: std::sync::Mutex<Option<(Vec<DeltaPasswordItem>, std::time::Instant)>> = std::sync::Mutex::new(None);

fn fetch_delta_passwords_from_api() -> Option<Vec<DeltaPasswordItem>> {
    let url = "https://i.elaina.vin/api/%E4%B8%89%E8%A7%92%E6%B4%B2/%E5%AF%86%E7%A0%81/";
    
    let response = reqwest::blocking::Client::new()
        .get(url)
        .timeout(Duration::from_secs(10))
        .send()
        .ok()?
        .json::<DeltaPasswordResponse>()
        .ok()?;
    
    if response.status == "success" {
        Some(response.data)
    } else {
        None
    }
}

#[tauri::command]
pub async fn get_delta_passwords() -> Result<Vec<DeltaPasswordItem>, String> {
    {
        let cache = DELTA_PASSWORD_CACHE.lock().unwrap();
        if let Some((cached_data, cached_time)) = cache.as_ref() {
            if cached_time.elapsed().as_secs() < 60 {
                return Ok(cached_data.clone());
            }
        }
    }
    
    let passwords = tokio::task::spawn_blocking(|| {
        fetch_delta_passwords_from_api()
    })
    .await
    .map_err(|e| format!("获取密码失败: {}", e))?
    .ok_or_else(|| "无法获取三角洲密码数据".to_string())?;
    
    {
        let mut cache = DELTA_PASSWORD_CACHE.lock().unwrap();
        *cache = Some((passwords.clone(), std::time::Instant::now()));
    }
    
    Ok(passwords)
}

pub fn get_cached_delta_password() -> Option<String> {
    {
        let cache = DELTA_PASSWORD_CACHE.lock().unwrap();
        if let Some((data, time)) = cache.as_ref() {
            if time.elapsed().as_secs() < 300 {
        if data.is_empty() {
          return None;
        }
        // 返回所有地图-密码对，按顺序拼接，例如 "零号大坝：3333 航天基地：2222"
        let joined = data
          .iter()
          .map(|item| format!("{}：{}", item.name, item.password))
          .collect::<Vec<_>>()
          .join("  ");
        return Some(joined);
            }
        }
    }

    match fetch_delta_passwords_from_api() {
        Some(passwords) => {
            if !passwords.is_empty() {
        let joined = passwords
          .iter()
          .map(|item| format!("{}：{}", item.name, item.password))
          .collect::<Vec<_>>()
          .join("  ");
        let mut cache = DELTA_PASSWORD_CACHE.lock().unwrap();
        *cache = Some((passwords, std::time::Instant::now()));
        Some(joined)
            } else {
                None
            }
        }
        None => None,
    }
}

#[tauri::command]
pub async fn get_weapon_codes() -> Result<Vec<WeaponCode>, String> {
    Ok(vec![
        WeaponCode {
            id: "1".to_string(),
            name: "M4A1 竞技配置".to_string(),
            code: "DELTA-M4A1-001".to_string(),
            category: "突击步枪".to_string(),
            description: "高稳定性竞技配置".to_string(),
        },
        WeaponCode {
            id: "2".to_string(),
            name: "AK47 压枪配置".to_string(),
            code: "DELTA-AK47-001".to_string(),
            category: "突击步枪".to_string(),
            description: "低后座力压枪配置".to_string(),
        },
        WeaponCode {
            id: "3".to_string(),
            name: "AWM 狙击配置".to_string(),
            code: "DELTA-AWM-001".to_string(),
            category: "狙击枪".to_string(),
            description: "精准狙击配置".to_string(),
        },
        WeaponCode {
            id: "4".to_string(),
            name: "MP5 冲锋配置".to_string(),
            code: "DELTA-MP5-001".to_string(),
            category: "冲锋枪".to_string(),
            description: "近距离快速射击".to_string(),
        },
    ])
}

#[tauri::command]
pub async fn get_dlss_model_presets() -> Result<Vec<DLSSModelPreset>, String> {
    Ok(vec![
        DLSSModelPreset { id: "A".to_string(), name: "Preset A".to_string(), description: "早期模型".to_string(), recommended: false },
        DLSSModelPreset { id: "B".to_string(), name: "Preset B".to_string(), description: "早期模型".to_string(), recommended: false },
        DLSSModelPreset { id: "C".to_string(), name: "Preset C".to_string(), description: "早期模型".to_string(), recommended: false },
        DLSSModelPreset { id: "D".to_string(), name: "Preset D".to_string(), description: "稳定模型".to_string(), recommended: false },
        DLSSModelPreset { id: "E".to_string(), name: "Preset E".to_string(), description: "实验性模型".to_string(), recommended: false },
        DLSSModelPreset { id: "F".to_string(), name: "Preset F".to_string(), description: "改进模型".to_string(), recommended: false },
        DLSSModelPreset { id: "G".to_string(), name: "Preset G".to_string(), description: "改进模型".to_string(), recommended: false },
        DLSSModelPreset { id: "J".to_string(), name: "Preset J".to_string(), description: "较新模型，画质优先".to_string(), recommended: false },
        DLSSModelPreset { id: "K".to_string(), name: "Preset K".to_string(), description: "推荐模型，大多数DLSS模式".to_string(), recommended: true },
        DLSSModelPreset { id: "L".to_string(), name: "Preset L".to_string(), description: "优化Ultra Performance模式".to_string(), recommended: true },
        DLSSModelPreset { id: "M".to_string(), name: "Preset M".to_string(), description: "优化Performance模式".to_string(), recommended: true },
    ])
}

fn get_npi_path() -> Result<PathBuf, String> {
    let exe_dir = std::env::current_exe()
        .map_err(|e| format!("获取程序路径失败: {}", e))?;
    let parent_dir = exe_dir.parent().ok_or("无法获取父目录")?;
    
    let candidates = [
        parent_dir.join("nvidiaProfileInspector.exe"),
        parent_dir.join("_up_").join("nvidiaProfileInspector.exe"),
        parent_dir.join("resources").join("nvidiaProfileInspector.exe"),
    ];
    
    for path in &candidates {
        if path.exists() {
            return Ok(path.clone());
        }
    }
    
    Err("未找到NVIDIA Profile Inspector，请确保已安装或将其放在程序目录下".to_string())
}

fn generate_nip_config(preset: &str) -> Vec<u8> {
    let preset_value = match preset.to_uppercase().as_str() {
        "A" => "1",
        "B" => "2",
        "C" => "3",
        "D" => "4",
        "E" => "5",
        "F" => "6",
        "G" => "7",
        "J" => "10",
        "K" => "11",
        "L" => "12",
        "M" => "13",
        _ => "11",
    };

    let xml_content = format!(
        r#"<?xml version="1.0" encoding="utf-16"?>
<ArrayOfProfile>
  <Profile>
    <ProfileName>Delta Force</ProfileName>
    <Executeables>
      <string>deltaforceclient-win64-shipping.exe</string>
    </Executeables>
    <Settings>
      <ProfileSetting>
        <SettingNameInfo>Vertical Sync Tear Control</SettingNameInfo>
        <SettingID>5912412</SettingID>
        <SettingValue>2525368439</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>DLSS Model Preset Profile</SettingNameInfo>
        <SettingID>6505105</SettingID>
        <SettingValue>2</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Enable DeepDVC Feature</SettingNameInfo>
        <SettingID>9963648</SettingID>
        <SettingValue>0</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Vertical Sync</SettingNameInfo>
        <SettingID>11041231</SettingID>
        <SettingValue>1620202130</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Saturation value for DeepDVC</SettingNameInfo>
        <SettingID>11250451</SettingID>
        <SettingValue>50</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Intensity value for DeepDVC</SettingNameInfo>
        <SettingID>11250466</SettingID>
        <SettingValue>50</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Flag to control smooth AFR behavior</SettingNameInfo>
        <SettingID>270198627</SettingID>
        <SettingValue>0</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Override DLSSG mode</SettingNameInfo>
        <SettingID>271614616</SettingID>
        <SettingValue>1</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Override DLSSG multi-frame count</SettingNameInfo>
        <SettingID>273507943</SettingID>
        <SettingValue>0</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Override maximum DLSSG dynamic multi frame count</SettingNameInfo>
        <SettingID>274083087</SettingID>
        <SettingValue>0</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>VRR requested state</SettingNameInfo>
        <SettingID>278196727</SettingID>
        <SettingValue>0</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Override DLSSG Target Frame Rate</SettingNameInfo>
        <SettingID>282018085</SettingID>
        <SettingValue>0</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Override DLSS-FG preset</SettingNameInfo>
        <SettingID>283385329</SettingID>
        <SettingValue>0</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Override DLSS-SR presets</SettingNameInfo>
        <SettingID>283385331</SettingID>
        <SettingValue>{}</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Enable DLSS-SR override</SettingNameInfo>
        <SettingID>283385345</SettingID>
        <SettingValue>1</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Enable DLSS-FG override</SettingNameInfo>
        <SettingID>283385347</SettingID>
        <SettingValue>0</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
    </Settings>
  </Profile>
</ArrayOfProfile>"#,
        preset_value
    );

    let mut bytes: Vec<u8> = vec![0xFF, 0xFE];
    bytes.extend(
        xml_content.encode_utf16().collect::<Vec<u16>>()
            .iter()
            .flat_map(|&c| c.to_le_bytes())
    );
    bytes
}

#[tauri::command]
pub async fn apply_dlss_model_preset(preset: String) -> Result<DLSSApplyResult, String> {
    let npi_path = get_npi_path()?;

    let config_content = generate_nip_config(&preset);

    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join("delta_force_dlss.nip");
    fs::write(&temp_path, &config_content)
        .map_err(|e| format!("写入配置文件失败: {}", e))?;
    
    let npi_str = npi_path.to_str().ok_or("路径编码错误")?;
    let temp_str = temp_path.to_str().ok_or("临时路径编码错误")?;
    
    let ps_command = format!(
        "Start-Process -FilePath '{}' -ArgumentList '-silentImport','\"{}\"' -Verb RunAs -Wait",
        npi_str.replace('\'', "''"),
        temp_str.replace('\'', "''")
    );
    
    let output = Command::new("powershell")
        .args(["-WindowStyle", "Hidden", "-NoProfile", "-Command", &ps_command])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行失败: {}", e))?;
    
    let _ = fs::remove_file(&temp_path);
    
    if output.status.success() {
        // 保存当前应用的预设状态，供前端查询
        let status = DLSSPresetStatus {
            preset: preset.clone(),
        };
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(parent_dir) = exe_path.parent() {
                let status_path = parent_dir.join("delta_force_dlss_status.json");
                let _ = fs::write(&status_path, serde_json::to_string(&status).unwrap_or_default());
            }
        }

        Ok(DLSSApplyResult {
            success: true,
            message: format!("DLSS预设: {}", preset),
            preset,
        })
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(format!("应用失败: {}", stderr))
    }
}

#[tauri::command]
pub async fn get_delta_maps() -> Result<Vec<MapInfo>, String> {
    Ok(vec![
        MapInfo {
            id: "1".to_string(),
            name: "长弓溪谷".to_string(),
            url: "https://df.qq.com/cp/a20241029map/index.html".to_string(),
            description: "大型开放地图".to_string(),
        },
        MapInfo {
            id: "2".to_string(),
            name: "零号大坝".to_string(),
            url: "https://df.qq.com/cp/a20241029map/index.html".to_string(),
            description: "中型战术地图".to_string(),
        },
        MapInfo {
            id: "3".to_string(),
            name: "巴克什".to_string(),
            url: "https://df.qq.com/cp/a20241029map/index.html".to_string(),
            description: "城市战斗地图".to_string(),
        },
    ])
}

fn get_dlss_indicator_registry_value() -> bool {
    let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
    let path = r"SOFTWARE\NVIDIA Corporation\Global\NGXCore";
    if let Ok(key) = hklm.open_subkey(path) {
        if let Ok(value) = key.get_value::<u32, _>("ShowDlssIndicator") {
            return value != 0;
        }
    }
    false
}

#[tauri::command]
pub async fn toggle_dlss_indicator(enable: bool) -> Result<bool, String> {
    let value = if enable { 1024 } else { 0 };

    let script_content = format!(
        "$p='HKLM:\\SOFTWARE\\NVIDIA Corporation\\Global\\NGXCore'; \
         if(-not(Test-Path $p)){{New-Item -Path $p -Force|Out-Null}}; \
         Set-ItemProperty -Path $p -Name 'ShowDlssIndicator' -Value {} -Type DWord",
        value
    );

    let temp_dir = std::env::temp_dir();
    let temp_script = temp_dir.join("dlss_indicator.ps1");
    fs::write(&temp_script, &script_content)
        .map_err(|e| format!("写入临时脚本失败: {}", e))?;

    let script_path = temp_script.to_str().ok_or("路径编码错误")?;

    let vbs_content = format!(
        "Set objShell = CreateObject(\"Shell.Application\")\r\n\
         objShell.ShellExecute \"powershell.exe\", \"-NoProfile -ExecutionPolicy Bypass -File \"\"{}\"\"\", \"\", \"runas\", 0",
        script_path.replace('"', "\"\"")
    );

    let temp_vbs = temp_dir.join("dlss_indicator.vbs");
    fs::write(&temp_vbs, &vbs_content)
        .map_err(|e| format!("写入VBScript失败: {}", e))?;

    let vbs_path = temp_vbs.to_str().ok_or("路径编码错误")?;

    let output = Command::new("wscript.exe")
        .arg(vbs_path)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行失败: {}", e))?;

    let _ = fs::remove_file(&temp_script);
    let _ = fs::remove_file(&temp_vbs);

    if output.status.success() {
        Ok(enable)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(format!("设置DLSS指示器失败(需要管理员权限): {}", stderr))
    }
}

#[tauri::command]
pub async fn toggle_dlss_lock(enable: bool) -> Result<bool, String> {
    let npi_path = get_npi_path()?;

    let lock_config = generate_dlss_lock_config(enable);

    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join("delta_force_dlss_lock.nip");
    fs::write(&temp_path, &lock_config)
        .map_err(|e| format!("写入锁定配置文件失败: {}", e))?;

    let npi_str = npi_path.to_str().ok_or("路径编码错误")?;
    let temp_str = temp_path.to_str().ok_or("临时路径编码错误")?;

    let ps_command = format!(
        "Start-Process -FilePath '{}' -ArgumentList '-silentImport','\"{}\"' -Verb RunAs -Wait",
        npi_str.replace('\'', "''"),
        temp_str.replace('\'', "''")
    );

    let output = Command::new("powershell")
        .args(["-WindowStyle", "Hidden", "-NoProfile", "-Command", &ps_command])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行失败: {}", e))?;

    let _ = fs::remove_file(&temp_path);

    if output.status.success() {
        Ok(enable)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(format!("DLSS锁定操作失败: {}", stderr))
    }
}

fn generate_dlss_lock_config(lock_enabled: bool) -> Vec<u8> {
    let lock_value = if lock_enabled { "1" } else { "0" };

    let xml_content = format!(
        r#"<?xml version="1.0" encoding="utf-16"?>
<ArrayOfProfile>
  <Profile>
    <ProfileName>Delta Force DLSS Lock</ProfileName>
    <Executeables>
      <string>deltaforceclient-win64-shipping.exe</string>
    </Executeables>
    <Settings>
      <ProfileSetting>
        <SettingNameInfo />
        <SettingID>275602687</SettingID>
        <SettingValue>{}</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Override DLSS-SR presets</SettingNameInfo>
        <SettingID>283385331</SettingID>
        <SettingValue>11</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
      <ProfileSetting>
        <SettingNameInfo>Enable DLSS-SR override</SettingNameInfo>
        <SettingID>283385345</SettingID>
        <SettingValue>1</SettingValue>
        <ValueType>Dword</ValueType>
      </ProfileSetting>
    </Settings>
  </Profile>
</ArrayOfProfile>"#,
        lock_value
    );

    let mut bytes: Vec<u8> = vec![0xFF, 0xFE];
    bytes.extend(
        xml_content.encode_utf16().collect::<Vec<u16>>()
            .iter()
            .flat_map(|&c| c.to_le_bytes())
    );
    bytes
}

#[tauri::command]
pub async fn get_dlss_settings_status() -> Result<DLSSSettingsStatus, String> {
    let dlss_indicator_enabled = get_dlss_indicator_registry_value();

    let dlss_lock_enabled = false;

    Ok(DLSSSettingsStatus {
        dlss_indicator_enabled,
        dlss_lock_enabled,
    })
}

#[tauri::command]
pub async fn get_dlss_preset_status() -> Result<DLSSPresetStatus, String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("获取程序路径失败: {}", e))?;
    let parent_dir = exe_path.parent().ok_or("无法获取父目录")?;
    let status_path = parent_dir.join("delta_force_dlss_status.json");

    if status_path.exists() {
        let content = fs::read_to_string(&status_path)
            .map_err(|e| format!("读取状态文件失败: {}", e))?;
        if let Ok(status) = serde_json::from_str::<DLSSPresetStatus>(&content) {
            return Ok(status);
        }
    }

    Ok(DLSSPresetStatus { preset: "K".to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_utf16_le(bytes: &[u8]) -> String {
        if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
            // skip BOM
            let rest = &bytes[2..];
            let mut u16s = Vec::with_capacity(rest.len() / 2);
            for chunk in rest.chunks(2) {
                if chunk.len() == 2 {
                    let lo = chunk[0] as u16;
                    let hi = chunk[1] as u16;
                    u16s.push(lo | (hi << 8));
                }
            }
            String::from_utf16(&u16s).unwrap_or_default()
        } else {
            String::from_utf8(bytes.to_vec()).unwrap_or_default()
        }
    }


}
