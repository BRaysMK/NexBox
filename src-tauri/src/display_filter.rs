use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::path::PathBuf;
use std::fs;

static FILTER_ACTIVE: AtomicBool = AtomicBool::new(false);
static CURRENT_TEMPERATURE: AtomicI32 = AtomicI32::new(6500);
static CURRENT_BRIGHTNESS: AtomicI32 = AtomicI32::new(100);
static CURRENT_CONTRAST: AtomicI32 = AtomicI32::new(100);
static CURRENT_SATURATION: AtomicI32 = AtomicI32::new(100);
static CURRENT_MODE: AtomicI32 = AtomicI32::new(0);

static ORIGINAL_GAMMA: Mutex<Option<[[u16; 256]; 3]>> = Mutex::new(None);
static FILTER_THREAD_RUNNING: AtomicBool = AtomicBool::new(false);

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
    let temperature = CURRENT_TEMPERATURE.load(Ordering::SeqCst);
    let brightness = CURRENT_BRIGHTNESS.load(Ordering::SeqCst);
    let contrast = CURRENT_CONTRAST.load(Ordering::SeqCst);
    let saturation = CURRENT_SATURATION.load(Ordering::SeqCst);
    let mode = FilterMode::from_i32(CURRENT_MODE.load(Ordering::SeqCst));
    
    let ramp = build_gamma_ramp(temperature, brightness, contrast, saturation, mode);
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
