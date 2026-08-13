use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::Accessor;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct MusicFile {
    pub name: String,
    pub path: String,
}

/// 本地导入歌曲的元信息（provider 为 "local"）
#[derive(Debug, Clone, Serialize, Default)]
pub struct LocalSongInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub size: u64,
    pub extension: String,
    /// 标题（来自音频标签，可能为空）
    pub title: String,
    /// 艺术家（来自音频标签，可能为空）
    pub artist: String,
    /// 专辑（来自音频标签，可能为空）
    pub album: String,
    /// 时长（毫秒，来自音频属性）
    pub duration_ms: u64,
    /// 封面 data URI（base64，可能为空）
    pub cover: String,
    /// 封面来源："embedded" 内嵌 / "folder" 同目录图片 / "none" 无
    pub cover_source: String,
}

/// 常见的同目录专辑封面文件名（小写）
const FOLDER_COVER_NAMES: [&str; 8] = [
    "cover.jpg", "folder.jpg", "album.jpg", "front.jpg",
    "cover.png", "folder.png", "album.png", "front.png",
];

/// 读取单个音频文件的内嵌元数据（标题/艺术家/专辑/时长/封面）
/// 返回 (title, artist, album, duration_ms, cover_data_uri, cover_source)
fn read_audio_metadata(path: &std::path::Path) -> (String, String, String, u64, String, String) {
    let (mut title, mut artist, mut album) = (String::new(), String::new(), String::new());
    let mut duration_ms: u64 = 0;
    let mut cover = String::new();
    let mut cover_source = "none".to_string();

    if let Ok(tagged_file) = lofty::read_from_path(path) {
        // 时长（秒 → 毫秒）
        let secs = tagged_file.properties().duration().as_secs();
        duration_ms = secs.saturating_mul(1000);

        // 标签字段：优先主标签，回退到第一个标签
        let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());
        if let Some(tag) = tag {
            title = tag.title().map(|v| v.to_string()).unwrap_or_default();
            artist = tag.artist().map(|v| v.to_string()).unwrap_or_default();
            album = tag.album().map(|v| v.to_string()).unwrap_or_default();
        }

        // 封面：遍历所有标签中的所有内嵌图片，取第一张转 base64 data URI
        for t in tagged_file.tags() {
            for pic in t.pictures() {
                let data = pic.data();
                // 限制封面大小（原始数据 ≤ 1MB），避免超大封面拖慢存储与渲染
                if !data.is_empty() && data.len() <= 1024 * 1024 {
                    let mime = detect_image_mime(data);
                    cover = format!("data:{mime};base64,{}", BASE64.encode(data));
                    cover_source = "embedded".to_string();
                    break;
                }
            }
            if !cover.is_empty() {
                break;
            }
        }
    }

    // 兜底：音频没有内嵌封面时，尝试读取同目录的封面图片文件
    // 顺序：固定封面名（cover/folder/album/front）→ 与音频同名的图片
    if cover.is_empty() {
        if let Some(dir) = path.parent() {
            for name in FOLDER_COVER_NAMES {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    cover = file_to_data_uri(&candidate);
                    if !cover.is_empty() {
                        cover_source = "folder".to_string();
                        break;
                    }
                }
            }
            if cover.is_empty() {
                if let Some(stem) = path.file_stem().and_then(|v| v.to_str()) {
                    for ext in ["jpg", "png", "jpeg", "webp", "bmp"] {
                        let candidate = dir.join(format!("{stem}.{ext}"));
                        if candidate.is_file() {
                            cover = file_to_data_uri(&candidate);
                            if !cover.is_empty() {
                                cover_source = "folder".to_string();
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    log::debug!(
        "local music metadata: title={title:?} artist={artist:?} album={album:?} dur={duration_ms}ms cover_source={cover_source} cover_bytes={}",
        cover.len()
    );

    (title, artist, album, duration_ms, cover, cover_source)
}

/// 支持的本地音频扩展名
pub const SUPPORTED_AUDIO_EXTENSIONS: [&str; 11] = [
    "mp3", "wav", "ogg", "m4a", "flac", "aac", "opus", "wma", "aiff", "ape", "oga",
];

/// 检查某个扩展名是否受支持（传入时已去掉点号并小写）
fn is_supported_extension(ext: &str) -> bool {
    SUPPORTED_AUDIO_EXTENSIONS.iter().any(|item| *item == ext)
}

/// 通过图片数据的 magic bytes 判断 MIME 类型，比标签声明的 MIME 更可靠
fn detect_image_mime(data: &[u8]) -> &'static str {
    if data.len() >= 8 && data[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        "image/png"
    } else if data.len() >= 3 && data[..3] == [0xFF, 0xD8, 0xFF] {
        "image/jpeg"
    } else if data.len() >= 6 && (&data[..6] == b"GIF87a" || &data[..6] == b"GIF89a") {
        "image/gif"
    } else if data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        "image/webp"
    } else if data.len() >= 2 && data[..2] == [0x42, 0x4D] {
        "image/bmp"
    } else if data.len() >= 12 && &data[..12] == b"AVIF" {
        "image/avif"
    } else {
        // 无法识别时回退到标签声明的类型
        "image/jpeg"
    }
}

/// 将图片文件读取为 base64 data URI
fn file_to_data_uri(path: &std::path::Path) -> String {
    let mime = match path
        .extension()
        .and_then(|v| v.to_str())
        .map(|v| v.to_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("bmp") => "image/bmp",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    };
    match std::fs::read(path) {
        Ok(data) if !data.is_empty() && data.len() <= 1024 * 1024 => {
            format!("data:{mime};base64,{}", BASE64.encode(data))
        }
        _ => String::new(),
    }
}

#[tauri::command]
pub async fn get_music_files() -> Result<Vec<MusicFile>, String> {
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let music_dir = cwd.join("public").join("music");

    if !music_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();

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

        if !is_supported_extension(&extension) {
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

/// 导入本地音频文件，返回可直接加入播放列表的歌曲元信息。
/// 不复制文件，仅校验扩展名与存在性，并读取文件大小。
#[tauri::command]
pub async fn import_local_music(paths: Vec<String>) -> Result<Vec<LocalSongInfo>, String> {
    let mut results = Vec::new();
    for raw_path in paths {
        let path = std::path::Path::new(&raw_path);
        if !path.is_file() {
            continue;
        }

        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_lowercase())
            .unwrap_or_default();

        if !is_supported_extension(&extension) {
            continue;
        }

        let name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                path.file_name()
                    .and_then(|value| value.to_str())
                    .map(|value| value.to_string())
                    .unwrap_or_default()
            });

        let size = std::fs::metadata(&path)
            .map(|meta| meta.len())
            .unwrap_or(0);

        // 读取内嵌元数据（标题/艺术家/专辑/时长/封面）
        let (title, artist, album, duration_ms, cover, cover_source) = read_audio_metadata(&path);

        // 以绝对路径作为唯一 id，确保不同位置的同名文件可区分
        let id = match std::fs::canonicalize(&path) {
            Ok(canon) => canon.to_string_lossy().to_string(),
            Err(_) => raw_path.clone(),
        };

        results.push(LocalSongInfo {
            id,
            name,
            path: raw_path,
            size,
            extension,
            title,
            artist,
            album,
            duration_ms,
            cover,
            cover_source,
        });
    }

    // 按名称排序，方便列表展示稳定
    results.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    Ok(results)
}

/// 递归收集文件夹下所有受支持的音频文件路径
fn collect_audio_files(dir: &std::path::Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_audio_files(&path, out);
        } else if path.is_file() {
            let ext = path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.to_lowercase())
                .unwrap_or_default();
            if is_supported_extension(&ext) {
                out.push(path.to_string_lossy().to_string());
            }
        }
    }
}

/// 导入整个文件夹（递归）下的音频文件：收集文件后复用单文件导入逻辑
#[tauri::command]
pub async fn import_local_music_folder(folder: String) -> Result<Vec<LocalSongInfo>, String> {
    let mut paths = Vec::new();
    collect_audio_files(std::path::Path::new(&folder), &mut paths);
    import_local_music(paths).await
}

/// 读取本地音频文件同目录的同名 .lrc 歌词文件。
/// 优先级：同名 .lrc → 同名 .txt（文本歌词）。
/// 返回原始歌词文本，若不存在返回空字符串。
#[tauri::command]
pub async fn get_local_lyric(path: String) -> Result<String, String> {
    let audio_path = std::path::Path::new(&path);
    let dir = audio_path.parent().ok_or_else(|| "无法解析路径".to_string())?;
    let stem = audio_path
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or_default();

    // 尝试 .lrc 和 .txt
    for ext in ["lrc", "txt"] {
        let candidate = dir.join(format!("{stem}.{ext}"));
        if candidate.is_file() {
            match std::fs::read_to_string(&candidate) {
                Ok(text) if !text.trim().is_empty() => return Ok(text),
                _ => continue,
            }
        }
    }

    Ok(String::new())
}
