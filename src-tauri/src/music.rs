use image::{GenericImageView, ImageEncoder};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::Accessor;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

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
    /// 封面缓存文件绝对路径（可能为空）
    pub cover_path: String,
    /// 封面来源："embedded" 内嵌 / "folder" 同目录图片 / "none" 无
    pub cover_source: String,
}

/// 封面缓存目录：应用缓存目录下的 covers 子目录
fn cover_cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("无法获取应用缓存目录: {e}"))?
        .join("covers");
    std::fs::create_dir_all(&dir).map_err(|e| format!("无法创建封面缓存目录: {e}"))?;
    Ok(dir)
}

/// 稳定的 FNV-1a 64 位哈希：保证同一输入在多次进程运行中得到相同结果。
/// 不能用 DefaultHasher（随机种子导致跨会话 hash 变化、重启后缓存路径失效）。
fn stable_hash(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// 把压缩后的封面数据写入缓存目录，返回文件绝对路径。
/// 文件已存在时跳过写入直接返回路径，支持重复导入去重。
fn save_cover_file(cache_dir: &Path, id: &str, data: &[u8], mime: &str) -> String {
    if data.is_empty() {
        return String::new();
    }
    let ext = if mime == "image/png" { "png" } else { "jpg" };
    let name = format!("{:016x}.{}", stable_hash(id), ext);
    let file_path = cache_dir.join(&name);
    if !file_path.exists() {
        let _ = std::fs::write(&file_path, data);
    }
    file_path.to_string_lossy().to_string()
}

/// 常见的同目录专辑封面文件名（小写）
const FOLDER_COVER_NAMES: [&str; 8] = [
    "cover.jpg", "folder.jpg", "album.jpg", "front.jpg",
    "cover.png", "folder.png", "album.png", "front.png",
];

/// 在某个目录下查找同目录封面图片，写入封面缓存并返回缓存文件路径。
/// 优先固定封面名（cover/folder/album/front），其次返回空字符串表示无同目录通用封面。
/// 缓存 key 使用目录路径，同一目录的歌曲共享同一张通用封面缓存文件。
fn find_folder_generic_cover(dir: &Path, cache_dir: &Path) -> String {
    let key = dir.to_string_lossy().to_string();
    for name in FOLDER_COVER_NAMES {
        let candidate = dir.join(name);
        if candidate.is_file() {
            let cover = save_cover_from_file(cache_dir, &key, &candidate);
            if !cover.is_empty() {
                return cover;
            }
        }
    }
    String::new()
}

/// 读取单个音频文件的内嵌元数据（标题/艺术家/专辑/时长/封面）
/// 返回 (title, artist, album, duration_ms, cover_path, cover_source)
///
/// `folder_cover_cache`：同目录通用封面缓存（目录 → 封面缓存文件路径），避免对同一目录下
/// 的每首歌重复做磁盘探测与写入，大幅减少大文件夹导入时的重复 I/O。
/// `cache_dir`：封面缓存目录；为 None 时降级为不提取封面（不阻塞导入主流程）。
fn read_audio_metadata(
    path: &Path,
    folder_cover_cache: &mut HashMap<PathBuf, String>,
    cache_dir: Option<&Path>,
    id: &str,
) -> (String, String, String, u64, String, String) {
    let (mut title, mut artist, mut album) = (String::new(), String::new(), String::new());
    let mut duration_ms: u64 = 0;
    let mut cover_path = String::new();
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

        // 封面：遍历所有标签中的所有内嵌图片，取第一张压缩后写入封面缓存
        for t in tagged_file.tags() {
            for pic in t.pictures() {
                let data = pic.data();
                // 限制封面大小（原始数据 ≤ 1MB），避免超大封面拖慢解析与缓存；
                // 超过阈值的大封面会先压缩再写入
                if !data.is_empty() && data.len() <= 1024 * 1024 {
                    let mime = detect_image_mime(data);
                    let (data, mime) = downscale_cover(data, mime);
                    if !data.is_empty() {
                        if let Some(cache_dir) = cache_dir {
                            cover_path = save_cover_file(cache_dir, id, &data, &mime);
                        }
                        if !cover_path.is_empty() {
                            cover_source = "embedded".to_string();
                        }
                        break;
                    }
                }
            }
            if !cover_path.is_empty() {
                break;
            }
        }
    }

    // 兜底：音频没有内嵌封面时，尝试读取同目录的封面图片文件
    // 顺序：固定封面名（cover/folder/album/front）→ 与音频同名的图片
    if cover_path.is_empty() {
        if let (Some(dir), Some(cache_dir)) = (path.parent(), cache_dir) {
            // 1. 通用固定封面：先查缓存，未命中才探测并写入缓存
            if !folder_cover_cache.contains_key(dir) {
                let cached = find_folder_generic_cover(dir, cache_dir);
                folder_cover_cache.insert(dir.to_path_buf(), cached);
            }
            let generic = folder_cover_cache.get(dir).cloned().unwrap_or_default();
            if !generic.is_empty() {
                cover_path = generic;
                cover_source = "folder".to_string();
            } else {
                // 2. 与音频同名的图片（每首歌不同，用完整路径做缓存 key）
                if let Some(stem) = path.file_stem().and_then(|v| v.to_str()) {
                    for ext in ["jpg", "png", "jpeg", "webp", "bmp"] {
                        let candidate = dir.join(format!("{stem}.{ext}"));
                        if candidate.is_file() {
                            cover_path = save_cover_from_file(cache_dir, &candidate.to_string_lossy(), &candidate);
                            if !cover_path.is_empty() {
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
        "local music metadata: title={title:?} artist={artist:?} album={album:?} dur={duration_ms}ms cover_source={cover_source} cover_path={cover_path:?}",
    );

    (title, artist, album, duration_ms, cover_path, cover_source)
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
const COVER_MAX_PX: u32 = 256;
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
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut w, 80)
            .write_image(resized.as_bytes(), resized.width(), resized.height(), resized.color().into())
            .map_err(|_| ())
            .map(|_| "image/jpeg")
    };
    match out_mime {
        Ok(m) if !out.is_empty() => (out, m.to_string()),
        _ => (data.to_vec(), mime.to_string()),
    }
}

/// 将图片文件压缩后写入封面缓存，返回缓存文件绝对路径。
/// 用 `cache_key` 生成缓存文件名（同一目录通用封面用目录路径，同名封面用图片完整路径）。
fn save_cover_from_file(cache_dir: &Path, cache_key: &str, path: &Path) -> String {
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
                save_cover_file(cache_dir, cache_key, &data, &mime)
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
    folder_cover_cache: &mut HashMap<PathBuf, String>,
    cache_dir: Option<&Path>,
) -> Option<LocalSongInfo> {
    let path = Path::new(raw_path);
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

    // 以绝对路径作为唯一 id，确保不同位置的同名文件可区分
    let id = match std::fs::canonicalize(path) {
        Ok(canon) => canon.to_string_lossy().to_string(),
        Err(_) => raw_path.to_string(),
    };

    // 读取内嵌元数据（标题/艺术家/专辑/时长/封面）
    let (title, artist, album, duration_ms, cover_path, cover_source) =
        read_audio_metadata(path, folder_cover_cache, cache_dir, &id);

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
        cover_path,
        cover_source,
    })
}

/// 导入本地音频文件，返回所有歌曲的元信息数组。
/// 不复制文件，仅校验扩展名与存在性、读取元数据，并把封面压缩写入应用缓存目录。
///
/// 采用多线程并行解析 + mpsc 通道收集结果；封面只返回缓存文件路径（小字符串），
/// 避免超大 base64 payload 经 IPC 传输导致前端卡顿。
#[tauri::command]
pub async fn import_local_music(app: AppHandle, paths: Vec<String>) -> Result<Vec<LocalSongInfo>, String> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }

    // 封面缓存目录；获取失败时降级为不提取封面（不阻塞导入主流程）
    let cache_dir = cover_cache_dir(&app).ok();

    // 共享的同目录封面缓存：目录 → 封面缓存文件路径
    let folder_cover_cache: Arc<Mutex<HashMap<PathBuf, String>>> =
        Arc::new(Mutex::new(HashMap::new()));
    // 并行 worker 数量：使用 CPU 最大可用线程（含超线程），尽量缩短导入耗时
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let total = paths.len();
    // 进度计数（仅用于日志，避免高频输出）
    let done = std::sync::atomic::AtomicUsize::new(0);

    let infos: Vec<LocalSongInfo> = std::thread::scope(|scope| {
        let (tx, rx) = std::sync::mpsc::channel::<LocalSongInfo>();
        // 将路径分片分配给各 worker，worker 解析后通过通道发送
        let chunk_size = (total + workers - 1) / workers;
        for chunk in paths.chunks(chunk_size) {
            let chunk = chunk.to_vec();
            let tx = tx.clone();
            let folder_cover_cache = Arc::clone(&folder_cover_cache);
            let cache_dir = cache_dir.as_deref();
            let done = &done;
            scope.spawn(move || {
                let mut local_cache = HashMap::new();
                for raw_path in chunk {
                    if let Some(info) = build_local_song_info(&raw_path, &mut local_cache, cache_dir) {
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

        // 命令线程：收集全部解析结果
        let mut result: Vec<LocalSongInfo> = Vec::with_capacity(total);
        for info in rx {
            result.push(info);
        }
        result
    });

    Ok(infos)
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
pub async fn import_local_music_folder(app: AppHandle, folder: String) -> Result<Vec<LocalSongInfo>, String> {
    let mut paths = Vec::new();
    collect_audio_files(Path::new(&folder), &mut paths);
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
