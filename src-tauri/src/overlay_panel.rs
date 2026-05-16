use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

static OVERLAY_ACTIVE: AtomicBool = AtomicBool::new(false);
static OVERLAY_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct DisplayItems {
    #[serde(default)]
    pub fps: bool,
    #[serde(default = "default_true")]
    pub cpu_usage: bool,
    #[serde(default = "default_true")]
    pub gpu_temp: bool,
    #[serde(default = "default_true")]
    pub gpu_usage: bool,
    #[serde(default = "default_true")]
    pub memory_usage: bool,
    #[serde(default)]
    pub delta_password: bool,
}

fn default_true() -> bool {
    true
}

impl Default for DisplayItems {
    fn default() -> Self {
        Self {
            fps: false,
            cpu_usage: true,
            gpu_temp: true,
            gpu_usage: true,
            memory_usage: true,
            delta_password: false,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct OverlaySettings {
    pub display_items: DisplayItems,
    pub opacity: u8,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            display_items: DisplayItems::default(),
            opacity: 200,
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
        }
    }
}

static CURRENT_SETTINGS: Mutex<Option<OverlaySettings>> = Mutex::new(None);
static CURRENT_HARDWARE_DATA: Mutex<Option<OverlayHardwareData>> = Mutex::new(None);

fn get_or_init_settings() -> OverlaySettings {
    let mut settings_lock = CURRENT_SETTINGS.lock().unwrap();
    if settings_lock.is_none() {
        *settings_lock = Some(OverlaySettings::default());
    }
    settings_lock.as_ref().unwrap().clone()
}

fn collect_hardware_data() -> OverlayHardwareData {
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

    let fps: Option<u32> = None;

    OverlayHardwareData {
        fps,
        cpu_usage,
        gpu_temp,
        gpu_usage,
        memory_usage,
        delta_password,
    }
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
        if settings.display_items.delta_password {
            if let Ok(lock) = super::CURRENT_HARDWARE_DATA.lock() {
                if let Some(ref data) = *lock {
                    if let Some(ref pwd) = data.delta_password {
                        unsafe {
                            // 使用屏幕 DC 和字体测量文本宽度，按 DPI 缩放
                            let screen_dc = GetDC(ptr::null_mut());
                            if !screen_dc.is_null() {
                                let dpi_x = GetDeviceCaps(screen_dc, 88);
                                let dpi_scale = dpi_x as f32 / 96.0;
                                let hfont = create_compatible_font(dpi_scale);
                                if !hfont.is_null() {
                                    let val_w = measure_text_width(screen_dc, hfont, pwd);
                                    // 加上间距与一点缓冲，确保不会截断
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
        if settings.display_items.cpu_usage { width += normal_item_width; }
        if settings.display_items.gpu_temp { width += normal_item_width; }
        if settings.display_items.gpu_usage { width += normal_item_width; }
        if settings.display_items.memory_usage { width += normal_item_width; }
        if settings.display_items.delta_password { width += password_item_width; }
        if width == 0 { return 200; }
        let enabled_count = settings.display_items.cpu_usage as i32
            + settings.display_items.gpu_temp as i32
            + settings.display_items.gpu_usage as i32
            + settings.display_items.memory_usage as i32
            + settings.display_items.delta_password as i32;
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
        // 缩小悬浮框高度以节省屏幕空间
        let logical_height = 28;
        let physical_width = (logical_width as f32 * dpi_scale) as i32;
        let physical_height = (logical_height as f32 * dpi_scale) as i32;

        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let x = (screen_width - physical_width) / 2;
        let y = 0;

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

        SetLayeredWindowAttributes(hwnd, 0, settings.opacity, LWA_ALPHA | LWA_COLORKEY);

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

    unsafe fn create_compatible_font(dpi_scale: f32) -> HFONT {
        let font_height = -(13.0 * dpi_scale).round() as i32;
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
            windows_sys::core::w!("Microsoft YaHei"),
        )
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
    }

    fn build_display_items(
        settings: &super::OverlaySettings,
        data: &super::OverlayHardwareData,
    ) -> Vec<DisplayItem> {
        let mut items = Vec::new();
        if settings.display_items.cpu_usage {
            let val = data.cpu_usage.map(|v| format!("{}%", v)).unwrap_or_else(|| "--%".to_string());
            items.push(DisplayItem { label: "CPU".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0 });
        }
        if settings.display_items.gpu_temp {
            let val = data.gpu_temp.map(|v| format!("{}\u{00B0}C", v)).unwrap_or_else(|| "--\u{00B0}C".to_string());
            items.push(DisplayItem { label: "GPU".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0 });
        }
        if settings.display_items.gpu_usage {
            let val = data.gpu_usage.map(|v| format!("{}%", v)).unwrap_or_else(|| "--%".to_string());
            items.push(DisplayItem { label: "GPU".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0 });
        }
        if settings.display_items.memory_usage {
            let val = data.memory_usage.map(|v| format!("{}%", v.round() as i32)).unwrap_or_else(|| "--%".to_string());
            items.push(DisplayItem { label: "RAM".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0 });
        }
        if settings.display_items.delta_password {
            let val = data.delta_password.as_deref().unwrap_or("--").to_string();
            // 不显示“密码”二字，直接将完整地图:密码字符串作为 value
            items.push(DisplayItem { label: "".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0 });
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

    unsafe fn draw_overlay_content(
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

        let hdc = GetDC(hwnd);
        if hdc.is_null() {
            return;
        }

        let hfont = create_compatible_font(dpi_scale);
        if hfont.is_null() {
            ReleaseDC(hwnd, hdc);
            return;
        }

        let mut items = build_display_items(settings, data);
        let padding = (16.0 * dpi_scale) as i32;
        let item_gap = (16.0 * dpi_scale) as i32;
        let content_width = measure_and_layout_items(hdc, hfont, &mut items, dpi_scale);
        let sep_count = if items.len() > 1 { items.len() as i32 - 1 } else { 0 };
        let total_content_width = content_width + sep_count * item_gap + padding * 2;
        let logical_height = 28;
        let physical_height = (logical_height as f32 * dpi_scale) as i32;

        let mut win_rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        GetWindowRect(hwnd, &mut win_rect);
        let current_width = win_rect.right - win_rect.left;
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let new_x = (screen_width - total_content_width) / 2;

        if current_width != total_content_width || win_rect.left != new_x {
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                new_x,
                0,
                total_content_width,
                physical_height,
                SWP_NOACTIVATE,
            );
        }

        let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        GetClientRect(hwnd, &mut rect);
        let win_width = (rect.right - rect.left) as f32;
        let win_height = (rect.bottom - rect.top) as f32;

        let mut graphics: *mut GpGraphics = ptr::null_mut();
        if GdipCreateFromHDC(hdc, &mut graphics) != 0 {
            DeleteObject(hfont as _);
            ReleaseDC(hwnd, hdc);
            return;
        }

        GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias);

        let transparent_brush = CreateSolidBrush(0);
        FillRect(hdc, &rect, transparent_brush);
        DeleteObject(transparent_brush as _);

        // 纯长矩形背景，无圆角
        let bg_color: u32 = ((settings.opacity as u32) << 24) | 0x00111111;
        let mut bg_brush: *mut GpSolidFill = ptr::null_mut();
        GdipCreateSolidFill(bg_color, &mut bg_brush);
        GdipFillRectangle(graphics, bg_brush as *mut GpBrush, 0.0, 0.0, win_width, win_height);
        GdipDeleteBrush(bg_brush as *mut GpBrush);

        let gap = (10.0 * dpi_scale) as i32;
        let mut current_x: i32 = padding;
        let win_height_i32 = win_height as i32;

        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                current_x += item_gap;
            }

            let old_font = SelectObject(hdc, hfont as _);
            SetBkMode(hdc, TRANSPARENT as i32);

            if !item.label.is_empty() {
                let wide_label: Vec<u16> = item.label.encode_utf16().chain(std::iter::once(0)).collect();
                let mut label_rect = RECT {
                    left: current_x,
                    top: 0,
                    right: current_x + item.label_width,
                    bottom: win_height_i32,
                };
                SetTextColor(hdc, 0x00AAAAAA);
                DrawTextW(
                    hdc,
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
            if !item.label.is_empty() && !item.value.contains("--") {
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

            SetTextColor(hdc, color);
            DrawTextW(
                hdc,
                wide_value.as_ptr(),
                (wide_value.len() - 1) as i32,
                &mut value_rect,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            SelectObject(hdc, old_font);
            current_x += item.total_width;
        }

        GdipDeleteGraphics(graphics);
        DeleteObject(hfont as _);
        ReleaseDC(hwnd, hdc);
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
                let settings = super::get_or_init_settings();
                let data = super::CURRENT_HARDWARE_DATA
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap_or_default();
                draw_overlay_content(hwnd, &settings, &data);
                EndPaint(hwnd, &ps);
                0
            }
            WM_TIMER => {
                let data = super::collect_hardware_data();
                *super::CURRENT_HARDWARE_DATA.lock().unwrap() = Some(data);
                InvalidateRect(hwnd, ptr::null(), 0);
                SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
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

    // crate::fps_tracker::start_fps_tracking();

    {
        let mut settings_lock = CURRENT_SETTINGS.lock().unwrap();
        *settings_lock = Some(settings.clone());
    }

    thread::spawn(move || {
        unsafe {
            match win32::create_overlay_window(&settings) {
                std::result::Result::Ok(hwnd) => {
                    OVERLAY_HANDLE.store(hwnd, Ordering::SeqCst);

                    let data = collect_hardware_data();
                    *CURRENT_HARDWARE_DATA.lock().unwrap() = Some(data);

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
    use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, PostQuitMessage, WM_CLOSE};

    if !OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        return Ok(OverlayResult {
            success: true,
            message: "悬浮框已处于关闭状态".to_string(),
        });
    }

    OVERLAY_ACTIVE.store(false, Ordering::SeqCst);

    // crate::fps_tracker::stop_fps_tracking();

    unsafe {
        let hwnd = OVERLAY_HANDLE.load(Ordering::SeqCst);
        if !hwnd.is_null() {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
            PostQuitMessage(0);
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
    let opacity = settings.opacity;
    {
        let mut settings_lock = CURRENT_SETTINGS.lock().unwrap();
        *settings_lock = Some(settings);
    }

    if OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        #[cfg(target_os = "windows")]
        unsafe {
            let hwnd = OVERLAY_HANDLE.load(Ordering::SeqCst);
            if !hwnd.is_null() {
                use windows_sys::Win32::UI::WindowsAndMessaging::{SetLayeredWindowAttributes, LWA_ALPHA};
                SetLayeredWindowAttributes(hwnd, 0, opacity, LWA_ALPHA);
            }
        }
    }

    Ok(OverlayResult {
        success: true,
        message: "设置已更新".to_string(),
    })
}

pub fn cleanup() {
    // crate::fps_tracker::cleanup();
    if OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        let _ = stop_overlay();
    }
    #[cfg(target_os = "windows")]
    unsafe {
        win32::shutdown_gdiplus();
    }
}
