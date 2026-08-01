// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // ═══════════════════════════════════════════════════════════════════
    // 第一优先：初始化日志（在任何 Tauri 代码之前）
    // 这样即使 .build() 崩溃或 single-instance 插件退出，
    // 也能在日志文件中留下记录，便于排查。
    // 日志路径：%LOCALAPPDATA%/NexBox/nexbox.log
    // ═══════════════════════════════════════════════════════════════════
    init_early_logging();

    log::info!(
        "═══════════════════════════════════════════════════════════════"
    );
    log::info!(
        "[BOOT] nexbox.exe 启动 | pid={} exe={:?} cwd={:?} args={:?}",
        std::process::id(),
        std::env::current_exe().ok(),
        std::env::current_dir().ok(),
        std::env::args().collect::<Vec<_>>()
    );

    // 开机自启修复：Windows 通过注册表 Run 键 / 计划任务启动程序时
    // 工作目录可能是 System32，导致 Tauri 加载资源和依赖失败。
    // 主动切换到 exe 所在目录。
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let _ = std::env::set_current_dir(exe_dir);
            log::info!("[BOOT] 工作目录已切换到: {}", exe_dir.display());
        }
    }

    log::info!("[BOOT] 即将进入 nexbox_lib::run()");
    nexbox_lib::run();
}

/// 在 main() 最开始初始化日志，确保 .build() 之前的崩溃也能被记录。
///
/// Release 模式：日志写入文件 `%LOCALAPPDATA%/NexBox/nexbox.log`
/// Debug 模式：日志同时输出到控制台和文件
fn init_early_logging() {
    // Debug 模式：由 tauri_plugin_log 处理日志（控制台输出）
    // Release 模式：初始化文件日志，因为开机自启时没有控制台
    if cfg!(debug_assertions) {
        return;
    }

    let log_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("NexBox");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("nexbox.log");

    // 限制日志文件大小，避免无限增长（超过 5MB 时截断）
    if let Ok(metadata) = std::fs::metadata(&log_path) {
        if metadata.len() > 5 * 1024 * 1024 {
            let _ = std::fs::remove_file(&log_path);
        }
    }

    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path);

    let mut builder = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    );

    if let Ok(file) = log_file {
        builder.target(env_logger::Target::Pipe(Box::new(file)));
    }

    let _ = builder.try_init();
}
