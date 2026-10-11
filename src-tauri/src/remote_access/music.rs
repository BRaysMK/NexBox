//! 远程接入：统一「当前播放」——软件内播放器 / 外部 SMTC 二选一。
//!
//! 手机端只需要一条「当前播放」，优先级为：
//! 软件内在播 → 外部在播 → 软件内(暂停) → 外部(暂停) → 无。
//!
//! 两个来源的状态都来自各自模块的全局缓存：
//! - 软件内：`crate::smtc::current_with_age()`（前端 zustand 每秒推送，见 `smtc_update_state`）
//! - 外部：`crate::external_player::current_state()`（后台线程每秒轮询系统会话）
//!
//! 封面**不随 WS 帧下发**（外部封面 data URI 可达 5MB，base64 后 ~6.7MB），
//! 只给 `coverKey` 与相对 `coverUrl`；字节由 `GET /api/music/cover` 按需提供，
//! 内容寻址 → 同一首歌 URL 不变，手机端可长期缓存。

use std::sync::Mutex;

use serde::Serialize;
use serde_json::{json, Value};
use tauri::Emitter;

use crate::external_player::{ExternalPlayback, ExternalTrack};
use crate::netease_lyrics::RemoteLyrics;
use crate::smtc::SmtcState;

/// 手机端可见的统一「当前播放」快照。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicState {
    /// `"app"` | `"external"`；`None` 表示当前无播放内容。
    pub source: Option<String>,
    pub playing: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub position_ms: i64,
    pub duration_ms: i64,
    /// 时长已知才可拖动进度。
    pub can_seek: bool,
    /// 封面内容标识（封面来源字符串的稳定 hash）；无封面为 None。
    pub cover_key: Option<String>,
    /// 相对路径，手机端自行拼 host 与 token。
    pub cover_url: Option<String>,
    /// 外部来源的 AUMID（如 `cloudmusic.exe`）；软件内为 None。
    pub source_app_id: Option<String>,
    /// 服务端合成时刻（epoch 毫秒）。
    pub updated_at: u64,
}

/// 已知不支持 SMTC 位置调整的客户端（AUMID 小写子串匹配）：
/// QQ 音乐会正常汇报进度，但 `TryChangePlaybackPosition` 被静默忽略，
/// 拖了也不会跳 —— 手机端对这类来源直接禁用进度条拖动。
const SEEK_BLOCKED_KEYS: &[&str] = &["qqmusic", "tencentmusic"];

fn seek_blocked(source_app_id: &str) -> bool {
    let id = source_app_id.to_lowercase();
    SEEK_BLOCKED_KEYS.iter().any(|key| id.contains(key))
}

impl MusicState {
    /// 变化签名：签名不变时 WS 不必重推全量（位置推进由 `position_ms` 单独判定）。
    pub fn signature(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}",
            self.source.as_deref().unwrap_or(""),
            self.title,
            self.artist,
            self.album,
            self.playing,
            self.duration_ms,
            self.cover_key.as_deref().unwrap_or(""),
        )
    }
}

/// 当前生效的来源。
enum Active {
    /// 软件内来源；附带数据年龄（距前端上次推送的毫秒数），用于线性推算实时位置
    App(SmtcState, u64),
    External(ExternalPlayback),
}

/// 按优先级选定当前来源；标题为空的来源视为无效（与灵动岛的判定口径一致）。
fn pick() -> Option<Active> {
    let app_with_age = crate::smtc::current_with_age().filter(|(s, _)| !s.title.trim().is_empty());
    let app = app_with_age.as_ref().map(|(s, _)| s.clone());
    let ext = crate::external_player::current_state()
        .filter(|e| e.track.as_ref().map(|t| !t.title.trim().is_empty()).unwrap_or(false));

    if app.as_ref().map(|s| s.playing).unwrap_or(false) {
        return app_with_age.map(|(s, age)| Active::App(s, age));
    }
    if ext.as_ref().map(|e| e.is_playing).unwrap_or(false) {
        return ext.map(Active::External);
    }
    app_with_age
        .map(|(s, age)| Active::App(s, age))
        .or_else(|| ext.map(Active::External))
}

/// 封面来源字符串 → 稳定短 hash（FNV-1a 64）。
/// 刻意不用 `DefaultHasher`：其输出不保证跨 Rust 版本稳定，而 `coverKey` 会进 URL 被手机端缓存。
fn cover_key(src: &str) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    for b in src.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(PRIME);
    }
    format!("{h:016x}")
}

/// 当前生效来源的封面「来源字符串」：
/// 软件内是 URL / `file://` 路径 / data URI；外部是 data URI。
fn active_cover_source() -> Option<String> {
    match pick()? {
        Active::App(s, _) => s.cover.filter(|c| !c.trim().is_empty()),
        Active::External(e) => e.track.and_then(|t| t.cover).filter(|c| !c.trim().is_empty()),
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 合成一次统一快照。
pub fn snapshot() -> MusicState {
    let (source, playing, title, artist, album, position_ms, duration_ms, source_app_id, ckey) =
        match pick() {
            Some(Active::App(s, age)) => {
                // 位置线性推算：前端每 1s 节流推送，两次推送之间的位置用
                // 「上次推送值 + 播放中流逝」补齐（age 上限 5s 防异常乱跳），
                // 否则手机端拿到的位置最多滞后 1s，歌词高亮/进度条都会慢半拍
                let position = if s.playing {
                    s.position_ms + age.min(5_000) as i64
                } else {
                    s.position_ms
                };
                (
                    Some("app".to_string()),
                    s.playing,
                    s.title,
                    s.artist,
                    s.album,
                    position,
                    s.duration_ms,
                    None,
                    s.cover.filter(|c| !c.trim().is_empty()).map(|c| cover_key(&c)),
                )
            },
            Some(Active::External(e)) => {
                let t: ExternalTrack = e.track.unwrap_or_default();
                (
                    Some("external".to_string()),
                    e.is_playing,
                    t.title,
                    t.artist,
                    t.album,
                    e.position_ms,
                    e.duration_ms,
                    Some(t.source_app_id),
                    t.cover.filter(|c| !c.trim().is_empty()).map(|c| cover_key(&c)),
                )
            }
            None => (
                None,
                false,
                String::new(),
                String::new(),
                String::new(),
                0,
                0,
                None,
                None,
            ),
        };

    MusicState {
        cover_url: ckey.as_ref().map(|k| format!("/api/music/cover?k={k}")),
        cover_key: ckey,
        // 外部来源还要过 seek 黑名单：QQ 音乐之类拖了也不生效，不如直接禁用
        can_seek: duration_ms > 0
            && source_app_id
                .as_deref()
                .map(|id| !seek_blocked(id))
                .unwrap_or(true),
        updated_at: now_ms(),
        source,
        playing,
        title,
        artist,
        album,
        position_ms,
        duration_ms,
        source_app_id,
    }
}

/// 供 `exec_query("music.state")` 使用。
pub fn state_json() -> Result<Value, String> {
    serde_json::to_value(snapshot()).map_err(|e| e.to_string())
}

/// 单槽歌词缓存：`(写入时刻, 歌词)`。有歌词长期有效；
/// `lines` 为空是负缓存（这首歌没找到歌词），30s 后过期允许重查
/// —— 否则「app 歌词稍后才推到位」会被第一次的空结果永远挡住。
static LYRICS_CACHE: Mutex<Option<(std::time::Instant, RemoteLyrics)>> = Mutex::new(None);

/// 负缓存有效期：空歌词结果 30s 后允许重查（外部来源重搜一次也就两次 HTTP）。
const LYRICS_NEG_TTL: std::time::Duration = std::time::Duration::from_secs(30);

/// 软件内播放器的歌词缓存：前端随 `smtc_update_state` 推来的 LRC 原文解析结果。
/// 按歌曲 id 拉的精确歌词，比按元数据搜网易云可靠 —— app 来源查询优先消费它。
static APP_LYRICS_CACHE: Mutex<Option<RemoteLyrics>> = Mutex::new(None);

/// 软件内歌词写入入口（smtc.rs 在前端推送状态时顺路调用）。
/// `lrc` 为 None/空 = 这首歌没歌词：**不清已有缓存**（保留可能存在的搜索结果）。
/// 解析失败静默忽略（歌词行空包照存，等价「无歌词」）。
pub fn cache_app_lyrics(title: &str, artist: &str, lrc: Option<String>) {
    let Some(text) = lrc.filter(|s| !s.trim().is_empty()) else {
        return;
    };
    let lines = crate::netease_lyrics::parse_lrc_text(&text);
    if lines.is_empty() {
        log::warn!("[RemoteLyrics] 软件内歌词推送解析为空（LRC 格式异常？）: {title}");
        return;
    }
    log::info!(
        "[RemoteLyrics] 软件内歌词已入缓存: {title} - {artist}，共 {} 行",
        lines.len()
    );
    let lyrics = RemoteLyrics {
        track_key: format!("app|{title}|{artist}"),
        title: title.to_string(),
        artist: artist.to_string(),
        lines,
    };
    if let Ok(mut cache) = APP_LYRICS_CACHE.lock() {
        *cache = Some(lyrics);
    }
}

/// 外部歌词后台搜索的防重标志（同一时刻至多一个搜索线程）
static LYRICS_SEARCHING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// WS 循环每秒调用：返回当前应下发给手机端的歌词。
///
/// 软件内：直接内存匹配 APP_LYRICS_CACHE（前端随状态推来的精确歌词，零 HTTP）；
/// 外部：读 LYRICS_CACHE，未命中/负缓存过期时后台补搜一次（防重，不阻塞帧循环）。
/// 返回 None 表示当前没有歌词可推（手机端保持现状/回律动条）。
pub fn lyrics_for_stream(title: &str, artist: &str, source: Option<&str>) -> Option<RemoteLyrics> {
    if title.trim().is_empty() {
        return None;
    }
    let cache_key = cover_key(&format!("{title}|{artist}"));

    if source == Some("app") {
        return APP_LYRICS_CACHE.lock().ok().and_then(|app| {
            app.as_ref()
                .filter(|l| l.title == title && l.artist == artist)
                .cloned()
        });
    }

    let mut needs_search = false;
    let mut hit = None;
    if let Ok(cache) = LYRICS_CACHE.lock() {
        if let Some((ts, l)) = cache.as_ref() {
            if cover_key(&format!("{}|{}", l.title, l.artist)) == cache_key {
                // 有歌词长期有效；空结果（负缓存）过期后允许后台重搜
                if !l.lines.is_empty() || ts.elapsed() < LYRICS_NEG_TTL {
                    hit = Some(l.clone());
                } else {
                    needs_search = true;
                }
            } else {
                needs_search = true;
            }
        } else {
            needs_search = true;
        }
    }
    if needs_search {
        spawn_lyrics_search(title.to_string(), artist.to_string());
    }
    hit
}

/// 后台补搜外部歌词（防重：搜索中不重复触发；结果写回 LYRICS_CACHE，
/// 下一秒的 WS 循环就能看到并推帧）。
fn spawn_lyrics_search(title: String, artist: String) {
    use std::sync::atomic::Ordering;
    if LYRICS_SEARCHING.swap(true, Ordering::SeqCst) {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("remote-lyrics-search".into())
        .spawn(move || {
            let lyrics = crate::netease_lyrics::fetch_meta_lyrics(&title, &artist)
                .unwrap_or_else(|| RemoteLyrics {
                    track_key: String::new(),
                    title: title.clone(),
                    artist: artist.clone(),
                    lines: Vec::new(),
                });
            log::info!(
                "[RemoteLyrics] 外部歌词搜索完成: {} - {}，共 {} 行",
                lyrics.title,
                lyrics.artist,
                lyrics.lines.len()
            );
            if let Ok(mut cache) = LYRICS_CACHE.lock() {
                *cache = Some((std::time::Instant::now(), lyrics));
            }
            LYRICS_SEARCHING.store(false, Ordering::SeqCst);
        })
        .is_ok();
    if !spawned {
        LYRICS_SEARCHING.store(false, Ordering::SeqCst);
    }
}

/// 供 `exec_query("music.lyrics")` 使用：按当前播放的「歌名 + 歌手」
/// 查网易云歌词（带时间戳，手机端按 positionMs 高亮滚动）。
///
/// **阻塞**（eapi 搜索 + 歌词两次 HTTP，共 8s 超时）——只能在 `spawn_blocking` 中调用。
/// 无播放内容返回 `Null`；没匹配到歌词返回 `lines: []` 的包（照样缓存）。
/// 歌词来源与播放器无关：软件内播放器和 QQ/酷狗等外部客户端都能查网易云曲库。
pub fn lyrics_json() -> Result<Value, String> {
    let snap = snapshot();
    if snap.title.trim().is_empty() {
        return Ok(Value::Null);
    }

    let cache_key = cover_key(&format!("{}|{}", snap.title, snap.artist));
    let now = std::time::Instant::now();

    // ① 主缓存：有歌词长期命中；空歌词是负缓存，30s 后过期放行重查
    if let Ok(cache) = LYRICS_CACHE.lock() {
        if let Some((cached_at, l)) = cache.as_ref() {
            if cover_key(&format!("{}|{}", l.title, l.artist)) == cache_key {
                let still_valid = !l.lines.is_empty() || now.duration_since(*cached_at) < LYRICS_NEG_TTL;
                if still_valid {
                    return serde_json::to_value(l).map_err(|e| e.to_string());
                }
            }
        }
    }

    // ② 软件内来源：优先用前端随状态推来的精确歌词（按歌曲 id 拉取，零匹配误差）。
    //    未命中（前端歌词还没推到位）时返回空包但**不写负缓存** —— 歌词可能几秒后就位。
    if snap.source.as_deref() == Some("app") {
        if let Ok(app) = APP_LYRICS_CACHE.lock() {
            if let Some(l) = app.as_ref() {
                if l.title == snap.title && l.artist == snap.artist {
                    if let Ok(mut cache) = LYRICS_CACHE.lock() {
                        *cache = Some((now, l.clone())); // 顺手并入主缓存，下轮直接命中
                    }
                    log::info!("[RemoteLyrics] 软件内歌词命中: {}", snap.title);
                    return serde_json::to_value(l).map_err(|e| e.to_string());
                }
            }
        }
        log::info!(
            "[RemoteLyrics] 软件内歌词未就位（前端未推到或标题不匹配），返回空包: {} - {}",
            snap.title,
            snap.artist
        );
        let pending = RemoteLyrics {
            track_key: String::new(),
            title: snap.title.clone(),
            artist: snap.artist.clone(),
            lines: Vec::new(),
        };
        return serde_json::to_value(&pending).map_err(|e| e.to_string());
    }

    // ③ 外部来源 / app 兜底：按「歌名 + 歌手」搜网易云曲库（两次 HTTP）
    let lyrics = crate::netease_lyrics::fetch_meta_lyrics(&snap.title, &snap.artist)
        .unwrap_or_else(|| RemoteLyrics {
            track_key: String::new(),
            title: snap.title.clone(),
            artist: snap.artist.clone(),
            lines: Vec::new(),
        });
    if let Ok(mut cache) = LYRICS_CACHE.lock() {
        *cache = Some((now, lyrics.clone()));
    }
    serde_json::to_value(&lyrics).map_err(|e| e.to_string())
}

/// 控制当前播放。
///
/// - `source` = `None` / `"auto"`：按统一优先级打给当前生效来源
/// - `"app"` / `"external"`：强制指定目标
/// - `action`：`play-pause` | `prev` | `next` | `seek`（`value_ms` 仅 seek 使用，毫秒）
///
/// 无播放内容时返回 `target:"none"`（HTTP 200，手机端据此提示即可，不当作错误）。
pub fn control(
    app: &tauri::AppHandle,
    action: &str,
    value_ms: i64,
    source: Option<&str>,
) -> Result<Value, String> {
    let target = match source {
        Some("app") => Some("app"),
        Some("external") => Some("external"),
        _ => match pick() {
            Some(Active::App(..)) => Some("app"),
            Some(Active::External(_)) => Some("external"),
            None => None,
        },
    };

    match target {
        // 软件内播放器活在 WebView 里，只能把命令发回前端执行
        // （走独立事件 remote-music:control，不复用 smtc:control：
        //   后者被 mediaKeysEnabled 门禁与 150ms 去重挡住，语义也不对）
        Some("app") => {
            app.emit(
                "remote-music:control",
                json!({ "action": action, "positionMs": value_ms }),
            )
            .map_err(|e| e.to_string())?;
            Ok(json!({ "ok": true, "target": "app" }))
        }
        Some("external") => {
            crate::external_player::control(action, value_ms);
            Ok(json!({ "ok": true, "target": "external" }))
        }
        _ => Ok(json!({ "ok": true, "target": "none" })),
    }
}

/// 单条封面缓存：(coverKey, 字节, mime)。避免同一张封面被反复下载/解码。
static COVER_CACHE: Mutex<Option<(String, Vec<u8>, &'static str)>> = Mutex::new(None);

/// 依据文件头嗅探图片类型（SMTC 缩略图与各平台封面一般是 png/jpeg/webp）。
fn sniff_mime(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "image/png"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "application/octet-stream"
    }
}

/// 取封面字节。`k` 必须等于**当前生效来源**的封面 hash，否则返回 None
/// （既拦下过期 key，也杜绝通过该接口读取任意本机文件）。
///
/// **阻塞**（网络下载 + 图片解码），只能在 `spawn_blocking` 中调用。
pub fn cover_bytes(k: &str) -> Option<(Vec<u8>, &'static str)> {
    let src = active_cover_source()?;
    if cover_key(&src) != k {
        return None;
    }
    if let Ok(cache) = COVER_CACHE.lock() {
        if let Some((ck, bytes, mime)) = cache.as_ref() {
            if ck == k {
                return Some((bytes.clone(), *mime));
            }
        }
    }
    let bytes = crate::smtc::cover_bytes_for(&src)?;
    let mime = sniff_mime(&bytes);
    if let Ok(mut cache) = COVER_CACHE.lock() {
        *cache = Some((k.to_string(), bytes.clone(), mime));
    }
    Some((bytes, mime))
}
