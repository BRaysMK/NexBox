use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

/// 全部热键总开关（默认开启）
static HOTKEYS_ENABLED: AtomicBool = AtomicBool::new(true);

pub fn set_hotkeys_enabled(enabled: bool) {
    HOTKEYS_ENABLED.store(enabled, Ordering::SeqCst);
}

pub fn is_hotkeys_enabled() -> bool {
    HOTKEYS_ENABLED.load(Ordering::SeqCst)
}

static OVERLAY_SHORTCUT: Mutex<Option<String>> = Mutex::new(None);
static OVERLAY_SHORTCUT_ID: AtomicU32 = AtomicU32::new(0);

static CROSSHAIR_SHORTCUT: Mutex<Option<String>> = Mutex::new(None);
static CROSSHAIR_SHORTCUT_ID: AtomicU32 = AtomicU32::new(0);

static FILTER_SHORTCUT: Mutex<Option<String>> = Mutex::new(None);
static FILTER_SHORTCUT_ID: AtomicU32 = AtomicU32::new(0);

static AUTOCLICKER_SHORTCUT: Mutex<Option<String>> = Mutex::new(None);
static AUTOCLICKER_SHORTCUT_ID: AtomicU32 = AtomicU32::new(0);

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

    // 新旧热键相同，无需更新
    if old_shortcut == new_shortcut {
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;

        if !old_shortcut.is_empty() {
            let _ = app_handle.global_shortcut().unregister(old_shortcut.as_str());
        }

        if !new_shortcut.is_empty() {
            if let Err(e) = app_handle.global_shortcut().register(new_shortcut) {
                // 注册失败：回滚旧热键，保证状态一致，避免热键静默失效
                if !old_shortcut.is_empty() {
                    let _ = app_handle.global_shortcut().register(old_shortcut.as_str());
                }
                return Err(format!("注册悬浮框热键失败: {}", e));
            }
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

    // 新旧热键相同，无需更新
    if old_shortcut == new_shortcut {
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;

        if !old_shortcut.is_empty() {
            let _ = app_handle.global_shortcut().unregister(old_shortcut.as_str());
        }

        if !new_shortcut.is_empty() {
            if let Err(e) = app_handle.global_shortcut().register(new_shortcut) {
                // 注册失败：回滚旧热键，保证状态一致
                if !old_shortcut.is_empty() {
                    let _ = app_handle.global_shortcut().register(old_shortcut.as_str());
                }
                return Err(format!("注册准心热键失败: {}", e));
            }
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

    // 新旧热键相同，无需更新
    if old_shortcut == new_shortcut {
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;

        if !old_shortcut.is_empty() {
            let _ = app_handle.global_shortcut().unregister(old_shortcut.as_str());
        }

        if !new_shortcut.is_empty() {
            if let Err(e) = app_handle.global_shortcut().register(new_shortcut) {
                // 注册失败：回滚旧热键，保证状态一致
                if !old_shortcut.is_empty() {
                    let _ = app_handle.global_shortcut().register(old_shortcut.as_str());
                }
                return Err(format!("注册滤镜热键失败: {}", e));
            }
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

/// 鼠标键热键（tauri 的 global-shortcut 不支持，需用低级鼠标钩子处理）
fn is_mouse_key(shortcut: &str) -> bool {
    shortcut.starts_with("Mouse")
}

pub fn init_autoclicker(app_handle: &tauri::AppHandle, shortcut: &str) -> Result<(), String> {
    set_autoclicker_shortcut(shortcut);

    #[cfg(target_os = "windows")]
    {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;

        if is_mouse_key(shortcut) {
            crate::autoclicker::set_mouse_hotkey(app_handle, Some(shortcut));
        } else {
            app_handle
                .global_shortcut()
                .register(shortcut)
                .map_err(|e| format!("注册连点器热键失败: {}", e))?;
        }
    }

    log::info!("连点器热键已注册: {}", shortcut);
    Ok(())
}

pub fn update_autoclicker(app_handle: &tauri::AppHandle, new_shortcut: &str) -> Result<(), String> {
    let old_shortcut = get_autoclicker_shortcut();

    // 新旧热键相同，无需更新
    if old_shortcut == new_shortcut {
        return Ok(());
    }

    let new_is_mouse = is_mouse_key(new_shortcut);

    #[cfg(target_os = "windows")]
    {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;

        // 旧热键若是键盘快捷键，先注销
        if !old_shortcut.is_empty() && !is_mouse_key(&old_shortcut) {
            let _ = app_handle.global_shortcut().unregister(old_shortcut.as_str());
        }

        // 鼠标键走低级钩子，键盘键走全局快捷键
        if new_is_mouse {
            crate::autoclicker::set_mouse_hotkey(app_handle, Some(new_shortcut));
        } else {
            crate::autoclicker::set_mouse_hotkey(app_handle, None);
            if !new_shortcut.is_empty() {
                if let Err(e) = app_handle.global_shortcut().register(new_shortcut) {
                    // 注册失败：回滚旧热键，保证状态一致
                    if !old_shortcut.is_empty() {
                        if is_mouse_key(&old_shortcut) {
                            crate::autoclicker::set_mouse_hotkey(app_handle, Some(&old_shortcut));
                        } else {
                            let _ = app_handle.global_shortcut().register(old_shortcut.as_str());
                        }
                    }
                    return Err(format!("注册连点器热键失败: {}", e));
                }
            }
        }
    }

    set_autoclicker_shortcut(new_shortcut);
    log::info!("连点器热键已更新: {} -> {}", old_shortcut, new_shortcut);
    Ok(())
}

pub fn get_autoclicker_shortcut() -> String {
    AUTOCLICKER_SHORTCUT
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_default()
}

pub fn get_autoclicker_shortcut_id() -> u32 {
    AUTOCLICKER_SHORTCUT_ID.load(Ordering::SeqCst)
}

fn set_autoclicker_shortcut(shortcut: &str) {
    *AUTOCLICKER_SHORTCUT.lock().unwrap() = Some(shortcut.to_string());
    if let Ok(hotkey) = tauri_plugin_global_shortcut::Shortcut::from_str(shortcut) {
        AUTOCLICKER_SHORTCUT_ID.store(hotkey.id(), Ordering::SeqCst);
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

        let autoclicker = get_autoclicker_shortcut();
        if !autoclicker.is_empty() && !is_mouse_key(&autoclicker) {
            let _ = app_handle.global_shortcut().unregister(autoclicker.as_str());
        }
    }
}

#[tauri::command]
pub fn get_overlay_hotkey() -> String {
    get_overlay_shortcut()
}

#[tauri::command]
pub fn set_overlay_hotkey(app_handle: tauri::AppHandle, shortcut: String) -> Result<(), String> {
    update_overlay(&app_handle, &shortcut)?;
    save_settings_value(
        &app_handle,
        "overlay-hotkey",
        serde_json::Value::String(shortcut),
    );
    Ok(())
}

#[tauri::command]
pub fn get_crosshair_hotkey() -> String {
    get_crosshair_shortcut()
}

#[tauri::command]
pub fn set_crosshair_hotkey(app_handle: tauri::AppHandle, shortcut: String) -> Result<(), String> {
    update_crosshair(&app_handle, &shortcut)?;
    save_settings_value(
        &app_handle,
        "crosshair-hotkey",
        serde_json::Value::String(shortcut),
    );
    Ok(())
}

#[tauri::command]
pub fn get_filter_hotkey() -> String {
    get_filter_shortcut()
}

#[tauri::command]
pub fn set_filter_hotkey(app_handle: tauri::AppHandle, shortcut: String) -> Result<(), String> {
    update_filter(&app_handle, &shortcut)?;
    save_settings_value(
        &app_handle,
        "filter-hotkey",
        serde_json::Value::String(shortcut),
    );
    Ok(())
}

#[tauri::command]
pub fn get_autoclicker_hotkey() -> String {
    get_autoclicker_shortcut()
}

#[tauri::command]
pub fn set_autoclicker_hotkey(app_handle: tauri::AppHandle, shortcut: String) -> Result<(), String> {
    update_autoclicker(&app_handle, &shortcut)?;
    save_settings_value(
        &app_handle,
        "autoclicker-hotkey",
        serde_json::Value::String(shortcut),
    );
    Ok(())
}

#[tauri::command]
pub fn set_hotkeys_enabled_cmd(app_handle: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    set_hotkeys_enabled(enabled);
    save_settings_value(
        &app_handle,
        "hotkeys-enabled",
        serde_json::Value::Bool(enabled),
    );
    log::info!(
        "全部热键总开关: {}",
        if enabled { "开启" } else { "关闭" }
    );
    Ok(())
}

#[tauri::command]
pub fn get_hotkeys_enabled_cmd() -> bool {
    is_hotkeys_enabled()
}

// ==================== 配置持久化 ====================

/// 串行化 settings.json 写入，避免多个热键并发保存时互相覆盖
static SETTINGS_WRITE_LOCK: Mutex<()> = Mutex::new(());

/// 将指定 key 写入 settings.json（保留文件中的其他 key，兼容前端 LazyStore）
fn save_settings_value(app: &tauri::AppHandle, key: &str, value: serde_json::Value) {
    #[cfg(target_os = "windows")]
    {
        use tauri::Manager;
        let _guard = SETTINGS_WRITE_LOCK.lock().unwrap();
        let Ok(dir) = app.path().app_data_dir() else {
            return;
        };
        let path = dir.join("settings.json");
        let mut json: serde_json::Value = std::fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_else(|| serde_json::json!({}));
        if let Some(obj) = json.as_object_mut() {
            obj.insert(key.to_string(), value);
        }
        if let Ok(content) = serde_json::to_string_pretty(&json) {
            let _ = std::fs::write(&path, content);
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, key, value);
    }
}

/// 从 settings.json（前端 LazyStore 写入）读取指定 key 的值
fn read_settings_value(app: &tauri::AppHandle, key: &str) -> Option<serde_json::Value> {
    #[cfg(target_os = "windows")]
    {
        use tauri::Manager;
        let dir = app.path().app_data_dir().ok()?;
        let path = dir.join("settings.json");
        let content = std::fs::read_to_string(path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;
        json.get(key).cloned()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, key);
        None
    }
}

/// 读取保存的快捷键，未保存或值无效时使用默认值
pub fn load_saved_hotkey(app: &tauri::AppHandle, key: &str, default: &str) -> String {
    match read_settings_value(app, key) {
        Some(serde_json::Value::String(s)) if !s.trim().is_empty() => s,
        _ => default.to_string(),
    }
}

/// 读取热键总开关，未保存时默认开启
pub fn load_saved_hotkeys_enabled(app: &tauri::AppHandle) -> bool {
    match read_settings_value(app, "hotkeys-enabled") {
        Some(serde_json::Value::Bool(b)) => b,
        _ => true,
    }
}


