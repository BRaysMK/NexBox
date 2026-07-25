//! FPS 监控模块 — 基于 PresentMon 2.5.1 控制台进程方案
//!
//! 通过启动 PresentMon.exe 子进程，读取 stdout 管道中的 CSV 帧数据，
//! 动态解析表头获取 `MsBetweenPresents` 列，按前台进程名过滤，
//! 使用 EMA 指数移动平均计算平滑 FPS。
//!
//! 架构说明：
//! - 读取线程：阻塞式 reader.lines() 读 stdout 管道，通过 channel 发送到主线程
//! - 主线程：recv_timeout(200ms) 非阻塞接收，即使 PresentMon 停止输出也不会冻结
//! - 看门狗（poller 线程）：检测 line_count 连续 10 秒不增长 → 设置 RESTART_FLAG
//! - 主线程检测到 RESTART_FLAG → break → 终止 PresentMon → 管道 EOF → 读取线程退出 → 重启
//!
//! 优势：
//! - 支持 DirectX 9/10/11/12、OpenGL、Vulkan 全部图形 API
//! - 微软官方分析库，数据帧级精确
//! - 无需手写 ETW，代码简洁易维护
//! - 看门狗自动恢复：PresentMon 长时间运行后若停止输出，10 秒内自动重启

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::OnceLock;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

// ============ 全局状态 ============

/// 平滑后的 FPS 值，供 overlay 读取
static SMOOTHED_FPS: AtomicU32 = AtomicU32::new(0);
/// FPS 监控是否处于活跃状态
static FPS_ACTIVE: AtomicBool = AtomicBool::new(false);
/// PresentMon exe 路径缓存（避免每次启动 session 都重新查找并打印日志）
static PM_EXE_CACHE: OnceLock<Option<PathBuf>> = OnceLock::new();
/// 自身 overlay 窗口句柄（用于排除前台切换到自身 overlay）
static OVERLAY_HWND: AtomicU64 = AtomicU64::new(0);
/// 当前前台目标进程名（小写 exe 文件名，如 "game.exe"）
static TARGET_PROCESS_NAME: Mutex<String> = Mutex::new(String::new());
/// 当前前台窗口 PID（钩子回调中快速存储，后台线程查询进程名）
static CURRENT_FG_PID: AtomicU32 = AtomicU32::new(0);
/// PresentMon 子进程句柄（用于停止时 kill）
static CHILD_HANDLE: Mutex<Option<Child>> = Mutex::new(None);
/// 上次匹配到目标帧数据的时间戳（ms），用于检测 FPS 卡住
static LAST_MATCH_TIMESTAMP: AtomicU64 = AtomicU64::new(0);
/// 主循环已处理的总行数（供看门狗线程检查 PresentMon 是否无输出）
static LINE_COUNT_ATOMIC: AtomicU64 = AtomicU64::new(0);
/// 重启标志（看门狗设置，主循环检查后退出当前 session 触发重启）
static RESTART_FLAG: AtomicBool = AtomicBool::new(false);
/// 前台切换时请求重启 PresentMon（定向监控需要用新 --process_name 重新启动）
static RESTART_REQUESTED: AtomicBool = AtomicBool::new(false);
/// 当前 session 的启动时间（供看门狗判断启动阶段，避免启动期间误触发）
static SESSION_START_INSTANT: Mutex<Option<std::time::Instant>> = Mutex::new(None);

// ============ Windows 前台窗口钩子 ============

#[cfg(target_os = "windows")]
mod win32_fg {
    use super::*;
    use std::ptr;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::UI::Accessibility::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    static HOOK_HANDLE: Mutex<usize> = Mutex::new(0);

    /// 前台窗口切换回调 — 仅存储 PID，不做耗时操作（此回调运行在 overlay 渲染线程上）
    unsafe extern "system" fn on_foreground_changed(
        _hook: HWINEVENTHOOK,
        _event: u32,
        hwnd: HWND,
        _id_object: i32,
        _id_child: i32,
        _id_event_thread: u32,
        _dw_event_time: u32,
    ) {
        if !FPS_ACTIVE.load(Ordering::SeqCst) {
            return;
        }
        // 排除自身 overlay 窗口
        let overlay = OVERLAY_HWND.load(Ordering::Relaxed) as usize;
        if overlay != 0 && hwnd as usize == overlay {
            return;
        }

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid != 0 {
            // 仅原子存储 PID，进程名由后台轮询线程查询
            CURRENT_FG_PID.store(pid, Ordering::Relaxed);
        }
    }

    /// 注册前台窗口切换事件钩子
    pub unsafe fn register_hook() -> bool {
        let hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            ptr::null_mut(),
            Some(on_foreground_changed),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        );
        if !hook.is_null() {
            *HOOK_HANDLE.lock().unwrap() = hook as usize;
            true
        } else {
            log::warn!("FPS监控: 前台窗口 Hook 注册失败");
            false
        }
    }

    /// 注销前台窗口切换事件钩子
    pub unsafe fn unregister_hook() {
        let mut lock = HOOK_HANDLE.lock().unwrap();
        if *lock != 0 {
            UnhookWinEvent(*lock as *mut _);
            *lock = 0;
        }
    }

    /// 初始化时获取当前前台窗口 PID（仅原子存储，进程名由后台线程查询）
    pub fn init_foreground_process() {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid != 0 {
                CURRENT_FG_PID.store(pid, Ordering::Relaxed);
            }
        }
    }
}

// ============ PresentMon 优雅退出 ============

/// 尝试向 PresentMon 的消息窗口发送 WM_QUIT，触发优雅退出
///
/// PresentMon 创建了一个消息专用窗口（类名 "PresentMon"），用于接收热键和定时器消息。
/// 它的主线程运行 GetMessage 循环，收到 WM_QUIT 后会正常退出并调用 ETW 会话清理。
#[cfg(windows)]
fn try_send_quit_to_presentmon() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    let class_name: Vec<u16> = "PresentMon\0".encode_utf16().collect();

    // HWND_MESSAGE = (-3) 作为父窗口，FindWindowExW 可查找消息专用窗口
    let hwnd = unsafe {
        FindWindowExW(
            (-3isize) as _,
            std::ptr::null_mut(),
            class_name.as_ptr(),
            std::ptr::null(),
        )
    };

    if !hwnd.is_null() {
        unsafe {
            PostMessageW(hwnd, WM_QUIT, 0, 0);
        }
        true
    } else {
        false
    }
}

/// 优雅停止 PresentMon 子进程
///
/// 优先通过 WM_QUIT 信号让 PresentMon 自行清理 ETW 会话，
/// 超时后（3秒）才回退到强制终止。
fn stop_presentmon_graceful() {
    let child_opt = CHILD_HANDLE.lock().unwrap().take();
    let mut child = match child_opt {
        Some(c) => c,
        None => return,
    };

    #[cfg(windows)]
    {
        let sent = try_send_quit_to_presentmon();

        if sent {
            // 轮询等待进程退出（每100ms检查一次，最多3秒）
            for _ in 0..30 {
                thread::sleep(Duration::from_millis(100));
                match child.try_wait() {
                    Ok(Some(_)) => {
                        log::info!("FPS监控: PresentMon 优雅退出（ETW 会话已清理）");
                        return;
                    }
                    Ok(None) => continue,
                    Err(_) => break,
                }
            }
            log::warn!("FPS监控: PresentMon 优雅退出超时(3秒)，回退到强制终止");
        } else {
            log::warn!("FPS监控: 未找到 PresentMon 窗口，强制终止");
        }
    }

    let _ = child.kill();
    let _ = child.wait();
}

/// 预清理：停止可能残留的 ETW 会话
///
/// 在启动新的 PresentMon 之前，先执行 --terminate_existing_session
/// 清理上次强杀可能遗留的卡死 ETW 会话。
fn cleanup_stale_etw_session(exe_path: &std::path::Path) {
    let mut cmd = Command::new(exe_path);
    cmd.args(["--terminate_existing_session"]);
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

    match cmd.spawn() {
        Ok(mut child) => {
            let _ = child.wait();
        }
        Err(e) => {
            log::warn!("FPS监控: 预清理 ETW 会话失败: {}", e);
        }
    }
}

// ============ PresentMon 进程管理 ============

/// 查找 PresentMon exe 路径
///
/// 按优先级在以下位置查找（兼容开发模式和生产模式）：
/// 1. 可执行文件同级目录（生产模式）
/// 2. exe_dir/resources/（生产模式 Tauri 打包后）
/// 3. exe_dir/_up_/resources/binaries/（开发模式，_up_ 从 target/debug 跳到 src-tauri）
/// 4. exe_dir/../../resources/binaries/（开发模式备选）
/// 5. 当前工作目录下 resources/binaries/（开发模式 cwd=src-tauri）
/// 6. 当前工作目录下 PresentMon-main/（仓库根目录）
fn find_presentmon_exe() -> Option<PathBuf> {
    // 如果已经查找过，直接返回缓存结果
    if let Some(cached) = PM_EXE_CACHE.get() {
        return cached.clone();
    }

    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let cwd = std::env::current_dir().ok();
    let candidates: Vec<PathBuf> = vec![
        // 生产模式
        exe_dir.join("PresentMon-2.5.1-x64.exe"),
        exe_dir.join("resources").join("PresentMon-2.5.1-x64.exe"),
        exe_dir.join("bin").join("PresentMon-2.5.1-x64.exe"),
        // 开发模式：exe 在 src-tauri/target/debug/ 下
        exe_dir.join("_up_").join("resources").join("binaries").join("PresentMon-2.5.1-x64.exe"),
        exe_dir.join("..").join("..").join("resources").join("binaries").join("PresentMon-2.5.1-x64.exe"),
        // 开发模式：cwd 通常为 src-tauri/
        cwd.as_ref().map(|c| c.join("resources").join("binaries").join("PresentMon-2.5.1-x64.exe")).unwrap_or_default(),
        // 开发模式：仓库根目录
        cwd.as_ref().map(|c| c.join("PresentMon-main").join("PresentMon-2.5.1-x64.exe")).unwrap_or_default(),
        // 开发模式：从 cwd 向上一级（如果 cwd 是 src-tauri，向上一级到仓库根）
        cwd.as_ref().and_then(|c| c.parent()).map(|p| p.join("PresentMon-main").join("PresentMon-2.5.1-x64.exe")).unwrap_or_default(),
    ];

    let result = candidates.iter().find(|c| c.exists()).cloned();
    if let Some(ref p) = result {
        log::info!("FPS监控: 找到 PresentMon → {}", p.display());
    } else {
        log::error!(
            "FPS监控: 未找到 PresentMon-2.5.1-x64.exe，已搜索以下路径: {:?}",
            candidates
        );
    }

    // 存入缓存
    let _ = PM_EXE_CACHE.set(result.clone());
    result
}

/// Session 退出原因
#[derive(Clone, Copy, PartialEq)]
enum SessionExit {
    /// 用户主动停止
    Stopped,
    /// 前台目标切换（计划内重启，不计入失败次数）
    TargetChanged,
    /// 无前台目标或非游戏进程（等待中，不计入失败次数）
    NoTarget,
    /// 管道 EOF / PresentMon 崩溃（计入失败次数）
    PipeError,
    /// 看门狗触发（计入失败次数）
    Watchdog,
}

/// 判断进程是否为非游戏进程（不值得监控 FPS 的系统/开发工具进程）
fn is_non_game_process(name: &str) -> bool {
    const NON_GAME_PROCESSES: &[&str] = &[
        "explorer.exe", "dwm.exe", "windowsterminal.exe", "cmd.exe",
        "powershell.exe", "conhost.exe", "catpawai.exe", "nexbox.exe",
        "msedgewebview2.exe", "searchhost.exe", "shellexperiencehost.exe",
        "sihost.exe", "ctfmon.exe", "textinputhost.exe",
        "applicationframehost.exe", "winlogon.exe", "fontdrvhost.exe",
        "systemsettings.exe", "taskmgr.exe",
    ];
    NON_GAME_PROCESSES.contains(&name)
}

/// PresentMon 读取循环：启动子进程，读取 stdout CSV 数据，解析 FPS
///
/// 架构：读取线程（阻塞 reader.lines()）→ channel → 主线程（recv_timeout 非阻塞）
/// 这样即使 PresentMon 停止输出，主线程也不会冻结，可以检测重启标志并输出诊断。
fn run_presentmon_session() -> SessionExit {
    let exe_path = match find_presentmon_exe() {
        Some(p) => p,
        None => {
            log::error!("FPS监控: 未找到 PresentMon-2.5.1-x64.exe，请确保文件已打包");
            FPS_ACTIVE.store(false, Ordering::SeqCst);
            return SessionExit::PipeError;
        }
    };

    // 读取当前前台目标进程名，用于定向监控
    let target_name = TARGET_PROCESS_NAME.lock().unwrap().clone();

    // 如果没有前台目标（如纯桌面），不启动 PresentMon，等待轮询线程检测到新目标
    if target_name.is_empty() {
        thread::sleep(Duration::from_millis(500));
        return SessionExit::NoTarget;
    }

    // 预清理：停止可能残留的 ETW 会话（防止上次强杀导致会话卡死）
    cleanup_stale_etw_session(&exe_path);

    // 构建 PresentMon 启动命令
    // 注意：不使用 --no_track_display，因为它会禁用 DWM 追踪，
    // 导致 OpenGL/Vulkan 应用（如 Minecraft Java 版）的帧无法被捕获
    let mut cmd = Command::new(&exe_path);
    cmd.args([
        "--output_stdout",         // CSV 输出到 stdout
        "--no_console_stats",      // 关闭控制台统计（防止污染 stdout）
        "--stop_existing_session", // 停止已有同名 ETW 会话
    ]);

    // 定向监控：只追踪前台目标进程，彻底解决 ETW 事件雪崩
    // PresentMon 的 CanonicalizeProcessName 会自动去掉路径和 .exe 后缀
    // 所以传完整进程名（含 .exe）也能正常匹配
    if !target_name.is_empty() {
        cmd.args(["--process_name", &target_name]);
        log::info!("FPS监控: 定向监控目标 → {}", target_name);
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped()); // 捕获 stderr 用于诊断（之前完全丢弃了 PresentMon 的错误信息）
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            log::error!(
                "FPS监控: PresentMon 启动失败: {}（可能需要管理员权限）",
                e
            );
            FPS_ACTIVE.store(false, Ordering::SeqCst);
            return SessionExit::PipeError;
        }
    };

    // 取出 stdout 管道
    let stdout = child.stdout.take().unwrap();

    // 捕获 stderr 用于诊断（过滤掉退出时的正常 ETW 事件丢失警告）
    if let Some(stderr) = child.stderr.take() {
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().flatten() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                // 过滤掉退出时的正常事件丢失警告（主动 kill 时丢包是正常现象）
                if trimmed.contains("ETW events were lost") || trimmed.contains("ETW buffers were lost") {
                    continue;
                }
                log::warn!("FPS监控: PresentMon stderr: {}", trimmed);
            }
        });
    }

    // 存储子进程句柄（用于停止时优雅退出）
    {
        let mut lock = CHILD_HANDLE.lock().unwrap();
        *lock = Some(child);
    }

    // 记录 session 启动时间（供看门狗判断启动阶段）
    *SESSION_START_INSTANT.lock().unwrap() = Some(std::time::Instant::now());

    log::info!("FPS监控: PresentMon 子进程已启动，开始读取 CSV 数据");

    // 清除启动期间可能由 poller 设置的 RESTART_REQUESTED（避免启动竞态导致立即退出）
    RESTART_REQUESTED.store(false, Ordering::SeqCst);

    // ===== 启动读取线程 =====
    // 读取线程阻塞式读取 stdout 管道，通过 channel 发送行到主线程。
    // PresentMon 不输出时，读取线程阻塞在 read_line()，不影响主线程。
    // PresentMon 退出时管道 EOF，读取线程自动退出。
    let (tx, rx) = mpsc::channel::<String>();
    let reader_handle = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line_result in reader.lines() {
            match line_result {
                Ok(line) => {
                    if tx.send(line).is_err() {
                        break; // 主线程退出，channel 断开
                    }
                }
                Err(_) => break,
            }
        }
        // 管道 EOF 或错误，线程自动退出
    });

    // ===== 主循环（非阻塞接收） =====
    // 使用 recv_timeout 替代阻塞式 reader.lines()，
    // 即使 PresentMon 停止输出，主循环也能定期醒来检查重启标志和输出诊断。

    // CSV 表头解析状态
    let mut header_parsed = false;
    let mut app_idx: usize = 0;
    let mut ms_idx: usize = 0;
    let mut header_map: HashMap<String, usize> = HashMap::new();

    // EMA 平滑状态
    let mut smoothed_fps: f64 = 0.0;
    let mut first_frame = true;
    let mut last_target_name = String::new();
    let mut last_match_time = std::time::Instant::now();

    // 统计计数（调试用）
    let mut line_count: u64 = 0;
    let mut matched_count: u64 = 0;
    // 诊断：记录见过的所有应用名
    let mut seen_apps: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut last_diag_time = std::time::Instant::now();

    // Session 退出原因（默认为管道错误）
    let mut exit_reason = SessionExit::PipeError;

    loop {
        // 1. 检查活跃状态
        if !FPS_ACTIVE.load(Ordering::SeqCst) {
            exit_reason = SessionExit::Stopped;
            break;
        }

        // 2. 检查重启标志（看门狗设置）
        if RESTART_FLAG.load(Ordering::Relaxed) {
            RESTART_FLAG.store(false, Ordering::Relaxed);
            log::info!("FPS监控: 看门狗触发重启，退出当前 session（line={}）", line_count);
            exit_reason = SessionExit::Watchdog;
            break;
        }

        // 2b. 检查前台切换重启请求（需要用新 --process_name 重新启动 PresentMon）
        if RESTART_REQUESTED.swap(false, Ordering::SeqCst) {
            log::info!("FPS监控: 前台切换，重启 PresentMon 以更新 --process_name（line={}）", line_count);
            exit_reason = SessionExit::TargetChanged;
            break;
        }

        // 2c. 直接比较目标进程名（兜底：即使 RESTART_REQUESTED 被清除，也能检测到切换）
        {
            let current_target = TARGET_PROCESS_NAME.lock().unwrap().clone();
            if current_target != target_name {
                log::info!("FPS监控: 目标切换 '{}' → '{}'，重启 PresentMon（line={}）", target_name, current_target, line_count);
                exit_reason = SessionExit::TargetChanged;
                break;
            }
        }

        // 3. 非阻塞接收行（200ms 超时）
        match rx.recv_timeout(Duration::from_millis(200)) {
            // 3a. 收到一行 → 处理
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                line_count += 1;
                LINE_COUNT_ATOMIC.store(line_count, Ordering::Relaxed);

                let cols: Vec<&str> = trimmed.split(',').collect();

                // ===== 第一阶段：解析表头 =====
                if !header_parsed {
                    for (i, col) in cols.iter().enumerate() {
                        header_map.insert(col.trim().to_string(), i);
                    }

                    app_idx = match header_map.get("Application") {
                        Some(&idx) => idx,
                        None => {
                            log::error!(
                                "FPS监控: CSV 表头中找不到 'Application' 列，表头 = {:?}",
                                cols
                            );
                            break;
                        }
                    };

                    ms_idx = match header_map.get("MsBetweenPresents") {
                        Some(&idx) => idx,
                        None => {
                            log::error!(
                                "FPS监控: CSV 表头中找不到 'MsBetweenPresents' 列，表头 = {:?}",
                                cols
                            );
                            break;
                        }
                    };

                    header_parsed = true;
                    log::info!(
                        "FPS监控: CSV 表头解析成功 → Application[{}] MsBetweenPresents[{}] (共{}列)",
                        app_idx,
                        ms_idx,
                        cols.len()
                    );
                    continue;
                }

                // ===== 第二阶段：解析数据行 =====
                if app_idx >= cols.len() || ms_idx >= cols.len() {
                    continue;
                }

                let app_name_raw = cols[app_idx].trim();
                let app_name = app_name_raw.to_lowercase();

                // 诊断：记录所有不重复的应用名
                if !seen_apps.contains(&app_name) {
                    seen_apps.insert(app_name.clone());
                    log::info!("FPS监控: 发现应用 [{}] (第{}行)", app_name, line_count);
                }

                let target_name = TARGET_PROCESS_NAME.lock().unwrap().clone();

                // 检测目标进程切换 → 重置 EMA 状态和 FPS
                if target_name != last_target_name {
                    if !last_target_name.is_empty() {
                        log::info!(
                            "FPS监控: 目标切换 '{}' → '{}'，重置 FPS",
                            last_target_name,
                            target_name
                        );
                    }
                    last_target_name = target_name.clone();
                    smoothed_fps = 0.0;
                    first_frame = true;
                    SMOOTHED_FPS.store(0, Ordering::Relaxed);
                    matched_count = 0;
                }

                // 空目标 → 跳过
                if target_name.is_empty() {
                    continue;
                }

                // 进程名匹配：支持带/不带 .exe 后缀，以及 java/javaw 互通
                let target_lower = target_name.to_lowercase();
                let target_no_exe = target_lower.strip_suffix(".exe").unwrap_or(&target_lower);
                let app_no_exe = app_name.strip_suffix(".exe").unwrap_or(&app_name);

                // 特殊处理：java ↔ javaw 互通（Minecraft Java 版可能用 javaw.exe）
                let target_is_java = target_no_exe == "java" || target_no_exe == "javaw";
                let app_is_java = app_no_exe == "java" || app_no_exe == "javaw";

                let is_match = app_name == target_lower
                    || app_no_exe == target_no_exe
                    || (target_is_java && app_is_java);

                if !is_match {
                    continue;
                }

                // 【关键修复】只要匹配到目标进程的行，就立即更新时间戳！
                // 不管这一行的帧时间是否合法，都说明游戏在正常运行，绝对不能误判为无数据。
                LAST_MATCH_TIMESTAMP.store(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                    Ordering::Relaxed,
                );
                last_match_time = std::time::Instant::now();
                matched_count += 1;

                // 解析 MsBetweenPresents（毫秒）
                if let Ok(ms_between_presents) = cols[ms_idx].trim().parse::<f64>() {
                    // 前 5 个匹配行输出原始值用于诊断
                    if matched_count <= 5 {
                        log::info!(
                            "FPS监控: 匹配行#{} MsBetweenPresents={}ms → raw_fps={:.1}",
                            matched_count,
                            ms_between_presents,
                            if ms_between_presents > 0.0 { 1000.0 / ms_between_presents } else { 0.0 }
                        );
                    }

                    // 仅用合理的帧时间计算 EMA（放宽下限到 0.1ms，支持 10000 FPS）
                    if ms_between_presents >= 0.1 && ms_between_presents < 2000.0 {
                        let raw_fps = 1000.0 / ms_between_presents;

                        // EMA 指数移动平均
                        if first_frame {
                            smoothed_fps = raw_fps;
                            first_frame = false;
                        } else {
                            // 平滑系数 0.2：值越小越平滑，越大越灵敏
                            smoothed_fps = 0.2 * raw_fps + 0.8 * smoothed_fps;
                        }

                        SMOOTHED_FPS.store(smoothed_fps.round() as u32, Ordering::Relaxed);
                    }
                }

                // 每秒输出一次统计
                if last_diag_time.elapsed() >= std::time::Duration::from_secs(1) {
                    log::info!(
                        "FPS监控: 统计 line={} matched={} fps={} target={}",
                        line_count,
                        matched_count,
                        SMOOTHED_FPS.load(Ordering::Relaxed),
                        target_name
                    );
                    last_diag_time = std::time::Instant::now();
                }
            }

            // 3b. 200ms 无新行 → 检查重启标志 + 输出诊断（不依赖新行到达！）
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // 诊断日志（每 5 秒，即使无新数据也输出）
                if last_diag_time.elapsed() >= std::time::Duration::from_secs(5) {
                    log::warn!(
                        "FPS监控: 诊断 line={} matched={} fps={} target='{}' seen_apps={:?}",
                        line_count,
                        matched_count,
                        SMOOTHED_FPS.load(Ordering::Relaxed),
                        TARGET_PROCESS_NAME.lock().unwrap(),
                        seen_apps
                    );
                    last_diag_time = std::time::Instant::now();
                }

                // 超时归零：如果 3 秒内没有匹配数据，FPS 归零
                if matched_count > 0 && last_match_time.elapsed() >= std::time::Duration::from_secs(3) {
                    if SMOOTHED_FPS.load(Ordering::Relaxed) != 0 {
                        log::warn!(
                            "FPS监控: 目标 '{}' 3秒无数据，FPS 归零",
                            TARGET_PROCESS_NAME.lock().unwrap()
                        );
                        SMOOTHED_FPS.store(0, Ordering::Relaxed);
                    }
                }
            }

            // 3c. 读取线程退出（管道 EOF / 错误）
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                log::info!(
                    "FPS监控: 管道结束 (line={} matched={})",
                    line_count,
                    matched_count
                );
                exit_reason = SessionExit::PipeError;
                break;
            }
        }
    }

    // 退出 session：终止 PresentMon → 管道 EOF → 读取线程退出
    stop_presentmon_graceful();

    // 等待读取线程退出（PresentMon 已终止，管道 EOF，读取线程会很快退出）
    let _ = reader_handle.join();

    SMOOTHED_FPS.store(0, Ordering::Relaxed);
    log::info!(
        "FPS监控: PresentMon 会话结束 (line={} matched={})",
        line_count,
        matched_count
    );
    exit_reason
}

/// FPS 监控主线程：含自动重启逻辑
fn fps_monitor_main() {
    let mut restart_count = 0u32;
    const MAX_RESTARTS: u32 = 5;

    while FPS_ACTIVE.load(Ordering::SeqCst) {
        let session_start = std::time::Instant::now();
        let exit_reason = run_presentmon_session();

        if !FPS_ACTIVE.load(Ordering::SeqCst) {
            break;
        }

        match exit_reason {
            // 计划内重启：前台切换或无目标，不计入失败次数，不等待
            SessionExit::TargetChanged | SessionExit::NoTarget => {
                restart_count = 0;
                continue;
            }
            // 用户主动停止
            SessionExit::Stopped => {
                break;
            }
            // 失败重启：管道错误或看门狗，计入失败次数
            SessionExit::PipeError | SessionExit::Watchdog => {
                // 上次会话运行超过 30 秒说明是正常运行后断开，重置重启计数器
                if session_start.elapsed() >= Duration::from_secs(30) {
                    restart_count = 0;
                }

                restart_count += 1;
                if restart_count > MAX_RESTARTS {
                    log::error!(
                        "FPS监控: PresentMon 失败重启超过 {} 次，放弃",
                        MAX_RESTARTS
                    );
                    FPS_ACTIVE.store(false, Ordering::SeqCst);
                    break;
                }

                log::warn!(
                    "FPS监控: PresentMon 管道断开，{}秒后重启 (第{}/{}次)",
                    2,
                    restart_count,
                    MAX_RESTARTS
                );
                thread::sleep(Duration::from_secs(2));
            }
        }
    }
}

/// 前台窗口轮询线程（后台线程）
///
/// 负责：
/// 1. 查询 CURRENT_FG_PID 对应的进程名（使用 sysinfo，耗时操作放在此后台线程）
/// 2. 兜底轮询前台窗口（防止 EVENT_SYSTEM_FOREGROUND 漏发）
/// 3. 将进程名写入 TARGET_PROCESS_NAME 供 CSV 过滤使用
/// 4. 看门狗：检测 LINE_COUNT_ATOMIC 连续 10 秒不增长 → 设置 RESTART_FLAG 触发重启
fn foreground_poller_loop() {
    use sysinfo::{Pid, System};
    let mut sys = System::new();
    let mut last_pid: u32 = 0;

    // 看门狗状态
    let mut watchdog_last_lines: u64 = 0;
    let mut watchdog_last_changed = std::time::Instant::now();
    const WATCHDOG_STALL_THRESHOLD: Duration = Duration::from_secs(5);
    const WATCHDOG_WARMUP: Duration = Duration::from_secs(30);

    // 前台切换去抖动状态（避免 Alt+Tab 快速切换时频繁重启 PresentMon）
    let mut pending_name: Option<String> = None;
    let mut pending_since: Option<std::time::Instant> = None;
    const DEBOUNCE_DELAY: Duration = Duration::from_millis(300);

    while FPS_ACTIVE.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(200));
        if !FPS_ACTIVE.load(Ordering::SeqCst) {
            break;
        }

        // 1. 兜底轮询前台窗口（防止 Hook 漏发）
        #[cfg(target_os = "windows")]
        unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::*;
            let hwnd = GetForegroundWindow();
            if !hwnd.is_null() {
                let overlay = OVERLAY_HWND.load(Ordering::Relaxed) as usize;
                if overlay == 0 || hwnd as usize != overlay {
                    let mut pid = 0u32;
                    GetWindowThreadProcessId(hwnd, &mut pid);
                    if pid != 0 {
                        CURRENT_FG_PID.store(pid, Ordering::Relaxed);
                    }
                }
            }
        }

        // 2. 检测 FPS 是否过期（无匹配数据超过 3 秒），仅归零显示，不杀 PresentMon
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let last_match = LAST_MATCH_TIMESTAMP.load(Ordering::Relaxed);
        if last_match > 0 && now > last_match + 3000 {
            let current_fps = SMOOTHED_FPS.load(Ordering::Relaxed);
            if current_fps != 0 {
                log::info!(
                    "FPS监控: 帧率数据已过期 ({}ms 无匹配)，FPS 归零",
                    now - last_match
                );
                SMOOTHED_FPS.store(0, Ordering::Relaxed);
            }
        }

        // 3. 看门狗：检测 PresentMon 是否长时间无输出
        let current_lines = LINE_COUNT_ATOMIC.load(Ordering::Relaxed);
        if current_lines != watchdog_last_lines {
            // line_count 增长了，记录时间
            watchdog_last_lines = current_lines;
            watchdog_last_changed = std::time::Instant::now();
        }

        // 获取 session 启动至今的时间
        let session_age = SESSION_START_INSTANT
            .lock()
            .unwrap()
            .map(|t| t.elapsed())
            .unwrap_or(Duration::from_secs(0));

        // 只在 session 启动 30 秒后才检测（避免启动阶段误触发）
        if session_age > WATCHDOG_WARMUP
            && watchdog_last_changed.elapsed() >= WATCHDOG_STALL_THRESHOLD
            && !RESTART_FLAG.load(Ordering::Relaxed)
        {
            log::warn!(
                "FPS监控: 看门狗触发 — PresentMon {}秒无新数据 (line={})，请求重启",
                watchdog_last_changed.elapsed().as_secs(),
                current_lines
            );
            RESTART_FLAG.store(true, Ordering::Relaxed);
            // 重置 watchdog 计时，避免重启期间重复触发
            watchdog_last_changed = std::time::Instant::now();
        }

        // 4. 查询进程名（耗时操作，在后台线程执行）
        let pid = CURRENT_FG_PID.load(Ordering::Relaxed);
        if pid == 0 {
            continue;
        }

        // PID 变化时才刷新进程列表
        if pid != last_pid {
            sys.refresh_processes();
            last_pid = pid;
        }

        if let Some(process) = sys.process(Pid::from_u32(pid)) {
            let name = process.name().to_string().to_lowercase();
            let current_target = TARGET_PROCESS_NAME.lock().unwrap().clone();

            if name == current_target {
                // 与当前目标一致，取消待提交的变更
                pending_name = None;
                pending_since = None;
            } else {
                // 进程已变化，启动/更新去抖动计时
                if pending_name.as_deref() != Some(name.as_str()) {
                    pending_name = Some(name.clone());
                    pending_since = Some(std::time::Instant::now());
                }

                // 检查去抖动是否到期
                if let (Some(ref pending), Some(since)) = (&pending_name, pending_since) {
                    if since.elapsed() >= DEBOUNCE_DELAY {
                        // 去抖动到期，提交变更
                        let is_game = !pending.is_empty() && !is_non_game_process(pending.as_str());
                        log::info!(
                            "FPS监控: 前台进程 → {} (pid={}){}",
                            pending, pid,
                            if is_game { "" } else { " (非游戏，跳过)" }
                        );

                        {
                            let mut target = TARGET_PROCESS_NAME.lock().unwrap();
                            *target = if is_game { pending.clone() } else { String::new() };
                        }

                        if !is_game {
                            SMOOTHED_FPS.store(0, Ordering::Relaxed);
                        }

                        // 请求重启（游戏→新 --process_name，非游戏→空目标不启动 PresentMon）
                        RESTART_REQUESTED.store(true, Ordering::SeqCst);

                        pending_name = None;
                        pending_since = None;
                    }
                }
            }
        }
    }
}

// ============ 公共 API（保持与旧方案完全兼容） ============

/// 启动 FPS 监控
pub fn start_fps_monitor() {
    if FPS_ACTIVE.load(Ordering::SeqCst) {
        return;
    }
    FPS_ACTIVE.store(true, Ordering::SeqCst);

    #[cfg(target_os = "windows")]
    {
        win32_fg::init_foreground_process();
        unsafe {
            win32_fg::register_hook();
        }
    }

    // 启动 PresentMon 读取主线程
    thread::spawn(|| {
        fps_monitor_main();
    });

    // 启动前台窗口轮询线程（兜底 + 看门狗）
    thread::spawn(|| {
        foreground_poller_loop();
    });

    log::info!("FPS监控: 已启动 (PresentMon 方案 + 看门狗)");
}

/// 停止 FPS 监控
pub fn stop_fps_monitor() {
    if !FPS_ACTIVE.load(Ordering::SeqCst) {
        return;
    }
    FPS_ACTIVE.store(false, Ordering::SeqCst);

    // 优雅停止 PresentMon 子进程（让 ETW 会话被正确清理）
    stop_presentmon_graceful();

    #[cfg(target_os = "windows")]
    unsafe {
        win32_fg::unregister_hook();
    }

    SMOOTHED_FPS.store(0, Ordering::Relaxed);
    LAST_MATCH_TIMESTAMP.store(0, Ordering::Relaxed);
    LINE_COUNT_ATOMIC.store(0, Ordering::Relaxed);
    RESTART_FLAG.store(false, Ordering::Relaxed);
    RESTART_REQUESTED.store(false, Ordering::SeqCst);
    *SESSION_START_INSTANT.lock().unwrap() = None;
    *TARGET_PROCESS_NAME.lock().unwrap() = String::new();
    CURRENT_FG_PID.store(0, Ordering::Relaxed);

    log::info!("FPS监控: 已停止");
}

/// 获取缓存的平滑 FPS 值
pub fn get_cached_fps() -> Option<u32> {
    let fps = SMOOTHED_FPS.load(Ordering::Relaxed);
    if fps == 0 {
        None
    } else {
        Some(fps)
    }
}

/// 设置自身 overlay 窗口句柄（用于排除前台切换到自身 overlay）
pub fn set_overlay_hwnd(hwnd: u64) {
    OVERLAY_HWND.store(hwnd, Ordering::SeqCst);
    log::info!("FPS监控: Overlay窗口句柄设置为 {:#X}", hwnd);
}

/// 清除自身 overlay 窗口句柄
pub fn clear_overlay_hwnd() {
    OVERLAY_HWND.store(0, Ordering::SeqCst);
    log::info!("FPS监控: Overlay窗口句柄已清除");
}

/// 清理资源
pub fn cleanup() {
    stop_fps_monitor();
}
