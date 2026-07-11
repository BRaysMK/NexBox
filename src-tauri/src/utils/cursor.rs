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

/// 子窗口的 hook 窗口过程（备用方案，与 EnableWindow 双保险）
#[cfg(windows)]
unsafe extern "system" fn hook_wndproc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, DefWindowProcW, GetPropW, HTTRANSPARENT, WM_NCHITTEST, WNDPROC,
    };

    // 穿透模式下，WM_NCHITTEST 返回 HTTRANSPARENT
    if msg == WM_NCHITTEST && CLICK_THROUGH.load(Ordering::Relaxed) {
        return HTTRANSPARENT as isize;
    }

    // 其他消息：调用原始窗口过程
    let original = GetPropW(hwnd, PROP_ORIG_PROC.as_ptr());
    if !original.is_null() {
        let proc_fn: WNDPROC = std::mem::transmute(original as isize);
        return CallWindowProcW(proc_fn, hwnd, msg, wparam, lparam);
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
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
    use tauri::Manager;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE,
        WS_EX_TRANSPARENT,
    };
    use windows_sys::Win32::Foundation::HWND;

    let window = app_handle
        .get_webview_window("desktop-lyrics")
        .ok_or_else(|| "desktop-lyrics window not found".to_string())?;

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
