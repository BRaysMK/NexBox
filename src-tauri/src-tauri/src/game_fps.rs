use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

static CURRENT_PID: AtomicU32 = AtomicU32::new(0);
static CURRENT_HWND: AtomicU64 = AtomicU64::new(0);
static DXGI_FRAME_COUNTER: AtomicU64 = AtomicU64::new(0);
static DWM_FRAME_COUNTER: AtomicU64 = AtomicU64::new(0);
static CURRENT_FPS: AtomicU32 = AtomicU32::new(0);
static SMOOTHED_FPS: AtomicU32 = AtomicU32::new(0);
static FPS_ACTIVE: AtomicBool = AtomicBool::new(false);
static TRACE_SESSION: AtomicU64 = AtomicU64::new(0);
static HOOK_HANDLE: Mutex<usize> = Mutex::new(0);
static OVERLAY_HWND: AtomicU64 = AtomicU64::new(0);

static TOTAL_EVENTS: AtomicU64 = AtomicU64::new(0);
static DXGI_PID_HITS: AtomicU64 = AtomicU64::new(0);
static DWM_PID_HITS: AtomicU64 = AtomicU64::new(0);
static DWM_TOTAL_EVENTS: AtomicU64 = AtomicU64::new(0);
static ETW_STARTED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "windows")]
mod win32_fps {
    use super::*;
    use std::mem;
    use std::ptr;
    use windows_sys::core::GUID;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::System::Diagnostics::Etw::*;
    use windows_sys::Win32::UI::Accessibility::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    const DXGI_PROVIDER_GUID: GUID = GUID {
        data1: 0xCA11C036,
        data2: 0x0102,
        data3: 0x4A2D,
        data4: [0xA6, 0xAD, 0xF0, 0x3C, 0xFE, 0xD5, 0xD3, 0xC9],
    };

    const D3D9_PROVIDER_GUID: GUID = GUID {
        data1: 0x783ACA0A,
        data2: 0x790E,
        data3: 0x4D7F,
        data4: [0x84, 0x51, 0xAA, 0x85, 0x05, 0x11, 0xC6, 0xB9],
    };

    const DWM_PROVIDER_GUID: GUID = GUID {
        data1: 0x9E9BBA3C,
        data2: 0x2E38,
        data3: 0x40CB,
        data4: [0x99, 0xF4, 0x9E, 0x82, 0x81, 0x42, 0x51, 0x64],
    };

    const WNODE_FLAG_TRACED_GUID: u32 = 0x00020000;
    const EVENT_TRACE_REAL_TIME_MODE: u32 = 0x00000100;
    const PROCESS_TRACE_MODE_EVENT_RECORD: u32 = 0x10000000;
    const EVENT_CONTROL_CODE_ENABLE_PROVIDER: u32 = 1;

    unsafe extern "system" fn on_foreground_changed(
        _hook: HWINEVENTHOOK,
        _event: u32,
        hwnd: HWND,
        _id_object: i32,
        _id_child: i32,
        _id_event_thread: u32,
        _dw_event_time: u32,
    ) {
        if !FPS_ACTIVE.load(Ordering::SeqCst) {
            return;
        }
        let overlay_hwnd = OVERLAY_HWND.load(Ordering::Relaxed) as usize;
        if overlay_hwnd != 0 && hwnd as usize == overlay_hwnd {
            return;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid != 0 {
            log::info!("FPS监控: 前台窗口切换 PID={} hwnd={:#X}", pid, hwnd as usize);
            CURRENT_PID.store(pid, Ordering::Relaxed);
            CURRENT_HWND.store(hwnd as u64, Ordering::Relaxed);
            DXGI_FRAME_COUNTER.store(0, Ordering::Relaxed);
            DWM_FRAME_COUNTER.store(0, Ordering::Relaxed);
            DXGI_PID_HITS.store(0, Ordering::Relaxed);
            DWM_PID_HITS.store(0, Ordering::Relaxed);
            DWM_TOTAL_EVENTS.store(0, Ordering::Relaxed);
            TOTAL_EVENTS.store(0, Ordering::Relaxed);
            LAST_DWM_TS.store(0, Ordering::Relaxed);
            DWM_SAMPLE_LOGGED.store(false, Ordering::Relaxed);
        }
    }

    unsafe fn guid_eq(a: &GUID, b: &GUID) -> bool {
        a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 && a.data4 == b.data4
    }

    static LAST_DWM_TS: AtomicU64 = AtomicU64::new(0);
    const DWM_DEDUP_100NS: u64 = 80_000;

    const DXGI_PRESENT_START_ID: u16 = 42;
    const D3D9_PRESENT_START_ID: u16 = 1;

    static DWM_SAMPLE_LOGGED: AtomicBool = AtomicBool::new(false);

    fn log_dwm_sample(target_hwnd: usize, target_pid: u32, data_ptr: *const usize, count: usize) {
        let n = count.min(8);
        let mut vals = [0usize; 8];
        unsafe {
            for i in 0..n {
                vals[i] = *data_ptr.add(i);
            }
        }
        log::info!(
            "DWM诊断: hwnd={:#X} pid={} udata[0..{}]={:X?}",
            target_hwnd, target_pid, n, &vals[..n]
        );
    }

    unsafe extern "system" fn on_etw_event(event_record: *mut EVENT_RECORD) {
        if event_record.is_null() {
            return;
        }
        TOTAL_EVENTS.fetch_add(1, Ordering::Relaxed);

        let record = &*event_record;
        let target_pid = CURRENT_PID.load(Ordering::Relaxed);
        if target_pid == 0 {
            return;
        }

        let pid = record.EventHeader.ProcessId;
        let opcode = record.EventHeader.EventDescriptor.Opcode;
        let event_id = record.EventHeader.EventDescriptor.Id;

        if pid == target_pid {
            let is_dxgi_present = event_id == DXGI_PRESENT_START_ID && opcode == 1;
            let is_d3d9_present = event_id == D3D9_PRESENT_START_ID && opcode == 13;

            if is_dxgi_present || is_d3d9_present {
                DXGI_PID_HITS.fetch_add(1, Ordering::Relaxed);
                DXGI_FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);
                return;
            }
        }

        if guid_eq(&record.EventHeader.ProviderId, &DWM_PROVIDER_GUID)
            && record.UserDataLength > 0
            && !record.UserData.is_null()
        {
            DWM_TOTAL_EVENTS.fetch_add(1, Ordering::Relaxed);
            let target_hwnd = CURRENT_HWND.load(Ordering::Relaxed) as usize;
            let ptr_size = mem::size_of::<usize>();
            let count = (record.UserDataLength as usize) / ptr_size;
            let data_ptr = record.UserData as *const usize;

            let mut matched = false;
            for i in 0..count {
                let value = *data_ptr.add(i);
                if value == 0 {
                    continue;
                }
                if target_hwnd != 0 && value == target_hwnd {
                    matched = true;
                    break;
                }
                let mut wpid = 0u32;
                GetWindowThreadProcessId(value as HWND, &mut wpid);
                if wpid == target_pid {
                    matched = true;
                    break;
                }
            }

            if matched {
                DWM_PID_HITS.fetch_add(1, Ordering::Relaxed);
                let dwm_ts = record.EventHeader.TimeStamp as u64;
                if dwm_ts > 0 {
                    let last = LAST_DWM_TS.load(Ordering::Relaxed);
                    if dwm_ts.saturating_sub(last) >= DWM_DEDUP_100NS {
                        LAST_DWM_TS.store(dwm_ts, Ordering::Relaxed);
                        DWM_FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);
                    }
                } else {
                    DWM_FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);
                }
            } else if !DWM_SAMPLE_LOGGED.load(Ordering::Relaxed) {
                DWM_SAMPLE_LOGGED.store(true, Ordering::Relaxed);
                log_dwm_sample(target_hwnd, target_pid, data_ptr, count);
            }
        }
    }

    pub unsafe fn register_foreground_hook() -> bool {
        let hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            ptr::null_mut(),
            Some(on_foreground_changed),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        );
        if !hook.is_null() {
            let mut lock = HOOK_HANDLE.lock().unwrap();
            *lock = hook as usize;
            true
        } else {
            log::warn!("FPS监控: SetWinEventHook 失败");
            false
        }
    }

    pub unsafe fn unregister_foreground_hook() {
        let mut lock = HOOK_HANDLE.lock().unwrap();
        if *lock != 0 {
            UnhookWinEvent(*lock as *mut std::ffi::c_void);
            *lock = 0;
        }
    }

    pub unsafe fn get_initial_foreground_pid() -> u32 {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return 0;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        CURRENT_HWND.store(hwnd as u64, Ordering::Relaxed);
        pid
    }

    unsafe fn build_trace_props_buffer(session_name: &[u16]) -> Vec<u8> {
        let prop_size = mem::size_of::<EVENT_TRACE_PROPERTIES>() + session_name.len() * 2;
        let mut buffer: Vec<u8> = vec![0; prop_size];

        let props = buffer.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
        (*props).Wnode.BufferSize = prop_size as u32;
        (*props).Wnode.Flags = WNODE_FLAG_TRACED_GUID;
        (*props).LogFileMode = EVENT_TRACE_REAL_TIME_MODE;
        (*props).LoggerNameOffset = mem::size_of::<EVENT_TRACE_PROPERTIES>() as u32;
        (*props).BufferSize = 16;
        (*props).MinimumBuffers = 10;
        (*props).MaximumBuffers = 30;

        let name_dst = buffer
            .as_mut_ptr()
            .add(mem::size_of::<EVENT_TRACE_PROPERTIES>()) as *mut u16;
        ptr::copy_nonoverlapping(session_name.as_ptr(), name_dst, session_name.len());

        buffer
    }

    unsafe fn enable_provider(session_handle: CONTROLTRACE_HANDLE, guid: &GUID, name: &str) {
        let result = EnableTraceEx2(
            session_handle,
            guid,
            EVENT_CONTROL_CODE_ENABLE_PROVIDER,
            0xFF,
            0,
            0,
            0,
            ptr::null(),
        );
        if result != ERROR_SUCCESS {
            log::warn!("FPS监控: 启用{} Provider失败: err={}", name, result);
        } else {
            log::info!("FPS监控: 启用{} Provider成功", name);
        }
    }

    pub unsafe fn start_etw_trace() -> Result<(), String> {
        let session_name: Vec<u16> = "NexBoxFpsMonitor\0".encode_utf16().collect();
        let name_ptr = session_name.as_ptr();

        let mut buffer = build_trace_props_buffer(&session_name);
        let props = buffer.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;

        let mut session_handle = CONTROLTRACE_HANDLE { Value: 0 };
        let result = StartTraceW(&mut session_handle, name_ptr, props);
        log::info!("FPS监控: StartTraceW 返回: {} (handle={})", result, session_handle.Value);

        if result == ERROR_ALREADY_EXISTS {
            log::warn!("FPS监控: 会话已存在，尝试停止后重建");
            let mut stop_buf = build_trace_props_buffer(&session_name);
            let stop_props = stop_buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
            StopTraceW(session_handle, name_ptr, stop_props);

            buffer = build_trace_props_buffer(&session_name);
            let props2 = buffer.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
            let result2 = StartTraceW(&mut session_handle, name_ptr, props2);
            log::info!("FPS监控: StartTraceW 重试返回: {} (handle={})", result2, session_handle.Value);
            if result2 != ERROR_SUCCESS {
                return Err(format!("StartTraceW 重试失败: {}", result2));
            }
        } else if result != ERROR_SUCCESS {
            return Err(format!("StartTraceW 失败: {}", result));
        }

        TRACE_SESSION.store(session_handle.Value, Ordering::SeqCst);

        enable_provider(session_handle, &DXGI_PROVIDER_GUID, "DXGI");
        enable_provider(session_handle, &D3D9_PROVIDER_GUID, "D3D9");
        enable_provider(session_handle, &DWM_PROVIDER_GUID, "DWM");

        if !FPS_ACTIVE.load(Ordering::SeqCst) {
            stop_etw_session(session_handle, &session_name);
            return Ok(());
        }

        let mut log_file: EVENT_TRACE_LOGFILEW = mem::zeroed();
        log_file.LoggerName = name_ptr as _;
        log_file.Anonymous1.ProcessTraceMode =
            EVENT_TRACE_REAL_TIME_MODE | PROCESS_TRACE_MODE_EVENT_RECORD;
        log_file.Anonymous2.EventRecordCallback = Some(on_etw_event);

        log::info!("FPS监控: 正在打开ETW跟踪...");
        let trace_handle = OpenTraceW(&mut log_file);
        if trace_handle.Value == u64::MAX {
            let err = GetLastError();
            stop_etw_session(session_handle, &session_name);
            return Err(format!("OpenTraceW 失败: GetLastError={}", err));
        }

        log::info!("FPS监控: OpenTraceW 成功 (trace_handle={}), 开始 ProcessTrace", trace_handle.Value);
        ETW_STARTED.store(true, Ordering::SeqCst);

        let handle_array = [trace_handle];
        let process_result = ProcessTrace(handle_array.as_ptr(), 1, ptr::null(), ptr::null());

        log::info!("FPS监控: ProcessTrace 返回: {}", process_result);
        ETW_STARTED.store(false, Ordering::SeqCst);

        CloseTrace(trace_handle);
        stop_etw_session(session_handle, &session_name);

        Ok(())
    }

    unsafe fn stop_etw_session(session_handle: CONTROLTRACE_HANDLE, session_name: &[u16]) {
        let name_ptr = session_name.as_ptr();
        let mut buffer = build_trace_props_buffer(session_name);
        let props = buffer.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
        StopTraceW(session_handle, name_ptr, props);
        TRACE_SESSION.store(0, Ordering::SeqCst);
        log::info!("FPS监控: ETW会话已停止");
    }

    pub unsafe fn stop_etw_trace() {
        let session_value = TRACE_SESSION.load(Ordering::SeqCst);
        if session_value == 0 {
            return;
        }
        let session_handle = CONTROLTRACE_HANDLE { Value: session_value };
        let session_name: Vec<u16> = "NexBoxFpsMonitor\0".encode_utf16().collect();
        stop_etw_session(session_handle, &session_name);
    }
}

pub fn get_cached_fps() -> Option<u32> {
    let fps = SMOOTHED_FPS.load(Ordering::Relaxed);
    if fps == 0 { None } else { Some(fps) }
}

pub fn set_overlay_hwnd(hwnd: u64) {
    OVERLAY_HWND.store(hwnd, Ordering::SeqCst);
    log::info!("FPS监控: Overlay窗口句柄设置为 {:#X}", hwnd);
}

pub fn clear_overlay_hwnd() {
    OVERLAY_HWND.store(0, Ordering::SeqCst);
    log::info!("FPS监控: Overlay窗口句柄已清除");
}

fn fps_counter_loop() {
    let mut smoothed: f64 = -1.0;
    let mut tick_count: u32 = 0;
    while FPS_ACTIVE.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(500));
        if !FPS_ACTIVE.load(Ordering::SeqCst) {
            break;
        }

        let dxgi_count = DXGI_FRAME_COUNTER.swap(0, Ordering::Relaxed);
        let dwm_count = DWM_FRAME_COUNTER.swap(0, Ordering::Relaxed);
        let total = TOTAL_EVENTS.swap(0, Ordering::Relaxed);
        let dxgi_hits = DXGI_PID_HITS.swap(0, Ordering::Relaxed);
        let dwm_hits = DWM_PID_HITS.swap(0, Ordering::Relaxed);
        let dwm_total = DWM_TOTAL_EVENTS.swap(0, Ordering::Relaxed);
        let etw_ok = ETW_STARTED.load(Ordering::Relaxed);
        let hwnd = CURRENT_HWND.load(Ordering::Relaxed);

        tick_count += 1;
        if tick_count <= 30 || tick_count % 20 == 0 {
            let pid = CURRENT_PID.load(Ordering::Relaxed);
            log::info!(
                "FPS监控: PID={} hwnd={:#X} etw={} total={} dxgi={}({}) dwm={}/{}/{} fps={} (tick={})",
                pid, hwnd, etw_ok, total,
                dxgi_count, dxgi_hits,
                dwm_count, dwm_hits, dwm_total,
                SMOOTHED_FPS.load(Ordering::Relaxed),
                tick_count
            );
        }

        let raw_count = if dxgi_count > 0 { dxgi_count } else { dwm_count };
        let current_fps = raw_count as f64 * 2.0;

        if smoothed < 0.0 {
            smoothed = current_fps;
        } else {
            smoothed = 0.3 * current_fps + 0.7 * smoothed;
        }

        let final_fps = smoothed.round() as u32;
        CURRENT_FPS.store(current_fps.round() as u32, Ordering::Relaxed);
        SMOOTHED_FPS.store(final_fps, Ordering::Relaxed);
    }
}

pub fn start_fps_monitor() {
    if FPS_ACTIVE.load(Ordering::SeqCst) {
        return;
    }
    FPS_ACTIVE.store(true, Ordering::SeqCst);

    #[cfg(target_os = "windows")]
    unsafe {
        let initial_pid = win32_fps::get_initial_foreground_pid();
        CURRENT_PID.store(initial_pid, Ordering::Relaxed);

        if !win32_fps::register_foreground_hook() {
            log::warn!("FPS监控: 前台窗口Hook注册失败，FPS可能不准确");
        }

        thread::spawn(|| {
            if let Err(e) = win32_fps::start_etw_trace() {
                log::error!("FPS监控: ETW启动失败: {}", e);
                FPS_ACTIVE.store(false, Ordering::SeqCst);
            }
        });

        thread::spawn(|| {
            fps_counter_loop();
        });

        log::info!("FPS监控: 已启动 (初始PID={})", initial_pid);
    }
}

pub fn stop_fps_monitor() {
    if !FPS_ACTIVE.load(Ordering::SeqCst) {
        return;
    }
    FPS_ACTIVE.store(false, Ordering::SeqCst);

    #[cfg(target_os = "windows")]
    unsafe {
        win32_fps::stop_etw_trace();
        win32_fps::unregister_foreground_hook();
    }

    CURRENT_PID.store(0, Ordering::Relaxed);
    CURRENT_HWND.store(0, Ordering::Relaxed);
    DXGI_FRAME_COUNTER.store(0, Ordering::Relaxed);
    DWM_FRAME_COUNTER.store(0, Ordering::Relaxed);
    CURRENT_FPS.store(0, Ordering::Relaxed);
    SMOOTHED_FPS.store(0, Ordering::Relaxed);
    TOTAL_EVENTS.store(0, Ordering::Relaxed);
    DXGI_PID_HITS.store(0, Ordering::Relaxed);
    DWM_PID_HITS.store(0, Ordering::Relaxed);
    DWM_TOTAL_EVENTS.store(0, Ordering::Relaxed);
    ETW_STARTED.store(false, Ordering::Relaxed);

    log::info!("FPS监控: 已停止");
}

pub fn cleanup() {
    stop_fps_monitor();
}
