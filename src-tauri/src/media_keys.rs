//! 系统媒体键（键盘 播放/暂停/上一首/下一首）全局捕获
//!
//! 背景：播放器通过前端 Media Session API 注册系统媒体会话，任务栏媒体控件
//! （flyout）按钮与「应用处于后台时」的物理媒体键均可命中会话（ButtonPressed）
//! 正常工作；但当应用处于前台时，硬件媒体键以 WM_APPCOMMAND 发送给前台窗口，
//! WebView2 会吞掉该消息却不做处理（已知限制），导致前台按键无效。
//!
//! 方案（两条互补通道）：
//! 1. RegisterHotKey 全局注册 VK_MEDIA_*（0xB0/0xB1/0xB2/0xB3/0xFA/0xFB）——
//!    焦点无关，普通键盘事件与 HID 消费类控制键（走 APPCOMMAND 路径）都能捕获，
//!    是主通道；命中后 WM_HOTKEY 投递到钩子线程消息队列；
//! 2. WH_KEYBOARD_LL 低层键盘钩子——兜底捕获以普通键盘事件形式送达的媒体键。
//!
//! 会话开关 set_music_session_active：仅当有歌曲（含暂停，由前端在歌曲切换时上报）
//! 才注册热键 / 消费按键；无歌曲时注销热键并放行，避免抢占其他应用的媒体键。
//! 命中后统一转发为 'music-hotkey' 事件（与全局热键同一通道，前端 music store 处理），
//! 窗口打开与最小化到托盘两种状态下都有效。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::AppHandle;

#[cfg(target_os = "windows")]
use tauri::Emitter;

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey};
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, KBDLLHOOKSTRUCT, MSG, PM_REMOVE, PeekMessageW, SetWindowsHookExW,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_HOTKEY, WM_KEYDOWN, WM_SYSKEYDOWN,
};

#[cfg(target_os = "windows")]
static HOOK_RUNNING: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "windows")]
static HOOK_STOP: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "windows")]
static APP: OnceLock<AppHandle> = OnceLock::new();
/// 音乐会话是否活跃（前端通过 set_music_session_active 上报：有歌曲即视为活跃，
/// 包括暂停状态）。仅活跃时注册热键 / 消费按键，避免抢占其他应用的媒体键。
#[cfg(target_os = "windows")]
static SESSION_ACTIVE: AtomicBool = AtomicBool::new(false);
/// LL 钩子自动重复过滤：上次已消费的 (vk, time)，250ms 内同键只处理一次
#[cfg(target_os = "windows")]
static LAST_KEY: Mutex<(u32, u32)> = Mutex::new((0, 0));
/// 跨通道去重：同一动作 150ms 内只转发一次（LL 钩子与热键可能同时命中同一按键）
#[cfg(target_os = "windows")]
static LAST_EMIT: Mutex<(u32, u128)> = Mutex::new((0, 0));

#[cfg(target_os = "windows")]
const VK_MEDIA_NEXT_TRACK: u32 = 0xB0;
#[cfg(target_os = "windows")]
const VK_MEDIA_PREV_TRACK: u32 = 0xB1;
#[cfg(target_os = "windows")]
const VK_MEDIA_STOP: u32 = 0xB2;
#[cfg(target_os = "windows")]
const VK_MEDIA_PLAY_PAUSE: u32 = 0xB3;
#[cfg(target_os = "windows")]
const VK_MEDIA_PLAY: u32 = 0xFA;
#[cfg(target_os = "windows")]
const VK_MEDIA_PAUSE: u32 = 0xFB;

/// (热键 id, VK 码)：热键 id 直接复用 VK 码，WM_HOTKEY 的 wParam 即 VK 码
#[cfg(target_os = "windows")]
const MEDIA_KEYS: [(i32, u32); 6] = [
    (VK_MEDIA_NEXT_TRACK as i32, VK_MEDIA_NEXT_TRACK),
    (VK_MEDIA_PREV_TRACK as i32, VK_MEDIA_PREV_TRACK),
    (VK_MEDIA_STOP as i32, VK_MEDIA_STOP),
    (VK_MEDIA_PLAY_PAUSE as i32, VK_MEDIA_PLAY_PAUSE),
    (VK_MEDIA_PLAY as i32, VK_MEDIA_PLAY),
    (VK_MEDIA_PAUSE as i32, VK_MEDIA_PAUSE),
];

/// 前端上报音乐会话活跃状态（有歌曲加载即 true，含暂停；无歌曲 false）。
#[tauri::command]
pub fn set_music_session_active(active: bool) {
    #[cfg(target_os = "windows")]
    {
        SESSION_ACTIVE.store(active, Ordering::Relaxed);
        log::info!("[media-keys] 音乐会话活跃状态 -> {active}");
    }
    #[cfg(not(target_os = "windows"))]
    let _ = active;
}

/// 初始化：安装媒体键捕获（低层键盘钩子 + 全局热键），幂等。
pub fn init(app: &AppHandle) {
    #[cfg(target_os = "windows")]
    {
        if HOOK_RUNNING.load(Ordering::Relaxed) {
            return;
        }
        let _ = APP.set(app.clone());
        HOOK_STOP.store(false, Ordering::Relaxed);
        std::thread::Builder::new()
            .name("nexbox-media-keys".into())
            .spawn(hook_thread)
            .ok();
    }
    #[cfg(not(target_os = "windows"))]
    let _ = app;
}

/// 退出时卸载钩子与热键（进程退出时系统也会自动清理，这里仅作显式兜底）。
pub fn cleanup() {
    #[cfg(target_os = "windows")]
    HOOK_STOP.store(true, Ordering::Relaxed);
}

/// 把媒体键 VK 码映射为 music-hotkey 动作（与全局热键动作一致）。
#[cfg(target_os = "windows")]
fn media_action_for_vk(vk: u32) -> Option<&'static str> {
    match vk {
        VK_MEDIA_NEXT_TRACK => Some("next"),
        VK_MEDIA_PREV_TRACK => Some("prev"),
        // 停止：播放器没有独立停止命令，按暂停处理
        VK_MEDIA_STOP => Some("pause"),
        VK_MEDIA_PLAY_PAUSE => Some("play-pause"),
        VK_MEDIA_PLAY => Some("play"),
        VK_MEDIA_PAUSE => Some("pause"),
        _ => None,
    }
}

/// 转发动作到前端（跨通道去重后）。
#[cfg(target_os = "windows")]
fn emit_action(action: &'static str) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let key = match action {
        "next" => 1,
        "prev" => 2,
        "play-pause" => 3,
        "play" => 4,
        "pause" => 5,
        _ => 0,
    };
    let mut last = match LAST_EMIT.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    if last.0 == key && now.wrapping_sub(last.1) < 150 {
        return;
    }
    *last = (key, now);
    drop(last);
    if let Some(app) = APP.get() {
        let _ = app.emit("music-hotkey", serde_json::json!({ "action": action }));
    }
    log::info!("[media-keys] emit action: {action}");
}

/// 低层键盘钩子回调：兜底捕获以键盘事件形式送达的媒体键。
#[cfg(target_os = "windows")]
unsafe extern "system" fn keyboard_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if n_code >= 0 && (w_param == WM_KEYDOWN as usize || w_param == WM_SYSKEYDOWN as usize) {
        let kb = l_param as *const KBDLLHOOKSTRUCT;
        if !kb.is_null() {
            let vk = (*kb).vkCode;
            if let Some(action) = media_action_for_vk(vk) {
                // 音乐会话不存在时放行，不抢占其他应用的媒体键
                if SESSION_ACTIVE.load(Ordering::Relaxed) {
                    // 键盘自动重复过滤：同一按键 250ms 内只处理一次
                    let t = (*kb).time;
                    let mut last = match LAST_KEY.lock() {
                        Ok(g) => g,
                        Err(poisoned) => poisoned.into_inner(),
                    };
                    if last.0 == vk && t.wrapping_sub(last.1) < 250 {
                        return 1; // 重复触发也吞掉，防止透传给前台应用造成双响
                    }
                    *last = (vk, t);
                    drop(last);
                    emit_action(action);
                    return 1; // 消费该按键
                }
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param)
}

/// 钩子线程体：安装低层键盘钩子，常驻消息循环处理 WM_HOTKEY，
/// 并随会话活跃状态动态注册/注销媒体键全局热键。
#[cfg(target_os = "windows")]
fn hook_thread() {
    unsafe {
        if HOOK_RUNNING.load(Ordering::Relaxed) {
            return;
        }
        // 1. 低层键盘钩子（兜底通道）
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), std::ptr::null_mut(), 0);
        if hook.is_null() {
            log::error!("[media-keys] 安装低层键盘钩子失败");
        } else {
            HOOK_RUNNING.store(true, Ordering::Relaxed);
            log::info!("[media-keys] 低层键盘钩子已安装");
        }

        // 2. 消息循环 + 热键动态注册（主通道）
        let mut msg: MSG = std::mem::zeroed();
        let mut hotkeys_registered = false;
        while !HOOK_STOP.load(Ordering::Relaxed) {
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_HOTKEY {
                    let id = msg.wParam as u32;
                    if SESSION_ACTIVE.load(Ordering::Relaxed) {
                        if let Some(action) = media_action_for_vk(id) {
                            emit_action(action);
                        }
                    }
                }
            }

            let active = SESSION_ACTIVE.load(Ordering::Relaxed);
            if active && !hotkeys_registered {
                for &(id, vk) in &MEDIA_KEYS {
                    let ok = RegisterHotKey(std::ptr::null_mut(), id, MOD_NOREPEAT, vk);
                    log::info!(
                        "[media-keys] 注册媒体键热键 vk=0x{vk:x} -> {}",
                        if ok != 0 { "OK" } else { "失败(可能已被其他应用占用)" }
                    );
                }
                hotkeys_registered = true;
            } else if !active && hotkeys_registered {
                for &(id, _) in &MEDIA_KEYS {
                    UnregisterHotKey(std::ptr::null_mut(), id);
                }
                log::info!("[media-keys] 已注销媒体键热键（无歌曲，按键放行）");
                hotkeys_registered = false;
            }

            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        // 3. 清理
        if !hook.is_null() {
            UnhookWindowsHookEx(hook);
            HOOK_RUNNING.store(false, Ordering::Relaxed);
        }
        if hotkeys_registered {
            for &(id, _) in &MEDIA_KEYS {
                UnregisterHotKey(std::ptr::null_mut(), id);
            }
        }
        log::info!("[media-keys] 钩子与热键已清理");
    }
}
