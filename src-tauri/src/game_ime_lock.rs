//! 游戏启动时自动锁定输入法模块
//!
//! 复用 `game_filter` 的内置 + 自定义游戏名单进行进程轮询。
//! 当检测到名单内游戏运行时：
//! - 通过低层键盘钩子吞掉输入法切换快捷键（Ctrl+Space、Alt+Shift、Win+Space、Ctrl+Shift）
//! - 记录当前键盘布局（HKL）并定时强制恢复，锁定当前输入法
//! 当游戏全部退出时恢复。
//!
//! 线程模型（与 `game_win_key` 保持一致）：
//! - 轮询线程：由 `GENERATION` 代次控制生命周期，游戏启动/退出仅切换锁定标志。
//! - 钩子线程：功能开启期间常驻（安装一次低层键盘钩子）。
//! - 布局看门狗线程：功能开启且游戏运行时周期性强制恢复锁定的键盘布局。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use sysinfo::System;
use tauri_plugin_store::StoreExt;

use crate::game_filter;

/// 轮询间隔（秒）
const POLL_INTERVAL_SECS: u64 = 2;
/// 布局看门狗检查间隔（毫秒）
const HKL_WATCHDOG_MS: u64 = 500;

// ─── 全局状态 ───

/// 功能开关是否开启（内存态）
static ENABLED: AtomicBool = AtomicBool::new(false);
/// 代次：开关切换时 +1，通知旧轮询线程退出
static GENERATION: AtomicU64 = AtomicU64::new(0);

// ─── 配置持久化 ───

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct GameImeLockConfig {
    #[serde(default)]
    pub enabled: bool,
}

async fn load_persisted_config(app: &tauri::AppHandle) -> GameImeLockConfig {
    match app.store("game_ime_lock.json") {
        Ok(store) => {
            if let Some(value) = store.get("config") {
                if let Ok(config) = serde_json::from_value::<GameImeLockConfig>(value) {
                    return config;
                }
            }
        }
        Err(e) => {
            log::warn!("Failed to open game_ime_lock store: {}", e);
        }
    }
    GameImeLockConfig::default()
}

async fn save_persisted_config(app: &tauri::AppHandle, config: &GameImeLockConfig) {
    match app.store("game_ime_lock.json") {
        Ok(store) => {
            store.set("config", serde_json::to_value(config).unwrap());
            if let Err(e) = store.save() {
                log::error!("Failed to save game_ime_lock config: {}", e);
            }
        }
        Err(e) => {
            log::error!("Failed to open game_ime_lock store for saving: {}", e);
        }
    }
}

// ─── Windows 输入法锁定 ───

#[cfg(windows)]
mod win_ime {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};

    use super::HKL_WATCHDOG_MS;
    use winapi::shared::minwindef::{HINSTANCE, HKL__, LPARAM, LRESULT, WPARAM};
    use winapi::shared::windef::{HHOOK, HWND};
    use winapi::um::winuser::{
        ActivateKeyboardLayout, CallNextHookEx, GetAsyncKeyState, GetForegroundWindow,
        GetKeyboardLayout, GetWindowThreadProcessId, PeekMessageW, SetWindowsHookExW,
        UnhookWindowsHookEx, KBDLLHOOKSTRUCT, MSG, PM_REMOVE, WH_KEYBOARD_LL,
    };

    /// 钩子线程退出标志
    static HOOK_STOP: AtomicBool = AtomicBool::new(false);
    /// 钩子线程是否已安装运行
    static HOOK_RUNNING: AtomicBool = AtomicBool::new(false);
    /// 布局看门狗线程退出标志（开关关闭时置 true，立即停止后台强制恢复）
    static WATCHDOG_STOP: AtomicBool = AtomicBool::new(false);
    /// 布局看门狗线程是否运行中（防止重复安装）
    static WATCHDOG_RUNNING: AtomicBool = AtomicBool::new(false);
    /// 是否处于锁定态（游戏运行时为 true）
    static LOCK_ACTIVE: AtomicBool = AtomicBool::new(false);
    /// 已安装的钩子句柄
    static HOOK_HANDLE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
    /// 锁定的键盘布局（HKL）
    static LOCKED_HKL: AtomicUsize = AtomicUsize::new(0);

    pub fn set_lock_active(active: bool) {
        LOCK_ACTIVE.store(active, Ordering::Relaxed);
    }

    pub fn set_hook_stop(stop: bool) {
        HOOK_STOP.store(stop, Ordering::Relaxed);
    }

    pub fn set_watchdog_stop(stop: bool) {
        WATCHDOG_STOP.store(stop, Ordering::Relaxed);
    }

    pub fn watchdog_running() -> bool {
        WATCHDOG_RUNNING.load(Ordering::Relaxed)
    }

    pub fn reset_locked_hkl() {
        LOCKED_HKL.store(0, Ordering::Relaxed);
    }

    pub fn set_locked_hkl(hkl: usize) {
        if hkl != 0 {
            LOCKED_HKL.store(hkl, Ordering::Relaxed);
        }
    }

    pub fn locked_hkl() -> usize {
        LOCKED_HKL.load(Ordering::Relaxed)
    }

    /// 判断修饰键是否按下（cvk 为虚拟键码）
    fn is_down(cvk: i32) -> bool {
        (unsafe { GetAsyncKeyState(cvk) } as i32 & 0x8000) != 0
    }

    /// 低层键盘钩子回调：吞掉输入法切换快捷键
    unsafe extern "system" fn keyboard_proc(
        n_code: i32,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        if n_code >= 0 && LOCK_ACTIVE.load(Ordering::Relaxed) {
            let kb = l_param as *const KBDLLHOOKSTRUCT;
            if !kb.is_null() {
                let vk = (*kb).vkCode;
                // 修饰键当前是否已按下（低层钩子 vkCode 可能是通用码或左右区分码）
                let ctrl = is_down(0x11) || is_down(0xA2) || is_down(0xA3);
                let alt = is_down(0x12) || is_down(0xA4) || is_down(0xA5);
                let shift = is_down(0x10) || is_down(0xA0) || is_down(0xA1);
                let win = is_down(0x5B) || is_down(0x5C); // VK_LWIN / VK_RWIN
                // 本次按下的键是否为某个修饰键（左右变体都算）
                let is_shift_vk = vk == 0x10 || vk == 0xA0 || vk == 0xA1;
                let is_ctrl_vk = vk == 0x11 || vk == 0xA2 || vk == 0xA3;
                let is_alt_vk = vk == 0x12 || vk == 0xA4 || vk == 0xA5;
                // Space：Ctrl+Space 切换中英文；Win+Space 切换语言
                if vk == 0x20 && ((ctrl && !shift) || win) {
                    return 1; // 吞掉按键，不向下传递
                }
                // Shift 后按：Ctrl+Shift / Alt+Shift 切换输入法或键盘布局
                if is_shift_vk && (alt || ctrl) {
                    return 1;
                }
                // Ctrl / Alt 后按（Shift 已按下）：同样吞掉，保证两种按键顺序都拦截
                if (is_ctrl_vk || is_alt_vk) && shift {
                    return 1;
                }
            }
        }
        CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param)
    }

    /// 钩子线程体：安装钩子后常驻消息循环，直到收到退出标志
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
                log::error!("游戏输入法锁定: 安装低层键盘钩子失败");
                return;
            }
            HOOK_HANDLE.store(hook as *mut c_void, Ordering::Relaxed);
            HOOK_RUNNING.store(true, Ordering::Relaxed);
            log::info!("游戏输入法锁定: 低层键盘钩子已安装");

            let mut msg: MSG = std::mem::zeroed();
            while !HOOK_STOP.load(Ordering::Relaxed) {
                while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    // 抽取消息让系统分发低层键盘钩子回调
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }

            UnhookWindowsHookEx(hook as HHOOK);
            HOOK_HANDLE.store(std::ptr::null_mut(), Ordering::Relaxed);
            HOOK_RUNNING.store(false, Ordering::Relaxed);
            log::info!("游戏输入法锁定: 低层键盘钩子已卸载");
        }
    }

    /// 获取前台窗口所属线程的当前键盘布局（HKL），失败返回 None
    unsafe fn foreground_hkl() -> Option<usize> {
        let fg: HWND = GetForegroundWindow();
        let thread_id = GetWindowThreadProcessId(fg, std::ptr::null_mut());
        if thread_id == 0 {
            return None;
        }
        let hkl = GetKeyboardLayout(thread_id);
        if hkl.is_null() {
            None
        } else {
            Some(hkl as usize)
        }
    }

    /// 进入锁定时记录首次捕获的 HKL 作为锁定值
    fn capture_hkl() {
        if locked_hkl() != 0 {
            return;
        }
        unsafe {
            if let Some(hkl) = foreground_hkl() {
                set_locked_hkl(hkl);
            }
        }
    }

    /// 若前台键盘布局被切换走，强制恢复为锁定值
    fn enforce_locked_state() {
        unsafe {
            let locked = locked_hkl();
            if locked == 0 {
                return;
            }
            if let Some(current) = foreground_hkl() {
                if current != locked {
                    // HKL = *mut HKL__，用存储的指针地址恢复锁定的输入法
                    ActivateKeyboardLayout(locked as *mut HKL__, 0);
                }
            }
        }
    }

    /// 布局看门狗线程体：游戏运行时周期性强制恢复锁定的输入法
    /// 收到退出标志时退出，防止开关关闭后仍在后台空转。
    pub fn watchdog_thread() {
        if WATCHDOG_RUNNING.load(Ordering::Relaxed) {
            return;
        }
        WATCHDOG_RUNNING.store(true, Ordering::Relaxed);
        let mut was_active = false;
        loop {
            if WATCHDOG_STOP.load(Ordering::Relaxed) {
                break;
            }
            let active = LOCK_ACTIVE.load(Ordering::Relaxed);
            if active {
                // 刚进入锁定时记录当前布局，此后仅强制恢复
                if !was_active {
                    capture_hkl();
                } else {
                    enforce_locked_state();
                }
            }
            was_active = active;
            std::thread::sleep(std::time::Duration::from_millis(HKL_WATCHDOG_MS));
        }
        WATCHDOG_RUNNING.store(false, Ordering::Relaxed);
    }
}

// ─── 后台轮询线程 ───

fn game_ime_lock_loop(generation: u64) {
    let mut system = System::new();
    let mut game_running = false;

    loop {
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
            #[cfg(windows)]
            win_ime::set_lock_active(true);
            log::info!("游戏输入法锁定: 检测到游戏运行，开始锁定输入法");
        } else if !running && game_running {
            #[cfg(windows)]
            win_ime::set_lock_active(false);
            log::info!("游戏输入法锁定: 游戏已退出，恢复输入法切换");
        }
        game_running = running;
    }
}

// ─── 初始化 / 启动 ───

/// 应用退出时清理：卸载键盘钩子，停止看门狗
pub fn cleanup() {
    #[cfg(windows)]
    {
        win_ime::set_lock_active(false);
        win_ime::set_hook_stop(true);
        win_ime::set_watchdog_stop(true);
        win_ime::reset_locked_hkl();
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
            win_ime::set_hook_stop(false);
            win_ime::set_watchdog_stop(false);
            thread::spawn(move || {
                let _ = std::panic::catch_unwind(win_ime::hook_thread);
            });
            // 看门狗线程随功能开启常驻
            thread::spawn(move || {
                let _ = std::panic::catch_unwind(win_ime::watchdog_thread);
            });
        }
        thread::spawn(move || {
            let _ = std::panic::catch_unwind(|| game_ime_lock_loop(gen));
        });
        log::info!("游戏输入法锁定: 已根据持久化配置启动");
    }
    Ok(())
}

// ─── Tauri 命令 ───

/// 获取开关状态
#[tauri::command]
pub async fn get_game_ime_lock_status() -> Result<bool, String> {
    Ok(ENABLED.load(Ordering::Relaxed))
}

/// 开关切换：开启启动后台线程，关闭停止（代次 +1 使旧轮询线程退出）
#[tauri::command]
pub async fn set_game_ime_lock_enabled(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let current = ENABLED.load(Ordering::Relaxed);
    if current == enabled {
        return Ok(());
    }

    let gen = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    ENABLED.store(enabled, Ordering::Relaxed);

    // 持久化
    let config = GameImeLockConfig { enabled };
    save_persisted_config(&app, &config).await;

    if enabled {
        #[cfg(windows)]
        {
            win_ime::set_lock_active(false);
            win_ime::set_hook_stop(false);
            win_ime::set_watchdog_stop(false);
            thread::spawn(move || {
                let _ = std::panic::catch_unwind(win_ime::hook_thread);
            });
            // 若旧看门狗线程仍在运行则复用，否则重新启动
            if !win_ime::watchdog_running() {
                thread::spawn(move || {
                    let _ = std::panic::catch_unwind(win_ime::watchdog_thread);
                });
            }
        }
        thread::spawn(move || {
            let _ = std::panic::catch_unwind(|| game_ime_lock_loop(gen));
        });
        log::info!("游戏输入法锁定: 已开启");
    } else {
        // 关闭开关：立即解除锁定并停止全部后台活动，下次开启重新捕获输入法
        #[cfg(windows)]
        {
            win_ime::set_lock_active(false);
            win_ime::set_hook_stop(true);
            win_ime::set_watchdog_stop(true);
            win_ime::reset_locked_hkl();
        }
        log::info!("游戏输入法锁定: 已关闭");
    }
    Ok(())
}