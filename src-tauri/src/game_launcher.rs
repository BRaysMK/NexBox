use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x08000000;

/// 图标缓存目录：%LOCALAPPDATA%/NexBox/game-icons/
fn icon_cache_dir() -> PathBuf {
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("NexBox").join("game-icons")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameShortcut {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub is_default: bool,
}

#[tauri::command]
pub async fn launch_game(game_path: String) -> Result<(), String> {
    let path = PathBuf::from(&game_path);
    if !path.exists() {
        return Err(format!("游戏路径不存在: {}", game_path));
    }

    // 统一通过 explorer.exe 启动（支持 .exe / .lnk / 文件夹等）。
    // 原因：NexBox 的 manifest 为 requireAdministrator，自身以管理员身份运行，
    // 直接 Command::new 启动的子进程会继承提升令牌而同样以管理员身份运行；
    // 而 explorer 运行在非提升的桌面 shell 中，由它代为启动即回到与正常双击一致的普通权限。
    Command::new("explorer")
        .arg(&game_path)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("启动失败: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn search_delta_force_launcher() -> Option<String> {
    let common_paths = [
        "D:\\Delta Force\\launcher\\delta_force_launcher.exe",
        "C:\\Delta Force\\launcher\\delta_force_launcher.exe",
        "E:\\Delta Force\\launcher\\delta_force_launcher.exe",
        "F:\\Delta Force\\launcher\\delta_force_launcher.exe",
    ];

    for path in &common_paths {
        if PathBuf::from(path).exists() {
            return Some(path.to_string());
        }
    }

    let ps_script = r#"
        $drives = Get-PSDrive -PSProvider FileSystem | Where-Object { $_.Root -match "^[C-Z]:" } | ForEach-Object { $_.Root }
        foreach ($drive in $drives) {
            $path = Join-Path $drive "Delta Force\launcher\delta_force_launcher.exe"
            if (Test-Path $path) {
                $path
                break
            }
        }
    "#;

    if let Ok(output) = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        if output.status.success() {
            let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !result.is_empty() {
                return Some(result);
            }
        }
    }

    None
}

#[tauri::command]
pub async fn get_default_delta_force_game() -> Option<GameShortcut> {
    let launcher_path = search_delta_force_launcher().await?;
    
    Some(GameShortcut {
        id: "delta-force".to_string(),
        name: "三角洲行动".to_string(),
        path: launcher_path,
        is_default: true,
    })
}

#[tauri::command]
pub async fn select_exe_file() -> Option<String> {
    let file = rfd::FileDialog::new()
        .set_title("选择游戏可执行文件")
        .add_filter("可执行文件和快捷方式", &["exe", "lnk"])
        .add_filter("所有文件", &["*"])
        .pick_file();

    file.map(|f| f.to_string_lossy().to_string())
}

/// 获取可执行文件或快捷方式的图标，返回 base64 PNG data URI
/// 复用启动项管理的 Shell API 提取方式（SHDefExtractIconW，256px 高清），不再走 PowerShell
#[tauri::command]
pub async fn get_file_icon(file_path: String) -> Result<String, String> {
    let path = PathBuf::from(&file_path);
    if !path.exists() {
        return Err("文件不存在".to_string());
    }

    // 检查缓存
    let cache_dir = icon_cache_dir();
    let cache_key = format!("icon_{}.png", hash_path(&file_path));
    let cache_path = cache_dir.join(&cache_key);

    if cache_path.exists() {
        if let Ok(bytes) = fs::read(&cache_path) {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            return Ok(format!("data:image/png;base64,{}", b64));
        }
    }

    // 用共享图标提取逻辑（与启动项管理一致，高分辨率不模糊）。
    // .lnk 先解析到目标 exe，避免显示快捷方式的小箭头。
    let icon_src = if file_path.to_lowercase().ends_with(".lnk") {
        crate::startup_manager::resolve_shortcut_target(&file_path)
            .unwrap_or_else(|| file_path.clone())
    } else {
        file_path
    };
    let data_uri = crate::startup_manager::extract_icon_data_uri(&icon_src)
        .ok_or_else(|| "无法提取图标".to_string())?;

    // 解码并存入缓存
    if let Ok(bytes) = base64::engine::general_purpose::STANDARD
        .decode(data_uri.trim_start_matches("data:image/png;base64,"))
    {
        let _ = fs::create_dir_all(&cache_dir);
        let _ = fs::write(&cache_path, &bytes);
    }

    Ok(data_uri)
}

/// 简单的路径哈希，用于缓存文件名（取前8个十六进制字符）
fn hash_path(p: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    p.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}
