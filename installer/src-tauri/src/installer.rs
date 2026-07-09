use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use winreg::enums::*;
use winreg::RegKey;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Serialize)]
pub struct FileEntry {
    relative_path: String,
    size: u64,
}

/// Embedded payload ZIP (created by build.ps1 staging step)
const PAYLOAD_ZIP: &[u8] = include_bytes!("../payload.zip");

#[tauri::command]
pub fn get_default_install_path() -> String {
    let program_files = std::env::var("ProgramFiles")
        .unwrap_or_else(|_| "C:\\Program Files".to_string());
    Path::new(&program_files)
        .join("NexBox")
        .to_string_lossy()
        .to_string()
}

#[tauri::command]
pub fn check_disk_space(path: String) -> Result<u64, String> {
    let path = Path::new(&path);
    let available = fs2_available_space(path).map_err(|e| format!("无法检查磁盘空间: {}", e))?;
    Ok(available)
}

#[tauri::command]
pub fn get_app_version() -> String {
    env!("NEXBOX_APP_VERSION").to_string()
}

#[tauri::command]
pub fn get_resource_files() -> Result<Vec<FileEntry>, String> {
    let cursor = std::io::Cursor::new(PAYLOAD_ZIP);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("无法读取安装包: {}", e))?;

    let mut files = Vec::new();
    for i in 0..archive.len() {
        let file = archive.by_index(i).map_err(|e| format!("读取文件失败: {}", e))?;
        if !file.name().ends_with('/') {
            files.push(FileEntry {
                relative_path: file.name().to_string(),
                size: file.size(),
            });
        }
    }
    Ok(files)
}

#[tauri::command]
pub fn install(
    target_dir: String,
    create_desktop_shortcut: bool,
) -> Result<(), String> {
    let target = PathBuf::from(&target_dir);

    // Create target directory
    fs::create_dir_all(&target)
        .map_err(|e| format!("无法创建目标目录: {}", e))?;

    // Extract payload ZIP to target directory
    extract_payload(&target)?;

    // Create Start Menu shortcut
    let exe_path = target.join("nexbox.exe");
    create_lnk_shortcut("新境盒", &exe_path, "StartMenu")
        .map_err(|e| format!("无法创建开始菜单快捷方式: {}", e))?;

    // Create Desktop shortcut if requested
    if create_desktop_shortcut {
        create_lnk_shortcut("新境盒", &exe_path, "Desktop")
            .map_err(|e| format!("无法创建桌面快捷方式: {}", e))?;
    }

    // Register uninstaller
    register_uninstall(&target_dir, env!("NEXBOX_APP_VERSION"))
        .map_err(|e| format!("无法注册卸载信息: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn cancel_install(target_dir: String) -> Result<(), String> {
    let path = Path::new(&target_dir);
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|e| format!("无法清理安装目录: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn launch_installed_app(target_dir: String) -> Result<(), String> {
    let exe_path = Path::new(&target_dir).join("nexbox.exe");
    if !exe_path.exists() {
        return Err("未找到 nexbox.exe".to_string());
    }

    Command::new(&exe_path)
        .spawn()
        .map_err(|e| format!("无法启动应用: {}", e))?;

    Ok(())
}

// === Payload extraction ===

fn extract_payload(target_dir: &Path) -> Result<(), String> {
    let cursor = std::io::Cursor::new(PAYLOAD_ZIP);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("无法读取安装包: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("读取文件失败: {}", e))?;

        let name = file.name().to_string();
        let outpath = target_dir.join(&name);

        if name.ends_with('/') {
            fs::create_dir_all(&outpath)
                .map_err(|e| format!("无法创建目录 {}: {}", outpath.display(), e))?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("无法创建目录 {}: {}", parent.display(), e))?;
            }
            let mut outfile = fs::File::create(&outpath)
                .map_err(|e| format!("无法创建文件 {}: {}", outpath.display(), e))?;
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("无法写入文件 {}: {}", outpath.display(), e))?;
        }
    }

    Ok(())
}

// === Shortcut creation via PowerShell ===

fn create_lnk_shortcut(name: &str, target_exe: &Path, location: &str) -> Result<(), String> {
    let folder = if location == "Desktop" {
        get_special_folder_path("Desktop")
    } else {
        get_special_folder_path("StartMenu")
    };

    let folder = folder.ok_or_else(|| "无法获取系统目录路径".to_string())?;
    let target_str = target_exe.to_string_lossy();
    let workdir = target_exe.parent().unwrap_or(Path::new("")).to_string_lossy();
    let shortcut_path = format!("{}\\{}.lnk", folder, name);

    let ps_script = format!(
        "$sh = New-Object -ComObject WScript.Shell; \
         $lnk = $sh.CreateShortcut('{sc}'); \
         $lnk.TargetPath = '{exe}'; \
         $lnk.WorkingDirectory = '{wd}'; \
         $lnk.Save()",
        sc = shortcut_path.replace('\'', "''"),
        exe = target_str.replace('\'', "''"),
        wd = workdir.replace('\'', "''"),
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("无法执行 PowerShell: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PowerShell 创建快捷方式失败: {}", stderr));
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

// === Registry operations ===

fn register_uninstall(install_dir: &str, version: &str) -> Result<(), String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let uninstall_path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\NexBox";

    let (key, _) = hklm
        .create_subkey(uninstall_path)
        .map_err(|e| format!("无法创建注册表键: {}", e))?;

    let icon_path = format!("{}\\nexbox.exe", install_dir);
    let uninstaller_path = format!("{}\\Uninstnexbox.exe", install_dir);

    key.set_value("DisplayName", &"新境盒")
        .map_err(|e| format!("写入注册表失败: {}", e))?;
    key.set_value("DisplayVersion", &version)
        .map_err(|e| format!("写入注册表失败: {}", e))?;
    key.set_value("DisplayIcon", &icon_path)
        .map_err(|e| format!("写入注册表失败: {}", e))?;
    key.set_value("Publisher", &"MuLiu")
        .map_err(|e| format!("写入注册表失败: {}", e))?;
    key.set_value("InstallLocation", &install_dir)
        .map_err(|e| format!("写入注册表失败: {}", e))?;
    key.set_value("UninstallString", &uninstaller_path)
        .map_err(|e| format!("写入注册表失败: {}", e))?;
    key.set_value("NoModify", &1u32)
        .map_err(|e| format!("写入注册表失败: {}", e))?;
    key.set_value("NoRepair", &1u32)
        .map_err(|e| format!("写入注册表失败: {}", e))?;
    key.set_value("EstimatedSize", &250_000u32)
        .map_err(|e| format!("写入注册表失败: {}", e))?;
    key.set_value("URLInfoAbout", &"https://www.nexbox.top/")
        .map_err(|e| format!("写入注册表失败: {}", e))?;

    Ok(())
}

// === Disk space check ===

fn fs2_available_space(path: &Path) -> Result<u64, std::io::Error> {
    let path_str = path.to_string_lossy();
    let drive = if path_str.len() >= 2 && path_str.as_bytes()[1] == b':' {
        format!("{}", &path_str[..2])
    } else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "无效的路径",
        ));
    };

    let ps_cmd = format!(
        "(Get-PSDrive -Name '{}').Free",
        drive.trim_end_matches(':')
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_cmd])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let trimmed = stdout.trim();
        if let Ok(bytes) = trimmed.parse::<u64>() {
            return Ok(bytes);
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "无法获取磁盘空间信息",
    ))
}
