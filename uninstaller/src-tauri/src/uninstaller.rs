use serde::Serialize;
use std::fs;
use std::path::Path;
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use winreg::enums::*;
use winreg::RegKey;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Serialize)]
pub struct UninstallInfo {
    pub install_dir: String,
    pub app_name: String,
}

#[derive(Serialize)]
pub struct UninstallProgress {
    pub percent: u32,
    pub message: String,
    pub done: bool,
}

#[tauri::command]
pub fn get_install_info() -> Result<UninstallInfo, String> {
    let exe_path = std::env::current_exe().map_err(|e| format!("获取路径失败: {}", e))?;
    let install_dir = exe_path
        .parent()
        .ok_or("无法获取安装目录")?
        .to_string_lossy()
        .to_string();

    Ok(UninstallInfo {
        install_dir,
        app_name: "新境盒".to_string(),
    })
}

#[tauri::command]
pub fn start_uninstall() -> Result<UninstallProgress, String> {
    let exe_path = std::env::current_exe().map_err(|e| format!("获取路径失败: {}", e))?;
    let install_dir = exe_path
        .parent()
        .ok_or("无法获取安装目录")?
        .to_path_buf();

    // 1. Delete all files recursively (except self)
    delete_directory_contents(&install_dir, &exe_path)?;

    // 2. Delete Start Menu shortcut
    delete_shortcut("新境盒", "StartMenu")
        .map_err(|e| eprintln!("删除开始菜单快捷方式失败: {}", e)).ok();

    // 3. Delete Desktop shortcut
    delete_shortcut("新境盒", "Desktop")
        .map_err(|e| eprintln!("删除桌面快捷方式失败: {}", e)).ok();

    // 4. Delete registry entry
    unregister_uninstall()
        .map_err(|e| eprintln!("删除注册表项失败: {}", e)).ok();

    Ok(UninstallProgress {
        percent: 100,
        message: "卸载完成".to_string(),
        done: true,
    })
}

#[tauri::command]
pub fn self_delete() -> Result<(), String> {
    let exe_path = std::env::current_exe().map_err(|e| format!("获取路径失败: {}", e))?;
    let install_dir = exe_path
        .parent()
        .ok_or("无法获取安装目录")?
        .to_path_buf();

    let temp_dir = std::env::temp_dir();
    let vbs_path = temp_dir.join("nexbox_cleanup.vbs");

    let exe_str = exe_path.to_string_lossy().to_string();
    let dir_str = install_dir.to_string_lossy().to_string();

    let vbs_content = format!(
        r#"Set WMI = GetObject("winmgmts:root\cimv2")
Set Processes = WMI.ExecQuery("SELECT * FROM Win32_Process WHERE Name='uninstnexbox.exe'")
For Each P In Processes
    P.Terminate()
Next
WScript.Sleep 3000
Set F = CreateObject("Scripting.FileSystemObject")
On Error Resume Next
F.DeleteFile "{}", True
F.DeleteFolder "{}", True
"#,
        exe_str, dir_str
    );

    fs::write(&vbs_path, vbs_content)
        .map_err(|e| format!("无法创建清理脚本: {}", e))?;

    #[cfg(target_os = "windows")]
    {
        const DETACHED_PROCESS: u32 = 0x00000008;
        Command::new("wscript.exe")
            .args(["//B", "//Nologo", &vbs_path.to_string_lossy()])
            .creation_flags(DETACHED_PROCESS)
            .spawn()
            .map_err(|e| format!("无法执行清理脚本: {}", e))?;
    }

    Ok(())
}

fn delete_directory_contents(dir: &Path, exclude: &Path) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }

    let entries = fs::read_dir(dir)
        .map_err(|e| format!("无法读取目录: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let path = entry.path();

        if path == exclude {
            continue;
        }

        if path.is_dir() {
            let _ = fs::remove_dir_all(&path);
        } else {
            let _ = fs::remove_file(&path);
        }
    }

    Ok(())
}

fn delete_shortcut(name: &str, location: &str) -> Result<(), String> {
    let folder = if location == "Desktop" {
        get_special_folder_path("Desktop")
    } else {
        get_special_folder_path("StartMenu")
    };

    let folder = folder.ok_or_else(|| "无法获取系统目录路径".to_string())?;
    let shortcut_path = format!("{}\\{}.lnk", folder, name);

    if Path::new(&shortcut_path).exists() {
        fs::remove_file(&shortcut_path)
            .map_err(|e| format!("无法删除快捷方式 {}: {}", shortcut_path, e))?;
    }

    Ok(())
}

fn get_special_folder_path(folder: &str) -> Option<String> {
    let ps_cmd = if folder == "Desktop" {
        "[Environment]::GetFolderPath('Desktop')"
    } else {
        "[Environment]::GetFolderPath('CommonStartMenu') + '\\Programs'"
    };

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", ps_cmd])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn unregister_uninstall() -> Result<(), String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let uninstall_path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\NexBox";

    hklm.delete_subkey_all(uninstall_path)
        .map_err(|e| format!("无法删除注册表键: {}", e))?;

    Ok(())
}
