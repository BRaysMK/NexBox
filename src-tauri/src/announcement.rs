use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

const ANNOUNCEMENT_URL: &str = "https://gitee.com/muliuawa/nexbox/raw/master/notice.json";
const CACHE_FILE_NAME: &str = "announcement_cache.json";
const REQUEST_TIMEOUT_SECS: u64 = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Announcement {
    pub title: String,
    pub content: String,
    pub important: bool,
    pub create_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnnouncementResponse {
    pub version: u32,
    pub announce_list: Vec<Announcement>,
}

fn get_cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|p| p.join("NexBox").join(CACHE_FILE_NAME))
}

fn save_to_cache(data: &AnnouncementResponse) {
    if let Some(cache_path) = get_cache_path() {
        if let Some(parent) = cache_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(data) {
            let _ = fs::write(&cache_path, json);
        }
    }
}

fn load_from_cache() -> Option<AnnouncementResponse> {
    if let Some(cache_path) = get_cache_path() {
        if cache_path.exists() {
            if let Ok(content) = fs::read_to_string(&cache_path) {
                if let Ok(data) = serde_json::from_str::<AnnouncementResponse>(&content) {
                    return Some(data);
                }
            }
        }
    }
    None
}

pub async fn fetch_announcements() -> Result<AnnouncementResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(ANNOUNCEMENT_URL)
        .send()
        .await
        .map_err(|e| format!("Network request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()));
    }

    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    let data: AnnouncementResponse = serde_json::from_str(&text)
        .map_err(|e| format!("JSON parse error: {}", e))?;

    save_to_cache(&data);

    Ok(data)
}

#[tauri::command]
pub async fn get_announcements() -> AnnouncementResponse {
    match fetch_announcements().await {
        Ok(data) => data,
        Err(e) => {
            log::warn!("Failed to fetch announcements: {}, trying cache...", e);
            load_from_cache().unwrap_or_default()
        }
    }
}

#[tauri::command]
pub async fn get_important_announcements() -> Vec<Announcement> {
    let announcements = get_announcements().await;
    announcements
        .announce_list
        .into_iter()
        .filter(|a| a.important)
        .collect()
}
