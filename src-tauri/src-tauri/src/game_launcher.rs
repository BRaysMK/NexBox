use serde::{Deserialize, Serialize};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x08000000;

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

    Command::new("cmd")
        .args(["/c", "start", "", &game_path])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("启动游戏失败: {}", e))?;

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
    let ps_script = r#"
        Add-Type -AssemblyName System.Windows.Forms
        $dialog = New-Object System.Windows.Forms.OpenFileDialog
        $dialog.Filter = "Executable Files (*.exe)|*.exe|All Files (*.*)|*.*"
        $dialog.Title = "Select Game Executable"
        if ($dialog.ShowDialog() -eq 'OK') {
            $dialog.FileName
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
