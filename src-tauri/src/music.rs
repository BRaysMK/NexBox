use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use image::{GenericImageView, ImageEncoder};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::Accessor;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

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

/// 在某个目录下查找同目录封面图片，返回 (data_uri, source)
/// 优先固定封面名（cover/folder/album/front），其次返回 None 表示无同目录通用封面。
/// 由于每个音频都可能带独立同名封面，这里只负责「通用固定封面」的探测。
fn find_folder_generic_cover(dir: &std::path::Path) -> String {
    for name in FOLDER_COVER_NAMES {
        let candidate = dir.join(name);
        if candidate.is_file() {
            let cover = file_to_data_uri(&candidate);
            if !cover.is_empty() {
                return cover;
            }
        }
    }
    String::new()
}

/// 读取单个音频文件的内嵌元数据（标题/艺术家/专辑/时长/封面）
/// 返回 (title, artist, album, duration_ms, cover_data_uri, cover_source)
///
/// `folder_cover_cache`：同目录通用封面缓存（目录 → 封面 data_uri），避免对同一目录下
/// 的每首歌重复做磁盘探测与 base64 编码，大幅减少大文件夹导入时的重复 I/O。
fn read_audio_metadata(
    path: &std::path::Path,
    folder_cover_cache: &mut HashMap<std::path::PathBuf, String>,
) -> (String, String, String, u64, String, String) {
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
                // 限制封面大小（原始数据 ≤ 1MB），避免超大封面拖慢存储与渲染；
                // 超过阈值的大封面会先压缩再编码
                if !data.is_empty() && data.len() <= 1024 * 1024 {
                    let mime = detect_image_mime(data);
                    let (data, mime) = downscale_cover(data, mime);
                    if !data.is_empty() {
                        cover = format!("data:{mime};base64,{}", BASE64.encode(data));
                        cover_source = "embedded".to_string();
                        break;
                    }
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
            // 1. 通用固定封面：先查缓存，未命中才探测并写入缓存
            if !folder_cover_cache.contains_key(dir) {
                let cached = find_folder_generic_cover(dir);
                folder_cover_cache.insert(dir.to_path_buf(), cached);
            }
            let generic = folder_cover_cache.get(dir).cloned().unwrap_or_default();
            if !generic.is_empty() {
                cover = generic;
                cover_source = "folder".to_string();
            } else {
                // 2. 与音频同名的图片（每首歌不同，无需缓存）
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

/// 压缩封面：最大边 COVER_MAX_PX，输出 JPEG（PNG 源输出 PNG 保留透明）。
/// 数据 ≤ COVER_MIN_BYTES、尺寸已足够小、或解码/编码失败时原样返回（仅大小阈值保护）。
/// 返回压缩后的 (数据, mime)。
const COVER_MAX_PX: u32 = 512;
const COVER_MIN_BYTES: usize = 64 * 1024;

fn downscale_cover(data: &[u8], mime: &str) -> (Vec<u8>, String) {
    if data.len() <= COVER_MIN_BYTES {
        return (data.to_vec(), mime.to_string());
    }
    let format = match mime {
        "image/png" => image::ImageFormat::Png,
        _ => image::ImageFormat::Jpeg,
    };
    let img = match image::load_from_memory_with_format(data, format) {
        Ok(img) => img,
        Err(_) => return (data.to_vec(), mime.to_string()),
    };
    let (w, h) = img.dimensions();
    let max = w.max(h);
    if max <= COVER_MAX_PX {
        return (data.to_vec(), mime.to_string());
    }
    let scale = COVER_MAX_PX as f32 / max as f32;
    let nw = ((w as f32) * scale).round().max(1.0) as u32;
    let nh = ((h as f32) * scale).round().max(1.0) as u32;
    let resized = img.resize(nw, nh, image::imageops::FilterType::Triangle);
    let mut out = Vec::new();
    let out_mime = if mime == "image/png" {
        let mut w = std::io::Cursor::new(&mut out);
        image::codecs::png::PngEncoder::new(&mut w)
            .write_image(resized.as_bytes(), resized.width(), resized.height(), resized.color().into())
            .map_err(|_| ())
            .map(|_| "image/png")
    } else {
        let mut w = std::io::Cursor::new(&mut out);
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut w, 85)
            .write_image(resized.as_bytes(), resized.width(), resized.height(), resized.color().into())
            .map_err(|_| ())
            .map(|_| "image/jpeg")
    };
    match out_mime {
        Ok(m) if !out.is_empty() => (out, m.to_string()),
        _ => (data.to_vec(), mime.to_string()),
    }
}

/// 将图片文件读取为压缩后的 base64 data URI
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
        Ok(data) if !data.is_empty() => {
            let (data, mime) = downscale_cover(&data, mime);
            if !data.is_empty() && data.len() <= 1024 * 1024 {
                format!("data:{mime};base64,{}", BASE64.encode(data))
            } else {
                String::new()
            }
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

/// 读取单个音频文件的元信息并组装成 LocalSongInfo（供并行导入使用）。
fn build_local_song_info(
    raw_path: &str,
    folder_cover_cache: &mut HashMap<std::path::PathBuf, String>,
) -> Option<LocalSongInfo> {
    let path = std::path::Path::new(raw_path);
    if !path.is_file() {
        return None;
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_lowercase())
        .unwrap_or_default();

    if !is_supported_extension(&extension) {
        return None;
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
    let (title, artist, album, duration_ms, cover, cover_source) =
        read_audio_metadata(path, folder_cover_cache);

    // 以绝对路径作为唯一 id，确保不同位置的同名文件可区分
    let id = match std::fs::canonicalize(path) {
        Ok(canon) => canon.to_string_lossy().to_string(),
        Err(_) => raw_path.to_string(),
    };

    Some(LocalSongInfo {
        id,
        name,
        path: raw_path.to_string(),
        size,
        extension,
        title,
        artist,
        album,
        duration_ms,
        cover,
        cover_source,
    })
}

/// 导入本地音频文件，通过 Tauri 事件 `local-music-import-chunk` 分批推送到前端。
/// 返回实际导入数量（小整数）。不复制文件，仅校验扩展名与存在性，并读取元数据。
///
/// 采用多线程并行解析，并通过 mpsc 通道 + 命令线程分批 emit 事件，
/// 避免一次性返回超大集合阻塞 WebView 主线程，同时提供实时进度。
#[tauri::command]
pub async fn import_local_music(app: AppHandle, paths: Vec<String>) -> Result<i32, String> {
    if paths.is_empty() {
        return Ok(0);
    }

    // 共享的同目录封面缓存：目录 → 封面 data URI
    let folder_cover_cache: Arc<Mutex<HashMap<std::path::PathBuf, String>>> =
        Arc::new(Mutex::new(HashMap::new()));
    // 并行 worker 数量：使用 CPU 最大可用线程（含超线程），尽量缩短导入耗时
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let total = paths.len();
    // 进度计数（仅用于日志，避免高频输出）
    let done = std::sync::atomic::AtomicUsize::new(0);
    // 每批推送条数
    const BATCH: usize = 20;

    let imported: i32 = std::thread::scope(|scope| {
        let (tx, rx) = std::sync::mpsc::channel::<LocalSongInfo>();
        // 将路径分片分配给各 worker，worker 解析后通过通道发送
        let chunk_size = (total + workers - 1) / workers;
        for chunk in paths.chunks(chunk_size) {
            let chunk = chunk.to_vec();
            let tx = tx.clone();
            let folder_cover_cache = Arc::clone(&folder_cover_cache);
            let done = &done;
            scope.spawn(move || {
                let mut local_cache = HashMap::new();
                for raw_path in chunk {
                    if let Some(info) = build_local_song_info(&raw_path, &mut local_cache) {
                        let _ = tx.send(info);
                    }
                    let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    if n % 50 == 0 || n == total {
                        log::info!("[Music] 本地导入进度 {n}/{total}");
                    }
                }
                // 合并该 worker 的同目录封面缓存
                {
                    let mut cache = folder_cover_cache.lock().unwrap();
                    for (dir, cover) in local_cache {
                        if !cover.is_empty() {
                            cache.entry(dir).or_insert(cover);
                        }
                    }
                }
            });
        }
        drop(tx); // 释放本线程的发送端，当所有 worker 结束后 rx 结束

        // 命令线程：接收解析结果并分批 emit 事件
        let mut batch: Vec<LocalSongInfo> = Vec::with_capacity(BATCH);
        let mut imported: i32 = 0;
        for info in rx {
            batch.push(info);
            imported += 1;
            if batch.len() >= BATCH || imported as usize == total {
                let payload = serde_json::json!({
                    "chunk": batch,
                    "imported": imported,
                    "total": total,
                    "done": imported as usize == total,
                });
                let _ = app.emit("local-music-import-chunk", payload);
                batch.clear();
            }
        }
        // 收尾：把不足一批的剩余部分（或全部无效的空批次）连同 done:true 一起推送
        if !batch.is_empty() {
            let payload = serde_json::json!({
                "chunk": batch,
                "imported": imported,
                "total": total,
                "done": true,
            });
            let _ = app.emit("local-music-import-chunk", payload);
        } else if imported as usize != total {
            let payload = serde_json::json!({
                "chunk": [],
                "imported": imported,
                "total": total,
                "done": true,
            });
            let _ = app.emit("local-music-import-chunk", payload);
        }
        imported
    });

    Ok(imported)
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
pub async fn import_local_music_folder(app: AppHandle, folder: String) -> Result<i32, String> {
    let mut paths = Vec::new();
    collect_audio_files(std::path::Path::new(&folder), &mut paths);
    import_local_music(app, paths).await
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
