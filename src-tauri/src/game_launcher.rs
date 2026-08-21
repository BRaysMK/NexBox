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
        log::warn!("[GameLauncher] 图标提取失败：文件不存在 - {}", file_path);
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
    // 优先用共享图标提取逻辑（与启动项管理一致，高分辨率不模糊）。
    // 部分 Squirrel 打包的 Electron 应用 stub（如某些 exe）不内嵌图标资源，
    // 提取失败时回退到同目录的 .ico 文件。
    let data_uri = crate::startup_manager::extract_icon_data_uri(&icon_src)
        .or_else(|| fallback_ico_data_uri(&icon_src))
        .ok_or_else(|| "无法提取图标".to_string())
        .map_err(|e| {
            log::warn!("[GameLauncher] 图标提取失败（{}）：{}", e, icon_src);
            e
        })?;

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

/// 从与目标文件同目录的 .ico 文件读取图标，编码为 PNG data URI。
/// 用于 Squirrel 打包的 Electron 应用 stub（如「东东电竞.exe」）等不内嵌图标资源的场景。
fn fallback_ico_data_uri(file_path: &str) -> Option<String> {
    let path = PathBuf::from(file_path);

    // 收集同目录下的 .ico 候选：同名 ico 优先，其次目录下任意 .ico
    let mut candidates: Vec<PathBuf> = Vec::new();
    let same_name = path.with_extension("ico");
    if same_name.exists() {
        candidates.push(same_name);
    }
    if let Some(dir) = path.parent() {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().map(|e| e.to_string_lossy().to_lowercase() == "ico").unwrap_or(false) {
                    // 去重（避免与同名 ico 重复）
                    if !candidates.iter().any(|c| c == &p) {
                        candidates.push(p);
                    }
                }
            }
        }
    }

    for ico_path in candidates {
        let Ok(bytes) = fs::read(&ico_path) else { continue };
        // 仅解码 ico 格式；从容器中取最大分辨率帧
        let Ok(icon) = image::load_from_memory_with_format(&bytes, image::ImageFormat::Ico) else {
            continue;
        };
        let img = icon.into_rgba8();
        let mut out = std::io::Cursor::new(Vec::new());
        if image::DynamicImage::ImageRgba8(img)
            .write_to(&mut out, image::ImageFormat::Png)
            .is_err()
        {
            continue;
        }
        let b64 = base64::engine::general_purpose::STANDARD.encode(out.into_inner());
        log::info!("[GameLauncher] 使用同目录 .ico 文件作为图标：{}", ico_path.display());
        return Some(format!("data:image/png;base64,{}", b64));
    }

    None
}
