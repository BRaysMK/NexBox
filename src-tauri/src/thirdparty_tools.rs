use serde::{Deserialize, Serialize};
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use tauri::Emitter;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThirdPartyTool {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub tool_type: String,
    pub download_url: String,
    pub file_name: String,
    pub check_executable: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolWithStatus {
    pub tool: ThirdPartyTool,
    pub installed: bool,
}

fn get_tools_directory() -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("NexBox");
    path.push("Tools");
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path
}

fn get_tools_list() -> Vec<ThirdPartyTool> {
    vec![
        ThirdPartyTool {
            id: "memreduct".to_string(),
            name: "Memreduct".to_string(),
            description: "内存清理工具".to_string(),
            category: "optimization".to_string(),
            tool_type: "install".to_string(),
            download_url: "https://gitee.com/muliuawa/nexboxtools/releases/download/Tools/memreduct-3.5.2-setup.exe".to_string(),
            file_name: "memreduct-3.5.2-setup.exe".to_string(),
            check_executable: Some("memreduct.exe".to_string()),
        },
        ThirdPartyTool {
            id: "windows-core-optimizer".to_string(),
            name: "Windows大小核优化".to_string(),
            description: "仅对有大小核心的CPU有效".to_string(),
            category: "optimization".to_string(),
            tool_type: "portable".to_string(),
            download_url: "https://gitee.com/muliuawa/nexboxtools/releases/download/Tools/Windows%E5%A4%A7%E5%B0%8F%E6%A0%B8%E8%B0%83%E5%BA%A6%E4%B8%80%E9%94%AE%E4%BC%98%E5%8C%96_x64.exe".to_string(),
            file_name: "Windows大小核调度一键优化_x64.exe".to_string(),
            check_executable: None,
        },
        ThirdPartyTool {
            id: "optimizer".to_string(),
            name: "Optimizer".to_string(),
            description: "Windows系统优化工具".to_string(),
            category: "optimization".to_string(),
            tool_type: "portable".to_string(),
            download_url: "https://gitee.com/muliuawa/nexboxtools/releases/download/Tools/Optimizer-16.7.exe".to_string(),
            file_name: "Optimizer-16.7.exe".to_string(),
            check_executable: None,
        },
        ThirdPartyTool {
            id: "cpu-z".to_string(),
            name: "CPU-Z".to_string(),
            description: "CPU信息检测工具".to_string(),
            category: "hardware".to_string(),
            tool_type: "install".to_string(),
            download_url: "https://gitee.com/muliuawa/nexboxtools/releases/download/Tools/cpu-z_2.19-cn.exe".to_string(),
            file_name: "cpu-z_2.19-cn.exe".to_string(),
            check_executable: Some("cpuz.exe".to_string()),
        },
        ThirdPartyTool {
            id: "gpu-z".to_string(),
            name: "GPU-Z".to_string(),
            description: "显卡信息检测工具".to_string(),
            category: "hardware".to_string(),
            tool_type: "portable".to_string(),
            download_url: "https://gitee.com/muliuawa/nexboxtools/releases/download/Tools/GPU-Z.2.69.0.exe".to_string(),
            file_name: "GPU-Z.2.69.0.exe".to_string(),
            check_executable: None,
        },
        ThirdPartyTool {
            id: "clash-verge".to_string(),
            name: "Clash Verge".to_string(),
            description: "专业网络代理工具".to_string(),
            category: "network".to_string(),
            tool_type: "install".to_string(),
            download_url: "https://gitee.com/muliuawa/nexboxtools/releases/download/Tools/CV-2.2.3.exe".to_string(),
            file_name: "CV-2.2.3.exe".to_string(),
            check_executable: Some("clash-verge.exe".to_string()),
        },
        ThirdPartyTool {
            id: "process-lasso".to_string(),
            name: "Process Lasso".to_string(),
            description: "系统进程优化工具".to_string(),
            category: "optimization".to_string(),
            tool_type: "install".to_string(),
            download_url: "https://gitee.com/muliuawa/nexboxtools/releases/download/Tools/ProcesLasso.exe".to_string(),
            file_name: "ProcesLasso.exe".to_string(),
            check_executable: Some("ProcessLassoLauncher.exe".to_string()),
        },
        ThirdPartyTool {
            id: "fxsound".to_string(),
            name: "FxSound".to_string(),
            description: "音频增强工具".to_string(),
            category: "assistant".to_string(),
            tool_type: "install".to_string(),
            download_url: "https://gitee.com/muliuawa/nexboxtools/releases/download/Tools/fxsound_setup.exe".to_string(),
            file_name: "fxsound_setup.exe".to_string(),
            check_executable: Some("FxSound.exe".to_string()),
        },
        ThirdPartyTool {
            id: "msi-afterburner".to_string(),
            name: "MSI Afterburner".to_string(),
            description: "显卡超频监控工具".to_string(),
            category: "hardware".to_string(),
            tool_type: "install".to_string(),
            download_url: "https://gitee.com/muliuawa/nexboxtools/releases/download/Tools/MSIAfterburnerInstaller466.exe".to_string(),
            file_name: "MSIAfterburnerInstaller466.exe".to_string(),
            check_executable: Some("MSIAfterburner.exe".to_string()),
        },
        ThirdPartyTool {
            id: "geek".to_string(),
            name: "Geek".to_string(),
            description: "软件卸载清理工具".to_string(),
            category: "assistant".to_string(),
            tool_type: "portable".to_string(),
            download_url: "https://gitee.com/muliuawa/nexboxtools/releases/download/Tools/geek.exe".to_string(),
            file_name: "geek.exe".to_string(),
            check_executable: None,
        },
    ]
}

#[tauri::command]
pub fn get_thirdparty_tools() -> Vec<ThirdPartyTool> {
    get_tools_list()
}

#[tauri::command]
pub fn get_thirdparty_tools_with_status() -> Vec<ToolWithStatus> {
    let tools = get_tools_list();
    let tools_dir = get_tools_directory();

    tools
        .into_iter()
        .map(|tool| {
            let installed = match tool.tool_type.as_str() {
                "install" => {
                    if let Some(check_exe) = &tool.check_executable {
                        find_via_desktop_shortcut(check_exe).is_some()
                    } else {
                        false
                    }
                }
                "portable" => {
                    let tool_path = tools_dir.join(&tool.file_name);
                    tool_path.exists()
                }
                _ => false,
            };

            ToolWithStatus { tool, installed }
        })
        .collect()
}

#[tauri::command]
pub fn check_tool_installed(tool_id: String) -> bool {
    let tools = get_tools_list();
    if let Some(tool) = tools.iter().find(|t| t.id == tool_id) {
        match tool.tool_type.as_str() {
            "install" => {
                if let Some(check_exe) = &tool.check_executable {
                    find_via_desktop_shortcut(check_exe).is_some()
                } else {
                    false
                }
            }
            "portable" => {
                let tools_dir = get_tools_directory();
                let tool_path = tools_dir.join(&tool.file_name);
                tool_path.exists()
            }
            _ => false,
        }
    } else {
        false
    }
}

#[tauri::command]
pub fn get_tool_install_path(tool_id: String) -> Option<String> {
    let tools = get_tools_list();
    if let Some(tool) = tools.iter().find(|t| t.id == tool_id) {
        if tool.tool_type == "install" {
            if let Some(check_exe) = &tool.check_executable {
                return find_via_desktop_shortcut(check_exe)
                    .map(|p| p.to_string_lossy().into_owned());
            }
        }
    }
    None
}

fn find_via_desktop_shortcut(exe_name: &str) -> Option<PathBuf> {
    let desktop_paths = get_desktop_paths();
    let mut lnk_files: Vec<PathBuf> = Vec::new();

    for desktop_path in desktop_paths {
        if !desktop_path.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&desktop_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    if ext.eq_ignore_ascii_case("lnk") {
                        lnk_files.push(path);
                    }
                }
            }
        }
    }

    if lnk_files.is_empty() {
        return None;
    }

    let lnk_paths: Vec<String> = lnk_files
        .iter()
        .map(|p| format!("'{}'", p.to_string_lossy().escape_default()))
        .collect();

    let ps_script = format!(
        r#"$shell = New-Object -ComObject WScript.Shell; @({}) | ForEach-Object {{ $shell.CreateShortcut($_).TargetPath }}"#,
        lnk_paths.join(",")
    );

    if let Ok(output) = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                let target = line.trim();
                if !target.is_empty() {
                    let target_path = PathBuf::from(target);
                    if let Some(file_name) = target_path.file_name() {
                        if file_name.to_string_lossy().eq_ignore_ascii_case(exe_name) {
                            return Some(target_path);
                        }
                    }
                }
            }
        }
    }

    None
}

fn get_desktop_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(user_profile) = dirs::home_dir() {
        paths.push(user_profile.join("Desktop"));
    }

    paths.push(PathBuf::from("C:\\Users\\Public\\Desktop"));

    paths
}

#[tauri::command]
pub fn get_tool_download_path(tool_id: String) -> String {
    let tools = get_tools_list();
    if let Some(tool) = tools.iter().find(|t| t.id == tool_id) {
        let tools_dir = get_tools_directory();
        tools_dir
            .join(&tool.file_name)
            .to_string_lossy()
            .into_owned()
    } else {
        String::new()
    }
}

#[tauri::command]
pub fn run_tool(tool_id: String) -> Result<(), String> {
    let tools = get_tools_list();
    if let Some(tool) = tools.iter().find(|t| t.id == tool_id) {
        match tool.tool_type.as_str() {
            "install" => {
                if let Some(check_exe) = &tool.check_executable {
                    if let Some(exe_path) = find_via_desktop_shortcut(check_exe) {
                        Command::new("cmd")
                            .args(["/c", "start", "", exe_path.to_str().unwrap()])
                            .creation_flags(CREATE_NO_WINDOW)
                            .spawn()
                            .map_err(|e| e.to_string())?;
                        return Ok(());
                    }
                    return Err("Executable not found".to_string());
                }
            }
            "portable" => {
                let tools_dir = get_tools_directory();
                let tool_path = tools_dir.join(&tool.file_name);
                if tool_path.exists() {
                    Command::new("cmd")
                        .args(["/c", "start", "", tool_path.to_str().unwrap()])
                        .creation_flags(CREATE_NO_WINDOW)
                        .spawn()
                        .map_err(|e| e.to_string())?;
                } else {
                    return Err("Tool not found".to_string());
                }
            }
            _ => {}
        }
        Ok(())
    } else {
        Err("Tool not found".to_string())
    }
}

#[tauri::command]
pub async fn download_tool(tool_id: String, window: tauri::Window) -> Result<String, String> {
    let tools = get_tools_list();
    if let Some(tool) = tools.iter().find(|t| t.id == tool_id) {
        let tools_dir = get_tools_directory();
        let file_path = tools_dir.join(&tool.file_name);

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(|e| e.to_string())?;
        let response = client
            .get(&tool.download_url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let total_size = response.content_length().unwrap_or(0);

        let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;

        use futures_util::StreamExt;
        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| e.to_string())?;
            std::io::Write::write_all(&mut file, &chunk).map_err(|e| e.to_string())?;
            downloaded += chunk.len() as u64;

            let progress = if total_size > 0 {
                ((downloaded * 100) / total_size) as u32
            } else {
                0
            };

            let _ = window.emit(
                "tool-download-progress",
                serde_json::json!({
                    "tool_id": tool_id,
                    "progress": progress,
                }),
            );
        }

        Ok(file_path.to_string_lossy().into_owned())
    } else {
        Err("Tool not found".to_string())
    }
}

#[tauri::command]
pub async fn open_tool_installer(file_path: String) -> Result<(), String> {
    Command::new("cmd")
        .args(["/c", "start", "", &file_path])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
