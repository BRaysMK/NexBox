use crate::nvapi;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::fs;
use std::time::Duration;
use tauri::Manager;

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
    pub quality: String,
    pub texture_quality: String,
    pub antialiasing: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct DLSSPresetStatus {
    pub preset: String,
    pub quality: String,
    pub texture_quality: String,
    pub antialiasing: String,
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
}

static DELTA_PASSWORD_CACHE: std::sync::Mutex<Option<(Vec<DeltaPasswordItem>, std::time::Instant)>> = std::sync::Mutex::new(None);

const DLSS_SR_OVERRIDE_ID: u32 = 0x10E41E01;
const DLSS_SR_PRESET_ID: u32 = 0x10E41DF3;
const DLSS_SR_MODE_ID: u32 = 0x10AFB768;
const TEXTURE_FILTERING_QUALITY_ID: u32 = 0x00CE2691;
const AA_MODE_REPLAY_ID: u32 = 0x10D48A85;
/// "DLSS - Forced Model Preset Profile". NVIDIA's own docs note that on some
/// games the preset letter only takes effect once this is set to Custom.
const DLSS_MODEL_PRESET_PROFILE_ID: u32 = 0x00634291;
const DLSS_MODEL_PRESET_CUSTOM: u32 = 2;

const DLSS_STATUS_SETTING_IDS: [u32; 6] = [
    DLSS_SR_OVERRIDE_ID,
    DLSS_SR_PRESET_ID,
    DLSS_SR_MODE_ID,
    TEXTURE_FILTERING_QUALITY_ID,
    AA_MODE_REPLAY_ID,
    DLSS_MODEL_PRESET_PROFILE_ID,
];

/// A set of DRS edits for the game's driver profile.
///
/// `writes` sets an explicit value, `deletes` removes an existing override so
/// the setting falls back to the driver/game default. "Default" in the UI means
/// *delete*, never *write zero* — writing zero leaves an explicit override that
/// pins the feature off.
#[derive(Debug, Default, PartialEq, Eq)]
struct DlssSettingPlan {
    writes: Vec<(u32, u32)>,
    deletes: Vec<u32>,
}

impl DlssSettingPlan {
    fn set(&mut self, setting_id: u32, value: u32) {
        self.writes.push((setting_id, value));
    }

    /// Explicit value, or remove the override when `value` is `None`.
    fn set_or_clear(&mut self, setting_id: u32, value: Option<u32>) {
        match value {
            Some(v) => self.set(setting_id, v),
            None => self.deletes.push(setting_id),
        }
    }
}

/// Preset letter -> NVDRS value for "DLSS - Forced Preset Letter".
///
/// L and M are only accepted by drivers that ship Transformer Gen 2 presets;
/// callers filter them out via `nvapi::enum_available_setting_values` before
/// offering them, so they are mapped here but never assumed to be writable.
fn dlss_preset_letter_value(preset: &str) -> Option<u32> {
    match preset.to_uppercase().as_str() {
        "A" => Some(1),
        "B" => Some(2),
        "C" => Some(3),
        "D" => Some(4),
        "E" => Some(5),
        "F" => Some(6),
        "G" => Some(7),
        "J" => Some(10),
        "K" => Some(11),
        "L" => Some(12),
        "M" => Some(13),
        _ => None,
    }
}

/// Quality level -> NVDRS value for "DLSS - Forced Quality Level".
fn dlss_quality_value(quality: &str) -> Option<u32> {
    match quality {
        "performance" => Some(0),
        "balanced" => Some(1),
        "quality" => Some(2),
        "dlaa" => Some(4),
        "ultra_performance" => Some(5),
        _ => None,
    }
}

/// Texture quality -> NVDRS value for "Texture filtering - Quality".
fn texture_quality_value(texture_quality: &str) -> Option<u32> {
    match texture_quality {
        "high_quality" => Some(0xFFFF_FFF6),
        "quality" => Some(0x0000_0000),
        "performance" => Some(0x0000_000A),
        "high_performance" => Some(0x0000_0014),
        _ => None,
    }
}

/// Antialiasing mode -> NVDRS value for "Antialiasing - Transparency Supersampling".
fn antialiasing_value(antialiasing: &str) -> Option<u32> {
    match antialiasing {
        "off" => Some(0x0000_0000),
        "2x" => Some(0x0000_0014),
        "4x" => Some(0x0000_0024),
        "8x" => Some(0x0000_0034),
        _ => None,
    }
}

/// Build the DRS edits for the requested combination.
///
/// Deliberately scoped to DLSS (plus the two opt-in quality knobs). VSync,
/// DeepDVC, VRR, DLSS-FG and the DLSSG frame-count settings are *not* touched:
/// they are unrelated to the preset and overwriting them silently destroyed the
/// user's existing game profile.
///
/// "DLSS - Forced Scaling Ratio" is intentionally not written either: NVIDIA's
/// reference notes it only applies when "Forced Quality Level" is `Custom`, so
/// writing it alongside a named quality level is at best a no-op and at worst
/// makes the driver treat the level as custom.
fn build_dlss_plan(
    preset: &str,
    quality: &str,
    texture_quality: &str,
    antialiasing: &str,
) -> DlssSettingPlan {
    let mut plan = DlssSettingPlan::default();

    let preset_letter = dlss_preset_letter_value(preset);

    // The override switch itself is always written: 1 to force the preset,
    // 0 to hand the decision back to the game.
    plan.set(DLSS_SR_OVERRIDE_ID, if preset_letter.is_some() { 1 } else { 0 });
    plan.set_or_clear(DLSS_SR_PRESET_ID, preset_letter);
    plan.set_or_clear(
        DLSS_MODEL_PRESET_PROFILE_ID,
        preset_letter.map(|_| DLSS_MODEL_PRESET_CUSTOM),
    );

    plan.set_or_clear(DLSS_SR_MODE_ID, dlss_quality_value(quality));
    plan.set_or_clear(
        TEXTURE_FILTERING_QUALITY_ID,
        texture_quality_value(texture_quality),
    );
    plan.set_or_clear(AA_MODE_REPLAY_ID, antialiasing_value(antialiasing));

    plan
}

const DELTA_EXECUTABLE_NAMES: [&str; 2] = [
    "deltaforceclient-win64-shipping.exe",
    "DeltaForceClient-Win64-Shipping.exe",
];

const DELTA_PROFILE_NAMES: [&str; 6] = [
    "Delta Force",
    "Delta Force: Hawk Ops",
    "DeltaForce",
    "DeltaForceClient-Win64-Shipping",
    "三角洲行动",
    "三角洲",
];

fn fetch_delta_passwords_from_primary_api() -> Option<Vec<DeltaPasswordItem>> {
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

/// 备用接口：https://api.s0o1.com/API/sjz/mm/
/// 返回纯文本，格式形如：
///   1. 潮汐监狱
///   具体点位：监狱行政区1楼大厅楼梯拐角处
///   每日密码：0557
///   地点图片：https://...
/// 即 "1." 之后是地图名，"每日密码：" 之后是密码
fn parse_backup_password_text(text: &str) -> Vec<DeltaPasswordItem> {
    let map_re = regex::Regex::new(r"^\s*\d+\.\s*(.+?)\s*$").unwrap();
    let pw_re = regex::Regex::new(r"每日密码\s*[:：]\s*(\S+)").unwrap();

    let mut items: Vec<DeltaPasswordItem> = Vec::new();
    let mut current: Option<(String, String)> = None;

    for line in text.lines() {
        if let Some(caps) = map_re.captures(line) {
            // 上一组数据完整则先保存
            if let Some((name, pw)) = current.take() {
                if !pw.is_empty() {
                    items.push(DeltaPasswordItem { name, password: pw });
                }
            }
            current = Some((caps[1].to_string(), String::new()));
            continue;
        }
        if let Some(caps) = pw_re.captures(line) {
            if let Some((_, pw)) = current.as_mut() {
                if pw.is_empty() {
                    *pw = caps[1].to_string();
                }
            }
        }
    }

    if let Some((name, pw)) = current.take() {
        if !pw.is_empty() {
            items.push(DeltaPasswordItem { name, password: pw });
        }
    }

    items
}

fn fetch_delta_passwords_from_backup_api() -> Option<Vec<DeltaPasswordItem>> {
    let url = "https://api.s0o1.com/API/sjz/mm/";

    let text = reqwest::blocking::Client::new()
        .get(url)
        .timeout(Duration::from_secs(10))
        .send()
        .ok()?
        .text()
        .ok()?;

    let items = parse_backup_password_text(&text);
    if items.is_empty() { None } else { Some(items) }
}

/// 主接口优先，失败或数据为空时回退到备用接口
fn fetch_delta_passwords_from_api() -> Option<Vec<DeltaPasswordItem>> {
    if let Some(items) = fetch_delta_passwords_from_primary_api() {
        if !items.is_empty() {
            return Some(items);
        }
    }
    fetch_delta_passwords_from_backup_api()
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

fn get_cached_delta_password_items() -> Option<Vec<DeltaPasswordItem>> {
    {
        let cache = DELTA_PASSWORD_CACHE.lock().unwrap();
        if let Some((data, time)) = cache.as_ref() {
            if time.elapsed().as_secs() < 300 {
                return if data.is_empty() { None } else { Some(data.clone()) };
            }
        }
    }

    match fetch_delta_passwords_from_api() {
        Some(passwords) => {
            if passwords.is_empty() {
                return None;
            }
            let mut cache = DELTA_PASSWORD_CACHE.lock().unwrap();
            *cache = Some((passwords.clone(), std::time::Instant::now()));
            Some(passwords)
        }
        None => None,
    }
}

pub fn get_cached_delta_password_filtered(selected_maps: &[String]) -> Option<String> {
    let items = get_cached_delta_password_items()?;
    let filtered: Vec<_> = if selected_maps.is_empty() {
        items
    } else {
        items.into_iter().filter(|item| selected_maps.contains(&item.name)).collect()
    };
    if filtered.is_empty() {
        return None;
    }
    let joined = filtered
        .iter()
        .map(|item| format!("{}：{}", item.name, item.password))
        .collect::<Vec<_>>()
        .join("  ");
    Some(joined)
}

pub fn get_cached_delta_password() -> Option<String> {
    get_cached_delta_password_filtered(&[])
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
    // Only advertise presets the installed driver actually implements. L and M
    // are Transformer Gen 2 letters that pre-Gen-2 drivers accept into the enum
    // but never apply, so asking the driver is the only honest source.
    let available = nvapi::enum_available_setting_values(DLSS_SR_PRESET_ID).unwrap_or_default();

    let all = [
        ("A", "早期模型", false),
        ("B", "早期模型", false),
        ("C", "早期模型", false),
        ("D", "稳定模型", false),
        ("E", "实验性模型", false),
        ("F", "改进模型", false),
        ("G", "改进模型", false),
        ("J", "较新模型，画质优先", false),
        ("K", "推荐模型，大多数DLSS模式", true),
        ("L", "Transformer Gen 2，画质优先", false),
        ("M", "Transformer Gen 2，性能优先", false),
    ];

    let mut presets = vec![DLSSModelPreset {
        id: "default".to_string(),
        name: "默认（由3D程序设置）".to_string(),
        description: "由3D程序自行决定，不做强制覆盖".to_string(),
        recommended: false,
    }];

    for (id, description, recommended) in all {
        let value = dlss_preset_letter_value(id);
        // An empty enum list means the driver query failed; fall back to the
        // static table rather than hiding everything.
        if !available.is_empty() && !value.is_some_and(|v| available.contains(&v)) {
            log::info!("驱动不支持 DLSS 预设 {id}（枚举值缺失），已从列表隐藏");
            continue;
        }
        presets.push(DLSSModelPreset {
            id: id.to_string(),
            name: format!("Preset {id}"),
            description: description.to_string(),
            recommended,
        });
    }

    Ok(presets)
}

/// Persist the applied combination for diagnostics only.
///
/// This file is never read back as the source of truth — status always comes
/// from the driver. It exists so a bug report can show what was requested.
fn write_applied_status(status: &DLSSPresetStatus) {
    let Ok(exe_path) = std::env::current_exe() else {
        return;
    };
    let Some(parent_dir) = exe_path.parent() else {
        return;
    };
    let status_path = parent_dir.join("delta_force_dlss_status.json");
    if let Ok(json) = serde_json::to_string_pretty(status) {
        let _ = fs::write(status_path, json);
    }
}

#[tauri::command]
pub async fn apply_dlss_model_preset(
    preset: String,
    quality: String,
    texture_quality: String,
    antialiasing: String,
) -> Result<DLSSApplyResult, String> {
    let plan = build_dlss_plan(&preset, &quality, &texture_quality, &antialiasing);

    log::info!(
        "DLSS 应用请求: preset={preset} quality={quality} texture={texture_quality} aa={antialiasing}; \
         writes={:?} deletes={:?}",
        plan.writes
            .iter()
            .map(|(id, v)| format!("0x{id:08X}=0x{v:08X}"))
            .collect::<Vec<_>>(),
        plan.deletes
            .iter()
            .map(|id| format!("0x{id:08X}"))
            .collect::<Vec<_>>()
    );

    // Writes go straight to the driver profile that owns the game executable.
    // No external tool, no profile reset, no unrelated settings touched.
    let stored = nvapi::apply_application_settings(
        &DELTA_EXECUTABLE_NAMES,
        &plan.writes,
        &plan.deletes,
    )
    .map_err(|e| {
        log::error!("DLSS 应用失败: {e}");
        e
    })?;

    log::info!("DLSS 应用已通过驱动回读校验: {stored:?}");

    let status = DLSSPresetStatus {
        preset: preset.clone(),
        quality: quality.clone(),
        texture_quality: texture_quality.clone(),
        antialiasing: antialiasing.clone(),
    };
    write_applied_status(&status);

    let mut parts = vec![format!(
        "DLSS预设: {}",
        if preset.eq_ignore_ascii_case("default") {
            "默认（由3D程序设置）".to_string()
        } else {
            preset.clone()
        }
    )];
    if quality != "default" {
        parts.push(format!("质量: {quality}"));
    }
    if texture_quality != "default" {
        parts.push(format!("纹理: {texture_quality}"));
    }
    if antialiasing != "default" {
        parts.push(format!("抗锯齿: {antialiasing}"));
    }

    Ok(DLSSApplyResult {
        success: true,
        message: format!("{}（已写入驱动并校验）", parts.join(" / ")),
        preset,
        quality,
        texture_quality,
        antialiasing,
    })
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

    let ps_command = format!(
        "Start-Process -FilePath 'powershell' -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','\"{}\"' -Verb RunAs -Wait -WindowStyle Hidden",
        script_path.replace('\'', "''")
    );

    let output = Command::new("powershell")
        .args(["-WindowStyle", "Hidden", "-NoProfile", "-Command", &ps_command])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行失败: {}", e))?;

    let _ = fs::remove_file(&temp_script);

    // 验证注册表值是否已更新（防止Start-Process返回成功但实际写入失败的情况）
    let actual = get_dlss_indicator_registry_value();
    if actual != enable {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if stderr.to_lowercase().contains("cancel") || stderr.to_lowercase().contains("denied") {
            return Err("管理员授权被取消，DLSS指示器设置未生效".to_string());
        }
        return Err("设置DLSS指示器失败: 注册表值未能更新，请确认已授予管理员权限".to_string());
    }

    Ok(enable)
}

fn dlss_preset_name(value: u32) -> String {
    match value {
        1 => "A",
        2 => "B",
        3 => "C",
        4 => "D",
        5 => "E",
        6 => "F",
        7 => "G",
        8 => "H",
        9 => "I",
        10 => "J",
        11 => "K",
        12 => "L",
        13 => "M",
        14 => "N",
        15 => "O",
        _ => "default",
    }
    .to_string()
}

fn dlss_quality_name(value: u32) -> String {
    match value {
        0 => "performance",
        1 => "balanced",
        2 => "quality",
        4 => "dlaa",
        5 => "ultra_performance",
        _ => "default",
    }
    .to_string()
}

fn texture_quality_name(value: u32) -> String {
    match value {
        0xFFFFFFF6 => "high_quality",
        0x00000000 => "quality",
        0x0000000A => "performance",
        0x00000014 => "high_performance",
        _ => "default",
    }
    .to_string()
}

fn antialiasing_name(value: u32) -> String {
    match value & 0x70 {
        0x10 => "2x",
        0x20 => "4x",
        0x30 => "8x",
        _ if value & 0x0F == 0 => "off",
        _ => "default",
    }
    .to_string()
}

fn setting_value(
    values: &[Option<nvapi::NvidiaDrsSettingValue>],
    setting_id: u32,
) -> Option<nvapi::NvidiaDrsSettingValue> {
    let index = DLSS_STATUS_SETTING_IDS
        .iter()
        .position(|id| *id == setting_id)?;
    values.get(index).and_then(|value| *value)
}

fn read_dlss_preset_status_from_nvapi() -> Result<DLSSPresetStatus, String> {
    let values = nvapi::get_nvidia_profile_settings(
        &DELTA_EXECUTABLE_NAMES,
        &DELTA_PROFILE_NAMES,
        &DLSS_STATUS_SETTING_IDS,
    )?;

    let override_value = setting_value(&values, DLSS_SR_OVERRIDE_ID);
    let preset_value = setting_value(&values, DLSS_SR_PRESET_ID);
    let preset = match (override_value, preset_value) {
        (Some(override_value), Some(preset_value))
            if override_value.is_explicit() && override_value.current_value != 0 =>
        {
            dlss_preset_name(preset_value.current_value)
        }
        (_, Some(preset_value)) if preset_value.is_explicit() && preset_value.current_value != 0 => {
            dlss_preset_name(preset_value.current_value)
        }
        _ => "default".to_string(),
    };

    let quality = setting_value(&values, DLSS_SR_MODE_ID)
        .filter(|value| value.is_explicit() && value.current_value != 3)
        .map(|value| dlss_quality_name(value.current_value))
        .unwrap_or_else(|| "default".to_string());

    let texture_quality = setting_value(&values, TEXTURE_FILTERING_QUALITY_ID)
        .filter(|value| value.is_explicit())
        .map(|value| texture_quality_name(value.current_value))
        .unwrap_or_else(|| "default".to_string());

    let antialiasing = setting_value(&values, AA_MODE_REPLAY_ID)
        .filter(|value| value.is_explicit())
        .map(|value| antialiasing_name(value.current_value))
        .unwrap_or_else(|| "default".to_string());

    Ok(DLSSPresetStatus {
        preset,
        quality,
        texture_quality,
        antialiasing,
    })
}

#[tauri::command]
pub async fn get_dlss_settings_status() -> Result<DLSSSettingsStatus, String> {
    Ok(DLSSSettingsStatus {
        dlss_indicator_enabled: get_dlss_indicator_registry_value(),
    })
}

/// Read the DLSS state straight from the driver profile.
///
/// No file fallback: a cached JSON describing what was *requested* is not
/// evidence that the driver accepted it, and surfacing it as current state is
/// exactly how a failed apply used to look successful.
#[tauri::command]
pub async fn get_dlss_preset_status() -> Result<DLSSPresetStatus, String> {
    read_dlss_preset_status_from_nvapi()
}

/// 用系统默认浏览器打开外部链接（不走内嵌窗口）
#[tauri::command]
pub fn open_url_in_system_browser(url: String) -> Result<(), String> {
    tauri_plugin_opener::open_url(&url, None::<&str>).map_err(|e| format!("打开失败: {e}"))
}

/// 在独立 WebView 窗口中打开外部平台链接，iframe 内嵌支持完整跳转
#[tauri::command]
pub async fn open_platform_window(
    app: tauri::AppHandle,
    url: String,
    title: String,
    label: String,
) -> Result<(), String> {
    use tauri::WebviewUrl;
    use tauri::WebviewWindowBuilder;
    use tauri::Manager;

    // 目标 URL 编码后通过 hash fragment 传入 platform-viewer.html
    let encoded = urlencoding::encode(&url);
    let app_path = format!("platform-viewer.html#{}", encoded);

    let app_clone = app.clone();
    let label_clone = label.clone();

    WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(app_path.into()))
        .title(&title)
        // 与其它窗口保持一致的 WebView2 参数（禁用 Chromium 自动媒体会话，避免与 smtc.rs 会话重复）
        .additional_browser_args("--disable-features=MediaSessionService,HardwareMediaKeyHandling,msWebOOUI,msPdfOOUI,msSmartScreenProtection,msEdgeAutofill,msEdgeShopping,msEdgeWallet,CalculateNativeWinOcclusion --autoplay-policy=no-user-gesture-required --disable-background-networking --disable-client-side-phishing-detection --disable-component-update --disable-default-apps --disable-extensions --disable-sync --disable-backgrounding-occluded-windows --disable-renderer-backgrounding --disable-background-timer-throttling")
        .inner_size(1200.0, 800.0)
        .resizable(true)
        .center()
        // 拦截 iframe 内 target="_blank" / window.open() 的跳转
        // 改为在 iframe 内导航，不弹出新窗口
        .on_new_window(move |new_url, _features| {
            if let Some(window) = app_clone.get_webview_window(&label_clone) {
                let safe = new_url.as_str().replace('\\', "\\\\").replace('\'', "\\'");
                let _ = window.eval(&format!(
                    "var f=document.getElementById('viewer');if(f)f.src='{safe}';"
                ));
            }
            tauri::webview::NewWindowResponse::Deny
        })
        .build()
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    Ok(())
}

/// 下载远程图片到应用缓存目录（本地缓存），返回本地文件路径（前端用 convertFileSrc 使用）。
/// 复用 QQ 群图标的缓存范式：按 URL hash 存文件，二次访问走本地缓存。
#[tauri::command]
pub async fn cache_delta_image(app: tauri::AppHandle, url: String) -> Result<String, String> {
    if url.trim().is_empty() {
        return Ok(String::new());
    }

    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("roulette_icons");
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
            return Err(format!("image http {}", resp.status()));
        }
        let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
        std::fs::write(&file, &bytes).map_err(|e| e.to_string())?;
    }

    Ok(file.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(plan: &DlssSettingPlan, setting_id: u32) -> Option<u32> {
        plan.writes
            .iter()
            .find(|(id, _)| *id == setting_id)
            .map(|(_, v)| *v)
    }

    fn is_deleted(plan: &DlssSettingPlan, setting_id: u32) -> bool {
        plan.deletes.contains(&setting_id)
    }

    #[test]
    fn preset_letters_map_to_driver_values() {
        let cases = [
            ("A", 1u32), ("B", 2), ("C", 3), ("D", 4), ("E", 5), ("F", 6), ("G", 7),
            ("J", 10), ("K", 11), ("L", 12), ("M", 13),
        ];
        for (letter, expected) in cases {
            assert_eq!(
                dlss_preset_letter_value(letter),
                Some(expected),
                "preset {letter}"
            );
            // Lower-case input must behave identically (the UI sends upper-case,
            // but a stale config file might not).
            assert_eq!(
                dlss_preset_letter_value(&letter.to_lowercase()),
                Some(expected)
            );
        }
        assert_eq!(dlss_preset_letter_value("default"), None);
        assert_eq!(dlss_preset_letter_value(""), None);
        assert_eq!(dlss_preset_letter_value("Z"), None);
    }

    #[test]
    fn quality_levels_map_to_driver_values() {
        assert_eq!(dlss_quality_value("performance"), Some(0));
        assert_eq!(dlss_quality_value("balanced"), Some(1));
        assert_eq!(dlss_quality_value("quality"), Some(2));
        assert_eq!(dlss_quality_value("dlaa"), Some(4));
        assert_eq!(dlss_quality_value("ultra_performance"), Some(5));
        // 3 is the driver's "N/A" slot and must never be written.
        assert_eq!(dlss_quality_value("default"), None);
    }

    #[test]
    fn plan_never_touches_unrelated_settings() {
        let plan = build_dlss_plan("K", "quality", "high_quality", "4x");

        // VSync / DeepDVC / VRR / DLSS-FG / DLSSG frame counts used to be written
        // unconditionally, which wiped the user's game profile on every apply.
        let forbidden = [
            11041231u32,  // Vertical Sync
            5912412,      // Vertical Sync Tear Control
            9963648,      // Enable DeepDVC Feature
            11250451,     // DeepDVC saturation
            11250466,     // DeepDVC intensity
            278196727,    // VRR requested state
            270198627,    // smooth AFR
            271614616,    // DLSSG mode
            273507943,    // DLSSG multi-frame count
            274083087,    // DLSSG dynamic multi-frame count
            282018085,    // DLSSG target frame rate
            283385329,    // DLSS-FG preset
            283385347,    // Enable DLSS-FG override
            283385333,    // Forced scaling ratio (only valid with quality = Custom)
        ];
        for id in forbidden {
            assert!(
                written(&plan, id).is_none() && !is_deleted(&plan, id),
                "setting 0x{id:08X} must not be part of the DLSS plan"
            );
        }
    }

    #[test]
    fn applying_a_preset_writes_override_preset_and_model_profile() {
        let plan = build_dlss_plan("K", "default", "default", "default");

        assert_eq!(written(&plan, DLSS_SR_OVERRIDE_ID), Some(1));
        assert_eq!(written(&plan, DLSS_SR_PRESET_ID), Some(11));
        assert_eq!(
            written(&plan, DLSS_MODEL_PRESET_PROFILE_ID),
            Some(DLSS_MODEL_PRESET_CUSTOM),
            "preset letter only takes effect on some games when the model \
             preset profile is Custom"
        );

        // Untouched dimensions must be cleared, not pinned to zero.
        assert!(is_deleted(&plan, DLSS_SR_MODE_ID));
        assert!(is_deleted(&plan, TEXTURE_FILTERING_QUALITY_ID));
        assert!(is_deleted(&plan, AA_MODE_REPLAY_ID));
    }

    #[test]
    fn default_preset_clears_overrides_instead_of_writing_zero() {
        let plan = build_dlss_plan("default", "default", "default", "default");

        // The switch itself is explicitly off so the game decides.
        assert_eq!(written(&plan, DLSS_SR_OVERRIDE_ID), Some(0));
        assert!(
            is_deleted(&plan, DLSS_SR_PRESET_ID),
            "preset 0 means 'N/A' in the driver; leaving it set would pin the \
             game to a non-preset instead of restoring the default"
        );
        assert!(
            is_deleted(&plan, DLSS_MODEL_PRESET_PROFILE_ID),
            "a leftover Custom model profile keeps forcing custom presets"
        );
    }

    #[test]
    fn quality_texture_and_antialiasing_are_independent() {
        let plan = build_dlss_plan("default", "quality", "high_performance", "8x");

        assert_eq!(written(&plan, DLSS_SR_MODE_ID), Some(2));
        assert_eq!(written(&plan, TEXTURE_FILTERING_QUALITY_ID), Some(0x14));
        assert_eq!(written(&plan, AA_MODE_REPLAY_ID), Some(0x34));
        // No preset selected -> override off, preset cleared.
        assert_eq!(written(&plan, DLSS_SR_OVERRIDE_ID), Some(0));
        assert!(is_deleted(&plan, DLSS_SR_PRESET_ID));
    }

    #[test]
    fn texture_quality_maps_known_values_only() {
        assert_eq!(texture_quality_value("high_quality"), Some(0xFFFF_FFF6));
        assert_eq!(texture_quality_value("quality"), Some(0));
        assert_eq!(texture_quality_value("performance"), Some(0x0A));
        assert_eq!(texture_quality_value("high_performance"), Some(0x14));
        assert_eq!(texture_quality_value("default"), None);
        assert_eq!(texture_quality_value("bogus"), None);
    }

    #[test]
    fn antialiasing_maps_known_values_only() {
        assert_eq!(antialiasing_value("off"), Some(0));
        assert_eq!(antialiasing_value("2x"), Some(0x14));
        assert_eq!(antialiasing_value("4x"), Some(0x24));
        assert_eq!(antialiasing_value("8x"), Some(0x34));
        assert_eq!(antialiasing_value("default"), None);
    }

    #[test]
    fn plan_has_no_duplicate_setting_ids() {
        let plan = build_dlss_plan("M", "dlaa", "quality", "2x");
        let mut ids: Vec<u32> = plan.writes.iter().map(|(id, _)| *id).collect();
        ids.extend(plan.deletes.iter().copied());
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(ids.len(), sorted.len(), "duplicate setting id in plan: {ids:?}");

        let mut writes: Vec<u32> = plan.writes.iter().map(|(id, _)| *id).collect();
        for id in &plan.deletes {
            assert!(
                !writes.contains(id),
                "0x{id:08X} is both written and deleted"
            );
        }
        writes.clear();
    }

    #[test]
    fn status_round_trips_preset_names() {
        // The reader maps driver values back to the same ids the UI sends, so a
        // round-trip through dlss_preset_name must be stable for every letter.
        for letter in ["A", "B", "C", "D", "E", "F", "G", "J", "K", "L", "M"] {
            let value = dlss_preset_letter_value(letter).unwrap();
            assert_eq!(dlss_preset_name(value), letter);
        }
        assert_eq!(dlss_preset_name(0), "default");
    }
}
