//! 竖排悬浮框模块 — 基于 Tauri Webview 窗口
//!
//! 与 Win32 GDI+ overlay 并存，当 style == "vertical_panel" 时使用此模块。

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use tauri::{Emitter, Manager};

use crate::overlay_panel::{OverlayResult, OverlaySettings, collect_hardware_data, CURRENT_HARDWARE_DATA, CURRENT_SETTINGS, get_or_init_settings};

static VERTICAL_OVERLAY_ACTIVE: AtomicBool = AtomicBool::new(false);
static DATA_THREAD_STARTED: AtomicBool = AtomicBool::new(false);

/// 启动竖排悬浮框
#[tauri::command]
pub async fn start_vertical_overlay(
    app_handle: tauri::AppHandle,
    settings: Option<OverlaySettings>,
) -> Result<OverlayResult, String> {
    if VERTICAL_OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        return Ok(OverlayResult {
            success: true,
            message: "竖排悬浮框已处于启用状态".to_string(),
        });
    }

    // 如果 Win32 overlay 正在运行，先停止
    if crate::overlay_panel::is_overlay_active() {
        crate::overlay_panel::stop_overlay()?;
        std::thread::sleep(Duration::from_millis(200));
    }

    // 保存设置
    if let Some(ref s) = settings {
        let mut settings_lock = CURRENT_SETTINGS.lock().unwrap();
        *settings_lock = Some(s.clone());
    }

    let settings = get_or_init_settings();
    VERTICAL_OVERLAY_ACTIVE.store(true, Ordering::SeqCst);

    // 显示 Tauri 窗口
    let window = app_handle
        .get_webview_window("vertical-overlay")
        .ok_or("找不到 vertical-overlay 窗口")?;

    // 恢复保存的位置或使用默认位置（屏幕右上角）
    if let (Some(x), Some(y)) = (settings.position_x, settings.position_y) {
        let _ = window.set_position(tauri::PhysicalPosition { x, y });
    } else {
        // 默认：屏幕右上角
        if let Ok(monitor) = window.current_monitor() {
            if let Some(monitor) = monitor {
                let screen_size = monitor.size();
                let scale = monitor.scale_factor();
                let win_w = 220.0 * scale;
                let _win_h = 400.0 * scale;
                let x = (screen_size.width as f64 - win_w - 20.0 * scale) as i32;
                let y = (20.0 * scale) as i32;
                let _ = window.set_position(tauri::PhysicalPosition { x, y });
            }
        }
    }

    let _ = window.show();
    let _ = window.set_always_on_top(true);

    // 启动数据推送线程（如果尚未启动）
    if !DATA_THREAD_STARTED.swap(true, Ordering::SeqCst) {
        let handle_clone = app_handle.clone();
        thread::spawn(move || {
            while DATA_THREAD_STARTED.load(Ordering::SeqCst) {
                let data = collect_hardware_data();
                // 更新 CURRENT_HARDWARE_DATA 供硬件报告使用
                *CURRENT_HARDWARE_DATA.lock().unwrap() = Some(data.clone());

                // 推送给竖排悬浮框窗口
                if VERTICAL_OVERLAY_ACTIVE.load(Ordering::SeqCst) {
                    let _ = handle_clone.emit("vertical-overlay-data", &data);
                }

                thread::sleep(Duration::from_millis(1000));
            }
        });
    }

    // 推送当前设置到前端
    let _ = app_handle.emit("vertical-overlay-settings", &settings);

    let _ = app_handle.emit("overlay-status-changed", ());
    Ok(OverlayResult {
        success: true,
        message: "竖排悬浮框已启动".to_string(),
    })
}

/// 停止竖排悬浮框
#[tauri::command]
pub async fn stop_vertical_overlay(
    app_handle: tauri::AppHandle,
) -> Result<OverlayResult, String> {
    if !VERTICAL_OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        return Ok(OverlayResult {
            success: true,
            message: "竖排悬浮框已处于关闭状态".to_string(),
        });
    }

    VERTICAL_OVERLAY_ACTIVE.store(false, Ordering::SeqCst);

    if let Some(window) = app_handle.get_webview_window("vertical-overlay") {
        let _ = window.hide();
    }

    let _ = app_handle.emit("overlay-status-changed", ());
    Ok(OverlayResult {
        success: true,
        message: "竖排悬浮框已关闭".to_string(),
    })
}

/// 保存悬浮框位置
#[tauri::command]
pub async fn save_vertical_overlay_position(
    x: i32,
    y: i32,
) -> Result<OverlayResult, String> {
    let mut settings_lock = CURRENT_SETTINGS.lock().unwrap();
    if let Some(ref mut settings) = *settings_lock {
        settings.position_x = Some(x);
        settings.position_y = Some(y);
    }
    Ok(OverlayResult {
        success: true,
        message: "位置已保存".to_string(),
    })
}

/// 设置鼠标穿透
#[tauri::command]
pub async fn set_vertical_overlay_click_through(
    app_handle: tauri::AppHandle,
    enabled: bool,
) -> Result<OverlayResult, String> {
    if let Some(window) = app_handle.get_webview_window("vertical-overlay") {
        let _ = window.set_ignore_cursor_events(enabled);
    }
    Ok(OverlayResult {
        success: true,
        message: if enabled {
            "已开启鼠标穿透".to_string()
        } else {
            "已关闭鼠标穿透".to_string()
        },
    })
}

/// 重置竖排悬浮框位置
#[tauri::command]
pub async fn reset_vertical_overlay_position(
    app_handle: tauri::AppHandle,
) -> Result<OverlayResult, String> {
    // 清除保存的位置
    {
        let mut settings_lock = CURRENT_SETTINGS.lock().unwrap();
        if let Some(ref mut settings) = *settings_lock {
            settings.position_x = None;
            settings.position_y = None;
        }
    }

    // 移动窗口到默认位置（右上角）
    if let Some(window) = app_handle.get_webview_window("vertical-overlay") {
        if let Ok(monitor) = window.current_monitor() {
            if let Some(monitor) = monitor {
                let screen_size = monitor.size();
                let scale = monitor.scale_factor();
                let win_w = 220.0 * scale;
                let x = (screen_size.width as f64 - win_w - 20.0 * scale) as i32;
                let y = (20.0 * scale) as i32;
                let _ = window.set_position(tauri::PhysicalPosition { x, y });
            }
        }
    }

    Ok(OverlayResult {
        success: true,
        message: "位置已重置为默认".to_string(),
    })
}

/// 调整竖排悬浮框窗口大小
#[tauri::command]
pub async fn resize_vertical_overlay(
    app_handle: tauri::AppHandle,
    height: u32,
) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("vertical-overlay") {
        let scale = window
            .scale_factor()
            .unwrap_or(1.0);
        let _ = window.set_size(tauri::LogicalSize {
            width: 220.0,
            height: height as f64 / scale,
        });
    }
    Ok(())
}

/// 切换竖排悬浮框开关（供快捷键调用）
pub fn toggle_vertical_overlay(app_handle: &tauri::AppHandle) -> Result<OverlayResult, String> {
    if VERTICAL_OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        // 使用 blocking 方式调用 async 命令
        let handle = app_handle.clone();
        tauri::async_runtime::block_on(async move {
            stop_vertical_overlay(handle).await
        })
    } else {
        let handle = app_handle.clone();
        tauri::async_runtime::block_on(async move {
            start_vertical_overlay(handle, None).await
        })
    }
}

/// 查询竖排悬浮框是否活跃
pub fn is_vertical_overlay_active() -> bool {
    VERTICAL_OVERLAY_ACTIVE.load(Ordering::SeqCst)
}

/// 停止数据推送线程
pub fn stop_data_thread() {
    DATA_THREAD_STARTED.store(false, Ordering::SeqCst);
}

/// 清理（应用退出时调用）
pub fn cleanup(app_handle: &tauri::AppHandle) {
    if VERTICAL_OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        VERTICAL_OVERLAY_ACTIVE.store(false, Ordering::SeqCst);
        if let Some(window) = app_handle.get_webview_window("vertical-overlay") {
            let _ = window.hide();
        }
    }
    stop_data_thread();
}
