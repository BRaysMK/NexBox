pub mod audio_proxy;
pub mod cookie;
pub mod crypto;
pub mod models;
pub mod netease;

use tauri::{AppHandle, Emitter, Manager};
use url::Url;
use models::*;

// ============================================================
//  Tauri Commands - 网易云
// ============================================================

#[tauri::command]
pub async fn music_search(keywords: String, limit: Option<u32>) -> Result<Vec<Song>, String> {
    let app_cookie = get_app_cookie().await;
    netease::search(&keywords, limit.unwrap_or(30), &app_cookie).await
}

#[tauri::command]
pub async fn music_song_url(id: String, quality: Option<String>) -> Result<SongUrlResult, String> {
    let app_cookie = get_app_cookie().await;
    netease::song_url(&id, &quality.unwrap_or_else(|| "hires".into()), &app_cookie).await
}

#[tauri::command]
pub async fn music_login_qr_key() -> Result<String, String> {
    let app_cookie = get_app_cookie().await;
    netease::login_qr_key(&app_cookie).await
}

#[tauri::command]
pub async fn music_login_qr_create(key: String) -> Result<String, String> {
    let app_cookie = get_app_cookie().await;
    netease::login_qr_create(&key, &app_cookie).await
}

#[tauri::command]
pub async fn music_login_qr_check(app: AppHandle, key: String) -> Result<QrCheckResult, String> {
    let app_cookie = get_app_cookie().await;
    let result = netease::login_qr_check(&key, &app_cookie).await?;

    // code 803 = 登录成功，自动保存 cookie
    if result.code == 803 {
        if let Some(ref cookie) = result.cookie {
            let normalized = cookie::normalize_cookie_header(cookie);
            if cookie::netease_cookie_has_login(&normalized) {
                let _ = cookie::save_cookie(&app, "netease", &normalized);
                set_app_cookie(normalized).await;
                log::info!("[MusicAPI] QR login successful, cookie saved");
            }
        }
    }

    Ok(result)
}

#[tauri::command]
pub async fn music_login_status(app: AppHandle) -> Result<LoginInfo, String> {
    let app_cookie = load_app_cookie(&app).await;
    netease::login_status(&app_cookie).await
}

#[tauri::command]
pub async fn music_login_cookie(app: AppHandle, cookie: String) -> Result<LoginInfo, String> {
    let normalized = cookie::normalize_cookie_header(&cookie);
    if !cookie::netease_cookie_has_login(&normalized) {
        return Ok(LoginInfo {
            provider: "netease".into(),
            ..Default::default()
        });
    }
    cookie::save_cookie(&app, "netease", &normalized)?;
    set_app_cookie(normalized).await;
    netease::login_status(get_app_cookie().await.as_str()).await
}

#[tauri::command]
pub async fn music_logout(app: AppHandle) -> Result<(), String> {
    cookie::clear_cookie(&app, "netease")?;
    set_app_cookie(String::new()).await;
    Ok(())
}

#[tauri::command]
pub async fn music_user_playlist(app: AppHandle) -> Result<Vec<Playlist>, String> {
    let app_cookie = load_app_cookie(&app).await;
    log::info!("[MusicAPI] music_user_playlist: cookie length={}, has MUSIC_U={}", 
        app_cookie.len(), cookie::netease_cookie_has_login(&app_cookie));
    
    let info = netease::login_status(&app_cookie).await?;
    log::info!("[MusicAPI] music_user_playlist: logged_in={}, user_id={}", info.logged_in, info.user_id);
    
    if !info.logged_in || info.user_id.is_empty() {
        return Ok(vec![]);
    }
    let result = netease::user_playlist(&info.user_id, &app_cookie).await;
    log::info!("[MusicAPI] music_user_playlist: result count={}", result.as_ref().map(|v| v.len()).unwrap_or(0));
    result
}

#[tauri::command]
pub async fn music_playlist_tracks(id: String) -> Result<(Playlist, Vec<Song>), String> {
    let app_cookie = get_app_cookie().await;
    netease::playlist_tracks(&id, &app_cookie).await
}

#[tauri::command]
pub async fn music_likelist(app: AppHandle) -> Result<Vec<String>, String> {
    let app_cookie = load_app_cookie(&app).await;
    let info = netease::login_status(&app_cookie).await?;
    netease::likelist(&info.user_id, &app_cookie).await
}

#[tauri::command]
pub async fn music_like(id: String, like: bool) -> Result<(), String> {
    let app_cookie = get_app_cookie().await;
    netease::like(&id, like, &app_cookie).await
}

#[tauri::command]
pub async fn music_lyric(id: String) -> Result<Lyrics, String> {
    let app_cookie = get_app_cookie().await;
    netease::lyric(&id, &app_cookie).await
}

#[tauri::command]
pub async fn music_personalized() -> Result<Vec<Playlist>, String> {
    let app_cookie = get_app_cookie().await;
    netease::personalized(&app_cookie).await
}

#[tauri::command]
pub async fn music_recommend_songs() -> Result<Vec<Song>, String> {
    let app_cookie = get_app_cookie().await;
    netease::recommend_songs(&app_cookie).await
}

#[tauri::command]
pub async fn music_artist_search(keywords: String, limit: Option<u32>) -> Result<Vec<Artist>, String> {
    let app_cookie = get_app_cookie().await;
    netease::artist_search(&keywords, limit.unwrap_or(30), &app_cookie).await
}

#[tauri::command]
pub async fn music_artist_songs(artist_id: String, limit: Option<u32>, offset: Option<u32>) -> Result<Vec<Song>, String> {
    let app_cookie = get_app_cookie().await;
    netease::artist_songs(&artist_id, limit.unwrap_or(50), offset.unwrap_or(0), &app_cookie).await
}

/// 网易云登录 cookie 优先级 (参考 Mineradio)
const NETEASE_COOKIE_PRIORITY: &[&str] = &[
    "MUSIC_U",
    "__csrf",
    "NMTID",
    "MUSIC_A",
    "__remember_me",
    "_ntes_nuid",
    "_ntes_nnid",
    "WEVNSM",
    "WNMCID",
    "JSESSIONID-WYYY",
];

/// 检查域名是否属于网易云
fn is_netease_domain(domain: &str) -> bool {
    let d = domain.trim_start_matches('.').to_lowercase();
    d == "163.com" || d.ends_with(".163.com") ||
    d == "music.163.com" || d.ends_with(".music.163.com") ||
    d == "netease.com" || d.ends_with(".netease.com")
}

/// 打开登录窗口 (网易云) - 使用 Tauri cookies() API 直接读取 HttpOnly cookie
/// 参考 Mineradio 的 Electron session.cookies.get() 方案
#[tauri::command]
pub async fn music_open_login_window(app: AppHandle) -> Result<String, String> {
    use tauri::WebviewUrl;

    let url = "https://music.163.com/#/login";
    let label = "netease-login";

    // 如果窗口已存在，清除 cookie 后刷新登录页（切换账号场景）
    if let Some(existing) = app.get_webview_window(label) {
        let _ = existing.clear_all_browsing_data();
        let login_url = url.parse::<Url>().map_err(|e| e.to_string())?;
        let _ = existing.navigate(login_url);
        let _ = existing.set_focus();
        return Ok("window_refreshed".into());
    }

    // 新窗口：先以 about:blank 创建，清除 cookie 后再导航到登录页
    let login_window = tauri::WebviewWindowBuilder::new(
        &app,
        label,
        WebviewUrl::External("about:blank".parse().map_err(|e: url::ParseError| e.to_string())?),
    )
    .title("网易云音乐登录")
    .inner_size(940.0, 760.0)
    .min_inner_size(780.0, 580.0)
    .build()
    .map_err(|e| format!("Failed to create login window: {e}"))?;

    // 清除残留 cookie，确保登录页不会自动登录
    let _ = login_window.clear_all_browsing_data();
    // 导航到登录页
    let login_url = url.parse::<Url>().map_err(|e| e.to_string())?;
    let _ = login_window.navigate(login_url);

    // 轮询读取 webview 的 cookie (包括 HttpOnly 的 MUSIC_U)
    // 这是关键: document.cookie 无法读取 HttpOnly cookie,
    // 但 Tauri 的 cookies() API 直接从 WebView2/WKWebView 读取, 可以拿到全部
    let win = login_window.clone();
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        // 等待页面加载
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        for _ in 0..150 {
            // 直接从 webview 读取所有 cookie (包括 HttpOnly)
            match win.cookies() {
                Ok(cookies) => {
                    // 内联构建 cookie 字符串 (类型由编译器推断, 无需命名)
                    use std::collections::HashMap;
                    let mut picked: HashMap<String, String> = HashMap::new();
                    for c in &cookies {
                        let domain_opt: Option<&str> = c.domain();
                        if let Some(domain) = domain_opt {
                            if is_netease_domain(domain) {
                                let name = c.name().to_string();
                                let value = c.value().to_string();
                                if !name.is_empty() && !value.is_empty() {
                                    picked.insert(name, value);
                                }
                            }
                        }
                    }
                    let mut ordered: Vec<(String, String)> = Vec::new();
                    for name in NETEASE_COOKIE_PRIORITY {
                        if let Some(value) = picked.remove(*name) {
                            ordered.push((name.to_string(), value));
                        }
                    }
                    for (name, value) in picked {
                        ordered.push((name, value));
                    }
                    let cookie_str = ordered.iter()
                        .map(|(k, v)| format!("{k}={v}"))
                        .collect::<Vec<_>>()
                        .join("; ");

                    if cookie::netease_cookie_has_login(&cookie_str) {
                        log::info!("[MusicAPI] MUSIC_U cookie found via webview cookies() API, cookie length: {}", cookie_str.len());
                        let _ = cookie::save_cookie(&app_handle, "netease", &cookie_str);
                        set_app_cookie(cookie_str).await;
                        // 强制关闭登录窗口
                        let _ = win.close();
                        // 直接在后端调用 login_status 验证 cookie 是否有效
                        let app_cookie = get_app_cookie().await;
                        match netease::login_status(&app_cookie).await {
                            Ok(info) => {
                                log::info!("[MusicAPI] Login status after cookie capture: logged_in={}, nickname={}", info.logged_in, info.nickname);
                                if info.logged_in {
                                    let _ = app_handle.emit("netease-login-success", &info);
                                    log::info!("[MusicAPI] Login success event emitted with user info");
                                } else {
                                    log::warn!("[MusicAPI] Cookie captured but login_status returned not logged in");
                                    let _ = app_handle.emit("netease-login-failed", "Cookie 无效或已过期");
                                }
                            }
                            Err(e) => {
                                log::error!("[MusicAPI] Login status check failed after cookie capture: {e}");
                                let _ = app_handle.emit("netease-login-failed", &e);
                            }
                        }
                        return;
                    }
                }
                Err(e) => {
                    log::warn!("[MusicAPI] Failed to read cookies from webview: {e}");
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        log::warn!("[MusicAPI] Login window polling timed out after 5 minutes");
    });

    Ok("window_created".into())
}

// ============================================================
//  全局 Cookie 缓存 (内存中，避免每次都读 store)
// ============================================================

static APP_COOKIE: tokio::sync::RwLock<String> = tokio::sync::RwLock::const_new(String::new());

async fn get_app_cookie() -> String {
    APP_COOKIE.read().await.clone()
}

pub async fn set_app_cookie(cookie: String) {
    let mut guard = APP_COOKIE.write().await;
    *guard = cookie;
}

async fn load_app_cookie(app: &AppHandle) -> String {
    // 先检查内存缓存
    let cached = APP_COOKIE.read().await.clone();
    if !cached.is_empty() {
        return cached;
    }
    // 从 store 加载
    match cookie::load_cookie(app, "netease") {
        Ok(c) => {
            set_app_cookie(c.clone()).await;
            c
        }
        Err(_) => String::new(),
    }
}

/// 初始化时从 store 加载 cookie 到内存
pub async fn init_cookie_cache(app: &AppHandle) {
    if let Ok(c) = cookie::load_cookie(app, "netease") {
        set_app_cookie(c).await;
    }
}
