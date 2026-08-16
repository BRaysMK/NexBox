#[cfg(windows)]
use serde::Serialize;

#[cfg(windows)]
#[derive(Serialize)]
pub struct CursorPosition {
    pub x: i32,
    pub y: i32,
}

/// 获取全局鼠标光标位置 (物理屏幕坐标)
#[cfg(windows)]
#[tauri::command]
pub fn get_cursor_position() -> Result<CursorPosition, String> {
    use winapi::um::winuser::GetCursorPos;
    use winapi::shared::windef::POINT;

    let mut point = POINT { x: 0, y: 0 };
    let result = unsafe { GetCursorPos(&mut point) };
    if result == 0 {
        return Err("GetCursorPos failed".to_string());
    }
    Ok(CursorPosition {
        x: point.x,
        y: point.y,
    })
}

#[cfg(not(windows))]
#[tauri::command]
pub fn get_cursor_position() -> Result<serde_json::Value, String> {
    Err("get_cursor_position is only supported on Windows".to_string())
}

// ═══════════════════════════════════════════════════════════════
// 桌面歌词窗口鼠标穿透
// ═══════════════════════════════════════════════════════════════
//
// 问题：
//   Tauri v2 的 setIgnoreCursorEvents(true) 只在父 Win32 窗口上设置
//   WS_EX_TRANSPARENT，但 WebView2 是子窗口，不继承该标志，导致鼠标
//   事件仍被子窗口捕获，穿透无效。
//
//   直接对子窗口设置 WS_EX_TRANSPARENT 会干扰 WebView2 的 DirectComposition
//   渲染，导致内容变透明。
//
//   Hook WM_NCHITTEST 返回 HTTRANSPARENT 对 WebView2 也无效，因为
//   WebView2 使用 DirectComposition 处理输入，绕过传统 Win32 消息路由。
//
// 方案（EnableWindow + WS_EX_TRANSPARENT 组合）：
//   1. 父窗口：设置 WS_EX_TRANSPARENT（使父窗口对鼠标透明）
//   2. 子窗口：EnableWindow(FALSE) 禁用输入
//      - MSDN: "If a child window is disabled, the system skips it
//        when determining which window should receive mouse messages."
//      - 系统在鼠标命中测试时跳过禁用的子窗口
//      - 鼠标事件传递到父窗口，父窗口的 WS_EX_TRANSPARENT 使事件
//        继续传递到下方的应用窗口
//      - EnableWindow 不影响 WebView2 渲染

#[cfg(windows)]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(windows)]
/// 全局穿透开关
static CLICK_THROUGH: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
/// 属性名，用于在子窗口上存储原始窗口过程指针
const PROP_ORIG_PROC: &[u16] = &[
    b'n' as u16, b'x' as u16, b'_' as u16, b'o' as u16, b'r' as u16, b'i' as u16,
    b'g' as u16, b'_' as u16, b'p' as u16, b'r' as u16, b'o' as u16, b'c' as u16, 0,
];

/// 将窗口矩形钳制到所在（最近的）显示器工作区内，保证窗口完全可见
/// （rcWork 已排除任务栏，用于拖动时的"碰撞体"）
#[cfg(windows)]
unsafe fn clamp_rect_to_work_area(rect: &mut windows_sys::Win32::Foundation::RECT) {
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromRect, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };

    let w = rect.right - rect.left;
    let h = rect.bottom - rect.top;
    let monitor = MonitorFromRect(rect, MONITOR_DEFAULTTONEAREST);
    let mut info: MONITORINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(monitor, &mut info) == 0 {
        return;
    }
    let wa = info.rcWork;
    let mut left = rect.left;
    let mut top = rect.top;
    if left < wa.left {
        left = wa.left;
    }
    if left + w > wa.right {
        left = wa.right - w;
    }
    if top < wa.top {
        top = wa.top;
    }
    if top + h > wa.bottom {
        top = wa.bottom - h;
    }
    // 兜底：窗口比工作区还大时，保证左上角不跑到工作区外
    if left < wa.left {
        left = wa.left;
    }
    if top < wa.top {
        top = wa.top;
    }
    rect.left = left;
    rect.top = top;
    rect.right = left + w;
    rect.bottom = top + h;
}

/// 桌面歌词窗口的 hook 窗口过程（父窗口 + 子窗口共用）
///
/// 父窗口（本模块安装）：拦截 WM_MOVING，拖动时把窗口矩形钳制在
/// 工作区内 —— 这是真正的"碰撞体"，由系统拖动循环直接使用修正后的
/// 矩形，不会闪烁、不会越界。
///
/// 子窗口（鼠标穿透时安装）：WM_NCHITTEST 返回 HTTRANSPARENT，
/// 与 EnableWindow 双保险实现穿透。
#[cfg(windows)]
unsafe extern "system" fn hook_wndproc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, DefWindowProcW, GetPropW, HTTRANSPARENT, WM_MOVING, WM_NCHITTEST,
        WNDPROC,
    };

    // 穿透模式下，WM_NCHITTEST 返回 HTTRANSPARENT
    if msg == WM_NCHITTEST && CLICK_THROUGH.load(Ordering::Relaxed) {
        return HTTRANSPARENT as isize;
    }

    // 拖动中拦截 WM_MOVING：钳制窗口矩形，任何部分都不允许越出工作区
    if msg == WM_MOVING {
        let rect = lparam as *mut RECT;
        if !rect.is_null() {
            clamp_rect_to_work_area(&mut *rect);
        }
        return 0; // 系统会使用修正后的矩形继续移动
    }

    // 其他消息：调用原始窗口过程
    let original = GetPropW(hwnd, PROP_ORIG_PROC.as_ptr());
    if !original.is_null() {
        let proc_fn: WNDPROC = std::mem::transmute(original as isize);
        return CallWindowProcW(proc_fn, hwnd, msg, wparam, lparam);
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// 给桌面歌词窗口安装 WM_MOVING 拦截（幂等）。
/// 启动时调用一次即可，此后用户拖动窗口时系统会实时钳制其位置，
/// 保证窗口永远不越出屏幕外、不压住任务栏。
#[cfg(windows)]
pub fn install_lyrics_move_clamp(app_handle: &tauri::AppHandle) -> Result<(), String> {
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetPropW, GetWindowLongPtrW, SetPropW, SetWindowLongPtrW, GWLP_WNDPROC,
    };

    let window = crate::window_manager::ensure_desktop_lyrics(&app_handle)
        .ok_or_else(|| "desktop-lyrics window create failed".to_string())?;
    let hwnd = window
        .hwnd()
        .map_err(|e| format!("Failed to get lyrics window HWND: {}", e))?;
    let hwnd_raw = hwnd.0 as windows_sys::Win32::Foundation::HWND;

    unsafe {
        // 幂等：已安装过则跳过
        if !GetPropW(hwnd_raw, PROP_ORIG_PROC.as_ptr()).is_null() {
            return Ok(());
        }
        let current = GetWindowLongPtrW(hwnd_raw, GWLP_WNDPROC);
        if current == 0 {
            return Err("Failed to get original window proc".to_string());
        }
        SetPropW(hwnd_raw, PROP_ORIG_PROC.as_ptr(), current as HANDLE);
        SetWindowLongPtrW(
            hwnd_raw,
            GWLP_WNDPROC,
            hook_wndproc as *const () as usize as isize,
        );
    }

    Ok(())
}

#[cfg(not(windows))]
pub fn install_lyrics_move_clamp(_app_handle: &tauri::AppHandle) -> Result<(), String> {
    Ok(())
}

/// EnumChildWindows 回调：为每个子窗口安装 hook + 禁用输入
#[cfg(windows)]
unsafe extern "system" fn enum_child_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    lparam: isize,
) -> i32 {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetPropW, GetWindowLongPtrW, SetPropW, SetWindowLongPtrW, GWLP_WNDPROC,
    };
    use windows_sys::Win32::Foundation::HANDLE;
    // EnableWindow 在 winapi crate 中
    use winapi::um::winuser::EnableWindow;

    let enable = lparam != 0; // lparam=1 → 启用, lparam=0 → 禁用

    // 安装 hook（幂等，仅首次安装）
    if !enable {
        let existing = GetPropW(hwnd, PROP_ORIG_PROC.as_ptr());
        if existing.is_null() {
            let current = GetWindowLongPtrW(hwnd, GWLP_WNDPROC);
            if current != 0 {
                SetPropW(hwnd, PROP_ORIG_PROC.as_ptr(), current as HANDLE);
                SetWindowLongPtrW(
                    hwnd,
                    GWLP_WNDPROC,
                    hook_wndproc as *const () as usize as isize,
                );
            }
        }
    }

    // 禁用/启用子窗口输入
    // 禁用的子窗口在鼠标命中测试时被系统跳过，事件传递到父窗口
    // 父窗口的 WS_EX_TRANSPARENT 使事件继续传递到下方的应用窗口
    EnableWindow(hwnd as winapi::shared::windef::HWND, if enable { 1 } else { 0 });
    1 // 继续枚举
}

#[cfg(windows)]
#[tauri::command]
pub fn set_desktop_lyrics_click_through(
    app_handle: tauri::AppHandle,
    ignore: bool,
) -> Result<bool, String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE,
        WS_EX_TRANSPARENT,
    };
    use windows_sys::Win32::Foundation::HWND;

    let window = crate::window_manager::ensure_desktop_lyrics(&app_handle)
        .ok_or_else(|| "desktop-lyrics window create failed".to_string())?;

    let hwnd = window
        .hwnd()
        .map_err(|e| format!("Failed to get HWND: {}", e))?;
    let hwnd_raw = hwnd.0 as HWND;

    // 更新全局穿透标志
    CLICK_THROUGH.store(ignore, Ordering::Relaxed);

    unsafe {
        // 1. 父窗口：设置/取消 WS_EX_TRANSPARENT
        let style = GetWindowLongPtrW(hwnd_raw, GWL_EXSTYLE);
        if ignore {
            SetWindowLongPtrW(hwnd_raw, GWL_EXSTYLE, style | WS_EX_TRANSPARENT as isize);
        } else {
            SetWindowLongPtrW(hwnd_raw, GWL_EXSTYLE, style & !(WS_EX_TRANSPARENT as isize));
        }

        // 2. 子窗口：禁用/启用输入 + 安装 hook（双保险）
        //    lparam=0 → 禁用(穿透), lparam=1 → 启用(正常)
        EnumChildWindows(hwnd_raw, Some(enum_child_proc), if ignore { 0 } else { 1 });
    }

    Ok(true)
}

#[cfg(not(windows))]
#[tauri::command]
pub fn set_desktop_lyrics_click_through(
    _app_handle: tauri::AppHandle,
    _ignore: bool,
) -> Result<bool, String> {
    Err("set_desktop_lyrics_click_through is only supported on Windows".to_string())
}

// ═══════════════════════════════════════════════════════════════
// 桌面歌词窗口边界约束
// ═══════════════════════════════════════════════════════════════
//
// 将歌词窗口完全钳制在所在显示器的工作区（排除任务栏）内，
// 防止被拖出屏幕外或压住任务栏。
// 通过 MonitorFromWindow 找到窗口所在（最近）的显示器，
// 再用 GetMonitorInfoW 取其 rcWork（工作区矩形），
// 最后把窗口左上角调整到窗口完全落在工作区内。

#[cfg(windows)]
#[tauri::command]
pub fn clamp_lyrics_window_position(app_handle: tauri::AppHandle) -> Result<bool, String> {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };

    let window = crate::window_manager::ensure_desktop_lyrics(&app_handle)
        .ok_or_else(|| "desktop-lyrics window create failed".to_string())?;

    let pos = window
        .outer_position()
        .map_err(|e| format!("Failed to get lyrics window position: {}", e))?;
    let size = window
        .outer_size()
        .map_err(|e| format!("Failed to get lyrics window size: {}", e))?;
    let hwnd = window
        .hwnd()
        .map_err(|e| format!("Failed to get lyrics window hwnd: {}", e))?;

    let mut clamped = false;

    unsafe {
        let monitor = MonitorFromWindow(hwnd.0 as HWND, MONITOR_DEFAULTTONEAREST);
        if !monitor.is_null() {
            let mut info: MONITORINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if GetMonitorInfoW(monitor, &mut info) != 0 {
                let wa = info.rcWork;
                let win_w = size.width as i32;
                let win_h = size.height as i32;

                let mut x = pos.x;
                let mut y = pos.y;

                if x < wa.left {
                    x = wa.left;
                }
                if x + win_w > wa.right {
                    x = wa.right - win_w;
                }
                if y < wa.top {
                    y = wa.top;
                }
                if y + win_h > wa.bottom {
                    y = wa.bottom - win_h;
                }

                // 兜底：若窗口比工作区还大，上面的减法可能把左上角推到
                // 工作区左侧/上侧之外，这里再钳制一次，保证窗口左上角可见。
                if x < wa.left {
                    x = wa.left;
                }
                if y < wa.top {
                    y = wa.top;
                }

                if x != pos.x || y != pos.y {
                    clamped = true;
                    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
                }
            }
        }
    }

    Ok(clamped)
}

#[cfg(not(windows))]
#[tauri::command]
pub fn clamp_lyrics_window_position(_app_handle: tauri::AppHandle) -> Result<bool, String> {
    Err("clamp_lyrics_window_position is only supported on Windows".to_string())
}

// ═══════════════════════════════════════════════════════════════
// 桌面歌词窗口复位（居中到屏幕中央）
// ═══════════════════════════════════════════════════════════════

#[cfg(windows)]
#[tauri::command]
pub fn center_lyrics_window(app_handle: tauri::AppHandle) -> Result<(), String> {

    let window = crate::window_manager::ensure_desktop_lyrics(&app_handle)
        .ok_or_else(|| "desktop-lyrics window create failed".to_string())?;
    window
        .center()
        .map_err(|e| format!("Failed to center lyrics window: {}", e))
}

#[cfg(not(windows))]
#[tauri::command]
pub fn center_lyrics_window(_app_handle: tauri::AppHandle) -> Result<(), String> {
    Err("center_lyrics_window is only supported on Windows".to_string())
}
