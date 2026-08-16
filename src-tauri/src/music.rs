use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::Accessor;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::Emitter;

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

    // lofty 对某些损坏/特殊格式音频（APE/WMA/截断 MP3 等）可能 panic，
    // 在并行导入线程中 panic 会直接崩掉整个进程，这里用 catch_unwind 兜住。
    let parsed = std::panic::catch_unwind(|| lofty::read_from_path(path))
        .ok()
        .and_then(|r| r.ok());

    if let Some(tagged_file) = parsed {
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

/// 导入本地音频文件，返回可直接加入播放列表的歌曲元信息。
/// 不复制文件，仅校验扩展名与存在性，并读取文件大小。
///
/// 参考 any-listen 的分批策略：每批最多 100 首串行解析（批内并行、批间顺序），
/// 内存峰值恒定在单批量级，杜绝大文件夹（如 2000 首）一次性全量解析导致的内存
/// 飙升/线程风暴。坏文件通过 catch_unwind 兜住，不崩进程。
#[tauri::command]
pub async fn import_local_music(paths: Vec<String>) -> Result<Vec<LocalSongInfo>, String> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    // 单次导入上限：防止超大文件夹一次性导入导致内存/线程压力过大
    const MAX_IMPORT: usize = 2000;
    // 单批数量：借鉴 any-listen（每批 100 首），内存峰值恒定
    const BATCH_SIZE: usize = 100;
    let paths: Vec<String> = paths.into_iter().take(MAX_IMPORT).collect();
    let total = paths.len();

    let mut all_results: Vec<LocalSongInfo> = Vec::with_capacity(total);
    // 同目录封面缓存跨批次共享，避免重复扫描同一目录的通用封面
    let folder_cover_cache: Arc<Mutex<HashMap<std::path::PathBuf, String>>> =
        Arc::new(Mutex::new(HashMap::new()));

    for batch in paths.chunks(BATCH_SIZE) {
        let results: Arc<Mutex<Vec<LocalSongInfo>>> = Arc::new(Mutex::new(Vec::with_capacity(batch.len())));
        // 并行 worker 数量：按 CPU 核数控制，避免过度创建线程
        let workers = std::thread::available_parallelism()
            .map(|n| n.get().clamp(2, 8))
            .unwrap_or(4);

        std::thread::scope(|scope| {
            let mut handles = Vec::new();
            let chunk_size = (batch.len() + workers - 1) / workers;
            for chunk in batch.chunks(chunk_size) {
                let chunk = chunk.to_vec();
                let results = Arc::clone(&results);
                let folder_cover_cache = Arc::clone(&folder_cover_cache);
                handles.push(scope.spawn(move || {
                    // 整个 worker 体 catch_unwind：任何意外 panic 都只丢弃该分片，不崩进程
                    let _ = std::panic::catch_unwind(|| {
                        let mut local_results = Vec::with_capacity(chunk.len());
                        let mut local_cache = HashMap::new();
                        for raw_path in chunk {
                            if let Some(info) = build_local_song_info(&raw_path, &mut local_cache) {
                                local_results.push(info);
                            }
                        }
                        // 合并该 worker 的同目录封面缓存
                        {
                            if let Ok(mut cache) = folder_cover_cache.lock() {
                                for (dir, cover) in local_cache {
                                    if !cover.is_empty() {
                                        cache.entry(dir).or_insert(cover);
                                    }
                                }
                            }
                        }
                        if let Ok(mut res) = results.lock() {
                            res.extend(local_results);
                        }
                    });
                }));
            }
            for handle in handles {
                let _ = handle.join();
            }
        });

        // 取出本批结果并累积
        if let Ok(mut res) = Arc::try_unwrap(results).map(|m| m.into_inner().unwrap_or_default()) {
            all_results.append(&mut res);
        }
        log::info!("[Music] 本地导入批次完成，累计 {}/{}", all_results.len(), total);
    }

    // 按名称排序，方便列表展示稳定
    all_results.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    Ok(all_results)
}

/// 迭代式收集文件夹下所有受支持的音频文件路径（非递归，避免深目录栈溢出；
/// 跳过隐藏目录/回收站等系统目录，限制单次导入数量防止超大目录内存爆掉）。
fn collect_audio_files(dir: &std::path::Path, out: &mut Vec<String>) {
    const MAX_FILES: usize = 2000;
    let mut stack = vec![dir.to_path_buf()];

    while let Some(current) = stack.pop() {
        // 达到上限直接停止收集（避免 E:\CloudMusic 这类几千首的大目录卡死）
        if out.len() >= MAX_FILES {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy().to_lowercase();
            // 跳过隐藏目录 / 回收站 / 系统目录
            if name.starts_with(".") || name == "$recycle.bin" || name == "system volume information" {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else if path.is_file() {
                let ext = path
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(|value| value.to_lowercase())
                    .unwrap_or_default();
                if is_supported_extension(&ext) {
                    out.push(path.to_string_lossy().to_string());
                    if out.len() >= MAX_FILES {
                        break;
                    }
                }
            }
        }
    }
}

/// 导入整个文件夹（递归）下的音频文件。
///
/// 关键：不能把全部歌曲（每首带 base64 封面，可能几十 MB）一次性通过 invoke 返回——
/// WebView2 IPC 传不动这么大 payload 会卡死/崩溃。改为分批解析 + local-import-batch
/// 事件增量推送（每批 100 首），前端监听事件边收边显示，进度实时可见。
#[tauri::command]
pub async fn import_local_music_folder(
    folder: String,
    window: tauri::Window,
) -> Result<usize, String> {
    const BATCH_SIZE: usize = 100;
    let mut paths = Vec::new();
    collect_audio_files(std::path::Path::new(&folder), &mut paths);
    if paths.is_empty() {
        return Ok(0);
    }
    let total = paths.len();

    let folder_cover_cache: Arc<Mutex<HashMap<std::path::PathBuf, String>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let mut done = 0usize;
    for batch in paths.chunks(BATCH_SIZE) {
        let infos = parse_local_batch(batch, &folder_cover_cache);
        done += infos.len();
        let _ = window.emit("local-import-batch", &infos);
        log::info!("[Music] 本地导入批次推送 {done}/{total}");
    }
    Ok(total)
}

/// 解析一批本地音频文件（批内并行，共享封面缓存）
fn parse_local_batch(
    batch: &[String],
    folder_cover_cache: &Arc<Mutex<HashMap<std::path::PathBuf, String>>>,
) -> Vec<LocalSongInfo> {
    let results: Arc<Mutex<Vec<LocalSongInfo>>> = Arc::new(Mutex::new(Vec::with_capacity(batch.len())));
    let workers = std::thread::available_parallelism()
        .map(|n| n.get().clamp(2, 8))
        .unwrap_or(4);

    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        let chunk_size = (batch.len() + workers - 1) / workers;
        for chunk in batch.chunks(chunk_size) {
            let chunk = chunk.to_vec();
            let results = Arc::clone(&results);
            let folder_cover_cache = Arc::clone(folder_cover_cache);
            handles.push(scope.spawn(move || {
                let _ = std::panic::catch_unwind(|| {
                    let mut local_results = Vec::with_capacity(chunk.len());
                    let mut local_cache = HashMap::new();
                    for raw_path in chunk {
                        if let Some(info) = build_local_song_info(&raw_path, &mut local_cache) {
                            local_results.push(info);
                        }
                    }
                    {
                        if let Ok(mut cache) = folder_cover_cache.lock() {
                            for (dir, cover) in local_cache {
                                if !cover.is_empty() {
                                    cache.entry(dir).or_insert(cover);
                                }
                            }
                        }
                    }
                    if let Ok(mut res) = results.lock() {
                        res.extend(local_results);
                    }
                });
            }));
        }
        for handle in handles {
            let _ = handle.join();
        }
    });

    Arc::try_unwrap(results)
        .map(|m| m.into_inner().unwrap_or_default())
        .unwrap_or_default()
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

/// 按文件路径读取本地音频的内嵌封面（base64 data URI）。
/// 用于歌曲列表按需加载封面，避免大量封面一次性写入 store 导致崩溃。
#[tauri::command]
pub fn get_local_song_cover(path: String) -> Result<String, String> {
    let audio_path = std::path::Path::new(&path);
    if !audio_path.is_file() {
        return Ok(String::new());
    }
    let mut cache = HashMap::new();
    let (_, _, _, _, cover, _) = read_audio_metadata(audio_path, &mut cache);
    Ok(cover)
}

/// 获取远程图片并转为 base64 data URI（用于 SMTC 封面等跨域场景）。
/// 网易云等图床有防盗链，直接前端 fetch 会被 CORS 拦截，这里走后端 reqwest 下载。
#[tauri::command]
pub async fn fetch_remote_image(url: String) -> Result<String, String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("非法的 URL".to_string());
    }
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36 NexBox")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("下载失败: HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if bytes.is_empty() || bytes.len() > 5 * 1024 * 1024 {
        return Err("图片过大或为空".to_string());
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:image/jpeg;base64,{}", b64))
}
