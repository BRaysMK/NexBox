use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct MusicFile {
    pub name: String,
    pub path: String,
}

#[tauri::command]
pub async fn get_music_files() -> Result<Vec<MusicFile>, String> {
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let music_dir = cwd.join("public").join("music");

    if !music_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    let supported_extensions = ["mp3", "wav", "ogg", "m4a", "flac", "aac"];

    let entries = std::fs::read_dir(&music_dir).map_err(|error| error.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_lowercase())
            .unwrap_or_default();

        if !supported_extensions.iter().any(|item| *item == extension) {
            continue;
        }

        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .map(|value| value.to_string())
            .unwrap_or_default();

        if name.is_empty() {
            continue;
        }

        files.push(MusicFile {
            name: name.clone(),
            path: format!("music/{name}"),
        });
    }

    files.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    Ok(files)
}
