//! FPS 监控模块 — 重写版（参考 CapFrameX 架构）
//!
//! 核心改进：
//! 1. PresentMon 持续运行，前台切换不重启（零中断）— 消除了旧方案频繁杀启 ETW 会话的问题
//! 2. Reader 线程直接处理 CSV 数据，无 channel 中转 — 消除了 stdout 管道缓冲区阻塞的根因
//! 3. 环形缓冲区 + EMA 双重平滑 — 比单一 EMA 更稳定，支持未来扩展百分位指标
//! 4. 简化线程模型：2 线程 + 8 全局状态（原 3 线程 + 12 全局状态）
//! 5. 消费端过滤（参考 CapFrameX OnlineMetricService）— PresentMon 捕获所有进程，reader 按目标过滤
//!
//! 架构说明：
//! - fps_monitor_main 线程：管理 PresentMon 进程生命周期，等待 reader 线程退出后重启
//! - reader 线程：持续读取 stdout CSV，解析帧时间，更新 SMOOTHED_FPS 原子变量
//! - process_tracker 线程：追踪前台窗口，更新 TARGET_PROCESS，检测 FPS 过期归零
//!
//! 参考：CapFrameX PresentMonCaptureService + OnlineMetricService

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::OnceLock;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU32, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use parking_lot::RwLock;

// ============ 全局状态（8 项，原 12 项） ============

/// 平滑后的 FPS 值，供 overlay 读取
static SMOOTHED_FPS: AtomicU32 = AtomicU32::new(0);
/// FPS 监控是否处于活跃状态
static FPS_ACTIVE: AtomicBool = AtomicBool::new(false);
/// PresentMon exe 路径缓存
static PM_EXE_CACHE: OnceLock<Option<PathBuf>> = OnceLock::new();
/// 自身 overlay 窗口句柄（用于排除前台切换到自身 overlay）
static OVERLAY_HWND: AtomicU64 = AtomicU64::new(0);
/// 当前前台窗口 PID（钩子回调中快速存储）
static CURRENT_FG_PID: AtomicU32 = AtomicU32::new(0);
/// 当前前台目标进程名（小写 exe 文件名，如 "game.exe"）— RwLock 读多写少
static TARGET_PROCESS: RwLock<String> = RwLock::new(String::new());
/// PresentMon 子进程句柄（用于停止时 kill）
static CHILD_HANDLE: Mutex<Option<Child>> = Mutex::new(None);
/// 上次匹配到目标帧数据的时间戳（ms），用于检测 FPS 过期归零
static LAST_FRAME_TIME: AtomicU64 = AtomicU64::new(0);
/// 目标切换代次（每次切换 +1），reader 线程检测到变化后清空缓冲区
static TARGET_GENERATION: AtomicU64 = AtomicU64::new(0);
/// 1% Low FPS（最慢 1% 帧的平均 FPS），参考 CapFrameX OnePercentLowAverage
static ONE_PCT_LOW_FPS: AtomicU32 = AtomicU32::new(0);
/// 0.1% Low FPS（最慢 0.1% 帧的平均 FPS），参考 CapFrameX ZerodotOnePercentLowAverage
static ZERO_DOT_ONE_PCT_LOW_FPS: AtomicU32 = AtomicU32::new(0);

// ============ Windows 前台窗口钩子 ============

#[cfg(target_os = "windows")]
mod win32_fg {
    use super::*;
    use std::ptr;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::UI::Accessibility::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    static HOOK_HANDLE: Mutex<usize> = Mutex::new(0);

    /// 前台窗口切换回调 — 仅存储 PID，不做耗时操作
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

    /// 初始化时获取当前前台窗口 PID
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
#[cfg(windows)]
fn try_send_quit_to_presentmon() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    let class_name: Vec<u16> = "PresentMon\0".encode_utf16().collect();

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
/// 优先通过 WM_QUIT 让 PresentMon 自行清理 ETW 会话，
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
fn cleanup_stale_etw_session(exe_path: &std::path::Path) {
    let mut cmd = Command::new(exe_path);
    cmd.args(["--terminate_existing_session"]);
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);

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

/// 查找 PresentMon exe 路径（兼容开发模式和生产模式）
fn find_presentmon_exe() -> Option<PathBuf> {
    if let Some(cached) = PM_EXE_CACHE.get() {
        return cached.clone();
    }

    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let cwd = std::env::current_dir().ok();
    let candidates: Vec<PathBuf> = vec![
        exe_dir.join("PresentMon-2.5.1-x64.exe"),
        exe_dir.join("resources").join("PresentMon-2.5.1-x64.exe"),
        exe_dir.join("bin").join("PresentMon-2.5.1-x64.exe"),
        exe_dir.join("_up_").join("resources").join("binaries").join("PresentMon-2.5.1-x64.exe"),
        exe_dir.join("..").join("..").join("resources").join("binaries").join("PresentMon-2.5.1-x64.exe"),
        cwd.as_ref().map(|c| c.join("resources").join("binaries").join("PresentMon-2.5.1-x64.exe")).unwrap_or_default(),
        cwd.as_ref().map(|c| c.join("PresentMon-main").join("PresentMon-2.5.1-x64.exe")).unwrap_or_default(),
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

    let _ = PM_EXE_CACHE.set(result.clone());
    result
}

/// 判断进程是否为非游戏进程（大小写不敏感，避免 to_lowercase 堆分配）
fn is_non_game_process(name: &str) -> bool {
    const NON_GAME_PROCESSES: &[&str] = &[
        "explorer.exe", "dwm.exe", "windowsterminal.exe", "cmd.exe",
        "powershell.exe", "conhost.exe", "catpawai.exe", "nexbox.exe",
        "msedgewebview2.exe", "searchhost.exe", "shellexperiencehost.exe",
        "sihost.exe", "ctfmon.exe", "textinputhost.exe",
        "applicationframehost.exe", "winlogon.exe", "fontdrvhost.exe",
        "systemsettings.exe", "taskmgr.exe",
    ];
    NON_GAME_PROCESSES.iter().any(|p| name.eq_ignore_ascii_case(p))
}

/// 去除 .exe 后缀（大小写不敏感）
#[inline]
fn strip_exe_suffix(s: &str) -> &str {
    if s.len() >= 4 && s[s.len() - 4..].eq_ignore_ascii_case(".exe") {
        &s[..s.len() - 4]
    } else {
        s
    }
}

/// 进程名匹配：支持带/不带 .exe 后缀，以及 java/javaw 互通
///
/// 优化：使用 eq_ignore_ascii_case 避免 to_lowercase() 堆分配
/// 参考 CapFrameX OnlineMetricService.UpdateOnlineMetrics 的消费端过滤
fn process_name_matches(app: &str, target: &str) -> bool {
    if target.is_empty() {
        return false;
    }
    // 大小写不敏感直接比较
    if app.eq_ignore_ascii_case(target) {
        return true;
    }
    // 去除 .exe 后缀后再比较
    let app_no_exe = strip_exe_suffix(app);
    let target_no_exe = strip_exe_suffix(target);
    if app_no_exe.eq_ignore_ascii_case(target_no_exe) {
        return true;
    }
    // java ↔ javaw 互通（Minecraft Java 版可能用 javaw.exe）
    let app_is_java = app_no_exe.eq_ignore_ascii_case("java")
        || app_no_exe.eq_ignore_ascii_case("javaw");
    let target_is_java = target_no_exe.eq_ignore_ascii_case("java")
        || target_no_exe.eq_ignore_ascii_case("javaw");
    app_is_java && target_is_java
}

// ============ 帧时间环形缓冲区 ============

/// 固定大小环形缓冲区，O(1) 写入，O(n) 计算（n 为窗口内帧数）
///
/// 参考 CapFrameX CircularBuffer 和 capframex-linux timing.c 的 frame_buffer
struct FrameTimeBuffer {
    data: Vec<f64>,
    head: usize,
    count: usize,
}

impl FrameTimeBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            data: vec![0.0; capacity],
            head: 0,
            count: 0,
        }
    }

    /// O(1) 写入，自动覆盖最旧数据
    fn push(&mut self, value: f64) {
        if self.data.is_empty() {
            return;
        }
        self.data[self.head] = value;
        self.head = (self.head + 1) % self.data.len();
        if self.count < self.data.len() {
            self.count += 1;
        }
    }

    fn clear(&mut self) {
        self.head = 0;
        self.count = 0;
    }

    /// 计算窗口内平均帧时间对应的 FPS
    ///
    /// FPS = 1000 * count / sum(frametimes)
    /// 等价于 CapFrameX 的 GetFpsMetricValue(Average)
    fn average_fps(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        let sum: f64 = if self.count < self.data.len() {
            // 缓冲区未满，有效数据在 [0..count)
            self.data[..self.count].iter().sum()
        } else {
            // 缓冲区已满，整个数组都有效
            self.data.iter().sum()
        };
        if sum <= 0.0 {
            return 0.0;
        }
        1000.0 * self.count as f64 / sum
    }

    /// Calculate x% Low FPS (average FPS of the worst x% frames)
    ///
    /// Algorithm (from CapFrameX GetPercentageHighAverageSequence):
    /// 1. Sort frame times ascending
    /// 2. Get the (1-x) quantile value as threshold
    /// 3. Select all frame times >= threshold (the slowest x%)
    /// 4. Average those frame times
    /// 5. FPS = 1000 / average
    ///
    /// Parameter: low_percent — 0.01 for 1% Low, 0.001 for 0.1% Low
    fn percentile_low_fps(&self, low_percent: f64) -> f64 {
        if self.count == 0 {
            return 0.0;
        }

        // Collect valid frame times
        let samples: Vec<f64> = if self.count < self.data.len() {
            self.data[..self.count].to_vec()
        } else {
            self.data.clone()
        };

        if samples.is_empty() {
            return 0.0;
        }

        // Sort ascending
        let mut sorted = samples;
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Get the (1 - low_percent) quantile as threshold
        // e.g. 1% Low -> p_quantile = 0.99 -> 99th percentile of frame times
        let p_quantile = 1.0 - low_percent;
        let q_idx = (p_quantile * (sorted.len() - 1) as f64).round() as usize;
        let q_idx = q_idx.min(sorted.len() - 1);
        let quantile_val = sorted[q_idx];

        // Select all frame times >= threshold (the slowest low_percent)
        let low_frametimes: Vec<f64> = sorted
            .iter()
            .filter(|&&x| x >= quantile_val)
            .cloned()
            .collect();

        if low_frametimes.is_empty() {
            return 0.0;
        }

        let avg = low_frametimes.iter().sum::<f64>() / low_frametimes.len() as f64;
        if avg <= 0.0 {
            return 0.0;
        }

        1000.0 / avg
    }
}

// ============ Reader 线程 ============

/// Reader 线程：持续读取 PresentMon stdout CSV，解析帧时间，更新 SMOOTHED_FPS
///
/// 关键优化（参考 CapFrameX PresentMonCaptureService.OutputDataReceived）：
/// - 无 channel 中转：直接在 reader 线程内处理每行数据
/// - 无 Mutex 锁竞争：使用 RwLock 读锁（不阻塞其他读），仅做字符串比较
/// - 无 HashMap 查找：表头解析一次后直接用索引
/// - 无 String::clone()：目标进程名通过 &str 比较，不克隆
///
/// 这确保 reader 线程以最高速度消费 stdout，避免管道缓冲区填满导致 PresentMon 阻塞。
fn reader_thread_main(stdout: std::process::ChildStdout) {
    let reader = BufReader::new(stdout);

    // CSV 列索引（表头解析一次，参考 CapFrameX static readonly int 编译期常量设计）
    let mut header_parsed = false;
    let mut app_idx: usize = 0;
    let mut ms_idx: usize = 0;

    // 帧时间环形缓冲区（3000 帧 ≈ 12.5s@240fps / 50s@60fps）
    // 需要 >= 1000 帧才能计算有意义的 0.1% Low
    let mut buffer = FrameTimeBuffer::new(3000);

    // EMA 平滑状态
    let mut smoothed_fps: f64 = 0.0;
    let mut first_frame = true;

    // 目标切换检测
    let mut last_gen: u64 = 0;

    // 1% Low / 0.1% Low 计算计时器（每 ~1 秒计算一次，避免每帧排序）
    let mut last_low_calc_time = Instant::now();

    for line_result in reader.lines() {
        // 检查活跃状态（PresentMon 被外部 kill 时 stdout 会 EOF）
        if !FPS_ACTIVE.load(Ordering::SeqCst) {
            break;
        }

        let line = match line_result {
            Ok(l) => l,
            Err(_) => break, // 管道 EOF 或错误
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // ===== 第一阶段：解析表头 =====
        if !header_parsed {
            let cols: Vec<&str> = trimmed.split(',').collect();
            for (i, col) in cols.iter().enumerate() {
                let col = col.trim();
                if col == "Application" {
                    app_idx = i;
                } else if col == "MsBetweenPresents" {
                    ms_idx = i;
                }
            }
            header_parsed = true;
            log::info!(
                "FPS监控: CSV 表头 → Application[{}] MsBetweenPresents[{}] (共{}列)",
                app_idx, ms_idx, cols.len()
            );
            continue;
        }

        // ===== 第二阶段：解析数据行 =====
        // 优化：仅提取需要的两列，避免每帧 Vec<&str> 堆分配
        // 240 FPS 下每秒省 240 次 Vec 分配 + 240 次 String::to_lowercase 分配
        // 参考 CapFrameX 固定列索引 + OutputDataReceived 直接索引设计
        let mut app_name: &str = "";
        let mut ms_value: &str = "";
        let mut found = 0u8; // bitmask: bit0=app, bit1=ms
        for (i, col) in trimmed.split(',').enumerate() {
            if i == app_idx {
                app_name = col.trim();
                found |= 1;
            } else if i == ms_idx {
                ms_value = col.trim();
                found |= 2;
            }
            if found == 3 {
                break; // 两列都找到了，提前退出
            }
        }
        if found != 3 {
            continue; // 列数不足，跳过
        }

        // 过滤 <error> 条目（参考 CapFrameX <error> 过滤）
        // 优化：eq_ignore_ascii_case 避免 to_lowercase 堆分配
        if app_name.eq_ignore_ascii_case("<error>") || app_name.is_empty() {
            continue;
        }

        // 检测目标切换 → 清空缓冲区 + 重置 EMA
        let gen = TARGET_GENERATION.load(Ordering::Relaxed);
        if gen != last_gen {
            buffer.clear();
            smoothed_fps = 0.0;
            first_frame = true;
            last_gen = gen;
            ONE_PCT_LOW_FPS.store(0, Ordering::Relaxed);
            ZERO_DOT_ONE_PCT_LOW_FPS.store(0, Ordering::Relaxed);
        }

        // 消费端过滤：匹配目标进程（参考 CapFrameX OnlineMetricService.UpdateOnlineMetrics）
        // 优化：process_name_matches 内部使用 eq_ignore_ascii_case，无堆分配
        let target = TARGET_PROCESS.read();
        if !process_name_matches(app_name, &target) {
            continue;
        }
        drop(target); // 立即释放读锁

        // 更新最后匹配时间戳
        LAST_FRAME_TIME.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            Ordering::Relaxed,
        );

        // 解析 MsBetweenPresents（毫秒）
        if let Ok(ms_between_presents) = ms_value.parse::<f64>() {
            // 仅用合理的帧时间（0.1ms ~ 2000ms）
            if ms_between_presents >= 0.1 && ms_between_presents < 2000.0 {
                // 环形缓冲区 + EMA 双重平滑
                buffer.push(ms_between_presents);
                let raw_fps = buffer.average_fps();

                if raw_fps > 0.0 {
                    if first_frame {
                        smoothed_fps = raw_fps;
                        first_frame = false;
                    } else {
                        // EMA 系数 0.2：值越小越平滑，越大越灵敏
                        smoothed_fps = 0.2 * raw_fps + 0.8 * smoothed_fps;
                    }

                    SMOOTHED_FPS.store(smoothed_fps.round() as u32, Ordering::Relaxed);
                }

                // 每 ~1 秒计算 1% Low / 0.1% Low（排序 3000 帧仅需微秒级）
                if last_low_calc_time.elapsed() >= Duration::from_secs(1) {
                    // 1% Low 需要至少 100 帧
                    if buffer.count >= 100 {
                        let one_low = buffer.percentile_low_fps(0.01);
                        if one_low > 0.0 {
                            ONE_PCT_LOW_FPS.store(one_low.round() as u32, Ordering::Relaxed);
                        }
                    }
                    // 0.1% Low 需要至少 1000 帧
                    if buffer.count >= 1000 {
                        let zero_one_low = buffer.percentile_low_fps(0.001);
                        if zero_one_low > 0.0 {
                            ZERO_DOT_ONE_PCT_LOW_FPS.store(zero_one_low.round() as u32, Ordering::Relaxed);
                        }
                    }
                    last_low_calc_time = Instant::now();
                }
            }
        }
    }

    log::info!("FPS监控: Reader 线程退出");
}

// ============ 进程追踪线程 ============

/// 进程追踪线程：追踪前台窗口，更新 TARGET_PROCESS，检测 FPS 过期归零
///
/// 简化说明（对比旧方案 foreground_poller_loop）：
/// - 去掉 200ms 高频轮询 → 500ms（Hook 已足够可靠，轮询仅兜底）
/// - 去掉 LINE_COUNT_ATOMIC 看门狗 → reader 线程退出即触发重启
/// - 去掉 RESTART_REQUESTED 标志 → 不重启 PresentMon，仅更新 TARGET_PROCESS
/// - 去抖动从 300ms → 500ms（减少 Alt+Tab 抖动）
fn process_tracker_loop() {
    use sysinfo::{Pid, System};

    let mut sys = System::new();
    let mut last_pid: u32 = 0;

    // 前台切换去抖动状态
    let mut pending_name: Option<String> = None;
    let mut pending_since: Option<Instant> = None;
    const DEBOUNCE_DELAY: Duration = Duration::from_millis(500);

    while FPS_ACTIVE.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(500));
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

        // 2. 检测 FPS 过期（无匹配数据超过 3 秒 → 归零显示）
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let last_frame = LAST_FRAME_TIME.load(Ordering::Relaxed);
        if last_frame > 0 && now > last_frame + 3000 {
            if SMOOTHED_FPS.load(Ordering::Relaxed) != 0 {
                log::info!(
                    "FPS监控: 帧率数据已过期 ({}ms 无匹配)，FPS 归零",
                    now - last_frame
                );
                SMOOTHED_FPS.store(0, Ordering::Relaxed);
            }
        }

        // 3. 查询进程名（耗时操作放在后台线程）
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
            let name = process.name().to_string();
            let current_target = TARGET_PROCESS.read().clone();

            if name == current_target {
                // 与当前目标一致，取消待提交的变更
                pending_name = None;
                pending_since = None;
            } else {
                // 进程已变化，启动/更新去抖动计时
                if pending_name.as_deref() != Some(name.as_str()) {
                    pending_name = Some(name.clone());
                    pending_since = Some(Instant::now());
                }

                // 检查去抖动是否到期
                if let (Some(ref pending), Some(since)) = (&pending_name, pending_since) {
                    if since.elapsed() >= DEBOUNCE_DELAY {
                        let is_game = !pending.is_empty() && !is_non_game_process(pending.as_str());

                        {
                            let mut target = TARGET_PROCESS.write();
                            *target = if is_game { pending.clone() } else { String::new() };
                        }

                        if !is_game {
                            SMOOTHED_FPS.store(0, Ordering::Relaxed);
                        }

                        // 递增代次，通知 reader 线程清空缓冲区
                        TARGET_GENERATION.fetch_add(1, Ordering::Relaxed);

                        log::info!(
                            "FPS监控: 前台进程 → {} (pid={}){}",
                            pending, pid,
                            if is_game { "" } else { " (非游戏)" }
                        );

                        pending_name = None;
                        pending_since = None;
                    }
                }
            }
        }
    }
}

// ============ FPS 监控主线程 ============

/// FPS 监控主线程：管理 PresentMon 进程生命周期
///
/// 与旧方案的关键区别：
/// - 不传 --process_name：PresentMon 捕获所有进程，reader 线程消费端过滤
/// - 前台切换不重启 PresentMon：仅更新 TARGET_PROCESS 原子变量
/// - 仅在 PresentMon 崩溃/退出（reader 线程退出）时才重启
fn fps_monitor_main() {
    let exe_path = match find_presentmon_exe() {
        Some(p) => p,
        None => {
            log::error!("FPS监控: 未找到 PresentMon-2.5.1-x64.exe，请确保文件已打包");
            FPS_ACTIVE.store(false, Ordering::SeqCst);
            return;
        }
    };

    let mut restart_count = 0u32;
    const MAX_RESTARTS: u32 = 5;

    while FPS_ACTIVE.load(Ordering::SeqCst) {
        let session_start = Instant::now();

        // 预清理：停止可能残留的 ETW 会话
        cleanup_stale_etw_session(&exe_path);

        // 构建 PresentMon 启动命令
        // 关键：不传 --process_name！PresentMon 捕获所有进程，
        // reader 线程按 TARGET_PROCESS 过滤（参考 CapFrameX 架构）
        let mut cmd = Command::new(&exe_path);
        cmd.args([
            "--output_stdout",         // CSV 输出到 stdout
            "--no_console_stats",      // 关闭控制台统计（防止污染 stdout）
            "--stop_existing_session", // 停止已有同名 ETW 会话
            "--no_track_input",        // 不追踪输入延迟（减少 ETW 开销，参考 CapFrameX）
        ]);
        // 排除系统进程，减少 ETW 事件量和 stdout 输出
        // 参考 CapFrameX PresentMonServiceConfiguration.ExcludeProcesses
        for proc in &[
            "explorer.exe", "dwm.exe", "SearchHost.exe", "ShellExperienceHost.exe",
            "msedgewebview2.exe", "TextInputHost.exe", "sihost.exe", "ctfmon.exe",
            "ApplicationFrameHost.exe", "fontdrvhost.exe", "SystemSettings.exe",
            "Taskmgr.exe", "WindowsTerminal.exe", "cmd.exe", "powershell.exe",
            "conhost.exe", "CatPawAI.exe", "NexBox.exe",
        ] {
            cmd.args(["--exclude", proc]);
        }
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
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
                restart_count += 1;
                if restart_count > MAX_RESTARTS {
                    log::error!("FPS监控: PresentMon 启动失败超过 {} 次，放弃", MAX_RESTARTS);
                    FPS_ACTIVE.store(false, Ordering::SeqCst);
                    break;
                }
                thread::sleep(Duration::from_secs(2));
                continue;
            }
        };

        // 取出 stdout 管道
        let stdout = child.stdout.take().unwrap();

        // 捕获 stderr 用于诊断
        if let Some(stderr) = child.stderr.take() {
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().flatten() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    // 过滤退出时的正常事件丢失警告
                    if trimmed.contains("ETW events were lost") || trimmed.contains("ETW buffers were lost") {
                        continue;
                    }
                    log::warn!("FPS监控: PresentMon stderr: {}", trimmed);
                }
            });
        }

        // 存储子进程句柄
        {
            let mut lock = CHILD_HANDLE.lock().unwrap();
            *lock = Some(child);
        }

        log::info!("FPS监控: PresentMon 已启动（全局监控模式，前台切换不重启）");

        // 启动 reader 线程
        let reader_handle = thread::spawn(move || {
            reader_thread_main(stdout);
        });

        // 等待 reader 线程退出（管道 EOF / PresentMon 崩溃 / 用户停止）
        let _ = reader_handle.join();

        // 停止 PresentMon（清理 ETW 会话）
        stop_presentmon_graceful();

        if !FPS_ACTIVE.load(Ordering::SeqCst) {
            break;
        }

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
            "FPS监控: PresentMon 管道断开，2秒后重启 (第{}/{})",
            restart_count, MAX_RESTARTS
        );
        thread::sleep(Duration::from_secs(2));
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

    // 启动 PresentMon 进程管理主线程
    thread::spawn(|| {
        fps_monitor_main();
    });

    // 启动前台窗口追踪线程
    thread::spawn(|| {
        process_tracker_loop();
    });

    log::info!("FPS监控: 已启动 (重写版 — 全局监控 + 消费端过滤)");
}

/// 停止 FPS 监控
pub fn stop_fps_monitor() {
    if !FPS_ACTIVE.load(Ordering::SeqCst) {
        return;
    }
    FPS_ACTIVE.store(false, Ordering::SeqCst);

    // 优雅停止 PresentMon 子进程
    stop_presentmon_graceful();

    #[cfg(target_os = "windows")]
    unsafe {
        win32_fg::unregister_hook();
    }

    SMOOTHED_FPS.store(0, Ordering::Relaxed);
    ONE_PCT_LOW_FPS.store(0, Ordering::Relaxed);
    ZERO_DOT_ONE_PCT_LOW_FPS.store(0, Ordering::Relaxed);
    LAST_FRAME_TIME.store(0, Ordering::Relaxed);
    TARGET_GENERATION.store(0, Ordering::Relaxed);
    *TARGET_PROCESS.write() = String::new();
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

/// 获取缓存的 1% Low FPS 值（最慢 1% 帧的平均 FPS）
///
/// 参考 CapFrameX EMetric.OnePercentLowAverage：
/// 取 99 分位帧时间作为阈值，筛选 >= 阈值的帧（最慢 1%），计算平均帧时间，
/// FPS = 1000 / 平均帧时间
pub fn get_cached_1low_fps() -> Option<u32> {
    let fps = ONE_PCT_LOW_FPS.load(Ordering::Relaxed);
    if fps == 0 {
        None
    } else {
        Some(fps)
    }
}

/// 获取缓存的 0.1% Low FPS 值（最慢 0.1% 帧的平均 FPS）
///
/// 参考 CapFrameX EMetric.ZerodotOnePercentLowAverage：
/// 取 99.9 分位帧时间作为阈值，筛选 >= 阈值的帧（最慢 0.1%），计算平均帧时间，
/// FPS = 1000 / 平均帧时间
pub fn get_cached_01low_fps() -> Option<u32> {
    let fps = ZERO_DOT_ONE_PCT_LOW_FPS.load(Ordering::Relaxed);
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
