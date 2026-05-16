use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

static CROSSHAIR_ACTIVE: AtomicBool = AtomicBool::new(false);
static CROSSHAIR_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CrosshairSettings {
    pub enabled: bool,
    pub style: String,
    pub size: i32,
    pub thickness: i32,
    pub color: String,
    pub gap: i32,
    pub dot_size: i32,
    pub opacity: u8,
}

impl Default for CrosshairSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            style: "Cross".to_string(),
            size: 20,
            thickness: 2,
            color: "#ff0000".to_string(),
            gap: 0,
            dot_size: 2,
            opacity: 255,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CrosshairResult {
    pub success: bool,
    pub message: String,
}

static CURRENT_SETTINGS: Mutex<Option<CrosshairSettings>> = Mutex::new(None);

fn get_settings() -> CrosshairSettings {
    let lock = CURRENT_SETTINGS.lock().unwrap();
    lock.as_ref().cloned().unwrap_or_default()
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
    use std::sync::atomic::Ordering;
    use std::sync::Mutex;
    use std::result::Result::Ok;

    static GDIPLUS_TOKEN: Mutex<Option<usize>> = Mutex::new(None);
    static WIN_EVENT_HOOK: Mutex<Option<usize>> = Mutex::new(None);

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
            log::error!("GDI+ init failed: {}", result);
            false
        }
    }

    pub unsafe fn shutdown_gdiplus() {
        let mut token = GDIPLUS_TOKEN.lock().unwrap();
        if let Some(t) = token.take() {
            GdiplusShutdown(t);
        }
    }

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
        let crosshair_hwnd = super::CROSSHAIR_HANDLE.load(Ordering::SeqCst);
        if crosshair_hwnd.is_null() {
            return;
        }
        if hwnd != crosshair_hwnd {
            SetWindowPos(
                crosshair_hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }

    pub unsafe fn install_topmost_guard() {
        let hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            ptr::null_mut(),
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

    fn parse_hex_color(hex: &str) -> (u8, u8, u8) {
        let hex = hex.trim_start_matches('#');
        if hex.len() < 6 {
            return (255, 0, 0);
        }
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        (r, g, b)
    }

    unsafe fn draw_style_cross(
        graphics: *mut GpGraphics,
        pen: *mut GpPen,
        center_x: f32,
        center_y: f32,
        gap: f32,
        size: f32,
    ) {
        if size <= gap {
            return;
        }
        GdipDrawLine(graphics, pen, center_x, center_y - gap - size, center_x, center_y - gap);
        GdipDrawLine(graphics, pen, center_x, center_y + gap, center_x, center_y + gap + size);
        GdipDrawLine(graphics, pen, center_x - gap - size, center_y, center_x - gap, center_y);
        GdipDrawLine(graphics, pen, center_x + gap, center_y, center_x + gap + size, center_y);
    }

    unsafe fn draw_style_circle(
        graphics: *mut GpGraphics,
        pen: *mut GpPen,
        center_x: f32,
        center_y: f32,
        size: f32,
    ) {
        GdipDrawEllipse(
            graphics,
            pen,
            center_x - size,
            center_y - size,
            size * 2.0,
            size * 2.0,
        );
    }

    unsafe fn draw_style_dot(
        graphics: *mut GpGraphics,
        brush: *mut GpBrush,
        center_x: f32,
        center_y: f32,
        dot_size: f32,
    ) {
        let r = dot_size / 2.0;
        GdipFillEllipse(graphics, brush, center_x - r, center_y - r, r * 2.0, r * 2.0);
    }

    unsafe fn draw_crosshair(
        graphics: *mut GpGraphics,
        settings: &super::CrosshairSettings,
        center_x: f32,
        center_y: f32,
    ) {
        let (r, g, b) = parse_hex_color(&settings.color);
        let argb: u32 =
            ((settings.opacity as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);

        let mut brush: *mut GpSolidFill = ptr::null_mut();
        GdipCreateSolidFill(argb, &mut brush);

        let mut pen: *mut GpPen = ptr::null_mut();
        GdipCreatePen1(argb, settings.thickness as f32, 2, &mut pen);

        GdipSetPenStartCap(pen, 2);
        GdipSetPenEndCap(pen, 2);

        let size = settings.size as f32;
        let gap = settings.gap as f32;
        let dot_size = settings.dot_size as f32;

        match settings.style.as_str() {
            "Cross" => {
                draw_style_cross(graphics, pen, center_x, center_y, gap, size);
            }
            "Dot" => {
                draw_style_dot(graphics, brush as *mut GpBrush, center_x, center_y, dot_size);
            }
            "Circle" => {
                draw_style_circle(graphics, pen, center_x, center_y, size);
            }
            "CrossDot" => {
                draw_style_cross(graphics, pen, center_x, center_y, gap, size);
                draw_style_dot(graphics, brush as *mut GpBrush, center_x, center_y, dot_size);
            }
            "CircleCross" => {
                draw_style_circle(graphics, pen, center_x, center_y, size);
                draw_style_cross(graphics, pen, center_x, center_y, gap, size);
            }
            _ => {
                draw_style_cross(graphics, pen, center_x, center_y, gap, size);
            }
        }

        if !pen.is_null() {
            GdipDeletePen(pen);
        }
        if !brush.is_null() {
            GdipDeleteBrush(brush as *mut GpBrush);
        }
    }

    unsafe fn render(hwnd: HWND, settings: &super::CrosshairSettings) {
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);

        let extent = settings.size + settings.gap + settings.thickness;
        let dib_size = ((extent * 2 + 16) as i32).max(64);

        let screen_dc = GetDC(ptr::null_mut());

        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = dib_size;
        bmi.bmiHeader.biHeight = -dib_size;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;

        let mut bits: *mut std::ffi::c_void = ptr::null_mut();
        let hbitmap = CreateDIBSection(
            screen_dc,
            &bmi,
            DIB_RGB_COLORS,
            &mut bits,
            ptr::null_mut(),
            0,
        );
        ReleaseDC(ptr::null_mut(), screen_dc);

        if hbitmap.is_null() {
            return;
        }

        let mem_dc = CreateCompatibleDC(ptr::null_mut());
        let old_bmp = SelectObject(mem_dc, hbitmap as HGDIOBJ);

        let mut graphics: *mut GpGraphics = ptr::null_mut();
        if GdipCreateFromHDC(mem_dc, &mut graphics) != 0 {
            SelectObject(mem_dc, old_bmp);
            DeleteObject(hbitmap as HGDIOBJ);
            DeleteDC(mem_dc);
            return;
        }

        GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias);

        let mut clear_brush: *mut GpSolidFill = ptr::null_mut();
        GdipCreateSolidFill(0x00000000, &mut clear_brush);
        GdipFillRectangle(
            graphics,
            clear_brush as *mut GpBrush,
            0.0,
            0.0,
            dib_size as f32,
            dib_size as f32,
        );
        GdipDeleteBrush(clear_brush as *mut GpBrush);

        let center_x = dib_size as f32 / 2.0;
        let center_y = dib_size as f32 / 2.0;
        draw_crosshair(graphics, settings, center_x, center_y);

        GdipDeleteGraphics(graphics);

        let win_x = (screen_width - dib_size) / 2;
        let win_y = (screen_height - dib_size) / 2;

        let ppt_dst = POINT { x: win_x, y: win_y };
        let psize = SIZE {
            cx: dib_size,
            cy: dib_size,
        };
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
    }

    pub unsafe fn create_window(settings: &super::CrosshairSettings) -> Result<HWND, String> {
        init_gdiplus();

        let h_instance = GetModuleHandleW(ptr::null());
        if h_instance.is_null() {
            return Err("Failed to get module handle".to_string());
        }

        let class_name = windows_sys::core::w!("NexBoxCrosshairOverlay");

        let wnd_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: h_instance,
            hIcon: ptr::null_mut(),
            hCursor: ptr::null_mut(),
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: class_name,
        };

        if RegisterClassW(&wnd_class) == 0 {
            let error = GetLastError();
            if error != 1410 {
                return Err(format!("RegisterClass failed: {}", error));
            }
        }

        let extent = settings.size + settings.gap + settings.thickness;
        let dib_size = ((extent * 2 + 16) as i32).max(64);

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST
                | WS_EX_LAYERED
                | WS_EX_TRANSPARENT
                | WS_EX_TOOLWINDOW
                | WS_EX_NOACTIVATE,
            class_name,
            windows_sys::core::w!("NexBox Crosshair"),
            WS_POPUP,
            0,
            0,
            dib_size,
            dib_size,
            ptr::null_mut(),
            ptr::null_mut(),
            h_instance,
            ptr::null_mut(),
        );

        if hwnd.is_null() {
            return Err("Failed to create window".to_string());
        }

        ShowWindow(hwnd, SW_SHOW);

        render(hwnd, settings);

        Ok(hwnd)
    }

    pub unsafe fn destroy_window(hwnd: HWND) -> bool {
        if hwnd.is_null() {
            return false;
        }
        KillTimer(hwnd, 1);
        DestroyWindow(hwnd) != 0
    }

    pub const WM_CROSSHAIR_REFRESH: u32 = 0x8001;

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
            WM_DISPLAYCHANGE => {
                let settings = super::get_settings();
                render(hwnd, &settings);
                0
            }
            WM_CROSSHAIR_REFRESH => {
                let settings = super::get_settings();
                render(hwnd, &settings);
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
pub fn start(settings: CrosshairSettings) -> Result<CrosshairResult, String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    if CROSSHAIR_ACTIVE.load(Ordering::SeqCst) {
        return Ok(CrosshairResult {
            success: true,
            message: "准心已处于启用状态".to_string(),
        });
    }

    CROSSHAIR_ACTIVE.store(true, Ordering::SeqCst);

    {
        let mut settings_lock = CURRENT_SETTINGS.lock().unwrap();
        *settings_lock = Some(settings.clone());
    }

    thread::spawn(move || unsafe {
        match win32::create_window(&settings) {
            Ok(hwnd) => {
                CROSSHAIR_HANDLE.store(hwnd, Ordering::SeqCst);

                SetTimer(hwnd, 1, 500, None);
                win32::install_topmost_guard();

                let mut msg: MSG = std::mem::zeroed();
                while CROSSHAIR_ACTIVE.load(Ordering::SeqCst) {
                    while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                        if msg.message == WM_QUIT {
                            break;
                        }
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }

                    if !CROSSHAIR_ACTIVE.load(Ordering::SeqCst) {
                        break;
                    }

                    thread::sleep(Duration::from_millis(50));
                }

                win32::uninstall_topmost_guard();
                win32::destroy_window(hwnd);
                CROSSHAIR_HANDLE.store(std::ptr::null_mut(), Ordering::SeqCst);
            }
            Err(e) => {
                log::error!("Failed to create crosshair window: {}", e);
                CROSSHAIR_ACTIVE.store(false, Ordering::SeqCst);
            }
        }
    });

    Ok(CrosshairResult {
        success: true,
        message: "准心已启动".to_string(),
    })
}

#[cfg(not(target_os = "windows"))]
pub fn start(_settings: CrosshairSettings) -> Result<CrosshairResult, String> {
    Err("此功能仅支持 Windows 系统".to_string())
}

#[cfg(target_os = "windows")]
pub fn stop() -> Result<CrosshairResult, String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, PostQuitMessage, WM_CLOSE};

    if !CROSSHAIR_ACTIVE.load(Ordering::SeqCst) {
        return Ok(CrosshairResult {
            success: true,
            message: "准心已处于关闭状态".to_string(),
        });
    }

    CROSSHAIR_ACTIVE.store(false, Ordering::SeqCst);

    unsafe {
        let hwnd = CROSSHAIR_HANDLE.load(Ordering::SeqCst);
        if !hwnd.is_null() {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
            PostQuitMessage(0);
        }
    }

    Ok(CrosshairResult {
        success: true,
        message: "准心已关闭".to_string(),
    })
}

#[cfg(not(target_os = "windows"))]
pub fn stop() -> Result<CrosshairResult, String> {
    Err("此功能仅支持 Windows 系统".to_string())
}

#[tauri::command]
pub async fn get_crosshair_status() -> Result<CrosshairSettings, String> {
    let mut settings = get_settings();
    settings.enabled = CROSSHAIR_ACTIVE.load(Ordering::SeqCst);
    Ok(settings)
}

#[tauri::command]
pub async fn toggle_crosshair() -> Result<CrosshairResult, String> {
    if CROSSHAIR_ACTIVE.load(Ordering::SeqCst) {
        stop()
    } else {
        let settings = get_settings();
        start(settings)
    }
}

#[tauri::command]
pub async fn update_crosshair_settings(
    settings: CrosshairSettings,
) -> Result<CrosshairResult, String> {
    let was_active = CROSSHAIR_ACTIVE.load(Ordering::SeqCst);

    {
        let mut settings_lock = CURRENT_SETTINGS.lock().unwrap();
        *settings_lock = Some(settings.clone());
    }

    if was_active {
        #[cfg(target_os = "windows")]
        {
            let hwnd = CROSSHAIR_HANDLE.load(Ordering::SeqCst);
            if !hwnd.is_null() {
                unsafe {
                    windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                        hwnd,
                        win32::WM_CROSSHAIR_REFRESH,
                        0,
                        0,
                    );
                }
                return Ok(CrosshairResult {
                    success: true,
                    message: "设置已更新".to_string(),
                });
            }
        }
    }

    if settings.enabled || was_active {
        let mut start_settings = settings;
        start_settings.enabled = true;
        start(start_settings)?;
    }

    Ok(CrosshairResult {
        success: true,
        message: "设置已更新".to_string(),
    })
}

pub fn cleanup() {
    if CROSSHAIR_ACTIVE.load(Ordering::SeqCst) {
        let _ = stop();
    }
    #[cfg(target_os = "windows")]
    unsafe {
        win32::shutdown_gdiplus();
    }
}