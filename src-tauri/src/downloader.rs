use std::fs;
use std::fs::File;
use std::io::Write;
use std::process::Command;
use std::os::windows::process::CommandExt;

use futures_util::StreamExt;
use reqwest::Client;
use tauri::{Window, Emitter, AppHandle};

#[derive(Clone, serde::Serialize)]
struct DownloadProgress {
    progress: u64,
    total: u64,
}

#[tauri::command]
pub async fn download_file(
    url: String,
    file_name: String,
    window: Window,
) -> Result<String, String> {
    let client = Client::new();
    let response = client.get(&url).send().await.map_err(|e| e.to_string())?;

    let total_size = response.content_length().unwrap_or(0);

    let download_path = match dirs::download_dir() {
        Some(mut path) => {
            path.push(file_name);
            path
        }
        None => {
            let mut path = std::env::current_dir().map_err(|e| e.to_string())?;
            path.push(file_name);
            path
        }
    };

    let mut file = File::create(&download_path).map_err(|e| e.to_string())?;

    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;

        let progress = if total_size > 0 {
            (downloaded * 100) / total_size
        } else {
            0
        };

        match window.emit(
            "download-progress",
            DownloadProgress {
                progress,
                total: total_size,
            },
        ) {
            Ok(_) => {},
            Err(e) => {
                eprintln!("Failed to emit progress: {}", e);
            }
        }
    }

    file.flush().map_err(|e| e.to_string())?;

    Ok(download_path.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn open_installer(file_path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new("cmd")
            .args(["/c", "start", "", &file_path])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        return Err("Only Windows is supported".to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn download_update(
    url: String,
    file_name: String,
    window: Window,
) -> Result<String, String> {
    download_file(url, file_name, window).await
}

#[tauri::command]
pub async fn install_update(
    file_path: String,
    app_handle: AppHandle,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let file_path_wide: Vec<u16> = file_path
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let verb: Vec<u16> = "open\0".encode_utf16().collect();

        // 使用 ShellExecuteW 直接启动安装包，避免 cmd 中介弹出终端窗口
        let hinst = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                file_path_wide.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };

        // ShellExecuteW 返回值 <= 32 表示错误
        if hinst as isize <= 32 {
            return Err(format!("Failed to launch installer (error: {})", hinst as isize));
        }
    }

    std::thread::sleep(std::time::Duration::from_millis(500));

    app_handle.exit(0);

    Ok(())
}

#[tauri::command]
pub async fn delete_download_file(file_path: String) -> Result<(), String> {
    if std::path::Path::new(&file_path).exists() {
        fs::remove_file(&file_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
