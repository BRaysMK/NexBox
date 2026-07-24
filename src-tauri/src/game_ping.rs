use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

static PING_ACTIVE: AtomicBool = AtomicBool::new(false);
static CURRENT_PING: AtomicU32 = AtomicU32::new(0);
static PING_THREAD_HANDLE: Mutex<Option<thread::JoinHandle<()>>> = Mutex::new(None);

const DEFAULT_SERVER: &str = "game.qq.com";

#[cfg(target_os = "windows")]
mod win32_ping {
    use windows_sys::Win32::NetworkManagement::IpHelper::*;
    use windows_sys::Win32::Foundation::*;
    use std::mem;
    use std::ptr;
    use std::time::Instant;
    use std::net::ToSocketAddrs;

    const ICMP_TIMEOUT: u32 = 3000;

    pub fn ping_host(host: &str) -> Option<u32> {
        let addr = resolve_host(host)?;
        let handle = unsafe { IcmpCreateFile() };
        if handle == INVALID_HANDLE_VALUE {
            return None;
        }
        let result = ping_addr(handle, addr);
        unsafe { IcmpCloseHandle(handle) };
        result
    }

    fn resolve_host(host: &str) -> Option<u32> {
        let host_with_port = format!("{}:0", host);
        let addrs: Vec<std::net::SocketAddr> = host_with_port.to_socket_addrs().ok()?.collect();
        for addr in addrs {
            if let std::net::SocketAddr::V4(v4) = addr {
                let ip = v4.ip().octets();
                let ip_u32 = ((ip[3] as u32) << 24) | ((ip[2] as u32) << 16) | ((ip[1] as u32) << 8) | (ip[0] as u32);
                return Some(ip_u32);
            }
        }
        None
    }

    fn ping_addr(handle: HANDLE, dest_addr: u32) -> Option<u32> {
        let send_data: [u8; 32] = [0; 32];
        let mut reply_buffer: [u8; 256] = [0; 256];
        let reply_size = mem::size_of::<ICMP_ECHO_REPLY>() as u32 + send_data.len() as u32;

        let start = Instant::now();

        let ret = unsafe {
            IcmpSendEcho(
                handle,
                dest_addr,
                send_data.as_ptr() as *const _,
                send_data.len() as u16,
                ptr::null(),
                reply_buffer.as_mut_ptr() as *mut _,
                reply_size,
                ICMP_TIMEOUT,
            )
        };

        if ret == 0 {
            return None;
        }

        let reply_ptr = reply_buffer.as_ptr() as *const ICMP_ECHO_REPLY;
        let reply_ref = unsafe { &*reply_ptr };

        if reply_ref.Status != IP_SUCCESS {
            return None;
        }

        Some(start.elapsed().as_millis() as u32)
    }
}

#[cfg(not(target_os = "windows"))]
mod win32_ping {
    pub fn ping_host(_host: &str) -> Option<u32> {
        None
    }
}

pub fn get_cached_ping() -> Option<u32> {
    let ping = CURRENT_PING.load(Ordering::SeqCst);
    if ping == 0 { None } else { Some(ping) }
}

pub fn start_ping_thread() {
    if PING_ACTIVE.load(Ordering::SeqCst) {
        return;
    }

    PING_ACTIVE.store(true, Ordering::SeqCst);

    let handle = thread::spawn(|| {
        while PING_ACTIVE.load(Ordering::SeqCst) {
            let ping_result = win32_ping::ping_host(DEFAULT_SERVER);
            let ping_value = ping_result.unwrap_or(0);
            CURRENT_PING.store(ping_value, Ordering::SeqCst);

            for _ in 0..40 {
                if !PING_ACTIVE.load(Ordering::SeqCst) {
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
    });

    let mut handle_lock = PING_THREAD_HANDLE.lock().unwrap();
    *handle_lock = Some(handle);
}

pub fn stop_ping_thread() {
    PING_ACTIVE.store(false, Ordering::SeqCst);

    let mut handle_lock = PING_THREAD_HANDLE.lock().unwrap();
    if let Some(handle) = handle_lock.take() {
        let _ = handle.join();
    }

    CURRENT_PING.store(0, Ordering::SeqCst);
}

#[tauri::command]
pub async fn get_current_ping() -> Result<Option<u32>, String> {
    Ok(get_cached_ping())
}

pub fn cleanup() {
    stop_ping_thread();
}