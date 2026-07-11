//! 桌面歌词解锁按钮独立窗口管理
//!
//! 创建一个永不穿透的小窗口，固定叠在歌词窗口顶部中央。
//! 从根本上解决 WebView2 穿透状态下 mousemove 不可靠的问题。

use tauri::Manager;
use tauri::Emitter;

/// 显示解锁按钮窗口，定位到歌词窗口顶部中央，并强制置于最顶层
#[tauri::command]
pub fn show_lyrics_unlock_btn(app_handle: tauri::AppHandle) -> Result<(), String> {
    let lyrics_win = app_handle
        .get_webview_window("desktop-lyrics")
        .ok_or_else(|| "desktop-lyrics window not found".to_string())?;

    let btn_win = app_handle
        .get_webview_window("lyrics-unlock-btn")
        .ok_or_else(|| "lyrics-unlock-btn window not found".to_string())?;

    // 获取歌词窗口位置和大小
    let pos = lyrics_win
        .outer_position()
        .map_err(|e| format!("Failed to get lyrics window position: {}", e))?;
    let size = lyrics_win
        .outer_size()
        .map_err(|e| format!("Failed to get lyrics window size: {}", e))?;

    // 计算按钮位置：窗口顶部中央
    let btn_size = 48.0_f64;
    let x = pos.x as f64 + (size.width as f64 - btn_size) / 2.0;
    let y = pos.y as f64;

    // 设置按钮窗口位置和大小
    btn_win
        .set_position(tauri::LogicalPosition::new(x, y))
        .map_err(|e| format!("Failed to set button position: {}", e))?;
    btn_win
        .set_size(tauri::LogicalSize::new(btn_size, btn_size))
        .map_err(|e| format!("Failed to set button size: {}", e))?;

    // 显示按钮
    btn_win
        .show()
        .map_err(|e| format!("Failed to show button: {}", e))?;

    // 用 Win32 SetWindowPos(HWND_TOPMOST) 强制按钮到所有窗口最顶层
    #[cfg(windows)]
    force_topmost(&btn_win);

    Ok(())
}

#[cfg(windows)]
fn force_topmost(btn_win: &tauri::WebviewWindow) {
    use winapi::um::winuser::{SetWindowPos, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SWP_NOACTIVATE};
    use winapi::shared::windef::HWND;

    if let Ok(hwnd) = btn_win.hwnd() {
        unsafe {
            SetWindowPos(
                hwnd.0 as HWND,
                HWND_TOPMOST,
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
}

/// 隐藏解锁按钮窗口
#[tauri::command]
pub fn hide_lyrics_unlock_btn(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(btn_win) = app_handle.get_webview_window("lyrics-unlock-btn") {
        btn_win
            .hide()
            .map_err(|e| format!("Failed to hide button: {}", e))?;
    }
    Ok(())
}

/// 解锁按钮被点击时调用：关闭穿透、隐藏按钮、通知歌词窗口
#[tauri::command]
pub fn unlock_lyrics(app_handle: tauri::AppHandle) -> Result<(), String> {
    // 1. 隐藏按钮窗口
    if let Some(btn_win) = app_handle.get_webview_window("lyrics-unlock-btn") {
        let _ = btn_win.hide();
    }

    // 2. 关闭歌词窗口穿透
    if let Some(lyrics_win) = app_handle.get_webview_window("desktop-lyrics") {
        let _ = lyrics_win.set_ignore_cursor_events(false);
    }

    // 3. 通知歌词窗口前端更新状态
    let _ = app_handle.emit("lyrics:unlock-triggered", ());

    Ok(())
}
