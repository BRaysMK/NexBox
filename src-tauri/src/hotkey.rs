use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

static OVERLAY_SHORTCUT: Mutex<Option<String>> = Mutex::new(None);
static OVERLAY_SHORTCUT_ID: AtomicU32 = AtomicU32::new(0);

static CROSSHAIR_SHORTCUT: Mutex<Option<String>> = Mutex::new(None);
static CROSSHAIR_SHORTCUT_ID: AtomicU32 = AtomicU32::new(0);

static FILTER_SHORTCUT: Mutex<Option<String>> = Mutex::new(None);
static FILTER_SHORTCUT_ID: AtomicU32 = AtomicU32::new(0);

static ISLAND_SHORTCUT: Mutex<Option<String>> = Mutex::new(None);
static ISLAND_SHORTCUT_ID: AtomicU32 = AtomicU32::new(0);

pub fn init_overlay(app_handle: &tauri::AppHandle, shortcut: &str) -> Result<(), String> {
    set_overlay_shortcut(shortcut);

    #[cfg(target_os = "windows")]
    {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        app_handle
            .global_shortcut()
            .register(shortcut)
            .map_err(|e| format!("注册悬浮框热键失败: {}", e))?;
    }

    log::info!("悬浮框热键已注册: {}", shortcut);
    Ok(())
}

pub fn update_overlay(app_handle: &tauri::AppHandle, new_shortcut: &str) -> Result<(), String> {
    let old_shortcut = get_overlay_shortcut();

    #[cfg(target_os = "windows")]
    {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;

        if !old_shortcut.is_empty() {
            let _ = app_handle.global_shortcut().unregister(old_shortcut.as_str());
        }

        if !new_shortcut.is_empty() {
            app_handle
                .global_shortcut()
                .register(new_shortcut)
                .map_err(|e| format!("注册悬浮框热键失败: {}", e))?;
        }
    }

    set_overlay_shortcut(new_shortcut);
    log::info!("悬浮框热键已更新: {} -> {}", old_shortcut, new_shortcut);
    Ok(())
}

pub fn get_overlay_shortcut() -> String {
    OVERLAY_SHORTCUT
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_default()
}

pub fn get_overlay_shortcut_id() -> u32 {
    OVERLAY_SHORTCUT_ID.load(Ordering::SeqCst)
}

fn set_overlay_shortcut(shortcut: &str) {
    *OVERLAY_SHORTCUT.lock().unwrap() = Some(shortcut.to_string());
    if let Ok(hotkey) = tauri_plugin_global_shortcut::Shortcut::from_str(shortcut) {
        OVERLAY_SHORTCUT_ID.store(hotkey.id(), Ordering::SeqCst);
    }
}

pub fn init_crosshair(app_handle: &tauri::AppHandle, shortcut: &str) -> Result<(), String> {
    set_crosshair_shortcut(shortcut);

    #[cfg(target_os = "windows")]
    {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        app_handle
            .global_shortcut()
            .register(shortcut)
            .map_err(|e| format!("注册准心热键失败: {}", e))?;
    }

    log::info!("准心热键已注册: {}", shortcut);
    Ok(())
}

pub fn update_crosshair(app_handle: &tauri::AppHandle, new_shortcut: &str) -> Result<(), String> {
    let old_shortcut = get_crosshair_shortcut();

    #[cfg(target_os = "windows")]
    {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;

        if !old_shortcut.is_empty() {
            let _ = app_handle.global_shortcut().unregister(old_shortcut.as_str());
        }

        if !new_shortcut.is_empty() {
            app_handle
                .global_shortcut()
                .register(new_shortcut)
                .map_err(|e| format!("注册准心热键失败: {}", e))?;
        }
    }

    set_crosshair_shortcut(new_shortcut);
    log::info!("准心热键已更新: {} -> {}", old_shortcut, new_shortcut);
    Ok(())
}

pub fn get_crosshair_shortcut() -> String {
    CROSSHAIR_SHORTCUT
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_default()
}

pub fn get_crosshair_shortcut_id() -> u32 {
    CROSSHAIR_SHORTCUT_ID.load(Ordering::SeqCst)
}

fn set_crosshair_shortcut(shortcut: &str) {
    *CROSSHAIR_SHORTCUT.lock().unwrap() = Some(shortcut.to_string());
    if let Ok(hotkey) = tauri_plugin_global_shortcut::Shortcut::from_str(shortcut) {
        CROSSHAIR_SHORTCUT_ID.store(hotkey.id(), Ordering::SeqCst);
    }
}

pub fn init_filter(app_handle: &tauri::AppHandle, shortcut: &str) -> Result<(), String> {
    set_filter_shortcut(shortcut);

    #[cfg(target_os = "windows")]
    {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        app_handle
            .global_shortcut()
            .register(shortcut)
            .map_err(|e| format!("注册滤镜热键失败: {}", e))?;
    }

    log::info!("滤镜热键已注册: {}", shortcut);
    Ok(())
}

pub fn update_filter(app_handle: &tauri::AppHandle, new_shortcut: &str) -> Result<(), String> {
    let old_shortcut = get_filter_shortcut();

    #[cfg(target_os = "windows")]
    {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;

        if !old_shortcut.is_empty() {
            let _ = app_handle.global_shortcut().unregister(old_shortcut.as_str());
        }

        if !new_shortcut.is_empty() {
            app_handle
                .global_shortcut()
                .register(new_shortcut)
                .map_err(|e| format!("注册滤镜热键失败: {}", e))?;
        }
    }

    set_filter_shortcut(new_shortcut);
    log::info!("滤镜热键已更新: {} -> {}", old_shortcut, new_shortcut);
    Ok(())
}

pub fn get_filter_shortcut() -> String {
    FILTER_SHORTCUT
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_default()
}

pub fn get_filter_shortcut_id() -> u32 {
    FILTER_SHORTCUT_ID.load(Ordering::SeqCst)
}

fn set_filter_shortcut(shortcut: &str) {
    *FILTER_SHORTCUT.lock().unwrap() = Some(shortcut.to_string());
    if let Ok(hotkey) = tauri_plugin_global_shortcut::Shortcut::from_str(shortcut) {
        FILTER_SHORTCUT_ID.store(hotkey.id(), Ordering::SeqCst);
    }
}

pub fn init_island(app_handle: &tauri::AppHandle, shortcut: &str) -> Result<(), String> {
    set_island_shortcut(shortcut);

    #[cfg(target_os = "windows")]
    {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        app_handle
            .global_shortcut()
            .register(shortcut)
            .map_err(|e| format!("注册灵动岛热键失败: {}", e))?;
    }

    log::info!("灵动岛热键已注册: {}", shortcut);
    Ok(())
}

pub fn update_island(app_handle: &tauri::AppHandle, new_shortcut: &str) -> Result<(), String> {
    let old_shortcut = get_island_shortcut();

    #[cfg(target_os = "windows")]
    {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;

        if !old_shortcut.is_empty() {
            let _ = app_handle.global_shortcut().unregister(old_shortcut.as_str());
        }

        if !new_shortcut.is_empty() {
            app_handle
                .global_shortcut()
                .register(new_shortcut)
                .map_err(|e| format!("注册灵动岛热键失败: {}", e))?;
        }
    }

    set_island_shortcut(new_shortcut);
    log::info!("灵动岛热键已更新: {} -> {}", old_shortcut, new_shortcut);
    Ok(())
}

pub fn get_island_shortcut() -> String {
    ISLAND_SHORTCUT
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_default()
}

pub fn get_island_shortcut_id() -> u32 {
    ISLAND_SHORTCUT_ID.load(Ordering::SeqCst)
}

fn set_island_shortcut(shortcut: &str) {
    *ISLAND_SHORTCUT.lock().unwrap() = Some(shortcut.to_string());
    if let Ok(hotkey) = tauri_plugin_global_shortcut::Shortcut::from_str(shortcut) {
        ISLAND_SHORTCUT_ID.store(hotkey.id(), Ordering::SeqCst);
    }
}

pub fn cleanup(app_handle: &tauri::AppHandle) {
    #[cfg(target_os = "windows")]
    {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;

        let overlay = get_overlay_shortcut();
        if !overlay.is_empty() {
            let _ = app_handle.global_shortcut().unregister(overlay.as_str());
        }

        let crosshair = get_crosshair_shortcut();
        if !crosshair.is_empty() {
            let _ = app_handle.global_shortcut().unregister(crosshair.as_str());
        }

        let filter = get_filter_shortcut();
        if !filter.is_empty() {
            let _ = app_handle.global_shortcut().unregister(filter.as_str());
        }

        let island = get_island_shortcut();
        if !island.is_empty() {
            let _ = app_handle.global_shortcut().unregister(island.as_str());
        }
    }
}

#[tauri::command]
pub fn get_overlay_hotkey() -> String {
    get_overlay_shortcut()
}

#[tauri::command]
pub fn set_overlay_hotkey(app_handle: tauri::AppHandle, shortcut: String) -> Result<(), String> {
    update_overlay(&app_handle, &shortcut)
}

#[tauri::command]
pub fn get_crosshair_hotkey() -> String {
    get_crosshair_shortcut()
}

#[tauri::command]
pub fn set_crosshair_hotkey(app_handle: tauri::AppHandle, shortcut: String) -> Result<(), String> {
    update_crosshair(&app_handle, &shortcut)
}

#[tauri::command]
pub fn get_filter_hotkey() -> String {
    get_filter_shortcut()
}

#[tauri::command]
pub fn set_filter_hotkey(app_handle: tauri::AppHandle, shortcut: String) -> Result<(), String> {
    update_filter(&app_handle, &shortcut)
}

#[tauri::command]
pub fn get_island_hotkey() -> String {
    get_island_shortcut()
}

#[tauri::command]
pub fn set_island_hotkey(app_handle: tauri::AppHandle, shortcut: String) -> Result<(), String> {
    update_island(&app_handle, &shortcut)
}
