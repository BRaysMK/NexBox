use std::collections::HashMap;
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::{env, fs, path::Path};
use sysinfo::System;
use tauri::Manager;
use winreg::enums::*;
use winreg::RegKey;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::Threading::{OpenProcess, SetPriorityClass};

pub(crate) const CREATE_NO_WINDOW: u32 = 0x08000000;

fn get_powershell_path() -> String {
    if let Ok(sysroot) = env::var("SystemRoot") {
        let ps_path = format!(r"{}\System32\WindowsPowerShell\v1.0\powershell.exe", sysroot);
        if Path::new(&ps_path).exists() {
            return ps_path;
        }
    }
    "powershell.exe".to_string()
}

const PROCESS_SET_INFORMATION: u32 = 0x0200;
const IDLE_PRIORITY_CLASS: u32 = 0x00000040;

#[link(name = "kernel32")]
extern "system" {
    fn SetProcessInformation(
        hProcess: HANDLE,
        ProcessInformationClass: u32,
        ProcessInformation: *const std::ffi::c_void,
        ProcessInformationSize: u32,
    ) -> i32;
}

fn enable_process_efficiency_mode(pid: u32) -> bool {
    unsafe {
        let handle = OpenProcess(PROCESS_SET_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }

        let mut ok = true;

        if SetPriorityClass(handle, IDLE_PRIORITY_CLASS) == 0 {
            ok = false;
        }

        let state: [u32; 3] = [1, 1, 1];
        if SetProcessInformation(
            handle,
            12,
            &state as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<[u32; 3]>() as u32,
        ) == 0
        {
            ok = false;
        }

        CloseHandle(handle);
        ok
    }
}

fn run_bcdedit_admin(args: &str) -> Result<String, String> {
    let ps_script = format!(
        "Start-Process bcdedit -ArgumentList '{}' -Verb RunAs -Wait -WindowStyle Hidden",
        args
    );
    
    let result = Command::new("powershell")
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match result {
        Ok(output) => {
            if output.status.success() {
                Ok("命令执行成功".to_string())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                Err(format!("执行失败: {}", stderr))
            }
        }
        Err(e) => Err(format!("执行命令失败: {}", e)),
    }
}

#[derive(serde::Serialize)]
pub struct MemoryInfo {
    total: u64,
    available: u64,
    used: u64,
    usage_percent: f32,
}

#[derive(serde::Serialize)]
pub struct OptimizationResult {
    success: bool,
    message: String,
    before: MemoryInfo,
    after: MemoryInfo,
    freed_mb: u64,
}

fn get_memory_info() -> MemoryInfo {
    let mut sys = System::new_all();
    sys.refresh_all();

    let total = sys.total_memory() / 1024 / 1024;
    let available = sys.available_memory() / 1024 / 1024;
    let used = total - available;
    let usage_percent = if total > 0 {
        (used as f32 / total as f32) * 100.0
    } else {
        0.0
    };

    MemoryInfo {
        total,
        available,
        used,
        usage_percent,
    }
}

#[tauri::command]
pub async fn optimize_memory() -> Result<OptimizationResult, String> {
    let before = get_memory_info();

    if cfg!(target_os = "windows") {
        let result = Command::new("powershell")
            .args(&[
                "-NoProfile",
                "-ExecutionPolicy", "Bypass",
                "-Command",
                r#"
                    Add-Type -TypeDefinition @"
                    using System;
                    using System.Runtime.InteropServices;
                    public class Memory {
                        [DllImport("psapi.dll")]
                        public static extern bool EmptyWorkingSet(IntPtr hProcess);
                        [DllImport("kernel32.dll")]
                        public static extern IntPtr OpenProcess(uint dwDesiredAccess, bool bInheritHandle, int dwProcessId);
                        [DllImport("kernel32.dll")]
                        public static extern bool CloseHandle(IntPtr hObject);
                    }
"@
                    $PROCESS_QUERY_INFORMATION = 0x0400
                    $PROCESS_SET_QUOTA = 0x0100
                    $access = $PROCESS_QUERY_INFORMATION -bor $PROCESS_SET_QUOTA
                    $freed = 0
                    $processes = Get-Process -ErrorAction SilentlyContinue
                    foreach ($proc in $processes) {
                        try {
                            $handle = [Memory]::OpenProcess($access, $false, $proc.Id)
                            if ($handle -ne [IntPtr]::Zero) {
                                $wsBefore = $proc.WorkingSet64
                                [Memory]::EmptyWorkingSet($handle)
                                [Memory]::CloseHandle($handle) | Out-Null
                                $proc.Refresh()
                                $wsAfter = $proc.WorkingSet64
                                if ($wsBefore -gt $wsAfter) {
                                    $freed += [math]::Round(($wsBefore - $wsAfter) / 1MB, 2)
                                }
                            }
                        } catch {}
                    }
                    Write-Host "Freed: $freed MB"
                "#
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        match result {
            Ok(output) => {
                if output.status.success() {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    let after = get_memory_info();
                    let freed = if after.available > before.available {
                        after.available - before.available
                    } else {
                        0
                    };

                    Ok(OptimizationResult {
                        success: true,
                        message: format!("内存优化完成，释放约 {} MB", freed),
                        before,
                        after,
                        freed_mb: freed,
                    })
                } else {
                    let error_msg = String::from_utf8_lossy(&output.stderr).to_string();
                    Err(format!("内存优化失败: {}", error_msg))
                }
            }
            Err(e) => Err(format!("执行内存优化命令失败: {}", e)),
        }
    } else {
        Err("内存优化仅支持 Windows 系统".to_string())
    }
}

#[tauri::command]
pub async fn get_memory_status() -> Result<MemoryInfo, String> {
    Ok(get_memory_info())
}

#[derive(serde::Serialize)]
pub struct ProcessKillResult {
    success: bool,
    message: String,
    process_name: String,
    was_running: bool,
}

#[tauri::command]
pub async fn kill_wallpaper_engine() -> Result<ProcessKillResult, String> {
    let process_names = ["wallpaper64", "wallpaper32", "wallpaper_engine"];

    if cfg!(target_os = "windows") {
        let mut killed_any = false;
        let mut killed_name = String::new();

        for name in process_names {
            let result = Command::new("powershell")
                .args(&[
                    "-NoProfile",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-Command",
                    &format!(
                        r#"
                        $process = Get-Process -Name "{}" -ErrorAction SilentlyContinue
                        if ($process) {{
                            Stop-Process -Name "{}" -Force -ErrorAction SilentlyContinue
                            Write-Host "Killed: {}"
                            exit 0
                        }} else {{
                            Write-Host "Not running: {}"
                            exit 1
                        }}
                        "#,
                        name, name, name, name
                    ),
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .output();

            match result {
                Ok(output) => {
                    if output.status.success() {
                        killed_any = true;
                        killed_name = name.to_string();
                        break;
                    }
                }
                Err(_) => continue,
            }
        }

        if killed_any {
            Ok(ProcessKillResult {
                success: true,
                message: "Wallpaper Engine 进程已关闭".to_string(),
                process_name: killed_name,
                was_running: true,
            })
        } else {
            Ok(ProcessKillResult {
                success: true,
                message: "Wallpaper Engine 未在运行".to_string(),
                process_name: String::new(),
                was_running: false,
            })
        }
    } else {
        Err("此功能仅支持 Windows 系统".to_string())
    }
}

#[derive(serde::Serialize)]
pub struct PowerPlanResult {
    success: bool,
    message: String,
    previous_plan: Option<String>,
    current_plan: String,
}

#[tauri::command]
pub async fn set_high_performance_power_plan() -> Result<PowerPlanResult, String> {
    if cfg!(target_os = "windows") {
        let get_current_script = r#"
            powercfg /getactivescheme
        "#;

        let current_result = Command::new("powershell")
            .args(&[
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                get_current_script,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        let previous_plan = match current_result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                if let Some(line) = stdout.lines().next() {
                    Some(line.to_string())
                } else {
                    None
                }
            }
            Err(_) => None,
        };

        let set_script = r#"
            $highPerf = powercfg /list | Select-String "高性能|High performance|Ultimate" | Select-Object -First 1
            if ($highPerf) {
                $guid = ($highPerf -split '\s+')[3]
                if ($guid -match '^[a-fA-F0-9]{8}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{12}$') {
                    powercfg /setactive $guid
                    Write-Host "Switched to: $guid"
                    exit 0
                }
            }
            
            $ultimate = powercfg /list | Select-String "卓越性能|Ultimate Performance" | Select-Object -First 1
            if ($ultimate) {
                $guid = ($ultimate -split '\s+')[3]
                if ($guid -match '^[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{12}$') {
                    powercfg /setactive $guid
                    Write-Host "Switched to: $guid"
                    exit 0
                }
            }
            
            $highPerfGuid = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c"
            powercfg /setactive $highPerfGuid
            Write-Host "Switched to: $highPerfGuid"
            exit 0
        "#;

        let result = Command::new("powershell")
            .args(&[
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                set_script,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        match result {
            Ok(output) => {
                if output.status.success() {
                    let verify_result = Command::new("powershell")
                        .args(&[
                            "-NoProfile",
                            "-ExecutionPolicy",
                            "Bypass",
                            "-Command",
                            "powercfg /getactivescheme",
                        ])
                        .creation_flags(CREATE_NO_WINDOW)
                        .output();

                    let current_plan = match verify_result {
                        Ok(verify_output) => {
                            let stdout = String::from_utf8_lossy(&verify_output.stdout).to_string();
                            if let Some(line) = stdout.lines().next() {
                                line.to_string()
                            } else {
                                "高性能".to_string()
                            }
                        }
                        Err(_) => "高性能".to_string(),
                    };

                    Ok(PowerPlanResult {
                        success: true,
                        message: "已切换到高性能电源计划".to_string(),
                        previous_plan,
                        current_plan,
                    })
                } else {
                    let error_msg = String::from_utf8_lossy(&output.stderr).to_string();
                    Err(format!("切换电源计划失败: {}", error_msg))
                }
            }
            Err(e) => Err(format!("执行电源计划切换命令失败: {}", e)),
        }
    } else {
        Err("此功能仅支持 Windows 系统".to_string())
    }
}

#[derive(serde::Serialize)]
pub struct AceOptimizeResult {
    success: bool,
    message: String,
    optimized_processes: Vec<String>,
}

#[tauri::command]
pub async fn optimize_ace_processes() -> Result<AceOptimizeResult, String> {
    let process_names = ["ACE-Tray.exe", "SGuard64.exe", "SGuardSvc64.exe"];

    if cfg!(target_os = "windows") {
        let mut optimized_processes = Vec::new();

        let ps_script = r#"
            $processNames = @("ACE-Tray", "SGuard64", "SGuardSvc64")
            $optimized = @()
            
            foreach ($name in $processNames) {
                $processes = Get-Process -Name $name -ErrorAction SilentlyContinue
                if ($processes) {
                    foreach ($proc in $processes) {
                        try {
                            # 设置优先级为最低 (Low = 64)
                            $proc.PriorityClass = [System.Diagnostics.ProcessPriorityClass]::Low
                            
                            # 设置核心相关性为只使用 CPU0 (affinity = 1)
                            $proc.ProcessorAffinity = 1
                            
                            $optimized += $proc.ProcessName
                            Write-Host "Optimized: $($proc.ProcessName)"
                        } catch {
                            Write-Host "Failed to optimize: $name - $_"
                        }
                    }
                }
            }
            
            if ($optimized.Count -gt 0) {
                Write-Host "Optimized processes: $($optimized -join ', ')"
                exit 0
            } else {
                Write-Host "No ACE processes found"
                exit 1
            }
        "#;

        let result = Command::new("powershell")
            .args(&[
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                ps_script,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();

                for name in process_names {
                    let process_name = name.trim_end_matches(".exe");
                    if stdout.contains(&format!("Optimized: {}", process_name)) {
                        optimized_processes.push(process_name.to_string());
                    }
                }

                if !optimized_processes.is_empty() {
                    Ok(AceOptimizeResult {
                        success: true,
                        message: format!("已优化 {} 个ACE进程", optimized_processes.len()),
                        optimized_processes,
                    })
                } else {
                    Ok(AceOptimizeResult {
                        success: true,
                        message: "未找到运行中的ACE进程".to_string(),
                        optimized_processes: vec![],
                    })
                }
            }
            Err(e) => Err(format!("执行ACE进程优化命令失败: {}", e)),
        }
    } else {
        Err("此功能仅支持 Windows 系统".to_string())
    }
}

#[derive(serde::Serialize)]
pub struct AceEfficiencyResult {
    pub success: bool,
    pub message: String,
    pub count: u32,
}

#[tauri::command]
pub async fn set_ace_efficiency_mode() -> Result<AceEfficiencyResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let process_names = ["ACE-Tray.exe", "SGuard64.exe", "SGuardSvc64.exe"];
    let mut count = 0u32;

    let mut system = System::new();
    system.refresh_processes();

    for (_, process) in system.processes() {
        let name = process.name().to_string();
        let name_lower = name.to_lowercase();
        if process_names.iter().any(|n| n.to_lowercase() == name_lower) {
            if enable_process_efficiency_mode(process.pid().as_u32()) {
                count += 1;
            }
        }
    }

    Ok(AceEfficiencyResult {
        success: count > 0,
        message: if count > 0 {
            format!("已为 {} 个 ACE 进程开启效能模式", count)
        } else {
            "未找到运行中的 ACE 进程".to_string()
        },
        count,
    })
}

#[derive(serde::Serialize)]
pub struct DnsFlushResult {
    success: bool,
    message: String,
}

#[derive(serde::Serialize)]
pub struct TempCleanupResult {
    success: bool,
    message: String,
    scanned_files: u64,
    deleted_files: u64,
    deleted_dirs: u64,
    failed_items: u64,
}

#[derive(serde::Serialize)]
pub struct PrivacyServiceOptimizeResult {
    success: bool,
    message: String,
    stopped_services: Vec<String>,
}

fn clean_temp_dir(path: &Path) -> (u64, u64, u64, u64) {
    let mut scanned_files = 0;
    let mut deleted_files = 0;
    let mut deleted_dirs = 0;
    let mut failed_items = 0;

    let Ok(entries) = fs::read_dir(path) else {
        return (0, 0, 0, 1);
    };

    for entry in entries.flatten() {
        let entry_path = entry.path();
        if entry_path.is_dir() {
            let (s, df, dd, f) = clean_temp_dir(&entry_path);
            scanned_files += s;
            deleted_files += df;
            deleted_dirs += dd;
            failed_items += f;

            match fs::remove_dir(&entry_path) {
                Ok(_) => deleted_dirs += 1,
                Err(_) => failed_items += 1,
            }
        } else {
            scanned_files += 1;
            match fs::remove_file(&entry_path) {
                Ok(_) => deleted_files += 1,
                Err(_) => failed_items += 1,
            }
        }
    }

    (scanned_files, deleted_files, deleted_dirs, failed_items)
}

#[tauri::command]
pub async fn clean_temp_files() -> Result<TempCleanupResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let mut temp_paths = Vec::new();
    if let Ok(user_temp) = env::var("TEMP") {
        temp_paths.push(user_temp);
    }
    if let Ok(system_root) = env::var("SystemRoot") {
        temp_paths.push(format!("{system_root}\\Temp"));
    }
    temp_paths.sort();
    temp_paths.dedup();

    if temp_paths.is_empty() {
        return Err("未找到可清理的临时目录".to_string());
    }

    let mut scanned_files = 0;
    let mut deleted_files = 0;
    let mut deleted_dirs = 0;
    let mut failed_items = 0;

    for path in temp_paths {
        let dir = Path::new(&path);
        if !dir.exists() {
            continue;
        }
        let (s, df, dd, f) = clean_temp_dir(dir);
        scanned_files += s;
        deleted_files += df;
        deleted_dirs += dd;
        failed_items += f;
    }

    Ok(TempCleanupResult {
        success: true,
        message: format!("临时文件清理完成：删除 {} 个文件，{} 个目录", deleted_files, deleted_dirs),
        scanned_files,
        deleted_files,
        deleted_dirs,
        failed_items,
    })
}

#[tauri::command]
pub async fn optimize_privacy_services() -> Result<PrivacyServiceOptimizeResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let target_services = ["DiagTrack", "dmwappushservice", "diagnosticshub.standardcollector.service"];
    let mut stopped_services = Vec::new();

    for service in target_services {
        let result = Command::new("powershell")
            .args(&[
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &format!(
                    r#"
                    $svc = Get-Service -Name "{}" -ErrorAction SilentlyContinue
                    if ($svc) {{
                        if ($svc.Status -ne 'Stopped') {{
                            Stop-Service -Name "{}" -Force -ErrorAction SilentlyContinue
                            Write-Host "Stopped: {}"
                        }} else {{
                            Write-Host "AlreadyStopped: {}"
                        }}
                    }} else {{
                        Write-Host "NotFound: {}"
                    }}
                    "#,
                    service, service, service, service, service
                ),
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        if let Ok(output) = result {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            if stdout.contains(&format!("Stopped: {}", service))
                || stdout.contains(&format!("AlreadyStopped: {}", service))
            {
                stopped_services.push(service.to_string());
            }
        }
    }

    Ok(PrivacyServiceOptimizeResult {
        success: true,
        message: format!("服务优化完成：已处理 {} 个服务", stopped_services.len()),
        stopped_services,
    })
}

#[tauri::command]
pub async fn flush_dns() -> Result<DnsFlushResult, String> {
    if cfg!(target_os = "windows") {
        let result = Command::new("ipconfig")
            .args(&["/flushdns"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        match result {
            Ok(output) => {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    if stdout.contains("successfully") || stdout.contains("成功") {
                        Ok(DnsFlushResult {
                            success: true,
                            message: "DNS 缓存已成功清理".to_string(),
                        })
                    } else {
                        Ok(DnsFlushResult {
                            success: true,
                            message: "DNS 缓存清理完成".to_string(),
                        })
                    }
                } else {
                    let error_msg = String::from_utf8_lossy(&output.stderr).to_string();
                    Err(format!("DNS 清理失败: {}", error_msg))
                }
            }
            Err(e) => Err(format!("执行 DNS 清理命令失败: {}", e)),
        }
    } else {
        Err("此功能仅支持 Windows 系统".to_string())
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct MemoryLimitOption {
    pub id: String,
    pub label: String,
    pub limit_gb: f64,
    pub min_physical_gb: f64,
}

#[derive(serde::Serialize)]
pub struct MemoryLimitStatus {
    pub physical_memory_gb: f64,
    pub physical_memory_mb: u64,
    pub current_limit_mb: Option<u64>,
    pub available_options: Vec<MemoryLimitOption>,
}

#[derive(serde::Serialize)]
pub struct MemoryLimitResult {
    pub success: bool,
    pub message: String,
    pub limit_mb: Option<u64>,
    pub requires_restart: bool,
}

fn get_physical_memory_mb() -> u64 {
    let mut sys = System::new_all();
    sys.refresh_all();
    sys.total_memory() / 1024 / 1024
}

fn get_memory_limit_options_internal() -> Vec<MemoryLimitOption> {
    vec![
        MemoryLimitOption {
            id: "11.9gb".to_string(),
            label: "11.9 GB".to_string(),
            limit_gb: 11.9,
            min_physical_gb: 0.0,
        },
        MemoryLimitOption {
            id: "13.9gb".to_string(),
            label: "13.9 GB".to_string(),
            limit_gb: 13.9,
            min_physical_gb: 0.0,
        },
        MemoryLimitOption {
            id: "15.9gb".to_string(),
            label: "15.9 GB".to_string(),
            limit_gb: 15.9,
            min_physical_gb: 0.0,
        },
    ]
}

fn get_current_memory_limit() -> Option<u64> {
    if !cfg!(target_os = "windows") {
        return None;
    }

    let result = Command::new("bcdedit")
        .args(&["/enum", "{current}"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            for line in stdout.lines() {
                let lower_line = line.to_lowercase();
                if lower_line.contains("removememory") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    for part in parts.iter().rev() {
                        if let Ok(value) = part.parse::<u64>() {
                            return Some(value);
                        }
                    }
                }
            }
            None
        }
        Err(_) => None,
    }
}

#[tauri::command]
pub async fn get_memory_limit_options() -> Vec<MemoryLimitOption> {
    get_memory_limit_options_internal()
}

#[tauri::command]
pub async fn get_memory_limit_status() -> Result<MemoryLimitStatus, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let physical_memory_mb = get_physical_memory_mb();
    let physical_memory_gb = physical_memory_mb as f64 / 1024.0;
    let current_limit_mb = get_current_memory_limit();
    let all_options = get_memory_limit_options_internal();

    let available_options: Vec<MemoryLimitOption> = all_options
        .into_iter()
        .filter(|opt| opt.min_physical_gb <= physical_memory_gb)
        .collect();

    Ok(MemoryLimitStatus {
        physical_memory_gb,
        physical_memory_mb,
        current_limit_mb,
        available_options,
    })
}

#[tauri::command]
pub async fn set_memory_limit(limit_gb: f64) -> Result<MemoryLimitResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let physical_memory_mb = get_physical_memory_mb();
    let physical_memory_gb = physical_memory_mb as f64 / 1024.0;
    let limit_mb = (limit_gb * 1024.0) as u64;

    if limit_gb >= physical_memory_gb {
        return Err(format!(
            "限制值 ({:.1} GB) 不能大于或等于物理内存 ({:.1} GB)",
            limit_gb, physical_memory_gb
        ));
    }

    let remove_mb = physical_memory_mb.saturating_sub(limit_mb);
    let args = format!("/set \"{{current}}\" removememory {}", remove_mb);

    match run_bcdedit_admin(&args) {
        Ok(_) => Ok(MemoryLimitResult {
            success: true,
            message: format!("内存限制已设置为 {:.1} GB，需要重启生效", limit_gb),
            limit_mb: Some(limit_mb),
            requires_restart: true,
        }),
        Err(e) => Err(format!("设置内存限制失败: {}。请以管理员身份运行应用。", e)),
    }
}

#[tauri::command]
pub async fn restore_memory_limit() -> Result<MemoryLimitResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let args = "/deletevalue \"{current}\" removememory";

    match run_bcdedit_admin(args) {
        Ok(_) => Ok(MemoryLimitResult {
            success: true,
            message: "内存限制已恢复默认，需要重启生效".to_string(),
            limit_mb: None,
            requires_restart: true,
        }),
        Err(e) => Err(format!("恢复内存限制失败: {}。请以管理员身份运行应用。", e)),
    }
}

#[derive(serde::Serialize)]
pub struct DetailedMemoryInfo {
    pub physical_total: u64,
    pub physical_used: u64,
    pub physical_available: u64,
    pub virtual_total: u64,
    pub virtual_used: u64,
    pub virtual_available: u64,
    pub working_set_total: u64,
    pub working_set_used: u64,
    pub working_set_available: u64,
}

#[derive(serde::Serialize)]
pub struct MemoryCleanupResult {
    pub success: bool,
    pub message: String,
    pub freed_mb: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AutoCleanConfig {
    pub enabled: bool,
    pub interval_seconds: u64,
    pub threshold_mb: u64,
    pub clean_type: String,
}

static AUTO_CLEAN_CONFIG: Mutex<Option<AutoCleanConfig>> = Mutex::new(None);
static AUTO_CLEAN_GENERATION: AtomicU64 = AtomicU64::new(0);

#[tauri::command]
pub async fn get_detailed_memory_status() -> Result<DetailedMemoryInfo, String> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let physical_total = sys.total_memory() / 1024 / 1024;
    let physical_available = sys.available_memory() / 1024 / 1024;
    let physical_used = physical_total - physical_available;

    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let ps_script = r#"
        $os = Get-CimInstance Win32_OperatingSystem
        $virtualTotal = [math]::Round($os.TotalVirtualMemorySize / 1024)
        $virtualFree = [math]::Round($os.FreeVirtualMemory / 1024)
        $virtualUsed = $virtualTotal - $virtualFree
        $workingSet = [math]::Round(((Get-Process | Measure-Object WorkingSet64 -Sum -ErrorAction SilentlyContinue).Sum) / 1MB)
        Write-Host "VTOTAL:$virtualTotal"
        Write-Host "VFREE:$virtualFree"
        Write-Host "VUSED:$virtualUsed"
        Write-Host "WS:$workingSet"
    "#;

    let result = Command::new("powershell")
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let mut virtual_total: u64 = 0;
            let mut virtual_used: u64 = 0;
            let mut virtual_available: u64 = 0;
            let mut working_set_used: u64 = 0;

            for line in stdout.lines() {
                if line.starts_with("VTOTAL:") {
                    virtual_total = line.trim_start_matches("VTOTAL:").trim().parse().unwrap_or(0);
                } else if line.starts_with("VFREE:") {
                    virtual_available = line.trim_start_matches("VFREE:").trim().parse().unwrap_or(0);
                } else if line.starts_with("VUSED:") {
                    virtual_used = line.trim_start_matches("VUSED:").trim().parse().unwrap_or(0);
                } else if line.starts_with("WS:") {
                    working_set_used = line.trim_start_matches("WS:").trim().parse().unwrap_or(0);
                }
            }

            if virtual_available == 0 && virtual_total > 0 {
                virtual_available = virtual_total - virtual_used;
            }

            let working_set_total = sys.total_memory() / 1024 / 1024;
            let working_set_available = if working_set_total > working_set_used {
                working_set_total - working_set_used
            } else {
                0
            };

            Ok(DetailedMemoryInfo {
                physical_total,
                physical_used,
                physical_available,
                virtual_total,
                virtual_used,
                virtual_available,
                working_set_total,
                working_set_used,
                working_set_available,
            })
        }
        Err(e) => Err(format!("获取内存状态失败: {}", e)),
    }
}

fn clean_standby_memory_inner() -> u64 {
    let before = get_memory_info();

    let ps_script = r#"
        Add-Type -TypeDefinition @"
        using System;
        using System.Runtime.InteropServices;
        public class Win32Mem {
            [DllImport("kernel32.dll", SetLastError = true)]
            public static extern bool SetProcessWorkingSetSize(IntPtr hProcess, int dwMinimumWorkingSetSize, int dwMaximumWorkingSetSize);
            [DllImport("kernel32.dll")]
            public static extern IntPtr GetCurrentProcess();
        }
"@
        $handle = [Win32Mem]::GetCurrentProcess()
        [Win32Mem]::SetProcessWorkingSetSize($handle, -1, -1) | Out-Null
        Start-Sleep -Milliseconds 500
        Write-Host "Done"
    "#;

    let result = Command::new("powershell")
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match result {
        Ok(_) => {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let after = get_memory_info();
            if after.available > before.available {
                after.available - before.available
            } else {
                0
            }
        }
        Err(_) => 0,
    }
}

fn trim_working_set_inner() -> u64 {
    let before = get_memory_info();

    let ps_script = r#"
        Add-Type -TypeDefinition @"
        using System;
        using System.Runtime.InteropServices;
        public class Mem {
            [DllImport("psapi.dll", SetLastError = true)]
            public static extern bool EmptyWorkingSet(IntPtr hProcess);
            [DllImport("kernel32.dll")]
            public static extern IntPtr OpenProcess(uint dwDesiredAccess, bool bInheritHandle, int dwProcessId);
            [DllImport("kernel32.dll")]
            public static extern bool CloseHandle(IntPtr hObject);
        }
"@
        $PROCESS_QUERY_INFORMATION = 0x0400
        $PROCESS_SET_QUOTA = 0x0100
        $access = $PROCESS_QUERY_INFORMATION -bor $PROCESS_SET_QUOTA
        $freed = 0
        $processes = Get-Process -ErrorAction SilentlyContinue
        foreach ($proc in $processes) {
            try {
                $handle = [Mem]::OpenProcess($access, $false, $proc.Id)
                if ($handle -ne [IntPtr]::Zero) {
                    $wsBefore = $proc.WorkingSet64
                    [Mem]::EmptyWorkingSet($handle)
                    [Mem]::CloseHandle($handle) | Out-Null
                    $proc.Refresh()
                    $wsAfter = $proc.WorkingSet64
                    if ($wsBefore -gt $wsAfter) {
                        $freed += [math]::Round(($wsBefore - $wsAfter) / 1MB, 2)
                    }
                }
            } catch {}
        }
        Write-Host "Freed: $freed MB"
    "#;

    let result = Command::new("powershell")
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match result {
        Ok(_) => {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let after = get_memory_info();
            if after.available > before.available {
                after.available - before.available
            } else {
                0
            }
        }
        Err(_) => 0,
    }
}

#[tauri::command]
pub async fn clean_standby_memory() -> Result<MemoryCleanupResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let freed = clean_standby_memory_inner();

    Ok(MemoryCleanupResult {
        success: true,
        message: if freed > 0 {
            format!("待机内存清理完成，释放 {} MB", freed)
        } else {
            "待机内存已清理".to_string()
        },
        freed_mb: freed,
    })
}

#[tauri::command]
pub async fn trim_system_working_set() -> Result<MemoryCleanupResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let freed = trim_working_set_inner();

    Ok(MemoryCleanupResult {
        success: true,
        message: if freed > 0 {
            format!("系统工作集已收紧，释放 {} MB", freed)
        } else {
            "系统工作集已收紧".to_string()
        },
        freed_mb: freed,
    })
}

fn auto_clean_loop(config: AutoCleanConfig, generation: u64) {
    use std::time::Instant;

    const CHECK_INTERVAL_SECS: u64 = 5;
    let mut last_clean_time = Instant::now();

    loop {
        if AUTO_CLEAN_GENERATION.load(Ordering::Relaxed) != generation {
            break;
        }

        thread::sleep(Duration::from_secs(CHECK_INTERVAL_SECS));

        if AUTO_CLEAN_GENERATION.load(Ordering::Relaxed) != generation {
            break;
        }

        let mem_info = get_memory_info();
        let elapsed = last_clean_time.elapsed().as_secs();
        let interval_reached = elapsed >= config.interval_seconds;
        let threshold_reached = mem_info.used >= config.threshold_mb;

        if interval_reached || threshold_reached {
            match config.clean_type.as_str() {
                "all" => {
                    clean_standby_memory_inner();
                    trim_working_set_inner();
                }
                "standby" => {
                    clean_standby_memory_inner();
                }
                "working_set" => {
                    trim_working_set_inner();
                }
                _ => {}
            }
            last_clean_time = Instant::now();
        }
    }
}

#[tauri::command]
pub async fn start_auto_clean(config: AutoCleanConfig) -> Result<(), String> {
    let gen = AUTO_CLEAN_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;

    let mut cfg = AUTO_CLEAN_CONFIG.lock().map_err(|e| e.to_string())?;
    *cfg = Some(config.clone());
    drop(cfg);

    thread::spawn(move || {
        auto_clean_loop(config, gen);
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_auto_clean() -> Result<(), String> {
    AUTO_CLEAN_GENERATION.fetch_add(1, Ordering::Relaxed);
    let mut cfg = AUTO_CLEAN_CONFIG.lock().map_err(|e| e.to_string())?;
    *cfg = None;
    Ok(())
}

#[tauri::command]
pub async fn get_auto_clean_config() -> Result<Option<AutoCleanConfig>, String> {
    let cfg = AUTO_CLEAN_CONFIG.lock().map_err(|e| e.to_string())?;
    Ok(cfg.clone())
}

#[derive(serde::Serialize)]
pub struct ProcessOptimizeResult {
    pub success: bool,
    pub message: String,
    pub process_name: String,
    pub was_running: bool,
}

#[tauri::command]
pub async fn boost_delta_force_priority() -> Result<ProcessOptimizeResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let ps_script = r#"
        $proc = Get-Process -Name "DeltaForceClient-Win64-Shipping" -ErrorAction SilentlyContinue
        if ($proc) {
            $proc.PriorityClass = [System.Diagnostics.ProcessPriorityClass]::RealTime
            Write-Host "BOOSTED"
            exit 0
        } else {
            Write-Host "NOT_FOUND"
            exit 1
        }
    "#;

    let result = Command::new("powershell")
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            if stdout.contains("BOOSTED") {
                Ok(ProcessOptimizeResult {
                    success: true,
                    message: "三角洲进程优先级已提升为「超高」（实时）".to_string(),
                    process_name: "DeltaForceClient-Win64-Shipping.exe".to_string(),
                    was_running: true,
                })
            } else {
                Ok(ProcessOptimizeResult {
                    success: false,
                    message: "三角洲游戏未运行，请先启动游戏".to_string(),
                    process_name: "DeltaForceClient-Win64-Shipping.exe".to_string(),
                    was_running: false,
                })
            }
        }
        Err(e) => Err(format!("优化三角洲进程失败: {}", e)),
    }
}

#[derive(serde::Serialize)]
pub struct PriorityResult {
    pub success: bool,
    pub message: String,
    pub process_name: String,
    pub was_running: bool,
}

#[tauri::command]
pub async fn boost_delta_force_affinity() -> Result<PriorityResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let ps_script = r#"
        $proc = Get-Process -Name "DeltaForceClient-Win64-Shipping" -ErrorAction SilentlyContinue
        if ($proc) {
            $numCores = [Environment]::ProcessorCount
            $allCores = [Math]::Pow(2, $numCores) - 1
            $affinity = $allCores -bxor 1
            $proc.ProcessorAffinity = $affinity
            Write-Host "AFFINITY_SET"
            exit 0
        } else {
            Write-Host "NOT_FOUND"
            exit 1
        }
    "#;

    let result = Command::new("powershell")
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            if stdout.contains("AFFINITY_SET") {
                Ok(PriorityResult {
                    success: true,
                    message: "三角洲进程已设置为使用所有处理器核心".to_string(),
                    process_name: "DeltaForceClient-Win64-Shipping.exe".to_string(),
                    was_running: true,
                })
            } else {
                Ok(PriorityResult {
                    success: false,
                    message: "三角洲游戏未运行，请先启动游戏".to_string(),
                    process_name: "DeltaForceClient-Win64-Shipping.exe".to_string(),
                    was_running: false,
                })
            }
        }
        Err(e) => Err(format!("设置三角洲进程核心分配失败: {}", e)),
    }
}

#[derive(serde::Serialize)]
pub struct AcePartialResult {
    pub success: bool,
    pub message: String,
    pub count: u32,
}

#[tauri::command]
pub async fn limit_ace_priority() -> Result<AcePartialResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let ps_script = r#"
        $count = 0
        $processNames = @("ACE-Tray", "SGuard64", "SGuardSvc64")
        foreach ($name in $processNames) {
            $processes = Get-Process -Name $name -ErrorAction SilentlyContinue
            if ($processes) {
                foreach ($proc in $processes) {
                    try {
                        $proc.PriorityClass = [System.Diagnostics.ProcessPriorityClass]::Low
                        $count++
                    } catch {}
                }
            }
        }
        Write-Host "LIMITED:$count"
        if ($count -gt 0) { exit 0 } else { exit 1 }
    "#;

    let result = Command::new("powershell")
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let count: u32 = stdout.lines()
                .find_map(|l| l.strip_prefix("LIMITED:"))
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            Ok(AcePartialResult {
                success: count > 0,
                message: if count > 0 { format!("已限制 {} 个 ACE 进程优先级", count) } else { "未找到运行中的 ACE 进程".to_string() },
                count,
            })
        }
        Err(e) => Err(format!("限制 ACE 进程优先级失败: {}", e)),
    }
}

#[tauri::command]
pub async fn restrict_ace_affinity() -> Result<AcePartialResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let ps_script = r#"
        $count = 0
        $processNames = @("ACE-Tray", "SGuard64", "SGuardSvc64")
        foreach ($name in $processNames) {
            $processes = Get-Process -Name $name -ErrorAction SilentlyContinue
            if ($processes) {
                foreach ($proc in $processes) {
                    try {
                        $proc.ProcessorAffinity = 1
                        $count++
                    } catch {}
                }
            }
        }
        Write-Host "RESTRICTED:$count"
        if ($count -gt 0) { exit 0 } else { exit 1 }
    "#;

    let result = Command::new("powershell")
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let count: u32 = stdout.lines()
                .find_map(|l| l.strip_prefix("RESTRICTED:"))
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            Ok(AcePartialResult {
                success: count > 0,
                message: if count > 0 { format!("已限制 {} 个 ACE 进程使用单核心", count) } else { "未找到运行中的 ACE 进程".to_string() },
                count,
            })
        }
        Err(e) => Err(format!("限制 ACE 进程核心分配失败: {}", e)),
    }
}

#[derive(serde::Serialize)]
pub struct AllGameOptimizeResult {
    pub success: bool,
    pub message: String,
    pub delta_boosted: bool,
    pub ace_limited: bool,
    pub ace_count: u32,
}

#[tauri::command]
pub async fn optimize_all_game_processes() -> Result<AllGameOptimizeResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let mut delta_boosted = false;
    let mut ace_limited = false;
    let mut ace_count: u32 = 0;

    let ps_script = r#"
        $results = @{}

        $delta = Get-Process -Name "DeltaForceClient-Win64-Shipping" -ErrorAction SilentlyContinue
        if ($delta) {
            $delta.PriorityClass = [System.Diagnostics.ProcessPriorityClass]::RealTime
            $results["Delta"] = "BOOSTED"
        } else {
            $results["Delta"] = "NOT_FOUND"
        }

        $aceProcesses = @("ACE-Tray", "SGuard64", "SGuardSvc64")
        $aceDone = 0
        foreach ($name in $aceProcesses) {
            $proc = Get-Process -Name $name -ErrorAction SilentlyContinue
            if ($proc) {
                $proc.PriorityClass = [System.Diagnostics.ProcessPriorityClass]::Idle
                $proc.ProcessorAffinity = [IntPtr]1
                $aceDone++
            }
        }
        $results["Ace"] = $aceDone

        $results.GetEnumerator() | ForEach-Object { Write-Host "$($_.Key):$($_.Value)" }
    "#;

    let result = Command::new("powershell")
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            for line in stdout.lines() {
                if line.starts_with("Delta:BOOSTED") {
                    delta_boosted = true;
                }
                if line.starts_with("Ace:") {
                    ace_count = line.trim_start_matches("Ace:").trim().parse().unwrap_or(0);
                    ace_limited = ace_count > 0;
                }
            }

            let mut msgs: Vec<String> = Vec::new();
            if delta_boosted {
                msgs.push("三角洲: 已优化".to_string());
            } else {
                msgs.push("三角洲: 未运行".to_string());
            }
            if ace_limited {
                msgs.push(format!("ACE: 已限制 {} 个进程", ace_count));
            } else {
                msgs.push("ACE: 未运行".to_string());
            }

            Ok(AllGameOptimizeResult {
                success: delta_boosted || ace_limited,
                message: msgs.join(" | "),
                delta_boosted,
                ace_limited,
                ace_count,
            })
        }
        Err(e) => Err(format!("全部游戏优化失败: {}", e)),
    }
}

#[derive(serde::Serialize, Clone)]
pub struct BuiltinPowerPlan {
    pub id: String,
    pub filename: String,
    pub name: String,
    pub description: String,
    pub is_imported: bool,
    pub guid: Option<String>,
    pub is_active: bool,
}

#[derive(serde::Serialize)]
pub struct SystemPowerPlan {
    pub guid: String,
    pub name: String,
    pub is_active: bool,
}

#[derive(serde::Serialize)]
pub struct ActivePowerPlan {
    pub guid: String,
    pub name: String,
}

#[derive(serde::Serialize)]
pub struct PowerPlanOperationResult {
    pub success: bool,
    pub message: String,
    pub guid: Option<String>,
}

fn get_builtin_plan_filename(id: &str) -> String {
    match id {
        "ggOSDesktopGaming" => "ggOS Desktop Gaming.pow".to_string(),
        _ => format!("{}.pow", id),
    }
}

fn get_builtin_plan_metadata(id: &str) -> (String, String) {
    match id {
        "ACMEPCAMD" => ("ACMEPCAMD".to_string(), "AMD平台极致性能优化，最大化CPU/GPU频率与响应".to_string()),
        "AMD电源计划" => ("AMD电源计划".to_string(), "AMD官方推荐高性能电源方案，适合Ryzen平台".to_string()),
        "ggOSDesktopGaming" => ("ggOS Desktop Gaming".to_string(), "桌面游戏场景深度优化，降低延迟提升帧率".to_string()),
        "Intel大核心电源计划" => ("Intel大核心电源计划".to_string(), "Intel大小核调度优化，优先使用大核心运行游戏".to_string()),
        "amd" => ("AMD （社区推荐）".to_string(), "AMD平台通用高性能电源方案".to_string()),
        "intel" => ("Intel（社区推荐）".to_string(), "Intel平台通用高性能电源方案".to_string()),
        _ => (id.to_string(), String::new()),
    }
}

fn extract_guid_from_line(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    for part in parts {
        let segs: Vec<&str> = part.split('-').collect();
        if segs.len() == 5
            && segs[0].len() == 8
            && segs[1].len() == 4
            && segs[2].len() == 4
            && segs[3].len() == 4
            && segs[4].len() == 12
            && segs.iter().all(|s| s.chars().all(|c| c.is_ascii_hexdigit()))
        {
            return Some(part.to_string());
        }
    }
    None
}

fn parse_powercfg_list(output: &str) -> Vec<(String, String, bool)> {
    let mut plans = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(guid) = extract_guid_from_line(trimmed) {
            let is_active = trimmed.contains('*');
            let after_guid = trimmed.find(&guid).map(|pos| &trimmed[pos + guid.len()..]).unwrap_or("");
            let name = after_guid
                .trim()
                .trim_start_matches('(')
                .trim_end_matches(')')
                .trim()
                .trim_end_matches('*')
                .trim()
                .to_string();
            plans.push((guid, name, is_active));
        }
    }
    plans
}

fn run_powercfg_ps(script: &str) -> std::io::Result<std::process::Output> {
    let full_script = format!(
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; {}",
        script
    );
    Command::new("powershell")
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &full_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
}

fn get_system_plans_internal() -> Vec<(String, String, bool)> {
    let result = run_powercfg_ps("powercfg /list");

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            parse_powercfg_list(&stdout)
        }
        Err(_) => Vec::new(),
    }
}

fn get_active_plan_internal() -> Option<(String, String)> {
    let result = run_powercfg_ps("powercfg /getactivescheme");

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let trimmed = stdout.trim();
            if let Some(guid) = extract_guid_from_line(trimmed) {
                let after_guid = trimmed.find(&guid).map(|pos| &trimmed[pos + guid.len()..]).unwrap_or("");
                let name = after_guid
                    .trim()
                    .trim_start_matches('(')
                    .trim_end_matches(')')
                    .trim()
                    .to_string();
                Some((guid, name))
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

fn find_plan_guid_by_name(system_plans: &[(String, String, bool)], plan_name: &str) -> Option<String> {
    for (guid, name, _) in system_plans {
        if name.contains(plan_name) {
            return Some(guid.clone());
        }
    }
    None
}

fn resolve_power_plans_dir(app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let candidates = [
            resource_dir.join("power-plans"),
            resource_dir.join("_up_").join("power-plans"),
        ];
        for path in &candidates {
            if path.exists() {
                return Some(path.clone());
            }
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let candidates = [
                parent.join("power-plans"),
                parent.join("_up_").join("power-plans"),
            ];
            for path in &candidates {
                if path.exists() {
                    return Some(path.clone());
                }
            }
        }
    }

    None
}

#[tauri::command]
pub async fn get_builtin_power_plans(app: tauri::AppHandle) -> Result<Vec<BuiltinPowerPlan>, String> {
    let power_plans_dir = resolve_power_plans_dir(&app)
        .ok_or("未找到电源计划文件目录，请确保 power-plans 文件夹存在")?;

    let system_plans = get_system_plans_internal();
    let active_plan = get_active_plan_internal();
    let active_guid = active_plan.as_ref().map(|(g, _)| g.as_str()).unwrap_or("");

    let builtin_ids = ["ACMEPCAMD", "AMD电源计划", "ggOSDesktopGaming", "Intel大核心电源计划", "amd", "intel"];

    let mut plans = Vec::new();

    for id in builtin_ids {
        let (display_name, description) = get_builtin_plan_metadata(id);
        let filename = get_builtin_plan_filename(id);
        let file_path = power_plans_dir.join(&filename);
        let file_exists = file_path.exists();

        let (is_imported, guid, is_active) = if file_exists {
            let matched_guid = find_plan_guid_by_name(&system_plans, &display_name);
            let active = matched_guid.as_ref().map(|g| g == active_guid).unwrap_or(false);
            (matched_guid.is_some(), matched_guid, active)
        } else {
            (false, None, false)
        };

        plans.push(BuiltinPowerPlan {
            id: id.to_string(),
            filename,
            name: display_name.to_string(),
            description: description.to_string(),
            is_imported,
            guid,
            is_active,
        });
    }

    Ok(plans)
}

#[tauri::command]
pub async fn get_system_power_plans() -> Result<Vec<SystemPowerPlan>, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let plans = get_system_plans_internal();
    Ok(plans.into_iter().map(|(guid, name, is_active)| SystemPowerPlan { guid, name, is_active }).collect())
}

#[tauri::command]
pub async fn get_active_power_plan() -> Result<ActivePowerPlan, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    match get_active_plan_internal() {
        Some((guid, name)) => Ok(ActivePowerPlan { guid, name }),
        None => Err("获取当前电源计划失败".to_string()),
    }
}

#[tauri::command]
pub async fn import_power_plan(app: tauri::AppHandle, plan_id: String) -> Result<PowerPlanOperationResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let (display_name, _) = get_builtin_plan_metadata(&plan_id);

    let system_plans_before = get_system_plans_internal();
    let guids_before: Vec<String> = system_plans_before.iter().map(|(g, _, _)| g.clone()).collect();

    if let Some(existing_guid) = find_plan_guid_by_name(&system_plans_before, &display_name) {
        return Ok(PowerPlanOperationResult {
            success: true,
            message: format!("电源计划 '{}' 已存在于系统中", display_name),
            guid: Some(existing_guid),
        });
    }

    let power_plans_dir = resolve_power_plans_dir(&app)
        .ok_or("未找到电源计划文件目录")?;
    let file_path = power_plans_dir.join(get_builtin_plan_filename(&plan_id));

    if !file_path.exists() {
        return Err(format!("电源计划文件不存在: {}", plan_id));
    }

    let file_path_str = file_path.to_string_lossy().to_string();
    let result = Command::new("powercfg")
        .args(["/import", &file_path_str])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match result {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let err_msg = if !stderr.trim().is_empty() { stderr.trim().to_string() } else if !stdout.trim().is_empty() { stdout.trim().to_string() } else { "未知错误".to_string() };
                return Err(format!("导入电源计划失败: {}", err_msg));
            }

            std::thread::sleep(std::time::Duration::from_millis(800));

            let system_plans_after = get_system_plans_internal();
            
            let mut new_guid: Option<String> = None;
            for (guid, _, _) in &system_plans_after {
                if !guids_before.contains(guid) {
                    new_guid = Some(guid.clone());
                    break;
                }
            }

            if let Some(guid) = new_guid {
                Ok(PowerPlanOperationResult {
                    success: true,
                    message: format!("电源计划 '{}' 导入成功", display_name),
                    guid: Some(guid),
                })
            } else if let Some(guid) = find_plan_guid_by_name(&system_plans_after, &display_name) {
                Ok(PowerPlanOperationResult {
                    success: true,
                    message: format!("电源计划 '{}' 导入成功", display_name),
                    guid: Some(guid),
                })
            } else {
                Err(format!("电源计划 '{}' 导入后未在系统中找到，可能导入失败", display_name))
            }
        }
        Err(e) => Err(format!("执行导入命令失败: {}", e)),
    }
}

#[tauri::command]
pub async fn activate_power_plan(guid: String) -> Result<PowerPlanOperationResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let result = Command::new("powercfg")
        .args(["/setactive", &guid])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match result {
        Ok(output) => {
            if output.status.success() {
                std::thread::sleep(std::time::Duration::from_millis(500));
                let verify = get_active_plan_internal();
                match verify {
                    Some((active_guid, active_name)) => {
                        if active_guid == guid {
                            Ok(PowerPlanOperationResult {
                                success: true,
                                message: format!("电源计划 '{}' 已激活", active_name),
                                guid: Some(guid),
                            })
                        } else {
                            Ok(PowerPlanOperationResult {
                                success: true,
                                message: "激活命令已执行，请确认是否生效".to_string(),
                                guid: Some(guid),
                            })
                        }
                    }
                    None => Ok(PowerPlanOperationResult {
                        success: true,
                        message: "激活命令已执行".to_string(),
                        guid: Some(guid),
                    }),
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let err_msg = if !stderr.trim().is_empty() { stderr.trim().to_string() } else if !stdout.trim().is_empty() { stdout.trim().to_string() } else { "未知错误".to_string() };
                Err(format!("激活电源计划失败: {}", err_msg))
            }
        }
        Err(e) => Err(format!("执行激活命令失败: {}", e)),
    }
}

#[tauri::command]
pub async fn import_and_activate_power_plan(app: tauri::AppHandle, plan_id: String) -> Result<PowerPlanOperationResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let (display_name, _) = get_builtin_plan_metadata(&plan_id);

    let system_plans_before = get_system_plans_internal();
    let guids_before: Vec<String> = system_plans_before.iter().map(|(g, _, _)| g.clone()).collect();
    let existing_guid = find_plan_guid_by_name(&system_plans_before, &display_name);

    let (guid, was_existing) = match existing_guid {
        Some(g) => (g, true),
        None => {
            let power_plans_dir = resolve_power_plans_dir(&app)
                .ok_or("未找到电源计划文件目录")?;
            let file_path = power_plans_dir.join(get_builtin_plan_filename(&plan_id));

            if !file_path.exists() {
                return Err(format!("电源计划文件不存在: {}", plan_id));
            }

            let file_path_str = file_path.to_string_lossy().to_string();
            let import_result = Command::new("powercfg")
                .args(["/import", &file_path_str])
                .creation_flags(CREATE_NO_WINDOW)
                .output();

            let g = match import_result {
                Ok(output) => {
                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        let err_msg = if !stderr.trim().is_empty() { stderr.trim().to_string() } else if !stdout.trim().is_empty() { stdout.trim().to_string() } else { "未知错误".to_string() };
                        return Err(format!("导入失败: {}", err_msg));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(800));
                    let system_plans_after = get_system_plans_internal();
                    
                    let mut new_guid: Option<String> = None;
                    for (guid, _, _) in &system_plans_after {
                        if !guids_before.contains(guid) {
                            new_guid = Some(guid.clone());
                            break;
                        }
                    }

                    if let Some(g) = new_guid {
                        g
                    } else if let Some(g) = find_plan_guid_by_name(&system_plans_after, &display_name) {
                        g
                    } else {
                        return Err(format!("电源计划 '{}' 导入后未在系统中找到，可能导入失败", display_name));
                    }
                }
                Err(e) => return Err(format!("导入失败: {}", e)),
            };
            (g, false)
        }
    };

    let activate_result = activate_power_plan(guid.clone()).await?;
    Ok(PowerPlanOperationResult {
        success: true,
        message: if was_existing {
            format!("电源计划 '{}' 已存在，{}", display_name, activate_result.message)
        } else {
            format!("电源计划 '{}' 导入并激活成功", display_name)
        },
        guid: Some(guid),
    })
}

#[derive(serde::Serialize)]
pub struct PerfTweakResult {
    pub success: bool,
    pub message: String,
}
// ========== AQ_REGISTRY 模块 - 纯 Rust 注册表操作 ==========

/// 读取 .reg 文件内容（自动处理 UTF-8 和 UTF-16LE 编码）
fn read_reg_file(name: &str, is_restore: bool) -> Result<String, String> {
    let path = resolve_reg_path(name, is_restore)?;
    let bytes = fs::read(&path)
        .map_err(|e| format!("读取注册表文件失败: {}", e))?;
    
    // 检测编码：UTF-16LE BOM (FF FE) 或 UTF-8 BOM (EF BB BF)
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        // UTF-16LE 编码（部分 .reg 文件使用此编码）
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        String::from_utf16_lossy(&u16s)
    } else if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        // UTF-8 BOM
        String::from_utf8_lossy(&bytes[3..]).to_string()
    } else {
        // 纯 UTF-8
        String::from_utf8_lossy(&bytes).to_string()
    }
}

/// 解析 .reg 文件路径
fn resolve_reg_path(name: &str, is_restore: bool) -> Result<PathBuf, String> {
    let exe_dir = std::env::current_exe()
        .map_err(|e| format!("获取程序路径失败: {}", e))?;
    let parent = exe_dir.parent().ok_or("无法获取父目录")?;

    let (dir, suffix) = if is_restore {
        ("aq_registry_restore", ".restore.reg")
    } else {
        ("aq_registry", ".reg")
    };

    // 尝试多个候选路径（适配不同打包模式）
    let candidates = [
        parent.join(dir).join(format!("{}{}", name, suffix)),
        parent.join("_up_").join(dir).join(format!("{}{}", name, suffix)),
        parent.join("resources").join(dir).join(format!("{}{}", name, suffix)),
    ];

    for path in &candidates {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    Err(format!("未找到注册表文件: {}{}", name, suffix))
}

/// 解析 .reg 文件内容并直接通过 winreg 写入注册表
fn apply_reg_content(content: &str) -> Result<(), String> {
    let mut current_key: Option<RegKey> = None;

    for line in content.lines() {
        let line = line.trim();

        // 跳过空行和注释
        if line.is_empty() || line.starts_with(';') || line.starts_with("Windows Registry Editor") {
            continue;
        }

        // [HKEY_LOCAL_MACHINE\SYSTEM\...] — 注册表键路径
        if line.starts_with('[') && line.ends_with(']') {
            let path = &line[1..line.len() - 1];
            current_key = Some(open_or_create_reg_key(path)?);
            continue;
        }

        // "ValueName"=dword:00000001 — DWORD 值
        // "ValueName"="string"        — 字符串值
        // "ValueName"=-                — 删除值
        if let Some(ref key) = current_key {
            if let Some(rest) = line.strip_prefix('"') {
                if let Some(eq_pos) = rest.find("\"=") {
                    let name = &rest[..eq_pos];
                    // 反转义 .reg 中的双引号 \"
                    let name = name.replace("\\\"", "\"");
                    let value = &rest[eq_pos + 2..];

                    if value.starts_with("dword:") {
                        // DWORD 值
                        let hex_str = &value[6..];
                        let val = u32::from_str_radix(hex_str, 16)
                            .map_err(|e| format!("解析 dword 值失败: {}", e))?;
                        key.set_value(&name, &val)
                            .map_err(|e| format!("写入注册表值失败: {}", e))?;
                    } else if value.starts_with('"') {
                        // 字符串值（去掉首尾引号，反转义）
                        let val = &value[1..value.len() - 1];
                        let val = val.replace("\\\"", "\"");
                        key.set_value(&name, &val)
                            .map_err(|e| format!("写入注册表值失败: {}", e))?;
                    } else if value == "-" {
                        // 删除值
                        let _ = key.delete_value(&name);
                    }
                    // hex: 格式（二进制值）暂不支持，aq_registry 中未使用
                }
            }
        }
    }

    Ok(())
}

/// 根据 .reg 文件中的路径打开或创建注册表键
fn open_or_create_reg_key(path: &str) -> Result<RegKey, String> {
    // 分割 root hive 和子路径
    let (root, subpath) = if let Some(sub) = path.strip_prefix("HKEY_LOCAL_MACHINE\\") {
        (RegKey::predef(HKEY_LOCAL_MACHINE), sub)
    } else if let Some(sub) = path.strip_prefix("HKEY_CURRENT_USER\\") {
        (RegKey::predef(HKEY_CURRENT_USER), sub)
    } else if let Some(sub) = path.strip_prefix("HKEY_CLASSES_ROOT\\") {
        (RegKey::predef(HKEY_CLASSES_ROOT), sub)
    } else if let Some(sub) = path.strip_prefix("HKEY_USERS\\") {
        (RegKey::predef(HKEY_USERS), sub)
    } else {
        return Err(format!("不支持的注册表根键: {}", path));
    };

    // create_subkey 会自动创建所有不存在的中间键
    let (key, _) = root
        .create_subkey(subpath)
        .map_err(|e| format!("创建注册表键失败: {} - {}", path, e))?;

    Ok(key)
}

/// 应用单个注册表优化
#[tauri::command]
pub async fn apply_registry_tweak(name: String) -> Result<PerfTweakResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let content = read_reg_file(&name, false)?;
    apply_reg_content(&content)?;

    Ok(PerfTweakResult {
        success: true,
        message: "优化已应用".to_string(),
    })
}

/// 恢复单个注册表优化
#[tauri::command]
pub async fn restore_registry_tweak(name: String) -> Result<PerfTweakResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let content = read_reg_file(&name, true)?;
    apply_reg_content(&content)?;

    Ok(PerfTweakResult {
        success: true,
        message: "优化已恢复".to_string(),
    })
}

/// 批量应用所有优化
#[tauri::command]
pub async fn batch_apply_registry_tweaks(names: Vec<String>) -> Result<PerfTweakResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let mut success_count = 0;
    let mut failed = Vec::new();

    for name in &names {
        match read_reg_file(name, false) {
            Ok(content) => {
                if let Err(e) = apply_reg_content(&content) {
                    failed.push((name.clone(), e));
                } else {
                    success_count += 1;
                }
            }
            Err(e) => {
                failed.push((name.clone(), e));
            }
        }
    }

    if failed.is_empty() {
        Ok(PerfTweakResult {
            success: true,
            message: format!("成功应用 {} 项优化", success_count),
        })
    } else {
        Ok(PerfTweakResult {
            success: false,
            message: format!("成功应用 {} 项优化，失败 {} 项: {:?}",
                success_count, failed.len(), failed.iter().map(|(n, e)| format!("{}: {}", n, e)).collect::<Vec<_>>()),
        })
    }
}

/// 批量恢复所有优化
#[tauri::command]
pub async fn batch_restore_registry_tweaks(names: Vec<String>) -> Result<PerfTweakResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let mut success_count = 0;
    let mut failed = Vec::new();

    for name in &names {
        match read_reg_file(name, true) {
            Ok(content) => {
                if let Err(e) = apply_reg_content(&content) {
                    failed.push((name.clone(), e));
                } else {
                    success_count += 1;
                }
            }
            Err(e) => {
                failed.push((name.clone(), e));
            }
        }
    }

    if failed.is_empty() {
        Ok(PerfTweakResult {
            success: true,
            message: format!("成功恢复 {} 项优化", success_count),
        })
    } else {
        Ok(PerfTweakResult {
            success: false,
            message: format!("成功恢复 {} 项优化，失败 {} 项: {:?}",
                success_count, failed.len(), failed.iter().map(|(n, e)| format!("{}: {}", n, e)).collect::<Vec<_>>()),
        })
    }
}

// ========== AQ_REGISTRY 模块结束 ==========

#[tauri::command]
pub async fn delete_power_plan(guid: String) -> Result<PowerPlanOperationResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let active_plan = get_active_plan_internal();
    if let Some((active_guid, _)) = active_plan {
        if active_guid == guid {
            return Err("无法删除当前激活的电源计划，请先切换到其他计划".to_string());
        }
    }

    let result = Command::new("powercfg")
        .args(["/delete", &guid])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match result {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let err_msg = if !stderr.trim().is_empty() { stderr.trim().to_string() } else if !stdout.trim().is_empty() { stdout.trim().to_string() } else { "未知错误".to_string() };
                return Err(format!("删除电源计划失败: {}", err_msg));
            }

            std::thread::sleep(std::time::Duration::from_millis(500));

            let system_plans = get_system_plans_internal();
            let still_exists = system_plans.iter().any(|(g, _, _)| g == &guid);

            if still_exists {
                Err("电源计划删除可能未生效，请确认是否具有管理员权限".to_string())
            } else {
                Ok(PowerPlanOperationResult {
                    success: true,
                    message: "电源计划已删除".to_string(),
                    guid: None,
                })
            }
        }
        Err(e) => Err(format!("执行删除命令失败: {}", e)),
    }
}
