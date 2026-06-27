use std::sync::Mutex;
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicU32, AtomicBool, Ordering};
use std::os::windows::ffi::OsStrExt;
use sysinfo::{Networks, System};
use tauri::{State, Manager, Emitter};

static LAST_NOTIFICATION_ID: AtomicU32 = AtomicU32::new(0);
static IS_NOTIF_INIT: AtomicBool = AtomicBool::new(false);

struct MusicInfo {
    title: String,
}

#[derive(serde::Serialize, Clone)]
pub struct ToastData {
    pub app_name: String,
    pub title: String,
    pub body: String,
    pub aumid: String,
}

unsafe extern "system" fn enum_windows_proc(
    hwnd: winapi::shared::windef::HWND,
    lparam: winapi::shared::minwindef::LPARAM,
) -> winapi::shared::minwindef::BOOL {
    let mut class_name = [0u16; 256];
    winapi::um::winuser::GetClassNameW(hwnd, class_name.as_mut_ptr(), class_name.len() as i32);
    let class_str = String::from_utf16_lossy(&class_name);

    let is_netease = class_str.contains("Orpheus") || class_str.contains("CloudMusic");
    if !is_netease {
        return 1; // Only check Netease windows
    }

    // Don't skip invisible windows — Netease may be in system tray
    let mut title = [0u16; 512];
    winapi::um::winuser::GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
    let title_str = String::from_utf16_lossy(&title);
    let clean_title = title_str.trim_matches('\0').trim().to_string();

    if !clean_title.is_empty() && clean_title != "网易云音乐" && clean_title != "DesktopLyric" {
        let info = &mut *(lparam as *mut MusicInfo);
        info.title = clean_title;
        return 0;
    }
    1
}

#[tauri::command]
pub async fn get_random_cover_url(song_name: String, artist_name: String) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let query = format!("{} {}", song_name, artist_name);
    let encoded_query = urlencoding::encode(&query);
    let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36";

    // Source 1: Netease Cloud Music search
    let netease_search_url = "https://music.163.com/api/search/get/web";
    if let Ok(resp) = client
        .post(netease_search_url)
        .header("Referer", "https://music.163.com")
        .header("User-Agent", ua)
        .form(&[
            ("s", query.as_str()),
            ("type", "1"),
            ("limit", "1"),
            ("offset", "0"),
        ])
        .send()
        .await
    {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(pic) = json
                .pointer("/result/songs/0/al/picUrl")
                .and_then(|v| v.as_str())
            {
                if !pic.is_empty() && pic != "http://p4.music.126.net/UeTuwE7pvjBpypWLudqukQ==/3135032972947607.jpg"
                {
                    return Ok(pic.replace("http://", "https://") + "?param=300y300");
                }
            }
        }
    }

    // Source 2: Deezer API
    let deezer_url = format!(
        "https://api.deezer.com/search?q=track:\"{}\" artist:\"{}\"&limit=1",
        urlencoding::encode(&song_name),
        urlencoding::encode(&artist_name)
    );
    if let Ok(resp) = client.get(&deezer_url).header("User-Agent", ua).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(cover) = json
                .pointer("/data/0/album/cover_medium")
                .and_then(|v| v.as_str())
            {
                if !cover.is_empty() {
                    return Ok(cover.to_string());
                }
            }
            if let Some(cover) = json
                .pointer("/data/0/album/cover_big")
                .and_then(|v| v.as_str())
            {
                if !cover.is_empty() {
                    return Ok(cover.to_string());
                }
            }
        }
    }

    // Source 3: iTunes Search API
    let itunes_url = format!(
        "https://itunes.apple.com/search?term={}&media=music&limit=1",
        encoded_query
    );
    if let Ok(resp) = client.get(&itunes_url).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(artwork) = json
                .pointer("/results/0/artworkUrl100")
                .and_then(|v| v.as_str())
            {
                return Ok(artwork.replace("100x100bb", "300x300bb"));
            }
        }
    }

    // Fallback: inline SVG gradient
    Ok("data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMTUwIiBoZWlnaHQ9IjE1MCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48ZGVmcz48bGluZWFyR3JhZGllbnQgaWQ9ImciIHgxPSIwJSIgeTE9IjAlIiB4Mj0iMTAwJSIgeTI9IjEwMCUiPjxzdG9wIG9mZnNldD0iMCUiIHN0b3AtY29sb3I9IiNhOGVkZWEiLz48c3RvcCBvZmZzZXQ9IjEwMCUiIHN0b3AtY29sb3I9IiNmZWQ2ZTMiLz48L2xpbmVhckdyYWRpZW50PjwvZGVmcz48cmVjdCB3aWR0aD0iMTUwIiBoZWlnaHQ9IjE1MCIgcng9Ijc1IiBmaWxsPSJ1cmwoI2cpIi8+PC9zdmc+".to_string())
}

#[tauri::command]
pub fn fetch_netease_music_info() -> Result<Option<(String, String, bool)>, String> {
    let mut music_info = MusicInfo {
        title: String::new(),
    };

    unsafe {
        winapi::um::winuser::EnumWindows(
            Some(enum_windows_proc),
            &mut music_info as *mut _ as winapi::shared::minwindef::LPARAM,
        );
    }

    if music_info.title.is_empty() {
        return Ok(None);
    }

    let parts: Vec<&str> = music_info.title.splitn(2, " - ").collect();
    if parts.len() == 2 {
        let song_name = parts[0].trim().to_string();
        let artist_name = parts[1].trim().to_string();
        Ok(Some((song_name, artist_name, true)))
    } else {
        Ok(Some((music_info.title, "未知歌手".to_string(), true)))
    }
}

#[tauri::command]
pub fn control_system_media(action: String) {
    use winapi::um::winuser::{
        keybd_event, VK_MEDIA_NEXT_TRACK, VK_MEDIA_PLAY_PAUSE, VK_MEDIA_PREV_TRACK,
    };
    unsafe {
        let vk = match action.as_str() {
            "play_pause" => VK_MEDIA_PLAY_PAUSE,
            "next" => VK_MEDIA_NEXT_TRACK,
            "prev" => VK_MEDIA_PREV_TRACK,
            _ => return,
        };
        keybd_event(vk as u8, 0, 0, 0);
        keybd_event(vk as u8, 0, 2, 0);
    }
}

#[tauri::command]
pub async fn fetch_latest_notification() -> Result<Option<ToastData>, String> {
    use windows::UI::Notifications::Management::UserNotificationListener;
    use windows::UI::Notifications::NotificationKinds;

    let listener = match UserNotificationListener::Current() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };

    let _ = listener.RequestAccessAsync();

    let notifications = match listener.GetNotificationsAsync(NotificationKinds::Toast) {
        Ok(op) => match op.get() {
            Ok(ns) => ns,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };

    let mut latest_notif = None;
    let mut max_id = 0u32;

    for notif in notifications {
        if let Ok(id) = notif.Id() {
            if id > max_id {
                max_id = id;
                latest_notif = Some(notif);
            }
        }
    }

    if max_id == 0 {
        return Ok(None);
    }

    let last_processed_id = LAST_NOTIFICATION_ID.load(Ordering::SeqCst);

    if !IS_NOTIF_INIT.load(Ordering::SeqCst) {
        LAST_NOTIFICATION_ID.store(max_id, Ordering::SeqCst);
        IS_NOTIF_INIT.store(true, Ordering::SeqCst);
        return Ok(None);
    }

    if max_id > last_processed_id {
        LAST_NOTIFICATION_ID.store(max_id, Ordering::SeqCst);

        if let Some(notif) = latest_notif {
            let app_name = notif.AppInfo()
                .and_then(|info| info.DisplayInfo())
                .and_then(|dinfo| dinfo.DisplayName())
                .map(|name| name.to_string())
                .unwrap_or_else(|_| "系统通知".to_string());

            let aumid = notif.AppInfo()
                .and_then(|info| info.AppUserModelId())
                .map(|id| id.to_string())
                .unwrap_or_default();

            if let Ok(toast_binding) = notif
                .Notification()
                .and_then(|n| n.Visual())
                .and_then(|v| v.GetBinding(&windows::core::HSTRING::from("ToastGeneric")))
            {
                if let Ok(text_elements) = toast_binding.GetTextElements() {
                    let mut text_list = Vec::new();
                    for elem in text_elements {
                        if let Ok(text) = elem.Text() {
                            text_list.push(text.to_string());
                        }
                    }

                    if !text_list.is_empty() {
                        let title = text_list.first().cloned().unwrap_or_default();
                        let body = if text_list.len() > 1 {
                            text_list[1..].join(" ")
                        } else {
                            String::new()
                        };

                        if title.contains("微信")
                            || title.contains("WeChat")
                            || body.contains("微信")
                            || body.contains("WeChat")
                        {
                            return Ok(None);
                        }

                        return Ok(Some(ToastData {
                            app_name,
                            title,
                            body,
                            aumid,
                        }));
                    }
                }
            }
        }
    }

    Ok(None)
}

#[tauri::command]
pub fn open_app_by_aumid(aumid: String, app_name: String) {
    let app_lower = app_name.to_lowercase();

    unsafe {
        winapi::um::winuser::keybd_event(
            winapi::um::winuser::VK_MENU as u8,
            0,
            0,
            0,
        );
        winapi::um::winuser::keybd_event(
            winapi::um::winuser::VK_MENU as u8,
            0,
            winapi::um::winuser::KEYEVENTF_KEYUP,
            0,
        );
    }

    let execute_protocol = |protocol: &str| {
        unsafe {
            let op = std::ffi::OsStr::new("open")
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<u16>>();
            let file = std::ffi::OsStr::new(protocol)
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<u16>>();
            winapi::um::shellapi::ShellExecuteW(
                std::ptr::null_mut(),
                op.as_ptr(),
                file.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                winapi::um::winuser::SW_SHOWNORMAL,
            );
        }
    };

    if app_lower.contains("qq") {
        execute_protocol("tencent://message/");
    } else if app_lower.contains("微信") || app_lower.contains("wechat") {
        execute_protocol("weixin://");
    } else if app_lower.contains("钉钉") || app_lower.contains("dingtalk") {
        execute_protocol("dingtalk://");
    } else if !aumid.is_empty() {
        unsafe {
            let op = std::ffi::OsStr::new("open")
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<u16>>();
            let file = std::ffi::OsStr::new("explorer.exe")
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<u16>>();
            let params = std::ffi::OsStr::new(&format!("shell:AppsFolder\\{}", aumid))
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<u16>>();
            winapi::um::shellapi::ShellExecuteW(
                std::ptr::null_mut(),
                op.as_ptr(),
                file.as_ptr(),
                params.as_ptr(),
                std::ptr::null(),
                winapi::um::winuser::SW_SHOWNORMAL,
            );
        }
    }
}

#[tauri::command]
pub fn force_window_topmost(app: tauri::AppHandle) {
    #[cfg(target_os = "windows")]
    {
        unsafe {
            let fg_hwnd = winapi::um::winuser::GetForegroundWindow();
            if !fg_hwnd.is_null() {
                let mut class_name = [0u16; 256];
                let len = winapi::um::winuser::GetClassNameW(
                    fg_hwnd,
                    class_name.as_mut_ptr(),
                    class_name.len() as i32,
                );
                let class_str = String::from_utf16_lossy(&class_name[..len as usize]);

                if class_str == "#32768" {
                    return;
                }

                let mut rect: winapi::shared::windef::RECT = std::mem::zeroed();
                winapi::um::winuser::GetWindowRect(fg_hwnd, &mut rect);

                let monitor = winapi::um::winuser::MonitorFromWindow(
                    fg_hwnd,
                    winapi::um::winuser::MONITOR_DEFAULTTONEAREST,
                );
                let mut mi: winapi::um::winuser::MONITORINFO = std::mem::zeroed();
                mi.cbSize = std::mem::size_of::<winapi::um::winuser::MONITORINFO>() as u32;
                winapi::um::winuser::GetMonitorInfoW(monitor, &mut mi);

                if rect.left == mi.rcMonitor.left
                    && rect.top == mi.rcMonitor.top
                    && rect.right == mi.rcMonitor.right
                    && rect.bottom == mi.rcMonitor.bottom
                {
                    if class_str != "Progman" && class_str != "WorkerW" {
                        return;
                    }
                }
            }

            if let Some(win) = app.get_webview_window("widget") {
                if let Ok(hwnd) = win.hwnd() {
                    winapi::um::winuser::SetWindowPos(
                        hwnd.0 as _,
                        -1isize as _,
                        0,
                        0,
                        0,
                        0,
                        19,
                    );
                }
            }
        }
    }
}

pub struct AppState {
    pub networks: Mutex<Networks>,
    pub system: Mutex<System>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            networks: Mutex::new(Networks::new_with_refreshed_list()),
            system: Mutex::new(System::new_all()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[tauri::command]
pub fn get_hardware_stats(state: State<'_, AppState>) -> (f32, u64, u64) {
    let mut sys = state.system.lock().unwrap();
    sys.refresh_cpu_usage();
    sys.refresh_memory();
    (
        sys.global_cpu_info().cpu_usage(),
        sys.used_memory(),
        sys.total_memory(),
    )
}

#[tauri::command]
pub fn get_network_stats(state: State<'_, AppState>) -> (u64, u64) {
    let mut networks = state.networks.lock().unwrap();
    networks.refresh_list();

    let mut total_rx = 0u64;
    let mut total_tx = 0u64;

    for (_interface_name, data) in networks.iter() {
        total_rx += data.total_received();
        total_tx += data.total_transmitted();
    }

    (total_rx, total_tx)
}

#[tauri::command]
pub fn get_network_latency() -> Result<u128, String> {
    let addr: SocketAddr = "223.5.5.5:53".parse().unwrap();
    let timeout = Duration::from_millis(1500);

    let start = Instant::now();
    match TcpStream::connect_timeout(&addr, timeout) {
        Ok(_) => {
            let elapsed = start.elapsed().as_millis();
            Ok(elapsed)
        }
        Err(_) => Err("Timeout or disconnected".to_string()),
    }
}

#[tauri::command]
pub fn is_widget_visible(app: tauri::AppHandle) -> bool {
    match app.get_webview_window("widget") {
        Some(win) => win.is_visible().unwrap_or(false),
        None => false,
    }
}

pub fn toggle_island(app_handle: &tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app_handle.get_webview_window("widget") {
        if win.is_visible().unwrap_or(false) {
            // 隐藏：发送事件让前端执行退出动画
            let _ = app_handle.emit("control-island-visibility", serde_json::json!({ "show": false }));
        } else {
            // 显示：发送事件让前端先定位再显示
            let _ = app_handle.emit("control-island-visibility", serde_json::json!({ "show": true }));
        }
    }
    Ok(())
}
