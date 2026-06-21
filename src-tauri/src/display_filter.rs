use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::path::PathBuf;
use std::fs;
use std::io::Read;
use tauri::Emitter;

// ─── Display enumeration ───

#[derive(serde::Serialize, Clone)]
pub struct DisplayInfo {
    pub index: usize,
    pub name: String,
    pub device_name: String,
    pub is_primary: bool,
    pub width: i32,
    pub height: i32,
}

static DISPLAY_DEVICES: Mutex<Option<Vec<String>>> = Mutex::new(None);

/// Sync internal: enumerate all displays and populate DISPLAY_DEVICES cache.
/// Returns the list of DisplayInfo. Returns empty vec on non-Windows.
#[cfg(target_os = "windows")]
fn enumerate_displays_inner() -> Vec<DisplayInfo> {
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW,
        HDC, HMONITOR, MONITORINFOEXW,
    };

    struct MonitorData {
        displays: Vec<DisplayInfo>,
        device_names: Vec<String>,
    }

    unsafe extern "system" fn monitor_enum_proc(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut windows_sys::Win32::Foundation::RECT,
        lparam: isize,
    ) -> i32 {
        let data = &mut *(lparam as *mut MonitorData);
        let mut info: MONITORINFOEXW = std::mem::zeroed();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

        if GetMonitorInfoW(hmonitor, &mut info as *mut _ as *mut _) != 0 {
            let device_name = String::from_utf16_lossy(
                &info.szDevice[..info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len())],
            );
            let is_primary = (info.monitorInfo.dwFlags & 1) != 0;
            let width = info.monitorInfo.rcMonitor.right - info.monitorInfo.rcMonitor.left;
            let height = info.monitorInfo.rcMonitor.bottom - info.monitorInfo.rcMonitor.top;
            let index = data.displays.len();

            let monitor_model = get_monitor_model_name(&device_name);

            let name = if !monitor_model.is_empty() {
                format!("{} ({}x{})", monitor_model, width, height)
            } else {
                format!("{} ({}x{})", device_name, width, height)
            };

            data.displays.push(DisplayInfo {
                index,
                name,
                device_name: device_name.clone(),
                is_primary,
                width,
                height,
            });
            data.device_names.push(device_name);
        }
        1
    }

    let mut data = MonitorData {
        displays: Vec::new(),
        device_names: Vec::new(),
    };

    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(monitor_enum_proc),
            &mut data as *mut _ as isize,
        );
    }

    // Cache device names
    if let Ok(mut lock) = DISPLAY_DEVICES.lock() {
        *lock = Some(data.device_names);
    }

    if data.displays.is_empty() {
        data.displays.push(DisplayInfo {
            index: 0,
            name: "DISPLAY1 (Primary)".to_string(),
            device_name: "DISPLAY1".to_string(),
            is_primary: true,
            width: 0,
            height: 0,
        });
        if let Ok(mut lock) = DISPLAY_DEVICES.lock() {
            *lock = Some(vec!["DISPLAY1".to_string()]);
        }
    }

    data.displays
}

#[tauri::command]
pub async fn get_displays() -> Result<Vec<DisplayInfo>, String> {
    #[cfg(target_os = "windows")]
    {
        log::info!("get_displays: 枚举所有显示器…");
        Ok(enumerate_displays_inner())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows 系统".to_string())
    }
}

/// 在应用启动时预填显示器信息缓存，确保热键路径也能正确获取设备名。
#[cfg(target_os = "windows")]
pub fn init() {
    log::info!("display_filter::init — 预填显示器信息…");
    enumerate_displays_inner();
}

#[cfg(not(target_os = "windows"))]
pub fn init() {
    // no-op on non-Windows
}

#[cfg(target_os = "windows")]
fn get_monitor_model_name(device_name: &str) -> String {
    use windows_sys::Win32::Graphics::Gdi::{EnumDisplayDevicesW, DISPLAY_DEVICEW};
    use std::mem;

    unsafe {
        let device_name_wide: Vec<u16> = device_name.encode_utf16().chain(std::iter::once(0)).collect();
        
        let mut disp_device: DISPLAY_DEVICEW = mem::zeroed();
        disp_device.cb = mem::size_of::<DISPLAY_DEVICEW>() as u32;
        
        if EnumDisplayDevicesW(device_name_wide.as_ptr(), 0, &mut disp_device, 0) != 0 {
            let len = disp_device.DeviceString.iter().position(|&c| c == 0).unwrap_or(disp_device.DeviceString.len());
            if len > 0 {
                let model = String::from_utf16_lossy(&disp_device.DeviceString[..len]);
                let trimmed = model.trim();
                if !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("Generic PnP Monitor") {
                    return trimmed.to_string();
                }
                return model.trim().to_string();
            }
        }
    }
    
    String::new()
}

// ─── Per-display state ───

struct DisplayState {
    original_gamma: Option<[[u16; 256]; 3]>,
    temperature: i32,
    brightness: i32,
    contrast: i32,
    saturation: i32,
    mode: i32,
    icc_ramp: Option<[[u16; 256]; 3]>,
    icc_active: bool,
    filter_active: bool,
}

impl Default for DisplayState {
    fn default() -> Self {
        Self {
            original_gamma: None,
            temperature: 6500,
            brightness: 100,
            contrast: 100,
            saturation: 100,
            mode: 0,
            icc_ramp: None,
            icc_active: false,
            filter_active: false,
        }
    }
}

static DISPLAY_STATES: Mutex<Option<Vec<Mutex<DisplayState>>>> = Mutex::new(None);
static ACTIVE_DISPLAY_INDEX: AtomicUsize = AtomicUsize::new(0);

// Filter monitor thread flags
static FILTER_THREAD_RUNNING: AtomicBool = AtomicBool::new(false);
static GAMMA_RAMP_MUTEX: Mutex<()> = Mutex::new(());

fn ensure_display_states() {
    let mut lock = DISPLAY_STATES.lock().unwrap();
    if lock.is_none() {
        let count = if let Ok(dev_lock) = DISPLAY_DEVICES.lock() {
            dev_lock.as_ref().map(|d| d.len()).unwrap_or(1)
        } else {
            1
        };
        let states: Vec<Mutex<DisplayState>> = (0..count)
            .map(|_| Mutex::new(DisplayState::default()))
            .collect();
        *lock = Some(states);
    }
}

fn with_display_state<F, R>(idx: usize, f: F) -> R
where
    F: FnOnce(&mut DisplayState) -> R,
{
    ensure_display_states();
    let lock = DISPLAY_STATES.lock().unwrap();
    let states = lock.as_ref().unwrap();
    let idx = idx.min(states.len() - 1);
    let mut state = states[idx].lock().unwrap();
    f(&mut *state)
}

fn get_active_index() -> usize {
    let idx = ACTIVE_DISPLAY_INDEX.load(Ordering::SeqCst);
    ensure_display_states();
    let lock = DISPLAY_STATES.lock().unwrap();
    let states = lock.as_ref().unwrap();
    idx.min(states.len() - 1)
}

fn resolve_display_index(display_index: Option<usize>) -> usize {
    display_index.unwrap_or_else(|| get_active_index())
}

#[tauri::command]
pub async fn set_active_display(display_index: usize) -> Result<(), String> {
    ensure_display_states();
    ACTIVE_DISPLAY_INDEX.store(display_index, Ordering::SeqCst);
    Ok(())
}

// ─── Filter mode and setting types ───

#[derive(serde::Serialize, Clone, Copy, PartialEq)]
pub enum FilterMode {
    Normal = 0,
    Vivid = 1,
    Movie = 2,
    Highlight = 3,
    Soft = 4,
    Gaming = 5,
    Reading = 6,
    DeExposure = 7,
    ShadowBoost = 8,
}

impl FilterMode {
    pub fn from_i32(value: i32) -> Self {
        match value {
            1 => FilterMode::Vivid,
            2 => FilterMode::Movie,
            3 => FilterMode::Highlight,
            4 => FilterMode::Soft,
            5 => FilterMode::Gaming,
            6 => FilterMode::Reading,
            7 => FilterMode::DeExposure,
            8 => FilterMode::ShadowBoost,
            _ => FilterMode::Normal,
        }
    }
}

#[derive(serde::Serialize, Clone)]
pub struct FilterSettings {
    pub temperature: i32,
    pub brightness: i32,
    pub contrast: i32,
    pub saturation: i32,
    pub mode: i32,
    pub is_active: bool,
}

#[derive(serde::Serialize)]
pub struct FilterResult {
    pub success: bool,
    pub message: String,
    pub settings: Option<FilterSettings>,
}

#[derive(serde::Serialize)]
pub struct FilterPreset {
    pub id: String,
    pub name: String,
    pub mode: i32,
    pub temperature: i32,
    pub brightness: i32,
    pub contrast: i32,
    pub saturation: i32,
    pub description: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CustomFilterSettings {
    pub temperature: i32,
    pub brightness: i32,
    pub contrast: i32,
    pub saturation: i32,
}

impl Default for CustomFilterSettings {
    fn default() -> Self {
        Self {
            temperature: 6500,
            brightness: 100,
            contrast: 100,
            saturation: 100,
        }
    }
}

static CUSTOM_SETTINGS: Mutex<Option<HashMap<usize, CustomFilterSettings>>> = Mutex::new(None);

fn get_settings_file_path() -> PathBuf {
    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    config_dir.join("NexBox").join("settings.json")
}

fn load_custom_settings_from_file() -> HashMap<usize, CustomFilterSettings> {
    let path = get_settings_file_path();
    if path.exists() {
        match fs::read_to_string(&path) {
            Ok(content) => {
                match serde_json::from_str::<serde_json::Value>(&content) {
                    Ok(json) => {
                        if let Some(settings_value) = json.get("custom-filter-settings") {
                            // Try to deserialize as a map first (new format: {"0": {...}, "1": {...}})
                            if let Ok(map) = serde_json::from_value::<HashMap<String, CustomFilterSettings>>(
                                settings_value.clone(),
                            ) {
                                let result: HashMap<usize, CustomFilterSettings> = map
                                    .into_iter()
                                    .filter_map(|(k, v)| k.parse::<usize>().ok().map(|idx| (idx, v)))
                                    .collect();
                                if !result.is_empty() {
                                    return result;
                                }
                            }
                            // Fallback: old format (single CustomFilterSettings object)
                            match serde_json::from_value::<CustomFilterSettings>(settings_value.clone()) {
                                Ok(settings) => {
                                    let mut map = HashMap::new();
                                    map.insert(0, settings);
                                    return map;
                                }
                                Err(e) => {
                                    log::error!("解析自定义滤镜设置失败: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("解析设置文件JSON失败: {}", e);
                    }
                }
            }
            Err(e) => {
                log::error!("读取设置文件失败: {}", e);
            }
        }
    }
    HashMap::new()
}

fn save_custom_settings_to_file(settings: &HashMap<usize, CustomFilterSettings>) -> Result<(), String> {
    let path = get_settings_file_path();

    if let Some(parent) = path.parent() {
        if !parent.exists() {
            if let Err(e) = fs::create_dir_all(parent) {
                log::error!("创建目录失败: {}", e);
                return Err(format!("无法创建目录: {}", e));
            }
        }
    }

    let mut existing_settings: serde_json::Value = if path.exists() {
        match fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(json) => json,
                Err(_) => serde_json::json!({}),
            },
            Err(_) => serde_json::json!({}),
        }
    } else {
        serde_json::json!({})
    };

    // Serialize as string-keyed map for JSON compatibility
    let string_map: HashMap<String, &CustomFilterSettings> = settings
        .iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    existing_settings["custom-filter-settings"] =
        serde_json::to_value(&string_map).unwrap();

    match serde_json::to_string_pretty(&existing_settings) {
        Ok(json_str) => {
            match fs::write(&path, json_str) {
                Ok(_) => Ok(()),
                Err(e) => {
                    log::error!("写入设置文件失败: {}", e);
                    Err(format!("无法保存设置: {}", e))
                }
            }
        }
        Err(e) => {
            log::error!("序列化设置失败: {}", e);
            Err(format!("无法序列化设置: {}", e))
        }
    }
}

fn get_or_load_custom_settings() -> HashMap<usize, CustomFilterSettings> {
    let mut settings_lock = CUSTOM_SETTINGS.lock().unwrap();
    if settings_lock.is_none() {
        let settings = load_custom_settings_from_file();
        *settings_lock = Some(settings.clone());
        settings
    } else {
        settings_lock.as_ref().unwrap().clone()
    }
}

// ─── Gamma calculation ───

fn kelvin_to_rgb_multipliers(temperature: i32) -> (f64, f64, f64) {
    let temp = temperature as f64 / 100.0;

    let red = if temp <= 66.0 {
        1.0
    } else {
        let r = temp - 60.0;
        let val = 329.698727446 * r.powf(-0.1332047592);
        (val / 255.0).clamp(0.0, 1.0)
    };

    let green = if temp <= 66.0 {
        let val = 99.4708025861 * temp.ln() - 161.1195681661;
        (val / 255.0).clamp(0.0, 1.0)
    } else {
        let g = temp - 60.0;
        let val = 288.1221695283 * g.powf(-0.0755148492);
        (val / 255.0).clamp(0.0, 1.0)
    };

    let blue = if temp >= 66.0 {
        1.0
    } else if temp <= 19.0 {
        0.0
    } else {
        let b = temp - 10.0;
        let val = 138.5177312231 * b.ln() - 305.0447927307;
        (val / 255.0).clamp(0.0, 1.0)
    };

    (red, green, blue)
}

fn apply_gamma_curve(input: f64, gamma: f64) -> f64 {
    input.powf(1.0 / gamma)
}

fn apply_s_curve(input: f64, strength: f64) -> f64 {
    let strength = strength.clamp(-0.5, 0.5);
    let x = input - 0.5;
    let result = 0.5 + x * (1.0 + strength * (1.0 - 4.0 * x * x));
    result.clamp(0.0, 1.0)
}

fn build_gamma_ramp(
    temperature: i32,
    brightness: i32,
    contrast: i32,
    saturation: i32,
    mode: FilterMode,
) -> [[u16; 256]; 3] {
    let (r_temp_mult, g_temp_mult, b_temp_mult) = kelvin_to_rgb_multipliers(temperature);
    let brightness_factor = brightness as f64 / 100.0;
    let contrast_factor = contrast as f64 / 100.0;
    let sat_factor = saturation as f64 / 100.0;

    let (gamma, s_curve_strength, r_boost, g_boost, b_boost): (f64, f64, f64, f64, f64) = match mode {
        FilterMode::Normal => (1.0, 0.0, 1.0, 1.0, 1.0),
        FilterMode::Vivid => {
            (0.95, 0.08, 1.02, 1.0, 1.03)
        }
        FilterMode::Movie => {
            (1.05, -0.05, 1.0, 0.98, 0.96)
        }
        FilterMode::Highlight => {
            (0.92, 0.05, 1.0, 1.0, 1.0)
        }
        FilterMode::Soft => {
            (1.08, -0.08, 0.98, 1.0, 1.02)
        }
        FilterMode::Gaming => {
            (0.96, 0.1, 1.0, 1.0, 1.02)
        }
        FilterMode::Reading => {
            (1.0, 0.0, 1.0, 0.99, 0.97)
        }
        FilterMode::DeExposure => {
            // 去曝光：gamma<1 整体压暗、负 S 曲线压缩高光，恢复高光细节
            (0.96, -0.05, 1.0, 1.0, 1.0)
        }
        FilterMode::ShadowBoost => {
            // 暗部增强：gamma>1 提亮暗部、小幅正 S 曲线保留对比，让暗处显现
            (1.12, 0.03, 1.0, 1.0, 1.0)
        }
    };

    let mut ramp = [[0u16; 256]; 3];

    for i in 0..256 {
        let input = i as f64 / 255.0;

        let mut adjusted = apply_gamma_curve(input, gamma);

        adjusted = apply_s_curve(adjusted, s_curve_strength);

        adjusted = ((adjusted - 0.5) * contrast_factor + 0.5) * brightness_factor;
        adjusted = adjusted.clamp(0.0, 1.0);

        let base_output = adjusted * 65535.0;

        let r_final = (base_output * r_temp_mult * r_boost).min(65535.0);
        let g_final = (base_output * g_temp_mult * g_boost).min(65535.0);
        let b_final = (base_output * b_temp_mult * b_boost).min(65535.0);

        let r_luma = 0.299 * r_final;
        let g_luma = 0.587 * g_final;
        let b_luma = 0.114 * b_final;
        let luma = r_luma + g_luma + b_luma;

        let r_out = if (sat_factor - 1.0).abs() > 0.001 {
            luma + (r_final - luma) * sat_factor
        } else {
            r_final
        };
        let g_out = if (sat_factor - 1.0).abs() > 0.001 {
            luma + (g_final - luma) * sat_factor
        } else {
            g_final
        };
        let b_out = if (sat_factor - 1.0).abs() > 0.001 {
            luma + (b_final - luma) * sat_factor
        } else {
            b_final
        };

        ramp[0][i] = r_out.clamp(0.0, 65535.0) as u16;
        ramp[1][i] = g_out.clamp(0.0, 65535.0) as u16;
        ramp[2][i] = b_out.clamp(0.0, 65535.0) as u16;
    }

    for channel in 0..3 {
        for i in 1..256 {
            if ramp[channel][i] < ramp[channel][i - 1] {
                ramp[channel][i] = ramp[channel][i - 1];
            }
        }
    }

    ramp[0][0] = 0;
    ramp[1][0] = 0;
    ramp[2][0] = 0;
    ramp[0][255] = 65535;
    ramp[1][255] = 65535;
    ramp[2][255] = 65535;

    ramp
}

// ─── Per-display DC helpers ───

#[cfg(target_os = "windows")]
fn get_display_dc(
    display_index: usize,
) -> Result<(windows_sys::Win32::Graphics::Gdi::HDC, bool), String> {
    use windows_sys::Win32::Graphics::Gdi::{CreateDCW, GetDC};

    // 1. Try to get the device name from cache
    let device_names: Vec<String> = {
        let lock = DISPLAY_DEVICES
            .lock()
            .map_err(|_| "无法获取显示器列表锁".to_string())?;
        if let Some(ref names) = *lock {
            names.clone()
        } else {
            // 缓存为空 → 立即枚举一次（处理热键路径先于页面加载的情况）
            drop(lock);
            log::info!("get_display_dc[{}]: DISPLAY_DEVICES 缓存为空，即时枚举显示器", display_index);
            enumerate_displays_inner();
            // Re-lock and try again
            let lock2 = DISPLAY_DEVICES
                .lock()
                .map_err(|_| "无法获取显示器列表锁".to_string())?;
            lock2.as_ref().map(|n| n.clone()).unwrap_or_default()
        }
    };

    // Try multiple device name formats for robustness (handles Optimus/dGPU scenarios)
    let name: Option<String> = device_names.get(display_index).cloned();
    let name_formats: Vec<String> = if let Some(ref name) = name {
        let mut formats: Vec<String> = vec![name.to_string()];
        // Also try without \\.\ prefix (some systems need this)
        let stripped = name.trim_start_matches("\\\\.\\");
        if stripped != name.as_str() {
            formats.push(stripped.to_string());
        }
        formats
    } else {
        vec![]
    };

    // Attempt per-display CreateDCW with each name format
    for fmt in &name_formats {
        let device_name_wide: Vec<u16> = fmt.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let hdc = CreateDCW(
                device_name_wide.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
            );
            if !hdc.is_null() {
                log::info!("get_display_dc[{}]: CreateDCW 成功 (name={})", display_index, fmt);
                return Ok((hdc, false));
            }
        }
    }

    // ── Fallback 1: Try "DISPLAY1", "DISPLAY2" as plain driver names ──
    // On some Optimus laptops, the device name from EnumDisplayMonitors may not
    // work directly with CreateDCW, but the plain "DISPLAY1" form does.
    if name_formats.is_empty() || name_formats.iter().all(|f| f.starts_with("\\\\.\\")) {
        let alt_name = format!("DISPLAY{}", display_index + 1);
        let alt_name_wide: Vec<u16> = alt_name.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let hdc = CreateDCW(
                alt_name_wide.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
            );
            if !hdc.is_null() {
                log::info!("get_display_dc[{}]: CreateDCW 成功 (alt name={})", display_index, alt_name);
                return Ok((hdc, false));
            }
        }
    }

    // ── Fallback 2: Try plain "DISPLAY" (system-wide desktop DC via CreateDCW) ──
    // This may behave differently than GetDC(null) on certain GPU configurations.
    let display_wide: Vec<u16> = "DISPLAY\0".encode_utf16().collect();
    unsafe {
        let hdc = CreateDCW(
            display_wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
        );
        if !hdc.is_null() {
            log::warn!(
                "get_display_dc[{}]: 逐显示器 CreateDCW 全部失败，使用 CreateDCW(DISPLAY) 回退",
                display_index
            );
            return Ok((hdc, false)); // false → use DeleteDC
        }
    }

    // ── Fallback 3: Desktop DC via GetDC(null) ──
    // Works on virtually all systems including Optimus laptops.
    log::warn!(
        "get_display_dc[{}]: 所有 CreateDCW 尝试失败，使用 GetDC(null) 桌面 DC 回退",
        display_index
    );
    unsafe {
        let hdc = GetDC(std::ptr::null_mut());
        if hdc.is_null() {
            return Err("无法获取设备上下文".to_string());
        }
        Ok((hdc, true))
    }
}

// ─── Gamma ramp functions ───

#[cfg(target_os = "windows")]
fn set_gamma_ramp_for_display(display_index: usize, ramp: &[[u16; 256]; 3]) -> Result<(), String> {
    use windows_sys::Win32::Graphics::Gdi::{DeleteDC, ReleaseDC};
    use windows_sys::Win32::UI::ColorSystem::SetDeviceGammaRamp;

    let _guard = GAMMA_RAMP_MUTEX
        .lock()
        .map_err(|_| "无法获取 Gamma Ramp 锁".to_string())?;

    // Strategy 1: Try per-display DC
    let (hdc, use_release) = get_display_dc(display_index)?;

    unsafe {
        let result = SetDeviceGammaRamp(hdc, ramp.as_ptr() as *const _);
        let dc_is_per_display = !use_release;

        if use_release {
            ReleaseDC(std::ptr::null_mut(), hdc);
        } else {
            DeleteDC(hdc);
        }

        if result != 0 {
            log::info!(
                "set_gamma_ramp_for_display[{}]: Strategy1 逐显示器 SetDeviceGammaRamp 成功 (dc_is_per_display={})",
                display_index, dc_is_per_display
            );
            return Ok(());
        }

        // If per-display DC failed but we used desktop DC, no more retries
        if !dc_is_per_display {
            log::error!(
                "set_gamma_ramp_for_display[{}]: SetDeviceGammaRamp 失败(桌面DC回退，跳过Strategy2)! display_index={}",
                "可能是显卡驱动不支持",
                display_index
            );
            return Err("设置 Gamma Ramp 失败，可能是显卡驱动不支持".to_string());
        }

        log::warn!(
            "set_gamma_ramp_for_display[{}]: 逐显示器 DC 创建成功但 SetDeviceGammaRamp 失败，进入 Strategy2 桌面 DC 回退",
            display_index
        );
    }

    // Strategy 2: Per-display SetDeviceGammaRamp failed → retry with desktop-wide DC
    // This is critical for laptops with NVIDIA Optimus / AMD switchable graphics
    // where per-display SetDeviceGammaRamp may fail but desktop-wide works
    unsafe {
        let hdc = windows_sys::Win32::Graphics::Gdi::GetDC(std::ptr::null_mut());
        if hdc.is_null() {
            return Err("无法获取桌面设备上下文".to_string());
        }

        let result = SetDeviceGammaRamp(hdc, ramp.as_ptr() as *const _);
        windows_sys::Win32::Graphics::Gdi::ReleaseDC(std::ptr::null_mut(), hdc);

        if result == 0 {
            log::error!(
                "set_gamma_ramp_for_display[{}]: Strategy2 桌面 DC 回退也失败！显卡/驱动可能不支持 Gamma Ramp",
                display_index
            );
            return Err("设置 Gamma Ramp 失败，可能是显卡驱动不支持".to_string());
        }

        log::info!(
            "set_gamma_ramp_for_display[{}]: Strategy2 桌面 DC 回退成功 (适用于 Optimus/混合模式)",
            display_index
        );
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn get_current_gamma_ramp_for_display(
    display_index: usize,
) -> Result<[[u16; 256]; 3], String> {
    use windows_sys::Win32::Graphics::Gdi::{DeleteDC, ReleaseDC};
    use windows_sys::Win32::UI::ColorSystem::GetDeviceGammaRamp;

    let mut ramp = [[0u16; 256]; 3];

    // Strategy 1: Try per-display DC
    let (hdc, use_release) = get_display_dc(display_index)?;

    unsafe {
        let result = GetDeviceGammaRamp(hdc, ramp.as_mut_ptr() as *mut _);

        if use_release {
            ReleaseDC(std::ptr::null_mut(), hdc);
        } else {
            DeleteDC(hdc);
        }

        if result != 0 {
            return Ok(ramp);
        }
    }

    // Strategy 2: Per-display failed → retry with desktop-wide DC
    log::warn!(
        "get_current_gamma_ramp_for_display[{}]: 逐显示器 GetDeviceGammaRamp 失败，尝试桌面 DC 回退",
        display_index
    );

    unsafe {
        let hdc = windows_sys::Win32::Graphics::Gdi::GetDC(std::ptr::null_mut());
        if hdc.is_null() {
            return Err("无法获取桌面设备上下文".to_string());
        }

        let result = GetDeviceGammaRamp(hdc, ramp.as_mut_ptr() as *mut _);
        windows_sys::Win32::Graphics::Gdi::ReleaseDC(std::ptr::null_mut(), hdc);

        if result == 0 {
            log::error!(
                "get_current_gamma_ramp_for_display[{}]: 桌面 DC 回退也失败",
                display_index
            );
            return Err("读取 Gamma Ramp 失败".to_string());
        }

        log::info!(
            "get_current_gamma_ramp_for_display[{}]: 桌面 DC 回退成功",
            display_index
        );
        Ok(ramp)
    }
}

#[cfg(not(target_os = "windows"))]
fn set_gamma_ramp_for_display(_display_index: usize, _ramp: &[[u16; 256]; 3]) -> Result<(), String> {
    Err("此功能仅支持 Windows 系统".to_string())
}

#[cfg(not(target_os = "windows"))]
fn get_current_gamma_ramp_for_display(_display_index: usize) -> Result<[[u16; 256]; 3], String> {
    Err("此功能仅支持 Windows 系统".to_string())
}

// ─── Filter application ───

fn apply_filter_internal_for_display(display_index: usize) -> Result<(), String> {
    let (icc_active, temperature, brightness, contrast, saturation, mode, icc_ramp_opt) =
        with_display_state(display_index, |state| {
            (
                state.icc_active,
                state.temperature,
                state.brightness,
                state.contrast,
                state.saturation,
                state.mode,
                state.icc_ramp,
            )
        });

    if icc_active {
        if let Some(ref ramp) = icc_ramp_opt {
            log::info!("Monitor[{}]: applying ICC ramp", display_index);
            return set_gamma_ramp_for_display(display_index, ramp);
        }
    }

    let mode_enum = FilterMode::from_i32(mode);
    let ramp = build_gamma_ramp(temperature, brightness, contrast, saturation, mode_enum);
    log::info!("Monitor[{}]: applying regular filter ramp", display_index);
    set_gamma_ramp_for_display(display_index, &ramp)
}

fn restore_original_gamma_for_display(display_index: usize) -> Result<(), String> {
    let original =
        with_display_state(display_index, |state| state.original_gamma);

    if let Some(ref ramp) = original {
        set_gamma_ramp_for_display(display_index, ramp)?;
    } else {
        let identity_ramp = build_gamma_ramp(6500, 100, 100, 100, FilterMode::Normal);
        set_gamma_ramp_for_display(display_index, &identity_ramp)?;
    }

    Ok(())
}

fn start_filter_monitor() {
    if FILTER_THREAD_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    thread::spawn(|| {
        loop {
            // Collect active display indices (release all locks before applying)
            let active_indices: Vec<usize> = {
                ensure_display_states();
                let lock = DISPLAY_STATES.lock().unwrap();
                let states = lock.as_ref().unwrap();
                states
                    .iter()
                    .enumerate()
                    .filter_map(|(i, state_mutex)| {
                        let state = state_mutex.lock().unwrap();
                        if state.filter_active {
                            Some(i)
                        } else {
                            None
                        }
                    })
                    .collect()
            };

            if active_indices.is_empty() {
                break;
            }

            for i in &active_indices {
                if let Err(e) = apply_filter_internal_for_display(*i) {
                    log::error!("应用滤镜到显示器 {} 失败: {}", i, e);
                }
            }

            thread::sleep(Duration::from_millis(1000));
        }

        FILTER_THREAD_RUNNING.store(false, Ordering::SeqCst);
    });
}

// ─── Tauri commands ───

#[tauri::command]
pub async fn get_filter_settings(display_index: Option<usize>) -> Result<FilterSettings, String> {
    let idx = resolve_display_index(display_index);
    Ok(with_display_state(idx, |state| FilterSettings {
        temperature: state.temperature,
        brightness: state.brightness,
        contrast: state.contrast,
        saturation: state.saturation,
        mode: state.mode,
        is_active: state.filter_active,
    }))
}

#[tauri::command]
pub async fn set_filter_settings(
    display_index: Option<usize>,
    temperature: i32,
    brightness: i32,
    contrast: i32,
    saturation: i32,
    mode: i32,
) -> Result<FilterResult, String> {
    #[cfg(target_os = "windows")]
    {
        let idx = resolve_display_index(display_index);
        let temperature = temperature.clamp(1000, 10000);
        let brightness = brightness.clamp(50, 150);
        let contrast = contrast.clamp(50, 150);
        let saturation = saturation.clamp(50, 150);
        let mode = mode.clamp(0, 8);

        with_display_state(idx, |state| {
            state.temperature = temperature;
            state.brightness = brightness;
            state.contrast = contrast;
            state.saturation = saturation;
            state.mode = mode;
            state.icc_active = false;

            if !state.filter_active {
                if state.original_gamma.is_none() {
                    if let Ok(ramp) = get_current_gamma_ramp_for_display(idx) {
                        state.original_gamma = Some(ramp);
                    }
                }
                state.filter_active = true;
            }
        });

        apply_filter_internal_for_display(idx)?;
        start_filter_monitor();

        Ok(FilterResult {
            success: true,
            message: "滤镜设置已更新".to_string(),
            settings: Some(FilterSettings {
                temperature,
                brightness,
                contrast,
                saturation,
                mode,
                is_active: true,
            }),
        })
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows 系统".to_string())
    }
}

#[tauri::command]
pub async fn enable_filter(display_index: Option<usize>) -> Result<FilterResult, String> {
    #[cfg(target_os = "windows")]
    {
        let idx = resolve_display_index(display_index);

        let already_active = with_display_state(idx, |state| {
            if state.filter_active {
                true
            } else {
                if state.original_gamma.is_none() {
                    if let Ok(ramp) = get_current_gamma_ramp_for_display(idx) {
                        state.original_gamma = Some(ramp);
                    }
                }
                state.filter_active = true;
                false
            }
        });

        if already_active {
            return Ok(with_display_state(idx, |state| FilterResult {
                success: true,
                message: "滤镜已处于启用状态".to_string(),
                settings: Some(FilterSettings {
                    temperature: state.temperature,
                    brightness: state.brightness,
                    contrast: state.contrast,
                    saturation: state.saturation,
                    mode: state.mode,
                    is_active: true,
                }),
            }));
        }

        apply_filter_internal_for_display(idx)?;
        start_filter_monitor();

        Ok(with_display_state(idx, |state| FilterResult {
            success: true,
            message: "滤镜已启用".to_string(),
            settings: Some(FilterSettings {
                temperature: state.temperature,
                brightness: state.brightness,
                contrast: state.contrast,
                saturation: state.saturation,
                mode: state.mode,
                is_active: true,
            }),
        }))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows 系统".to_string())
    }
}

#[tauri::command]
pub async fn disable_filter(display_index: Option<usize>) -> Result<FilterResult, String> {
    #[cfg(target_os = "windows")]
    {
        let idx = resolve_display_index(display_index);

        let was_active = with_display_state(idx, |state| {
            if !state.filter_active {
                false
            } else {
                state.filter_active = false;
                state.icc_active = false;
                true
            }
        });

        if !was_active {
            return Ok(with_display_state(idx, |state| FilterResult {
                success: true,
                message: "滤镜已处于禁用状态".to_string(),
                settings: Some(FilterSettings {
                    temperature: state.temperature,
                    brightness: state.brightness,
                    contrast: state.contrast,
                    saturation: state.saturation,
                    mode: state.mode,
                    is_active: false,
                }),
            }));
        }

        if let Err(e) = restore_original_gamma_for_display(idx) {
            log::error!("恢复原始 Gamma 失败: {}", e);
        }

        with_display_state(idx, |state| {
            state.original_gamma = None;
        });

        Ok(with_display_state(idx, |state| FilterResult {
            success: true,
            message: "滤镜已禁用".to_string(),
            settings: Some(FilterSettings {
                temperature: state.temperature,
                brightness: state.brightness,
                contrast: state.contrast,
                saturation: state.saturation,
                mode: state.mode,
                is_active: false,
            }),
        }))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows 系统".to_string())
    }
}

#[tauri::command]
pub async fn toggle_filter(display_index: Option<usize>) -> Result<FilterResult, String> {
    let idx = resolve_display_index(display_index);
    let is_active = with_display_state(idx, |state| state.filter_active);
    if is_active {
        disable_filter(display_index).await
    } else {
        enable_filter(display_index).await
    }
}

/// Toggle filter on/off. Used by global hotkey.
pub fn toggle_filter_sync(app_handle: &tauri::AppHandle) -> Result<FilterResult, String> {
    #[cfg(target_os = "windows")]
    {
        let idx = get_active_index();
        let is_active = with_display_state(idx, |state| state.filter_active);
        let result = if is_active {
            disable_filter_sync()
        } else {
            enable_filter_sync()
        };

        if result.is_ok() {
            let _ = app_handle.emit("filter-status-changed", ());
        }

        result
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows 系统".to_string())
    }
}

#[cfg(target_os = "windows")]
fn enable_filter_sync() -> Result<FilterResult, String> {
    let idx = get_active_index();

    let already_active = with_display_state(idx, |state| {
        if state.filter_active {
            true
        } else {
            if state.original_gamma.is_none() {
                if let Ok(ramp) = get_current_gamma_ramp_for_display(idx) {
                    state.original_gamma = Some(ramp);
                }
            }
            state.filter_active = true;
            false
        }
    });

    if already_active {
        return Ok(with_display_state(idx, |state| FilterResult {
            success: true,
            message: "滤镜已处于启用状态".to_string(),
            settings: Some(FilterSettings {
                temperature: state.temperature,
                brightness: state.brightness,
                contrast: state.contrast,
                saturation: state.saturation,
                mode: state.mode,
                is_active: true,
            }),
        }));
    }

    apply_filter_internal_for_display(idx)?;
    start_filter_monitor();

    Ok(with_display_state(idx, |state| FilterResult {
        success: true,
        message: "滤镜已启用".to_string(),
        settings: Some(FilterSettings {
            temperature: state.temperature,
            brightness: state.brightness,
            contrast: state.contrast,
            saturation: state.saturation,
            mode: state.mode,
            is_active: true,
        }),
    }))
}

#[cfg(target_os = "windows")]
fn disable_filter_sync() -> Result<FilterResult, String> {
    let idx = get_active_index();

    let was_active = with_display_state(idx, |state| {
        if !state.filter_active {
            false
        } else {
            state.filter_active = false;
            state.icc_active = false;
            true
        }
    });

    if !was_active {
        return Ok(with_display_state(idx, |state| FilterResult {
            success: true,
            message: "滤镜已处于禁用状态".to_string(),
            settings: Some(FilterSettings {
                temperature: state.temperature,
                brightness: state.brightness,
                contrast: state.contrast,
                saturation: state.saturation,
                mode: state.mode,
                is_active: false,
            }),
        }));
    }

    if let Err(e) = restore_original_gamma_for_display(idx) {
        log::error!("恢复原始 Gamma 失败: {}", e);
    }

    Ok(with_display_state(idx, |state| FilterResult {
        success: true,
        message: "滤镜已禁用".to_string(),
        settings: Some(FilterSettings {
            temperature: state.temperature,
            brightness: state.brightness,
            contrast: state.contrast,
            saturation: state.saturation,
            mode: state.mode,
            is_active: false,
        }),
    }))
}

#[tauri::command]
pub async fn get_filter_presets() -> Result<Vec<FilterPreset>, String> {
    Ok(vec![
        FilterPreset {
            id: "normal".to_string(),
            name: "标准".to_string(),
            mode: 0,
            temperature: 6500,
            brightness: 100,
            contrast: 100,
            saturation: 100,
            description: "默认显示效果".to_string(),
        },
        FilterPreset {
            id: "vivid".to_string(),
            name: "鲜艳".to_string(),
            mode: 1,
            temperature: 6800,
            brightness: 102,
            contrast: 105,
            saturation: 115,
            description: "增强色彩饱和度，画面更鲜艳".to_string(),
        },
        FilterPreset {
            id: "movie".to_string(),
            name: "电影".to_string(),
            mode: 2,
            temperature: 5800,
            brightness: 98,
            contrast: 95,
            saturation: 95,
            description: "电影质感，柔和色调".to_string(),
        },
        FilterPreset {
            id: "highlight".to_string(),
            name: "高亮".to_string(),
            mode: 3,
            temperature: 7200,
            brightness: 110,
            contrast: 102,
            saturation: 100,
            description: "提高亮度，适合暗光环境".to_string(),
        },
        FilterPreset {
            id: "soft".to_string(),
            name: "柔和".to_string(),
            mode: 4,
            temperature: 5200,
            brightness: 98,
            contrast: 92,
            saturation: 95,
            description: "柔和画面，减少眼睛疲劳".to_string(),
        },
        FilterPreset {
            id: "gaming".to_string(),
            name: "游戏".to_string(),
            mode: 5,
            temperature: 6800,
            brightness: 103,
            contrast: 108,
            saturation: 110,
            description: "增强对比度和色彩，适合游戏".to_string(),
        },
        FilterPreset {
            id: "reading".to_string(),
            name: "阅读".to_string(),
            mode: 6,
            temperature: 4800,
            brightness: 95,
            contrast: 100,
            saturation: 92,
            description: "暖色调，保护眼睛".to_string(),
        },
        FilterPreset {
            id: "de-exposure".to_string(),
            name: "去曝光".to_string(),
            mode: 7,
            temperature: 6500,
            brightness: 92,
            contrast: 103,
            saturation: 98,
            description: "压暗高光，降低过度曝光，恢复高光细节".to_string(),
        },
        FilterPreset {
            id: "shadow-boost".to_string(),
            name: "暗部增强".to_string(),
            mode: 8,
            temperature: 6500,
            brightness: 106,
            contrast: 94,
            saturation: 104,
            description: "提亮暗部阴影，让黑暗角落的敌人无处遁形".to_string(),
        },
    ])
}

#[tauri::command]
pub async fn apply_preset(
    display_index: Option<usize>,
    preset_id: String,
) -> Result<FilterResult, String> {
    let presets = get_filter_presets().await?;

    if let Some(preset) = presets.iter().find(|p| p.id == preset_id) {
        set_filter_settings(
            display_index,
            preset.temperature,
            preset.brightness,
            preset.contrast,
            preset.saturation,
            preset.mode,
        )
        .await
    } else {
        Err(format!("未找到预设: {}", preset_id))
    }
}

pub fn cleanup() {
    #[cfg(target_os = "windows")]
    {
        ensure_display_states();
        let num_displays = {
            let lock = DISPLAY_STATES.lock().unwrap();
            let states = lock.as_ref().unwrap();
            for state_mutex in states.iter() {
                let mut state = state_mutex.lock().unwrap();
                state.filter_active = false;
                state.icc_active = false;
            }
            states.len()
        };
        // 在 DISPLAY_STATES 锁释放后逐个恢复 Gamma，
        // 避免 restore_original_gamma_for_display → with_display_state
        // → ensure_display_states → DISPLAY_STATES.lock() 的重入死锁
        for i in 0..num_displays {
            let _ = restore_original_gamma_for_display(i);
        }
    }
}

// ─── Custom filter settings commands ───

#[tauri::command]
pub async fn get_custom_filter_settings(
    display_index: Option<usize>,
) -> Result<CustomFilterSettings, String> {
    let idx = resolve_display_index(display_index);
    let all_settings = get_or_load_custom_settings();
    Ok(all_settings.get(&idx).cloned().unwrap_or_default())
}

#[tauri::command]
pub async fn save_custom_filter_settings(
    display_index: Option<usize>,
    temperature: i32,
    brightness: i32,
    contrast: i32,
    saturation: i32,
) -> Result<CustomFilterSettings, String> {
    let idx = resolve_display_index(display_index);
    let settings = CustomFilterSettings {
        temperature: temperature.clamp(1000, 10000),
        brightness: brightness.clamp(50, 150),
        contrast: contrast.clamp(50, 150),
        saturation: saturation.clamp(50, 150),
    };

    let mut all_settings = get_or_load_custom_settings();
    all_settings.insert(idx, settings.clone());

    save_custom_settings_to_file(&all_settings)?;

    let mut settings_lock = CUSTOM_SETTINGS.lock().unwrap();
    *settings_lock = Some(all_settings);

    Ok(settings)
}

// ─── ICC Profile Support ───

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct IccPreset {
    pub id: String,
    pub name: String,
    pub ramp: Vec<Vec<u16>>,
    pub description: String,
}

impl IccPreset {
    fn to_ramp_array(&self) -> [[u16; 256]; 3] {
        let mut ramp = [[0u16; 256]; 3];
        for c in 0..3 {
            for i in 0..256 {
                ramp[c][i] = self.ramp[c][i];
            }
        }
        ramp
    }
}

#[derive(serde::Serialize, Clone)]
pub struct IccPresetInfo {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(serde::Serialize)]
pub struct IccImportResult {
    pub success: bool,
    pub message: String,
    pub preset: Option<IccPresetInfo>,
}

static ICC_PRESETS: Mutex<Option<Vec<IccPreset>>> = Mutex::new(None);

fn get_icc_file_path() -> PathBuf {
    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    config_dir.join("NexBox").join("icc_presets.json")
}

fn load_icc_presets_from_file() -> Vec<IccPreset> {
    let path = get_icc_file_path();
    if path.exists() {
        match fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<Vec<IccPreset>>(&content) {
                Ok(presets) => return presets,
                Err(e) => log::error!("解析 ICC 预设文件失败: {}", e),
            },
            Err(e) => log::error!("读取 ICC 预设文件失败: {}", e),
        }
    }
    Vec::new()
}

fn save_icc_presets_to_file(presets: &[IccPreset]) -> Result<(), String> {
    let path = get_icc_file_path();
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("无法创建目录: {}", e))?;
        }
    }
    let json_str =
        serde_json::to_string_pretty(presets).map_err(|e| format!("序列化失败: {}", e))?;
    fs::write(&path, json_str).map_err(|e| format!("无法保存: {}", e))?;
    Ok(())
}

fn get_or_load_icc_presets() -> Vec<IccPreset> {
    let mut lock = ICC_PRESETS.lock().unwrap();
    if lock.is_none() {
        let presets = load_icc_presets_from_file();
        *lock = Some(presets.clone());
        presets
    } else {
        lock.as_ref().unwrap().clone()
    }
}

fn read_u32_be(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn read_u16_be(data: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([data[offset], data[offset + 1]])
}

/// Parse an ICC profile file and extract TRC curves as a gamma ramp.
fn parse_icc_file(file_path: &str) -> Result<IccPreset, String> {
    let mut file = fs::File::open(file_path).map_err(|e| format!("无法打开文件: {}", e))?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .map_err(|e| format!("无法读取文件: {}", e))?;

    if data.len() < 132 {
        return Err("文件太小，不是有效的 ICC 文件".to_string());
    }

    // Verify ICC magic number at offset 36 (acsp)
    let magic = &data[36..40];
    if magic != b"acsp" {
        return Err("不是有效的 ICC 文件（magic number 不正确）".to_string());
    }

    let profile_size = read_u32_be(&data, 0) as usize;
    if data.len() < profile_size {
        return Err("ICC 文件大小不匹配".to_string());
    }

    // Tag table starts at offset 128
    let tag_count = read_u32_be(&data, 128) as usize;
    if data.len() < 132 + tag_count * 12 {
        return Err("ICC 标签表损坏".to_string());
    }

    // Find vcgt, rTRC, gTRC, bTRC tag offsets
    let mut vcgt_offset: Option<u32> = None;
    let mut r_trc_offset: Option<u32> = None;
    let mut g_trc_offset: Option<u32> = None;
    let mut b_trc_offset: Option<u32> = None;

    for i in 0..tag_count {
        let tag_start = 132 + i * 12;
        let tag_sig = &data[tag_start..tag_start + 4];
        let tag_offset = read_u32_be(&data, tag_start + 4);
        let _tag_size = read_u32_be(&data, tag_start + 8);

        match tag_sig {
            b"vcgt" => vcgt_offset = Some(tag_offset),
            b"rTRC" => r_trc_offset = Some(tag_offset),
            b"gTRC" => g_trc_offset = Some(tag_offset),
            b"bTRC" => b_trc_offset = Some(tag_offset),
            _ => {}
        }
    }

    // If we don't have RGB TRCs, try kTRC (grayscale)
    if r_trc_offset.is_none() {
        for i in 0..tag_count {
            let tag_start = 132 + i * 12;
            let tag_sig = &data[tag_start..tag_start + 4];
            if tag_sig == b"kTRC" {
                let offset = read_u32_be(&data, tag_start + 4);
                r_trc_offset = Some(offset);
                g_trc_offset = Some(offset);
                b_trc_offset = Some(offset);
                break;
            }
        }
    }

    fn read_s15fixed16(data: &[u8], offset: usize) -> f64 {
        let raw = i32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        raw as f64 / 65536.0
    }

    let parse_curve = |offset: u32| -> Result<[u16; 256], String> {
        let off = offset as usize;
        if off + 12 > data.len() {
            return Err("曲线数据偏移超出文件范围".to_string());
        }
        let curve_type = &data[off..off + 4];

        let mut ramp = [0u16; 256];

        if curve_type == b"curv" {
            let count = read_u32_be(&data, off + 8) as usize;
            if off + 12 + count * 2 > data.len() {
                return Err("曲线数据长度超出文件范围".to_string());
            }
            if count == 0 {
                for i in 0..256 {
                    ramp[i] = (i * 257) as u16;
                }
            } else if count == 1 {
                let gamma = read_u16_be(&data, off + 12) as f64 / 256.0;
                for i in 0..256 {
                    let input = i as f64 / 255.0;
                    let output = input.powf(gamma) * 65535.0;
                    ramp[i] = output.clamp(0.0, 65535.0) as u16;
                }
            } else {
                for i in 0..256 {
                    let src_idx = (i as f64 / 255.0 * (count - 1) as f64) as usize;
                    let frac = (i as f64 / 255.0 * (count - 1) as f64) - src_idx as f64;
                    let v0 = read_u16_be(&data, off + 12 + src_idx * 2);
                    let v1 = if src_idx + 1 < count {
                        read_u16_be(&data, off + 12 + (src_idx + 1) * 2)
                    } else {
                        v0
                    };
                    ramp[i] =
                        ((v0 as f64 + (v1 as f64 - v0 as f64) * frac) as u16).min(65535);
                }
            }
        } else if curve_type == b"para" {
            if off + 16 > data.len() {
                return Err("参数化曲线数据不完整".to_string());
            }
            let func_type = read_u16_be(&data, off + 8);
            let params_offset = off + 12;

            for i in 0..256 {
                let x = i as f64 / 255.0;
                let y = match func_type {
                    // ICC v4 spec formulas (Annex A, Table 45)
                    // Type 0: Y = X^g (1 param)
                    0 => {
                        let g = read_s15fixed16(&data, params_offset);
                        x.powf(g)
                    }
                    // Type 1: Y = (aX + b)^g (3 params)
                    1 => {
                        let g = read_s15fixed16(&data, params_offset);
                        let a = read_s15fixed16(&data, params_offset + 4);
                        let b = read_s15fixed16(&data, params_offset + 8);
                        let threshold = if a.abs() > 1e-10 { -b / a } else { 0.0 };
                        if x >= threshold {
                            (a * x + b).max(0.0).powf(g)
                        } else {
                            0.0
                        }
                    }
                    // Type 2: Y = (aX + b)^g + c (4 params)
                    2 => {
                        let g = read_s15fixed16(&data, params_offset);
                        let a = read_s15fixed16(&data, params_offset + 4);
                        let b = read_s15fixed16(&data, params_offset + 8);
                        let c = read_s15fixed16(&data, params_offset + 12);
                        let threshold = if a.abs() > 1e-10 { -b / a } else { 0.0 };
                        if x >= threshold {
                            (a * x + b).max(0.0).powf(g) + c
                        } else {
                            c
                        }
                    }
                    // Type 3: Y = (aX + b)^g + c, X >= d; Y = cX, X < d (5 params)
                    // Note: c is used for BOTH the offset and the linear slope!
                    3 => {
                        let g = read_s15fixed16(&data, params_offset);
                        let a = read_s15fixed16(&data, params_offset + 4);
                        let b = read_s15fixed16(&data, params_offset + 8);
                        let c = read_s15fixed16(&data, params_offset + 12);
                        let d = read_s15fixed16(&data, params_offset + 16);
                        if x >= d {
                            (a * x + b).max(0.0).powf(g) + c
                        } else {
                            c * x
                        }
                    }
                    // Type 4: Y = (aX + b)^g + e, X >= d; Y = cX + f, X < d (7 params)
                    4 => {
                        let g = read_s15fixed16(&data, params_offset);
                        let a = read_s15fixed16(&data, params_offset + 4);
                        let b = read_s15fixed16(&data, params_offset + 8);
                        let c = read_s15fixed16(&data, params_offset + 12);
                        let d = read_s15fixed16(&data, params_offset + 16);
                        let e = read_s15fixed16(&data, params_offset + 20);
                        let f = read_s15fixed16(&data, params_offset + 24);
                        if x >= d {
                            (a * x + b).max(0.0).powf(g) + e
                        } else {
                            c * x + f
                        }
                    }
                    _ => {
                        return Err(format!(
                            "不支持的参数化曲线函数类型: {}",
                            func_type
                        ))
                    }
                };
                let output = y.clamp(0.0, 1.0) * 65535.0;
                ramp[i] = output.clamp(0.0, 65535.0) as u16;
            }
        } else {
            return Err(format!(
                "不支持的曲线类型: {:?}（仅支持 'curv' 和 'para'）",
                std::str::from_utf8(curve_type).unwrap_or("?")
            ));
        }
        Ok(ramp)
    };

    // Parse vcgt tag if available (preferred over TRC for SetDeviceGammaRamp)
    let parse_vcgt = |offset: u32| -> Result<[[u16; 256]; 3], String> {
        let off = offset as usize;
        if off + 18 > data.len() {
            return Err("vcgt 数据不完整".to_string());
        }
        let formula_type = read_u32_be(&data, off + 8);
        if formula_type != 0 {
            return Err(format!(
                "不支持的 vcgt 公式类型: {}（仅支持类型 0 表格）",
                formula_type
            ));
        }
        let channels = read_u16_be(&data, off + 12) as usize;
        let entries = read_u16_be(&data, off + 14) as usize;
        let entry_size = read_u16_be(&data, off + 16) as usize;

        if channels != 3 || entries != 256 || entry_size != 2 {
            return Err(format!(
                "不支持的 vcgt 格式: channels={}, entries={}, entry_size={}（需要 3x256x2）",
                channels, entries, entry_size
            ));
        }

        let data_start = off + 18;
        let data_end = data_start + channels * entries * entry_size;
        if data_end > data.len() {
            return Err("vcgt 数据超出文件范围".to_string());
        }

        let mut ramp = [[0u16; 256]; 3];
        // Planar format: R channel first, then G, then B
        for ch in 0..3 {
            let ch_start = data_start + ch * entries * entry_size;
            for i in 0..entries {
                let val = read_u16_be(&data, ch_start + i * entry_size);
                ramp[ch][i] = val;
            }
        }
        Ok(ramp)
    };

    // Use vcgt if available, otherwise fall back to TRC curves
    let mut ramp = if let Some(vcgt_off) = vcgt_offset {
        match parse_vcgt(vcgt_off) {
            Ok(vcgt_ramp) => {
                log::info!("Using vcgt tag for gamma ramp");
                vcgt_ramp
            }
            Err(e) => {
                log::warn!("vcgt 解析失败: {}，回退到 TRC 曲线", e);
                // Fall through to TRC parsing below
                let r_trc_off =
                    r_trc_offset.ok_or("ICC 文件中未找到 rTRC 曲线".to_string())?;
                let g_trc_off =
                    g_trc_offset.ok_or("ICC 文件中未找到 gTRC 曲线".to_string())?;
                let b_trc_off =
                    b_trc_offset.ok_or("ICC 文件中未找到 bTRC 曲线".to_string())?;
                let r_ramp = parse_curve(r_trc_off)?;
                let g_ramp = parse_curve(g_trc_off)?;
                let b_ramp = parse_curve(b_trc_off)?;
                [r_ramp, g_ramp, b_ramp]
            }
        }
    } else {
        let r_trc_off = r_trc_offset.ok_or("ICC 文件中未找到 rTRC 曲线".to_string())?;
        let g_trc_off = g_trc_offset.ok_or("ICC 文件中未找到 gTRC 曲线".to_string())?;
        let b_trc_off = b_trc_offset.ok_or("ICC 文件中未找到 bTRC 曲线".to_string())?;
        let r_ramp = parse_curve(r_trc_off)?;
        let g_ramp = parse_curve(g_trc_off)?;
        let b_ramp = parse_curve(b_trc_off)?;
        [r_ramp, g_ramp, b_ramp]
    };

    // Extract file name for display
    let name = std::path::Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("ICC Profile")
        .to_string();

    let id = uuid::Uuid::new_v4().to_string();

    // Apply EXACTLY the same post-processing as build_gamma_ramp!
    // This matches what the working built-in filters do!
    for channel in 0..3 {
        for i in 1..256 {
            if ramp[channel][i] < ramp[channel][i - 1] {
                ramp[channel][i] = ramp[channel][i - 1];
            }
        }
    }
    ramp[0][0] = 0;
    ramp[1][0] = 0;
    ramp[2][0] = 0;
    ramp[0][255] = 65535;
    ramp[1][255] = 65535;
    ramp[2][255] = 65535;

    log::info!(
        "ICC ramp ready: R[0]={}, R[64]={}, R[128]={}, R[192]={}, R[255]={}",
        ramp[0][0],
        ramp[0][64],
        ramp[0][128],
        ramp[0][192],
        ramp[0][255]
    );

    // Verify ramp is properly scaled to 16-bit
    if ramp[0][128] < 1000 {
        log::error!(
            "ICC ramp values appear to be 8-bit instead of 16-bit! R[128]={} should be ~32000",
            ramp[0][128]
        );
    }

    Ok(IccPreset {
        id,
        name,
        ramp: vec![ramp[0].to_vec(), ramp[1].to_vec(), ramp[2].to_vec()],
        description: format!(
            "从 {} 导入",
            std::path::Path::new(file_path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("未知")
        ),
    })
}

#[tauri::command]
pub async fn select_icc_file() -> Result<Option<String>, String> {
    #[cfg(target_os = "windows")]
    {
        let result = rfd::FileDialog::new()
            .set_title("选择 ICC 色彩配置文件")
            .add_filter("ICC 文件", &["icc", "icm"])
            .pick_file();

        Ok(result.and_then(|p| p.to_str().map(|s| s.to_string())))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows 系统".to_string())
    }
}

#[tauri::command]
pub async fn import_icc_profile(path: String) -> Result<IccImportResult, String> {
    #[cfg(target_os = "windows")]
    {
        let preset = parse_icc_file(&path)?;

        let info = IccPresetInfo {
            id: preset.id.clone(),
            name: preset.name.clone(),
            description: preset.description.clone(),
        };

        let mut lock = ICC_PRESETS.lock().unwrap();
        let mut presets = if lock.is_some() {
            lock.take().unwrap()
        } else {
            load_icc_presets_from_file()
        };
        presets.push(preset);
        save_icc_presets_to_file(&presets)?;
        *lock = Some(presets);

        Ok(IccImportResult {
            success: true,
            message: "ICC 文件已导入".to_string(),
            preset: Some(info),
        })
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows 系统".to_string())
    }
}

#[tauri::command]
pub async fn get_icc_presets() -> Result<Vec<IccPresetInfo>, String> {
    #[cfg(target_os = "windows")]
    {
        let presets = get_or_load_icc_presets();
        Ok(presets
            .iter()
            .map(|p| IccPresetInfo {
                id: p.id.clone(),
                name: p.name.clone(),
                description: p.description.clone(),
            })
            .collect())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(Vec::new())
    }
}

#[tauri::command]
pub async fn apply_icc_preset(
    display_index: Option<usize>,
    id: String,
) -> Result<FilterResult, String> {
    #[cfg(target_os = "windows")]
    {
        let idx = resolve_display_index(display_index);
        let presets = get_or_load_icc_presets();
        let preset = presets
            .iter()
            .find(|p| p.id == id)
            .ok_or("未找到 ICC 预设".to_string())?;

        let ramp_array = preset.to_ramp_array();

        log::info!(
            "Applying ICC preset '{}' to monitor[{}]: R[0]={}, R[64]={}, R[128]={}, R[192]={}, R[255]={}",
            preset.name, idx,
            ramp_array[0][0], ramp_array[0][64], ramp_array[0][128], ramp_array[0][192], ramp_array[0][255]
        );

        // Check if saved ramp values are valid 16-bit
        if ramp_array[0][128] < 1000 {
            log::error!(
                "Saved ICC ramp appears corrupted (8-bit values). Please delete and re-import the ICC file."
            );
        }

        with_display_state(idx, |state| {
            state.icc_ramp = Some(ramp_array);
            state.icc_active = true;

            if !state.filter_active {
                if state.original_gamma.is_none() {
                    if let Ok(ramp) = get_current_gamma_ramp_for_display(idx) {
                        state.original_gamma = Some(ramp);
                    }
                }
                state.filter_active = true;
            }
        });

        // Apply ICC ramp first, then start monitor
        set_gamma_ramp_for_display(idx, &ramp_array)?;

        start_filter_monitor();

        Ok(with_display_state(idx, |state| FilterResult {
            success: true,
            message: format!("ICC 预设 {} 已应用", preset.name),
            settings: Some(FilterSettings {
                temperature: state.temperature,
                brightness: state.brightness,
                contrast: state.contrast,
                saturation: state.saturation,
                mode: state.mode,
                is_active: true,
            }),
        }))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows 系统".to_string())
    }
}

#[tauri::command]
pub async fn delete_icc_preset(id: String) -> Result<FilterResult, String> {
    #[cfg(target_os = "windows")]
    {
        let mut lock = ICC_PRESETS.lock().unwrap();
        let mut presets = if lock.is_some() {
            lock.take().unwrap()
        } else {
            load_icc_presets_from_file()
        };

        let len_before = presets.len();
        presets.retain(|p| p.id != id);

        if presets.len() == len_before {
            *lock = Some(presets);
            return Err("未找到要删除的 ICC 预设".to_string());
        }

        save_icc_presets_to_file(&presets)?;
        *lock = Some(presets);

        Ok(FilterResult {
            success: true,
            message: "ICC 预设已删除".to_string(),
            settings: None,
        })
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows 系统".to_string())
    }
}