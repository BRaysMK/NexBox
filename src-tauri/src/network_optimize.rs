use std::collections::HashMap;
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::{env, path::Path};
use crate::optimization::{run_simple_feature, PerfTweakResult, CREATE_NO_WINDOW};

fn get_powershell_path() -> String {
    if let Ok(sysroot) = env::var("SystemRoot") {
        let ps_path = format!(r"{}\System32\WindowsPowerShell\v1.0\powershell.exe", sysroot);
        if Path::new(&ps_path).exists() {
            return ps_path;
        }
    }
    "powershell.exe".to_string()
}

// === 1. TCP 拥塞控制优化 ===

#[tauri::command]
pub async fn set_tcp_congestion() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
$current = netsh int tcp show supplemental | Select-String "拥塞控制提供程序|Congestion Control Provider"
$hasCtcp = $current -match "CTCP|CUBIC"
if (-not $hasCtcp) {
    netsh int tcp set supplemental Internet congestionprovider=ctcp 2>&1 | Out-Null
}
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn restore_tcp_congestion() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
netsh int tcp set supplemental Internet congestionprovider=newreno 2>&1 | Out-Null
Write-Output 'OK'
"#)
}

// === 2. TCP Chimney Offload ===

#[tauri::command]
pub async fn set_tcp_chimney_off() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
netsh int tcp set global chimney=disabled 2>&1 | Out-Null
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn restore_tcp_chimney() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
netsh int tcp set global chimney=enabled 2>&1 | Out-Null
Write-Output 'OK'
"#)
}

// === 3. Nagle 算法低延迟策略 ===

#[tauri::command]
pub async fn set_nagle_optimization() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
$interfaces = Get-ChildItem "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces"
foreach ($iface in $interfaces) {
    $guid = $iface.PSChildName
    $path = "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\$guid"
    $ipEnabled = (Get-ItemProperty -Path $path -Name "IPAddress" -ErrorAction SilentlyContinue).IPAddress
    if ($ipEnabled) {
        Set-ItemProperty -Path $path -Name "TCPNoDelay" -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
        Set-ItemProperty -Path $path -Name "TcpAckFrequency" -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
        Set-ItemProperty -Path $path -Name "TcpDelAckTicks" -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
    }
}
Set-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters" -Name "TcpAckFrequency" -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn restore_nagle_optimization() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
$interfaces = Get-ChildItem "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces"
foreach ($iface in $interfaces) {
    $guid = $iface.PSChildName
    $path = "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\$guid"
    $ipEnabled = (Get-ItemProperty -Path $path -Name "IPAddress" -ErrorAction SilentlyContinue).IPAddress
    if ($ipEnabled) {
        Remove-ItemProperty -Path $path -Name "TCPNoDelay" -ErrorAction SilentlyContinue
        Remove-ItemProperty -Path $path -Name "TcpAckFrequency" -ErrorAction SilentlyContinue
        Remove-ItemProperty -Path $path -Name "TcpDelAckTicks" -ErrorAction SilentlyContinue
    }
}
Remove-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters" -Name "TcpAckFrequency" -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === 4. 禁用网卡省电模式 ===

#[tauri::command]
pub async fn set_adapter_power_saving_off() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
$adapters = Get-NetAdapter -Physical -ErrorAction SilentlyContinue
foreach ($adapter in $adapters) {
    Disable-NetAdapterPowerManagement -Name $adapter.Name -ErrorAction SilentlyContinue | Out-Null
}
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn restore_adapter_power_saving() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
$adapters = Get-NetAdapter -Physical -ErrorAction SilentlyContinue
foreach ($adapter in $adapters) {
    Enable-NetAdapterPowerManagement -Name $adapter.Name -ErrorAction SilentlyContinue | Out-Null
}
Write-Output 'OK'
"#)
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

// === 6. 状态检测 ===

#[derive(serde::Serialize)]
pub struct NetworkTweakState {
    pub tcp_congestion_optimized: bool,
    pub chimney_offload: bool,
    pub nagle_optimized: bool,
    pub adapter_power_saving_off: bool,
    pub dns_primary: String,
    pub dns_secondary: String,
}

#[tauri::command]
pub async fn check_network_tweak_states() -> Result<NetworkTweakState, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let script = r#"
$ErrorActionPreference = 'SilentlyContinue'
$r = @{}

# TCP Congestion
$supp = netsh int tcp show supplemental 2>$null | Out-String
$r['tcp_congestion'] = ($supp -match "CTCP" -or $supp -match "CUBIC")

# Chimney Offload
$global = netsh int tcp show global 2>$null | Out-String
$r['chimney'] = ($global -match "Chimney Offload State.*disabled" -or $global -match "Chimney 卸载状态.*禁用")

# Nagle
$firstIface = Get-ChildItem "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces" |
    Where-Object { (Get-ItemProperty $_.PSPath -Name "IPAddress" -EA 0).IPAddress } |
    Select-Object -First 1
if ($firstIface) {
    $tcpNoDelay = (Get-ItemProperty $firstIface.PSPath -Name "TCPNoDelay" -EA 0).TCPNoDelay
    $r['nagle'] = ($tcpNoDelay -eq 1)
} else {
    $r['nagle'] = $false
}

# Adapter Power Saving
$powerMgmtDisabled = Get-NetAdapter -Physical -EA 0 | Where-Object { $_.PowerManagementEnabled -eq $false }
$r['powerSaving'] = ($powerMgmtDisabled.Count -gt 0)

# DNS
$dnsAdapter = Get-DnsClientServerAddress -AddressFamily IPv4 -EA 0 | Where-Object { $_.ServerAddresses -ne $null } | Select-Object -First 1
if ($dnsAdapter -and $dnsAdapter.ServerAddresses) {
    $addrs = $dnsAdapter.ServerAddresses
    $r['dns_primary'] = $addrs[0]
    $r['dns_secondary'] = if ($addrs.Count -gt 1) { $addrs[1] } else { "" }
} else {
    $r['dns_primary'] = ""
    $r['dns_secondary'] = ""
}

$r.Keys | ForEach-Object { Write-Host "$($_):$($r[$_])" }
"#;

    let ps_path = get_powershell_path();
    let result = Command::new(&ps_path)
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行检测命令失败: {}", e))?;

    if !result.status.success() {
        return Err("检测状态失败".to_string());
    }

    let stdout = String::from_utf8_lossy(&result.stdout);
    let mut state = HashMap::new();

    for line in stdout.lines() {
        if let Some(pos) = line.find(':') {
            let key = line[..pos].trim().to_string();
            let value = line[pos + 1..].trim().to_string();
            state.insert(key, value);
        }
    }

    let tcp_congestion = state.get("tcp_congestion").map(|v| v == "True").unwrap_or(false);
    let chimney = state.get("chimney").map(|v| v == "True").unwrap_or(false);
    let nagle = state.get("nagle").map(|v| v == "True").unwrap_or(false);
    let power_saving = state.get("powerSaving").map(|v| v == "True").unwrap_or(false);
    let dns_primary = state.get("dns_primary").cloned().unwrap_or_default();
    let dns_secondary = state.get("dns_secondary").cloned().unwrap_or_default();

    Ok(NetworkTweakState {
        tcp_congestion_optimized: tcp_congestion,
        chimney_offload: chimney,
        nagle_optimized: nagle,
        adapter_power_saving_off: power_saving,
        dns_primary,
        dns_secondary,
    })
}

// === 7. 批量优化 / 恢复（单次 PowerShell 调用） ===

#[tauri::command]
pub async fn batch_network_enable() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'

# TCP Congestion
$current = netsh int tcp show supplemental | Select-String "拥塞控制提供程序|Congestion Control Provider"
$hasCtcp = $current -match "CTCP|CUBIC"
if (-not $hasCtcp) {
    netsh int tcp set supplemental Internet congestionprovider=ctcp 2>&1 | Out-Null
}

# Chimney Offload
netsh int tcp set global chimney=disabled 2>&1 | Out-Null

# Nagle Optimization
$interfaces = Get-ChildItem "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces"
foreach ($iface in $interfaces) {
    $guid = $iface.PSChildName
    $path = "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\$guid"
    $ipEnabled = (Get-ItemProperty -Path $path -Name "IPAddress" -ErrorAction SilentlyContinue).IPAddress
    if ($ipEnabled) {
        Set-ItemProperty -Path $path -Name "TCPNoDelay" -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
        Set-ItemProperty -Path $path -Name "TcpAckFrequency" -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
        Set-ItemProperty -Path $path -Name "TcpDelAckTicks" -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
    }
}
Set-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters" -Name "TcpAckFrequency" -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue

# Adapter Power Saving
$adapters = Get-NetAdapter -Physical -ErrorAction SilentlyContinue
foreach ($adapter in $adapters) {
    Disable-NetAdapterPowerManagement -Name $adapter.Name -ErrorAction SilentlyContinue | Out-Null
}

Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn batch_network_disable() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'

# TCP Congestion - restore
netsh int tcp set supplemental Internet congestionprovider=newreno 2>&1 | Out-Null

# Chimney Offload - restore
netsh int tcp set global chimney=enabled 2>&1 | Out-Null

# Nagle - restore
$interfaces = Get-ChildItem "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces"
foreach ($iface in $interfaces) {
    $guid = $iface.PSChildName
    $path = "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\$guid"
    $ipEnabled = (Get-ItemProperty -Path $path -Name "IPAddress" -ErrorAction SilentlyContinue).IPAddress
    if ($ipEnabled) {
        Remove-ItemProperty -Path $path -Name "TCPNoDelay" -ErrorAction SilentlyContinue
        Remove-ItemProperty -Path $path -Name "TcpAckFrequency" -ErrorAction SilentlyContinue
        Remove-ItemProperty -Path $path -Name "TcpDelAckTicks" -ErrorAction SilentlyContinue
    }
}
Remove-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters" -Name "TcpAckFrequency" -ErrorAction SilentlyContinue

# Adapter Power Saving - restore
$adapters = Get-NetAdapter -Physical -ErrorAction SilentlyContinue
foreach ($adapter in $adapters) {
    Enable-NetAdapterPowerManagement -Name $adapter.Name -ErrorAction SilentlyContinue | Out-Null
}

Write-Output 'OK'
"#)
}
