use std::os::windows::process::CommandExt;
use std::process::Command;
use std::{env, path::Path};
use crate::optimization::{run_simple_feature, PerfTweakResult, CREATE_NO_WINDOW};
use encoding_rs::GBK;
use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE};
use winreg::RegKey;

fn get_powershell_path() -> String {
    if let Ok(sysroot) = env::var("SystemRoot") {
        let ps_path = format!(r"{}\System32\WindowsPowerShell\v1.0\powershell.exe", sysroot);
        if Path::new(&ps_path).exists() {
            return ps_path;
        }
    }
    "powershell.exe".to_string()
}

/// 原生执行 netsh 命令，返回解码后的输出；失败时检查权限错误
fn run_netsh_result(args: &[&str]) -> Result<String, String> {
    let out = Command::new("netsh")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行 netsh 失败: {}", e))?;
    let text = if !out.stdout.is_empty() {
        decode_console(out.stdout)
    } else {
        decode_console(out.stderr)
    };
    if out.status.success() {
        Ok(text)
    } else {
        let lower = text.to_lowercase();
        if lower.contains("access denied") || lower.contains("denied") || lower.contains("拒绝访问") || lower.contains("权限不足") {
            Err("需要管理员权限，请以管理员身份运行 NexBox".to_string())
        } else {
            Err(format!("命令执行失败: {}", text.trim()))
        }
    }
}

/// 网卡设备注册表类键（用于禁用/恢复网卡省电）
const NIC_CLASS_KEY: &str = r"SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}";

/// Nagle：原生注册表写入，对每个有 IP 的接口设置低延迟参数
fn set_nagle_native() -> Result<(), String> {
    let params = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters", KEY_SET_VALUE)
        .map_err(|e| format!("打开 Tcpip 参数键失败: {}", e))?;
    params
        .set_value("TcpAckFrequency", &1u32)
        .map_err(|e| format!("写入 TcpAckFrequency 失败: {}", e))?;

    let ifaces = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces", KEY_READ)
        .map_err(|e| format!("打开 Tcpip 接口键失败: {}", e))?;
    for name in ifaces.enum_keys().flatten() {
        if let Ok(key) = ifaces.open_subkey_with_flags(&name, KEY_SET_VALUE) {
            let has_ip = key.get_value::<Vec<String>, _>("IPAddress").is_ok()
                || key.get_value::<String, _>("IPAddress").is_ok();
            if has_ip {
                let _ = key.set_value("TCPNoDelay", &1u32);
                let _ = key.set_value("TcpAckFrequency", &1u32);
                let _ = key.set_value("TcpDelAckTicks", &0u32);
            }
        }
    }
    Ok(())
}

/// Nagle：原生删除低延迟参数，恢复默认
fn restore_nagle_native() -> Result<(), String> {
    let params = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters", KEY_SET_VALUE)
        .map_err(|e| format!("打开 Tcpip 参数键失败: {}", e))?;
    let _ = params.delete_value("TcpAckFrequency");

    let ifaces = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces", KEY_READ)
        .map_err(|e| format!("打开 Tcpip 接口键失败: {}", e))?;
    for name in ifaces.enum_keys().flatten() {
        if let Ok(key) = ifaces.open_subkey_with_flags(&name, KEY_SET_VALUE) {
            let has_ip = key.get_value::<Vec<String>, _>("IPAddress").is_ok()
                || key.get_value::<String, _>("IPAddress").is_ok();
            if has_ip {
                let _ = key.delete_value("TCPNoDelay");
                let _ = key.delete_value("TcpAckFrequency");
                let _ = key.delete_value("TcpDelAckTicks");
            }
        }
    }
    Ok(())
}

/// 网卡省电：off=true 设置 PnPCapabilities 0x100 位禁用省电；off=false 清除该位
fn set_power_saving_native(off: bool) -> Result<(), String> {
    let class = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(NIC_CLASS_KEY, KEY_READ)
        .map_err(|e| format!("打开网卡类键失败: {}", e))?;
    for name in class.enum_keys().flatten() {
        if let Ok(key) = class.open_subkey_with_flags(&name, KEY_READ | KEY_SET_VALUE) {
            // 仅处理网卡设备（有 DriverDesc）
            if key.get_value::<String, _>("DriverDesc").is_err() {
                continue;
            }
            let cap = key.get_value::<u32, _>("PnPCapabilities").unwrap_or(0);
            let new = if off { cap | 0x100 } else { cap & !0x100 };
            if new != cap {
                let _ = key.set_value("PnPCapabilities", &new);
            }
        }
    }
    Ok(())
}

// === 1. TCP 拥塞控制优化 ===

#[tauri::command]
pub async fn set_tcp_congestion() -> Result<PerfTweakResult, String> {
    run_netsh_result(&["int", "tcp", "set", "supplemental", "Internet", "congestionprovider=ctcp"])
        .map(|_| PerfTweakResult {
            success: true,
            message: "TCP 拥塞控制已优化".to_string(),
        })
}

#[tauri::command]
pub async fn restore_tcp_congestion() -> Result<PerfTweakResult, String> {
    run_netsh_result(&["int", "tcp", "set", "supplemental", "Internet", "congestionprovider=newreno"])
        .map(|_| PerfTweakResult {
            success: true,
            message: "TCP 拥塞控制已恢复".to_string(),
        })
}

// === 2. TCP Chimney Offload ===

#[tauri::command]
pub async fn set_tcp_chimney_off() -> Result<PerfTweakResult, String> {
    run_netsh_result(&["int", "tcp", "set", "global", "chimney=disabled"])
        .map(|_| PerfTweakResult {
            success: true,
            message: "TCP Chimney Offload 已禁用".to_string(),
        })
}

#[tauri::command]
pub async fn restore_tcp_chimney() -> Result<PerfTweakResult, String> {
    run_netsh_result(&["int", "tcp", "set", "global", "chimney=enabled"])
        .map(|_| PerfTweakResult {
            success: true,
            message: "TCP Chimney Offload 已恢复".to_string(),
        })
}

// === 3. Nagle 算法低延迟策略 ===

#[tauri::command]
pub async fn set_nagle_optimization() -> Result<PerfTweakResult, String> {
    set_nagle_native().map(|_| PerfTweakResult {
        success: true,
        message: "Nagle 低延迟优化已应用".to_string(),
    })
}

#[tauri::command]
pub async fn restore_nagle_optimization() -> Result<PerfTweakResult, String> {
    restore_nagle_native().map(|_| PerfTweakResult {
        success: true,
        message: "Nagle 低延迟优化已恢复".to_string(),
    })
}

// === 4. 禁用网卡省电模式 ===

#[tauri::command]
pub async fn set_adapter_power_saving_off() -> Result<PerfTweakResult, String> {
    set_power_saving_native(true).map(|_| PerfTweakResult {
        success: true,
        message: "网卡省电模式已禁用".to_string(),
    })
}

#[tauri::command]
pub async fn restore_adapter_power_saving() -> Result<PerfTweakResult, String> {
    set_power_saving_native(false).map(|_| PerfTweakResult {
        success: true,
        message: "网卡省电模式已恢复".to_string(),
    })
}

// === 5. DNS 优化 ===

#[tauri::command]
pub async fn set_dns_servers(dns_primary: String, dns_secondary: String) -> Result<PerfTweakResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let script = format!(
        r#"
$ErrorActionPreference = 'SilentlyContinue'
$adapters = Get-NetAdapter -Physical -ErrorAction SilentlyContinue | Where-Object {{ $_.Status -eq "Up" }}
foreach ($adapter in $adapters) {{
    Set-DnsClientServerAddress -InterfaceIndex $adapter.ifIndex -ServerAddresses ("{0}", "{1}") -ErrorAction SilentlyContinue | Out-Null
}}
Write-Output 'OK'
"#,
        dns_primary, dns_secondary
    );

    let ps_path = get_powershell_path();
    let result = Command::new(&ps_path)
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行命令失败: {}", e))?;

    if result.status.success() {
        Ok(PerfTweakResult { success: true, message: format!("DNS 已切换到 {} / {}", dns_primary, dns_secondary) })
    } else {
        let stderr = String::from_utf8_lossy(&result.stderr).to_string();
        let stdout = String::from_utf8_lossy(&result.stdout).to_string();
        let err_msg = if !stderr.trim().is_empty() { stderr.trim().to_string() } else { stdout.trim().to_string() };
        let lower = err_msg.to_lowercase();
        if lower.contains("access denied") || lower.contains("denied") || lower.contains("拒绝访问") || lower.contains("权限不足") {
            Err("需要管理员权限，请以管理员身份运行 NexBox".to_string())
        } else {
            Err(format!("DNS 设置失败: {}", err_msg))
        }
    }
}

#[tauri::command]
pub async fn restore_dns_servers() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
$adapters = Get-NetAdapter -Physical -ErrorAction SilentlyContinue | Where-Object { $_.Status -eq "Up" }
foreach ($adapter in $adapters) {
    Set-DnsClientServerAddress -InterfaceIndex $adapter.ifIndex -ResetServerAddresses -ErrorAction SilentlyContinue | Out-Null
}
Write-Output 'OK'
"#)
}

/// 清理 DNS 解析缓存（ipconfig /flushdns，无需 PowerShell）
#[tauri::command]
pub async fn clear_dns_cache() -> Result<PerfTweakResult, String> {
    let result = Command::new("ipconfig")
        .arg("/flushdns")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行清理 DNS 缓存失败: {}", e))?;

    if result.status.success() {
        Ok(PerfTweakResult { success: true, message: "DNS 缓存已清理".to_string() })
    } else {
        let stderr = decode_console(result.stderr);
        let stdout = decode_console(result.stdout);
        let err_msg = if !stderr.trim().is_empty() { stderr.trim().to_string() } else { stdout.trim().to_string() };
        let lower = err_msg.to_lowercase();
        if lower.contains("access denied") || lower.contains("denied") || lower.contains("拒绝访问") || lower.contains("权限不足") {
            Err("需要管理员权限，请以管理员身份运行 NexBox".to_string())
        } else {
            Err(format!("清理 DNS 缓存失败: {}", err_msg))
        }
    }
}

/// 重置网络栈（netsh winsock reset + netsh int ip reset），常用于解决网络异常
/// 注意：执行后需重启电脑才能完全生效。
#[tauri::command]
pub async fn reset_network() -> Result<PerfTweakResult, String> {
    let winsock = Command::new("netsh")
        .args(["winsock", "reset"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("重置 Winsock 失败: {}", e))?;
    let ip = Command::new("netsh")
        .args(["int", "ip", "reset"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("重置 TCP/IP 协议栈失败: {}", e))?;

    let mut combined = String::new();
    for out in [winsock, ip] {
        if !out.stdout.is_empty() {
            combined.push_str(&decode_console(out.stdout));
        }
        if !out.stderr.is_empty() {
            combined.push_str(&decode_console(out.stderr));
        }
        combined.push('\n');
    }

    let lower = combined.to_lowercase();
    if lower.contains("access denied") || lower.contains("denied") || lower.contains("拒绝访问") || lower.contains("权限不足") {
        return Err("需要管理员权限，请以管理员身份运行 NexBox".to_string());
    }

    Ok(PerfTweakResult {
        success: true,
        message: "网络已重置，建议重启电脑后生效".to_string(),
    })
}

// === 6. 状态检测（纯 Rust 实现，不启动 PowerShell，毫秒级） ===

#[derive(serde::Serialize)]
pub struct NetworkTweakState {
    pub tcp_congestion_optimized: bool,
    pub chimney_offload: bool,
    pub nagle_optimized: bool,
    pub adapter_power_saving_off: bool,
    pub dns_primary: String,
    pub dns_secondary: String,
}

/// 解码控制台输出（中文 Windows 的 netsh 输出为 CP936/GBK 编码）
fn decode_console(bytes: Vec<u8>) -> String {
    let (cow, _, _) = GBK.decode(&bytes);
    cow.into_owned()
}

/// 直接运行 netsh，返回解码后的输出（无 PowerShell 包装）
fn run_netsh(args: &[&str]) -> String {
    let out = Command::new("netsh")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    match out {
        Ok(o) => {
            if !o.stdout.is_empty() {
                decode_console(o.stdout)
            } else {
                decode_console(o.stderr)
            }
        }
        Err(_) => String::new(),
    }
}

fn is_chimney_disabled(output: &str) -> bool {
    let has_chimney = output.contains("Chimney Offload State") || output.contains("Chimney 卸载状态");
    has_chimney && (output.to_lowercase().contains("disabled") || output.contains("禁用"))
}

/// Nagle：读取有 IPAddress 的接口中是否存在 TCPNoDelay=1
fn check_nagle() -> bool {
    let Ok(interfaces) = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces")
    else {
        return false;
    };
    for name in interfaces.enum_keys().flatten() {
        let Ok(key) = interfaces.open_subkey(&name) else { continue };
        let has_ip = key.get_value::<Vec<String>, _>("IPAddress").is_ok()
            || key.get_value::<String, _>("IPAddress").is_ok();
        if !has_ip {
            continue;
        }
        if key.get_value::<u32, _>("TCPNoDelay").ok() == Some(1) {
            return true;
        }
    }
    false
}

/// 网卡省电：网卡设备注册表 PnPCapabilities 含 0x100 位表示已禁用省电
fn check_power_saving() -> bool {
    let Ok(adapters) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(
        r"SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}",
    ) else {
        return false;
    };
    for name in adapters.enum_keys().flatten() {
        let Ok(key) = adapters.open_subkey(&name) else { continue };
        if key
            .get_value::<u32, _>("PnPCapabilities")
            .ok()
            .is_some_and(|v| v & 0x100 != 0)
        {
            return true;
        }
    }
    false
}

/// DNS：优先读 NameServer（手动设置），否则读 DhcpNameServer（DHCP 分配）
fn read_dns() -> (String, String) {
    let Ok(interfaces) = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces")
    else {
        return (String::new(), String::new());
    };
    for name in interfaces.enum_keys().flatten() {
        let Ok(key) = interfaces.open_subkey(&name) else { continue };
        let has_ip = key.get_value::<Vec<String>, _>("IPAddress").is_ok()
            || key.get_value::<String, _>("IPAddress").is_ok();
        if !has_ip {
            continue;
        }
        let servers = key
            .get_value::<String, _>("NameServer")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| key.get_value::<String, _>("DhcpNameServer").ok())
            .filter(|s| !s.trim().is_empty());
        if let Some(s) = servers {
            let parts: Vec<&str> = s
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter(|x| !x.is_empty())
                .collect();
            if let Some(primary) = parts.first() {
                let secondary = parts.get(1).copied().unwrap_or_default();
                return (primary.to_string(), secondary.to_string());
            }
        }
    }
    (String::new(), String::new())
}

#[tauri::command]
pub async fn check_network_tweak_states() -> Result<NetworkTweakState, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    // 两个 netsh 查询并行执行（每个约 0.3~1s），避免串行等待
    let supp = tokio::task::spawn_blocking(|| run_netsh(&["int", "tcp", "show", "supplemental"]));
    let global = tokio::task::spawn_blocking(|| run_netsh(&["int", "tcp", "show", "global"]));
    let supp_out = supp.await.unwrap_or_default();
    let global_out = global.await.unwrap_or_default();

    // 以下均为注册表读取，毫秒级
    let supp_lower = supp_out.to_lowercase();
    let tcp_congestion_optimized = supp_lower.contains("ctcp") || supp_lower.contains("cubic");
    let chimney_offload = is_chimney_disabled(&global_out);
    let nagle_optimized = check_nagle();
    let adapter_power_saving_off = check_power_saving();
    let (dns_primary, dns_secondary) = read_dns();

    Ok(NetworkTweakState {
        tcp_congestion_optimized,
        chimney_offload,
        nagle_optimized,
        adapter_power_saving_off,
        dns_primary,
        dns_secondary,
    })
}

// === 7. 批量优化 / 恢复（原生实现，不启动 PowerShell） ===

#[tauri::command]
pub async fn batch_network_enable() -> Result<PerfTweakResult, String> {
    let mut errors = Vec::new();
    if let Err(e) = run_netsh_result(&["int", "tcp", "set", "supplemental", "Internet", "congestionprovider=ctcp"]) {
        errors.push(format!("TCP 拥塞控制: {}", e));
    }
    if let Err(e) = run_netsh_result(&["int", "tcp", "set", "global", "chimney=disabled"]) {
        errors.push(format!("Chimney Offload: {}", e));
    }
    if let Err(e) = set_nagle_native() {
        errors.push(format!("Nagle: {}", e));
    }
    if let Err(e) = set_power_saving_native(true) {
        errors.push(format!("网卡省电: {}", e));
    }
    if errors.is_empty() {
        Ok(PerfTweakResult {
            success: true,
            message: "网络优化已全部应用".to_string(),
        })
    } else {
        Err(errors.join("; "))
    }
}

#[tauri::command]
pub async fn batch_network_disable() -> Result<PerfTweakResult, String> {
    let mut errors = Vec::new();
    if let Err(e) = run_netsh_result(&["int", "tcp", "set", "supplemental", "Internet", "congestionprovider=newreno"]) {
        errors.push(format!("TCP 拥塞控制: {}", e));
    }
    if let Err(e) = run_netsh_result(&["int", "tcp", "set", "global", "chimney=enabled"]) {
        errors.push(format!("Chimney Offload: {}", e));
    }
    if let Err(e) = restore_nagle_native() {
        errors.push(format!("Nagle: {}", e));
    }
    if let Err(e) = set_power_saving_native(false) {
        errors.push(format!("网卡省电: {}", e));
    }
    if errors.is_empty() {
        Ok(PerfTweakResult {
            success: true,
            message: "网络优化已全部恢复".to_string(),
        })
    } else {
        Err(errors.join("; "))
    }
}

// === 8. 公网 IP 查询（国内可访问的免费 API，多源 fallback，仅返回 IPv4） ===

#[derive(Clone, Copy)]
enum PublicIpProvider {
    /// 返回纯 IP 文本
    Plain,
    /// 返回 key=value 文本（cloudflare trace），需解析 ip= 字段
    Trace,
}

/// 国内可访问的免费公网 IPv4 查询 API，按顺序 fallback
const PUBLIC_IP_PROVIDERS: &[(&str, PublicIpProvider)] = &[
    ("https://4.ipw.cn", PublicIpProvider::Plain),
    ("https://ip.3322.net", PublicIpProvider::Plain),
    ("https://myip.ipip.net", PublicIpProvider::Plain),
    ("https://api.ip.sb/ip", PublicIpProvider::Plain),
    ("https://api.ipify.org", PublicIpProvider::Plain),
    ("https://cloudflare.com/cdn-cgi/trace", PublicIpProvider::Trace),
];

/// 校验是否为合法的 IPv4 地址
fn is_valid_ipv4(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.len() <= 3
                && p.bytes().all(|b| b.is_ascii_digit())
                && p.parse::<u32>().map(|n| n <= 255).unwrap_or(false)
        })
}

/// 从任意文本中提取第一个 IPv4 地址
fn find_ipv4(text: &str) -> Option<String> {
    text.split(|c: char| !c.is_ascii_digit() && c != '.')
        .find(|s| is_valid_ipv4(s))
        .map(|s| s.to_string())
}

fn extract_ipv4(text: &str, provider: PublicIpProvider) -> Option<String> {
    match provider {
        PublicIpProvider::Plain => find_ipv4(text),
        PublicIpProvider::Trace => text
            .lines()
            .find_map(|line| line.trim().strip_prefix("ip=").and_then(find_ipv4)),
    }
}

/// 获取当前网络的公网 IPv4 地址（多 API 顺序 fallback，国内可访问）
#[tauri::command]
pub async fn get_public_ip() -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("初始化 HTTP 客户端失败: {}", e))?;

    for &(url, provider) in PUBLIC_IP_PROVIDERS {
        if let Ok(resp) = client.get(url).send().await {
            if let Ok(text) = resp.text().await {
                if let Some(ip) = extract_ipv4(&text, provider) {
                    return Ok(ip);
                }
            }
        }
    }

    Err("无法获取公网 IPv4 地址，请检查网络连接".to_string())
}
