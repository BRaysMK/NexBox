//! 后台音乐播放引擎（rodio + symphonia）
//!
//! 背景：前端 HTMLAudioElement 依赖 WebView2 存活；主窗口销毁(托盘)后音乐会断。
//! 本模块把播放移到 Rust 后端，主窗口销毁后音乐继续播放。
//!
//! 架构：单线程 actor 持有 rodio OutputStream + Sink，通过 mpsc 接收命令；
//! 后台线程每 250ms 广播 player-tick 事件（position/duration/isPlaying），
//! 歌曲自然结束广播 player-ended，出错广播 player-error。
//!
//! 网络歌曲：先按防盗链头下载到缓存文件再播放（与前端 audio 代理同一套 Referer/UA 逻辑）。

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

// ----------------------------------------------------------------
// 命令与状态类型
// ----------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct PlayerState {
    pub is_playing: bool,
    pub position: f64,
    pub duration: f64,
    pub volume: f32,
    pub current_src: String,
    pub loading: bool,
    /// 最近一次由前端告知的“当前歌曲”完整信息（供窗口重建后恢复 UI，保证与引擎实际播放一致）
    pub current_song: Option<serde_json::Value>,
}

/// 播放队列快照条目：前端在队列变化时同步给引擎，
/// 用于主窗口销毁（最小化到托盘）后 SMTC/热键/桌面歌词的上一曲/下一曲继续可用。
/// 在线歌曲的 src 仅当前曲目有值；其余曲目由引擎按 provider 字段在线解析。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueEntry {
    pub id: String,
    pub name: String,
    pub artist: String,
    pub album: String,
    /// 封面来源（http URL / data URI / 本地路径），引擎侧切歌时同步 SMTC 封面
    #[serde(default)]
    pub cover: String,
    pub provider: String,
    /// "local" | "url"
    pub kind: String,
    /// 已解析的播放源（本地路径或音频 URL；在线队列仅当前曲目有值）
    pub src: String,
    /// 时长（秒）
    pub duration: f64,
    /// 音质（与前端 PlaybackQuality 一致）
    pub quality: String,
    // 酷狗 URL 解析字段
    pub hash: String,
    pub album_id: String,
    pub album_audio_id: String,
    pub hq_hash: String,
    pub sq_hash: String,
    pub res_hash: String,
    // QQ 音乐 URL 解析字段
    pub mid: String,
    pub media_mid: String,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum PlayerCmd {
    /// 播放本地文件或网络 URL。kind: "local" | "url"
    Play { kind: String, src: String, seek: f64 },
    Pause,
    Resume,
    Toggle,
    Seek(f64),
    SetVolume(f32),
    Stop,
    /// 下一首（引擎按队列快照推进，主窗口销毁时由 SMTC/热键触发）
    Next,
    /// 上一首
    Prev,
    /// 同步播放队列快照（前端队列变化时）
    SetQueue(Vec<QueueEntry>),
    /// 同步当前播放歌曲（id + 完整 JSON，供窗口重建后恢复 UI）
    SetNowPlaying { id: String, song: serde_json::Value },
    GetState(Sender<PlayerState>),
    Shutdown,
}

/// URL → 缓存文件名（FNV-1a 简单哈希，避免引入额外依赖）
fn url_hash(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}

/// 全局命令通道（setup 时初始化）
static CMD_TX: OnceLock<Sender<PlayerCmd>> = OnceLock::new();
/// 最近一次状态快照（供同步命令快速读取，避免跨线程阻塞）
static LAST_STATE: OnceLock<Mutex<PlayerState>> = OnceLock::new();

fn default_state() -> PlayerState {
    PlayerState {
        is_playing: false,
        position: 0.0,
        duration: 0.0,
        volume: 0.7,
        current_src: String::new(),
        loading: false,
        current_song: None,
    }
}

pub fn get_last_state() -> PlayerState {
    LAST_STATE
        .get()
        .and_then(|m| m.lock().ok())
        .map(|g| g.clone())
        .unwrap_or_else(default_state)
}

fn set_last_state(s: PlayerState) {
    if let Some(m) = LAST_STATE.get() {
        if let Ok(mut g) = m.lock() {
            *g = s;
        }
    }
}

/// 初始化播放引擎（必须在 setup 中调用）
pub fn init(app: AppHandle) {
    if CMD_TX.get().is_some() {
        return;
    }
    LAST_STATE.get_or_init(|| Mutex::new(default_state()));
    let (tx, rx) = mpsc::channel::<PlayerCmd>();
    let _ = CMD_TX.set(tx);
    std::thread::Builder::new()
        .name("nexbox-player".into())
        .spawn(move || actor_main(app, rx))
        .expect("spawn player thread");
}

fn send(cmd: PlayerCmd) {
    if let Some(tx) = CMD_TX.get() {
        let _ = tx.send(cmd);
    }
}

// ----------------------------------------------------------------
// Tauri 命令
// ----------------------------------------------------------------

#[tauri::command]
pub fn player_play(kind: String, src: String, seek: Option<f64>) -> Result<(), String> {
    send(PlayerCmd::Play { kind, src, seek: seek.unwrap_or(0.0) });
    Ok(())
}

#[tauri::command]
pub fn player_pause() -> Result<(), String> {
    send(PlayerCmd::Pause);
    Ok(())
}

#[tauri::command]
pub fn player_resume() -> Result<(), String> {
    send(PlayerCmd::Resume);
    Ok(())
}

#[tauri::command]
pub fn player_toggle() -> Result<(), String> {
    send(PlayerCmd::Toggle);
    Ok(())
}

#[tauri::command]
pub fn player_seek(seconds: f64) -> Result<(), String> {
    send(PlayerCmd::Seek(seconds));
    Ok(())
}

#[tauri::command]
pub fn player_set_volume(volume: f64) -> Result<(), String> {
    send(PlayerCmd::SetVolume(volume as f32));
    Ok(())
}

#[tauri::command]
pub fn player_stop() -> Result<(), String> {
    send(PlayerCmd::Stop);
    Ok(())
}

#[tauri::command]
pub fn player_get_state() -> PlayerState {
    get_last_state()
}

/// 同步播放队列快照到引擎（主窗口销毁后 SMTC/热键/桌面歌词的上下曲依赖它）
#[tauri::command]
pub fn player_set_queue(queue: Vec<QueueEntry>) -> Result<(), String> {
    send(PlayerCmd::SetQueue(queue));
    Ok(())
}

/// 同步当前播放歌曲（完整 JSON）到引擎，供窗口重建后恢复 UI 使用
#[tauri::command]
pub fn player_set_now_playing(song: serde_json::Value) -> Result<(), String> {
    let id = song
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    send(PlayerCmd::SetNowPlaying { id, song });
    Ok(())
}

// ----------------------------------------------------------------
// Actor 主循环
// ----------------------------------------------------------------

const TICK_INTERVAL: Duration = Duration::from_millis(250);

/// 防盗链头：与前端 audio_proxy 保持一致
pub(crate) const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

pub(crate) fn referer_for(url: &str) -> &'static str {
    if url.contains("qq.com") || url.contains("qpic.cn") {
        "https://y.qq.com/"
    } else if url.contains("kugou.com") {
        "https://www.kugou.com/"
    } else {
        "https://music.163.com/"
    }
}

fn actor_main(app: AppHandle, rx: Receiver<PlayerCmd>) {
    // 持有 OutputStream 防止音频设备被释放
    let (_stream, handle) = match OutputStream::try_default() {
        Ok(pair) => pair,
        Err(e) => {
            log::error!("[player] cannot open audio output: {e}");
            // 无音频设备时仍保持 actor 存活，避免命令通道 panic
            let _ = app.emit("player-error", serde_json::json!({ "message": format!("无法打开音频输出: {e}") }));
            return;
        }
    };

    let mut engine = Engine::new(app, handle);
    loop {
        // 处理所有已到达的命令（非阻塞），然后 tick
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                PlayerCmd::Shutdown => {
                    engine.stop_sink();
                    return;
                }
                other => engine.handle(other),
            }
        }
        engine.tick();
        std::thread::sleep(TICK_INTERVAL);
    }
}

struct Engine {
    app: AppHandle,
    handle: OutputStreamHandle,
    sink: Option<Sink>,
    volume: f32,
    current_src: String,
    /// 当前实际播放的文件路径（本地路径或网络下载缓存路径），seek 重建时用
    current_file: Option<PathBuf>,
    loading: bool,
    duration: f64,
    /// 上次 tick 是否在播放（用于 ended 边沿检测）
    was_playing: bool,
    /// 缓存目录（网络歌曲下载）
    cache_dir: PathBuf,
    /// 已下载 URL → 缓存文件路径（进程内去重）
    url_cache: HashMap<String, PathBuf>,
    client: reqwest::blocking::Client,
    last_error: Option<String>,
    /// 播放开始 wall-clock 基准（tick 用）
    _anchor: Instant,
    pause_pos: f64,
    resume_at: Option<Instant>,
    seek_base: f64,
    last_tick_pos: f64,
    playing: bool,
    /// SMTC timeline 节流计数（每 4 tick ≈ 1s 更新一次）
    smtc_tick: u32,
    /// 播放队列快照（主窗口销毁后上下曲/自动续播用）
    queue: Vec<QueueEntry>,
    /// 当前播放歌曲 id（用于在队列快照中定位当前曲目）
    now_playing_id: String,
    /// 当前播放歌曲完整 JSON（前端同步，供窗口重建后恢复 UI）
    now_playing: Option<serde_json::Value>,
}

impl Engine {
    fn new(app: AppHandle, handle: OutputStreamHandle) -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("NexBox")
            .join("audio-cache");
        let _ = std::fs::create_dir_all(&cache_dir);
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("build reqwest client");
        Self {
            app,
            handle,
            sink: None,
            volume: 0.7,
            current_src: String::new(),
            current_file: None,
            loading: false,
            duration: 0.0,
            was_playing: false,
            cache_dir,
            url_cache: HashMap::new(),
            client,
            last_error: None,
            _anchor: Instant::now(),
            pause_pos: 0.0,
            resume_at: None,
            seek_base: 0.0,
            last_tick_pos: 0.0,
            playing: false,
            smtc_tick: 0,
            queue: Vec::new(),
            now_playing_id: String::new(),
            now_playing: None,
        }
    }

    fn stop_sink(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
        self.loading = false;
        self.current_src.clear();
        self.duration = 0.0;
        self.pause_pos = 0.0;
        self.seek_base = 0.0;
        self.playing = false;
        self.was_playing = false;
        self.resume_at = None;
    }

    fn handle(&mut self, cmd: PlayerCmd) {
        match cmd {
            PlayerCmd::Play { kind, src, seek } => self.play(kind, src, seek),
            PlayerCmd::Pause => {
                self.pause();
                crate::smtc::update_playback_state(false);
            }
            PlayerCmd::Resume => {
                self.resume();
                crate::smtc::update_playback_state(self.playing);
            }
            PlayerCmd::Toggle => self.toggle(),
            PlayerCmd::Seek(pos) => self.seek(pos),
            PlayerCmd::SetVolume(v) => {
                self.volume = v.clamp(0.0, 1.0);
                if let Some(sink) = &self.sink {
                    sink.set_volume(self.volume);
                }
            }
            PlayerCmd::Stop => self.stop_sink(),
            PlayerCmd::Next => self.next(),
            PlayerCmd::Prev => self.prev(),
            PlayerCmd::SetQueue(queue) => {
                self.queue = queue;
                log::debug!("[player] queue snapshot updated ({} entries)", self.queue.len());
            }
            PlayerCmd::SetNowPlaying { id, song } => {
                self.now_playing_id = id;
                self.now_playing = Some(song);
                log::debug!("[player] now playing synced: {}", self.now_playing_id);
            }
            PlayerCmd::GetState(tx) => {
                let _ = tx.send(self.snapshot());
            }
            PlayerCmd::Shutdown => {},
        }
        self.push_state();
    }

    fn snapshot(&self) -> PlayerState {
        let (pos, playing) = self.position_and_playing();
        PlayerState {
            is_playing: playing,
            position: pos,
            duration: self.duration,
            volume: self.volume,
            current_src: self.current_src.clone(),
            loading: self.loading,
            current_song: self.now_playing.clone(),
        }
    }

    /// 计算当前播放位置（sink.get_pos 在暂停/seek 后不精确，用 wall-clock 推算）
    fn position_and_playing(&self) -> (f64, bool) {
        if self.sink.is_none() {
            return (0.0, false);
        }
        let sink = self.sink.as_ref().unwrap();
        let paused = sink.is_paused();
        let empty = sink.empty();
        if self.loading || empty {
            return (self.pause_pos, false);
        }
        let playing = !paused;
        let pos = if playing {
            if let Some(t0) = self.resume_at {
                self.pause_pos + t0.elapsed().as_secs_f64()
            } else {
                self.pause_pos
            }
        } else {
            self.pause_pos
        };
        (pos.min(if self.duration > 0.0 { self.duration } else { pos }), playing)
    }

    /// 同步 LAST_STATE 并广播状态（播放状态变化时）
    fn push_state(&self) {
        set_last_state(self.snapshot());
    }

    fn tick(&mut self) {
        // 播放完成检测：sink 存在、非 loading、队列清空（natural end）
        let natural_end = if let Some(sink) = &self.sink {
            !self.loading && sink.empty() && self.was_playing
        } else {
            false
        };
        if natural_end {
            self.playing = false;
            self.was_playing = false;
            self.pause_pos = self.duration.max(0.0);
            // 主窗口已销毁（最小化到托盘）：前端无法收到 ended 事件续播，
            // 由引擎按队列快照自动切到下一首，保证音乐在后台持续播放
            if self.app.get_webview_window("main").is_none() && !self.queue.is_empty() {
                log::info!("[player] natural end (tray mode), auto next: {}", self.current_src);
                self.next();
                return;
            }
            let _ = self.app.emit("player-ended", serde_json::json!({}));
            crate::smtc::update_playback_state(false);
            log::info!("[player] natural end: {}", self.current_src);
            // 清除幽灵源：结束后 current_src 不再指向可播放内容，
            // 前端恢复时能据此判断“引擎已空闲”并自动续播队列
            self.current_src.clear();
            self.current_file = None;
            self.push_state();
            return;
        }

        let (pos, playing) = self.position_and_playing();
        self.last_tick_pos = pos;
        // 只在状态变化或播放中广播，避免无谓事件
        if playing || self.was_playing || self.loading {
            let _ = self.app.emit(
                "player-tick",
                serde_json::json!({
                    "position": pos,
                    "duration": self.duration,
                    "isPlaying": playing,
                    "loading": self.loading,
                }),
            );
        }
        self.was_playing = playing;
        // SMTC 进度更新（约每秒一次，避免频繁 COM 调用）
        self.smtc_tick = self.smtc_tick.wrapping_add(1);
        if self.smtc_tick % 4 == 0 {
            crate::smtc::update_timeline(pos, self.duration);
        }
        if let Some(err) = self.last_error.take() {
            let _ = self.app.emit("player-error", serde_json::json!({ "message": err }));
        }
        self.push_state();
    }

    // ---------- 命令实现 ----------

    fn play(&mut self, kind: String, src: String, seek: f64) {
        if src.is_empty() {
            return;
        }
        // 停掉旧播放
        self.stop_sink();
        self.current_src = src.clone();
        self.loading = true;
        self.push_state();
        let _ = self.app.emit("player-tick", serde_json::json!({ "position": 0.0, "duration": 0.0, "isPlaying": false, "loading": true }));

        // 网络 URL → 下载到缓存
        let path = if kind == "url" {
            match self.download_cached(&src) {
                Ok(p) => p,
                Err(e) => {
                    log::error!("[player] download failed: {e}");
                    self.loading = false;
                    self.last_error = Some(format!("下载音频失败: {e}"));
                    self.push_state();
                    return;
                }
            }
        } else {
            PathBuf::from(&src)
        };

        if !path.is_file() {
            log::error!("[player] file not found: {:?}", path);
            self.loading = false;
            self.last_error = Some("音频文件不存在".to_string());
            self.push_state();
            return;
        }

        let file = match File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                self.loading = false;
                self.last_error = Some(format!("打开音频失败: {e}"));
                self.push_state();
                return;
            }
        };

        // 记录实际播放文件（seek 重建时需要）
        self.current_file = Some(path.clone());

        // symphonia 解码（rodio 0.20 默认）
        let decoder = match Decoder::new(BufReader::new(file)) {
            Ok(d) => d,
            Err(e) => {
                self.loading = false;
                self.last_error = Some(format!("不支持的音频格式: {e}"));
                self.push_state();
                return;
            }
        };

        self.duration = decoder
            .total_duration()
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);

        let sink = match Sink::try_new(&self.handle) {
            Ok(s) => s,
            Err(e) => {
                self.loading = false;
                self.last_error = Some(format!("创建播放器失败: {e}"));
                self.push_state();
                return;
            }
        };
        sink.set_volume(self.volume);
        sink.append(decoder);

        self.pause_pos = seek.max(0.0);
        self.resume_at = Some(Instant::now());
        self.playing = true;
        self.loading = false;

        self.sink = Some(sink);
        // seek 到指定位置（需在 sink 挂载后，Sink::try_seek 要求 &mut self）
        if seek > 0.0 {
            self.seek(seek);
        }
        crate::smtc::update_playback_state(true);
        self.push_state();
        let _ = self.app.emit(
            "player-tick",
            serde_json::json!({ "position": self.pause_pos, "duration": self.duration, "isPlaying": true, "loading": false }),
        );
        log::info!("[player] playing: {} ({}s)", self.current_src, self.duration);
    }

    fn pause(&mut self) {
        if let Some(sink) = &self.sink {
            if !sink.is_paused() {
                // 记录当前位置
                if let Some(t0) = self.resume_at {
                    self.pause_pos += t0.elapsed().as_secs_f64();
                }
                sink.pause();
                self.resume_at = None;
                self.playing = false;
            }
        }
    }

    fn resume(&mut self) {
        if let Some(sink) = &self.sink {
            if sink.is_paused() && !sink.empty() {
                sink.play();
                self.resume_at = Some(Instant::now());
                self.playing = true;
            }
        }
    }

    fn toggle(&mut self) {
        if self.sink.is_none() {
            return;
        }
        if self.playing {
            self.pause();
        } else {
            self.resume();
        }
        // 同步 SMTC 播放状态（热键/桌面歌词在托盘模式走 Toggle 时也需要保持飞控状态正确）
        crate::smtc::update_playback_state(self.playing);
    }

    // ---------- 队列快照：上下曲 / 自动续播（主窗口销毁后接管） ----------

    /// 在当前队列快照中定位当前曲目索引（优先按 src 匹配，其次按歌曲 id）
    fn current_index(&self) -> Option<usize> {
        if self.queue.is_empty() {
            return None;
        }
        if !self.current_src.is_empty() {
            if let Some(i) = self
                .queue
                .iter()
                .position(|e| !e.src.is_empty() && e.src == self.current_src)
            {
                return Some(i);
            }
        }
        if !self.now_playing_id.is_empty() {
            if let Some(i) = self.queue.iter().position(|e| e.id == self.now_playing_id) {
                return Some(i);
            }
        }
        None
    }

    /// 下一首：顺序循环，跳过无法播放的曲目
    fn next(&mut self) {
        let Some(idx) = self.current_index() else { return };
        let n = self.queue.len();
        if n == 0 {
            return;
        }
        for offset in 1..n {
            let candidate = (idx + offset) % n;
            let entry = self.queue[candidate].clone();
            if self.try_play_entry(&entry) {
                return;
            }
        }
        log::warn!("[player] next: queue has no playable entry");
    }

    /// 上一首：播放超过 3 秒先回到当前曲目开头，否则切到上一首
    fn prev(&mut self) {
        let Some(idx) = self.current_index() else { return };
        let (pos, _) = self.position_and_playing();
        if pos > 3.0 {
            self.seek(0.0);
            return;
        }
        let n = self.queue.len();
        if n == 0 {
            return;
        }
        for offset in 1..n {
            let candidate = (idx + n - offset) % n;
            let entry = self.queue[candidate].clone();
            if self.try_play_entry(&entry) {
                return;
            }
        }
        log::warn!("[player] prev: queue has no playable entry");
    }

    /// 尝试播放队列条目：
    /// 已有 src 直接播放；无 src 的在线歌曲走 provider API 在线解析；
    /// 本地歌曲必须带 src，否则视为不可播放。
    /// 成功时同步 now_playing（引擎自动切歌后前端恢复能拿到真实歌曲）。
    fn try_play_entry(&mut self, entry: &QueueEntry) -> bool {
        // 同步当前歌曲信息（引擎侧切歌时前端已销毁，必须由引擎自行维护）
        self.now_playing_id = entry.id.clone();
        self.now_playing = Some(serde_json::json!({
            "provider": entry.provider,
            "id": entry.id,
            "name": entry.name,
            "artist": entry.artist,
            "artists": [],
            "album": entry.album,
            "cover": entry.cover,
            "duration": (entry.duration * 1000.0) as u64,
            "fee": 0,
            "playable": true,
            "language": 0,
            "hash": entry.hash,
            "mid": entry.mid,
            "media_mid": entry.media_mid,
            "_localPath": if entry.kind == "local" { entry.src.clone() } else { String::new() },
        }));

        if !entry.src.is_empty() {
            // 引擎侧切歌（主窗口销毁后）：同步 SMTC 元数据与封面，飞控面板保持正确
            crate::smtc::update_metadata(&entry.name, &entry.artist, &entry.album);
            crate::smtc::update_cover(entry.cover.clone());
            self.play(entry.kind.clone(), entry.src.clone(), 0.0);
            return true;
        }
        if entry.provider == "local" {
            return false;
        }
        match self.resolve_entry_url(entry) {
            Some(url) => {
                log::info!("[player] resolved url for {} ({})", entry.name, entry.provider);
                crate::smtc::update_metadata(&entry.name, &entry.artist, &entry.album);
                crate::smtc::update_cover(entry.cover.clone());
                self.play("url".to_string(), url, 0.0);
                true
            }
            None => {
                log::warn!("[player] resolve url failed for {}", entry.name);
                false
            }
        }
    }

    /// 在线解析队列条目的音频 URL（provider API，与前端一致；携带登录 cookie）
    fn resolve_entry_url(&self, entry: &QueueEntry) -> Option<String> {
        let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
        let app = self.app.clone();
        let entry = entry.clone();
        tauri::async_runtime::spawn(async move {
            let result = match entry.provider.as_str() {
                "netease" => {
                    let cookie = crate::music_api::cookie::load_cookie(&app, "netease").unwrap_or_default();
                    crate::music_api::netease::song_url(&entry.id, &entry.quality, &cookie).await
                }
                "kugou" => {
                    let cookie = crate::music_api::cookie::load_cookie(&app, "kugou").unwrap_or_default();
                    crate::music_api::kugou::song_url(
                        &entry.hash,
                        &entry.album_id,
                        &entry.album_audio_id,
                        &entry.quality,
                        &cookie,
                        &entry.hq_hash,
                        &entry.sq_hash,
                        &entry.res_hash,
                    )
                    .await
                }
                "qqmusic" => {
                    let cookie = crate::music_api::cookie::load_cookie(&app, "qqmusic").unwrap_or_default();
                    crate::music_api::qqmusic::song_url(&entry.mid, &entry.media_mid, &entry.quality, &cookie).await
                }
                _ => return,
            };
            let url = result.ok().and_then(|r| if r.playable { r.url } else { None });
            let _ = tx.send(url);
        });
        rx.recv_timeout(std::time::Duration::from_secs(20)).ok().flatten()
    }

    /// 跳转到指定位置（秒）。rodio 的 Sink::try_seek 对 symphonia 解码器不可靠，
    /// 这里用「重建 Sink + skip_duration」实现：停止当前播放，从目标位置重新解码。
    fn seek(&mut self, pos: f64) {
        let target = pos.max(0.0);
        let Some(file) = self.current_file.clone() else { return };
        // 停止旧 sink
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
        self.playing = false;
        self.was_playing = false;

        let file_handle = match File::open(&file) {
            Ok(f) => f,
            Err(e) => {
                log::warn!("[player] seek reopen failed: {e}");
                return;
            }
        };
        let decoder = match Decoder::new(BufReader::new(file_handle)) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("[player] seek decode failed: {e}");
                return;
            }
        };
        // 跳过到目标位置：skip_duration(self) -> SkipDuration<Self>（消费原 source 返回包装器）
        // 比 try_seek 可靠，mp3/flac 都支持
        let source = if target > 0.0 {
            decoder.skip_duration(Duration::from_secs_f64(target))
        } else {
            decoder.skip_duration(Duration::from_secs(0))
        };
        let sink = match Sink::try_new(&self.handle) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("[player] seek sink failed: {e}");
                return;
            }
        };
        sink.set_volume(self.volume);
        sink.append(source);
        self.sink = Some(sink);
        self.pause_pos = target;
        self.resume_at = Some(Instant::now());
        self.playing = true;
        self.was_playing = true;
        self.push_state();
        let _ = self.app.emit(
            "player-tick",
            serde_json::json!({ "position": target, "duration": self.duration, "isPlaying": true, "loading": false }),
        );
    }

    /// 下载网络音频到缓存（防盗链头）
    fn download_cached(&mut self, url: &str) -> Result<PathBuf, String> {
        if let Some(p) = self.url_cache.get(url) {
            if p.is_file() {
                return Ok(p.clone());
            }
        }
        // 文件名 = url 哈希 + 扩展名（从 content-type 推断）
        let hash = url_hash(url);
        let resp = self
            .client
            .get(url)
            .header("User-Agent", UA)
            .header("Referer", referer_for(url))
            .send()
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let ext = match ct.to_lowercase().as_str() {
            s if s.contains("flac") => "flac",
            s if s.contains("wav") => "wav",
            s if s.contains("ogg") => "ogg",
            s if s.contains("m4a") || s.contains("mp4") => "m4a",
            s if s.contains("aac") => "aac",
            _ => "mp3",
        };
        let dest = self.cache_dir.join(format!("{hash}.{ext}"));
        let bytes = resp.bytes().map_err(|e| e.to_string())?;
        std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
        self.url_cache.insert(url.to_string(), dest.clone());
        Ok(dest)
    }
}

/// 注册全局音乐控制事件监听：主窗口销毁(托盘)后，桌面歌词窗口/热键仍可控制播放
/// 注意：主窗口存活时前端 store 已处理这些事件，这里只在主窗口不存在时接管，避免双触发
pub fn register_control_listener(app: &AppHandle) {
    use tauri::{Listener, Manager};
    // 桌面歌词控制（来自歌词窗口）：主窗口存活时前端处理，销毁后这里接管
    let app2 = app.clone();
    let app2_inner = app2.clone();
    let _ = app2.listen("desktop-lyrics:control", move |event| {
        if app2_inner.get_webview_window("main").is_some() {
            return;
        }
        let payload = event.payload();
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) {
            let action = v.get("action").and_then(|a| a.as_str()).unwrap_or("");
            match action {
                "play-pause" => trigger_toggle(),
                "pause" => trigger_pause(),
                "play" => trigger_resume(),
                "next" => trigger_next(),
                "prev" => trigger_prev(),
                _ => {},
            }
        }
    });
    // 全局热键（主窗口销毁后事件无人接收，直接驱动引擎）
    let app3 = app.clone();
    let app3_inner = app3.clone();
    let _ = app3.listen("music-hotkey", move |event| {
        if app3_inner.get_webview_window("main").is_some() {
            return;
        }
        let payload = event.payload();
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) {
            let action = v.get("action").and_then(|a| a.as_str()).unwrap_or("");
            match action {
                "play-pause" => trigger_toggle(),
                "pause" => trigger_pause(),
                "play" => trigger_resume(),
                "next" => trigger_next(),
                "prev" => trigger_prev(),
                _ => {},
            }
        }
    });
    // SMTC 按钮事件（主窗口存活时前端处理，销毁后这里接管——SMTC 会话常驻，销毁后依然可用）
    let app4 = app.clone();
    let app4_inner = app4.clone();
    let _ = app4.listen("smtc-control", move |event| {
        if app4_inner.get_webview_window("main").is_some() {
            return;
        }
        let payload = event.payload();
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) {
            let action = v.get("action").and_then(|a| a.as_str()).unwrap_or("");
            match action {
                "play" => trigger_resume(),
                "pause" => trigger_pause(),
                "next" => trigger_next(),
                "prev" => trigger_prev(),
                _ => {},
            }
        }
    });
}
// 供外部（如热键）直接触发的便捷函数（部分暂未使用，保留供后续扩展）
#[allow(dead_code)]
pub fn trigger_play(kind: &str, src: &str, seek: f64) {
    send(PlayerCmd::Play { kind: kind.to_string(), src: src.to_string(), seek });
}

pub fn trigger_toggle() {
    send(PlayerCmd::Toggle);
}

pub fn trigger_pause() {
    send(PlayerCmd::Pause);
}

pub fn trigger_resume() {
    send(PlayerCmd::Resume);
}

pub fn trigger_next() {
    // 主窗口存活时由前端队列逻辑处理；此处供主窗口销毁后（SMTC/热键/桌面歌词）驱动引擎队列
    send(PlayerCmd::Next);
}

pub fn trigger_prev() {
    send(PlayerCmd::Prev);
}
