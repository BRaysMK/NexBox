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

// ─── CCD (QueryDisplayConfig) based enumeration ───
// 使用 Windows 现代显示配置 API (Vista+)，这是 Windows 显示设置本身使用的 API。
// 相比 GDI EnumDisplayMonitors 回调方式，CCD 更可靠、更快，且直接返回分辨率。

#[cfg(target_os = "windows")]
fn enumerate_displays_via_ccd() -> Option<Vec<DisplayInfo>> {
    use windows_sys::Win32::Devices::Display::*;
    use std::mem;

    unsafe {
        // ── Pass 1: 获取路径数和模式数 ──
        let mut path_count: u32 = 0;
        let mut mode_count: u32 = 0;
        let status = QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            std::ptr::null_mut(),
            &mut mode_count,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        if status != 0 || path_count == 0 {
            log::warn!("CCD 枚举: Pass1 失败或没有显示器 (status={})", status);
            return None;
        }

        // ── Pass 2: 分配并获取完整数据 ──
        let mut paths: Vec<DISPLAYCONFIG_PATH_INFO> = (0..path_count)
            .map(|_| mem::zeroed())
            .collect();
        let mut modes: Vec<DISPLAYCONFIG_MODE_INFO> = (0..mode_count)
            .map(|_| mem::zeroed())
            .collect();

        let status = QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            std::ptr::null_mut(),
        );
        if status != 0 {
            log::warn!("CCD 枚举: Pass2 失败 (status={})", status);
            return None;
        }

        let mut displays = Vec::new();

        for (path_idx, path) in paths.iter().enumerate() {
            let source_info = &path.sourceInfo;
            let target_info = &path.targetInfo;

            // 跳过未连接的显示器（QDC_ONLY_ACTIVE_PATHS 已经过滤，额外安全检查）
            // 在 windows-sys 中 target_info.Anonymous 是联合体，取其原始 u32 值读取 bit0=targetAvailable
            let target_flags: u32 = std::ptr::read_unaligned(&target_info.Anonymous as *const _ as *const u32);
            let target_available = target_flags & 0x01;
            if target_available == 0 {
                continue;
            }

            // 通过 source mode 获取分辨率
            let (width, height, pos_x, pos_y) = {
                let mode_idx = source_info.Anonymous.modeInfoIdx as usize;
                if mode_idx < modes.len() {
                    let mode = &modes[mode_idx];
                    if mode.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE {
                        let src = &mode.Anonymous.sourceMode;
                        (src.width as i32, src.height as i32, src.position.x, src.position.y)
                    } else {
                        continue;
                    }
                } else {
                    continue;
                }
            };

            if width <= 0 || height <= 0 {
                log::warn!("CCD: path[{}] 分辨率无效 ({}x{})", path_idx, width, height);
                continue;
            }

            let is_primary = pos_x == 0 && pos_y == 0;

            // 通过 DisplayConfigGetDeviceInfo 获取 GDI 设备名
            let device_name = {
                let mut source_name: DISPLAYCONFIG_SOURCE_DEVICE_NAME = mem::zeroed();
                source_name.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
                source_name.header.size = mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
                source_name.header.adapterId = source_info.adapterId;
                source_name.header.id = source_info.id;

                if DisplayConfigGetDeviceInfo(&mut source_name.header as *mut _ as *mut _) == 0 {
                    let len = source_name.viewGdiDeviceName
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(source_name.viewGdiDeviceName.len());
                    if len > 0 {
                        let name = String::from_utf16_lossy(&source_name.viewGdiDeviceName[..len]);
                        if !name.is_empty() {
                            name
                        } else {
                            format!("\\\\.\\DISPLAY{}", path_idx + 1)
                        }
                    } else {
                        format!("\\\\.\\DISPLAY{}", path_idx + 1)
                    }
                } else {
                    log::warn!("CCD: DisplayConfigGetDeviceInfo 失败，使用 DISPLAY{}", path_idx + 1);
                    format!("\\\\.\\DISPLAY{}", path_idx + 1)
                }
            };

            // 通过 DisplayConfigGetDeviceInfo(GET_TARGET_NAME) 直接获取显示器型号名
            // 这是最快的获取型号名称的方式，无需 EnumDisplayDevicesW / PowerShell
            let monitor_model = {
                let mut target_name: DISPLAYCONFIG_TARGET_DEVICE_NAME = mem::zeroed();
                target_name.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
                target_name.header.size = mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
                target_name.header.adapterId = target_info.adapterId;
                target_name.header.id = target_info.id;

                if DisplayConfigGetDeviceInfo(&mut target_name.header as *mut _ as *mut _) == 0 {
                    let len = target_name.monitorFriendlyDeviceName
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(target_name.monitorFriendlyDeviceName.len());
                    if len > 0 {
                        let name = String::from_utf16_lossy(&target_name.monitorFriendlyDeviceName[..len]);
                        let trimmed = name.trim();
                        if !trimmed.is_empty() {
                            trimmed.to_string()
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    }
                } else {
                    // 回退到 EnumDisplayDevicesW
                    get_monitor_model_name(&device_name)
                }
            };

            let name = if !monitor_model.is_empty() {
                format!("{} ({}x{})", monitor_model, width, height)
            } else {
                format!("{} ({}x{})", device_name.trim_start_matches("\\\\.\\"), width, height)
            };

            log::info!(
                "CCD 发现显示器[{}]: {} ({}x{}), device={}, primary={}",
                displays.len(), name, width, height, device_name, is_primary
            );

            displays.push(DisplayInfo {
                index: displays.len(),
                name,
                device_name,
                is_primary,
                width,
                height,
            });
        }

        if displays.is_empty() {
            log::warn!("CCD 枚举完成但未发现有效的显示器");
            return None;
        }

        Some(displays)
    }
}

/// 通过 EnumDisplaySettingsW(ENUM_CURRENT_SETTINGS) 获取设备当前分辨率
#[cfg(target_os = "windows")]
fn get_gdi_device_resolution(device_name: &str) -> (i32, i32) {
    use windows_sys::Win32::Graphics::Gdi::{EnumDisplaySettingsW, DEVMODEW, ENUM_CURRENT_SETTINGS};
    unsafe {
        let tries = [device_name, device_name.trim_start_matches("\\\\.\\")];
        for name in tries {
            if name.is_empty() { continue; }
            let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let mut dm: DEVMODEW = std::mem::zeroed();
            dm.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
            if EnumDisplaySettingsW(wide.as_ptr(), ENUM_CURRENT_SETTINGS, &mut dm) != 0 {
                let w = dm.dmPelsWidth as i32;
                let h = dm.dmPelsHeight as i32;
                if w > 0 && h > 0 { return (w, h); }
            }
        }
    }
    (0, 0)
}

/// 回退方案：使用 GDI EnumDisplayMonitors + GetMonitorInfoW
#[cfg(target_os = "windows")]
fn enumerate_displays_via_gdi() -> Vec<DisplayInfo> {
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW,
        HDC, HMONITOR, MONITORINFOEXW,
    };

    struct MonitorData {
        displays: Vec<DisplayInfo>,
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
            let mut width = info.monitorInfo.rcMonitor.right - info.monitorInfo.rcMonitor.left;
            let mut height = info.monitorInfo.rcMonitor.bottom - info.monitorInfo.rcMonitor.top;

            // 分辨率无效时回退到 EnumDisplaySettingsW
            if width <= 0 || height <= 0 {
                log::warn!(
                    "GDI: monitor[{}] GetMonitorInfoW 分辨率无效 ({}x{})，EnumDisplaySettingsW 回退",
                    data.displays.len(), width, height
                );
                let (fw, fh) = get_gdi_device_resolution(&device_name);
                if fw > 0 && fh > 0 { width = fw; height = fh; }
            }

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
        }
        1
    }

    let mut data = MonitorData {
        displays: Vec::new(),
    };

    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(monitor_enum_proc),
            &mut data as *mut _ as isize,
        );
    }

    data.displays
}

/// Sync internal: enumerate all displays and populate DISPLAY_DEVICES cache.
/// 优先使用 CCD (QueryDisplayConfig)，回退到 GDI (EnumDisplayMonitors)。
#[cfg(target_os = "windows")]
fn enumerate_displays_inner() -> Vec<DisplayInfo> {
    // ── 首选：CCD (QueryDisplayConfig) ──
    let mut displays = enumerate_displays_via_ccd().unwrap_or_default();

    // ── 回退：GDI (EnumDisplayMonitors) ──
    if displays.is_empty() {
        log::warn!("CCD 枚举失败，回退到 GDI EnumDisplayMonitors");
        displays = enumerate_displays_via_gdi();
    }

    // ── 如果仍然为空，尝试 EnumDisplayDevicesW + EnumDisplaySettingsW 暴力扫描 ──
    if displays.is_empty() {
        log::warn!("GDI 也失败，最后回退：暴力扫描 DISPLAY1..DISPLAY8");
        for i in 0..8 {
            let name = format!("\\\\.\\DISPLAY{}", i + 1);
            let (w, h) = get_gdi_device_resolution(&name);
            if w > 0 && h > 0 {
                let device_name = name;
                let is_primary = i == 0;
                let mon_name = format!("DISPLAY{} ({}x{})", i + 1, w, h);
                displays.push(DisplayInfo {
                    index: displays.len(),
                    name: mon_name,
                    device_name,
                    is_primary,
                    width: w,
                    height: h,
                });
            }
        }
    }

    // 缓存 device names
    if let Ok(mut lock) = DISPLAY_DEVICES.lock() {
        let names: Vec<String> = displays.iter().map(|d| d.device_name.clone()).collect();
        *lock = Some(names);
    }

    // 极端回退：仍然没有显示器
    if displays.is_empty() {
        log::error!("所有显示枚举方法都失败！返回 fallback 显示器");
        displays.push(DisplayInfo {
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

    // EDID 回退：EnumDisplayDevicesW 经常返回"通用即插即用显示器"，
    // 导致 name 退化为 GDI 设备名（如 \\.\DISPLAY1）。
    let is_fallback_name = |name: &str| -> bool {
        if name.starts_with('\\') { return true; }
        let prefix = name.split(" (").next().unwrap_or(name);
        is_generic_monitor_name(prefix)
    };
    if displays.iter().any(|d| is_fallback_name(&d.name)) {
        log::info!("检测到通用显示器名称，尝试从 EDID 获取真实型号...");
        let edid_names = crate::display_cache::get_edid_monitor_names();
        log::info!("EDID 查询结果: {} 个", edid_names.len());
        if !edid_names.is_empty() {
            for (i, d) in displays.iter_mut().enumerate() {
                if is_fallback_name(&d.name) {
                    if let Some(edid_name) = edid_names.get(i) {
                        if !edid_name.is_empty() {
                            let new_name = format!("{} ({}x{})", edid_name, d.width, d.height);
                            log::info!("显示器[{}]: EDID 替换 '{}' -> '{}'", i, d.name, new_name);
                            d.name = new_name;
                        }
                    } else if edid_names.len() == 1 && !edid_names[0].is_empty() {
                        let new_name = format!("{} ({}x{})", edid_names[0], d.width, d.height);
                        log::info!("显示器[{}]: EDID 替换(单结果) '{}' -> '{}'", i, d.name, new_name);
                        d.name = new_name;
                    }
                }
            }
        } else {
            log::warn!("EDID 查询无结果，保持原始名称");
        }
    }

    displays
}

#[tauri::command]
pub async fn get_displays() -> Result<Vec<DisplayInfo>, String> {
    #[cfg(target_os = "windows")]
    {
        log::info!("get_displays: 枚举所有显示器…");
        let displays = tauri::async_runtime::spawn_blocking(|| enumerate_displays_inner())
            .await
            .map_err(|e| format!("枚举显示器失败: {}", e))?;

        // 重试机制：如果所有显示器分辨率都是 0，等待后重试一次
        if !displays.is_empty() && displays.iter().all(|d| d.width <= 0 || d.height <= 0) {
            log::warn!("get_displays: 所有显示器分辨率无效，500ms 后重试…");
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let retry = tauri::async_runtime::spawn_blocking(|| enumerate_displays_inner())
                .await
                .map_err(|e| format!("枚举显示器重试失败: {}", e))?;
            if retry.iter().any(|d| d.width > 0 && d.height > 0) {
                log::info!("get_displays: 重试后成功获取到分辨率");
                return Ok(retry);
            }
            log::warn!("get_displays: 重试后分辨率仍未就绪，返回首次结果");
        }

        Ok(displays)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows 系统".to_string())
    }
}



/// Check if a monitor name is a generic/placeholder (any language variant)
fn is_generic_monitor_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("generic")
        || lower.contains("即插即用")
        || lower.contains("通用")
        || lower.contains("pnp")
        || lower.contains("standard monitor")
        || lower.contains("digital display")
        || lower.contains("analog display")
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
                if !trimmed.is_empty() && !is_generic_monitor_name(trimmed) {
                    return trimmed.to_string();
                }
                // Don't return generic names; return empty so callers use device_name
                return String::new();
            }
        }
    }
    
    String::new()
}
// ─── Per-display state ───

#[derive(Clone)]
struct DisplayState {
    original_gamma: Option<[[u16; 256]; 3]>,
    temperature: i32,
    brightness: i32,
    contrast: i32,
    saturation: i32,
    r_gamma: f64,
    g_gamma: f64,
    b_gamma: f64,
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
            r_gamma: 1.0,
            g_gamma: 1.0,
            b_gamma: 1.0,
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
    pub r_gamma: f64,
    pub g_gamma: f64,
    pub b_gamma: f64,
    pub mode: i32,
    pub is_active: bool,
}

#[derive(serde::Serialize)]
pub struct FilterResult {
    pub success: bool,
    pub message: String,
    pub settings: Option<FilterSettings>,
    pub preview_filter: Option<String>,
    pub preview_tint_color: Option<String>,
    pub preview_tint_opacity: Option<f64>,
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
    #[serde(default = "default_one_f64")]
    pub r_gamma: f64,
    #[serde(default = "default_one_f64")]
    pub g_gamma: f64,
    #[serde(default = "default_one_f64")]
    pub b_gamma: f64,
}

fn default_one_f64() -> f64 { 1.0 }

impl Default for CustomFilterSettings {
    fn default() -> Self {
        Self {
            temperature: 6500,
            brightness: 100,
            contrast: 100,
            saturation: 100,
            r_gamma: 1.0,
            g_gamma: 1.0,
            b_gamma: 1.0,
        }
    }
}

static CUSTOM_SETTINGS: Mutex<Option<HashMap<usize, CustomFilterSettings>>> = Mutex::new(None);

fn get_settings_file_path() -> PathBuf {
    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    config_dir.join("NexBox").join("filter-settings.json")
}

fn get_legacy_settings_file_path() -> PathBuf {
    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    config_dir.join("NexBox").join("settings.json")
}

fn load_custom_settings_from_file() -> HashMap<usize, CustomFilterSettings> {
    let path = get_settings_file_path();
    if path.exists() {
        return load_from_json_file(&path);
    }

    // Fallback: try legacy path (settings.json) for migration from older versions
    let legacy_path = get_legacy_settings_file_path();
    if legacy_path.exists() {
        if let Ok(content) = fs::read_to_string(&legacy_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(settings_value) = json.get("custom-filter-settings") {
                    let result = parse_custom_settings_value(settings_value.clone());
                    if !result.is_empty() {
                        // Migrate to new file
                        if let Some(parent) = path.parent() {
                            let _ = fs::create_dir_all(parent);
                        }
                        let string_map: HashMap<String, &CustomFilterSettings> = result
                            .iter()
                            .map(|(k, v)| (k.to_string(), v))
                            .collect();
                        if let Ok(json_str) = serde_json::to_string_pretty(&serde_json::json!({"custom-filter-settings": string_map})) {
                            let _ = fs::write(&path, json_str);
                        }
                        return result;
                    }
                }
            }
        }
    }

    HashMap::new()
}

fn load_from_json_file(path: &PathBuf) -> HashMap<usize, CustomFilterSettings> {
    match fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(json) => {
                if let Some(settings_value) = json.get("custom-filter-settings") {
                    let result = parse_custom_settings_value(settings_value.clone());
                    if !result.is_empty() {
                        return result;
                    }
                }
            }
            Err(e) => log::error!("解析滤镜设置文件JSON失败: {}", e),
        },
        Err(e) => log::error!("读取滤镜设置文件失败: {}", e),
    }
    HashMap::new()
}

fn parse_custom_settings_value(value: serde_json::Value) -> HashMap<usize, CustomFilterSettings> {
    if let Ok(map) = serde_json::from_value::<HashMap<String, CustomFilterSettings>>(value.clone()) {
        let result: HashMap<usize, CustomFilterSettings> = map
            .into_iter()
            .filter_map(|(k, v)| k.parse::<usize>().ok().map(|idx| (idx, v)))
            .collect();
        if !result.is_empty() {
            return result;
        }
    }
    // Fallback: old format (single CustomFilterSettings object)
    match serde_json::from_value::<CustomFilterSettings>(value) {
        Ok(settings) => {
            let mut map = HashMap::new();
            map.insert(0, settings);
            map
        }
        Err(e) => {
            log::error!("解析自定义滤镜设置失败: {}", e);
            HashMap::new()
        }
    }
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
    custom_gamma: Option<(f64, f64, f64)>,
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

    // Per-channel gamma: if custom_gamma is Some, use it as (r_gamma, g_gamma, b_gamma)
    let use_per_channel = custom_gamma.is_some()
        && (custom_gamma.unwrap().0 - 1.0).abs() > 0.001
        || (custom_gamma.unwrap_or((1.0, 1.0, 1.0)).1 - 1.0).abs() > 0.001
        || (custom_gamma.unwrap_or((1.0, 1.0, 1.0)).2 - 1.0).abs() > 0.001;

    let (r_gamma, g_gamma, b_gamma) = custom_gamma.unwrap_or((gamma, gamma, gamma));

    let mut ramp = [[0u16; 256]; 3];

    if use_per_channel {
        // Per-channel gamma pipeline: each channel gets independent gamma + s_curve
        for i in 0..256 {
            let input = i as f64 / 255.0;

            let r_adj = apply_gamma_curve(input, r_gamma);
            let g_adj = apply_gamma_curve(input, g_gamma);
            let b_adj = apply_gamma_curve(input, b_gamma);

            let r_adj = apply_s_curve(r_adj, s_curve_strength);
            let g_adj = apply_s_curve(g_adj, s_curve_strength);
            let b_adj = apply_s_curve(b_adj, s_curve_strength);

            let r_adj = ((r_adj - 0.5) * contrast_factor + 0.5) * brightness_factor;
            let g_adj = ((g_adj - 0.5) * contrast_factor + 0.5) * brightness_factor;
            let b_adj = ((b_adj - 0.5) * contrast_factor + 0.5) * brightness_factor;

            let r_base = r_adj.clamp(0.0, 1.0) * 65535.0;
            let g_base = g_adj.clamp(0.0, 1.0) * 65535.0;
            let b_base = b_adj.clamp(0.0, 1.0) * 65535.0;

            let r_final = (r_base * r_temp_mult * r_boost).min(65535.0);
            let g_final = (g_base * g_temp_mult * g_boost).min(65535.0);
            let b_final = (b_base * b_temp_mult * b_boost).min(65535.0);

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
    } else {
        // Original unified gamma pipeline (unchanged for preset modes)
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

/// 轻量级枚举：仅收集显示器 GDI 设备名并填充 DISPLAY_DEVICES 缓存。
/// 不调用 EnumDisplayDevicesW / EDID / PowerShell，适用于清理路径等不应阻塞的场景。
#[cfg(target_os = "windows")]
fn enumerate_device_names_only() -> Vec<String> {
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW,
        HDC, HMONITOR, MONITORINFOEXW,
    };

    struct MonitorData {
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
            data.device_names.push(device_name);
        }
        1
    }

    let mut data = MonitorData {
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
    if data.device_names.is_empty() {
        data.device_names.push("DISPLAY1".to_string());
    }
    if let Ok(mut lock) = DISPLAY_DEVICES.lock() {
        *lock = Some(data.device_names.clone());
    }
    data.device_names
}

/// 获取当前显示器设备名列表（不更新缓存）
/// 用于检测显示配置是否变化（独显直连切换等场景）
#[cfg(target_os = "windows")]
fn get_current_display_names() -> Vec<String> {
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW,
        HDC, HMONITOR, MONITORINFOEXW,
    };

    struct MonitorData {
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
            data.device_names.push(device_name);
        }
        1
    }

    let mut data = MonitorData {
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
    data.device_names
}

/// 检测显示配置是否发生变化（设备名或数量变化）
/// 用于独显直连/Advanced Optimus 切换后自动刷新缓存
#[cfg(target_os = "windows")]
fn check_display_config_changed() -> bool {
    let current_names = get_current_display_names();
    let cached_names = {
        let lock = DISPLAY_DEVICES.lock().unwrap();
        lock.as_ref().cloned().unwrap_or_default()
    };

    if current_names.is_empty() && cached_names.is_empty() {
        return false;
    }

    if current_names.len() != cached_names.len() {
        log::info!(
            "显示配置变化：显示器数量 {} -> {}",
            cached_names.len(),
            current_names.len()
        );
        return true;
    }

    for (a, b) in current_names.iter().zip(cached_names.iter()) {
        if a != b {
            log::info!("显示配置变化：设备名 '{}' -> '{}'", b, a);
            return true;
        }
    }

    false
}

/// 刷新显示器缓存（保留滤镜参数和 filter_active 状态）
/// 用于独显直连/Advanced Optimus 切换后恢复滤镜功能
#[cfg(target_os = "windows")]
fn refresh_display_caches() {
    log::info!("刷新显示器缓存（独显直连切换检测）");

    // 1. 保存当前的滤镜参数和 filter_active 状态
    let saved_states: Vec<DisplayState> = {
        let lock = DISPLAY_STATES.lock().unwrap();
        if let Some(ref states) = *lock {
            states
                .iter()
                .map(|s| {
                    let state = s.lock().unwrap();
                    state.clone()
                })
                .collect()
        } else {
            Vec::new()
        }
    };

    // 2. 清空缓存
    {
        let mut lock = DISPLAY_DEVICES.lock().unwrap();
        *lock = None;
    }
    {
        let mut lock = DISPLAY_STATES.lock().unwrap();
        *lock = None;
    }

    // 3. 重新枚举显示器（更新 DISPLAY_DEVICES 缓存）
    let _ = enumerate_displays_inner();

    // 4. 重新初始化 DISPLAY_STATES
    ensure_display_states();

    // 5. 恢复保存的滤镜参数（按索引映射）
    let new_count = {
        let lock = DISPLAY_STATES.lock().unwrap();
        lock.as_ref().map(|s| s.len()).unwrap_or(0)
    };

    for i in 0..new_count {
        if let Some(ref saved) = saved_states.get(i) {
            with_display_state(i, |state| {
                state.temperature = saved.temperature;
                state.brightness = saved.brightness;
                state.contrast = saved.contrast;
                state.saturation = saved.saturation;
                state.r_gamma = saved.r_gamma;
                state.g_gamma = saved.g_gamma;
                state.b_gamma = saved.b_gamma;
                state.mode = saved.mode;
                state.filter_active = saved.filter_active;
                state.icc_active = saved.icc_active;
                state.icc_ramp = saved.icc_ramp.clone();
                // original_gamma 需要重新获取，因为 GPU 切换后旧的 gamma ramp 已失效
                state.original_gamma = None;
            });

            // 如果滤镜是活跃的，重新获取 original_gamma
            if saved.filter_active {
                with_display_state(i, |state| {
                    if state.original_gamma.is_none() {
                        if let Ok(ramp) = get_current_gamma_ramp_for_display(i) {
                            state.original_gamma = Some(ramp);
                        }
                    }
                });
            }
        }
    }

    log::info!(
        "显示器缓存刷新完成，恢复 {} 个显示器的滤镜状态",
        new_count
    );
}

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
            // 缓存为空 → 轻量枚举（不触发 EnumDisplayDevicesW / EDID / PowerShell）
            drop(lock);
            log::info!("get_display_dc[{}]: DISPLAY_DEVICES 缓存为空，轻量枚举设备名", display_index);
            enumerate_device_names_only()
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

    // CreateDCW 参数说明（MSDN）：
    //   lpszDriver = "DISPLAY"（显示驱动名，固定值）
    //   lpszDevice = 设备名（如 "\\.\\DISPLAY1"，来自 EnumDisplayMonitors）
    //
    // 旧代码把设备名传给 lpszDriver 而 lpszDevice=NULL，在普通模式下 Windows
    // 能容忍，但在独显直连（dGPU 直连显示器、核显禁用）模式下会创建不完整的
    // DC，导致 SetDeviceGammaRamp 返回失败。
    let display_driver_wide: Vec<u16> = "DISPLAY\0".encode_utf16().collect();

    // Attempt per-display CreateDCW with each name format
    for fmt in &name_formats {
        let device_name_wide: Vec<u16> = fmt.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            // 正确格式：CreateDCW("DISPLAY", "\\.\\DISPLAY1", NULL, NULL)
            let hdc = CreateDCW(
                display_driver_wide.as_ptr(),
                device_name_wide.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
            );
            if !hdc.is_null() {
                log::info!("get_display_dc[{}]: CreateDCW 成功 (driver=DISPLAY, device={})", display_index, fmt);
                return Ok((hdc, false));
            }
            // 回退：旧格式（向后兼容，某些旧驱动可能需要）
            let hdc = CreateDCW(
                device_name_wide.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
            );
            if !hdc.is_null() {
                log::info!("get_display_dc[{}]: CreateDCW 成功 (legacy, name={})", display_index, fmt);
                return Ok((hdc, false));
            }
        }
    }

    // ── Fallback 1: Try "DISPLAY1", "DISPLAY2" as device names ──
    // On some Optimus laptops, the device name from EnumDisplayMonitors may not
    // work directly with CreateDCW, but the plain "DISPLAY1" form does.
    if name_formats.is_empty() || name_formats.iter().all(|f| f.starts_with("\\\\.\\")) {
        let alt_name = format!("DISPLAY{}", display_index + 1);
        let alt_name_wide: Vec<u16> = alt_name.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            // 正确格式
            let hdc = CreateDCW(
                display_driver_wide.as_ptr(),
                alt_name_wide.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
            );
            if !hdc.is_null() {
                log::info!("get_display_dc[{}]: CreateDCW 成功 (driver=DISPLAY, device={})", display_index, alt_name);
                return Ok((hdc, false));
            }
            // 旧格式回退
            let hdc = CreateDCW(
                alt_name_wide.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
            );
            if !hdc.is_null() {
                log::info!("get_display_dc[{}]: CreateDCW 成功 (legacy alt, name={})", display_index, alt_name);
                return Ok((hdc, false));
            }
        }
    }

    // ── Fallback 2: Desktop DC via GetDC(null) ──
    // Works on virtually all systems including Optimus laptops.
    // 注意：不再使用 CreateDCW("DISPLAY")，因为在独显直连/混合 GPU 场景下
    // 它可能返回错误 GPU（iGPU）的 DC，导致 SetDeviceGammaRamp 设置到
    // 不驱动显示器的 GPU 上，滤镜无效果。GetDC(null) 始终返回主显示器 GPU 的 DC。
    log::warn!(
        "get_display_dc[{}]: 逐显示器 CreateDCW 全部失败，使用 GetDC(null) 桌面 DC 回退",
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
    use windows_sys::Win32::Graphics::Gdi::{DeleteDC, ReleaseDC, GetDeviceCaps};
    use windows_sys::Win32::UI::ColorSystem::SetDeviceGammaRamp;

    // COLORMGMTCAPS = 1219, COLORMGMTCAP_GAMMA_RAMP = 1
    const COLORMGMTCAPS: i32 = 1219;

    let _guard = GAMMA_RAMP_MUTEX
        .lock()
        .map_err(|_| "无法获取 Gamma Ramp 锁".to_string())?;

    // Strategy 1: Try per-display DC
    let (hdc, use_release) = get_display_dc(display_index)?;

    unsafe {
        let result = SetDeviceGammaRamp(hdc, ramp.as_ptr() as *const _);
        let dc_is_per_display = !use_release;

        if result == 0 {
            // SetDeviceGammaRamp 失败，检查 DC 是否支持 gamma ramp
            let gamma_caps = GetDeviceCaps(hdc, COLORMGMTCAPS);
            log::warn!(
                "set_gamma_ramp_for_display[{}]: SetDeviceGammaRamp 失败, GetDeviceCaps(COLORMGMTCAPS)={}, gamma_ramp_supported={}",
                display_index, gamma_caps, gamma_caps & 1 != 0
            );
        }

        if use_release {
            ReleaseDC(std::ptr::null_mut(), hdc);
        } else {
            DeleteDC(hdc);
        }

        if result != 0 {
            // SetDeviceGammaRamp 返回成功，直接信任结果
            // 注意：不通过 GetDeviceGammaRamp 验证，因为 NVIDIA 驱动的
            // GetDeviceGammaRamp 返回值可能与设置值不一致（内部缓存/量化），
            // 误判会导致进入 Strategy2 桌面 DC 回退，而桌面 DC 的
            // SetDeviceGammaRamp 可能因驱动限制而失败，最终报"显卡不支持"。
            log::info!(
                "set_gamma_ramp_for_display[{}]: Strategy1 SetDeviceGammaRamp 成功 (dc_is_per_display={})",
                display_index, dc_is_per_display
            );
            return Ok(());
        }

        // SetDeviceGammaRamp 失败
        // If per-display DC failed but we used desktop DC, no more retries
        if !dc_is_per_display {
            log::error!(
                "set_gamma_ramp_for_display[{}]: SetDeviceGammaRamp 失败(桌面DC回退，跳过Strategy2)! display_index={}",
                "可能是显卡驱动不支持",
                display_index
            );
            return Err("您的设置太逆天啦，你的显卡可能不支持喔~".to_string());
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
            return Err("您的设置太逆天啦，你的显卡可能不支持喔~".to_string());
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
    let (icc_active, temperature, brightness, contrast, saturation, r_gamma, g_gamma, b_gamma, mode, icc_ramp_opt) =
        with_display_state(display_index, |state| {
            (
                state.icc_active,
                state.temperature,
                state.brightness,
                state.contrast,
                state.saturation,
                state.r_gamma,
                state.g_gamma,
                state.b_gamma,
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
    let custom_gamma = Some((r_gamma, g_gamma, b_gamma));
    let ramp = build_gamma_ramp(temperature, brightness, contrast, saturation, mode_enum, custom_gamma);
    log::info!("Monitor[{}]: applying regular filter ramp", display_index);
    set_gamma_ramp_for_display(display_index, &ramp)
}

fn restore_original_gamma_for_display(display_index: usize) -> Result<(), String> {
    let original =
        with_display_state(display_index, |state| state.original_gamma);

    if let Some(ref ramp) = original {
        set_gamma_ramp_for_display(display_index, ramp)?;
    } else {
        let identity_ramp = build_gamma_ramp(6500, 100, 100, 100, FilterMode::Normal, None);
        set_gamma_ramp_for_display(display_index, &identity_ramp)?;
    }

    Ok(())
}

fn start_filter_monitor() {
    if FILTER_THREAD_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    thread::spawn(|| {
        // Initialize a Windows message queue for this thread.
        // SetDeviceGammaRamp / GDI display operations require the calling
        // thread to have a message pump, otherwise they fail silently on
        // modern GPU drivers (especially Optimus / switchable graphics).
        #[cfg(target_os = "windows")]
        {
            use windows_sys::Win32::UI::WindowsAndMessaging::PeekMessageW;
            unsafe {
                let mut msg = std::mem::zeroed();
                PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, 0); // PM_NOREMOVE = 0
            }
        }

        loop {
            // 检测显示配置变化（独显直连/Advanced Optimus 切换等）
            // 切换后显示器可能从 iGPU 切到 dGPU，缓存的设备名和状态需要刷新
            if check_display_config_changed() {
                log::info!("检测到显示配置变化，刷新缓存并重新应用滤镜");
                refresh_display_caches();
            }

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
        r_gamma: state.r_gamma,
        g_gamma: state.g_gamma,
        b_gamma: state.b_gamma,
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
    is_active: bool,
    r_gamma: Option<f64>,
    g_gamma: Option<f64>,
    b_gamma: Option<f64>,
) -> Result<FilterResult, String> {
    #[cfg(target_os = "windows")]
    {
        let idx = resolve_display_index(display_index);
        let temperature = temperature.clamp(1000, 10000);
        let brightness = brightness.clamp(50, 150);
        let contrast = contrast.clamp(50, 150);
        let saturation = saturation.clamp(50, 150);
        let mode = mode.clamp(0, 8);
        let r_gamma = r_gamma.unwrap_or(1.0).clamp(0.50, 2.00);
        let g_gamma = g_gamma.unwrap_or(1.0).clamp(0.50, 2.00);
        let b_gamma = b_gamma.unwrap_or(1.0).clamp(0.50, 2.00);

        with_display_state(idx, |state| {
            state.temperature = temperature;
            state.brightness = brightness;
            state.contrast = contrast;
            state.saturation = saturation;
            state.r_gamma = r_gamma;
            state.g_gamma = g_gamma;
            state.b_gamma = b_gamma;
            state.mode = mode;
            state.icc_active = false;

            // Only activate if is_active is true and filter is not already active
            if is_active && !state.filter_active {
                if state.original_gamma.is_none() {
                    if let Ok(ramp) = get_current_gamma_ramp_for_display(idx) {
                        state.original_gamma = Some(ramp);
                    }
                }
                state.filter_active = true;
            }
        });

        // Only apply filter to hardware if it's actually active
        let actually_active = with_display_state(idx, |s| s.filter_active);
        if actually_active {
            apply_filter_internal_for_display(idx)?;
            start_filter_monitor();
        }

        Ok(FilterResult {
            success: true,
            message: "滤镜设置已更新".to_string(),
            settings: Some(FilterSettings {
                temperature,
                brightness,
                contrast,
                saturation,
                r_gamma,
                g_gamma,
                b_gamma,
                mode,
                is_active: actually_active,
            }),
            preview_filter: None,
            preview_tint_color: None,
            preview_tint_opacity: None,
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
                    r_gamma: state.r_gamma,
                    g_gamma: state.g_gamma,
                    b_gamma: state.b_gamma,
                    mode: state.mode,
                    is_active: true,
                }),
                preview_filter: None,
                preview_tint_color: None,
                preview_tint_opacity: None,
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
                r_gamma: state.r_gamma,
                g_gamma: state.g_gamma,
                b_gamma: state.b_gamma,
                mode: state.mode,
                is_active: true,
            }),
            preview_filter: None,
            preview_tint_color: None,
            preview_tint_opacity: None,
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
                    r_gamma: state.r_gamma,
                    g_gamma: state.g_gamma,
                    b_gamma: state.b_gamma,
                    mode: state.mode,
                    is_active: false,
                }),
                preview_filter: None,
                preview_tint_color: None,
                preview_tint_opacity: None,
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
                r_gamma: state.r_gamma,
                g_gamma: state.g_gamma,
                b_gamma: state.b_gamma,
                mode: state.mode,
                is_active: false,
            }),
            preview_filter: None,
            preview_tint_color: None,
            preview_tint_opacity: None,
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
                r_gamma: state.r_gamma,
                g_gamma: state.g_gamma,
                b_gamma: state.b_gamma,
                mode: state.mode,
                is_active: true,
            }),
            preview_filter: None,
            preview_tint_color: None,
            preview_tint_opacity: None,
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
            r_gamma: state.r_gamma,
            g_gamma: state.g_gamma,
            b_gamma: state.b_gamma,
            mode: state.mode,
            is_active: true,
        }),
        preview_filter: None,
        preview_tint_color: None,
        preview_tint_opacity: None,
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
                r_gamma: state.r_gamma,
                g_gamma: state.g_gamma,
                b_gamma: state.b_gamma,
                mode: state.mode,
                is_active: false,
            }),
            preview_filter: None,
            preview_tint_color: None,
            preview_tint_opacity: None,
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
            r_gamma: state.r_gamma,
            g_gamma: state.g_gamma,
            b_gamma: state.b_gamma,
            mode: state.mode,
            is_active: false,
        }),
        preview_filter: None,
        preview_tint_color: None,
        preview_tint_opacity: None,
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
    is_active: bool,
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
            is_active,
            None,
            None,
            None,
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
    r_gamma: Option<f64>,
    g_gamma: Option<f64>,
    b_gamma: Option<f64>,
) -> Result<CustomFilterSettings, String> {
    let idx = resolve_display_index(display_index);
    let settings = CustomFilterSettings {
        temperature: temperature.clamp(1000, 10000),
        brightness: brightness.clamp(50, 150),
        contrast: contrast.clamp(50, 150),
        saturation: saturation.clamp(50, 150),
        r_gamma: r_gamma.unwrap_or(1.0).clamp(0.50, 2.00),
        g_gamma: g_gamma.unwrap_or(1.0).clamp(0.50, 2.00),
        b_gamma: b_gamma.unwrap_or(1.0).clamp(0.50, 2.00),
    };

    let mut all_settings = get_or_load_custom_settings();
    all_settings.insert(idx, settings.clone());

    save_custom_settings_to_file(&all_settings)?;

    let mut settings_lock = CUSTOM_SETTINGS.lock().unwrap();
    *settings_lock = Some(all_settings);

    Ok(settings)
}

#[tauri::command]
pub async fn export_custom_filter(display_index: Option<usize>) -> Result<Option<String>, String> {
    #[cfg(target_os = "windows")]
    {
        let idx = resolve_display_index(display_index);
        let all_settings = get_or_load_custom_settings();
        let settings = all_settings.get(&idx).cloned().unwrap_or_default();

        // Build gamma ramp from custom settings (mode = Normal)
        let ramp = build_gamma_ramp(
            settings.temperature,
            settings.brightness,
            settings.contrast,
            settings.saturation,
            FilterMode::Normal,
            Some((settings.r_gamma, settings.g_gamma, settings.b_gamma)),
        );

        let default_name = "NexBox_Custom.icc";
        let result = rfd::FileDialog::new()
            .set_title("导出自定义滤镜为 ICC")
            .add_filter("ICC 文件", &["icc", "icm"])
            .set_file_name(default_name)
            .save_file();

        let path = match result {
            Some(p) => p,
            None => return Ok(None),
        };

        let icc_data = build_icc_profile(&ramp, "NexBox Custom Filter");
        fs::write(&path, &icc_data).map_err(|e| format!("无法保存文件: {}", e))?;

        log::info!("Custom ICC exported: {} ({} bytes)", path.display(), icc_data.len());
        Ok(Some(path.to_string_lossy().to_string()))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows 系统".to_string())
    }
}

// ─── User Filter Presets (named, shareable) ───

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct UserFilterPreset {
    pub id: String,
    pub name: String,
    pub temperature: i32,
    pub brightness: i32,
    pub contrast: i32,
    pub saturation: i32,
    #[serde(default = "default_one_f64")]
    pub r_gamma: f64,
    #[serde(default = "default_one_f64")]
    pub g_gamma: f64,
    #[serde(default = "default_one_f64")]
    pub b_gamma: f64,
}

#[derive(serde::Serialize, Clone)]
pub struct UserFilterPresetInfo {
    pub id: String,
    pub name: String,
    pub temperature: i32,
    pub brightness: i32,
    pub contrast: i32,
    pub saturation: i32,
    pub r_gamma: f64,
    pub g_gamma: f64,
    pub b_gamma: f64,
}

static USER_FILTER_PRESETS: Mutex<Option<Vec<UserFilterPreset>>> = Mutex::new(None);

fn get_user_filter_presets_file_path() -> PathBuf {
    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    config_dir.join("NexBox").join("user-filter-presets.json")
}

fn load_user_filter_presets_from_file() -> Vec<UserFilterPreset> {
    let path = get_user_filter_presets_file_path();
    if path.exists() {
        match fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<Vec<UserFilterPreset>>(&content) {
                Ok(presets) => return presets,
                Err(e) => log::error!("解析用户滤镜预设文件失败: {}", e),
            },
            Err(e) => log::error!("读取用户滤镜预设文件失败: {}", e),
        }
    }
    Vec::new()
}

fn save_user_filter_presets_to_file(presets: &[UserFilterPreset]) -> Result<(), String> {
    let path = get_user_filter_presets_file_path();
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

fn get_or_load_user_filter_presets() -> Vec<UserFilterPreset> {
    let mut lock = USER_FILTER_PRESETS.lock().unwrap();
    if lock.is_none() {
        let presets = load_user_filter_presets_from_file();
        *lock = Some(presets.clone());
        presets
    } else {
        lock.as_ref().unwrap().clone()
    }
}

#[tauri::command]
pub async fn get_user_filter_presets() -> Result<Vec<UserFilterPresetInfo>, String> {
    let presets = get_or_load_user_filter_presets();
    Ok(presets
        .iter()
        .map(|p| UserFilterPresetInfo {
            id: p.id.clone(),
            name: p.name.clone(),
            temperature: p.temperature,
            brightness: p.brightness,
            contrast: p.contrast,
            saturation: p.saturation,
            r_gamma: p.r_gamma,
            g_gamma: p.g_gamma,
            b_gamma: p.b_gamma,
        })
        .collect())
}

#[tauri::command]
pub async fn save_user_filter_preset(
    id: Option<String>,
    name: String,
    temperature: i32,
    brightness: i32,
    contrast: i32,
    saturation: i32,
    r_gamma: Option<f64>,
    g_gamma: Option<f64>,
    b_gamma: Option<f64>,
) -> Result<UserFilterPresetInfo, String> {
    let new_preset = UserFilterPreset {
        id: id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        name,
        temperature: temperature.clamp(1000, 10000),
        brightness: brightness.clamp(50, 150),
        contrast: contrast.clamp(50, 150),
        saturation: saturation.clamp(50, 150),
        r_gamma: r_gamma.unwrap_or(1.0).clamp(0.50, 2.00),
        g_gamma: g_gamma.unwrap_or(1.0).clamp(0.50, 2.00),
        b_gamma: b_gamma.unwrap_or(1.0).clamp(0.50, 2.00),
    };

    let mut lock = USER_FILTER_PRESETS.lock().unwrap();
    let mut presets = if lock.is_some() {
        lock.take().unwrap()
    } else {
        load_user_filter_presets_from_file()
    };

    if id.is_some() {
        // Update existing
        if let Some(existing) = presets.iter_mut().find(|p| p.id == new_preset.id) {
            *existing = new_preset.clone();
        } else {
            presets.push(new_preset.clone());
        }
    } else {
        presets.push(new_preset.clone());
    }

    save_user_filter_presets_to_file(&presets)?;
    *lock = Some(presets);

    Ok(UserFilterPresetInfo {
        id: new_preset.id,
        name: new_preset.name,
        temperature: new_preset.temperature,
        brightness: new_preset.brightness,
        contrast: new_preset.contrast,
        saturation: new_preset.saturation,
        r_gamma: new_preset.r_gamma,
        g_gamma: new_preset.g_gamma,
        b_gamma: new_preset.b_gamma,
    })
}

#[tauri::command]
pub async fn apply_user_filter_preset(
    display_index: Option<usize>,
    id: String,
    is_active: bool,
) -> Result<FilterResult, String> {
    let presets = get_or_load_user_filter_presets();
    let preset = presets
        .iter()
        .find(|p| p.id == id)
        .ok_or("未找到自定义滤镜预设".to_string())?;

    // Forward to set_filter_settings with gamma values
    set_filter_settings(
        display_index,
        preset.temperature,
        preset.brightness,
        preset.contrast,
        preset.saturation,
        0, // mode = Normal (custom)
        is_active,
        Some(preset.r_gamma),
        Some(preset.g_gamma),
        Some(preset.b_gamma),
    )
    .await
}

#[tauri::command]
pub async fn delete_user_filter_preset(id: String) -> Result<(), String> {
    let mut lock = USER_FILTER_PRESETS.lock().unwrap();
    let mut presets = if lock.is_some() {
        lock.take().unwrap()
    } else {
        load_user_filter_presets_from_file()
    };

    let len_before = presets.len();
    presets.retain(|p| p.id != id);

    if presets.len() == len_before {
        *lock = Some(presets);
        return Err("未找到要删除的自定义滤镜预设".to_string());
    }

    save_user_filter_presets_to_file(&presets)?;
    *lock = Some(presets);

    Ok(())
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

/// 从 ICC ramp 计算近似 CSS filter 预览参数
fn compute_icc_preview(ramp: &[[u16; 256]; 3]) -> (String, Option<String>, Option<f64>) {
    let mut ch_brightness = [1.0f64; 3];
    
    // 计算每个通道在中间调（32..224）的平均乘数
    for c in 0..3 {
        let mut sum = 0.0;
        let mut count = 0u32;
        for i in 32..224 {
            let identity = (i as u32 * 256) as u16;
            if identity > 0 {
                sum += ramp[c][i as usize] as f64 / identity as f64;
                count += 1;
            }
        }
        if count > 0 {
            ch_brightness[c] = sum / count as f64;
        }
    }
    
    let avg_brightness = (ch_brightness[0] + ch_brightness[1] + ch_brightness[2]) / 3.0;
    
    // 如果所有通道接近 1.0，返回空（无显著变化）
    if (avg_brightness - 1.0).abs() < 0.015
        && (ch_brightness[0] - ch_brightness[1]).abs() < 0.015
        && (ch_brightness[1] - ch_brightness[2]).abs() < 0.015
    {
        return (String::new(), None, None);
    }
    
    let mut filters: Vec<String> = Vec::new();
    
    // Brightness
    if (avg_brightness - 1.0).abs() > 0.01 {
        filters.push(format!("brightness({:.3})", avg_brightness.clamp(0.3, 2.5)));
    }
    
    let filter_str = filters.join(" ");
    
    // 检测颜色偏移（通道间差异）
    let drift_r = ch_brightness[0] - avg_brightness;
    let drift_g = ch_brightness[1] - avg_brightness;
    let drift_b = ch_brightness[2] - avg_brightness;
    let max_drift = drift_r.abs().max(drift_g.abs()).max(drift_b.abs());
    
    if max_drift > 0.02 {
        // 用颜色覆盖层近似通道偏移
        let r = ((0.5 + drift_r * 3.0).clamp(0.0, 1.0) * 255.0) as u8;
        let g = ((0.5 + drift_g * 3.0).clamp(0.0, 1.0) * 255.0) as u8;
        let b = ((0.5 + drift_b * 3.0).clamp(0.0, 1.0) * 255.0) as u8;
        let opacity = (max_drift * 1.5).min(0.4);
        (filter_str, Some(format!("#{:02X}{:02X}{:02X}", r, g, b)), Some(opacity))
    } else {
        (filter_str, None, None)
    }
}

#[tauri::command]
pub async fn apply_icc_preset(
    display_index: Option<usize>,
    id: String,
    is_active: bool,
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
            // 始终标记 ICC 活跃，无论 is_active 是否开启
            // 这样用户后续手动开启滤镜开关时，enable_filter 会读取 icc_active 并应用 ICC ramp
            state.icc_active = true;

            if is_active && !state.filter_active {
                if state.original_gamma.is_none() {
                    if let Ok(ramp) = get_current_gamma_ramp_for_display(idx) {
                        state.original_gamma = Some(ramp);
                    }
                }
                state.filter_active = true;
            }
        });

        let actually_active = with_display_state(idx, |s| s.filter_active);
        if actually_active {
            set_gamma_ramp_for_display(idx, &ramp_array)?;
            start_filter_monitor();
        }

        let (preview_filter, preview_tint_color, preview_tint_opacity) = compute_icc_preview(&ramp_array);

        Ok(with_display_state(idx, |state| FilterResult {
            success: true,
            message: format!("ICC 预设 {} 已应用", preset.name),
            settings: Some(FilterSettings {
                temperature: state.temperature,
                brightness: state.brightness,
                contrast: state.contrast,
                saturation: state.saturation,
                r_gamma: state.r_gamma,
                g_gamma: state.g_gamma,
                b_gamma: state.b_gamma,
                mode: state.mode,
                is_active: state.filter_active,
            }),
            preview_filter: if preview_filter.is_empty() { None } else { Some(preview_filter) },
            preview_tint_color,
            preview_tint_opacity,
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
            preview_filter: None,
            preview_tint_color: None,
            preview_tint_opacity: None,
        })
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows 系统".to_string())
    }
}

// ─── ICC Profile Export ───

/// Write a big-endian u32 into a byte vector.
fn push_u32_be(buf: &mut Vec<u8>, val: u32) {
    buf.extend_from_slice(&val.to_be_bytes());
}

/// Write a big-endian u16 into a byte vector.
fn push_u16_be(buf: &mut Vec<u8>, val: u16) {
    buf.extend_from_slice(&val.to_be_bytes());
}

/// Write an s15Fixed16Number (signed 16.16 fixed-point) into a byte vector.
fn push_s15fixed16(buf: &mut Vec<u8>, val: f64) {
    let raw = (val * 65536.0).round() as i32;
    buf.extend_from_slice(&raw.to_be_bytes());
}

/// Pad a byte vector to a 4-byte boundary with zero bytes.
fn pad_to_4(buf: &mut Vec<u8>) {
    while buf.len() % 4 != 0 {
        buf.push(0);
    }
}

/// Build a minimal valid ICC v2 display profile containing a vcgt
/// (Video Card Gamma Table) tag with the supplied 3×256 gamma ramp.
///
/// The profile also includes the required v2 tags (desc, cprt, wtpt,
/// rXYZ/gXYZ/bXYZ, rTRC/gTRC/bTRC) so it is accepted by Windows Color
/// Management and other ICC-aware tools.
fn build_icc_profile(ramp: &[[u16; 256]; 3], description: &str) -> Vec<u8> {
    // Collect tag data blocks as (tag_signature, data_bytes).
    // Each block's data already starts with its type signature + reserved.
    let mut blocks: Vec<([u8; 4], Vec<u8>)> = Vec::new();

    // ── desc (profileDescriptionTag) ──
    {
        let mut d = Vec::new();
        d.extend_from_slice(b"desc");
        d.extend_from_slice(&[0u8; 4]); // reserved
        let desc_bytes = description.as_bytes();
        push_u32_be(&mut d, desc_bytes.len() as u32 + 1); // length incl. null
        d.extend_from_slice(desc_bytes);
        d.push(0); // null terminator
        pad_to_4(&mut d);
        // Unicode section (empty)
        push_u32_be(&mut d, 0); // language code
        push_u32_be(&mut d, 0); // count
        // ScriptCode section
        push_u16_be(&mut d, 2); // code
        d.push(0); // string length
        d.extend_from_slice(&[0u8; 67]); // string (67 bytes fixed)
        blocks.push((*b"desc", d));
    }

    // ── cprt (copyrightTag) ──
    {
        let mut d = Vec::new();
        d.extend_from_slice(b"text");
        d.extend_from_slice(&[0u8; 4]);
        let cprt = b"NexBox Exported ICC Profile\0";
        d.extend_from_slice(cprt);
        pad_to_4(&mut d);
        blocks.push((*b"cprt", d));
    }

    // ── wtpt (mediaWhitePointTag) – D50 ──
    {
        let mut d = Vec::new();
        d.extend_from_slice(b"XYZ ");
        d.extend_from_slice(&[0u8; 4]);
        push_s15fixed16(&mut d, 0.9505); // X
        push_s15fixed16(&mut d, 1.0000); // Y
        push_s15fixed16(&mut d, 1.0890); // Z
        blocks.push((*b"wtpt", d));
    }

    // ── rXYZ / gXYZ / bXYZ – sRGB primaries (D50-adapted) ──
    {
        let colorants: [([u8; 4], f64, f64, f64); 3] = [
            (*b"rXYZ", 0.4360, 0.2225, 0.0139),
            (*b"gXYZ", 0.3851, 0.7169, 0.0971),
            (*b"bXYZ", 0.1431, 0.0606, 0.7141),
        ];
        for (sig, x, y, z) in colorants {
            let mut d = Vec::new();
            d.extend_from_slice(b"XYZ ");
            d.extend_from_slice(&[0u8; 4]);
            push_s15fixed16(&mut d, x);
            push_s15fixed16(&mut d, y);
            push_s15fixed16(&mut d, z);
            blocks.push((sig, d));
        }
    }

    // ── rTRC / gTRC / bTRC – identity curves (curv count=0) ──
    // All three share the same data block.
    {
        let mut d = Vec::new();
        d.extend_from_slice(b"curv");
        d.extend_from_slice(&[0u8; 4]);
        push_u32_be(&mut d, 0); // count = 0 → identity
        blocks.push((*b"rTRC", d.clone()));
        blocks.push((*b"gTRC", d.clone()));
        blocks.push((*b"bTRC", d));
    }

    // ── vcgt (Video Card Gamma Table) ──
    {
        let mut d = Vec::new();
        d.extend_from_slice(b"vcgt"); // type signature
        d.extend_from_slice(&[0u8; 4]); // reserved
        push_u32_be(&mut d, 0); // formula type = 0 (table)
        push_u16_be(&mut d, 3); // channels
        push_u16_be(&mut d, 256); // entries per channel
        push_u16_be(&mut d, 2); // entry size in bytes (16-bit)
        // Planar data: all R values, then all G, then all B.
        for ch in 0..3 {
            for i in 0..256 {
                push_u16_be(&mut d, ramp[ch][i]);
            }
        }
        blocks.push((*b"vcgt", d));
    }

    // ── Assemble header + tag table + data ──
    let num_tags = blocks.len();
    let header_size: usize = 128;
    let tag_table_size: usize = 4 + num_tags * 12;
    let data_start = header_size + tag_table_size;

    // Build concatenated tag data and record per-tag offsets/sizes.
    let mut all_data: Vec<u8> = Vec::new();
    let mut entries: Vec<([u8; 4], usize, usize)> = Vec::new();

    for (sig, data) in &blocks {
        let offset = data_start + all_data.len();
        let size = data.len();
        entries.push((*sig, offset, size));
        all_data.extend_from_slice(data);
        pad_to_4(&mut all_data);
    }

    let profile_size = data_start + all_data.len();

    // Header (128 bytes)
    let mut profile = Vec::with_capacity(profile_size);
    push_u32_be(&mut profile, profile_size as u32); // 0  profile size
    push_u32_be(&mut profile, 0); // 4  CMM type
    push_u32_be(&mut profile, 0x0210_0000); // 8  version 2.1.0
    profile.extend_from_slice(b"mntr"); // 12 device class (monitor)
    profile.extend_from_slice(b"RGB "); // 16 color space
    profile.extend_from_slice(b"XYZ "); // 20 PCS
    push_u16_be(&mut profile, 2025); // 24 year
    push_u16_be(&mut profile, 1); // 26 month
    push_u16_be(&mut profile, 1); // 28 day
    push_u16_be(&mut profile, 0); // 30 hour
    push_u16_be(&mut profile, 0); // 32 minute
    push_u16_be(&mut profile, 0); // 34 second
    profile.extend_from_slice(b"acsp"); // 36 file signature
    push_u32_be(&mut profile, 0); // 40 primary platform
    push_u32_be(&mut profile, 0); // 44 flags
    push_u32_be(&mut profile, 0); // 48 manufacturer
    push_u32_be(&mut profile, 0); // 52 model
    profile.extend_from_slice(&[0u8; 8]); // 56 attributes
    push_u32_be(&mut profile, 0); // 64 rendering intent
    push_s15fixed16(&mut profile, 0.9642); // 68 PCS illuminant X (D50)
    push_s15fixed16(&mut profile, 1.0000); // 72 PCS illuminant Y
    push_s15fixed16(&mut profile, 0.8249); // 76 PCS illuminant Z
    push_u32_be(&mut profile, 0); // 80 creator
    profile.extend_from_slice(&[0u8; 16]); // 84 profile ID
    profile.extend_from_slice(&[0u8; 28]); // 100 reserved
    assert_eq!(profile.len(), header_size);

    // Tag table
    push_u32_be(&mut profile, num_tags as u32);
    for (sig, offset, size) in &entries {
        profile.extend_from_slice(sig);
        push_u32_be(&mut profile, *offset as u32);
        push_u32_be(&mut profile, *size as u32);
    }
    assert_eq!(profile.len(), data_start);

    // Tag data
    profile.extend_from_slice(&all_data);

    // Fix profile size (it may differ if last block needed no padding)
    let final_size = profile.len() as u32;
    profile[0..4].copy_from_slice(&final_size.to_be_bytes());

    profile
}

#[tauri::command]
pub async fn export_preset_as_icc(preset_id: String) -> Result<Option<String>, String> {
    #[cfg(target_os = "windows")]
    {
        // Look up preset parameters
        let presets = get_filter_presets().await?;
        let preset = presets
            .iter()
            .find(|p| p.id == preset_id)
            .ok_or(format!("未找到预设: {}", preset_id))?;

        // Build gamma ramp from preset parameters
        let mode = FilterMode::from_i32(preset.mode);
        let ramp = build_gamma_ramp(
            preset.temperature,
            preset.brightness,
            preset.contrast,
            preset.saturation,
            mode,
            None,
        );

        // Show save file dialog
        let default_name = format!("NexBox_{}.icc", preset.name);
        let result = rfd::FileDialog::new()
            .set_title("保存 ICC 色彩配置文件")
            .add_filter("ICC 文件", &["icc", "icm"])
            .set_file_name(&default_name)
            .save_file();

        let path = match result {
            Some(p) => p,
            None => return Ok(None),
        };

        // Build ICC profile binary
        let description = format!("NexBox {} Filter", preset.name);
        let icc_data = build_icc_profile(&ramp, &description);

        // Write to file
        fs::write(&path, &icc_data).map_err(|e| format!("无法保存文件: {}", e))?;

        log::info!(
            "ICC profile exported: {} ({} bytes) from preset '{}'",
            path.display(),
            icc_data.len(),
            preset.name
        );

        Ok(path.to_str().map(|s| s.to_string()))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("此功能仅支持 Windows 系统".to_string())
    }
}