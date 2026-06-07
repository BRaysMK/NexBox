use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::path::PathBuf;
use std::fs;
use std::io::Read;
use tauri::Emitter;

static FILTER_ACTIVE: AtomicBool = AtomicBool::new(false);
static CURRENT_TEMPERATURE: AtomicI32 = AtomicI32::new(6500);
static CURRENT_BRIGHTNESS: AtomicI32 = AtomicI32::new(100);
static CURRENT_CONTRAST: AtomicI32 = AtomicI32::new(100);
static CURRENT_SATURATION: AtomicI32 = AtomicI32::new(100);
static CURRENT_MODE: AtomicI32 = AtomicI32::new(0);

static ORIGINAL_GAMMA: Mutex<Option<[[u16; 256]; 3]>> = Mutex::new(None);
static FILTER_THREAD_RUNNING: AtomicBool = AtomicBool::new(false);
static ICC_RAMP_ACTIVE: AtomicBool = AtomicBool::new(false);
static CURRENT_ICC_RAMP: Mutex<Option<[[u16; 256]; 3]>> = Mutex::new(None);
static GAMMA_RAMP_MUTEX: Mutex<()> = Mutex::new(());

#[derive(serde::Serialize, Clone, Copy, PartialEq)]
pub enum FilterMode {
    Normal = 0,
    Vivid = 1,
    Movie = 2,
    Highlight = 3,
    Soft = 4,
    Gaming = 5,
    Reading = 6,
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

static CUSTOM_SETTINGS: Mutex<Option<CustomFilterSettings>> = Mutex::new(None);

fn get_settings_file_path() -> PathBuf {
    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    config_dir.join("NexBox").join("settings.json")
}

fn load_custom_settings_from_file() -> CustomFilterSettings {
    let path = get_settings_file_path();
    if path.exists() {
        match fs::read_to_string(&path) {
            Ok(content) => {
                match serde_json::from_str::<serde_json::Value>(&content) {
                    Ok(json) => {
                        if let Some(settings_value) = json.get("custom-filter-settings") {
                            match serde_json::from_value::<CustomFilterSettings>(settings_value.clone()) {
                                Ok(settings) => {
                                    return settings;
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
    CustomFilterSettings::default()
}

fn save_custom_settings_to_file(settings: &CustomFilterSettings) -> Result<(), String> {
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
    
    existing_settings["custom-filter-settings"] = serde_json::to_value(settings).unwrap();
    
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

fn get_or_load_custom_settings() -> CustomFilterSettings {
    let mut settings_lock = CUSTOM_SETTINGS.lock().unwrap();
    if settings_lock.is_none() {
        let settings = load_custom_settings_from_file();
        *settings_lock = Some(settings.clone());
        settings
    } else {
        settings_lock.as_ref().unwrap().clone()
    }
}

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

#[cfg(target_os = "windows")]
fn set_gamma_ramp(ramp: &[[u16; 256]; 3]) -> Result<(), String> {
    let _guard = GAMMA_RAMP_MUTEX.lock().map_err(|_| "无法获取 Gamma Ramp 锁".to_string())?;
    use windows_sys::Win32::Graphics::Gdi::{GetDC, ReleaseDC};
    use windows_sys::Win32::UI::ColorSystem::SetDeviceGammaRamp;
    
    unsafe {
        let hdc = GetDC(std::ptr::null_mut());
        if hdc.is_null() {
            return Err("无法获取设备上下文".to_string());
        }
        
        let result = SetDeviceGammaRamp(hdc, ramp.as_ptr() as *const _);
        ReleaseDC(std::ptr::null_mut(), hdc);
        
        if result == 0 {
            log::error!(
                "SetDeviceGammaRamp 失败! R[0..5]=[{},{},{},{},{}] R[251..255]=[{},{},{},{},{}]",
                ramp[0][0], ramp[0][1], ramp[0][2], ramp[0][3], ramp[0][4],
                ramp[0][251], ramp[0][252], ramp[0][253], ramp[0][254], ramp[0][255],
            );
            return Err("设置 Gamma Ramp 失败，可能是显卡驱动不支持".to_string());
        }
    }
    
    Ok(())
}

#[cfg(target_os = "windows")]
fn get_current_gamma_ramp() -> Result<[[u16; 256]; 3], String> {
    use windows_sys::Win32::Graphics::Gdi::{GetDC, ReleaseDC};
    use windows_sys::Win32::UI::ColorSystem::GetDeviceGammaRamp;
    
    let mut ramp = [[0u16; 256]; 3];
    
    unsafe {
        let hdc = GetDC(std::ptr::null_mut());
        if hdc.is_null() {
            return Err("无法获取设备上下文".to_string());
        }
        
        let result = GetDeviceGammaRamp(hdc, ramp.as_mut_ptr() as *mut _);
        ReleaseDC(std::ptr::null_mut(), hdc);
        
        if result == 0 {
            for i in 0..256 {
                let value = (i * 257) as u16;
                ramp[0][i] = value;
                ramp[1][i] = value;
                ramp[2][i] = value;
            }
        }
    }
    
    Ok(ramp)
}

#[cfg(not(target_os = "windows"))]
fn set_gamma_ramp(_ramp: &[[u16; 256]; 3]) -> Result<(), String> {
    Err("此功能仅支持 Windows 系统".to_string())
}

#[cfg(not(target_os = "windows"))]
fn get_current_gamma_ramp() -> Result<[[u16; 256]; 3], String> {
    Err("此功能仅支持 Windows 系统".to_string())
}

fn apply_filter_internal() -> Result<(), String> {
    if ICC_RAMP_ACTIVE.load(Ordering::SeqCst) {
        if let Ok(icc_ramp) = CURRENT_ICC_RAMP.lock() {
            if let Some(ref ramp) = *icc_ramp {
                log::info!("Monitor: applying ICC ramp (R[0]={}, R[128]={}, R[255]={})", ramp[0][0], ramp[0][128], ramp[0][255]);
                return set_gamma_ramp(ramp);
            }
        }
    }

    let temperature = CURRENT_TEMPERATURE.load(Ordering::SeqCst);
    let brightness = CURRENT_BRIGHTNESS.load(Ordering::SeqCst);
    let contrast = CURRENT_CONTRAST.load(Ordering::SeqCst);
    let saturation = CURRENT_SATURATION.load(Ordering::SeqCst);
    let mode = FilterMode::from_i32(CURRENT_MODE.load(Ordering::SeqCst));
    
    let ramp = build_gamma_ramp(temperature, brightness, contrast, saturation, mode);
    log::info!("Monitor: applying regular filter ramp");
    set_gamma_ramp(&ramp)
}

fn restore_original_gamma() -> Result<(), String> {
    let original = ORIGINAL_GAMMA.lock().map_err(|_| "无法获取原始 Gamma 锁".to_string())?;
    
    if let Some(ref ramp) = *original {
        set_gamma_ramp(ramp)?;
    } else {
        let identity_ramp = build_gamma_ramp(6500, 100, 100, 100, FilterMode::Normal);
        set_gamma_ramp(&identity_ramp)?;
    }
    
    Ok(())
}

fn start_filter_monitor() {
    if FILTER_THREAD_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    
    thread::spawn(|| {
        while FILTER_ACTIVE.load(Ordering::SeqCst) {
            if let Err(e) = apply_filter_internal() {
                log::error!("应用滤镜失败: {}", e);
            }
            
            thread::sleep(Duration::from_millis(1000));
        }
        
        FILTER_THREAD_RUNNING.store(false, Ordering::SeqCst);
    });
}

#[tauri::command]
pub async fn get_filter_settings() -> Result<FilterSettings, String> {
    Ok(FilterSettings {
        temperature: CURRENT_TEMPERATURE.load(Ordering::SeqCst),
        brightness: CURRENT_BRIGHTNESS.load(Ordering::SeqCst),
        contrast: CURRENT_CONTRAST.load(Ordering::SeqCst),
        saturation: CURRENT_SATURATION.load(Ordering::SeqCst),
        mode: CURRENT_MODE.load(Ordering::SeqCst),
        is_active: FILTER_ACTIVE.load(Ordering::SeqCst),
    })
}

#[tauri::command]
pub async fn set_filter_settings(
    temperature: i32,
    brightness: i32,
    contrast: i32,
    saturation: i32,
    mode: i32,
) -> Result<FilterResult, String> {
    #[cfg(target_os = "windows")]
    {
        let temperature = temperature.clamp(1000, 10000);
        let brightness = brightness.clamp(50, 150);
        let contrast = contrast.clamp(50, 150);
        let saturation = saturation.clamp(50, 150);
        let mode = mode.clamp(0, 6);
        
        CURRENT_TEMPERATURE.store(temperature, Ordering::SeqCst);
        CURRENT_BRIGHTNESS.store(brightness, Ordering::SeqCst);
        CURRENT_CONTRAST.store(contrast, Ordering::SeqCst);
        CURRENT_SATURATION.store(saturation, Ordering::SeqCst);
        CURRENT_MODE.store(mode, Ordering::SeqCst);
        
        // Clear ICC ramp when applying regular filter settings
        ICC_RAMP_ACTIVE.store(false, Ordering::SeqCst);

        if !FILTER_ACTIVE.load(Ordering::SeqCst) {
            if let Ok(mut original) = ORIGINAL_GAMMA.lock() {
                if original.is_none() {
                    if let Ok(ramp) = get_current_gamma_ramp() {
                        *original = Some(ramp);
                    }
                }
            }
            FILTER_ACTIVE.store(true, Ordering::SeqCst);
            start_filter_monitor();
        }
        
        apply_filter_internal()?;
        
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
pub async fn enable_filter() -> Result<FilterResult, String> {
    #[cfg(target_os = "windows")]
    {
        if FILTER_ACTIVE.load(Ordering::SeqCst) {
            return Ok(FilterResult {
                success: true,
                message: "滤镜已处于启用状态".to_string(),
                settings: Some(FilterSettings {
                    temperature: CURRENT_TEMPERATURE.load(Ordering::SeqCst),
                    brightness: CURRENT_BRIGHTNESS.load(Ordering::SeqCst),
                    contrast: CURRENT_CONTRAST.load(Ordering::SeqCst),
                    saturation: CURRENT_SATURATION.load(Ordering::SeqCst),
                    mode: CURRENT_MODE.load(Ordering::SeqCst),
                    is_active: true,
                }),
            });
        }
        
        if let Ok(mut original) = ORIGINAL_GAMMA.lock() {
            if original.is_none() {
                if let Ok(ramp) = get_current_gamma_ramp() {
                    *original = Some(ramp);
                }
            }
        }
        
        FILTER_ACTIVE.store(true, Ordering::SeqCst);
        apply_filter_internal()?;
        start_filter_monitor();
        
        Ok(FilterResult {
            success: true,
            message: "滤镜已启用".to_string(),
            settings: Some(FilterSettings {
                temperature: CURRENT_TEMPERATURE.load(Ordering::SeqCst),
                brightness: CURRENT_BRIGHTNESS.load(Ordering::SeqCst),
                contrast: CURRENT_CONTRAST.load(Ordering::SeqCst),
                saturation: CURRENT_SATURATION.load(Ordering::SeqCst),
                mode: CURRENT_MODE.load(Ordering::SeqCst),
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
pub async fn disable_filter() -> Result<FilterResult, String> {
    #[cfg(target_os = "windows")]
    {
        if !FILTER_ACTIVE.load(Ordering::SeqCst) {
            return Ok(FilterResult {
                success: true,
                message: "滤镜已处于禁用状态".to_string(),
                settings: Some(FilterSettings {
                    temperature: CURRENT_TEMPERATURE.load(Ordering::SeqCst),
                    brightness: CURRENT_BRIGHTNESS.load(Ordering::SeqCst),
                    contrast: CURRENT_CONTRAST.load(Ordering::SeqCst),
                    saturation: CURRENT_SATURATION.load(Ordering::SeqCst),
                    mode: CURRENT_MODE.load(Ordering::SeqCst),
                    is_active: false,
                }),
            });
        }
        
        FILTER_ACTIVE.store(false, Ordering::SeqCst);
        
        // Clear ICC ramp when disabling filter
        ICC_RAMP_ACTIVE.store(false, Ordering::SeqCst);

        if let Err(e) = restore_original_gamma() {
            log::error!("恢复原始 Gamma 失败: {}", e);
        }
        
        if let Ok(mut original) = ORIGINAL_GAMMA.lock() {
            *original = None;
        }
        
        Ok(FilterResult {
            success: true,
            message: "滤镜已禁用".to_string(),
            settings: Some(FilterSettings {
                temperature: CURRENT_TEMPERATURE.load(Ordering::SeqCst),
                brightness: CURRENT_BRIGHTNESS.load(Ordering::SeqCst),
                contrast: CURRENT_CONTRAST.load(Ordering::SeqCst),
                saturation: CURRENT_SATURATION.load(Ordering::SeqCst),
                mode: CURRENT_MODE.load(Ordering::SeqCst),
                is_active: false,
            }),
        })
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows 系统".to_string())
    }
}

#[tauri::command]
pub async fn toggle_filter() -> Result<FilterResult, String> {
    if FILTER_ACTIVE.load(Ordering::SeqCst) {
        disable_filter().await
    } else {
        enable_filter().await
    }
}

/// Toggle filter on/off. Used by global hotkey.
pub fn toggle_filter_sync(app_handle: &tauri::AppHandle) -> Result<FilterResult, String> {
    #[cfg(target_os = "windows")]
    {
        let result = if FILTER_ACTIVE.load(Ordering::SeqCst) {
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
    if FILTER_ACTIVE.load(Ordering::SeqCst) {
        return Ok(FilterResult {
            success: true,
            message: "滤镜已处于启用状态".to_string(),
            settings: Some(FilterSettings {
                temperature: CURRENT_TEMPERATURE.load(Ordering::SeqCst),
                brightness: CURRENT_BRIGHTNESS.load(Ordering::SeqCst),
                contrast: CURRENT_CONTRAST.load(Ordering::SeqCst),
                saturation: CURRENT_SATURATION.load(Ordering::SeqCst),
                mode: CURRENT_MODE.load(Ordering::SeqCst),
                is_active: true,
            }),
        });
    }

    if let Ok(mut original) = ORIGINAL_GAMMA.lock() {
        if original.is_none() {
            if let Ok(ramp) = get_current_gamma_ramp() {
                *original = Some(ramp);
            }
        }
    }

    FILTER_ACTIVE.store(true, Ordering::SeqCst);
    apply_filter_internal()?;
    start_filter_monitor();

    Ok(FilterResult {
        success: true,
        message: "滤镜已启用".to_string(),
        settings: Some(FilterSettings {
            temperature: CURRENT_TEMPERATURE.load(Ordering::SeqCst),
            brightness: CURRENT_BRIGHTNESS.load(Ordering::SeqCst),
            contrast: CURRENT_CONTRAST.load(Ordering::SeqCst),
            saturation: CURRENT_SATURATION.load(Ordering::SeqCst),
            mode: CURRENT_MODE.load(Ordering::SeqCst),
            is_active: true,
        }),
    })
}

#[cfg(target_os = "windows")]
fn disable_filter_sync() -> Result<FilterResult, String> {
    if !FILTER_ACTIVE.load(Ordering::SeqCst) {
        return Ok(FilterResult {
            success: true,
            message: "滤镜已处于禁用状态".to_string(),
            settings: Some(FilterSettings {
                temperature: CURRENT_TEMPERATURE.load(Ordering::SeqCst),
                brightness: CURRENT_BRIGHTNESS.load(Ordering::SeqCst),
                contrast: CURRENT_CONTRAST.load(Ordering::SeqCst),
                saturation: CURRENT_SATURATION.load(Ordering::SeqCst),
                mode: CURRENT_MODE.load(Ordering::SeqCst),
                is_active: false,
            }),
        });
    }

    FILTER_ACTIVE.store(false, Ordering::SeqCst);

    ICC_RAMP_ACTIVE.store(false, Ordering::SeqCst);

    if let Err(e) = restore_original_gamma() {
        log::error!("恢复原始 Gamma 失败: {}", e);
    }

    Ok(FilterResult {
        success: true,
        message: "滤镜已禁用".to_string(),
        settings: Some(FilterSettings {
            temperature: CURRENT_TEMPERATURE.load(Ordering::SeqCst),
            brightness: CURRENT_BRIGHTNESS.load(Ordering::SeqCst),
            contrast: CURRENT_CONTRAST.load(Ordering::SeqCst),
            saturation: CURRENT_SATURATION.load(Ordering::SeqCst),
            mode: CURRENT_MODE.load(Ordering::SeqCst),
            is_active: false,
        }),
    })
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
    ])
}

#[tauri::command]
pub async fn apply_preset(preset_id: String) -> Result<FilterResult, String> {
    let presets = get_filter_presets().await?;
    
    if let Some(preset) = presets.iter().find(|p| p.id == preset_id) {
        set_filter_settings(
            preset.temperature,
            preset.brightness,
            preset.contrast,
            preset.saturation,
            preset.mode,
        ).await
    } else {
        Err(format!("未找到预设: {}", preset_id))
    }
}

pub fn cleanup() {
    FILTER_ACTIVE.store(false, Ordering::SeqCst);
    ICC_RAMP_ACTIVE.store(false, Ordering::SeqCst);
    
    #[cfg(target_os = "windows")]
    {
        let _ = restore_original_gamma();
    }
}

#[tauri::command]
pub async fn get_custom_filter_settings() -> Result<CustomFilterSettings, String> {
    Ok(get_or_load_custom_settings())
}

#[tauri::command]
pub async fn save_custom_filter_settings(
    temperature: i32,
    brightness: i32,
    contrast: i32,
    saturation: i32,
) -> Result<CustomFilterSettings, String> {
    let settings = CustomFilterSettings {
        temperature: temperature.clamp(1000, 10000),
        brightness: brightness.clamp(50, 150),
        contrast: contrast.clamp(50, 150),
        saturation: saturation.clamp(50, 150),
    };
    
    save_custom_settings_to_file(&settings)?;
    
    {
        let mut settings_lock = CUSTOM_SETTINGS.lock().unwrap();
        *settings_lock = Some(settings.clone());
    }
    
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
    let json_str = serde_json::to_string_pretty(presets).map_err(|e| format!("序列化失败: {}", e))?;
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
    u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
}

fn read_u16_be(data: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([data[offset], data[offset + 1]])
}

/// Parse an ICC profile file and extract TRC curves as a gamma ramp.
fn parse_icc_file(file_path: &str) -> Result<IccPreset, String> {
    let mut file = fs::File::open(file_path).map_err(|e| format!("无法打开文件: {}", e))?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).map_err(|e| format!("无法读取文件: {}", e))?;

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
        let raw = i32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
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
                    ramp[i] = ((v0 as f64 + (v1 as f64 - v0 as f64) * frac) as u16).min(65535);
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
                    _ => return Err(format!("不支持的参数化曲线函数类型: {}", func_type)),
                };
                let output = y.clamp(0.0, 1.0) * 65535.0;
                ramp[i] = output.clamp(0.0, 65535.0) as u16;
            }
        } else {
            return Err(format!("不支持的曲线类型: {:?}（仅支持 'curv' 和 'para'）", std::str::from_utf8(curve_type).unwrap_or("?")));
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
            return Err(format!("不支持的 vcgt 公式类型: {}（仅支持类型 0 表格）", formula_type));
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
                let r_trc_off = r_trc_offset.ok_or("ICC 文件中未找到 rTRC 曲线".to_string())?;
                let g_trc_off = g_trc_offset.ok_or("ICC 文件中未找到 gTRC 曲线".to_string())?;
                let b_trc_off = b_trc_offset.ok_or("ICC 文件中未找到 bTRC 曲线".to_string())?;
                // Need to parse TRC curves below...
                // We'll handle this by setting flags and continuing
                // Actually, let's refactor: use TRC if vcgt fails
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
        ramp[0][0], ramp[0][64], ramp[0][128], ramp[0][192], ramp[0][255]
    );

    // Verify ramp is properly scaled to 16-bit
    if ramp[0][128] < 1000 {
        log::error!("ICC ramp values appear to be 8-bit instead of 16-bit! R[128]={} should be ~32000", ramp[0][128]);
    }

    Ok(IccPreset {
        id,
        name,
        ramp: vec![
            ramp[0].to_vec(),
            ramp[1].to_vec(),
            ramp[2].to_vec()
        ],
        description: format!("从 {} 导入", std::path::Path::new(file_path).file_name().and_then(|s| s.to_str()).unwrap_or("未知")),
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
pub async fn apply_icc_preset(id: String) -> Result<FilterResult, String> {
    #[cfg(target_os = "windows")]
    {
        let presets = get_or_load_icc_presets();
        let preset = presets.iter().find(|p| p.id == id).ok_or("未找到 ICC 预设".to_string())?;

        let ramp_array = preset.to_ramp_array();

        log::info!(
            "Applying ICC preset '{}': R[0]={}, R[64]={}, R[128]={}, R[192]={}, R[255]={}",
            preset.name, ramp_array[0][0], ramp_array[0][64], ramp_array[0][128], ramp_array[0][192], ramp_array[0][255]
        );

        // Check if saved ramp values are valid 16-bit
        if ramp_array[0][128] < 1000 {
            log::error!("Saved ICC ramp appears corrupted (8-bit values). Please delete and re-import the ICC file.");
        }

        // Store ICC ramp before starting monitor so it's available immediately
        if let Ok(mut icc_ramp) = CURRENT_ICC_RAMP.lock() {
            *icc_ramp = Some(ramp_array);
        }
        ICC_RAMP_ACTIVE.store(true, Ordering::SeqCst);

        if !FILTER_ACTIVE.load(Ordering::SeqCst) {
            if let Ok(mut original) = ORIGINAL_GAMMA.lock() {
                if original.is_none() {
                    if let Ok(ramp) = get_current_gamma_ramp() {
                        *original = Some(ramp);
                    }
                }
            }
            FILTER_ACTIVE.store(true, Ordering::SeqCst);
        }

        // Apply ICC ramp first, then start monitor
        set_gamma_ramp(&ramp_array)?;

        start_filter_monitor();

        Ok(FilterResult {
            success: true,
            message: format!("ICC 预设 {} 已应用", preset.name),
            settings: Some(FilterSettings {
                temperature: CURRENT_TEMPERATURE.load(Ordering::SeqCst),
                brightness: CURRENT_BRIGHTNESS.load(Ordering::SeqCst),
                contrast: CURRENT_CONTRAST.load(Ordering::SeqCst),
                saturation: CURRENT_SATURATION.load(Ordering::SeqCst),
                mode: CURRENT_MODE.load(Ordering::SeqCst),
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
