use std::path::PathBuf;
use std::process::Command;

const APP_NAME: &str = "NexBox";
const TASK_NAME: &str = "NexBox";

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

// ========== 方案1：任务计划程序（主方案） ==========

/// 创建任务计划：用户登录后延迟 5 秒启动 NexBox
/// 通过 schtasks /create /xml 导入带 <Delay>PT5S</Delay> 的任务定义
/// 延迟 5 秒可确保 explorer.exe 已加载、托盘区就绪，避免启动过早导致托盘图标失败
/// 不使用 PowerShell，避免启动慢、杀软拦截、弹窗
#[cfg(windows)]
fn create_scheduled_task(exe_path: &str) -> Result<(), String> {
    use std::io::Write;

    // 获取工作目录（exe 所在目录）
    let working_dir = std::path::Path::new(exe_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let task_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers>
    <LogonTrigger>
      <Delay>PT5S</Delay>
      <Enabled>true</Enabled>
    </LogonTrigger>
  </Triggers>
  <Actions Context="Author">
    <Exec>
      <Command>{}</Command>
      <WorkingDirectory>{}</WorkingDirectory>
    </Exec>
  </Actions>
  <Settings>
    <AllowStartIfOnBatteries>true</AllowStartIfOnBatteries>
    <DontStopIfOnBatteries>true</DontStopIfOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
  </Settings>
</Task>"#,
        xml_escape(exe_path),
        xml_escape(&working_dir),
    );

    // 写入临时 XML 文件（UTF-16 LE，schtasks 标准格式）
    let temp_dir = std::env::temp_dir();
    let xml_path = temp_dir.join("nexbox_task.xml");
    {
        let mut file = std::fs::File::create(&xml_path)
            .map_err(|e| format!("创建临时文件失败: {}", e))?;
        // UTF-16 LE BOM
        file.write_all(&[0xFF, 0xFE])
            .map_err(|e| format!("写入XML失败: {}", e))?;
        for code_unit in task_xml.encode_utf16() {
            file.write_all(&code_unit.to_le_bytes())
                .map_err(|e| format!("写入XML失败: {}", e))?;
        }
    }

    let output = exec_hidden("schtasks", &[
        "/create",
        "/xml", &xml_path.to_string_lossy(),
        "/tn", TASK_NAME,
        "/f",
    ])?;

    // 清理临时文件
    let _ = std::fs::remove_file(&xml_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("创建计划任务失败: {}", stderr.trim()));
    }

    log::info!("计划任务已创建(登录后延迟5秒): {} -> {}", TASK_NAME, exe_path);
    Ok(())
}

/// 对 XML 特殊字符进行转义
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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

// ========== 清理旧版注册表 Run 键残留（已不再作为自启方案） ==========

/// 删除注册表 Run 键中的 NexBox 条目（仅清理历史遗留，不再写入）
#[cfg(windows)]
fn remove_registry_run() -> Result<(), String> {
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            winreg::enums::KEY_SET_VALUE,
        )
        .map_err(|e| format!("打开注册表 Run 键失败: {}", e))?;

    match key.delete_value(APP_NAME) {
        Ok(()) => {
            log::info!("注册表 Run 键已删除");
            Ok(())
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::info!("注册表 Run 键不存在，跳过删除");
            Ok(())
        }
        Err(e) => Err(format!("删除注册表 Run 键失败: {}", e)),
    }
}

// ========== 方案2：启动文件夹快捷方式（备选方案，mslnk 纯 Rust） ==========

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

// ========== Tauri Commands ==========

#[tauri::command]
pub async fn set_nexbox_auto_start(enable: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        let app_path = std::env::current_exe()
            .map_err(|e| format!("获取程序路径失败: {}", e))?
            .to_string_lossy()
            .replace("/", "\\");

        if enable {
            log::info!("准备设置开机自启，程序路径: {}", app_path);

            let mut errors: Vec<String> = Vec::new();

            // 方案1：任务计划程序（主方案）
            match create_scheduled_task(&app_path) {
                Ok(()) => {
                    log::info!("计划任务创建成功（主方案）");
                }
                Err(e) => {
                    log::warn!("计划任务创建失败: {}", e);
                    errors.push(format!("计划任务: {}", e));
                }
            }

            // 方案2：启动文件夹快捷方式（备选方案）
            match create_startup_shortcut(&app_path) {
                Ok(()) => {
                    log::info!("启动文件夹快捷方式创建成功（备选方案）");
                }
                Err(e) => {
                    log::warn!("启动文件夹快捷方式创建失败: {}", e);
                    errors.push(format!("快捷方式: {}", e));
                }
            }

            if check_scheduled_task() || check_startup_shortcut() {
                log::info!("开机自启设置成功（至少一种方案生效）");
                return Ok(());
            }

            return Err(format!(
                "开机自启设置失败，所有方案均未生效：{}",
                errors.join("；")
            ));
        } else {
            let mut errors: Vec<String> = Vec::new();

            match remove_scheduled_task() {
                Ok(()) => log::info!("计划任务已删除"),
                Err(e) => {
                    log::warn!("删除计划任务失败: {}", e);
                    errors.push(e);
                }
            }

            // 清理旧版注册表 Run 键残留（历史遗留，不再作为自启方案）
            match remove_registry_run() {
                Ok(()) => log::info!("注册表 Run 键残留已清理"),
                Err(e) => {
                    log::warn!("清理注册表 Run 键残留失败: {}", e);
                    errors.push(e);
                }
            }

            match remove_startup_shortcut() {
                Ok(()) => log::info!("快捷方式已删除"),
                Err(e) => {
                    log::warn!("删除快捷方式失败: {}", e);
                    errors.push(e);
                }
            }

            if !check_scheduled_task() && !check_startup_shortcut() {
                log::info!("开机自启已完全关闭");
                return Ok(());
            }

            if errors.is_empty() {
                log::warn!("开机自启关闭不完整：仍有残留启动项");
                return Err("部分启动项未能清除，请手动检查".to_string());
            }

            return Err(format!("关闭开机自启失败：{}", errors.join("；")));
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
