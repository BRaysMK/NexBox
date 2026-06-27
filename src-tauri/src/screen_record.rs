use std::io::{BufWriter, Seek, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tauri::Emitter;

// ─── Data structures ───

#[derive(serde::Serialize, Clone)]
pub struct DisplayInfo {
    pub index: usize,
    pub name: String,
    pub device_name: String,
    pub is_primary: bool,
    pub width: i32,
    pub height: i32,
}

#[derive(serde::Serialize, Clone)]
pub struct WindowInfo {
    pub hwnd: u64,
    pub title: String,
    pub class_name: String,
    pub exe_name: String,
    pub visible: bool,
    pub width: i32,
    pub height: i32,
}

#[derive(serde::Deserialize, Clone)]
pub struct RecordingConfig {
    pub mode: String,
    pub display_index: usize,
    pub window_hwnd: u64,
    pub output_width: i32,
    pub output_height: i32,
    pub fps: u32,
    #[allow(dead_code)]
    pub format: String,
    pub quality: u32,
    pub output_path: String,
    pub capture_cursor: bool,
}

#[derive(serde::Serialize, Clone)]
pub struct RecordingState {
    pub is_recording: bool,
    pub is_paused: bool,
    pub duration_secs: f64,
    pub file_path: Option<String>,
    pub file_size: u64,
    pub current_fps: f64,
    pub frames_captured: u64,
    pub frames_dropped: u64,
}

// ─── Global recording state ───

struct ActiveRecording {
    config: RecordingConfig,
    start_time: Instant,
    pause_time: Option<Instant>,
    total_pause_duration: std::time::Duration,
    frames_captured: u64,
    frames_dropped: u64,
    is_running: Arc<AtomicBool>,
    is_paused: Arc<AtomicBool>,
    cancel_token: Arc<AtomicBool>,
}

static RECORDING: Mutex<Option<ActiveRecording>> = Mutex::new(None);

// ─── Display enumeration ───

#[cfg(target_os = "windows")]
fn get_monitor_model_name(device_name: &str) -> String {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::Graphics::Gdi::{EnumDisplayDevicesW, DISPLAY_DEVICEW};

    let wide_name: Vec<u16> = device_name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut dd: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
    dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;

    let result = unsafe { EnumDisplayDevicesW(wide_name.as_ptr(), 0, &mut dd, 0) };
    if result == 0 {
        return String::new();
    }

    let model_end = dd.DeviceString.iter().position(|&c| c == 0).unwrap_or(dd.DeviceString.len());
    let model_name = OsString::from_wide(&dd.DeviceString[..model_end])
        .to_string_lossy()
        .into_owned();

    if model_name.contains("Generic") || model_name.is_empty() {
        return String::new();
    }
    model_name.trim().to_string()
}

#[cfg(not(target_os = "windows"))]
fn get_monitor_model_name(_device_name: &str) -> String {
    String::new()
}

#[tauri::command]
pub fn enumerate_screen_record_displays() -> Vec<DisplayInfo> {
    enumerate_displays_inner()
}

#[cfg(target_os = "windows")]
fn enumerate_displays_inner() -> Vec<DisplayInfo> {
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
    };

    struct MonitorData {
        displays: Vec<DisplayInfo>,
    }

    unsafe extern "system" fn monitor_enum_proc(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut windows_sys::Win32::Foundation::RECT,
        lparam: isize,
    ) -> i32 {
        let data = &mut *(lparam as *mut MonitorData);
        let mut info: MONITORINFOEXW = std::mem::zeroed();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

        if GetMonitorInfoW(hmonitor, &mut info as *mut _ as *mut _) != 0 {
            let device_name = String::from_utf16_lossy(
                &info.szDevice[..info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len())],
            );
            let is_primary = (info.monitorInfo.dwFlags & 1) != 0;
            let width = info.monitorInfo.rcMonitor.right - info.monitorInfo.rcMonitor.left;
            let height = info.monitorInfo.rcMonitor.bottom - info.monitorInfo.rcMonitor.top;
            let index = data.displays.len();

            let monitor_model = get_monitor_model_name(&device_name);
            let name = if !monitor_model.is_empty() {
                format!("{} ({}x{})", monitor_model, width, height)
            } else {
                format!("{} ({}x{})", device_name, width, height)
            };

            data.displays.push(DisplayInfo {
                index,
                name,
                device_name: device_name.clone(),
                is_primary,
                width,
                height,
            });
        }
        1
    }

    let mut data = MonitorData { displays: Vec::new() };
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(monitor_enum_proc),
            &mut data as *mut _ as isize,
        );
    }
    data.displays
}

#[cfg(not(target_os = "windows"))]
fn enumerate_displays_inner() -> Vec<DisplayInfo> {
    Vec::new()
}

// ─── Window enumeration ───

#[tauri::command]
pub fn enumerate_screen_record_windows() -> Vec<WindowInfo> {
    enumerate_windows_inner()
}

#[cfg(target_os = "windows")]
fn enumerate_windows_inner() -> Vec<WindowInfo> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindowLongPtrW, GetWindowRect,
        GetWindowTextLengthW, GetWindowTextW, IsWindowVisible, IsIconic,
        GWL_EXSTYLE, WS_EX_TOOLWINDOW,
    };
    use windows_sys::Win32::System::Threading::{
        QueryFullProcessImageNameW, OpenProcess,
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_NAME_NATIVE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    struct WindowData {
        windows: Vec<WindowInfo>,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let data = &mut *(lparam as *mut WindowData);
        if IsWindowVisible(hwnd) == 0 { return 1; }
        if IsIconic(hwnd) != 0 { return 1; }

        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        if (ex_style & WS_EX_TOOLWINDOW) != 0 { return 1; }

        let title_len = GetWindowTextLengthW(hwnd);
        if title_len == 0 { return 1; }

        let mut title_buf: Vec<u16> = vec![0; (title_len + 1) as usize];
        GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_len + 1);
        let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);

        if title.is_empty() || title == "Program Manager" || title == "NexBox" {
            return 1;
        }

        let mut class_buf: [u16; 256] = [0; 256];
        let class_len = GetClassNameW(hwnd, class_buf.as_mut_ptr(), 256);
        let _class_name = if class_len > 0 {
            String::from_utf16_lossy(&class_buf[..class_len as usize])
        } else {
            String::new()
        };

        let mut rect: RECT = std::mem::zeroed();
        let width: i32;
        let height: i32;
        if GetWindowRect(hwnd, &mut rect) != 0 {
            width = rect.right - rect.left;
            height = rect.bottom - rect.top;
        } else {
            width = 0;
            height = 0;
        }

        if width < 100 || height < 100 { return 1; }

        let mut process_id: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut process_id);

        let mut exe_name = String::new();
        if process_id != 0 {
            let process_handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id);
            if !process_handle.is_null() {
                let mut exe_buf: [u16; 260] = [0; 260];
                let mut exe_len: u32 = 260;
                if QueryFullProcessImageNameW(
                    process_handle, PROCESS_NAME_NATIVE,
                    exe_buf.as_mut_ptr(), &mut exe_len,
                ) != 0 {
                    exe_name = OsString::from_wide(&exe_buf[..exe_len as usize])
                        .to_string_lossy()
                        .into_owned();
                }
                use windows_sys::Win32::Foundation::CloseHandle;
                CloseHandle(process_handle);
            }
        }

        data.windows.push(WindowInfo {
            hwnd: hwnd as u64,
            title,
            class_name: _class_name,
            exe_name,
            visible: true,
            width,
            height,
        });
        1
    }

    let mut data = WindowData { windows: Vec::new() };
    unsafe { EnumWindows(Some(enum_proc), &mut data as *mut _ as LPARAM); }

    data.windows.sort_by(|a, b| {
        let a_app = if a.exe_name.is_empty() { 0 } else { 1 };
        let b_app = if b.exe_name.is_empty() { 0 } else { 1 };
        b_app.cmp(&a_app).then_with(|| a.title.cmp(&b.title))
    });
    data.windows
}

#[cfg(not(target_os = "windows"))]
fn enumerate_windows_inner() -> Vec<WindowInfo> {
    Vec::new()
}

// ─── Frame capture (GDI) ───

#[cfg(target_os = "windows")]
unsafe fn capture_frame_gdi(
    display_index: usize,
    window_hwnd: u64,
    width: i32,
    height: i32,
    capture_cursor: bool,
) -> Result<Vec<u8>, String> {
    use std::ptr;
    use windows_sys::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateDCW,
        DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject,
        BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, SRCCOPY, BI_RGB,
        CAPTUREBLT, HDC,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;

    let src_dc: HDC;
    let mut use_delete_dc_for_src = false;
    let mut x: i32 = 0;
    let mut y: i32 = 0;
    let mut src_w = width;
    let mut src_h = height;

    let displays = enumerate_displays_inner();

    if window_hwnd != 0 {
        // Window capture mode
        let hwnd = window_hwnd as windows_sys::Win32::Foundation::HWND;
        let mut rect = std::mem::zeroed();
        if GetWindowRect(hwnd, &mut rect) != 0 {
            x = rect.left;
            y = rect.top;
            src_w = rect.right - rect.left;
            src_h = rect.bottom - rect.top;
            src_dc = GetDC(hwnd);
        } else {
            return Err("Failed to get window rect".to_string());
        }
    } else {
        // Full screen capture
        if display_index < displays.len() {
            let display = &displays[display_index];
            x = 0;
            y = 0;
            src_w = display.width;
            src_h = display.height;
            let device_name_wide: Vec<u16> = display.device_name
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            src_dc = CreateDCW(
                device_name_wide.as_ptr(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
            );
            use_delete_dc_for_src = true;
        } else {
            src_dc = GetDC(ptr::null_mut());
        }
    }

    if src_dc.is_null() {
        return Err("Failed to get source DC".to_string());
    }

    // Create compatible DC and bitmap
    let mem_dc = CreateCompatibleDC(ptr::null_mut());
    if mem_dc.is_null() {
        if !use_delete_dc_for_src {
            ReleaseDC(ptr::null_mut(), src_dc);
        } else {
            DeleteDC(src_dc);
        }
        return Err("Failed to create compatible DC".to_string());
    }

    let bitmap = CreateCompatibleBitmap(src_dc, src_w, src_h);
    if bitmap.is_null() {
        DeleteDC(mem_dc);
        if !use_delete_dc_for_src {
            ReleaseDC(ptr::null_mut(), src_dc);
        } else {
            DeleteDC(src_dc);
        }
        return Err("Failed to create compatible bitmap".to_string());
    }

    let old_bitmap = SelectObject(mem_dc, bitmap);

    let mut blt_flags = SRCCOPY;
    if capture_cursor {
        blt_flags |= CAPTUREBLT;
    }

    let _result = BitBlt(mem_dc, 0, 0, src_w, src_h, src_dc, x, y, blt_flags);

    // Get bitmap data (BGRA, bottom-up due to negative height)
    let data_size = (src_w * src_h * 4) as usize;
    let mut frame_data: Vec<u8> = vec![0u8; data_size];

    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: src_w,
            biHeight: -src_h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [std::mem::zeroed(); 1],
    };

    GetDIBits(
        mem_dc,
        bitmap,
        0,
        src_h as u32,
        frame_data.as_mut_ptr() as *mut _,
        &mut bmi,
        DIB_RGB_COLORS,
    );

    // Cleanup
    SelectObject(mem_dc, old_bitmap);
    DeleteObject(bitmap);
    DeleteDC(mem_dc);

    if !use_delete_dc_for_src {
        ReleaseDC(ptr::null_mut(), src_dc);
    } else {
        DeleteDC(src_dc);
    }

    // Resize if output dimensions differ from source
    if width != src_w || height != src_h {
        frame_data = resize_frame(&frame_data, src_w, src_h, width, height);
    }

    Ok(frame_data)
}

#[cfg(not(target_os = "windows"))]
unsafe fn capture_frame_gdi(
    _display_index: usize,
    _window_hwnd: u64,
    _width: i32,
    _height: i32,
    _capture_cursor: bool,
) -> Result<Vec<u8>, String> {
    Err("Screen capture is only supported on Windows".to_string())
}

/// Simple nearest-neighbor resize for BGRA frame data
fn resize_frame(data: &[u8], src_w: i32, src_h: i32, dst_w: i32, dst_h: i32) -> Vec<u8> {
    let src_w_u = src_w as usize;
    let src_h_u = src_h as usize;
    let dst_w_u = dst_w as usize;
    let dst_h_u = dst_h as usize;
    let mut result = vec![0u8; dst_w_u * dst_h_u * 4];

    for dy in 0..dst_h_u {
        for dx in 0..dst_w_u {
            let sx = (dx * src_w_u) / dst_w_u;
            let sy = (dy * src_h_u) / dst_h_u;
            let src_idx = (sy * src_w_u + sx) * 4;
            let dst_idx = (dy * dst_w_u + dx) * 4;
            if src_idx + 3 < data.len() {
                result[dst_idx..dst_idx + 4].copy_from_slice(&data[src_idx..src_idx + 4]);
            }
        }
    }
    result
}

// ─── AVI MJPG encoding (pure Rust, no external tools) ───

struct AviEncoder {
    writer: BufWriter<std::fs::File>,
    frame_count: u64,
    frames_offset_pos: u64,  // file position of dwTotalFrames in avih header
    width: i32,
    height: i32,
    quality: u32,
    movi_size_pos: u64,      // file position of dwMoviChunkSize in avih (offset 52 from RIFF start)
}

impl AviEncoder {
    fn new(output_path: &str, width: i32, height: i32, _fps: u32, quality: u32) -> Result<Self, String> {
        let file = std::fs::File::create(output_path)
            .map_err(|e| format!("Cannot create output file: {}", e))?;
        let mut writer = BufWriter::with_capacity(512 * 1024, file); // 512KB write buffer

        let fps = _fps;

        // ─── Write RIFF header placeholder ───
        // We'll fill in the real sizes later.
        // RIFF 'AVI ' + LIST 'hdrl' + LIST 'movi' = 12 + hdrl_size + movi_size
        writer.write_all(b"RIFF").map_err(|e| e.to_string())?;
        let _riff_size_pos: u64 = 4; // position of RIFF size field
        writer.write_all(&[0u8; 4]).map_err(|e| e.to_string())?; // placeholder
        writer.write_all(b"AVI ").map_err(|e| e.to_string())?;

        // ─── LIST 'hdrl' ───
        writer.write_all(b"LIST").map_err(|e| e.to_string())?;
        writer.write_all(&[0u8; 4]).map_err(|e| e.to_string())?; // placeholder for hdrl size
        let hdrl_start = writer.stream_position().map_err(|e| e.to_string())?;
        writer.write_all(b"hdrl").map_err(|e| e.to_string())?;

        // avih chunk
        writer.write_all(b"avih").map_err(|e| e.to_string())?;
        let avih_size: u32 = 56;
        writer.write_all(&avih_size.to_le_bytes()).map_err(|e| e.to_string())?;

        let us_per_frame = (1_000_000.0 / fps as f64) as u32;
        writer.write_all(&us_per_frame.to_le_bytes()).map_err(|e| e.to_string())?;  // dwMicroSecPerFrame
        let max_bytes = (width * height * 2) as u32;
        writer.write_all(&max_bytes.to_le_bytes()).map_err(|e| e.to_string())?;    // dwMaxBytesPerSec
        writer.write_all(&[0u8; 4]).map_err(|e| e.to_string())?;                   // dwPaddingGranularity
        writer.write_all(&[0u8; 4]).map_err(|e| e.to_string())?;                   // dwFlags
        let frames_offset = writer.stream_position().map_err(|e| e.to_string())?;  // save position
        writer.write_all(&[0u8; 4]).map_err(|e| e.to_string())?;                   // dwTotalFrames (filled later)
        writer.write_all(&[0u8; 4]).map_err(|e| e.to_string())?;                   // dwInitialFrames
        writer.write_all(&1u32.to_le_bytes()).map_err(|e| e.to_string())?;         // dwStreams
        writer.write_all(&[0u8; 4]).map_err(|e| e.to_string())?;                   // dwSuggestedBufferSize
        writer.write_all(&(width as u32).to_le_bytes()).map_err(|e| e.to_string())?; // dwWidth
        writer.write_all(&(height as u32).to_le_bytes()).map_err(|e| e.to_string())?; // dwHeight
        writer.write_all(&[0u8; 16]).map_err(|e| e.to_string())?;                  // dwReserved[4]

        // ─── LIST 'strl' ───
        writer.write_all(b"LIST").map_err(|e| e.to_string())?;
        let strl_size: u32 = 4 + 8 + 56 + 8 + 40; // 'strl' + strh + strf
        writer.write_all(&strl_size.to_le_bytes()).map_err(|e| e.to_string())?;
        writer.write_all(b"strl").map_err(|e| e.to_string())?;

        // strh
        writer.write_all(b"strh").map_err(|e| e.to_string())?;
        writer.write_all(&56u32.to_le_bytes()).map_err(|e| e.to_string())?;         // strh size
        writer.write_all(b"vids").map_err(|e| e.to_string())?;                       // fccType
        writer.write_all(b"MJPG").map_err(|e| e.to_string())?;                       // fccHandler
        writer.write_all(&[0u8; 4]).map_err(|e| e.to_string())?;                    // dwFlags
        writer.write_all(&[0u8; 2]).map_err(|e| e.to_string())?;                    // wPriority, wLanguage
        writer.write_all(&[0u8; 4]).map_err(|e| e.to_string())?;                    // dwInitialFrames
        writer.write_all(&1u32.to_le_bytes()).map_err(|e| e.to_string())?;           // dwScale
        writer.write_all(&fps.to_le_bytes()).map_err(|e| e.to_string())?;            // dwRate (fps)
        writer.write_all(&[0u8; 4]).map_err(|e| e.to_string())?;                    // dwStart
        writer.write_all(&[0u8; 4]).map_err(|e| e.to_string())?;                    // dwLength (filled later from dwTotalFrames)
        writer.write_all(&max_bytes.to_le_bytes()).map_err(|e| e.to_string())?;     // dwSuggestedBufferSize
        writer.write_all(&[0u8; 4]).map_err(|e| e.to_string())?;                    // dwQuality
        writer.write_all(&[0u8; 4]).map_err(|e| e.to_string())?;                    // dwSampleSize
        let w = width as u16;
        let h = height as u16;
        writer.write_all(&w.to_le_bytes()).map_err(|e| e.to_string())?;             // rcFrame
        writer.write_all(&h.to_le_bytes()).map_err(|e| e.to_string())?;
        writer.write_all(&[0u8; 2]).map_err(|e| e.to_string())?;
        writer.write_all(&[0u8; 2]).map_err(|e| e.to_string())?;

        // strf (BITMAPINFOHEADER for MJPG)
        writer.write_all(b"strf").map_err(|e| e.to_string())?;
        writer.write_all(&40u32.to_le_bytes()).map_err(|e| e.to_string())?;         // biSize
        writer.write_all(&(width as u32).to_le_bytes()).map_err(|e| e.to_string())?; // biWidth
        writer.write_all(&(height as u32).to_le_bytes()).map_err(|e| e.to_string())?; // biHeight
        writer.write_all(&1u16.to_le_bytes()).map_err(|e| e.to_string())?;          // biPlanes
        writer.write_all(&24u16.to_le_bytes()).map_err(|e| e.to_string())?;         // biBitCount
        writer.write_all(b"MJPG").map_err(|e| e.to_string())?;                      // biCompression
        let image_size = (width * height * 3) as u32;
        writer.write_all(&image_size.to_le_bytes()).map_err(|e| e.to_string())?;    // biSizeImage
        writer.write_all(&[0u8; 16]).map_err(|e| e.to_string())?;                   // biXPels, biYPels, biClrUsed, biClrImportant

        // Update hdrl size
        let hdrl_end = writer.stream_position().map_err(|e| e.to_string())?;
        let hdrl_size = (hdrl_end - hdrl_start) as u32;
        // Seek back to write hdrl size (4 bytes after "LIST")
        let hdrl_size_field_pos = hdrl_start - 4; // position of size field
        let current_pos = writer.stream_position().map_err(|e| e.to_string())?;
        writer.seek(std::io::SeekFrom::Start(hdrl_size_field_pos)).map_err(|e| e.to_string())?;
        writer.write_all(&hdrl_size.to_le_bytes()).map_err(|e| e.to_string())?;
        writer.seek(std::io::SeekFrom::Start(current_pos)).map_err(|e| e.to_string())?;

        // ─── LIST 'movi' ───
        writer.write_all(b"LIST").map_err(|e| e.to_string())?;
        let movi_size_pos = writer.stream_position().map_err(|e| e.to_string())?;
        writer.write_all(&[0u8; 4]).map_err(|e| e.to_string())?; // placeholder
        writer.write_all(b"movi").map_err(|e| e.to_string())?;

        Ok(Self {
            writer,
            frame_count: 0,
            frames_offset_pos: frames_offset,
            width,
            height,
            quality: quality.clamp(5, 100),
            movi_size_pos,
        })
    }

    fn write_frame(&mut self, bgra_data: &[u8]) -> Result<(), String> {
        let w = self.width as usize;
        let h = self.height as usize;
        let expected = w * h * 4;

        // Convert BGRA → RGB for JPEG encoder
        let mut rgb = vec![0u8; w * h * 3];
        let src = if bgra_data.len() >= expected { bgra_data } else { return Err("Frame data too small".into()); };

        for y in 0..h {
            for x in 0..w {
                let src_idx = (y * w + x) * 4;
                let dst_idx = (y * w + x) * 3;
                rgb[dst_idx]     = src[src_idx + 2]; // R ← B
                rgb[dst_idx + 1] = src[src_idx + 1]; // G ← G
                rgb[dst_idx + 2] = src[src_idx];     // B ← R
            }
        }

        // JPEG encode
        // Encode to memory
        let mut jpeg_buf = Vec::new();
        {
            let enc = jpeg_encoder::Encoder::new(&mut jpeg_buf, self.quality as u8);
            enc.encode(&rgb, w as u16, h as u16, jpeg_encoder::ColorType::Rgb)
                .map_err(|e| format!("JPEG encode error: {:?}", e))?;
        }

        // Pad JPEG data to even size (AVI requirement)
        let pad = if jpeg_buf.len() % 2 != 0 { 1 } else { 0 };

        // Write '00dc' chunk
        self.writer.write_all(b"00dc").map_err(|e| e.to_string())?;
        let chunk_size = (jpeg_buf.len() + pad) as u32;
        self.writer.write_all(&chunk_size.to_le_bytes()).map_err(|e| e.to_string())?;
        self.writer.write_all(&jpeg_buf).map_err(|e| e.to_string())?;
        if pad > 0 {
            self.writer.write_all(&[0]).map_err(|e| e.to_string())?;
        }

        self.frame_count += 1;
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), String> {
        // Flush and compute final sizes
        self.writer.flush().map_err(|e| e.to_string())?;
        let file_end = self.writer.stream_position().map_err(|e| e.to_string())?;

        // Update dwTotalFrames in avih
        self.writer.seek(std::io::SeekFrom::Start(self.frames_offset_pos)).map_err(|e| e.to_string())?;
        let n = self.frame_count as u32;
        self.writer.write_all(&n.to_le_bytes()).map_err(|e| e.to_string())?;

        // Update movi LIST size
        let movi_data_start = self.movi_size_pos + 4 + 4; // after "LIST" + size + "movi"
        let movi_size = (file_end - movi_data_start) as u32;
        self.writer.seek(std::io::SeekFrom::Start(self.movi_size_pos)).map_err(|e| e.to_string())?;
        self.writer.write_all(&movi_size.to_le_bytes()).map_err(|e| e.to_string())?;

        // Update RIFF size
        let riff_size = (file_end - 8) as u32;
        self.writer.seek(std::io::SeekFrom::Start(4)).map_err(|e| e.to_string())?;
        self.writer.write_all(&riff_size.to_le_bytes()).map_err(|e| e.to_string())?;

        self.writer.flush().map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[tauri::command]
pub fn start_screen_recording(
    app: tauri::AppHandle,
    config: RecordingConfig,
) -> Result<(), String> {
    let mut recording = RECORDING.lock().map_err(|e| e.to_string())?;
    if recording.is_some() {
        return Err("Recording is already in progress".to_string());
    }

    // Validate output directory
    let output_path = std::path::Path::new(&config.output_path);
    if let Some(parent) = output_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create output directory: {}", e))?;
        }
    }

    // Ensure .avi extension
    let output_path = if !config.output_path.to_lowercase().ends_with(".avi") {
        format!("{}.avi", config.output_path)
    } else {
        config.output_path.clone()
    };

    let is_running = Arc::new(AtomicBool::new(true));
    let is_paused = Arc::new(AtomicBool::new(false));
    let cancel_token = Arc::new(AtomicBool::new(false));

    let is_running_clone = is_running.clone();
    let is_paused_clone = is_paused.clone();
    let cancel_token_clone = cancel_token.clone();
    let config_clone = config.clone();
    let config_clone2 = config.clone();
    let app_clone = app.clone();
    let output_path_clone = output_path.clone();

    std::thread::spawn(move || {
        let result = run_recording_loop(
            &config_clone, &output_path_clone,
            &is_running_clone, &is_paused_clone, &cancel_token_clone,
            &app_clone,
        );

        if let Ok(mut rec) = RECORDING.lock() {
            *rec = None;
        }

        if let Err(e) = &result {
            let _ = app_clone.emit("screen-record-error", e);
        } else {
            let _ = app_clone.emit("screen-record-complete", &output_path_clone);
        }
    });

    *recording = Some(ActiveRecording {
        config: config_clone2,
        start_time: Instant::now(),
        pause_time: None,
        total_pause_duration: std::time::Duration::ZERO,
        frames_captured: 0,
        frames_dropped: 0,
        is_running,
        is_paused,
        cancel_token,
    });

    Ok(())
}

#[tauri::command]
pub fn pause_screen_recording() -> Result<(), String> {
    let mut recording = RECORDING.lock().map_err(|e| e.to_string())?;
    match recording.as_mut() {
        Some(rec) => {
            if rec.is_paused.load(Ordering::SeqCst) {
                return Err("Recording is already paused".to_string());
            }
            rec.is_paused.store(true, Ordering::SeqCst);
            rec.pause_time = Some(Instant::now());
            Ok(())
        }
        None => Err("No active recording".to_string()),
    }
}

#[tauri::command]
pub fn resume_screen_recording() -> Result<(), String> {
    let mut recording = RECORDING.lock().map_err(|e| e.to_string())?;
    match recording.as_mut() {
        Some(rec) => {
            if !rec.is_paused.load(Ordering::SeqCst) {
                return Err("Recording is not paused".to_string());
            }
            if let Some(pause_start) = rec.pause_time {
                rec.total_pause_duration += pause_start.elapsed();
            }
            rec.pause_time = None;
            rec.is_paused.store(false, Ordering::SeqCst);
            Ok(())
        }
        None => Err("No active recording".to_string()),
    }
}

#[tauri::command]
pub fn stop_screen_recording() -> Result<String, String> {
    let mut recording = RECORDING.lock().map_err(|e| e.to_string())?;
    match recording.take() {
        Some(rec) => {
            let output_path = rec.config.output_path.clone();
            rec.cancel_token.store(true, Ordering::SeqCst);
            rec.is_running.store(false, Ordering::SeqCst);
            Ok(output_path)
        }
        None => Err("No active recording".to_string()),
    }
}

#[tauri::command]
pub fn get_screen_recording_status() -> RecordingState {
    match RECORDING.lock() {
        Ok(recording) => match recording.as_ref() {
            Some(rec) => {
                let elapsed = rec.start_time.elapsed();
                let active_duration = elapsed.saturating_sub(rec.total_pause_duration);
                let duration_secs = active_duration.as_secs_f64();
                let current_fps = if duration_secs > 0.0 {
                    rec.frames_captured as f64 / duration_secs
                } else {
                    0.0
                };
                let file_size = std::fs::metadata(&rec.config.output_path)
                    .map(|m| m.len()).unwrap_or(0);

                RecordingState {
                    is_recording: true,
                    is_paused: rec.is_paused.load(Ordering::SeqCst),
                    duration_secs,
                    file_path: Some(rec.config.output_path.clone()),
                    file_size,
                    current_fps,
                    frames_captured: rec.frames_captured,
                    frames_dropped: rec.frames_dropped,
                }
            }
            None => RecordingState {
                is_recording: false, is_paused: false, duration_secs: 0.0,
                file_path: None, file_size: 0, current_fps: 0.0,
                frames_captured: 0, frames_dropped: 0,
            },
        },
        Err(_) => RecordingState {
            is_recording: false, is_paused: false, duration_secs: 0.0,
            file_path: None, file_size: 0, current_fps: 0.0,
            frames_captured: 0, frames_dropped: 0,
        },
    }
}

// ─── File utilities ───

#[tauri::command]
pub fn get_recordings_folder() -> Result<String, String> {
    let videos_dir = dirs::home_dir()
        .ok_or_else(|| "Cannot determine user directory".to_string())?;
    let folder = videos_dir.join("Videos").join("NexBox");
    std::fs::create_dir_all(&folder)
        .map_err(|e| format!("Failed to create recordings folder: {}", e))?;
    Ok(folder.to_string_lossy().to_string())
}

#[tauri::command]
pub fn pick_recording_save_path(_default_name: String) -> Result<String, String> {
    let folder = get_recordings_folder().unwrap_or_else(|_| {
        dirs::home_dir()
            .map(|d| d.join("Videos").join("NexBox").to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string())
    });

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let path = std::path::PathBuf::from(&folder)
        .join(format!("recording_{}.avi", timestamp));

    // Ensure unique filename
    let mut final_path = path.clone();
    let mut counter = 1;
    while final_path.exists() {
        let stem = format!("recording_{}_{}", timestamp, counter);
        final_path = std::path::PathBuf::from(&folder).join(format!("{}.avi", stem));
        counter += 1;
    }

    Ok(final_path.to_string_lossy().to_string())
}

// ─── Recording loop ───

fn run_recording_loop(
    config: &RecordingConfig,
    output_path: &str,
    is_running: &Arc<AtomicBool>,
    is_paused: &Arc<AtomicBool>,
    cancel_token: &Arc<AtomicBool>,
    app: &tauri::AppHandle,
) -> Result<(), String> {
    let width = config.output_width;
    let height = config.output_height;
    let fps = config.fps;
    let frame_interval = std::time::Duration::from_secs_f64(1.0 / fps as f64);

    // Initialize encoder
    let mut encoder = AviEncoder::new(output_path, width, height, fps, config.quality)?;

    while is_running.load(Ordering::SeqCst) && !cancel_token.load(Ordering::SeqCst) {
        let loop_start = Instant::now();

        if is_paused.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }

        let display_index = if config.mode == "fullscreen" { config.display_index } else { 0 };
        let window_hwnd = if config.mode == "window" { config.window_hwnd } else { 0 };

        unsafe {
            match capture_frame_gdi(display_index, window_hwnd, width, height, config.capture_cursor) {
                Ok(frame_data) => {
                    if let Err(e) = encoder.write_frame(&frame_data) {
                        let _ = app.emit("screen-record-error", &e);
                    }
                    if let Ok(mut recording) = RECORDING.lock() {
                        if let Some(rec) = recording.as_mut() {
                            rec.frames_captured += 1;
                        }
                    }
                }
                Err(e) => {
                    let _ = app.emit("screen-record-error", &e);
                }
            }
        }

        let elapsed = loop_start.elapsed();
        if elapsed < frame_interval {
            std::thread::sleep(frame_interval - elapsed);
        } else {
            if let Ok(mut recording) = RECORDING.lock() {
                if let Some(rec) = recording.as_mut() {
                    rec.frames_dropped += 1;
                }
            }
        }
    }

    encoder.finalize()?;
    Ok(())
}

// ─── Cleanup ───

pub fn cleanup() {
    if let Ok(mut recording) = RECORDING.lock() {
        if let Some(rec) = recording.take() {
            rec.cancel_token.store(true, Ordering::SeqCst);
            rec.is_running.store(false, Ordering::SeqCst);
        }
    }
}
