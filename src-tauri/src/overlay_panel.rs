use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::{Emitter, Manager};

static OVERLAY_ACTIVE: AtomicBool = AtomicBool::new(false);
static OVERLAY_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
static DRAG_MODE: AtomicBool = AtomicBool::new(false);
static POSITION_CHANGED: AtomicBool = AtomicBool::new(false);

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct DisplayItem {
    pub id: String,
    pub label: String,
    pub enabled: bool,
}

pub type DisplayItems = Vec<DisplayItem>;

fn default_style() -> String {
    "default".to_string()
}

fn default_font() -> String {
    "MiSans Medium".to_string()
}

fn default_display_items() -> DisplayItems {
    vec![
        DisplayItem { id: "cpu_usage".to_string(), label: "CPU占用".to_string(), enabled: true },
        DisplayItem { id: "gpu_temp".to_string(), label: "GPU温度".to_string(), enabled: true },
        DisplayItem { id: "gpu_usage".to_string(), label: "GPU占用".to_string(), enabled: true },
        DisplayItem { id: "memory_usage".to_string(), label: "内存占用".to_string(), enabled: true },
        DisplayItem { id: "game_ping".to_string(), label: "游戏延迟".to_string(), enabled: true },
        DisplayItem { id: "delta_password".to_string(), label: "三角洲密码".to_string(), enabled: true },
        DisplayItem { id: "fps".to_string(), label: "FPS".to_string(), enabled: false },
    ]
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CustomOverlayItem {
    pub id: String,
    pub text: String,
    pub color: String,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct OverlaySettings {
    #[serde(default = "default_display_items")]
    pub display_items: DisplayItems,
    #[serde(default)]
    pub custom_items: Vec<CustomOverlayItem>,
    pub opacity: u8,
    #[serde(default = "default_style")]
    pub style: String,
    #[serde(default = "default_font")]
    pub font: String,
    #[serde(default)]
    pub position_x: Option<i32>,
    #[serde(default)]
    pub position_y: Option<i32>,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            display_items: default_display_items(),
            custom_items: Vec::new(),
            opacity: 255,
            style: "default".to_string(),
            font: "MiSans Medium".to_string(),
            position_x: None,
            position_y: None,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct OverlayResult {
    pub success: bool,
    pub message: String,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct OverlayHardwareData {
    fps: Option<u32>,
    cpu_usage: Option<u16>,
    gpu_temp: Option<u32>,
    gpu_usage: Option<u32>,
    memory_usage: Option<f64>,
    delta_password: Option<String>,
    game_ping: Option<u32>,
}

impl Default for OverlayHardwareData {
    fn default() -> Self {
        Self {
            fps: None,
            cpu_usage: None,
            gpu_temp: None,
            gpu_usage: None,
            memory_usage: None,
            delta_password: None,
            game_ping: None,
        }
    }
}

static CURRENT_SETTINGS: Mutex<Option<OverlaySettings>> = Mutex::new(None);
static CURRENT_HARDWARE_DATA: Mutex<Option<OverlayHardwareData>> = Mutex::new(None);
static MISANS_FONT_PATH: Mutex<Option<String>> = Mutex::new(None);

fn get_or_init_settings() -> OverlaySettings {
    let mut settings_lock = CURRENT_SETTINGS.lock().unwrap();
    if settings_lock.is_none() {
        *settings_lock = Some(OverlaySettings::default());
    }
    settings_lock.as_ref().unwrap().clone()
}

fn collect_hardware_data() -> OverlayHardwareData {
    let fps = crate::game_fps::get_cached_fps();
    let cpu_usage = crate::hardware::get_overlay_cpu_usage();
    let (gpu_temp, gpu_usage) = crate::hardware::get_overlay_gpu_info();

    let memory_usage = {
        use sysinfo::System;
        let mut sys = System::new();
        sys.refresh_memory();
        if sys.total_memory() > 0 {
            Some((sys.used_memory() as f64 / sys.total_memory() as f64) * 100.0)
        } else {
            None
        }
    };

    let delta_password = crate::delta_force::get_cached_delta_password();

    let game_ping = crate::game_ping::get_cached_ping();

    let new_data = OverlayHardwareData {
        fps,
        cpu_usage,
        gpu_temp,
        gpu_usage,
        memory_usage,
        delta_password,
        game_ping,
    };

    let prev_data = CURRENT_HARDWARE_DATA.lock().unwrap().clone();
    let result = if let Some(prev) = prev_data {
        OverlayHardwareData {
            fps: new_data.fps.or(prev.fps),
            cpu_usage: new_data.cpu_usage.or(prev.cpu_usage),
            gpu_temp: new_data.gpu_temp.or(prev.gpu_temp),
            gpu_usage: new_data.gpu_usage.or(prev.gpu_usage),
            memory_usage: new_data.memory_usage.or(prev.memory_usage),
            delta_password: new_data.delta_password.or_else(|| prev.delta_password.clone()),
            game_ping: new_data.game_ping.or(prev.game_ping),
        }
    } else {
        new_data
    };

    result
}

#[cfg(target_os = "windows")]
mod win32 {
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::Graphics::GdiPlus::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    use windows_sys::Win32::UI::Accessibility::*;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use std::ptr;
    use std::sync::Mutex;
    use std::result::Result::Ok;

    static GDIPLUS_TOKEN: Mutex<Option<usize>> = Mutex::new(None);
    static WIN_EVENT_HOOK: Mutex<Option<usize>> = Mutex::new(None);

    unsafe extern "system" fn win_event_proc(
        _h_win_event_hook: *mut std::ffi::c_void,
        _event: u32,
        hwnd: HWND,
        id_object: i32,
        _id_child: i32,
        _dw_event_thread: u32,
        _dwms_event_time: u32,
    ) {
        if id_object != 0 || hwnd.is_null() {
            return;
        }
        let overlay_hwnd = super::OVERLAY_HANDLE.load(std::sync::atomic::Ordering::SeqCst);
        if overlay_hwnd.is_null() {
            return;
        }
        if hwnd != overlay_hwnd {
            SetWindowPos(
                overlay_hwnd,
                HWND_TOPMOST,
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }

    pub unsafe fn install_topmost_guard() {
        let hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            std::ptr::null_mut(),
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        );
        if !hook.is_null() {
            let mut lock = WIN_EVENT_HOOK.lock().unwrap();
            *lock = Some(hook as usize);
        }
    }

    pub unsafe fn uninstall_topmost_guard() {
        let mut lock = WIN_EVENT_HOOK.lock().unwrap();
        if let Some(hook) = lock.take() {
            UnhookWinEvent(hook as *mut std::ffi::c_void);
        }
    }

    pub unsafe fn init_gdiplus() -> bool {
        let mut token = GDIPLUS_TOKEN.lock().unwrap();
        if token.is_some() {
            return true;
        }

        let mut input = GdiplusStartupInput {
            GdiplusVersion: 1,
            DebugEventCallback: 0,
            SuppressBackgroundThread: 0,
            SuppressExternalCodecs: 0,
        };

        let mut token_value: usize = 0;
        let result = GdiplusStartup(&mut token_value, &mut input, ptr::null_mut());

        if result == 0 {
            *token = Some(token_value);
            true
        } else {
            log::error!("GDI+ 初始化失败: {}", result);
            false
        }
    }

    pub unsafe fn shutdown_gdiplus() {
        let mut token = GDIPLUS_TOKEN.lock().unwrap();
        if let Some(t) = token.take() {
            GdiplusShutdown(t);
        }
    }

    fn calculate_window_width(settings: &super::OverlaySettings) -> i32 {
        // 使用较小的单项宽度以缩小悬浮框
        let normal_item_width = 130;
        // 默认密码项宽度（逻辑像素）
        let mut password_item_width = 220;

        // 如果已启用密码显示，尝试基于当前缓存的密码测量实际宽度
        if settings.display_items.iter().any(|item| item.id == "delta_password" && item.enabled) {
            if let Ok(lock) = super::CURRENT_HARDWARE_DATA.lock() {
                if let Some(ref data) = *lock {
                    if let Some(ref pwd) = data.delta_password {
                        unsafe {
                            // 使用屏幕 DC 和字体测量文本宽度，按 DPI 缩放
                            let screen_dc = GetDC(ptr::null_mut());
                            if !screen_dc.is_null() {
                                let dpi_x = GetDeviceCaps(screen_dc, 88);
                                let dpi_scale = dpi_x as f32 / 96.0;
                                let hfont = create_compatible_font(dpi_scale, &settings.font);
                                if !hfont.is_null() {
                                    let val_w = measure_text_width(screen_dc, hfont, pwd);
                                    let est = val_w + (12.0 * dpi_scale) as i32 + 20;
                                                if est > password_item_width {
                                                    password_item_width = est;
                                                }
                                    DeleteObject(hfont as _);
                                }
                                ReleaseDC(ptr::null_mut(), screen_dc);
                            }
                        }
                    }
                }
            }
        }

        let mut width = 0i32;
        let mut enabled_count = 0i32;
        for item in &settings.display_items {
            if item.enabled {
                enabled_count += 1;
                match item.id.as_str() {
                    "delta_password" => { width += password_item_width; }
                    _ => { width += normal_item_width; }
                }
            }
        }

        // 自定义项宽度（各 150px 基础宽度）
        let custom_item_width = 150;
        let mut custom_count = 0i32;
        for custom in &settings.custom_items {
            if custom.enabled && !custom.text.is_empty() {
                width += custom_item_width;
                custom_count += 1;
            }
        }

        if width == 0 { return 200; }
        enabled_count += custom_count;
        let sep_count = if enabled_count > 1 { enabled_count - 1 } else { 0 };
        width + 32 + sep_count * 16
    }

    pub unsafe fn create_overlay_window(
        settings: &super::OverlaySettings,
    ) -> Result<HWND, String> {
        init_gdiplus();

        let h_instance = GetModuleHandleW(ptr::null());
        if h_instance.is_null() {
            return Err("无法获取模块句柄".to_string());
        }

        let class_name = windows_sys::core::w!("NexBoxOverlayPanel");

        let wnd_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: h_instance,
            hIcon: LoadIconW(h_instance, IDI_APPLICATION),
            hCursor: LoadCursorW(h_instance, IDC_ARROW),
            hbrBackground: CreateSolidBrush(0),
            lpszMenuName: ptr::null(),
            lpszClassName: class_name,
        };

        if RegisterClassW(&wnd_class) == 0 {
            let error = GetLastError();
            if error != 1410 {
                return Err(format!("注册窗口类失败: {}", error));
            }
        }

        let screen_dc = GetDC(ptr::null_mut());
        let dpi_x = if screen_dc.is_null() { 96 } else { GetDeviceCaps(screen_dc, 88) };
        if !screen_dc.is_null() {
            ReleaseDC(ptr::null_mut(), screen_dc);
        }
        let dpi_scale = dpi_x as f32 / 96.0;

        let logical_width = calculate_window_width(settings);
        let logical_height = if settings.style == "dynamic_island" { 36 } else { 28 };
        let physical_width = (logical_width as f32 * dpi_scale) as i32;
        let physical_height = (logical_height as f32 * dpi_scale) as i32;

        // 使用保存的位置，或使用默认位置
        let (x, y) = if let (Some(px), Some(py)) = (settings.position_x, settings.position_y) {
            (px, py)
        } else {
            let screen_width = GetSystemMetrics(SM_CXSCREEN);
            let default_x = (screen_width - physical_width) / 2;
            let default_y = if settings.style == "dynamic_island" { 4 } else { 0 };
            (default_x, default_y)
        };

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT,
            class_name,
            windows_sys::core::w!("NexBox Overlay Panel"),
            WS_POPUP,
            x,
            y,
            physical_width,
            physical_height,
            ptr::null_mut(),
            ptr::null_mut(),
            h_instance,
            ptr::null_mut(),
        );

        if hwnd.is_null() {
            return Err("创建窗口失败".to_string());
        }

        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);

        Ok(hwnd)
    }

    pub unsafe fn destroy_overlay_window(hwnd: HWND) -> bool {
        if hwnd.is_null() {
            return false;
        }
        KillTimer(hwnd, 1);
        DestroyWindow(hwnd) != 0
    }

    unsafe fn create_compatible_font(dpi_scale: f32, font_name: &str) -> HFONT {
        let font_height = -(13.0 * dpi_scale).round() as i32;
        let wide_name: Vec<u16> = font_name.encode_utf16().chain(std::iter::once(0)).collect();
        CreateFontW(
            font_height,
            0,
            0,
            0,
            FW_NORMAL as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            (DEFAULT_PITCH | FF_DONTCARE) as u32,
            wide_name.as_ptr(),
        )
    }

    pub unsafe fn register_custom_font(path: &str) -> bool {
        let wide_path: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let result = AddFontResourceExW(wide_path.as_ptr(), FR_PRIVATE, std::ptr::null_mut());
        result > 0
    }

    pub unsafe fn unregister_custom_font(path: &str) -> bool {
        let wide_path: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let result = RemoveFontResourceExW(wide_path.as_ptr(), FR_PRIVATE, std::ptr::null_mut());
        result > 0
    }

    unsafe fn measure_text_width(hdc: HDC, hfont: HFONT, text: &str) -> i32 {
        let old_font = SelectObject(hdc, hfont as _);
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let mut size = SIZE { cx: 0, cy: 0 };
        GetTextExtentPoint32W(hdc, wide.as_ptr(), (wide.len() - 1) as i32, &mut size);
        SelectObject(hdc, old_font);
        size.cx
    }

    struct DisplayItem {
        label: String,
        value: String,
        label_width: i32,
        value_width: i32,
        total_width: i32,
        custom_color: Option<u32>,
    }

    fn parse_hex_color(hex: &str) -> u32 {
        let hex = hex.trim_start_matches('#');
        if let Ok(val) = u32::from_str_radix(hex, 16) {
            // 前端使用 #RRGGBB 格式，GDI+ 颜色格式为 0x00BBGGRR
            let r = (val >> 16) & 0xFF;
            let g = (val >> 8) & 0xFF;
            let b = val & 0xFF;
            (b << 16) | (g << 8) | r
        } else {
            0x00FFFFFF
        }
    }

    fn build_display_items(
        settings: &super::OverlaySettings,
        data: &super::OverlayHardwareData,
    ) -> Vec<DisplayItem> {
        let mut items = Vec::new();
        for display_item in &settings.display_items {
            if !display_item.enabled {
                continue;
            }
            match display_item.id.as_str() {
                "cpu_usage" => {
                    let val = data.cpu_usage.map(|v| format!("{}%", v)).unwrap_or_else(|| "--%".to_string());
                    items.push(DisplayItem { label: "CPU".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: None });
                }
                "gpu_temp" => {
                    let val = data.gpu_temp.map(|v| format!("{}\u{00B0}C", v)).unwrap_or_else(|| "--\u{00B0}C".to_string());
                    items.push(DisplayItem { label: "GPU".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: None });
                }
                "gpu_usage" => {
                    let val = data.gpu_usage.map(|v| format!("{}%", v)).unwrap_or_else(|| "--%".to_string());
                    items.push(DisplayItem { label: "GPU".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: None });
                }
                "memory_usage" => {
                    let val = data.memory_usage.map(|v| format!("{}%", v.round() as i32)).unwrap_or_else(|| "--%".to_string());
                    items.push(DisplayItem { label: "RAM".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: None });
                }
                "delta_password" => {
                    let val = data.delta_password.as_deref().unwrap_or("--").to_string();
                    items.push(DisplayItem { label: "".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: None });
                }
                "game_ping" => {
                    let val = data.game_ping.map(|v| format!("{}ms", v)).unwrap_or_else(|| "--ms".to_string());
                    items.push(DisplayItem { label: "PING".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: None });
                }
                "fps" => {
                    let (val, color) = match data.fps {
                        Some(v) => {
                            let c = if v < 30 {
                                0x000000FFu32
                            } else if v < 60 {
                                0x0000FFFFu32
                            } else {
                                0x0000FF00u32
                            };
                            (format!("{}", v), Some(c))
                        }
                        None => ("--".to_string(), None),
                    };
                    items.push(DisplayItem { label: "FPS".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: color });
                }
                _ => {}
            }
        }
        for custom in &settings.custom_items {
            if custom.enabled && !custom.text.is_empty() {
                let color = parse_hex_color(&custom.color);
                items.push(DisplayItem {
                    label: String::new(),
                    value: custom.text.clone(),
                    label_width: 0,
                    value_width: 0,
                    total_width: 0,
                    custom_color: Some(color),
                });
            }
        }
        items
    }

    unsafe fn measure_and_layout_items(
        hdc: HDC,
        hfont: HFONT,
        items: &mut [DisplayItem],
        dpi_scale: f32,
    ) -> i32 {
        let gap = (10.0 * dpi_scale) as i32;
        let mut total = 0i32;
        for item in items.iter_mut() {
            item.label_width = measure_text_width(hdc, hfont, &item.label);
            item.value_width = measure_text_width(hdc, hfont, &item.value);
            if item.label.is_empty() {
                item.total_width = item.value_width;
            } else {
                item.total_width = item.label_width + gap + item.value_width;
            }
            total += item.total_width;
        }
        total
    }

    pub unsafe fn draw_overlay_content(
        hwnd: HWND,
        settings: &super::OverlaySettings,
        data: &super::OverlayHardwareData,
    ) {
        let dpi_scale = {
            let dc = GetDC(hwnd);
            let dpi = if dc.is_null() { 96 } else { GetDeviceCaps(dc, 88) };
            if !dc.is_null() {
                ReleaseDC(hwnd, dc);
            }
            dpi as f32 / 96.0
        };

        let hfont = create_compatible_font(dpi_scale, &settings.font);
        if hfont.is_null() {
            return;
        }

        let temp_dc = GetDC(ptr::null_mut());
        let mut items = build_display_items(settings, data);
        let padding = (16.0 * dpi_scale) as i32;
        let item_gap = (16.0 * dpi_scale) as i32;
        let content_width = measure_and_layout_items(temp_dc, hfont, &mut items, dpi_scale);
        ReleaseDC(ptr::null_mut(), temp_dc);
        let sep_count = if items.len() > 1 { items.len() as i32 - 1 } else { 0 };
        let total_content_width = content_width + sep_count * item_gap + padding * 2;
        let logical_height = 28;
        let physical_height = (logical_height as f32 * dpi_scale) as i32;

        let dib_width = total_content_width;
        let dib_height = physical_height;

        let screen_dc = GetDC(ptr::null_mut());
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = dib_width;
        bmi.bmiHeader.biHeight = -dib_height;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;

        let mut bits: *mut std::ffi::c_void = ptr::null_mut();
        let hbitmap = CreateDIBSection(screen_dc, &bmi, DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0);
        ReleaseDC(ptr::null_mut(), screen_dc);

        if hbitmap.is_null() {
            DeleteObject(hfont as _);
            return;
        }

        let mem_dc = CreateCompatibleDC(ptr::null_mut());
        let old_bmp = SelectObject(mem_dc, hbitmap as HGDIOBJ);

        let mut graphics: *mut GpGraphics = ptr::null_mut();
        if GdipCreateFromHDC(mem_dc, &mut graphics) != 0 {
            SelectObject(mem_dc, old_bmp);
            DeleteObject(hbitmap as HGDIOBJ);
            DeleteDC(mem_dc);
            DeleteObject(hfont as _);
            return;
        }

        GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias);

        let mut clear_brush: *mut GpSolidFill = ptr::null_mut();
        GdipCreateSolidFill(0x00000000, &mut clear_brush);
        GdipFillRectangle(graphics, clear_brush as *mut GpBrush, 0.0, 0.0, dib_width as f32, dib_height as f32);
        GdipDeleteBrush(clear_brush as *mut GpBrush);

        let bg_argb: u32 = ((settings.opacity as u32) << 24) | 0x00111111;
        let mut bg_brush: *mut GpSolidFill = ptr::null_mut();
        GdipCreateSolidFill(bg_argb, &mut bg_brush);
        GdipFillRectangle(graphics, bg_brush as *mut GpBrush, 0.0, 0.0, dib_width as f32, dib_height as f32);
        GdipDeleteBrush(bg_brush as *mut GpBrush);
        GdipDeleteGraphics(graphics);

        let old_font = SelectObject(mem_dc, hfont as _);
        SetBkMode(mem_dc, TRANSPARENT as i32);

        let gap = (10.0 * dpi_scale) as i32;
        let mut current_x: i32 = padding;
        let win_height_i32 = dib_height;

        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                current_x += item_gap;
            }

            if !item.label.is_empty() {
                let wide_label: Vec<u16> = item.label.encode_utf16().chain(std::iter::once(0)).collect();
                let mut label_rect = RECT {
                    left: current_x,
                    top: 0,
                    right: current_x + item.label_width,
                    bottom: win_height_i32,
                };
                SetTextColor(mem_dc, 0x00FFFFFF);
                DrawTextW(
                    mem_dc,
                    wide_label.as_ptr(),
                    (wide_label.len() - 1) as i32,
                    &mut label_rect,
                    DT_RIGHT | DT_VCENTER | DT_SINGLELINE,
                );
            }

            let value_x = if item.label.is_empty() {
                current_x
            } else {
                current_x + item.label_width + gap
            };
            let wide_value: Vec<u16> = item.value.encode_utf16().chain(std::iter::once(0)).collect();
            let mut value_rect = RECT {
                left: value_x,
                top: 0,
                right: value_x + item.value_width,
                bottom: win_height_i32,
            };

            let mut color: u32 = 0x00FFFFFF;
            if let Some(custom_color) = item.custom_color {
                color = custom_color;
            } else if !item.label.is_empty() && !item.value.contains("--") {
                let mut num_str = String::new();
                for ch in item.value.chars() {
                    if ch.is_ascii_digit() || ch == '.' {
                        num_str.push(ch);
                    } else if !num_str.is_empty() {
                        break;
                    }
                }
                if !num_str.is_empty() {
                    if let Ok(nf) = num_str.parse::<f32>() {
                        let nv = nf as i32;
                        if nv < 50 {
                            color = 0x0000FF00;
                        } else if nv < 80 {
                            color = 0x0000FFFF;
                        } else {
                            color = 0x000000FF;
                        }
                    }
                }
            }

            SetTextColor(mem_dc, color);
            DrawTextW(
                mem_dc,
                wide_value.as_ptr(),
                (wide_value.len() - 1) as i32,
                &mut value_rect,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            current_x += item.total_width;
        }

        SelectObject(mem_dc, old_font);

        if !bits.is_null() {
            let pixels = std::slice::from_raw_parts_mut(
                bits as *mut u32,
                (dib_width * dib_height) as usize,
            );
            for pixel in pixels.iter_mut() {
                let alpha = (*pixel >> 24) & 0xFF;
                let rgb = *pixel & 0x00FFFFFF;
                if alpha == 0 && rgb != 0 {
                    *pixel = 0xFF000000 | rgb;
                }
            }
        }

        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let default_x = (screen_width - dib_width) / 2;
        let use_x = settings.position_x.unwrap_or(default_x);
        let use_y = settings.position_y.unwrap_or(0);

        let ppt_dst = POINT { x: use_x, y: use_y };
        let psize = SIZE { cx: dib_width, cy: dib_height };
        let ppt_src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };

        UpdateLayeredWindow(
            hwnd,
            ptr::null_mut(),
            &ppt_dst,
            &psize,
            mem_dc,
            &ppt_src,
            0,
            &blend,
            ULW_ALPHA,
        );

        SelectObject(mem_dc, old_bmp);
        DeleteObject(hbitmap as HGDIOBJ);
        DeleteDC(mem_dc);
        DeleteObject(hfont as _);
    }

    pub unsafe fn draw_overlay_content_dynamic_island(
        hwnd: HWND,
        settings: &super::OverlaySettings,
        data: &super::OverlayHardwareData,
    ) {
        let dpi_scale = {
            let dc = GetDC(hwnd);
            let dpi = if dc.is_null() { 96 } else { GetDeviceCaps(dc, 88) };
            if !dc.is_null() {
                ReleaseDC(hwnd, dc);
            }
            dpi as f32 / 96.0
        };

        let hfont = create_compatible_font(dpi_scale, &settings.font);
        if hfont.is_null() {
            return;
        }

        // Build a temp DC just for measurement
        let temp_dc = GetDC(ptr::null_mut());
        let mut items = build_display_items(settings, data);
        let padding = (16.0 * dpi_scale) as i32;
        let item_gap = (16.0 * dpi_scale) as i32;
        let content_width = measure_and_layout_items(temp_dc, hfont, &mut items, dpi_scale);
        ReleaseDC(ptr::null_mut(), temp_dc);
        let sep_count = if items.len() > 1 { items.len() as i32 - 1 } else { 0 };
        let total_content_width = content_width + sep_count * item_gap + padding * 2;
        let logical_height = 36;
        let physical_height = (logical_height as f32 * dpi_scale) as i32;

        let dib_width = total_content_width;
        let dib_height = physical_height;

        // --- Create 32-bit ARGB DIB section (like crosshair) ---
        let screen_dc = GetDC(ptr::null_mut());
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = dib_width;
        bmi.bmiHeader.biHeight = -dib_height;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;

        let mut bits: *mut std::ffi::c_void = ptr::null_mut();
        let hbitmap = CreateDIBSection(screen_dc, &bmi, DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0);
        ReleaseDC(ptr::null_mut(), screen_dc);

        if hbitmap.is_null() {
            DeleteObject(hfont as _);
            return;
        }

        let mem_dc = CreateCompatibleDC(ptr::null_mut());
        let old_bmp = SelectObject(mem_dc, hbitmap as HGDIOBJ);

        // --- GDI+ anti-aliased rounded rect background ---
        let mut graphics: *mut GpGraphics = ptr::null_mut();
        if GdipCreateFromHDC(mem_dc, &mut graphics) != 0 {
            SelectObject(mem_dc, old_bmp);
            DeleteObject(hbitmap as HGDIOBJ);
            DeleteDC(mem_dc);
            DeleteObject(hfont as _);
            return;
        }

        GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias);

        // Clear to fully transparent
        let mut clear_brush: *mut GpSolidFill = ptr::null_mut();
        GdipCreateSolidFill(0x00000000, &mut clear_brush);
        GdipFillRectangle(graphics, clear_brush as *mut GpBrush, 0.0, 0.0, dib_width as f32, dib_height as f32);
        GdipDeleteBrush(clear_brush as *mut GpBrush);

        // Draw rounded rect with GDI+ (proper per-pixel alpha anti-aliasing)
        let bg_argb: u32 = ((settings.opacity as u32) << 24) | 0x00111111;
        let corner_r = dib_height as f32 * 0.5;
        let mut bg_brush: *mut GpSolidFill = ptr::null_mut();
        GdipCreateSolidFill(bg_argb, &mut bg_brush);

        let mut path: *mut GpPath = ptr::null_mut();
        GdipCreatePath(FillModeAlternate, &mut path);
        if !path.is_null() {
            let w = dib_width as f32;
            let h = dib_height as f32;
            let r = corner_r;
            GdipAddPathArc(path, 0.0, 0.0, r * 2.0, r * 2.0, 180.0, 90.0);
            GdipAddPathLine(path, r, 0.0, w - r, 0.0);
            GdipAddPathArc(path, w - r * 2.0, 0.0, r * 2.0, r * 2.0, 270.0, 90.0);
            GdipAddPathLine(path, w, r, w, h - r);
            GdipAddPathArc(path, w - r * 2.0, h - r * 2.0, r * 2.0, r * 2.0, 0.0, 90.0);
            GdipAddPathLine(path, w - r, h, r, h);
            GdipAddPathArc(path, 0.0, h - r * 2.0, r * 2.0, r * 2.0, 90.0, 90.0);
            GdipAddPathLine(path, 0.0, h - r, 0.0, r);
            GdipClosePathFigure(path);
            GdipFillPath(graphics, bg_brush as *mut GpBrush, path);
            GdipDeletePath(path);
        }
        GdipDeleteBrush(bg_brush as *mut GpBrush);
        GdipDeleteGraphics(graphics);

        // --- Draw text using GDI ---
        let old_font = SelectObject(mem_dc, hfont as _);
        SetBkMode(mem_dc, TRANSPARENT as i32);

        let gap = (10.0 * dpi_scale) as i32;
        let mut current_x: i32 = padding;
        let win_height_i32 = dib_height;

        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                current_x += item_gap;
            }

            if !item.label.is_empty() {
                let wide_label: Vec<u16> = item.label.encode_utf16().chain(std::iter::once(0)).collect();
                let mut label_rect = RECT {
                    left: current_x,
                    top: 0,
                    right: current_x + item.label_width,
                    bottom: win_height_i32,
                };
                SetTextColor(mem_dc, 0x00FFFFFF);
                DrawTextW(
                    mem_dc,
                    wide_label.as_ptr(),
                    (wide_label.len() - 1) as i32,
                    &mut label_rect,
                    DT_RIGHT | DT_VCENTER | DT_SINGLELINE,
                );
            }

            let value_x = if item.label.is_empty() {
                current_x
            } else {
                current_x + item.label_width + gap
            };
            let wide_value: Vec<u16> = item.value.encode_utf16().chain(std::iter::once(0)).collect();
            let mut value_rect = RECT {
                left: value_x,
                top: 0,
                right: value_x + item.value_width,
                bottom: win_height_i32,
            };

            let mut color: u32 = 0x00FFFFFF;
            if let Some(custom_color) = item.custom_color {
                color = custom_color;
            } else if !item.label.is_empty() && !item.value.contains("--") {
                let mut num_str = String::new();
                for ch in item.value.chars() {
                    if ch.is_ascii_digit() || ch == '.' {
                        num_str.push(ch);
                    } else if !num_str.is_empty() {
                        break;
                    }
                }
                if !num_str.is_empty() {
                    if let Ok(nf) = num_str.parse::<f32>() {
                        let nv = nf as i32;
                        if nv < 50 {
                            color = 0x0000FF00;
                        } else if nv < 80 {
                            color = 0x0000FFFF;
                        } else {
                            color = 0x000000FF;
                        }
                    }
                }
            }

            SetTextColor(mem_dc, color);
            DrawTextW(
                mem_dc,
                wide_value.as_ptr(),
                (wide_value.len() - 1) as i32,
                &mut value_rect,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            current_x += item.total_width;
        }

        SelectObject(mem_dc, old_font);

        // Fix alpha for text pixels: GDI sets RGB but alpha stays 0
        if !bits.is_null() {
            let pixels = std::slice::from_raw_parts_mut(
                bits as *mut u32,
                (dib_width * dib_height) as usize,
            );
            for pixel in pixels.iter_mut() {
                let alpha = (*pixel >> 24) & 0xFF;
                let rgb = *pixel & 0x00FFFFFF;
                if alpha == 0 && rgb != 0 {
                    *pixel = 0xFF000000 | rgb;
                }
            }
        }

        // --- Position and composite via UpdateLayeredWindow ---
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let default_x = (screen_width - dib_width) / 2;
        let use_x = settings.position_x.unwrap_or(default_x);
        let use_y = settings.position_y.unwrap_or(4);

        let ppt_dst = POINT { x: use_x, y: use_y };
        let psize = SIZE { cx: dib_width, cy: dib_height };
        let ppt_src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };

        UpdateLayeredWindow(
            hwnd,
            ptr::null_mut(),
            &ppt_dst,
            &psize,
            mem_dc,
            &ppt_src,
            0,
            &blend,
            ULW_ALPHA,
        );

        SelectObject(mem_dc, old_bmp);
        DeleteObject(hbitmap as HGDIOBJ);
        DeleteDC(mem_dc);
        DeleteObject(hfont as _);
    }

    pub unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT {
                    hdc: ptr::null_mut(),
                    fErase: 0,
                    rcPaint: RECT {
                        left: 0,
                        top: 0,
                        right: 0,
                        bottom: 0,
                    },
                    fRestore: 0,
                    fIncUpdate: 0,
                    rgbReserved: [0u8; 32],
                };
                BeginPaint(hwnd, &mut ps);
                EndPaint(hwnd, &ps);
                0
            }
            WM_TIMER => {
                let data = super::collect_hardware_data();
                *super::CURRENT_HARDWARE_DATA.lock().unwrap() = Some(data.clone());
                let settings = super::get_or_init_settings();
                if settings.style == "dynamic_island" {
                    draw_overlay_content_dynamic_island(hwnd, &settings, &data);
                } else {
                    draw_overlay_content(hwnd, &settings, &data);
                }
                0
            }
            WM_NCHITTEST => {
                // 拖动模式下返回 HTCAPTION 允许拖动
                if super::DRAG_MODE.load(std::sync::atomic::Ordering::SeqCst) {
                    HTCAPTION as LRESULT
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
            WM_EXITSIZEMOVE => {
                // 拖动结束后只保存位置，不退出拖动模式
                // 退出由前端按钮控制，避免样式切换导致位置重置
                if super::DRAG_MODE.load(std::sync::atomic::Ordering::SeqCst) {
                    // 获取当前窗口位置
                    let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
                    GetWindowRect(hwnd, &mut rect);

                    // 保存位置到设置
                    {
                        let mut settings_lock = super::CURRENT_SETTINGS.lock().unwrap();
                        if let Some(ref mut settings) = *settings_lock {
                            settings.position_x = Some(rect.left);
                            settings.position_y = Some(rect.top);
                        }
                    }

                    // 标记位置已变更
                    super::POSITION_CHANGED.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

// 设置拖动模式
pub fn set_drag_mode(enabled: bool) {
    DRAG_MODE.store(enabled, Ordering::SeqCst);

    #[cfg(target_os = "windows")]
    unsafe {
        let hwnd = OVERLAY_HANDLE.load(Ordering::SeqCst);
        if !hwnd.is_null() {
            use windows_sys::Win32::UI::WindowsAndMessaging::*;

            // 获取当前窗口样式
            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;

            if enabled {
                // 进入拖动模式：移除 WS_EX_TRANSPARENT
                let new_style = ex_style & !WS_EX_TRANSPARENT;
                SetWindowLongW(hwnd, GWL_EXSTYLE, new_style as i32);
            } else {
                // 退出拖动模式：恢复 WS_EX_TRANSPARENT
                let new_style = ex_style | WS_EX_TRANSPARENT;
                SetWindowLongW(hwnd, GWL_EXSTYLE, new_style as i32);
            }

            // 刷新窗口样式
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }
}

#[cfg(target_os = "windows")]
pub fn start_overlay(settings: OverlaySettings) -> Result<OverlayResult, String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    if OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        return Ok(OverlayResult {
            success: true,
            message: "悬浮框已处于启用状态".to_string(),
        });
    }

    OVERLAY_ACTIVE.store(true, Ordering::SeqCst);

    {
        let mut settings_lock = CURRENT_SETTINGS.lock().unwrap();
        *settings_lock = Some(settings.clone());
    }

    // Register MiSans font if selected
    if settings.font == "MiSans Medium" {
        if let Ok(path_lock) = MISANS_FONT_PATH.lock() {
            if let Some(ref path) = *path_lock {
                unsafe {
                    win32::register_custom_font(path);
                }
            }
        }
    }

    thread::spawn(move || {
        crate::game_ping::start_ping_thread();
        crate::game_fps::start_fps_monitor();

        unsafe {
            match win32::create_overlay_window(&settings) {
                std::result::Result::Ok(hwnd) => {
                    OVERLAY_HANDLE.store(hwnd, Ordering::SeqCst);
                    crate::game_fps::set_overlay_hwnd(hwnd as u64);

                    let data = collect_hardware_data();
                    *CURRENT_HARDWARE_DATA.lock().unwrap() = Some(data.clone());

                    if settings.style == "dynamic_island" {
                        win32::draw_overlay_content_dynamic_island(hwnd, &settings, &data);
                    } else {
                        win32::draw_overlay_content(hwnd, &settings, &data);
                    }

                    SetTimer(hwnd, 1, 500, None);
                    win32::install_topmost_guard();

                    let mut msg: MSG = std::mem::zeroed();
                    while OVERLAY_ACTIVE.load(Ordering::SeqCst) {
                        while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                            if msg.message == WM_QUIT {
                                break;
                            }
                            TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                        }

                        if !OVERLAY_ACTIVE.load(Ordering::SeqCst) {
                            break;
                        }

                        thread::sleep(Duration::from_millis(50));
                    }

                    win32::uninstall_topmost_guard();
                    win32::destroy_overlay_window(hwnd);
                    crate::game_fps::clear_overlay_hwnd();
                    OVERLAY_HANDLE.store(std::ptr::null_mut(), Ordering::SeqCst);
                }
                std::result::Result::Err(e) => {
                    log::error!("创建悬浮框窗口失败: {}", e);
                    OVERLAY_ACTIVE.store(false, Ordering::SeqCst);
                }
            }
        }
    });

    Ok(OverlayResult {
        success: true,
        message: "悬浮框已启动".to_string(),
    })
}

#[cfg(target_os = "windows")]
pub fn stop_overlay() -> Result<OverlayResult, String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW;
    use windows_sys::Win32::UI::WindowsAndMessaging::WM_CLOSE;

    if !OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        return Ok(OverlayResult {
            success: true,
            message: "悬浮框已处于关闭状态".to_string(),
        });
    }

    OVERLAY_ACTIVE.store(false, Ordering::SeqCst);

    crate::game_ping::stop_ping_thread();
    crate::game_fps::stop_fps_monitor();

    unsafe {
        let hwnd = OVERLAY_HANDLE.load(Ordering::SeqCst);
        if !hwnd.is_null() {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
    }

    Ok(OverlayResult {
        success: true,
        message: "悬浮框已关闭".to_string(),
    })
}

#[cfg(not(target_os = "windows"))]
pub fn start_overlay(_settings: OverlaySettings) -> Result<OverlayResult, String> {
    Err("此功能仅支持 Windows 系统".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn stop_overlay() -> Result<OverlayResult, String> {
    Err("此功能仅支持 Windows 系统".to_string())
}

/// Toggle overlay on/off. Used by global hotkey.
pub fn toggle_overlay(app_handle: &tauri::AppHandle) -> Result<OverlayResult, String> {
    let result = if OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        stop_overlay()
    } else {
        let settings = get_or_init_settings();
        start_overlay(settings)
    };

    if result.is_ok() {
        let _ = app_handle.emit("overlay-status-changed", ());
    }

    result
}

#[tauri::command]
pub async fn start_overlay_panel(settings: Option<OverlaySettings>) -> Result<OverlayResult, String> {
    let settings = settings.unwrap_or_default();
    start_overlay(settings)
}

#[tauri::command]
pub async fn stop_overlay_panel() -> Result<OverlayResult, String> {
    stop_overlay()
}

#[tauri::command]
pub async fn toggle_overlay_panel(app_handle: tauri::AppHandle) -> Result<OverlayResult, String> {
    toggle_overlay(&app_handle)
}

#[tauri::command]
pub async fn get_overlay_panel_status() -> Result<bool, String> {
    Ok(OVERLAY_ACTIVE.load(Ordering::SeqCst))
}

#[tauri::command]
pub async fn get_overlay_hardware_data() -> Result<OverlayHardwareData, String> {
    let data = CURRENT_HARDWARE_DATA
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_default();
    Ok(data)
}

#[tauri::command]
pub async fn update_overlay_settings(settings: OverlaySettings) -> Result<OverlayResult, String> {
    let (old_style, old_font) = {
        let lock = CURRENT_SETTINGS.lock().unwrap();
        let s = lock.as_ref();
        (s.map(|s| s.style.clone()), s.map(|s| s.font.clone()))
    };
    let new_style = settings.style.clone();
    let new_font = settings.font.clone();

    {
        let mut settings_lock = CURRENT_SETTINGS.lock().unwrap();
        *settings_lock = Some(settings);
    }

    if OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        let style_changed = old_style.as_deref() != Some(&new_style);
        let font_changed = old_font.as_deref() != Some(&new_font);
        if style_changed || font_changed {
            let new_settings = CURRENT_SETTINGS.lock().unwrap().clone().unwrap_or_default();
            stop_overlay()?;
            std::thread::sleep(std::time::Duration::from_millis(200));
            start_overlay(new_settings)?;
        } else {
            #[cfg(target_os = "windows")]
            unsafe {
                let hwnd = OVERLAY_HANDLE.load(Ordering::SeqCst);
                if !hwnd.is_null() {
                    let data = CURRENT_HARDWARE_DATA.lock().unwrap().clone().unwrap_or_default();
                    let current_settings = CURRENT_SETTINGS.lock().unwrap().clone().unwrap_or_default();
                    if new_style == "dynamic_island" {
                        win32::draw_overlay_content_dynamic_island(hwnd, &current_settings, &data);
                    } else {
                        win32::draw_overlay_content(hwnd, &current_settings, &data);
                    }
                }
            }
        }
    }

    Ok(OverlayResult {
        success: true,
        message: "设置已更新".to_string(),
    })
}

#[tauri::command]
pub async fn set_overlay_drag_mode(enabled: bool) -> Result<OverlayResult, String> {
    if !OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        return Err("悬浮框未启用".to_string());
    }

    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        use windows_sys::Win32::Foundation::RECT;
        
        let hwnd = OVERLAY_HANDLE.load(Ordering::SeqCst);
        if hwnd.is_null() {
            return Err("悬浮框窗口不存在".to_string());
        }

        if !enabled {
            // 退出拖动模式时，先保存当前位置
            let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            GetWindowRect(hwnd, &mut rect);
            
            {
                let mut settings_lock = CURRENT_SETTINGS.lock().unwrap();
                if let Some(ref mut settings) = *settings_lock {
                    settings.position_x = Some(rect.left);
                    settings.position_y = Some(rect.top);
                }
            }
        }

        // 切换拖动模式
        set_drag_mode(enabled);

        if !enabled {
            // 退出拖动模式后，恢复窗口到保存的位置
            let (saved_x, saved_y) = {
                let settings_lock = CURRENT_SETTINGS.lock().unwrap();
                if let Some(ref settings) = *settings_lock {
                    (settings.position_x, settings.position_y)
                } else {
                    (None, None)
                }
            };
            
            // 获取当前位置
            let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            GetWindowRect(hwnd, &mut rect);
            
            // 如果位置发生变化，恢复到保存的位置
            if let (Some(sx), Some(sy)) = (saved_x, saved_y) {
                if rect.left != sx || rect.top != sy {
                    SetWindowPos(
                        hwnd,
                        HWND_TOPMOST,
                        sx,
                        sy,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                }
            }
            
            POSITION_CHANGED.store(false, Ordering::SeqCst);
        }
    }

    let message = if enabled { 
        "已进入拖动模式".to_string()
    } else {
        "已退出拖动模式".to_string()
    };

    Ok(OverlayResult {
        success: true,
        message,
    })
}

#[tauri::command]
pub async fn get_overlay_current_settings() -> Result<OverlaySettings, String> {
    let settings = CURRENT_SETTINGS.lock().unwrap().clone().unwrap_or_default();
    Ok(settings)
}

#[tauri::command]
pub async fn check_drag_mode_status() -> Result<bool, String> {
    // 返回当前拖动模式状态
    Ok(DRAG_MODE.load(Ordering::SeqCst))
}

#[tauri::command]
pub async fn reset_overlay_position() -> Result<OverlayResult, String> {
    if !OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        return Err("悬浮框未启用".to_string());
    }

    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;

        let hwnd = OVERLAY_HANDLE.load(Ordering::SeqCst);
        if hwnd.is_null() {
            return Err("悬浮框窗口不存在".to_string());
        }

        // 清除已保存的位置，恢复默认居中
        {
            let mut settings_lock = CURRENT_SETTINGS.lock().unwrap();
            if let Some(ref mut settings) = *settings_lock {
                settings.position_x = None;
                settings.position_y = None;
            }
        }

        // 获取当前窗口大小
        let mut rect = std::mem::zeroed();
        GetWindowRect(hwnd, &mut rect);
        let win_w = rect.right - rect.left;
        let win_h = rect.bottom - rect.top;

        // 计算居中位置
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);
        let new_x = (screen_width - win_w) / 2;
        let new_y = (screen_height - win_h) / 2;

        // 移动窗口到居中位置
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            new_x,
            new_y,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }

    Ok(OverlayResult {
        success: true,
        message: "位置已重置为默认".to_string(),
    })
}

pub fn cleanup() {
    if OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        let _ = stop_overlay();
    }
    crate::game_ping::cleanup();
    crate::game_fps::cleanup();
    #[cfg(target_os = "windows")]
    unsafe {
        // Unregister MiSans font
        if let Ok(path_lock) = MISANS_FONT_PATH.lock() {
            if let Some(ref path) = *path_lock {
                win32::unregister_custom_font(path);
            }
        }
        win32::shutdown_gdiplus();
    }
}

#[tauri::command]
pub async fn get_misans_font_path(app_handle: tauri::AppHandle) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let resource_dir = app_handle
            .path()
            .resource_dir()
            .map_err(|e| format!("Failed to get resource dir: {}", e))?;

        let font_path = resource_dir.join("MiSans-Medium.ttf");
        let path_str = font_path.to_string_lossy().to_string();

        // Cache the path for later use by start_overlay/cleanup
        if let Ok(mut lock) = MISANS_FONT_PATH.lock() {
            *lock = Some(path_str.clone());
        }

        Ok(path_str)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(String::new())
    }
}
