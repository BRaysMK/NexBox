use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
use winreg::enums::*;
use winreg::RegKey;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(target_os = "windows")]
extern "system" {
    fn GetDiskFreeSpaceExW(
        lpDirectoryName: *const u16,
        lpFreeBytesAvailableToCaller: *mut u64,
        lpTotalNumberOfBytes: *mut u64,
        lpTotalNumberOfFreeBytes: *mut u64,
    ) -> i32;
}

/// UTF-16LE + Base64 编码 PowerShell 脚本
/// 这是 Windows 上传递含非 ASCII 字符脚本的唯一可靠方式。
/// `-EncodedCommand` 参数支持 UTF-16LE Base64，完全绕过系统代码页问题。
fn encode_ps_command(script: &str) -> String {
    let utf16: Vec<u8> = script
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();
    base64_encode(&utf16)
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0] as u32, *chunk.get(1).unwrap_or(&0) as u32, *chunk.get(2).unwrap_or(&0) as u32];
        let n = (b[0] << 16) | (b[1] << 8) | b[2];
        out.push(CHARS[((n >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((n >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 { CHARS[((n >> 6) & 0x3F) as usize] } else { b'=' } as char);
        out.push(if chunk.len() > 2 { CHARS[(n & 0x3F) as usize] } else { b'=' } as char);
    }
    out
}

/// 通过 Base64 编码执行 PowerShell 脚本，避免系统代码页导致的乱码。
/// 脚本以 UTF-8 传入，内部自动转为 UTF-16LE Base64。
/// stdout 通过 `[Console]::OutputEncoding` 强制为 UTF-8 返回。
fn run_powershell(script: &str) -> Result<String, String> {
    let full_script = format!(
        "[Console]::OutputEncoding = [Text.Encoding]::UTF8; {}",
        script
    );
    let encoded = encode_ps_command(&full_script);

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &encoded])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("无法执行 PowerShell: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PowerShell 执行失败: {}", stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

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
        .display()
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

    // Cleanup old Inno Setup artifacts before installing
    cleanup_old_innosetup(&target);

    // Create target directory
    fs::create_dir_all(&target)
        .map_err(|e| format!("无法创建目标目录: {}", e))?;

    // Extract payload ZIP to target directory
    extract_payload(&target)?;

    // Remove any existing shortcuts to avoid duplicates from old version
    delete_existing_shortcuts("新境盒");

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
    // 使用 display() 保留原始路径编码，再转义单引号
    let target_str = target_exe.display().to_string().replace('\'', "''");
    let workdir = target_exe
        .parent()
        .unwrap_or(Path::new(""))
        .display()
        .to_string()
        .replace('\'', "''");
    let shortcut_path = format!("{}\\{}.lnk", folder, name).replace('\'', "''");

    let ps_script = format!(
        "$sh = New-Object -ComObject WScript.Shell; \
         $lnk = $sh.CreateShortcut('{sc}'); \
         $lnk.TargetPath = '{exe}'; \
         $lnk.WorkingDirectory = '{wd}'; \
         $lnk.Save()",
        sc = shortcut_path,
        exe = target_str,
        wd = workdir,
    );

    run_powershell(&ps_script)
        .map_err(|e| format!("创建快捷方式失败: {}", e))?;
    Ok(())
}

fn get_special_folder_path(folder: &str) -> Option<String> {
    let ps_cmd = if folder == "Desktop" {
        "[Environment]::GetFolderPath('Desktop')"
    } else {
        "[Environment]::GetFolderPath('CommonStartMenu') + '\\Programs'"
    };

    run_powershell(ps_cmd).ok()
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

/// Remove old Inno Setup uninstaller files (unins000.exe / unins000.dat)
fn cleanup_old_innosetup(target: &Path) {
    for name in &["unins000.exe", "unins000.dat"] {
        let path = target.join(name);
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
    }
}

/// Delete existing shortcuts before creating new ones to avoid duplicates
fn delete_existing_shortcuts(name: &str) {
    // Delete from Desktop
    if let Some(desktop) = get_special_folder_path("Desktop") {
        let path = format!("{}\\{}.lnk", desktop, name);
        if Path::new(&path).exists() {
            let _ = fs::remove_file(&path);
        }
    }
    // Delete from Start Menu
    if let Some(start_menu) = get_special_folder_path("StartMenu") {
        let path = format!("{}\\{}.lnk", start_menu, name);
        if Path::new(&path).exists() {
            let _ = fs::remove_file(&path);
        }
    }
}

fn fs2_available_space(path: &Path) -> Result<u64, std::io::Error> {
    let path_str = path.display().to_string();
    // 提取盘符（从路径开头取第一个字符 + ':'）
    let drive_letter = path_str
        .chars()
        .next()
        .filter(|c| c.is_ascii_alphabetic())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "无效的路径"))?;

    // 方案1: Win32 API GetDiskFreeSpaceExW —— 不依赖 PowerShell，不受安全软件/执行策略影响
    #[cfg(target_os = "windows")]
    {
        let root_path = format!("{}:\\", drive_letter);
        let wide_path: Vec<u16> = std::ffi::OsStr::new(&root_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut free_bytes: u64 = 0;
        let mut total_bytes: u64 = 0;
        let mut total_free_bytes: u64 = 0;

        unsafe {
            let result = GetDiskFreeSpaceExW(
                wide_path.as_ptr(),
                &mut free_bytes as *mut u64,
                &mut total_bytes as *mut u64,
                &mut total_free_bytes as *mut u64,
            );
            if result != 0 {
                return Ok(free_bytes);
            }
            // API 调用失败，记录错误信息供排查
            let api_error = std::io::Error::last_os_error();
            eprintln!(
                "GetDiskFreeSpaceExW({}) 失败: {}，回退到 PowerShell",
                root_path, api_error
            );
        }
    }

    // 方案2: PowerShell Get-CimInstance Win32_LogicalDisk（降级方案）
    let ps_cmd = format!(
        "(Get-CimInstance Win32_LogicalDisk -Filter \"DeviceID='{}:'\").FreeSpace",
        drive_letter
    );
    match run_powershell(&ps_cmd) {
        Ok(stdout) => {
            // 清理可能的 BOM 字符（U+FEFF）
            let cleaned = stdout.trim().trim_start_matches('\u{feff}');
            if let Ok(bytes) = cleaned.parse::<u64>() {
                return Ok(bytes);
            }
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "无法解析磁盘空间数值 (盘符 {}:): '{}'",
                    drive_letter,
                    stdout.trim()
                ),
            ))
        }
        Err(e) => Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("无法获取磁盘空间 (盘符 {}:): {}", drive_letter, e),
        )),
    }
}
