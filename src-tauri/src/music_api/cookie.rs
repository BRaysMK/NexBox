use std::sync::Arc;
use std::collections::HashMap;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "music-cookies.json";

fn get_store(app: &AppHandle) -> Result<Arc<tauri_plugin_store::Store<tauri::Wry>>, String> {
    app.store(STORE_FILE).map_err(|e| format!("Failed to load cookie store: {e}"))
}

pub fn save_cookie(app: &AppHandle, provider: &str, cookie: &str) -> Result<(), String> {
    let store = get_store(app)?;
    store.set(format!("cookie_{provider}"), cookie);
    store.save().map_err(|e| format!("Failed to save cookie: {e}"))
}

pub fn load_cookie(app: &AppHandle, provider: &str) -> Result<String, String> {
    let store = get_store(app)?;
    Ok(store
        .get(format!("cookie_{provider}"))
        .map(|v| v.as_str().unwrap_or("").to_string())
        .unwrap_or_default())
}

pub fn clear_cookie(app: &AppHandle, provider: &str) -> Result<(), String> {
    let store = get_store(app)?;
    store.delete(format!("cookie_{provider}"));
    store.save().map_err(|e| format!("Failed to clear cookie: {e}"))
}

pub fn parse_cookie_string(cookie: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for part in cookie.split(';') {
        let part = part.trim();
        if let Some(eq) = part.find('=') {
            let key = part[..eq].trim().to_string();
            let value = part[eq + 1..].trim().to_string();
            if !key.is_empty() {
                map.insert(key, value);
            }
        }
    }
    map
}

pub fn normalize_cookie_header(raw: &str) -> String {
    let map = parse_cookie_string(raw);
    map.iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

/// 检查网易云 Cookie 是否包含登录态 (MUSIC_U)
pub fn netease_cookie_has_login(cookie: &str) -> bool {
    parse_cookie_string(cookie).contains_key("MUSIC_U")
}
