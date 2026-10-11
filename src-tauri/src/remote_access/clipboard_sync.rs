//! 剪贴板同步（PC ↔ 已配对手机 · 纯文本）
//!
//! 方向与开关语义：
//! - 发送（PC → 手机）：轮询本机剪贴板，内容变化即经 WS 广播 `clipboard.update` 帧。
//!   由「剪贴板同步」开关控制（只管发送方向），状态落盘跨重启保持。
//! - 接收（手机 → PC）：手机 POST /api/clipboard 推送文本，写入本机剪贴板。始终开启。
//!
//! 回环抑制：`LAST_HASH` 记录最近一次已知剪贴板内容指纹（FNV-1a 64）。
//! 「本机轮询发现变化」与「收到手机推送写入本机」两条路径都会更新它——
//! 哈希相同即视为已同步，避免 A 写入后又被轮询推回 B 的死循环。
//! 副作用：同一内容重复复制（中间没换过内容）不会重新同步，v1 接受。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::json;
use tauri::Manager;
use tauri_plugin_clipboard_manager::ClipboardExt;

/// 单段文本上限（UTF-8 字节数），与安卓端一致；超限直接不同步
pub const MAX_TEXT_BYTES: usize = 512 * 1024;

/// 本机剪贴板轮询间隔
const POLL_INTERVAL: Duration = Duration::from_secs(1);

static SEND_ON: AtomicBool = AtomicBool::new(false);
/// 最近一次已知剪贴板内容指纹
static LAST_HASH: AtomicU64 = AtomicU64::new(0);
static POLL_TASK: Mutex<Option<tokio::task::JoinHandle<()>>> = Mutex::new(None);

// ───────────────────────── 开关持久化（照搬 mod.rs enabled_flag 模式） ─────────────────────────

fn flag_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    let dir = app.path().app_data_dir().ok()?;
    Some(dir.join("remote-access").join("clipboard-sync.json"))
}

fn persist_flag(app: &tauri::AppHandle, on: bool) {
    let path = match flag_path(app) {
        Some(p) => p,
        None => return,
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let content = json!({ "enabled": on }).to_string();
    if let Err(e) = std::fs::write(&path, content) {
        log::warn!("[ClipboardSync] 持久化开关状态失败: {e}");
    }
}

/// 启动时读取上次是否开启（文件缺失/损坏按未开启处理）
fn load_flag(app: &tauri::AppHandle) {
    let on = flag_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("enabled").and_then(|b| b.as_bool()))
        .unwrap_or(false);
    SEND_ON.store(on, Ordering::Relaxed);
}

/// 发送方向开关当前状态（前端展示用）
pub fn is_send_enabled() -> bool {
    SEND_ON.load(Ordering::Relaxed)
}

// ───────────────────────── 指纹（回环抑制） ─────────────────────────

/// FNV-1a 64 位哈希
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// 「收到远端内容写入本机剪贴板」路径记录指纹，轮询读到同内容即跳过
fn remember_hash(text: &str) {
    LAST_HASH.store(fnv1a64(text.as_bytes()), Ordering::Relaxed);
}

// ───────────────────────── 发送轮询 ─────────────────────────

/// 按当前状态启动轮询（幂等）：仅在服务开启且开关开启时生效。
async fn start_poll() {
    let mut task = POLL_TASK.lock();
    if task.is_some() || !SEND_ON.load(Ordering::Relaxed) || !super::is_enabled() {
        return;
    }
    let Some(app) = super::app_handle() else { return };
    *task = Some(tokio::spawn(poll_loop(app)));
    log::info!("[ClipboardSync] 发送轮询已启动");
}

fn stop_poll() {
    if let Some(h) = POLL_TASK.lock().take() {
        h.abort();
        log::info!("[ClipboardSync] 发送轮询已停止");
    }
}

/// 服务开关关闭时调用（停止轮询；SEND_ON 状态保留，服务重开时自动恢复）
pub fn shutdown_task() {
    stop_poll();
}

async fn poll_loop(app: tauri::AppHandle) {
    let mut tick = tokio::time::interval(POLL_INTERVAL);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        // 服务中途被关或开关被关：空转（重启由 cmd_set/enable 侧驱动）
        if !SEND_ON.load(Ordering::Relaxed) || !super::is_enabled() {
            continue;
        }
        // 读剪贴板是阻塞系统调用（且可能被其他程序短暂占用），挪出 async 上下文，失败静默
        let text = match tokio::task::spawn_blocking({
            let app = app.clone();
            move || app.clipboard().read_text()
        })
        .await
        {
            Ok(Ok(t)) if !t.is_empty() && t.len() <= MAX_TEXT_BYTES => t,
            _ => continue,
        };
        let h = fnv1a64(text.as_bytes());
        if h == LAST_HASH.load(Ordering::Relaxed) {
            continue;
        }
        LAST_HASH.store(h, Ordering::Relaxed);
        let frame = json!({ "type": "clipboard.update", "data": { "text": text } }).to_string();
        super::api::broadcast(frame);
    }
}

// ───────────────────────── 接收（手机 → PC） ─────────────────────────

/// 手机推送落地：先写剪贴板，成功后记指纹（顺序防回环——若先记指纹后写失败，
/// 轮询会把旧内容当新变化推回手机）。
pub async fn write_from_remote(app: &tauri::AppHandle, text: &str) -> Result<(), String> {
    let owned = text.to_string();
    let result = tokio::task::spawn_blocking({
        let app = app.clone();
        move || app.clipboard().write_text(owned)
    })
    .await
    .map_err(|e| format!("join write clipboard: {e}"))?
    .map_err(|e| format!("write clipboard: {e}"));
    if result.is_ok() {
        remember_hash(text);
    }
    result
}

/// 读本机剪贴板文本（GET /api/clipboard 补齐用）；空 / 超限 / 失败统一返回 None。
/// 读剪贴板可能瞬时阻塞（其他程序占用剪贴板），挪到阻塞线程执行。
pub async fn read_local_async() -> Option<String> {
    let app = super::app_handle()?;
    tokio::task::spawn_blocking(move || {
        app.clipboard()
            .read_text()
            .ok()
            .filter(|t| !t.is_empty() && t.len() <= MAX_TEXT_BYTES)
    })
    .await
    .ok()?
}

// ───────────────────────── 生命周期接入 ─────────────────────────

/// mod.rs::init 调：载入持久化开关状态（此时服务未开，轮询不启动；
/// 服务随后经 cmd_enable_remote_access 开启时再由 startup_if_needed 拉起）
pub fn load_persisted(app: &tauri::AppHandle) {
    load_flag(app);
}

/// 服务开启后调用（幂等）：开关开着才真正启动轮询
pub fn startup_if_needed() {
    if SEND_ON.load(Ordering::Relaxed) && super::is_enabled() {
        tauri::async_runtime::spawn(start_poll());
    }
}

// ───────────────────────── Tauri commands ─────────────────────────

#[tauri::command]
pub async fn cmd_get_clipboard_sync() -> Result<bool, String> {
    Ok(is_send_enabled())
}

/// 切换发送方向开关：落盘 + 启停轮询。接收方向不受此开关影响。
#[tauri::command]
pub async fn cmd_set_clipboard_sync(app: tauri::AppHandle, on: bool) -> Result<bool, String> {
    SEND_ON.store(on, Ordering::Relaxed);
    persist_flag(&app, on);
    if on {
        start_poll().await;
    } else {
        stop_poll();
    }
    log::info!("[ClipboardSync] 发送开关 -> {on}");
    Ok(on)
}
