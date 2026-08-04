//! 游戏启动时自动禁用 Win 键模块
//!
//! 复用 `game_filter` 的内置 + 自定义游戏名单进行进程轮询。
//! 当检测到名单内游戏运行时，拦截左右 Win 键（VK_LWIN / VK_RWIN），
//! 防止游戏过程中误触弹出开始菜单；当游戏全部退出时恢复。
//!
//! 线程模型：
//! - 轮询线程：只由 `GENERATION` 代次控制生命周期（开关关闭时 +1 退出），
//!   游戏启动/退出仅切换拦截标志，不会自行退出。
//! - 钩子线程：功能开启期间常驻（安装一次低层键盘钩子），用独立全局
//!   `HOOK_STOP` 标志控制退出，避免反复安装/卸载钩子。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use sysinfo::System;
use tauri_plugin_store::StoreExt;

use crate::game_filter;

/// 轮询间隔（秒）
const POLL_INTERVAL_SECS: u64 = 2;

// ─── 全局状态 ───

/// 功能开关是否开启（内存态，供轮询线程与状态查询读取）
static ENABLED: AtomicBool = AtomicBool::new(false);
/// 代次：开关切换时 +1，通知旧轮询线程退出
static GENERATION: AtomicU64 = AtomicU64::new(0);

// ─── 配置持久化 ───

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct GameWinKeyConfig {
    #[serde(default)]
    pub enabled: bool,
}

async fn load_persisted_config(app: &tauri::AppHandle) -> GameWinKeyConfig {
    match app.store("game_win_key.json") {
        Ok(store) => {
            if let Some(value) = store.get("config") {
                if let Ok(config) = serde_json::from_value::<GameWinKeyConfig>(value) {
                    return config;
                }
            }
        }
        Err(e) => {
            log::warn!("Failed to open game_win_key store: {}", e);
        }
    }
    GameWinKeyConfig::default()
}

async fn save_persisted_config(app: &tauri::AppHandle, config: &GameWinKeyConfig) {
    match app.store("game_win_key.json") {
        Ok(store) => {
            store.set("config", serde_json::to_value(config).unwrap());
            if let Err(e) = store.save() {
                log::error!("Failed to save game_win_key config: {}", e);
            }
        }
        Err(e) => {
            log::error!("Failed to open game_win_key store for saving: {}", e);
        }
    }
}

// ─── Windows 低层键盘钩子 ───

#[cfg(windows)]
mod win_hook {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

    use winapi::shared::minwindef::{HINSTANCE, LPARAM, LRESULT, WPARAM};
    use winapi::shared::windef::HHOOK;
    use winapi::um::winuser::{
        CallNextHookEx, PeekMessageW, SetWindowsHookExW, UnhookWindowsHookEx, KBDLLHOOKSTRUCT,
        MSG, PM_REMOVE, VK_LWIN, VK_RWIN, WH_KEYBOARD_LL,
    };

    /// 钩子线程退出标志（功能关闭 / 应用退出时置 true）
    static HOOK_STOP: AtomicBool = AtomicBool::new(false);
    /// 钩子线程是否已安装钩子并运行中（防止重复安装）
    static HOOK_RUNNING: AtomicBool = AtomicBool::new(false);
    /// 是否拦截 Win 键（游戏运行时为 true）
    static BLOCK_WIN: AtomicBool = AtomicBool::new(false);
    /// 已安装的钩子句柄（用于卸载）
    static HOOK_HANDLE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

    pub fn set_block(block: bool) {
        BLOCK_WIN.store(block, Ordering::Relaxed);
    }

    pub fn set_hook_stop(stop: bool) {
        HOOK_STOP.store(stop, Ordering::Relaxed);
    }

    /// 低层键盘钩子回调：拦截左右 Win 键
    unsafe extern "system" fn keyboard_proc(
        n_code: i32,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        if n_code >= 0 && BLOCK_WIN.load(Ordering::Relaxed) {
            let kb = l_param as *const KBDLLHOOKSTRUCT;
            if !kb.is_null() {
                let vk = (*kb).vkCode;
                // VK_LWIN = 0x5B, VK_RWIN = 0x5C
                if vk == VK_LWIN as u32 || vk == VK_RWIN as u32 {
                    // 返回 1 表示吞掉该按键，不向下传递
                    return 1;
                }
            }
        }
        // 其余按键照常传递
        CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param)
    }

    /// 钩子线程体：安装钩子后常驻消息循环，直到收到退出标志。
    /// 低层键盘钩子的回调依赖线程的消息队列抽取消息时被系统分发，
    /// 用 PeekMessageW 非阻塞抽取 + 短 sleep，既保证回调被分发又及时响应退出标志。
    pub fn hook_thread() {
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
                log::error!("游戏Win键禁用: 安装低层键盘钩子失败");
                return;
            }
            HOOK_HANDLE.store(hook as *mut c_void, Ordering::Relaxed);
            HOOK_RUNNING.store(true, Ordering::Relaxed);
            log::info!("游戏Win键禁用: 低层键盘钩子已安装");

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
            log::info!("游戏Win键禁用: 低层键盘钩子已卸载");
        }
    }
}

// ─── 后台轮询线程 ───

fn game_win_key_loop(generation: u64) {
    let mut system = System::new();
    let mut game_running = false;

    loop {
        // 生命周期只由代次控制：开关关闭时 GENERATION +1，本线程退出
        if GENERATION.load(Ordering::Relaxed) != generation {
            break;
        }
        thread::sleep(Duration::from_secs(POLL_INTERVAL_SECS));
        if GENERATION.load(Ordering::Relaxed) != generation {
            break;
        }
        if !ENABLED.load(Ordering::Relaxed) {
            game_running = false;
            continue;
        }

        system.refresh_processes();
        let running = game_filter::any_game_running(&system);

        if running && !game_running {
            // 游戏刚启动：开始拦截 Win 键（钩子已由开关开启时常驻）
            #[cfg(windows)]
            win_hook::set_block(true);
            log::info!("游戏Win键禁用: 检测到游戏运行，开始拦截 Win 键");
        } else if !running && game_running {
            // 游戏全部退出：停止拦截（钩子保持安装，等待下次游戏）
            #[cfg(windows)]
            win_hook::set_block(false);
            log::info!("游戏Win键禁用: 游戏已退出，恢复 Win 键");
        }
        game_running = running;
    }
}

// ─── 初始化 / 启动 ───

/// 应用退出时清理：卸载键盘钩子
pub fn cleanup() {
    #[cfg(windows)]
    {
        win_hook::set_block(false);
        win_hook::set_hook_stop(true);
    }
}

/// 应用启动时调用：恢复持久化配置并启动后台线程
pub async fn init(app: tauri::AppHandle) -> Result<(), String> {
    let config = load_persisted_config(&app).await;

    ENABLED.store(config.enabled, Ordering::Relaxed);

    if config.enabled {
        let gen = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
        #[cfg(windows)]
        {
            win_hook::set_hook_stop(false);
            thread::spawn(move || {
                let _ = std::panic::catch_unwind(win_hook::hook_thread);
            });
        }
        thread::spawn(move || {
            let _ = std::panic::catch_unwind(|| game_win_key_loop(gen));
        });
        log::info!("游戏Win键禁用: 已根据持久化配置启动");
    }
    Ok(())
}

// ─── Tauri 命令 ───

/// 获取开关状态
#[tauri::command]
pub async fn get_game_win_key_status() -> Result<bool, String> {
    Ok(ENABLED.load(Ordering::Relaxed))
}

/// 开关切换：开启启动后台线程，关闭停止（代次 +1 使旧轮询线程退出）
#[tauri::command]
pub async fn set_game_win_key_enabled(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let current = ENABLED.load(Ordering::Relaxed);
    if current == enabled {
        return Ok(());
    }

    let gen = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    ENABLED.store(enabled, Ordering::Relaxed);

    // 持久化
    let config = GameWinKeyConfig { enabled };
    save_persisted_config(&app, &config).await;

    if enabled {
        #[cfg(windows)]
        {
            win_hook::set_block(false);
            win_hook::set_hook_stop(false);
            thread::spawn(move || {
                let _ = std::panic::catch_unwind(win_hook::hook_thread);
            });
        }
        thread::spawn(move || {
            let _ = std::panic::catch_unwind(|| game_win_key_loop(gen));
        });
        log::info!("游戏Win键禁用: 已开启");
    } else {
        // 停止拦截并让钩子线程退出
        #[cfg(windows)]
        {
            win_hook::set_block(false);
            win_hook::set_hook_stop(true);
        }
        log::info!("游戏Win键禁用: 已关闭");
    }
    Ok(())
}
