//! 辅助窗口按需创建管理器。
//!
//! 背景：tauri.conf.json 里静态声明的 window 在启动时就会创建对应的 WebView2，
//! 即使 visible:false 也会派生渲染/GPU/utility 等进程，4 个窗口叠加导致
//! 运行时内存高达 ~1.1GB（9 个 WebView2 进程）。
//!
//! 优化：把辅助窗口（托盘菜单/桌面歌词/解锁按钮/竖排悬浮框）从静态配置移除，
//! 改为在真正需要显示时才用 WebviewWindowBuilder 动态创建；隐藏后由各自逻辑销毁
//! （或仅隐藏，重新 show 时复用）。这样启动时只有主窗口 → WebView2 进程减半，
//! 内存可降到 ~450MB。

use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// 确保托盘菜单窗口存在（不存在则创建），返回窗口
pub fn ensure_tray_menu<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    if let Some(w) = app.get_webview_window("tray-menu") {
        return Some(w);
    }
    WebviewWindowBuilder::new(
        app,
        "tray-menu",
        WebviewUrl::App("/tray-menu".into()),
    )
    .title("NexBox Tray Menu")
    .inner_size(190.0, 140.0)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .maximizable(false)
    .skip_taskbar(true)
    .shadow(false)
    .build()
    .ok()
}

/// 确保桌面歌词窗口存在（不存在则创建），返回窗口
pub fn ensure_desktop_lyrics<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    if let Some(w) = app.get_webview_window("desktop-lyrics") {
        return Some(w);
    }
    WebviewWindowBuilder::new(
        app,
        "desktop-lyrics",
        WebviewUrl::App("/desktop-lyrics".into()),
    )
    .title("NexBox Desktop Lyrics")
    .inner_size(800.0, 200.0)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .maximizable(false)
    .skip_taskbar(true)
    .shadow(false)
    .build()
    .ok()
}

/// 确保歌词解锁按钮窗口存在，返回窗口
pub fn ensure_lyrics_unlock_btn<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    if let Some(w) = app.get_webview_window("lyrics-unlock-btn") {
        return Some(w);
    }
    WebviewWindowBuilder::new(
        app,
        "lyrics-unlock-btn",
        WebviewUrl::App("/lyrics-unlock-btn".into()),
    )
    .title("Lyrics Unlock")
    .inner_size(48.0, 48.0)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .maximizable(false)
    .skip_taskbar(true)
    .shadow(false)
    .build()
    .ok()
}

// 竖排悬浮框窗口：使用 lib.rs 中已有的 ensure_vertical_overlay（带正确尺寸）。
