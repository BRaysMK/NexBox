use std::time::Duration;

use reqwest::header::{COOKIE, REFERER, USER_AGENT};
use serde_json::{json, Value};

use super::crypto::{build_eapi_header, encrypt_eapi_payload};
use super::models::*;

const NETEASE_USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 9; PCT-AL10) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/70.0.3538.64 HuaweiBrowser/10.0.3.311 Mobile Safari/537.36";
const EAPI_BASE: &str = "https://interface3.music.163.com/eapi";

fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .expect("Failed to build HTTP client")
}

/// 发送普通 POST 请求 (带 Cookie, 不加密)
/// 参考 NeteaseCloudMusicApi / Mineradio 的请求方式
async fn post_api(
    client: &reqwest::Client,
    api_path: &str,
    params: &[(&str, &str)],
    cookie: &str,
) -> Result<Value, String> {
    let url = format!("https://music.163.com/api{}", &api_path[4..]); // /api/xxx -> https://music.163.com/api/xxx

    let resp = client
        .post(&url)
        .header(USER_AGENT, NETEASE_USER_AGENT)
        .header(REFERER, "https://music.163.com/")
        .header(COOKIE, cookie)
        .form(params)
        .send()
        .await
        .map_err(|e| format!("POST {api_path} failed: {e}"))?;

    let text = resp.text().await.map_err(|e| format!("Failed to read response: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("Failed to parse JSON from {api_path}: {e}"))
}

/// 发送普通 GET 请求 (带 Cookie, 不加密)
async fn get_api(
    client: &reqwest::Client,
    api_path: &str,
    params: &[(&str, &str)],
    cookie: &str,
) -> Result<Value, String> {
    let url = format!("https://music.163.com/api{}", &api_path[4..]);

    let resp = client
        .get(&url)
        .header(USER_AGENT, NETEASE_USER_AGENT)
        .header(REFERER, "https://music.163.com/")
        .header(COOKIE, cookie)
        .query(params)
        .send()
        .await
        .map_err(|e| format!("GET {api_path} failed: {e}"))?;

    let text = resp.text().await.map_err(|e| format!("Failed to read response: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("Failed to parse JSON from {api_path}: {e}"))
}

/// EAPI 请求结果 (包含 JSON 和 Set-Cookie)
struct EapiResult {
    json: Value,
    cookies: String, // 从 Set-Cookie 头提取的 cookie 字符串
}

/// 发送 EAPI 请求 (捕获 Set-Cookie)
async fn post_eapi_full(
    client: &reqwest::Client,
    api_path: &str,
    payload: serde_json::Map<String, Value>,
    user_cookie: &str,
) -> Result<EapiResult, String> {
    let url = format!("{EAPI_BASE}{}", &api_path[4..]); // 去掉 /api 前缀拼接

    let mut full_payload = payload;
    let header = build_eapi_header();
    full_payload.insert(
        "header".into(),
        Value::String(serde_json::to_string(&header).map_err(|e| e.to_string())?),
    );

    let payload_text = serde_json::to_string(&full_payload).map_err(|e| e.to_string())?;
    let encrypted = encrypt_eapi_payload(api_path, &payload_text)?;

    // 合并 header cookie 和用户 cookie
    let header_cookie = header
        .iter()
        .map(|(k, v)| format!("{k}={}", v.as_str().unwrap_or("")))
        .collect::<Vec<_>>()
        .join("; ");

    let full_cookie = if user_cookie.is_empty() {
        header_cookie
    } else {
        format!("{header_cookie}; {user_cookie}")
    };

    let resp = client
        .post(&url)
        .header(USER_AGENT, NETEASE_USER_AGENT)
        .header(REFERER, "https://music.163.com/")
        .header(COOKIE, &full_cookie)
        .form(&[("params", encrypted.as_str())])
        .send()
        .await
        .map_err(|e| format!("EAPI request failed: {e}"))?;

    // 提取 Set-Cookie 头中的所有 cookie
    let cookies: Vec<String> = resp
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .filter_map(|s| {
            // 取每条 Set-Cookie 的第一部分 (key=value)
            let part = s.split(';').next()?.trim();
            if part.is_empty() {
                None
            } else {
                Some(part.to_string())
            }
        })
        .collect();
    let cookie_str = cookies.join("; ");

    let text = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read EAPI response: {e}"))?;

    let json = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse EAPI response: {e}"))?;

    Ok(EapiResult { json, cookies: cookie_str })
}

/// 发送 EAPI 请求 (仅返回 JSON)
async fn post_eapi(
    client: &reqwest::Client,
    api_path: &str,
    payload: serde_json::Map<String, Value>,
    user_cookie: &str,
) -> Result<Value, String> {
    post_eapi_full(client, api_path, payload, user_cookie)
        .await
        .map(|r| r.json)
}

fn map_artists(raw: &[Value]) -> Vec<Artist> {
    raw.iter()
        .filter_map(|a| {
            let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name.is_empty() {
                None
            } else {
                Some(Artist {
                    id: a
                        .get("id")
                        .and_then(|v| v.as_i64())
                        .map(|n| n.to_string()),
                    mid: a.get("mid").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    name: name.to_string(),
                    pic_url: a
                        .get("picUrl")
                        .or_else(|| a.get("img1v1Url"))
                        .or_else(|| a.get("cover"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    music_size: a.get("musicSize").and_then(|v| v.as_i64()),
                })
            }
        })
        .collect()
}

fn map_song_record(s: &Value) -> Song {
    let artists_raw = s
        .get("ar")
        .or_else(|| s.get("artists"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let artists = map_artists(&artists_raw);

    let album = s.get("al").or_else(|| s.get("album")).unwrap_or(&Value::Null);
    Song {
        provider: "netease".into(),
        id: s.get("id")
            .and_then(|v| v.as_i64())
            .map(|n| n.to_string())
            .or_else(|| s.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .unwrap_or_default(),
        mid: None,
        media_mid: None,
        name: s.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        artist: artists.iter().map(|a| a.name.clone()).collect::<Vec<_>>().join(" / "),
        artists,
        album: album
            .get("name")
            .or_else(|| album.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        cover: album
            .get("picUrl")
            .or_else(|| album.get("coverUrl"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        duration: s
            .get("dt")
            .or_else(|| s.get("duration"))
            .and_then(|v| v.as_i64())
            .map(|n| n as u64)
            .unwrap_or(0),
        fee: s.get("fee").and_then(|v| v.as_i64()).map(|n| n as i32).unwrap_or(0),
        playable: true,
        language: s.get("language").and_then(|v| v.as_i64()).map(|n| n as i32).unwrap_or(0),
    }
}

/// 搜索歌曲
pub async fn search(keywords: &str, limit: u32, cookie: &str) -> Result<Vec<Song>, String> {
    let client = build_client();

    // 参考 NeteaseCloudMusicApi: 用普通 POST 请求 (带 Cookie), 不用 EAPI 加密
    let url = "https://music.163.com/api/cloudsearch/get/web";
    let body = format!(
        "s={}&type=1&limit={}&offset=0",
        urlencoding::encode(keywords),
        limit
    );

    let resp = client
        .post(url)
        .header(USER_AGENT, NETEASE_USER_AGENT)
        .header(REFERER, "https://music.163.com/")
        .header(COOKIE, cookie)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("Search request failed: {e}"))?;

    let text = resp.text().await.map_err(|e| format!("Failed to read search response: {e}"))?;
    let result: Value = serde_json::from_str(&text).map_err(|e| format!("Failed to parse search JSON: {e}"))?;

    log::info!("[NetEase] search response code={}", result.get("code").and_then(|v| v.as_i64()).unwrap_or(-1));

    let songs = result
        .get("result")
        .and_then(|r| r.get("songs"))
        .and_then(|s| s.as_array())
        .ok_or("No songs in search result")?;

    let mapped: Vec<Song> = songs.iter().map(map_song_record).collect();

    // 补齐缺失封面
    let missing: Vec<String> = mapped
        .iter()
        .filter(|s| s.cover.is_empty())
        .map(|s| s.id.clone())
        .collect();

    if !missing.is_empty() {
        if let Ok(details) = song_detail(&client, &missing, cookie).await {
            let id_to_pic: std::collections::HashMap<String, String> = details
                .iter()
                .map(|s| (s.id.clone(), s.cover.clone()))
                .collect();
            return Ok(mapped
                .iter()
                .map(|s| {
                    if s.cover.is_empty() {
                        Song {
                            cover: id_to_pic.get(&s.id).cloned().unwrap_or_default(),
                            ..s.clone()
                        }
                    } else {
                        s.clone()
                    }
                })
                .collect());
        }
    }

    Ok(mapped)
}

/// 歌曲详情 (批量)
pub async fn song_detail(
    client: &reqwest::Client,
    ids: &[String],
    cookie: &str,
) -> Result<Vec<Song>, String> {
    // 构建 c 参数: [{"id": 123}, {"id": 456}]
    let c_value: Value = Value::Array(
        ids.iter()
            .map(|id| json!({ "id": id.parse::<i64>().unwrap_or(0) }))
            .collect(),
    );
    let c_str = serde_json::to_string(&c_value).unwrap_or_default();

    let result = post_api(client, "/api/v3/song/detail", &[("c", &c_str), ("csrf_token", "")], cookie).await?;

    let songs = result
        .get("songs")
        .and_then(|s| s.as_array())
        .ok_or("No songs in detail result")?;

    Ok(songs.iter().map(map_song_record).collect())
}

/// 获取播放地址 (带音质降级)
pub async fn song_url(id: &str, preferred_quality: &str, cookie: &str) -> Result<SongUrlResult, String> {
    let client = build_client();
    let candidates = quality_candidates_from(preferred_quality);
    let mut trial_fallback: Option<SongUrlResult> = None;

    for q in &candidates {
        let level = q.level.as_str();
        // ids 参数需要 JSON 数组格式: ["12345"]
        let ids_json = format!("[\"{id}\"]");
        let params: [(&str, &str); 4] = [
            ("ids", &ids_json),
            ("level", level),
            ("encodeType", "flac"),
            ("csrf_token", ""),
        ];

        match post_api(&client, "/api/song/enhance/player/url/v1", &params, cookie).await {
            Ok(result) => {
                let data = result
                    .get("data")
                    .and_then(|d| d.as_array())
                    .and_then(|arr| arr.first())
                    .cloned()
                    .unwrap_or(Value::Null);

                let url = data.get("url").and_then(|u| u.as_str()).unwrap_or("");
                let free_trial = data.get("freeTrialInfo").is_some();
                let br = data.get("br").and_then(|b| b.as_u64()).unwrap_or(0);
                let fee = data.get("fee").and_then(|f| f.as_i64()).map(|n| n as i32);

                if !url.is_empty() && !free_trial {
                    return Ok(SongUrlResult {
                        url: Some(url.to_string()),
                        playable: true,
                        trial: false,
                        level: q.level.clone(),
                        quality: q.label.clone(),
                        br,
                        fee,
                        ..Default::default()
                    });
                }

                if !url.is_empty() && free_trial && trial_fallback.is_none() {
                    trial_fallback = Some(SongUrlResult {
                        url: Some(url.to_string()),
                        playable: true,
                        trial: true,
                        level: q.level.clone(),
                        quality: q.label.clone(),
                        br,
                        fee,
                        message: Some("仅试听片段".into()),
                        ..Default::default()
                    });
                }
            }
            Err(_e) => {
                // 继续尝试下一个音质
            }
        }
    }

    if let Some(fallback) = trial_fallback {
        return Ok(fallback);
    }

    Ok(SongUrlResult {
        playable: false,
        reason: Some("url_unavailable".into()),
        message: Some("无法获取播放地址，可能需要登录或 VIP".into()),
        ..Default::default()
    })
}

/// 二维码登录 - 获取 key
pub async fn login_qr_key(cookie: &str) -> Result<String, String> {
    let client = build_client();
    let mut payload = serde_json::Map::new();
    payload.insert("type".into(), json!(1));
    payload.insert("csrf_token".into(), json!(""));

    let result = post_eapi(&client, "/api/login/qrcode/unikey", payload, cookie).await?;
    result
        .get("unikey")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or("Failed to get QR key".into())
}

/// 二维码登录 - 生成二维码 URL
/// 网易云 EAPI 不返回 qrimg，需要用 key 拼接 URL 在前端生成 QR 码
pub async fn login_qr_create(key: &str, _cookie: &str) -> Result<String, String> {
    // 返回扫码登录 URL，前端用 qrcode 库渲染
    Ok(format!("https://music.163.com/login?codekey={key}"))
}

/// 二维码登录 - 检查状态 (捕获 Set-Cookie)
pub async fn login_qr_check(key: &str, cookie: &str) -> Result<QrCheckResult, String> {
    let client = build_client();
    let mut payload = serde_json::Map::new();
    payload.insert("key".into(), json!(key));
    payload.insert("csrf_token".into(), json!(""));

    let eapi_result = post_eapi_full(&client, "/api/login/qrcode/client/login", payload, cookie).await?;
    let result = &eapi_result.json;

    let code = result
        .get("code")
        .and_then(|v| v.as_i64())
        .map(|n| n as i32)
        .unwrap_or(0);
    let message = result
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let nickname = result
        .get("nickname")
        .or_else(|| result.get("profile").and_then(|p| p.get("nickname")))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let avatar = result
        .get("avatarUrl")
        .or_else(|| result.get("profile").and_then(|p| p.get("avatarUrl")))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // code 803 = 登录成功
    // Cookie 在 Set-Cookie 响应头中，不在 JSON body 中
    let extracted_cookie = if code == 803 {
        if !eapi_result.cookies.is_empty() {
            Some(eapi_result.cookies.clone())
        } else {
            // 尝试从 JSON body 提取 (有些版本会返回)
            result
                .get("cookie")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        }
    } else {
        None
    };

    Ok(QrCheckResult {
        code,
        message,
        cookie: extracted_cookie,
        nickname,
        avatar,
    })
}

/// 获取登录状态和用户信息
/// 参考 Mineradio: 用普通 GET 请求 (带 Cookie), 不用 EAPI 加密
pub async fn login_status(cookie: &str) -> Result<LoginInfo, String> {
    if cookie.is_empty() || !super::cookie::netease_cookie_has_login(cookie) {
        return Ok(LoginInfo {
            provider: "netease".into(),
            ..Default::default()
        });
    }

    let client = build_client();

    // 优先用 /api/w/nuser/account/get (GET, 带 Cookie)
    let url = "https://music.163.com/api/w/nuser/account/get";
    let resp = client
        .get(url)
        .header(USER_AGENT, NETEASE_USER_AGENT)
        .header(REFERER, "https://music.163.com/")
        .header(COOKIE, cookie)
        .send()
        .await
        .map_err(|e| format!("login_status request failed: {e}"))?;

    let text = resp.text().await.map_err(|e| format!("Failed to read response: {e}"))?;
    let result: Value = serde_json::from_str(&text).map_err(|e| format!("Failed to parse JSON: {e}"))?;

    let code = result.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
    log::info!("[NetEase] login_status GET response code={code}");

    // 参考 Mineradio: data.profile || body.profile, data.account || body.account
    let data = result.get("data").unwrap_or(&result);
    let profile = data
        .get("profile")
        .or_else(|| result.get("profile"))
        .or_else(|| data.get("account").and_then(|a| a.get("profile")))
        .or_else(|| result.get("account").and_then(|a| a.get("profile")))
        .cloned()
        .unwrap_or(Value::Null);

    if profile.is_null() {
        log::warn!("[NetEase] login_status: profile is null, response: {result}");
        return Ok(LoginInfo {
            provider: "netease".into(),
            logged_in: false,
            ..Default::default()
        });
    }

    // 参考 Mineradio normalizeLoginInfo: userId 必须存在才算登录
    let user_id = profile
        .get("userId")
        .or_else(|| profile.get("user_id"))
        .or_else(|| profile.get("id"))
        .and_then(|v| v.as_i64())
        .map(|n| n.to_string())
        .unwrap_or_default();

    if user_id.is_empty() {
        log::warn!("[NetEase] login_status: no userId in profile, profile: {profile}");
        return Ok(LoginInfo {
            provider: "netease".into(),
            logged_in: false,
            ..Default::default()
        });
    }

    let vip_type = profile
        .get("vipType")
        .and_then(|v| v.as_i64())
        .map(|n| n as i32)
        .unwrap_or(0);
    let is_vip = vip_type >= 1;
    let is_svip = vip_type >= 10;

    let nickname = profile
        .get("nickname")
        .or_else(|| profile.get("userName"))
        .and_then(|v| v.as_str())
        .unwrap_or("网易云用户")
        .to_string();

    log::info!("[NetEase] login_status success: userId={user_id}, nickname={nickname}");

    Ok(LoginInfo {
        provider: "netease".into(),
        logged_in: true,
        user_id,
        nickname,
        avatar: profile
            .get("avatarUrl")
            .or_else(|| profile.get("avatar"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        vip_type,
        vip_level: if is_svip {
            "svip".into()
        } else if is_vip {
            "vip".into()
        } else {
            "none".into()
        },
        is_vip,
        is_svip,
    })
}

/// 获取用户歌单
pub async fn user_playlist(uid: &str, cookie: &str) -> Result<Vec<Playlist>, String> {
    let client = build_client();
    // 用 GET 请求, uid 作为 query 参数
    let limit_str = "100".to_string();
    let offset_str = "0".to_string();
    let result = get_api(&client, "/api/user/playlist", &[
        ("uid", uid),
        ("limit", &limit_str),
        ("offset", &offset_str),
        ("csrf_token", ""),
    ], cookie).await?;

    let code = result.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
    let keys = result.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>().join(",")).unwrap_or_default();
    log::info!("[NetEase] user_playlist response code={code}, keys={keys}");

    // 普通 API 可能返回 playlist 数组, 也可能直接是数组
    let playlists = result
        .get("playlist")
        .and_then(|p| p.as_array())
        .or_else(|| result.as_array())
        .ok_or_else(|| format!("No playlist in result, code={code}, keys={keys}"))?;

    Ok(playlists
        .iter()
        .map(|pl| Playlist {
            provider: "netease".into(),
            id: pl.get("id").and_then(|v| v.as_i64()).map(|n| n.to_string()).unwrap_or_default(),
            name: pl.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            cover: pl.get("coverImgUrl").or_else(|| pl.get("picUrl")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
            track_count: pl.get("trackCount").and_then(|v| v.as_u64()).map(|n| n as u32).unwrap_or(0),
            creator: pl
                .get("creator")
                .and_then(|c| c.get("nickname"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        })
        .collect())
}

/// 获取歌单内曲目
/// 用 /api/v6/playlist/detail 获取歌单信息 + trackIds（全部曲目ID），
/// 如果 tracks 字段已包含全部曲目则直接返回，
/// 否则用 trackIds + song_detail 批量获取完整曲目信息。
pub async fn playlist_tracks(id: &str, cookie: &str) -> Result<(Playlist, Vec<Song>), String> {
    let client = build_client();

    // ── 第一步：获取歌单信息 ──
    let n_str = "1000".to_string();
    let s_str = "0".to_string();
    let detail = post_api(&client, "/api/v6/playlist/detail", &[
        ("id", id),
        ("n", &n_str),
        ("s", &s_str),
        ("csrf_token", ""),
    ], cookie).await?;

    let pl = detail.get("playlist").ok_or("No playlist in result")?;
    let playlist = Playlist {
        provider: "netease".into(),
        id: pl.get("id").and_then(|v| v.as_i64()).map(|n| n.to_string()).unwrap_or_default(),
        name: pl.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        cover: pl.get("coverImgUrl").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        track_count: pl.get("trackCount").and_then(|v| v.as_u64()).map(|n| n as u32).unwrap_or(0),
        creator: pl.get("creator").and_then(|c| c.get("nickname")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
    };

    // detail 中的 tracks（可能不完整，推荐歌单可能只返回 20 首）
    let detail_tracks: Vec<Song> = pl
        .get("tracks")
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(map_song_record)
        .collect();

    // 提取 trackIds（包含歌单所有曲目 ID）
    let track_ids: Vec<String> = pl
        .get("trackIds")
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|v| v.get("id").and_then(|id| id.as_i64()).map(|n| n.to_string()))
        .collect();

    // 如果 detail tracks 已经包含全部曲目，直接返回
    let total = if !track_ids.is_empty() {
        track_ids.len()
    } else {
        playlist.track_count as usize
    };

    if detail_tracks.len() >= total || track_ids.is_empty() {
        return Ok((playlist, detail_tracks));
    }

    // ── 第二步：用 trackIds 批量获取完整曲目信息 ──
    // song_detail 每次最多约 1000 首
    let mut all_tracks: Vec<Song> = Vec::with_capacity(total);
    let batch_size = 1000;

    for chunk in track_ids.chunks(batch_size) {
        match song_detail(&client, chunk, cookie).await {
            Ok(songs) => all_tracks.extend(songs),
            Err(_) => {
                // 如果批量获取失败，回退到 detail tracks
                if all_tracks.len() < detail_tracks.len() {
                    return Ok((playlist, detail_tracks));
                }
                break;
            }
        }
    }

    if all_tracks.len() >= detail_tracks.len() {
        Ok((playlist, all_tracks))
    } else {
        Ok((playlist, detail_tracks))
    }
}

/// 获取喜欢列表
pub async fn likelist(uid: &str, cookie: &str) -> Result<Vec<String>, String> {
    let client = build_client();
    let result = post_api(&client, "/api/likelist/get", &[
        ("uid", uid),
        ("csrf_token", ""),
    ], cookie).await?;

    let ids = result.get("ids").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    Ok(ids
        .iter()
        .filter_map(|v| {
            v.as_i64().map(|n| n.to_string())
                .or_else(|| v.as_str().map(|s| s.to_string()))
        })
        .collect())
}

/// 红心/取消红心
pub async fn like(id: &str, like: bool, cookie: &str) -> Result<(), String> {
    let client = build_client();
    let like_str = if like { "true" } else { "false" };
    let result = post_api(&client, "/api/song/like", &[
        ("trackId", id),
        ("like", like_str),
        ("csrf_token", ""),
    ], cookie).await?;
    let code = result.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
    if code == 200 {
        Ok(())
    } else {
        Err(format!("Like failed with code: {code}"))
    }
}

/// 获取歌词
pub async fn lyric(id: &str, cookie: &str) -> Result<Lyrics, String> {
    let client = build_client();
    let result = post_api(&client, "/api/song/lyric/v1", &[
        ("id", id),
        ("cp", "false"),
        ("lv", "0"),
        ("kv", "0"),
        ("tv", "0"),
        ("rv", "0"),
        ("yv", "0"),
        ("ytv", "0"),
        ("yrv", "0"),
        ("csrf_token", ""),
    ], cookie).await?;

    let lyric = result
        .get("lrc")
        .and_then(|l| l.get("lyric"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let translation = result
        .get("tlyric")
        .and_then(|l| l.get("lyric"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let roma = result
        .get("romalrc")
        .and_then(|l| l.get("lyric"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let yrc = result
        .get("yrc")
        .and_then(|l| l.get("lyric"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    Ok(Lyrics {
        lyric,
        translation,
        roma,
        yrc,
    })
}

/// 推荐歌单
pub async fn personalized(cookie: &str) -> Result<Vec<Playlist>, String> {
    let client = build_client();
    let limit_str = "30".to_string();
    let result = post_api(&client, "/api/personalized/playlist", &[
        ("limit", &limit_str),
        ("csrf_token", ""),
    ], cookie).await?;

    let playlists = result.get("result").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    Ok(playlists
        .iter()
        .map(|pl| Playlist {
            provider: "netease".into(),
            id: pl.get("id").and_then(|v| v.as_i64()).map(|n| n.to_string()).unwrap_or_default(),
            name: pl.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            cover: pl.get("picUrl").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            track_count: pl.get("trackCount")
                .and_then(|v| v.as_u64().or_else(|| v.as_i64().map(|n| n as u64)).or_else(|| v.as_f64().map(|n| n as u64)))
                .map(|n| n as u32)
                .unwrap_or(0),
            creator: pl.get("creator").and_then(|c| c.get("nickname")).and_then(|v| v.as_str())
                .or_else(|| pl.get("copywriter").and_then(|v| v.as_str()))
                .unwrap_or("").to_string(),
        })
        .collect())
}

/// 每日推荐歌曲
pub async fn recommend_songs(cookie: &str) -> Result<Vec<Song>, String> {
    let client = build_client();
    let result = get_api(&client, "/api/v3/discovery/recommend/songs", &[
        ("csrf_token", ""),
    ], cookie).await?;

    let songs = result
        .get("data")
        .and_then(|d| d.get("dailySongs"))
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();

    Ok(songs.iter().map(map_song_record).collect())
}

/// 搜索歌手 (使用 cloudsearch type=100)
pub async fn artist_search(keywords: &str, limit: u32, cookie: &str) -> Result<Vec<Artist>, String> {
    let client = build_client();
    let url = "https://music.163.com/api/cloudsearch/get/web";
    let body = format!(
        "s={}&type=100&limit={}&offset=0",
        urlencoding::encode(keywords),
        limit
    );

    let resp = client
        .post(url)
        .header(USER_AGENT, NETEASE_USER_AGENT)
        .header(REFERER, "https://music.163.com/")
        .header(COOKIE, cookie)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("Artist search request failed: {e}"))?;

    let text = resp.text().await.map_err(|e| format!("Failed to read artist search response: {e}"))?;
    let result: Value = serde_json::from_str(&text).map_err(|e| format!("Failed to parse artist search JSON: {e}"))?;

    log::info!("[NetEase] artist_search response code={}", result.get("code").and_then(|v| v.as_i64()).unwrap_or(-1));

    let artists_raw = result
        .get("result")
        .and_then(|r| r.get("artists"))
        .and_then(|a| a.as_array())
        .ok_or("No artists in search result")?;

    let artists = map_artists(artists_raw);

    Ok(artists)
}

/// 获取歌手热门歌曲（参考 Mineradio：先用 EAPI artist/songs，失败回退 artist/top/song）
pub async fn artist_songs(
    artist_id: &str,
    limit: u32,
    offset: u32,
    cookie: &str,
) -> Result<Vec<Song>, String> {
    let client = build_client();

    // 主策略：EAPI POST /api/artist/songs
    let mut payload = serde_json::Map::new();
    payload.insert("id".into(), json!(artist_id));
    payload.insert("limit".into(), json!(limit));
    payload.insert("offset".into(), json!(offset));
    payload.insert("order".into(), json!("hot"));

    let result = post_eapi(&client, "/api/artist/songs", payload, cookie).await?;
    log::info!("[NetEase] artist_songs EAPI response code={}", result.get("code").and_then(|v| v.as_i64()).unwrap_or(-1));

    let songs_raw = result
        .get("songs")
        .or_else(|| result.get("data").and_then(|d| d.get("songs")))
        .or_else(|| result.get("hotSongs"))
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();

    // 回退策略：如果 artist/songs 返回空，尝试 artist/top/song
    let songs_raw = if songs_raw.is_empty() {
        log::info!("[NetEase] artist_songs empty, trying artist/top/song fallback");
        let mut fallback_payload = serde_json::Map::new();
        fallback_payload.insert("id".into(), json!(artist_id));

        match post_eapi(&client, "/api/artist/top/song", fallback_payload, cookie).await {
            Ok(fb) => {
                log::info!("[NetEase] artist/top/song response code={}", fb.get("code").and_then(|v| v.as_i64()).unwrap_or(-1));
                fb.get("songs")
                    .or_else(|| fb.get("data").and_then(|d| d.get("songs")))
                    .and_then(|s| s.as_array())
                    .cloned()
                    .unwrap_or_default()
            }
            Err(e) => {
                log::warn!("[NetEase] artist/top/song fallback failed: {e}");
                vec![]
            }
        }
    } else {
        songs_raw
    };

    if songs_raw.is_empty() {
        return Err("No songs in artist result".into());
    }

    let mut mapped: Vec<Song> = songs_raw.iter().map(map_song_record).collect();

    // 歌手歌曲 API 返回的歌曲可能缺少封面，用 song_detail 补齐
    let missing: Vec<String> = mapped
        .iter()
        .filter(|s| s.cover.is_empty())
        .map(|s| s.id.clone())
        .collect();

    if !missing.is_empty() {
        if let Ok(details) = song_detail(&client, &missing, cookie).await {
            let id_to_pic: std::collections::HashMap<String, String> = details
                .iter()
                .map(|s| (s.id.clone(), s.cover.clone()))
                .collect();
            mapped = mapped
                .iter()
                .map(|s| {
                    if s.cover.is_empty() {
                        Song {
                            cover: id_to_pic.get(&s.id).cloned().unwrap_or_default(),
                            ..s.clone()
                        }
                    } else {
                        s.clone()
                    }
                })
                .collect();
        }
    }

    Ok(mapped)
}
