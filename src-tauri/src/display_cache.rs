//! 显示器信息公共缓存
//!
//! 多个页面（滤镜、准心、分辨率管理）都需要获取显示器型号。
//! 获取真实型号需要通过 PowerShell 查询 WMI (`WmiMonitorID`)，
//! 这是一个耗时操作（启动 PowerShell 进程 + WMI 查询，通常 1~3 秒）。
//!
//! 之前每个页面进入时都会重复执行该查询，导致页面卡顿/卡死。
//! 本模块提供带 TTL 的全局缓存，使三个页面共享同一份查询结果，
//! 只有缓存过期（60 秒）后才会真正发起新的 PowerShell 查询。

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// 缓存有效期。
///
/// 显示器配置在一次使用会话中基本不会变化，60 秒足够安全；
/// 同时保证热插拔后一段时间能自动刷新。
const CACHE_TTL: Duration = Duration::from_secs(60);

struct EdidCache {
    fetched_at: Option<Instant>,
    names: Vec<String>,
}

static EDID_CACHE: Mutex<EdidCache> = Mutex::new(EdidCache {
    fetched_at: None,
    names: Vec::new(),
});

/// 获取 EDID 显示器型号名称（带 TTL 缓存）。
///
/// 缓存未命中时会启动 PowerShell 进程查询 WMI，属于阻塞操作，
/// 调用方应确保在 `spawn_blocking` 上下文中使用（或接受短时阻塞）。
pub fn get_edid_monitor_names() -> Vec<String> {
    // 先尝试读缓存
    {
        let lock = EDID_CACHE.lock().unwrap();
        if let Some(t) = lock.fetched_at {
            if t.elapsed() < CACHE_TTL {
                return lock.names.clone();
            }
        }
    }

    // 缓存过期或不存在，重新查询
    log::info!("display_cache: EDID 缓存未命中，启动 PowerShell 查询 WmiMonitorID…");
    let names = query_edid_via_powershell();
    log::info!("display_cache: EDID 查询完成，获取到 {} 个名称", names.len());

    let mut lock = EDID_CACHE.lock().unwrap();
    lock.fetched_at = Some(Instant::now());
    lock.names = names.clone();
    names
}

/// 强制刷新缓存（例如显示器配置变化时调用）。
#[allow(dead_code)]
pub fn invalidate() {
    let mut lock = EDID_CACHE.lock().unwrap();
    lock.fetched_at = None;
    lock.names.clear();
}

#[cfg(target_os = "windows")]
fn query_edid_via_powershell() -> Vec<String> {
    use base64::Engine;
    use std::process::Command;

    #[allow(non_snake_case)]
    #[derive(serde::Deserialize)]
    struct PsWmiMonitorId {
        UserFriendlyName: Option<String>,
    }

    let cmd = "ConvertTo-Json -Compress @(Get-CimInstance -Namespace root\\wmi WmiMonitorID | ForEach-Object { $friendly = ''; if ($_.UserFriendlyNameLength -gt 0) { $arr = @($_.UserFriendlyName); $max = [Math]::Min($arr.Count, $_.UserFriendlyNameLength); for ($i = 0; $i -lt $max; $i++) { $c = [char]$arr[$i]; if ($c -eq [char]0) { break } $friendly += $c } }; [PSCustomObject]@{ UserFriendlyName = $friendly.Trim() } })";
    let full = format!("[Console]::OutputEncoding = [Text.Encoding]::UTF8; {}", cmd);

    // UTF-16LE + Base64 编码，绕过系统代码页问题
    let utf16: Vec<u8> = full.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(&utf16);

    let mut command = Command::new("powershell");
    command.args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &encoded]);
    // CREATE_NO_WINDOW，避免弹出 PowerShell 窗口
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }

    let output = match command.output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Ok(o) => {
            log::warn!(
                "display_cache: PowerShell 查询失败，stderr: {}",
                String::from_utf8_lossy(&o.stderr)
            );
            return Vec::new();
        }
        Err(e) => {
            log::warn!("display_cache: 启动 PowerShell 失败: {}", e);
            return Vec::new();
        }
    };

    serde_json::from_str::<Vec<PsWmiMonitorId>>(&output)
        .unwrap_or_default()
        .into_iter()
        .map(|m| m.UserFriendlyName.unwrap_or_default())
        .collect()
}

#[cfg(not(target_os = "windows"))]
fn query_edid_via_powershell() -> Vec<String> {
    Vec::new()
}
