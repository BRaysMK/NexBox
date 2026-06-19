use std::process::Command;

const APP_NAME: &str = "NexBox";
const REG_PATH: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

#[cfg(windows)]
fn exec_reg(args: &[&str]) -> Result<std::process::Output, String> {
    use std::os::windows::process::CommandExt;
    Command::new("reg")
        .args(args)
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| format!("执行注册表命令失败: {}", e))
}

#[tauri::command]
pub async fn set_nexbox_auto_start(enable: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        if enable {
            let app_path = std::env::current_exe()
                .map_err(|e| format!("获取程序路径失败: {}", e))?
                .to_string_lossy()
                .replace("/", "\\");

            let output = exec_reg(&["add", REG_PATH, "/v", APP_NAME, "/t", "REG_SZ", "/d", &app_path, "/f"])?;
            if !output.status.success() {
                let error = String::from_utf8_lossy(&output.stderr);
                return Err(format!("写入注册表失败: {}", error));
            }
            log::info!("开机自启已启用: {}", app_path);
            Ok(())
        } else {
            exec_reg(&["delete", REG_PATH, "/v", APP_NAME, "/f"])?;
            log::info!("开机自启已禁用");
            Ok(())
        }
    }

    #[cfg(not(windows))]
    {
        Err("当前平台不支持开机自启动设置".to_string())
    }
}

#[tauri::command]
pub async fn check_nexbox_auto_start() -> Result<bool, String> {
    #[cfg(windows)]
    {
        let output = exec_reg(&["query", REG_PATH, "/v", APP_NAME])?;
        Ok(output.status.success())
    }

    #[cfg(not(windows))]
    {
        Ok(false)
    }
}
