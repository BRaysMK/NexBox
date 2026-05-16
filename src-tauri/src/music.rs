use serde::{Deserialize, Serialize};
use std::fs;
use tauri::Manager;

#[derive(Serialize, Deserialize, Clone)]
pub struct MusicFile {
    pub name: String,
    pub path: String,
}

#[tauri::command]
pub fn get_music_files(app: tauri::AppHandle) -> Result<Vec<MusicFile>, String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource directory: {}", e))?;

    let candidates = [
        ("public/music", "public/music"),
        ("music", "music"),
        ("_up_/public/music", "public/music"),
    ];

    let mut music_dir_path: Option<std::path::PathBuf> = None;
    let mut url_prefix = "public/music";

    for (dir_suffix, prefix) in candidates.iter() {
        let candidate = resource_dir.join(dir_suffix);
        if candidate.exists() {
            music_dir_path = Some(candidate);
            url_prefix = prefix;
            break;
        }
    }

    let music_dir = match music_dir_path {
        Some(dir) => dir,
        None => return Ok(Vec::new()),
    };

    if !music_dir.exists() {
        return Ok(Vec::new());
    }

    let mut music_files = Vec::new();

    let entries = fs::read_dir(&music_dir)
        .map_err(|e| format!("Failed to read music directory: {}", e))?;

    for entry in entries {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    let ext = extension.to_string_lossy().to_lowercase();
                    if ext == "mp3" || ext == "wav" || ext == "ogg" {
                        let name = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Unknown".to_string());

                        if let Some(file_name) = path.file_name() {
                            let file_name_str = file_name.to_string_lossy().to_string();
                            let relative_path = format!("{}/{}", url_prefix, file_name_str);

                            music_files.push(MusicFile {
                                name,
                                path: relative_path,
                            });
                        }
                    }
                }
            }
        }
    }

    music_files.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(music_files)
}
