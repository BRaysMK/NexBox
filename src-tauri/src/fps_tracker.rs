#![allow(dead_code)]
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use windows_sys::Win32::System::Diagnostics::Etw::{
    CloseTrace, ControlTraceW, CONTROLTRACE_HANDLE, EnableTraceEx2, ProcessTrace,
    PROCESSTRACE_HANDLE, StartTraceW, EVENT_CONTROL_CODE_ENABLE_PROVIDER, EVENT_RECORD,
    EVENT_TRACE_CONTROL_STOP, EVENT_TRACE_LOGFILEW, EVENT_TRACE_PROPERTIES,
    EVENT_TRACE_REAL_TIME_MODE, PROCESS_TRACE_MODE_EVENT_RECORD, PROCESS_TRACE_MODE_REAL_TIME,
    TRACE_LEVEL_VERBOSE, WNODE_FLAG_TRACED_GUID,
};
use windows_sys::Win32::System::Threading::GetCurrentProcessId;

#[link(name = "advapi32")]
extern "system" {
    fn OpenTraceW(logfile: *mut EVENT_TRACE_LOGFILEW) -> u64;
}

static FPS_ACTIVE: AtomicBool = AtomicBool::new(false);
static CURRENT_FPS: AtomicU32 = AtomicU32::new(0);
static TOTAL_EVENTS: AtomicU64 = AtomicU64::new(0);
static SESSION_HANDLE: AtomicU64 = AtomicU64::new(0);
static TRACE_HANDLE: AtomicU64 = AtomicU64::new(0);

const SESSION_NAME: &str = "NexBoxFpsSession";

const DXGKRNL_PROVIDER: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0x8C8F13B1,
    data2: 0x60EB,
    data3: 0x4B6A,
    data4: [0xA4, 0x33, 0xC4, 0x58, 0x41, 0xCE, 0xA1, 0x52],
};

const DXGI_PROVIDER: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0xCA11C036,
    data2: 0x0102,
    data3: 0x4A2D,
    data4: [0xA6, 0xAD, 0xF0, 0x3C, 0xFE, 0xD5, 0xD3, 0xC9],
};

struct FpsCalc {
    timestamps: VecDeque<Instant>,
    present_count: u64,
    last_fps_update: Instant,
}

impl FpsCalc {
    fn new() -> Self {
        Self {
            timestamps: VecDeque::with_capacity(600),
            present_count: 0,
            last_fps_update: Instant::now(),
        }
    }

    fn on_frame(&mut self) -> bool {
        let now = Instant::now();
        self.timestamps.push_back(now);
        self.present_count += 1;
        let cutoff = now - Duration::from_secs(2);
        while self.timestamps.front().map_or(false, |t| *t < cutoff) {
            self.timestamps.pop_front();
        }
        let elapsed = now.duration_since(self.last_fps_update);
        if elapsed >= Duration::from_millis(200) {
            self.last_fps_update = now;
            true
        } else {
            false
        }
    }

    fn fps(&self) -> u32 {
        let now = Instant::now();
        let cutoff = now - Duration::from_secs(1);
        self.timestamps.iter().filter(|t| **t >= cutoff).count() as u32
    }

    fn is_stale(&self) -> bool {
        self.timestamps
            .back()
            .map(|t| t.elapsed() > Duration::from_secs(3))
            .unwrap_or(true)
    }
}

struct TrackerState {
    per_pid: std::collections::HashMap<u32, FpsCalc>,
    evt_count: u64,
}

use std::sync::LazyLock;
static TRACKER_STATE: LazyLock<Mutex<TrackerState>> = LazyLock::new(|| {
    Mutex::new(TrackerState {
        per_pid: std::collections::HashMap::new(),
        evt_count: 0,
    })
});

#[derive(serde::Serialize, Clone)]
pub struct FpsDebugInfo {
    pub active: bool,
    pub fps: u32,
    pub total_events: u64,
    pub tracked_pids: Vec<u32>,
}

pub fn get_fps() -> Option<u32> {
    if !FPS_ACTIVE.load(Ordering::SeqCst) {
        return None;
    }
    let fps = CURRENT_FPS.load(Ordering::SeqCst);
    if fps == 0 {
        None
    } else {
        Some(fps)
    }
}

#[tauri::command]
pub fn get_debug_info() -> FpsDebugInfo {
    let active = FPS_ACTIVE.load(Ordering::SeqCst);
    let fps = CURRENT_FPS.load(Ordering::SeqCst);
    let total = TOTAL_EVENTS.load(Ordering::SeqCst);
    let pids: Vec<u32> = if active {
        TRACKER_STATE.lock().unwrap().per_pid.keys().copied().collect()
    } else {
        Vec::new()
    };
    FpsDebugInfo {
        active,
        fps,
        total_events: total,
        tracked_pids: pids,
    }
}

fn wcsdup(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn make_props_buffer(name_wide: &[u16]) -> (Vec<u8>, *mut EVENT_TRACE_PROPERTIES, u32) {
    let struct_sz = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() as u32;
    let name_bytes = name_wide.len() as u32 * 2;
    let total = struct_sz + name_bytes;
    let mut buf = vec![0u8; total as usize];
    let props = buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
    unsafe {
        (*props).Wnode.BufferSize = total;
        (*props).Wnode.Flags = WNODE_FLAG_TRACED_GUID;
        (*props).Wnode.ClientContext = 1;
        (*props).LogFileMode = EVENT_TRACE_REAL_TIME_MODE;
        (*props).LoggerNameOffset = struct_sz;
        (*props).LogFileNameOffset = 0;
        let dst = buf.as_mut_ptr().add(struct_sz as usize) as *mut u16;
        std::ptr::copy_nonoverlapping(name_wide.as_ptr(), dst, name_wide.len());
    }
    (buf, props, struct_sz)
}

fn stop_stale_session() {
    let name_wide = wcsdup(SESSION_NAME);
    unsafe {
        let (_buf, props, _) = make_props_buffer(&name_wide);
        let h = CONTROLTRACE_HANDLE { Value: 0 };
        let r = ControlTraceW(h, name_wide.as_ptr(), props, EVENT_TRACE_CONTROL_STOP);
        log::info!("FPS: stop_stale_session ControlTraceW=0x{:X}", r);
    }
}

pub fn start_fps_tracking() {
    if FPS_ACTIVE.load(Ordering::SeqCst) {
        return;
    }

    stop_stale_session();

    FPS_ACTIVE.store(true, Ordering::SeqCst);
    CURRENT_FPS.store(0, Ordering::SeqCst);
    TOTAL_EVENTS.store(0, Ordering::SeqCst);
    SESSION_HANDLE.store(0, Ordering::SeqCst);
    TRACE_HANDLE.store(0, Ordering::SeqCst);

    {
        let mut state = TRACKER_STATE.lock().unwrap();
        state.per_pid.clear();
        state.evt_count = 0;
    }

    thread::spawn(move || unsafe {
        let name_wide = wcsdup(SESSION_NAME);

        let (_buf, props, _struct_sz) = make_props_buffer(&name_wide);
        (*props).BufferSize = 64;
        (*props).MinimumBuffers = 8;
        (*props).MaximumBuffers = 64;

        let mut session_handle = CONTROLTRACE_HANDLE { Value: 0 };
        let result = StartTraceW(&mut session_handle, name_wide.as_ptr(), props);

        if result != 0 {
            log::error!("FPS: StartTraceW failed, error=0x{:X}", result);
            FPS_ACTIVE.store(false, Ordering::SeqCst);
            return;
        }

        log::info!("FPS: session started, handle=0x{:X}", session_handle.Value);
        SESSION_HANDLE.store(session_handle.Value, Ordering::SeqCst);

        let r1 = EnableTraceEx2(
            session_handle,
            &DXGKRNL_PROVIDER,
            EVENT_CONTROL_CODE_ENABLE_PROVIDER,
            TRACE_LEVEL_VERBOSE as u8,
            u64::MAX,
            0,
            0,
            std::ptr::null_mut(),
        );
        log::info!("FPS: EnableTraceEx2 DxgKrnl=0x{:X}", r1);

        let r2 = EnableTraceEx2(
            session_handle,
            &DXGI_PROVIDER,
            EVENT_CONTROL_CODE_ENABLE_PROVIDER,
            TRACE_LEVEL_VERBOSE as u8,
            u64::MAX,
            0,
            0,
            std::ptr::null_mut(),
        );
        log::info!("FPS: EnableTraceEx2 DXGI=0x{:X}", r2);

        let mut logfile: EVENT_TRACE_LOGFILEW = std::mem::zeroed();
        logfile.LoggerName = name_wide.as_ptr() as *mut u16;
        logfile.Anonymous1.ProcessTraceMode =
            PROCESS_TRACE_MODE_REAL_TIME | PROCESS_TRACE_MODE_EVENT_RECORD;
        logfile.Anonymous2.EventRecordCallback = Some(event_callback);

        log::info!(
            "FPS: EVENT_TRACE_LOGFILEW size={}",
            std::mem::size_of::<EVENT_TRACE_LOGFILEW>()
        );

        let trace_handle_raw = OpenTraceW(&mut logfile);
        log::info!("FPS: OpenTraceW=0x{:X}", trace_handle_raw);

        if trace_handle_raw == u64::MAX || trace_handle_raw == 0 {
            log::error!("FPS: OpenTraceW failed");
            cleanup_session(session_handle);
            FPS_ACTIVE.store(false, Ordering::SeqCst);
            return;
        }

        TRACE_HANDLE.store(trace_handle_raw, Ordering::SeqCst);

        let trace_handle = PROCESSTRACE_HANDLE {
            Value: trace_handle_raw,
        };

        log::info!("FPS: ProcessTrace starting...");

        let result = ProcessTrace(&trace_handle, 1, std::ptr::null(), std::ptr::null());
        log::info!("FPS: ProcessTrace returned 0x{:X}", result);

        CloseTrace(trace_handle);
        cleanup_session(session_handle);
        SESSION_HANDLE.store(0, Ordering::SeqCst);
        TRACE_HANDLE.store(0, Ordering::SeqCst);
        FPS_ACTIVE.store(false, Ordering::SeqCst);
        log::info!("FPS: tracing stopped");
    });

    thread::spawn(|| {
        let mut last_total = 0u64;
        loop {
            if !FPS_ACTIVE.load(Ordering::SeqCst) {
                break;
            }
            thread::sleep(Duration::from_secs(2));

            let total = TOTAL_EVENTS.load(Ordering::SeqCst);
            let fps = CURRENT_FPS.load(Ordering::SeqCst);
            let pids: Vec<u32> = TRACKER_STATE
                .lock()
                .unwrap()
                .per_pid
                .keys()
                .copied()
                .collect();

            if total > last_total {
                log::info!(
                    "FPS: total_events={} (+{}) tracked={:?} fps={}",
                    total,
                    total - last_total,
                    pids,
                    fps
                );
            } else {
                log::info!(
                    "FPS: total_events={} tracked={:?} fps={} (idle)",
                    total,
                    pids,
                    fps
                );
            }
            last_total = total;
        }
    });
}

unsafe fn cleanup_session(session_handle: CONTROLTRACE_HANDLE) {
    let props_size = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() as u32;
    let mut buffer = vec![0u8; props_size as usize];
    let props = buffer.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
    (*props).Wnode.BufferSize = props_size;
    ControlTraceW(
        session_handle,
        std::ptr::null(),
        props,
        EVENT_TRACE_CONTROL_STOP,
    );
}

pub fn stop_fps_tracking() {
    if !FPS_ACTIVE.load(Ordering::SeqCst) {
        return;
    }
    log::info!("FPS: stop_fps_tracking called");
    FPS_ACTIVE.store(false, Ordering::SeqCst);
    CURRENT_FPS.store(0, Ordering::SeqCst);

    let session_val = SESSION_HANDLE.load(Ordering::SeqCst);
    let trace_val = TRACE_HANDLE.load(Ordering::SeqCst);

    if trace_val != 0 {
        unsafe {
            CloseTrace(PROCESSTRACE_HANDLE { Value: trace_val });
        }
        TRACE_HANDLE.store(0, Ordering::SeqCst);
    }

    if session_val != 0 {
        unsafe {
            cleanup_session(CONTROLTRACE_HANDLE { Value: session_val });
        }
        SESSION_HANDLE.store(0, Ordering::SeqCst);
    }
}

unsafe extern "system" fn event_callback(record: *mut EVENT_RECORD) {
    if record.is_null() {
        return;
    }

    if !FPS_ACTIVE.load(Ordering::SeqCst) {
        return;
    }

    let provider = (*record).EventHeader.ProviderId;
    let pid = (*record).EventHeader.ProcessId;
    let event_id = (*record).EventHeader.EventDescriptor.Id;

    if pid == 0 || pid == 4 {
        return;
    }

    let self_pid = GetCurrentProcessId();
    if pid == self_pid {
        return;
    }

    TOTAL_EVENTS.fetch_add(1, Ordering::SeqCst);

    let mut state = match TRACKER_STATE.lock() {
        Ok(s) => s,
        Err(_) => return,
    };

    state.evt_count = state.evt_count.wrapping_add(1);

    let is_dxgkrnl = equal_guid(&provider, &DXGKRNL_PROVIDER);
    let is_dxgi = equal_guid(&provider, &DXGI_PROVIDER);

    let is_present = (is_dxgkrnl && event_id == 0x0B5A) || (is_dxgi && event_id == 0x002A);

    if is_dxgkrnl || is_dxgi {
        if !is_present && state.evt_count <= 20 {
            log::info!(
                "FPS: {} PID={} EventID=0x{:04X} (non-present)",
                if is_dxgkrnl { "DxgKrnl" } else { "DXGI" },
                pid,
                event_id
            );
        }
        if is_present && state.evt_count <= 5 {
            log::info!(
                "FPS: PRESENT {} PID={} EventID=0x{:04X}",
                if is_dxgkrnl { "DxgKrnl" } else { "DXGI" },
                pid,
                event_id
            );
        }
    }

    if !is_present {
        return;
    }

    let calc = state.per_pid.entry(pid).or_insert_with(FpsCalc::new);
    let should_update = calc.on_frame();

    if should_update {
        state.per_pid.retain(|_, c| !c.is_stale());
        let max_fps = state.per_pid.values().map(|c| c.fps()).max().unwrap_or(0);
        CURRENT_FPS.store(max_fps, Ordering::SeqCst);
    }
}

fn equal_guid(a: &windows_sys::core::GUID, b: &windows_sys::core::GUID) -> bool {
    a.data1 == b.data1
        && a.data2 == b.data2
        && a.data3 == b.data3
        && a.data4 == b.data4
}

pub fn cleanup() {
    stop_fps_tracking();
}
