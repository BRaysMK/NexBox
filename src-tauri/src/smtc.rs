//! Windows 任务栏媒体控件（SMTC）注册
//!
//! 背景：音乐播放已从 WebView2(HTMLAudioElement) 移到 Rust rodio 引擎，
//! 前端 Media Session API 不再能注册系统媒体控件。
//! 这里用 ISystemMediaTransportControlsInterop::GetForWindow 绑定一个
//! **进程级持久隐藏宿主窗口**（不再绑定主窗口），注册我们自己的 SMTC 会话：
//! 显示歌名/歌手/专辑/封面，支持播放暂停/上下曲/进度。
//!
//! 为什么用隐藏宿主窗口而不是主窗口：
//! 主窗口在“最小化到托盘”时会被 destroy 以释放全部 WebView2 进程，
//! GetForWindow 的会话生命周期跟随传入的 HWND，主窗口一销毁 SMTC 就失效，
//! 导致最小化期间任务栏媒体控件完全不可用。改为常驻隐藏窗口后，
//! 会话与主窗口生命周期解耦，最小化/托盘期间 SMTC 依然可用。
//!
//! 注意：本模块使用 windows 0.62 crate（Cargo.toml 中别名 windows62），
//! 与 tauri 依赖的 windows 0.58 共存，避免升级冲突。
//!
//! 按钮事件：通过 Tauri 事件 'smtc-control' 转发给前端 store（与 music-hotkey 一致）；
//! 主窗口销毁后由 player::register_control_listener 在 Rust 侧接管。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use tauri::Emitter;

use windows62::Win32::Foundation::HWND as WinRtHwnd;
use windows62::Media::{
    MediaPlaybackStatus,
    SystemMediaTransportControls,
    SystemMediaTransportControlsButton,
    SystemMediaTransportControlsButtonPressedEventArgs,
    SystemMediaTransportControlsTimelineProperties,
};
use windows62::Win32::System::WinRT::{ISystemMediaTransportControlsInterop, RoGetActivationFactory};

static SMTC: OnceLock<Mutex<Option<SystemMediaTransportControls>>> = OnceLock::new();
static SMTC_INITIALIZED: AtomicBool = AtomicBool::new(false);

fn smtc_slot() -> &'static Mutex<Option<SystemMediaTransportControls>> {
    SMTC.get_or_init(|| Mutex::new(None))
}

/// 进程级持久隐藏宿主窗口的 HWND（isize 以便跨平台编译；仅 Windows 上有效）。
/// 窗口只创建一次，永不销毁，SMTC 会话绑定到它上面。
static HOST_HWND: OnceLock<isize> = OnceLock::new();

#[cfg(target_os = "windows")]
fn host_hwnd() -> Option<isize> {
    use windows_sys::core::w;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, WS_EX_TOOLWINDOW, WS_POPUP};

    Some(*HOST_HWND.get_or_init(|| unsafe {
        let hinstance = GetModuleHandleW(std::ptr::null());
        // 隐藏顶层窗口：STATIC 类无需注册，永不显示、不进任务栏/Alt-Tab。
        // 用它承载 SMTC 会话，避免会话随主窗口销毁而失效。
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            w!("STATIC"),
            w!("NexBoxSmtcHost"),
            WS_POPUP,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null(),
        );
        hwnd as isize
    }))
}

#[cfg(not(target_os = "windows"))]
fn host_hwnd() -> Option<isize> {
    None
}

/// 从隐藏宿主窗口获取 SystemMediaTransportControls 实例并启用控制。
/// 幂等：进程内只注册一次，主窗口销毁/重建不影响会话。
pub fn init<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    #[cfg(target_os = "windows")]
    {
        use windows62::Foundation::TypedEventHandler;

        if SMTC_INITIALIZED.swap(true, Ordering::SeqCst) {
            return;
        }
        let Some(raw) = host_hwnd() else {
            log::warn!("[smtc] host window creation failed, skip");
            SMTC_INITIALIZED.store(false, Ordering::SeqCst);
            return;
        };
        let raw_ptr: *mut core::ffi::c_void = raw as *mut core::ffi::c_void;
        let hwnd62 = WinRtHwnd(raw_ptr);

        // 1. 获取 interop 接口（RoGetActivationFactory 按 IID 查询）
        let interop: ISystemMediaTransportControlsInterop = match unsafe {
            RoGetActivationFactory::<ISystemMediaTransportControlsInterop>(&windows62::core::HSTRING::from("Windows.Media.SystemMediaTransportControls"))
        } {
            Ok(i) => i,
            Err(e) => {
                log::warn!("[smtc] get interop failed: {e}");
                SMTC_INITIALIZED.store(false, Ordering::SeqCst);
                return;
            }
        };

        // 2. GetForWindow 绑定隐藏宿主窗口（常驻，不随主窗口销毁）
        let controls: SystemMediaTransportControls = match unsafe { interop.GetForWindow(hwnd62) } {
            Ok(c) => c,
            Err(e) => {
                log::warn!("[smtc] GetForWindow failed: {e}");
                SMTC_INITIALIZED.store(false, Ordering::SeqCst);
                return;
            }
        };

        // 3. 启用控制按钮
        let _ = controls.SetIsEnabled(true);
        let _ = controls.SetIsPlayEnabled(true);
        let _ = controls.SetIsPauseEnabled(true);
        let _ = controls.SetIsNextEnabled(true);
        let _ = controls.SetIsPreviousEnabled(true);
        let _ = controls.SetIsFastForwardEnabled(false);
        let _ = controls.SetIsRewindEnabled(false);

        // 4. 订阅按钮事件（TypedEventHandler）
        let app2 = app.clone();
        let handler = TypedEventHandler::<SystemMediaTransportControls, SystemMediaTransportControlsButtonPressedEventArgs>::new(
            move |_sender: windows62::core::Ref<SystemMediaTransportControls>, args: windows62::core::Ref<SystemMediaTransportControlsButtonPressedEventArgs>| {
                if let Some(args) = args.as_ref() {
                    let btn = args.Button().unwrap_or(SystemMediaTransportControlsButton::Play);
                    let action = if btn == SystemMediaTransportControlsButton::Play {
                        "play"
                    } else if btn == SystemMediaTransportControlsButton::Pause {
                        "pause"
                    } else if btn == SystemMediaTransportControlsButton::Next {
                        "next"
                    } else if btn == SystemMediaTransportControlsButton::Previous {
                        "prev"
                    } else {
                        ""
                    };
                    if !action.is_empty() {
                        let _ = app2.emit("smtc-control", serde_json::json!({ "action": action }));
                    }
                }
                Ok(())
            }
        );
        let _ = controls.ButtonPressed(&handler);

        if let Ok(mut slot) = smtc_slot().lock() {
            *slot = Some(controls);
        }
        log::info!("[smtc] registered on persistent host window");
    }
}

/// Tauri 命令：前端播放新歌时更新 SMTC 元数据（标题/歌手/专辑/封面）
/// cover 支持三种来源：data URI（本地内嵌封面）、http(s) URL（在线封面，后端下载绕过防盗链）、本地文件路径。
#[tauri::command]
pub async fn smtc_update_metadata(
    title: String,
    artist: String,
    album: String,
    cover: Option<String>,
) -> Result<(), String> {
    update_metadata(&title, &artist, &album);
    if let Some(cover_src) = cover {
        let cover_src = cover_src.trim().to_string();
        if !cover_src.is_empty() {
            #[cfg(target_os = "windows")]
            {
                // 异步设置缩略图，避免封面下载阻塞命令返回
                tauri::async_runtime::spawn(async move {
                    update_thumbnail(&cover_src).await;
                });
            }
        }
    }
    Ok(())
}

/// 引擎侧切歌时更新 SMTC 封面（主窗口销毁后引擎自动切歌/被控制切歌时调用）。
/// 与 smtc_update_metadata 的封面逻辑一致，仅更新缩略图不重复推送标题等文本。
pub fn update_cover(cover: String) {
    let cover = cover.trim().to_string();
    if cover.is_empty() {
        return;
    }
    #[cfg(target_os = "windows")]
    {
        tauri::async_runtime::spawn(async move {
            update_thumbnail(&cover).await;
        });
    }
    #[cfg(not(target_os = "windows"))]
    let _ = cover;
}

/// 更新 SMTC 播放状态
pub fn update_playback_state(is_playing: bool) {
    #[cfg(target_os = "windows")]
    {
        if let Ok(slot) = smtc_slot().lock() {
            if let Some(c) = slot.as_ref() {
                let _ = c.SetPlaybackStatus(if is_playing { MediaPlaybackStatus::Playing } else { MediaPlaybackStatus::Paused });
            }
        }
    }
}

/// 更新 SMTC 元数据（标题/歌手/专辑）
pub fn update_metadata(title: &str, artist: &str, album: &str) {
    #[cfg(target_os = "windows")]
    {
        use windows62::Media::MediaPlaybackType;
        if let Ok(slot) = smtc_slot().lock() {
            if let Some(c) = slot.as_ref() {
                if let Ok(updater) = c.DisplayUpdater() {
                    let _ = updater.SetType(MediaPlaybackType::Music);
                    if let Ok(props) = updater.MusicProperties() {
                        let _ = props.SetTitle(&windows62::core::HSTRING::from(title));
                        let _ = props.SetArtist(&windows62::core::HSTRING::from(artist));
                        let _ = props.SetAlbumTitle(&windows62::core::HSTRING::from(album));
                    }
                    let _ = updater.Update();
                }
            }
        }
    }
}

/// 更新 SMTC 进度（位置/时长，秒）
pub fn update_timeline(position_secs: f64, duration_secs: f64) {
    #[cfg(target_os = "windows")]
    {
        use windows62::Foundation::TimeSpan;
        if let Ok(slot) = smtc_slot().lock() {
            if let Some(c) = slot.as_ref() {
                if let Ok(props) = SystemMediaTransportControlsTimelineProperties::new() {
                    let pos = TimeSpan { Duration: (position_secs * 10_000_000.0) as i64 };
                    let end = TimeSpan { Duration: (duration_secs * 10_000_000.0) as i64 };
                    let _ = props.SetPosition(pos);
                    let _ = props.SetEndTime(end);
                    let _ = props.SetMinSeekTime(TimeSpan { Duration: 0 });
                    let _ = props.SetMaxSeekTime(end);
                    let _ = c.UpdateTimelineProperties(&props);
                }
            }
        }
    }
}

/// 更新 SMTC 封面缩略图。
/// 流程：解析封面来源 → 得到字节 → 写入临时文件 → StorageFile → RandomAccessStreamReference → SetThumbnail。
#[cfg(target_os = "windows")]
async fn update_thumbnail(cover_src: &str) {
    use windows62::Storage::StorageFile;
    use windows62::Storage::Streams::RandomAccessStreamReference;

    let Some(bytes) = resolve_cover_bytes(cover_src).await else {
        return;
    };
    if bytes.is_empty() || bytes.len() > 4 * 1024 * 1024 {
        return;
    }

    let dir = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("NexBox");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("smtc-cover.jpg");
    if std::fs::write(&path, &bytes).is_err() {
        return;
    }

    let hstring = windows62::core::HSTRING::from(path.to_string_lossy().as_ref());
    let file = match StorageFile::GetFileFromPathAsync(&hstring) {
        Ok(op) => match op.await {
            Ok(f) => f,
            Err(e) => {
                log::warn!("[smtc] GetFileFromPathAsync failed: {e}");
                return;
            }
        },
        Err(e) => {
            log::warn!("[smtc] GetFileFromPathAsync call failed: {e}");
            return;
        }
    };
    let stream_ref = match RandomAccessStreamReference::CreateFromFile(&file) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("[smtc] CreateFromFile failed: {e}");
            return;
        }
    };

    if let Ok(slot) = smtc_slot().lock() {
        if let Some(c) = slot.as_ref() {
            if let Ok(updater) = c.DisplayUpdater() {
                let _ = updater.SetThumbnail(&stream_ref);
                let _ = updater.Update();
                log::info!("[smtc] thumbnail updated ({} bytes)", bytes.len());
            }
        }
    }
}

/// 解析封面来源为图片字节：
/// - `data:image/...;base64,...` → 解码
/// - `http(s)://...` → 后端 reqwest 下载（带 UA/Referer 绕过防盗链）
/// - 其余 → 视为本地文件路径直接读取
#[cfg(target_os = "windows")]
async fn resolve_cover_bytes(src: &str) -> Option<Vec<u8>> {
    if let Some(rest) = src.strip_prefix("data:") {
        let comma = rest.find(',')?;
        let meta = &rest[..comma];
        let data = &rest[comma + 1..];
        if meta.contains(";base64") {
            use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
            return BASE64.decode(data).ok();
        }
        return None;
    }

    if src.starts_with("http://") || src.starts_with("https://") {
        let client = reqwest::Client::builder()
            .user_agent(crate::player::UA)
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .ok()?;
        let resp = client
            .get(src)
            .header("Referer", crate::player::referer_for(src))
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        return resp.bytes().await.ok().map(|b| b.to_vec());
    }

    std::fs::read(src).ok()
}
