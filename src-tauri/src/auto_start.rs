use std::path::PathBuf;
use std::process::Command;

const APP_NAME: &str = "NexBox";
const TASK_NAME: &str = "NexBox";
const REG_PATH: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

/// 执行命令（隐藏窗口）
#[cfg(windows)]
fn exec_hidden(cmd: &str, args: &[&str]) -> Result<std::process::Output, String> {
    use std::os::windows::process::CommandExt;
    Command::new(cmd)
        .args(args)
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| format!("执行 {} 失败: {}", cmd, e))
}

/// 获取用户启动文件夹路径
#[cfg(windows)]
fn get_startup_folder() -> Result<PathBuf, String> {
    dirs::config_dir()
        .map(|p| {
            p.join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs")
                .join("Startup")
        })
        .ok_or("无法获取启动文件夹路径".to_string())
}

// ========== 方案1：启动文件夹快捷方式（mslnk 纯 Rust） ==========

#[cfg(windows)]
fn create_startup_shortcut(exe_path: &str) -> Result<(), String> {
    let startup_dir = get_startup_folder()?;
    let lnk_path = startup_dir.join("NexBox.lnk");

    let working_dir = PathBuf::from(exe_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut sl = mslnk::ShellLink::new(exe_path)
        .map_err(|e| format!("创建 ShellLink 失败: {}", e))?;

    sl.set_working_dir(Some(working_dir));
    sl.set_name(Some("NexBox Auto Startup".to_string()));

    sl.create_lnk(&lnk_path)
        .map_err(|e| format!("写入快捷方式失败: {}", e))?;

    log::info!("启动文件夹快捷方式已创建: {}", lnk_path.display());
    Ok(())
}

#[cfg(windows)]
fn remove_startup_shortcut() -> Result<(), String> {
    let startup_dir = get_startup_folder()?;
    let lnk_path = startup_dir.join("NexBox.lnk");

    if lnk_path.exists() {
        std::fs::remove_file(&lnk_path)
            .map_err(|e| format!("删除快捷方式失败: {}", e))?;
        log::info!("启动文件夹快捷方式已删除");
    }
    Ok(())
}

#[cfg(windows)]
fn check_startup_shortcut() -> bool {
    if let Ok(startup_dir) = get_startup_folder() {
        return startup_dir.join("NexBox.lnk").exists();
    }
    false
}

// ========== 方案2：任务计划程序（绕过 UAC 限制） ==========

/// 创建任务计划：登录时以最高权限启动 NexBox
/// schtasks /create /tn "NexBox" /tr "\"path\"" /sc onlogon /rl highest /f
#[cfg(windows)]
fn create_scheduled_task(exe_path: &str) -> Result<(), String> {
    let quoted_exe = format!("\"{}\"", exe_path);

    let output = exec_hidden("schtasks", &[
        "/create",
        "/tn", TASK_NAME,
        "/tr", &quoted_exe,
        "/sc", "onlogon",
        "/rl", "highest",
        "/f",
    ])?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("创建计划任务失败: {}", stderr.trim()));
    }

    log::info!("计划任务已创建: {} -> {}", TASK_NAME, exe_path);
    Ok(())
}

/// 删除任务计划
#[cfg(windows)]
fn remove_scheduled_task() -> Result<(), String> {
    let output = exec_hidden("schtasks", &[
        "/delete",
        "/tn", TASK_NAME,
        "/f",
    ])?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // 任务不存在也算成功
        if stderr.contains("不存在") || stderr.contains("cannot find") {
            log::info!("计划任务不存在，跳过删除");
            return Ok(());
        }
        return Err(format!("删除计划任务失败: {}", stderr.trim()));
    }

    log::info!("计划任务已删除: {}", TASK_NAME);
    Ok(())
}

/// 检查任务计划是否存在
#[cfg(windows)]
fn check_scheduled_task() -> bool {
    match exec_hidden("schtasks", &["/query", "/tn", TASK_NAME]) {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

// ========== 清理旧注册表 Run 键 ==========

/// 清理旧的注册表 Run 键（requireAdministrator 下无效，纯粹清理）
#[cfg(windows)]
fn cleanup_old_registry() {
    let _ = exec_hidden("reg", &["delete", REG_PATH, "/v", APP_NAME, "/f"]);
}

// ========== Tauri Commands ==========

#[tauri::command]
pub async fn set_nexbox_auto_start(enable: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        if enable {
            let app_path = std::env::current_exe()
                .map_err(|e| format!("获取程序路径失败: {}", e))?
                .to_string_lossy()
                .replace("/", "\\");

            log::info!("准备设置开机自启，程序路径: {}", app_path);

            // 清理旧注册表（避免任务管理器残留无效项）
            cleanup_old_registry();

            // 方案1：任务计划程序（主方案，绕过 UAC 限制）
            let task_result = create_scheduled_task(&app_path);
            if let Err(e) = &task_result {
                log::warn!("创建计划任务失败: {}", e);
            }

            // 方案2：启动文件夹快捷方式（辅助方案）
            let shortcut_result = create_startup_shortcut(&app_path);
            if let Err(e) = &shortcut_result {
                log::warn!("创建启动文件夹快捷方式失败: {}", e);
            }

            if task_result.is_err() && shortcut_result.is_err() {
                return Err(format!(
                    "开机自启设置失败：计划任务({})，快捷方式({})",
                    task_result.unwrap_err(),
                    shortcut_result.unwrap_err()
                ));
            }

            log::info!("开机自启设置成功（计划任务={}, 快捷方式={}）",
                task_result.is_ok(), shortcut_result.is_ok());
            Ok(())
        } else {
            let task_result = remove_scheduled_task();
            if let Err(e) = &task_result {
                log::warn!("删除计划任务失败: {}", e);
            }

            let shortcut_result = remove_startup_shortcut();
            if let Err(e) = &shortcut_result {
                log::warn!("删除快捷方式失败: {}", e);
            }

            // 同时清理旧注册表
            cleanup_old_registry();

            if task_result.is_err() && shortcut_result.is_err() {
                return Err("关闭开机自启失败".to_string());
            }

            log::info!("开机自启已关闭");
            Ok(())
        }
    }

    #[cfg(not(windows))]
    {
        let _ = enable;
        Err("当前平台不支持开机自启动设置".to_string())
    }
}

#[tauri::command]
pub async fn check_nexbox_auto_start() -> Result<bool, String> {
    #[cfg(windows)]
    {
        let task_exists = check_scheduled_task();
        let shortcut_exists = check_startup_shortcut();

        let enabled = task_exists || shortcut_exists;
        log::debug!(
            "开机自启状态检查：计划任务={}, 快捷方式={}, 最终={}",
            task_exists,
            shortcut_exists,
            enabled
        );
        Ok(enabled)
    }

    #[cfg(not(windows))]
    {
        Ok(false)
    }
}
