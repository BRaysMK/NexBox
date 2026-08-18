//! 系统媒体键（键盘 播放/暂停/上一首/下一首）全局捕获
//!
//! 背景：SMTC 会话绑定在进程级隐藏宿主窗口上，而 Windows 的硬件媒体键
//! 只以 WM_APPCOMMAND 发送给「前台窗口」。隐藏宿主窗口永远不会获得焦点，
//! 主窗口又没有绑定 SMTC，因此键盘媒体键完全收不到（任务栏飞控面板上
//! 点按钮不受影响——那是直接命中会话的 ButtonPressed）。
//!
//! 方案：低层键盘钩子（WH_KEYBOARD_LL）全局捕获媒体键 VK 码，在音乐引擎
//! 存在播放源时消费按键并转发为 'smtc-control' 事件（与 SMTC 按钮事件同一
//! 通道：主窗口存活时由前端 store 处理，销毁后由 player::register_control_listener
//! 在 Rust 侧接管），保证窗口打开与最小化到托盘两种状态下键盘媒体键都有效。
//!
//! 音乐引擎无播放源时按键原样放行，避免抢占其他应用的媒体键。

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Mutex, OnceLock};

use tauri::AppHandle;

#[cfg(target_os = "windows")]
use tauri::Emitter;

#[cfg(target_os = "windows")]
use winapi::shared::minwindef::{HINSTANCE, LPARAM, LRESULT, WPARAM};
#[cfg(target_os = "windows")]
use winapi::shared::windef::HHOOK;
#[cfg(target_os = "windows")]
use winapi::um::winuser::{
    CallNextHookEx, PeekMessageW, SetWindowsHookExW, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, MSG,
    PM_REMOVE, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};

#[cfg(target_os = "windows")]
static HOOK_RUNNING: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "windows")]
static HOOK_STOP: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "windows")]
static HOOK_HANDLE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
#[cfg(target_os = "windows")]
static APP: OnceLock<AppHandle> = OnceLock::new();
/// 上次已消费的 (vk, time)，用于过滤键盘自动重复，避免按住不放时反复触发切换
#[cfg(target_os = "windows")]
static LAST_KEY: Mutex<(u32, u32)> = Mutex::new((0, 0));

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

/// 初始化：安装全局媒体键钩子（幂等）。
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

/// 退出时卸载钩子（进程退出时系统也会自动清理，这里仅作显式兜底）。
pub fn cleanup() {
    #[cfg(target_os = "windows")]
    HOOK_STOP.store(true, Ordering::Relaxed);
}

/// 把媒体键 VK 码映射为 smtc-control 动作。
/// play/pause 语义按当前引擎播放状态动态决定（与任务栏飞控一致）。
#[cfg(target_os = "windows")]
fn media_action_for_vk(vk: u32) -> Option<&'static str> {
    match vk {
        VK_MEDIA_NEXT_TRACK => Some("next"),
        VK_MEDIA_PREV_TRACK => Some("prev"),
        // 停止：引擎没有独立停止命令，按暂停处理
        VK_MEDIA_STOP => Some("pause"),
        VK_MEDIA_PLAY_PAUSE => {
            if crate::player::get_last_state().is_playing {
                Some("pause")
            } else {
                Some("play")
            }
        }
        VK_MEDIA_PLAY => Some("play"),
        VK_MEDIA_PAUSE => Some("pause"),
        _ => None,
    }
}

/// 低层键盘钩子回调：捕获媒体键并转发给音乐引擎。
#[cfg(target_os = "windows")]
unsafe extern "system" fn keyboard_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if n_code >= 0 && (w_param == WM_KEYDOWN as usize || w_param == WM_SYSKEYDOWN as usize) {
        let kb = l_param as *const KBDLLHOOKSTRUCT;
        if !kb.is_null() {
            let vk = (*kb).vkCode;
            if let Some(action) = media_action_for_vk(vk) {
                // 引擎无播放源（从未播过 / 已停止）时放行，不抢占其他应用的媒体键
                let st = crate::player::get_last_state();
                let active = !st.current_src.is_empty() || st.current_song.is_some();
                if active {
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
                    if let Some(app) = APP.get() {
                        let _ = app.emit("smtc-control", serde_json::json!({ "action": action }));
                    }
                    log::debug!("[media-keys] consumed media key vk=0x{vk:x} -> {action}");
                    return 1; // 消费该按键
                }
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param)
}

/// 钩子线程体：安装钩子后常驻消息循环（低层钩子回调依赖线程抽取消息分发）。
#[cfg(target_os = "windows")]
fn hook_thread() {
    unsafe {
        if HOOK_RUNNING.load(Ordering::Relaxed) {
            return;
        }
        let hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(keyboard_proc),
            std::ptr::null::<c_void>() as HINSTANCE,
            0,
        );
        if hook.is_null() {
            log::error!("[media-keys] 安装低层键盘钩子失败");
            return;
        }
        HOOK_HANDLE.store(hook as *mut c_void, Ordering::Relaxed);
        HOOK_RUNNING.store(true, Ordering::Relaxed);
        log::info!("[media-keys] 系统媒体键钩子已安装");

        let mut msg: MSG = std::mem::zeroed();
        while !HOOK_STOP.load(Ordering::Relaxed) {
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                // 抽取消息让系统有机会分发低层键盘钩子回调
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        UnhookWindowsHookEx(hook as HHOOK);
        HOOK_HANDLE.store(std::ptr::null_mut(), Ordering::Relaxed);
        HOOK_RUNNING.store(false, Ordering::Relaxed);
        log::info!("[media-keys] 系统媒体键钩子已卸载");
    }
}
