//! FPS 监控模块 — 基于 PresentMon 2.5.1 控制台进程方案
//!
//! 通过启动 PresentMon.exe 子进程，读取 stdout 管道中的 CSV 帧数据，
//! 动态解析表头获取 `MsBetweenPresents` 列，按前台进程名过滤，
//! 使用 EMA 指数移动平均计算平滑 FPS。
//!
//! 优势：
//! - 支持 DirectX 9/10/11/12、OpenGL、Vulkan 全部图形 API
//! - 微软官方分析库，数据帧级精确
//! - 无需手写 ETW，代码简洁易维护

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU32, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

// ============ 全局状态 ============

/// 平滑后的 FPS 值，供 overlay 读取
static SMOOTHED_FPS: AtomicU32 = AtomicU32::new(0);
/// FPS 监控是否处于活跃状态
static FPS_ACTIVE: AtomicBool = AtomicBool::new(false);
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
    for candidate in &candidates {
        if candidate.exists() {
            log::info!("FPS监控: 找到 PresentMon → {}", candidate.display());
            return Some(candidate.clone());
        }
    }
    log::error!(
        "FPS监控: 未找到 PresentMon-2.5.1-x64.exe，已搜索以下路径: {:?}",
        candidates
    );
    None
}

/// PresentMon 读取循环：启动子进程，读取 stdout CSV 数据，解析 FPS
fn run_presentmon_session() {
    let exe_path = match find_presentmon_exe() {
        Some(p) => p,
        None => {
            log::error!("FPS监控: 未找到 PresentMon-2.5.1-x64.exe，请确保文件已打包");
            FPS_ACTIVE.store(false, Ordering::SeqCst);
            return;
        }
    };

    // 构建 PresentMon 启动命令
    // 注意：不使用 --no_track_display，因为它会禁用 DWM 追踪，
    // 导致 OpenGL/Vulkan 应用（如 Minecraft Java 版）的帧无法被捕获
    let mut cmd = Command::new(&exe_path);
    cmd.args([
        "--output_stdout",         // CSV 输出到 stdout
        "--no_console_stats",      // 关闭控制台统计（防止污染 stdout）
        "--stop_existing_session", // 停止已有同名 ETW 会话
    ]);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
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
            return;
        }
    };

    // 取出 stdout 管道
    let stdout = child.stdout.take().unwrap();

    // 存储子进程句柄（用于停止时 kill）
    {
        let mut lock = CHILD_HANDLE.lock().unwrap();
        *lock = Some(child);
    }

    log::info!("FPS监控: PresentMon 子进程已启动，开始读取 CSV 数据");

    let reader = BufReader::new(stdout);

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

    for line_result in reader.lines() {
        if !FPS_ACTIVE.load(Ordering::SeqCst) {
            break;
        }

        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        line_count += 1;
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
            // 超时归零：如果 3 秒内没有匹配数据，FPS 归零（显示 --）
            if matched_count > 0 && last_match_time.elapsed() >= std::time::Duration::from_secs(3) {
                if SMOOTHED_FPS.load(Ordering::Relaxed) != 0 {
                    log::warn!(
                        "FPS监控: 目标 '{}' 3秒无数据，FPS 归零 (seen_apps={:?})",
                        target_name,
                        seen_apps
                    );
                    SMOOTHED_FPS.store(0, Ordering::Relaxed);
                }
            }

            // 定期诊断：每 5 秒输出一次状态
            if last_diag_time.elapsed() >= std::time::Duration::from_secs(5) {
                log::warn!(
                    "FPS监控: 诊断 line={} matched={} fps={} target='{}' seen_apps={:?}",
                    line_count,
                    matched_count,
                    SMOOTHED_FPS.load(Ordering::Relaxed),
                    target_name,
                    seen_apps
                );
                last_diag_time = std::time::Instant::now();
            }
            continue;
        }

        matched_count += 1;
        last_match_time = std::time::Instant::now();

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

            // 排除异常值：ms 过小（<1ms, 对应>1000fps）或过大（>1000ms）
            if ms_between_presents >= 1.0 && ms_between_presents < 1000.0 {
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

                // 更新匹配时间戳（用于外部检测 FPS 是否卡住）
                LAST_MATCH_TIMESTAMP.store(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                    Ordering::Relaxed,
                );
            }
        }

        // 每秒输出一次诊断
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

    // 管道结束：清理子进程
    let mut lock = CHILD_HANDLE.lock().unwrap();
    if let Some(mut child) = lock.take() {
        let _ = child.kill();
        let _ = child.wait();
    }

    SMOOTHED_FPS.store(0, Ordering::Relaxed);
    log::info!(
        "FPS监控: PresentMon 会话结束 (line={} matched={})",
        line_count,
        matched_count
    );
}

/// FPS 监控主线程：含自动重启逻辑
fn fps_monitor_main() {
    let mut restart_count = 0u32;
    const MAX_RESTARTS: u32 = 5;

    while FPS_ACTIVE.load(Ordering::SeqCst) {
        run_presentmon_session();

        if !FPS_ACTIVE.load(Ordering::SeqCst) {
            break;
        }

        restart_count += 1;
        if restart_count > MAX_RESTARTS {
            log::error!(
                "FPS监控: PresentMon 重启超过 {} 次，放弃",
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

/// 前台窗口轮询线程（后台线程）
///
/// 负责：
/// 1. 查询 CURRENT_FG_PID 对应的进程名（使用 sysinfo，耗时操作放在此后台线程）
/// 2. 兜底轮询前台窗口（防止 EVENT_SYSTEM_FOREGROUND 漏发）
/// 3. 将进程名写入 TARGET_PROCESS_NAME 供 CSV 过滤使用
fn foreground_poller_loop() {
    use sysinfo::{Pid, System};
    let mut sys = System::new();
    let mut last_pid: u32 = 0;

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

        // 2. 检测 FPS 是否过期（无匹配数据超过 3 秒），用于解决窗口切换后帧率卡住不更新的问题
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let last_match = LAST_MATCH_TIMESTAMP.load(Ordering::Relaxed);
        if last_match > 0 && now > last_match + 3000 {
            let current_fps = SMOOTHED_FPS.load(Ordering::Relaxed);
            if current_fps != 0 {
                log::warn!(
                    "FPS监控: 帧率数据已过期 ({}ms 无匹配)，重置 FPS 并重启 PresentMon",
                    now - last_match
                );
                SMOOTHED_FPS.store(0, Ordering::Relaxed);
                // 杀掉 PresentMon 子进程以强制重启，解阻塞 reader.lines()
                if let Some(mut child) = CHILD_HANDLE.lock().unwrap().take() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }

        // 3. 查询进程名（耗时操作，在后台线程执行）
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
            let mut target = TARGET_PROCESS_NAME.lock().unwrap();
            if *target != name {
                log::info!("FPS监控: 前台进程 → {} (pid={})", name, pid);
                *target = name;
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

    // 启动前台窗口轮询线程（兜底）
    thread::spawn(|| {
        foreground_poller_loop();
    });

    log::info!("FPS监控: 已启动 (PresentMon 方案)");
}

/// 停止 FPS 监控
pub fn stop_fps_monitor() {
    if !FPS_ACTIVE.load(Ordering::SeqCst) {
        return;
    }
    FPS_ACTIVE.store(false, Ordering::SeqCst);

    // kill 子进程（管道断开后 reader 循环自动退出）
    let mut lock = CHILD_HANDLE.lock().unwrap();
    if let Some(mut child) = lock.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    drop(lock);

    #[cfg(target_os = "windows")]
    unsafe {
        win32_fg::unregister_hook();
    }

    SMOOTHED_FPS.store(0, Ordering::Relaxed);
    LAST_MATCH_TIMESTAMP.store(0, Ordering::Relaxed);
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
