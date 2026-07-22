use serde::Serialize;
use std::fs;
use std::path::Path;
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use winreg::enums::*;
use winreg::RegKey;

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
        .display()
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

    let exe_str = exe_path.display().to_string();
    let dir_str = install_dir.display().to_string();

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

    // VBS 文件使用 UTF-16 LE + BOM 确保中文路径正确解析
    let mut vbs_bytes: Vec<u8> = vec![0xFF, 0xFE];
    for c in vbs_content.encode_utf16() {
        vbs_bytes.extend_from_slice(&c.to_le_bytes());
    }
    fs::write(&vbs_path, &vbs_bytes)
        .map_err(|e| format!("无法创建清理脚本: {}", e))?;

    #[cfg(target_os = "windows")]
    {
        const DETACHED_PROCESS: u32 = 0x00000008;
        Command::new("wscript.exe")
            .args(["//B", "//Nologo", &vbs_path.display().to_string()])
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
    if folder == "Desktop" {
        dirs::desktop_dir().map(|p| p.to_string_lossy().to_string())
    } else {
        get_common_programs_path()
    }
}

#[cfg(target_os = "windows")]
fn get_common_programs_path() -> Option<String> {
    use std::os::windows::ffi::OsStringExt;

    extern "system" {
        fn SHGetFolderPathW(
            hwnd: *mut std::ffi::c_void,
            csidl: i32,
            h_token: *mut std::ffi::c_void,
            dw_flags: u32,
            psz_path: *mut u16,
        ) -> i32;
    }

    const CSIDL_COMMON_PROGRAMS: i32 = 0x0017;
    let mut buf = vec![0u16; 260]; // MAX_PATH

    unsafe {
        let result = SHGetFolderPathW(
            std::ptr::null_mut(),
            CSIDL_COMMON_PROGRAMS,
            std::ptr::null_mut(),
            0,
            buf.as_mut_ptr(),
        );
        if result == 0 {
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            return Some(std::ffi::OsString::from_wide(&buf[..len]).to_string_lossy().to_string());
        }
    }
    None
}

fn unregister_uninstall() -> Result<(), String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let uninstall_path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\NexBox";

    hklm.delete_subkey_all(uninstall_path)
        .map_err(|e| format!("无法删除注册表键: {}", e))?;

    // Also clean up old Inno Setup registry entries
    cleanup_old_innosetup_registry();

    Ok(())
}

/// Remove leftover Inno Setup uninstall registry entries.
/// This handles the case where a user installs the new version
/// over an old INNO installation without the installer catching it.
fn cleanup_old_innosetup_registry() {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    let hive_paths = [
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
    ];

    for hive in &hive_paths {
        if let Ok(uninstall_key) = hklm.open_subkey_with_flags(hive, KEY_ALL_ACCESS) {
            let to_delete: Vec<String> = uninstall_key
                .enum_keys()
                .filter_map(|k| k.ok())
                .filter(|k| k.ends_with("_is1"))
                .filter(|key_name| {
                    if let Ok(subkey) =
                        uninstall_key.open_subkey_with_flags(key_name, KEY_READ)
                    {
                        if let Ok(name) = subkey.get_value::<String, _>("DisplayName") {
                            return name.contains("新境盒")
                                || name.to_lowercase().contains("nexbox");
                        }
                    }
                    false
                })
                .collect();

            for key_name in &to_delete {
                let _ = uninstall_key.delete_subkey_all(key_name);
            }
        }
    }
}
