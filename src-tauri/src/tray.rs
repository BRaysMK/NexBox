use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{
    tray::{TrayIcon, TrayIconBuilder},
    AppHandle, Emitter, Manager, Runtime, Window,
};

static TRAY_INITIALIZED: AtomicBool = AtomicBool::new(false);
static CLOSE_BEHAVIOR: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::from("ask")));
static DONT_ASK_AGAIN: AtomicBool = AtomicBool::new(false);

/// 获取指定屏幕坐标所在显示器的工作区（已排除任务栏），返回物理像素 (x, y, width, height)。
#[cfg(target_os = "windows")]
fn get_monitor_work_area(px: i32, py: i32) -> Option<(i32, i32, i32, i32)> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };

    unsafe {
        let point = POINT { x: px, y: py };
        let hmonitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        if hmonitor.is_null() {
            return None;
        }
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(hmonitor, &mut info) == 0 {
            return None;
        }
        Some((
            info.rcWork.left,
            info.rcWork.top,
            info.rcWork.right - info.rcWork.left,
            info.rcWork.bottom - info.rcWork.top,
        ))
    }
}

#[cfg(not(target_os = "windows"))]
fn get_monitor_work_area(_px: i32, _py: i32) -> Option<(i32, i32, i32, i32)> {
    None
}

pub fn init_tray<R: Runtime>(app: &AppHandle<R>) -> Result<TrayIcon<R>, Box<dyn std::error::Error>> {
    if TRAY_INITIALIZED.load(Ordering::SeqCst) {
        return Err("Tray already initialized".into());
    }

    let tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .on_tray_icon_event(|tray, event| {
            let app = tray.app_handle();
            match event {
                tauri::tray::TrayIconEvent::Click {
                    button: tauri::tray::MouseButton::Left,
                    ..
                } => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                        crate::emit_main_visibility(app, true);
                    }
                }
                tauri::tray::TrayIconEvent::Click {
                    button: tauri::tray::MouseButton::Right,
                    rect,
                    ..
                } => {
                    if let Some(menu_window) = app.get_webview_window("tray-menu") {
                        let (px, py) = match rect.position {
                            tauri::Position::Physical(p) => (p.x, p.y),
                            tauri::Position::Logical(p) => (p.x as i32, p.y as i32),
                        };
                        let (sw, _sh) = match rect.size {
                            tauri::Size::Physical(s) => (s.width as i32, s.height as i32),
                            tauri::Size::Logical(s) => (s.width as i32, s.height as i32),
                        };
                        // 使用窗口实际物理尺寸计算偏移，避免 DPI 缩放导致菜单侵入任务栏
                        let (mw, mh) = menu_window
                            .outer_size()
                            .ok()
                            .map(|s| (s.width as i32, s.height as i32))
                            .unwrap_or((190, 140));

                        // 默认：菜单底部对齐托盘图标顶部（任务栏在屏幕下方）
                        let mut x = px + sw / 2 - mw / 2;
                        let mut y = py - mh;

                        // 依据所在显示器的工作区(不含任务栏)钳制，确保菜单完整显示在任务栏上方
                        if let Some((wx, wy, ww, wh)) = get_monitor_work_area(px, py) {
                            if mw <= ww && mh <= wh {
                                x = x.clamp(wx, wx + ww - mw);
                                y = y.clamp(wy, wy + wh - mh);
                            } else {
                                // 工作区容纳不下时退回屏幕内
                                x = x.clamp(wx, wx + ww - mw.min(ww));
                                y = y.clamp(wy, wy + wh - mh.min(wh));
                            }
                        } else {
                            x = x.max(0);
                            y = y.max(0);
                        }

                        let _ = menu_window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                            x,
                            y,
                        }));
                        let _ = menu_window.set_always_on_top(true);
                        let _ = menu_window.show();
                        let _ = menu_window.set_focus();
                    }
                }
                _ => {}
            }
        })
        .build(app)?;

    TRAY_INITIALIZED.store(true, Ordering::SeqCst);

    Ok(tray)
}

#[tauri::command]
pub async fn minimize_to_tray<R: Runtime>(window: Window<R>) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())?;
    crate::emit_main_visibility(&window.app_handle(), false);
    Ok(())
}

#[tauri::command]
pub async fn show_window<R: Runtime>(window: Window<R>) -> Result<(), String> {
    let app = window.app_handle();
    window.show().map_err(|e| e.to_string())?;
    window.unminimize().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;

    let _ = window.set_focus();

    crate::emit_main_visibility(app, true);

    Ok(())
}

#[tauri::command]
pub fn get_close_behavior() -> String {
    CLOSE_BEHAVIOR.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_close_behavior(behavior: String) {
    if let Ok(mut b) = CLOSE_BEHAVIOR.lock() {
        *b = behavior;
    }
}

#[tauri::command]
pub fn get_dont_ask_again() -> bool {
    DONT_ASK_AGAIN.load(Ordering::SeqCst)
}

#[tauri::command]
pub fn set_dont_ask_again(value: bool) {
    DONT_ASK_AGAIN.store(value, Ordering::SeqCst);
}

#[tauri::command]
pub fn exit_app(app: tauri::AppHandle) {
    // 先隐藏所有窗口，避免退出时 WebView2 销毁后短暂露出原生标题栏
    for label in &["main", "tray-menu", "desktop-lyrics", "lyrics-unlock-btn", "vertical-overlay"] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.hide();
        }
    }
    // 给 Windows 消息队列一点时间处理隐藏操作
    std::thread::sleep(std::time::Duration::from_millis(50));
    app.exit(0);
}

#[tauri::command]
pub fn check_update_and_show(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    crate::emit_main_visibility(&app, true);
    let _ = app.emit("check-update", ());
}

pub fn cleanup() {
    TRAY_INITIALIZED.store(false, Ordering::SeqCst);
}
