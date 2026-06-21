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

#[tauri::command]
pub async fn enable_performance_tweaks() -> Result<PerfTweakResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let script = r#"
$ErrorActionPreference = 'SilentlyContinue'

# HKCU Desktop tweaks
Set-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'AutoEndTasks' -Value '1' -Type String -Force
Set-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'HungAppTimeout' -Value '1000' -Type String -Force
Set-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'WaitToKillAppTimeout' -Value '2000' -Type String -Force
Set-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'LowLevelHooksTimeout' -Value '1000' -Type String -Force

# HKCU Explorer tweaks
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced' -Name 'DisallowShaking' -Value 1 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced' -Name 'HideFileExt' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced' -Name 'Hidden' -Value 1 -Type DWord -Force

# HKCU Policies
$p = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer'
New-Item -Path $p -Force | Out-Null
Set-ItemProperty -Path $p -Name 'NoLowDiskSpaceChecks' -Value 1 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'LinkResolveIgnoreLinkInfo' -Value 1 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'NoResolveSearch' -Value 1 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'NoResolveTrack' -Value 1 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'NoInternetOpenWith' -Value 1 -Type DWord -Force

# HKLM Crash & Remote Assistance
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\CrashControl' -Name 'CrashDumpEnabled' -Value 3 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Remote Assistance' -Name 'fAllowToGetHelp' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control' -Name 'WaitToKillServiceTimeout' -Value '2000' -Type String -Force -ErrorAction SilentlyContinue

# Multimedia / Game performance
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' -Name 'SystemResponsiveness' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' -Name 'NoLazyMode' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' -Name 'AlwaysOn' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue

# Game GPU priority
$games = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games'
New-Item -Path $games -Force | Out-Null
Set-ItemProperty -Path $games -Name 'GPU Priority' -Value 8 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path $games -Name 'Priority' -Value 6 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path $games -Name 'Scheduling Category' -Value 'High' -Type String -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path $games -Name 'SFIO Priority' -Value 'High' -Type String -Force -ErrorAction SilentlyContinue

# Low latency audio
$ll = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Low Latency'
New-Item -Path $ll -Force | Out-Null
Set-ItemProperty -Path $ll -Name 'GPU Priority' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path $ll -Name 'Priority' -Value 8 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path $ll -Name 'Scheduling Category' -Value 'Medium' -Type String -Force -ErrorAction SilentlyContinue

# Disable Windows Media Foundation frame server
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows Media Foundation' -Name 'EnableFrameServerMode' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue

# Services
$svcs = @(
    @{Name='DiagTrack'; Startup='Disabled'},
    @{Name='diagnosticshub.standardcollector.service'; Startup='Disabled'},
    @{Name='dmwappushservice'; Startup='Disabled'},
    @{Name='RemoteRegistry'; Startup='Disabled'}
)
foreach ($svc in $svcs) {
    Set-Service -Name $svc.Name -StartupType $svc.Startup -ErrorAction SilentlyContinue
    Stop-Service -Name $svc.Name -Force -ErrorAction SilentlyContinue
}

Write-Output 'OK'
"#;

    let ps_path = get_powershell_path();
    let result = Command::new(&ps_path)
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行命令失败: {}", e))?;

    if result.status.success() {
        Ok(PerfTweakResult {
            success: true,
            message: "性能调整已启用，建议重启系统以完全生效".to_string(),
        })
    } else {
        let stderr = String::from_utf8_lossy(&result.stderr).to_string();
        let stdout = String::from_utf8_lossy(&result.stdout).to_string();
        let err_msg = if !stderr.trim().is_empty() { stderr.trim().to_string() } else { stdout.trim().to_string() };
        let lower = err_msg.to_lowercase();
        if lower.contains("access denied") || lower.contains("denied") || lower.contains("拒绝访问") || lower.contains("权限不足") {
            Err("需要管理员权限，请以管理员身份运行 NexBox".to_string())
        } else {
            Err(format!("操作失败: {}", err_msg))
        }
    }
}

#[tauri::command]
pub async fn disable_performance_tweaks() -> Result<PerfTweakResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let script = r#"
$ErrorActionPreference = 'SilentlyContinue'

# Restore HKCU Desktop
Remove-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'AutoEndTasks' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'HungAppTimeout' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'WaitToKillAppTimeout' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'LowLevelHooksTimeout' -ErrorAction SilentlyContinue

# Restore Explorer
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced' -Name 'DisallowShaking' -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced' -Name 'HideFileExt' -Value 1 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced' -Name 'Hidden' -Value 2 -Type DWord -Force

# Remove policies
$p = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer'
Remove-ItemProperty -Path $p -Name 'NoLowDiskSpaceChecks' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'LinkResolveIgnoreLinkInfo' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'NoResolveSearch' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'NoResolveTrack' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'NoInternetOpenWith' -ErrorAction SilentlyContinue

# Restore HKLM
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\CrashControl' -Name 'CrashDumpEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Remote Assistance' -Name 'fAllowToGetHelp' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control' -Name 'WaitToKillServiceTimeout' -ErrorAction SilentlyContinue

# Restore multimedia defaults
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' -Name 'SystemResponsiveness' -Value 10 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' -Name 'NoLazyMode' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' -Name 'AlwaysOn' -ErrorAction SilentlyContinue

# Remove game task values
$games = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games'
Remove-ItemProperty -Path $games -Name 'GPU Priority' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $games -Name 'Priority' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $games -Name 'Scheduling Category' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $games -Name 'SFIO Priority' -ErrorAction SilentlyContinue

# Remove low latency values
$ll = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Low Latency'
Remove-ItemProperty -Path $ll -Name 'GPU Priority' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $ll -Name 'Priority' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $ll -Name 'Scheduling Category' -ErrorAction SilentlyContinue

# Restore WMF
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows Media Foundation' -Name 'EnableFrameServerMode' -ErrorAction SilentlyContinue

# Restore services
$svcs = @(
    @{Name='DiagTrack'; Startup='Manual'},
    @{Name='diagnosticshub.standardcollector.service'; Startup='Manual'},
    @{Name='dmwappushservice'; Startup='Manual'},
    @{Name='RemoteRegistry'; Startup='Manual'}
)
foreach ($svc in $svcs) {
    Set-Service -Name $svc.Name -StartupType $svc.Startup -ErrorAction SilentlyContinue
}

Write-Output 'OK'
"#;

    let ps_path = get_powershell_path();
    let result = Command::new(&ps_path)
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行命令失败: {}", e))?;

    if result.status.success() {
        Ok(PerfTweakResult {
            success: true,
            message: "性能调整已还原".to_string(),
        })
    } else {
        let stderr = String::from_utf8_lossy(&result.stderr).to_string();
        let stdout = String::from_utf8_lossy(&result.stdout).to_string();
        let err_msg = if !stderr.trim().is_empty() { stderr.trim().to_string() } else { stdout.trim().to_string() };
        let lower = err_msg.to_lowercase();
        if lower.contains("access denied") || lower.contains("denied") || lower.contains("拒绝访问") || lower.contains("权限不足") {
            Err("需要管理员权限，请以管理员身份运行 NexBox".to_string())
        } else {
            Err(format!("操作失败: {}", err_msg))
        }
    }
}

pub(crate) fn run_simple_feature(script: &str) -> Result<PerfTweakResult, String> {
    let result = run_ps_script(script)?;
    if result.status.success() {
        Ok(PerfTweakResult { success: true, message: "操作成功".to_string() })
    } else {
        let stderr = String::from_utf8_lossy(&result.stderr).to_string();
        let stdout = String::from_utf8_lossy(&result.stdout).to_string();
        let err_msg = if !stderr.trim().is_empty() { stderr.trim().to_string() } else { stdout.trim().to_string() };
        let lower = err_msg.to_lowercase();
        if lower.contains("access denied") || lower.contains("denied") || lower.contains("拒绝访问") || lower.contains("权限不足") {
            Err("需要管理员权限，请以管理员身份运行 NexBox".to_string())
        } else {
            Err(format!("操作失败: {}", err_msg))
        }
    }
}

/// Write a PowerShell script to a temp .ps1 file and execute it with -File.
/// This avoids Windows command line length limits (os error 206).
fn run_ps_script(script: &str) -> Result<std::process::Output, String> {
    let ps_path = get_powershell_path();
    let tmp_dir = std::env::temp_dir();
    let script_path = tmp_dir.join(format!("nexbox_{}.ps1", std::process::id()));
    fs::write(&script_path, script).map_err(|e| format!("写入临时脚本失败: {}", e))?;
    let result = Command::new(&ps_path)
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", &script_path.to_string_lossy()])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行命令失败: {}", e));
    let _ = fs::remove_file(&script_path);
    result
}

// === Individual Performance Features ===

#[tauri::command]
pub async fn remove_menu_delay() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
Set-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'MenuShowDelay' -Value '0' -Type String -Force
Set-ItemProperty -Path 'HKCU:\Control Panel\Mouse' -Name 'MouseHoverTime' -Value '0' -Type String -Force
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn restore_menu_delay() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
Set-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'MenuShowDelay' -Value '400' -Type String -Force
Set-ItemProperty -Path 'HKCU:\Control Panel\Mouse' -Name 'MouseHoverTime' -Value '400' -Type String -Force
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_network_throttling() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
$v = [convert]::ToInt32('ffffffff', 16)
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' -Name 'NetworkThrottlingIndex' -Value $v -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Psched' -Name 'NonBestEffortLimit' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_network_throttling() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Psched' -Name 'NonBestEffortLimit' -Value 80 -Type DWord -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' -Name 'NetworkThrottlingIndex' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_error_reporting() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Error Reporting' -Name 'Disabled' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\PCHealth\ErrorReporting' -Name 'DoReport' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\Windows Error Reporting' -Name 'Disabled' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-Service -Name 'WerSvc' -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name 'WerSvc' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'wercplsupport' -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name 'wercplsupport' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_error_reporting() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Error Reporting' -Name 'Disabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\PCHealth\ErrorReporting' -Name 'DoReport' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\Windows Error Reporting' -Name 'Disabled' -ErrorAction SilentlyContinue
Set-Service -Name 'WerSvc' -StartupType Automatic -ErrorAction SilentlyContinue
Set-Service -Name 'wercplsupport' -StartupType Manual -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_compatibility_assistant() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Stop-Service -Name 'PcaSvc' -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\PcaSvc' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_compatibility_assistant() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\PcaSvc' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Start-Service -Name 'PcaSvc' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_print_service() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Stop-Service -Name 'Spooler' -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\Spooler' -Name 'Start' -Value 3 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_print_service() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\Spooler' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Start-Service -Name 'Spooler' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_fax_service() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Stop-Service -Name 'Fax' -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\Fax' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_fax_service() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\Fax' -Name 'Start' -Value 3 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_sticky_keys() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKCU:\Control Panel\Accessibility\StickyKeys' -Name 'Flags' -Value '506' -Type String -Force
Set-ItemProperty -Path 'HKCU:\Control Panel\Accessibility\Keyboard Response' -Name 'Flags' -Value '122' -Type String -Force
Set-ItemProperty -Path 'HKCU:\Control Panel\Accessibility\ToggleKeys' -Name 'Flags' -Value '58' -Type String -Force
# Also apply to default user profile
reg add "HKEY_USERS\.DEFAULT\Control Panel\Accessibility\StickyKeys" /v "Flags" /t REG_SZ /d "506" /f 2>nul
reg add "HKEY_USERS\.DEFAULT\Control Panel\Accessibility\Keyboard Response" /v "Flags" /t REG_SZ /d "122" /f 2>nul
reg add "HKEY_USERS\.DEFAULT\Control Panel\Accessibility\ToggleKeys" /v "Flags" /t REG_SZ /d "58" /f 2>nul
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_sticky_keys() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKCU:\Control Panel\Accessibility\StickyKeys' -Name 'Flags' -Value '510' -Type String -Force
Set-ItemProperty -Path 'HKCU:\Control Panel\Accessibility\Keyboard Response' -Name 'Flags' -Value '126' -Type String -Force
Set-ItemProperty -Path 'HKCU:\Control Panel\Accessibility\ToggleKeys' -Name 'Flags' -Value '62' -Type String -Force
reg add "HKEY_USERS\.DEFAULT\Control Panel\Accessibility\StickyKeys" /v "Flags" /t REG_SZ /d "510" /f 2>nul
reg add "HKEY_USERS\.DEFAULT\Control Panel\Accessibility\Keyboard Response" /v "Flags" /t REG_SZ /d "126" /f 2>nul
reg add "HKEY_USERS\.DEFAULT\Control Panel\Accessibility\ToggleKeys" /v "Flags" /t REG_SZ /d "62" /f 2>nul
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_smart_screen() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\Attachments' -Name 'SaveZoneInformation' -Value 1 -Type DWord -Force
Set-ItemProperty -Path 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Policies\Attachments' -Name 'ScanWithAntiVirus' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' -Name 'ShellSmartScreenLevel' -Value 'Warn' -Type String -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' -Name 'EnableSmartScreen' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer' -Name 'SmartScreenEnabled' -Value 'Off' -Type String -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Internet Explorer\PhishingFilter' -Name 'EnabledV9' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\AppHost' -Name 'PreventOverride' -Value 0 -Type DWord -Force
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_smart_screen() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\Attachments' -Name 'SaveZoneInformation' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Policies\Attachments' -Name 'ScanWithAntiVirus' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' -Name 'ShellSmartScreenLevel' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' -Name 'EnableSmartScreen' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer' -Name 'SmartScreenEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Internet Explorer\PhishingFilter' -Name 'EnabledV9' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\AppHost' -Name 'PreventOverride' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === 14 New Performance Features ===

#[tauri::command]
pub async fn disable_system_restore() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
vssadmin delete shadows /for=c: /all /quiet 2>nul
Stop-Service -Name 'VSS' -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows NT\SystemRestore' -Name 'DisableSR' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows NT\SystemRestore' -Name 'DisableConfig' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_system_restore() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows NT\SystemRestore' -Name 'DisableSR' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows NT\SystemRestore' -Name 'DisableConfig' -ErrorAction SilentlyContinue
Start-Service -Name 'VSS' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_superfetch() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Stop-Service -Name 'SysMain' -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\SysMain' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management\PrefetchParameters' -Name 'EnableSuperfetch' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management\PrefetchParameters' -Name 'EnablePrefetcher' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management\PrefetchParameters' -Name 'SfTracingState' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_superfetch() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\SysMain' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management\PrefetchParameters' -Name 'EnableSuperfetch' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management\PrefetchParameters' -Name 'EnablePrefetcher' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management\PrefetchParameters' -Name 'SfTracingState' -ErrorAction SilentlyContinue
Start-Service -Name 'SysMain' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_hibernate() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Power' -Name 'HibernateEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
powercfg -h off
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_hibernate() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Power' -Name 'HibernateEnabled' -ErrorAction SilentlyContinue
powercfg -h on
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_ntfs_timestamp() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
fsutil behavior set disablelastaccess 1
"#)
}

#[tauri::command]
pub async fn enable_ntfs_timestamp() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
fsutil behavior set disablelastaccess 2
"#)
}

#[tauri::command]
pub async fn disable_telemetry_tasks() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
$autoLogger = "$env:ProgramData\Microsoft\Diagnosis\ETLLogs\AutoLogger"
if (Test-Path $autoLogger) { icacls $autoLogger /deny SYSTEM:`(OI`)`(CI`)F 2>nul }
# Disable telemetry tasks
$tasks = @(
'\Microsoft\Windows\Customer Experience Improvement Program\Consolidator',
'\Microsoft\Windows\Customer Experience Improvement Program\BthSQM',
'\Microsoft\Windows\Customer Experience Improvement Program\KernelCeipTask',
'\Microsoft\Windows\Customer Experience Improvement Program\UsbCeip',
'\Microsoft\Windows\Customer Experience Improvement Program\Uploader',
'\Microsoft\Windows\Application Experience\Microsoft Compatibility Appraiser',
'\Microsoft\Windows\Application Experience\ProgramDataUpdater',
'\Microsoft\Windows\Application Experience\StartupAppTask',
'\Microsoft\Windows\DiskDiagnostic\Microsoft-Windows-DiskDiagnosticDataCollector',
'\Microsoft\Windows\DiskDiagnostic\Microsoft-Windows-DiskDiagnosticResolver',
'\Microsoft\Windows\Power Efficiency Diagnostics\AnalyzeSystem',
'\Microsoft\Windows\Shell\FamilySafetyMonitor',
'\Microsoft\Windows\Shell\FamilySafetyRefresh',
'\Microsoft\Windows\Shell\FamilySafetyUpload',
'\Microsoft\Windows\Autochk\Proxy',
'\Microsoft\Windows\Maintenance\WinSAT',
'\Microsoft\Windows\Application Experience\AitAgent',
'\Microsoft\Windows\Windows Error Reporting\QueueReporting',
'\Microsoft\Windows\CloudExperienceHost\CreateObjectTask',
'\Microsoft\Windows\DiskFootprint\Diagnostics',
'\Microsoft\Windows\FileHistory\File History (maintenance mode)',
'\Microsoft\Windows\PI\Sqm-Tasks',
'\Microsoft\Windows\NetTrace\GatherNetworkInfo',
'\Microsoft\Windows\AppID\SmartScreenSpecific',
'\Microsoft\Windows\HelloFace\FODCleanupTask',
'\Microsoft\Windows\Feedback\Siuf\DmClient',
'\Microsoft\Windows\Feedback\Siuf\DmClientOnScenarioDownload',
'\Microsoft\Windows\Application Experience\PcaPatchDbTask',
'\Microsoft\Windows\Device Information\Device',
'\Microsoft\Windows\Device Information\Device User'
)
foreach ($t in $tasks) { schtasks /end /tn "$t" /f 2>nul; schtasks /change /disable /tn "$t" 2>nul }
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_telemetry_tasks() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
$tasks = @(
'\Microsoft\Windows\Customer Experience Improvement Program\Consolidator',
'\Microsoft\Windows\Customer Experience Improvement Program\BthSQM',
'\Microsoft\Windows\Customer Experience Improvement Program\KernelCeipTask',
'\Microsoft\Windows\Customer Experience Improvement Program\UsbCeip',
'\Microsoft\Windows\Customer Experience Improvement Program\Uploader',
'\Microsoft\Windows\Application Experience\Microsoft Compatibility Appraiser',
'\Microsoft\Windows\Application Experience\ProgramDataUpdater',
'\Microsoft\Windows\Application Experience\StartupAppTask',
'\Microsoft\Windows\DiskDiagnostic\Microsoft-Windows-DiskDiagnosticDataCollector',
'\Microsoft\Windows\DiskDiagnostic\Microsoft-Windows-DiskDiagnosticResolver',
'\Microsoft\Windows\Power Efficiency Diagnostics\AnalyzeSystem',
'\Microsoft\Windows\Shell\FamilySafetyMonitor',
'\Microsoft\Windows\Shell\FamilySafetyRefresh',
'\Microsoft\Windows\Shell\FamilySafetyUpload',
'\Microsoft\Windows\Autochk\Proxy',
'\Microsoft\Windows\Maintenance\WinSAT',
'\Microsoft\Windows\Application Experience\AitAgent',
'\Microsoft\Windows\Windows Error Reporting\QueueReporting',
'\Microsoft\Windows\CloudExperienceHost\CreateObjectTask',
'\Microsoft\Windows\DiskFootprint\Diagnostics',
'\Microsoft\Windows\FileHistory\File History (maintenance mode)',
'\Microsoft\Windows\PI\Sqm-Tasks',
'\Microsoft\Windows\NetTrace\GatherNetworkInfo',
'\Microsoft\Windows\AppID\SmartScreenSpecific',
'\Microsoft\Windows\HelloFace\FODCleanupTask',
'\Microsoft\Windows\Feedback\Siuf\DmClient',
'\Microsoft\Windows\Feedback\Siuf\DmClientOnScenarioDownload',
'\Microsoft\Windows\Application Experience\PcaPatchDbTask',
'\Microsoft\Windows\Device Information\Device',
'\Microsoft\Windows\Device Information\Device User'
)
foreach ($t in $tasks) { schtasks /change /enable /tn "$t" 2>nul }
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_media_player_sharing() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Stop-Service -Name 'WMPNetworkSvc' -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\WMPNetworkSvc' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_media_player_sharing() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\WMPNetworkSvc' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Start-Service -Name 'WMPNetworkSvc' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_home_group() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Stop-Service -Name 'HomeGroupListener' -Force -ErrorAction SilentlyContinue
Stop-Service -Name 'HomeGroupProvider' -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\HomeGroup' -Name 'DisableHomeGroup' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\HomeGroupListener' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\HomeGroupProvider' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_home_group() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\HomeGroup' -Name 'DisableHomeGroup' -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\HomeGroupListener' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\HomeGroupProvider' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Start-Service -Name 'HomeGroupListener' -ErrorAction SilentlyContinue
Start-Service -Name 'HomeGroupProvider' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_smb1() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\LanmanServer\Parameters' -Name 'SMB1' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_smb1() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\LanmanServer\Parameters' -Name 'SMB1' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_smb2() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\LanmanServer\Parameters' -Name 'SMB2' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_smb2() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\LanmanServer\Parameters' -Name 'SMB2' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_office_telemetry() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
# Disable Office telemetry tasks
schtasks /end /tn '\Microsoft\Office\OfficeTelemetryAgentFallBack2016' /f 2>nul
schtasks /change /disable /tn '\Microsoft\Office\OfficeTelemetryAgentFallBack2016' 2>nul
schtasks /end /tn '\Microsoft\Office\OfficeTelemetryAgentLogOn2016' /f 2>nul
schtasks /change /disable /tn '\Microsoft\Office\OfficeTelemetryAgentLogOn2016' 2>nul
schtasks /end /tn '\Microsoft\Office\OfficeTelemetryAgentFallBack' /f 2>nul
schtasks /change /disable /tn '\Microsoft\Office\OfficeTelemetryAgentFallBack' 2>nul
schtasks /end /tn '\Microsoft\Office\OfficeTelemetryAgentLogOn' /f 2>nul
schtasks /change /disable /tn '\Microsoft\Office\OfficeTelemetryAgentLogOn' 2>nul
# Office 2016/365 telemetry registry
$paths = @(
@{Path='HKCU:\SOFTWARE\Microsoft\Office\15.0\Outlook\Options\Mail'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Microsoft\Office\16.0\Outlook\Options\Mail'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Microsoft\Office\15.0\Outlook\Options\Calendar'; Name='EnableCalendarLogging'},
@{Path='HKCU:\SOFTWARE\Microsoft\Office\16.0\Outlook\Options\Calendar'; Name='EnableCalendarLogging'},
@{Path='HKCU:\SOFTWARE\Microsoft\Office\15.0\Word\Options'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Microsoft\Office\16.0\Word\Options'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Policies\Microsoft\Office\15.0\OSM'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Policies\Microsoft\Office\16.0\OSM'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Policies\Microsoft\Office\15.0\OSM'; Name='EnableUpload'},
@{Path='HKCU:\SOFTWARE\Policies\Microsoft\Office\16.0\OSM'; Name='EnableUpload'}
)
foreach ($p in $paths) { New-Item -Path $p.Path -Force | Out-Null; Set-ItemProperty -Path $p.Path -Name $p.Name -Value 0 -Type DWord -Force }
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\Common\ClientTelemetry' -Name 'DisableTelemetry' -Value 1 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\Common\ClientTelemetry' -Name 'VerboseLogging' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\16.0\Common\ClientTelemetry' -Name 'DisableTelemetry' -Value 1 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\16.0\Common\ClientTelemetry' -Name 'VerboseLogging' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\15.0\Common' -Name 'QMEnable' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\16.0\Common' -Name 'QMEnable' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\15.0\Common\Feedback' -Name 'Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\16.0\Common\Feedback' -Name 'Enabled' -Value 0 -Type DWord -Force
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_office_telemetry() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
schtasks /change /enable /tn '\Microsoft\Office\OfficeTelemetryAgentFallBack2016' 2>nul
schtasks /change /enable /tn '\Microsoft\Office\OfficeTelemetryAgentLogOn2016' 2>nul
schtasks /change /enable /tn '\Microsoft\Office\OfficeTelemetryAgentFallBack' 2>nul
schtasks /change /enable /tn '\Microsoft\Office\OfficeTelemetryAgentLogOn' 2>nul
$paths = @(
@{Path='HKCU:\SOFTWARE\Microsoft\Office\15.0\Outlook\Options\Mail'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Microsoft\Office\16.0\Outlook\Options\Mail'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Microsoft\Office\15.0\Outlook\Options\Calendar'; Name='EnableCalendarLogging'},
@{Path='HKCU:\SOFTWARE\Microsoft\Office\16.0\Outlook\Options\Calendar'; Name='EnableCalendarLogging'},
@{Path='HKCU:\SOFTWARE\Microsoft\Office\15.0\Word\Options'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Microsoft\Office\16.0\Word\Options'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Policies\Microsoft\Office\15.0\OSM'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Policies\Microsoft\Office\16.0\OSM'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Policies\Microsoft\Office\15.0\OSM'; Name='EnableUpload'},
@{Path='HKCU:\SOFTWARE\Policies\Microsoft\Office\16.0\OSM'; Name='EnableUpload'}
)
foreach ($p in $paths) { Remove-ItemProperty -Path $p.Path -Name $p.Name -ErrorAction SilentlyContinue }
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\Common\ClientTelemetry' -Name 'DisableTelemetry' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\Common\ClientTelemetry' -Name 'VerboseLogging' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\16.0\Common\ClientTelemetry' -Name 'DisableTelemetry' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\16.0\Common\ClientTelemetry' -Name 'VerboseLogging' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\15.0\Common' -Name 'QMEnable' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\16.0\Common' -Name 'QMEnable' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\15.0\Common\Feedback' -Name 'Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\16.0\Common\Feedback' -Name 'Enabled' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_firefox_telemetry() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Mozilla\Firefox' -Name 'DisableTelemetry' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Mozilla\Firefox' -Name 'DisableDefaultBrowserAgent' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
schtasks /change /disable /tn '\Mozilla\Firefox Default Browser Agent 308046B0AF4A39CB' 2>nul
schtasks /change /disable /tn '\Mozilla\Firefox Default Browser Agent D2CEEC440E2074BD' 2>nul
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_firefox_telemetry() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Mozilla\Firefox' -Name 'DisableTelemetry' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Mozilla\Firefox' -Name 'DisableDefaultBrowserAgent' -ErrorAction SilentlyContinue
schtasks /change /enable /tn '\Mozilla\Firefox Default Browser Agent 308046B0AF4A39CB' 2>nul
schtasks /change /enable /tn '\Mozilla\Firefox Default Browser Agent D2CEEC440E2074BD' 2>nul
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_chrome_telemetry() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
$p = 'HKLM:\SOFTWARE\Policies\Google\Chrome'
New-Item -Path $p -Force | Out-Null
Set-ItemProperty -Path $p -Name 'MetricsReportingEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path $p -Name 'ChromeCleanupReportingEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path $p -Name 'ChromeCleanupEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path $p -Name 'UserFeedbackAllowed' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path $p -Name 'DeviceMetricsReportingEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path $p -Name 'ExtensionManifestV2Availability' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_chrome_telemetry() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
$p = 'HKLM:\SOFTWARE\Policies\Google\Chrome'
Remove-ItemProperty -Path $p -Name 'MetricsReportingEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'ChromeCleanupReportingEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'ChromeCleanupEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'UserFeedbackAllowed' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'DeviceMetricsReportingEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'ExtensionManifestV2Availability' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_nvidia_telemetry() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\NvTelemetryContainer' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue
sc.exe config NvTelemetryContainer start= disabled 2>nul
net.exe stop NvTelemetryContainer 2>nul
sc.exe stop NvTelemetryContainer 2>nul
schtasks /change /disable /tn 'NvTmRepOnLogon_{B2FE1952-0186-46C3-BAEC-A80AA35AC5B8}' 2>nul
schtasks /change /disable /tn 'NvTmRep_{B2FE1952-0186-46C3-BAEC-A80AA35AC5B8}' 2>nul
schtasks /change /disable /tn 'NvTmMon_{B2FE1952-0186-46C3-BAEC-A80AA35AC5B8}' 2>nul
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_nvidia_telemetry() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\NvTelemetryContainer' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
sc.exe config NvTelemetryContainer start= auto 2>nul
net.exe start NvTelemetryContainer 2>nul
sc.exe start NvTelemetryContainer 2>nul
schtasks /change /enable /tn 'NvTmRepOnLogon_{B2FE1952-0186-46C3-BAEC-A80AA35AC5B8}' 2>nul
schtasks /change /enable /tn 'NvTmRep_{B2FE1952-0186-46C3-BAEC-A80AA35AC5B8}' 2>nul
schtasks /change /enable /tn 'NvTmMon_{B2FE1952-0186-46C3-BAEC-A80AA35AC5B8}' 2>nul
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_vs_telemetry() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\VisualStudio\Telemetry' -Name 'TurnOffSwitch' -Value 1 -Type DWord -Force
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\Feedback' -Name 'DisableFeedbackDialog' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\Feedback' -Name 'DisableEmailInput' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\Feedback' -Name 'DisableScreenshotCapture' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\Software\Policies\Microsoft\VisualStudio\SQM' -Name 'OptIn' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\Setup' -Name 'ConcurrentDownloads' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\VSCommon\14.0\SQM' -Name 'OptIn' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\VSCommon\15.0\SQM' -Name 'OptIn' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\VSCommon\16.0\SQM' -Name 'OptIn' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
# Disable the protected service
sc.exe config VSStandardCollectorService150 start= disabled 2>nul
net.exe stop VSStandardCollectorService150 2>nul
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_vs_telemetry() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\VisualStudio\Telemetry' -Name 'TurnOffSwitch' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\Feedback' -Name 'DisableFeedbackDialog' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\Feedback' -Name 'DisableEmailInput' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\Feedback' -Name 'DisableScreenshotCapture' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\Software\Policies\Microsoft\VisualStudio\SQM' -Name 'OptIn' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\Setup' -Name 'ConcurrentDownloads' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\VSCommon\14.0\SQM' -Name 'OptIn' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\VSCommon\15.0\SQM' -Name 'OptIn' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\VSCommon\16.0\SQM' -Name 'OptIn' -ErrorAction SilentlyContinue
sc.exe config VSStandardCollectorService150 start= demand 2>nul
Write-Output 'OK'
"#)
}

// === Optimizer features: Telemetry Services ===

#[tauri::command]
pub async fn disable_telemetry_services() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Stop-Service -Name 'DiagTrack' -Force -ErrorAction SilentlyContinue
Stop-Service -Name 'diagnosticshub.standardcollector.service' -Force -ErrorAction SilentlyContinue
Stop-Service -Name 'dmwappushservice' -Force -ErrorAction SilentlyContinue
Stop-Service -Name 'DcpSvc' -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\DiagTrack' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\diagnosticshub.standardcollector.service' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\dmwappushservice' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\DcpSvc' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'DisableEngine' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'SbEnable' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'AITEnable' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'DisableInventory' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'DisablePCA' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'DisableUAR' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' -Name 'PublishUserActivities' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\SQMClient\Windows' -Name 'CEIPEnable' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Device Metadata' -Name 'PreventDeviceMetadataFromNetwork' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\WMI\AutoLogger\SQMLogger' -Name 'Start' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\PolicyManager\current\device\System' -Name 'AllowExperimentation' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
sc.exe config WdiServiceHost start= disabled 2>nul
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_telemetry_services() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\DiagTrack' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\diagnosticshub.standardcollector.service' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\dmwappushservice' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\DcpSvc' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Start-Service -Name 'DiagTrack' -ErrorAction SilentlyContinue
Start-Service -Name 'diagnosticshub.standardcollector.service' -ErrorAction SilentlyContinue
Start-Service -Name 'dmwappushservice' -ErrorAction SilentlyContinue
Start-Service -Name 'DcpSvc' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'DisableEngine' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'SbEnable' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'AITEnable' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'DisableInventory' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'DisablePCA' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'DisableUAR' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' -Name 'PublishUserActivities' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\SQMClient\Windows' -Name 'CEIPEnable' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Device Metadata' -Name 'PreventDeviceMetadataFromNetwork' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\WMI\AutoLogger\SQMLogger' -Name 'Start' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\PolicyManager\current\device\System' -Name 'AllowExperimentation' -ErrorAction SilentlyContinue
sc.exe config WdiServiceHost start= demand 2>nul
Write-Output 'OK'
"#)
}

// === Optimizer features: Cortana ===

#[tauri::command]
pub async fn disable_cortana() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name 'AllowCortana' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name 'DisableWebSearch' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name 'ConnectedSearchUseWeb' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name 'ConnectedSearchUseWebOverMeteredConnections' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name 'AllowCloudSearch' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name 'CortanaConsent' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name 'BingSearchEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name 'AllowSearchToUseLocation' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name 'HistoryViewEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name 'DeviceHistoryEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\SearchSettings' -Name 'IsDeviceSearchHistoryEnabled' -Value 0 -Type DWord -Force
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_cortana() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name 'AllowCortana' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name 'DisableWebSearch' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name 'ConnectedSearchUseWeb' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name 'ConnectedSearchUseWebOverMeteredConnections' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name 'AllowCloudSearch' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name 'CortanaConsent' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name 'BingSearchEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name 'AllowSearchToUseLocation' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name 'HistoryViewEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name 'DeviceHistoryEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\SearchSettings' -Name 'IsDeviceSearchHistoryEnabled' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: News and Interests ===

#[tauri::command]
pub async fn disable_news_interests() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Dsh' -Name 'AllowNewsAndInterests' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Feeds' -Name 'EnableFeeds' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\PolicyManager\default\NewsAndInterests\AllowNewsAndInterests' -Name 'value' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_news_interests() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Dsh' -Name 'AllowNewsAndInterests' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Feeds' -Name 'EnableFeeds' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\PolicyManager\default\NewsAndInterests\AllowNewsAndInterests' -Name 'value' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Start Menu Ads ===

#[tauri::command]
pub async fn disable_start_menu_ads() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Mobility' -Name 'OptedIn' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Notifications\Settings\Windows.SystemToast.Suggested' -Name 'Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-88000326Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\UserProfileEngagement' -Name 'ScoobeSystemSettingEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'ContentDeliveryAllowed' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'PreInstalledAppsEverEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SilentInstalledAppsEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-314559Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-338387Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-338389Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SystemPaneSuggestionsEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-338393Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-353694Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-353696Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-310093Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-338388Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContentEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SoftLandingEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'FeatureManagementEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Policies\Microsoft\Windows\Explorer' -Name 'DisableSearchBoxSuggestions' -Value 1 -Type DWord -Force
Set-ItemProperty -Path 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer' -Name 'AllowOnlineTips' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Explorer' -Name 'DisableSearchBoxSuggestions' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_start_menu_ads() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Mobility' -Name 'OptedIn' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Notifications\Settings\Windows.SystemToast.Suggested' -Name 'Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-88000326Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\UserProfileEngagement' -Name 'ScoobeSystemSettingEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'ContentDeliveryAllowed' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'PreInstalledAppsEverEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SilentInstalledAppsEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-314559Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-338387Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-338389Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SystemPaneSuggestionsEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-338393Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-353694Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-353696Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-310093Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-338388Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContentEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SoftLandingEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'FeatureManagementEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Policies\Microsoft\Windows\Explorer' -Name 'DisableSearchBoxSuggestions' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer' -Name 'AllowOnlineTips' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Explorer' -Name 'DisableSearchBoxSuggestions' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Edge Telemetry ===

#[tauri::command]
pub async fn disable_edge_telemetry() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'PersonalizationReportingEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\Edge' -Name 'PersonalizationReportingEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'UserFeedbackAllowed' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\Edge' -Name 'UserFeedbackAllowed' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'MetricsReportingEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\Edge' -Name 'MetricsReportingEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\MicrosoftEdge\BooksLibrary' -Name 'EnableExtendedBooksTelemetry' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\MicrosoftEdge\BooksLibrary' -Name 'EnableExtendedBooksTelemetry' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Edge\SmartScreenEnabled' -Name '(default)' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Edge\SmartScreenPuaEnabled' -Name '(default)' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'ExtensionManifestV2Availability' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'Edge3PSerpTelemetryEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'SpotlightExperiencesAndRecommendationsEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\Edge' -Name 'SpotlightExperiencesAndRecommendationsEnabled' -Value 0 -Type DWord -Force
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_edge_telemetry() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'PersonalizationReportingEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\Edge' -Name 'PersonalizationReportingEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'UserFeedbackAllowed' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\Edge' -Name 'UserFeedbackAllowed' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'MetricsReportingEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\Edge' -Name 'MetricsReportingEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\MicrosoftEdge\BooksLibrary' -Name 'EnableExtendedBooksTelemetry' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\MicrosoftEdge\BooksLibrary' -Name 'EnableExtendedBooksTelemetry' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Edge\SmartScreenEnabled' -Name '(default)' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Edge\SmartScreenPuaEnabled' -Name '(default)' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'ExtensionManifestV2Availability' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'Edge3PSerpTelemetryEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'SpotlightExperiencesAndRecommendationsEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\Edge' -Name 'SpotlightExperiencesAndRecommendationsEnabled' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Edge Discover Bar ===

#[tauri::command]
pub async fn disable_edge_discover_bar() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'HubsSidebarEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\Edge' -Name 'HubsSidebarEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'WebWidgetAllowed' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_edge_discover_bar() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'HubsSidebarEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\Edge' -Name 'HubsSidebarEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'WebWidgetAllowed' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Process Count ===

#[tauri::command]
pub async fn optimize_process_count() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
$v = [convert]::ToInt32('ffffffff', 16)
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control' -Name 'SvcHostSplitThresholdInKB' -Value $v -Type DWord -Force
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn restore_process_count() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control' -Name 'SvcHostSplitThresholdInKB' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Store search ===

#[tauri::command]
pub async fn disable_store_search_app() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
New-Item -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Explorer' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Explorer' -Name 'NoUseStoreOpenWith' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_store_search_app() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Explorer' -Name 'NoUseStoreOpenWith' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Store promotions ===

#[tauri::command]
pub async fn disable_store_promotions() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
$p = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager'
Set-ItemProperty -Path $p -Name 'ContentDeliveryAllowed' -Value 0 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'SubscribedContent-338387Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'SubscribedContent-338388Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'SubscribedContent-338389Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'SubscribedContent-338393Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'SilentInstalledAppsEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'OemPreInstalledAppsEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'PreInstalledAppsEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'PreInstalledAppsEverEnabled' -Value 0 -Type DWord -Force
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_store_promotions() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
$p = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager'
Remove-ItemProperty -Path $p -Name 'ContentDeliveryAllowed' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'SubscribedContent-338387Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'SubscribedContent-338388Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'SubscribedContent-338389Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'SubscribedContent-338393Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'SilentInstalledAppsEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'OemPreInstalledAppsEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'PreInstalledAppsEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'PreInstalledAppsEverEnabled' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Store auto-update ===

#[tauri::command]
pub async fn disable_store_auto_update() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
New-Item -Path 'HKLM:\SOFTWARE\Policies\Microsoft\WindowsStore' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\WindowsStore' -Name 'AutoDownload' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_store_auto_update() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\WindowsStore' -Name 'AutoDownload' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Windows Spotlight ===

#[tauri::command]
pub async fn disable_spotlight_lock() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
$p = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager'
Set-ItemProperty -Path $p -Name 'RotatingLockScreenEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'RotatingLockScreenOverlayEnabled' -Value 0 -Type DWord -Force
New-Item -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Personalization' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Personalization' -Name 'NoChangingLockScreen' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_spotlight_lock() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
$p = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager'
Remove-ItemProperty -Path $p -Name 'RotatingLockScreenEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'RotatingLockScreenOverlayEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Personalization' -Name 'NoChangingLockScreen' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: My People ===

#[tauri::command]
pub async fn disable_my_people() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Advanced\People' -Name 'PeopleBand' -Value 0 -Type DWord -Force
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_my_people() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Advanced\People' -Name 'PeopleBand' -Value 1 -Type DWord -Force
Write-Output 'OK'
"#)
}

// === Optimizer features: TPM Check ===

#[tauri::command]
pub async fn disable_tpm_check() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
New-Item -Path 'HKLM:\SYSTEM\Setup\MoSetup' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\SYSTEM\Setup\MoSetup' -Name 'AllowUpgradesWithUnsupportedTPMOrCPU' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
New-Item -Path 'HKLM:\SYSTEM\Setup\LabConfig' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\SYSTEM\Setup\LabConfig' -Name 'BypassCPUCheck' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\Setup\LabConfig' -Name 'BypassStorageCheck' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\Setup\LabConfig' -Name 'BypassTPMCheck' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\Setup\LabConfig' -Name 'BypassRAMCheck' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\Setup\LabConfig' -Name 'BypassSecureBootCheck' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\Control Panel\UnsupportedHardwareNotificationCache' -Name 'SV2' -Value 0 -Type DWord -Force
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_tpm_check() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SYSTEM\Setup\MoSetup' -Name 'AllowUpgradesWithUnsupportedTPMOrCPU' -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\Setup\LabConfig' -Name 'BypassCPUCheck' -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\Setup\LabConfig' -Name 'BypassStorageCheck' -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\Setup\LabConfig' -Name 'BypassTPMCheck' -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\Setup\LabConfig' -Name 'BypassRAMCheck' -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\Setup\LabConfig' -Name 'BypassSecureBootCheck' -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\Control Panel\UnsupportedHardwareNotificationCache' -Name 'SV2' -Value 1 -Type DWord -Force
Write-Output 'OK'
"#)
}

// === Optimizer features: Sensor Services ===

#[tauri::command]
pub async fn disable_sensor_services() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-Service -Name 'SensrSvc' -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name 'SensrSvc' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'SensorService' -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name 'SensorService' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_sensor_services() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-Service -Name 'SensrSvc' -StartupType Automatic -ErrorAction SilentlyContinue
Start-Service -Name 'SensrSvc' -ErrorAction SilentlyContinue
Set-Service -Name 'SensorService' -StartupType Automatic -ErrorAction SilentlyContinue
Start-Service -Name 'SensorService' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Cast to Device ===

#[tauri::command]
pub async fn remove_cast_to_device() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
New-Item -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Shell Extensions\Blocked' -Force -ErrorAction SilentlyContinue | Out-Null
New-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Shell Extensions\Blocked' -Name '{7AD84985-87B4-4a16-BE58-8B72A5B390F7}' -Value 'Play to Menu' -PropertyType String -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn add_cast_to_device() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Shell Extensions\Blocked' -Name '{7AD84985-87B4-4a16-BE58-8B72A5B390F7}' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: VBS ===

#[tauri::command]
pub async fn disable_vbs() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\DeviceGuard' -Name 'EnableVirtualizationBasedSecurity' -Value 0 -Type DWord -Force
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_vbs() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\DeviceGuard' -Name 'EnableVirtualizationBasedSecurity' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Modern Standby ===

#[tauri::command]
pub async fn disable_modern_standby() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Power' -Name 'PlatformAoAcOverride' -Value 0 -Type DWord -Force
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_modern_standby() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Power' -Name 'PlatformAoAcOverride' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Gaming Mode ===

#[tauri::command]
pub async fn enable_gaming_mode() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\GraphicsDrivers' -Name 'HwSchMode' -Value 2 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\GameBar' -Name 'AllowAutoGameMode' -Value 1 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\GameBar' -Name 'AutoGameModeEnabled' -Value 1 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\System\GameConfigStore' -Name 'GameDVR_FSEBehaviorMode' -Value 2 -Type DWord -Force
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_gaming_mode() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\GraphicsDrivers' -Name 'HwSchMode' -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\GameBar' -Name 'AllowAutoGameMode' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\GameBar' -Name 'AutoGameModeEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\System\GameConfigStore' -Name 'GameDVR_FSEBehaviorMode' -Value 0 -Type DWord -Force
Write-Output 'OK'
"#)
}

// === Optimizer features: Xbox Live ===

#[tauri::command]
pub async fn disable_xbox_live() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-Service -Name 'XboxNetApiSvc' -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name 'XboxNetApiSvc' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'XblAuthManager' -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name 'XblAuthManager' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'XblGameSave' -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name 'XblGameSave' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'XboxGipSvc' -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name 'XboxGipSvc' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'xbgm' -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name 'xbgm' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_xbox_live() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-Service -Name 'XboxNetApiSvc' -StartupType Automatic -ErrorAction SilentlyContinue
Set-Service -Name 'XblAuthManager' -StartupType Automatic -ErrorAction SilentlyContinue
Set-Service -Name 'XblGameSave' -StartupType Automatic -ErrorAction SilentlyContinue
Set-Service -Name 'XboxGipSvc' -StartupType Automatic -ErrorAction SilentlyContinue
Set-Service -Name 'xbgm' -StartupType Automatic -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Game Bar ===

#[tauri::command]
pub async fn disable_game_bar() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\GameDVR' -Name 'AppCaptureEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\GameDVR' -Name 'AudioCaptureEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\GameDVR' -Name 'CursorCaptureEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\GameBar' -Name 'UseNexusForGameBarEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\GameBar' -Name 'ShowStartupPanel' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\System\GameConfigStore' -Name 'GameDVR_Enabled' -Value 0 -Type DWord -Force
New-Item -Path 'HKLM:\Software\Policies\Microsoft\Windows\GameDVR' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\Software\Policies\Microsoft\Windows\GameDVR' -Name 'AllowGameDVR' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_game_bar() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\GameDVR' -Name 'AppCaptureEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\GameDVR' -Name 'AudioCaptureEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\GameDVR' -Name 'CursorCaptureEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\GameBar' -Name 'UseNexusForGameBarEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\GameBar' -Name 'ShowStartupPanel' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\System\GameConfigStore' -Name 'GameDVR_Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\Software\Policies\Microsoft\Windows\GameDVR' -Name 'AllowGameDVR' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Windows Ink ===

#[tauri::command]
pub async fn disable_windows_ink() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
New-Item -Path 'HKLM:\SOFTWARE\Policies\Microsoft\WindowsInkWorkspace' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\WindowsInkWorkspace' -Name 'AllowWindowsInkWorkspace' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\WindowsInkWorkspace' -Name 'AllowSuggestedAppsInWindowsInkWorkspace' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\TabletTip\1.7' -Name 'EnableInkingWithTouch' -Value 0 -Type DWord -Force
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_windows_ink() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\WindowsInkWorkspace' -Name 'AllowWindowsInkWorkspace' -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\WindowsInkWorkspace' -Name 'AllowSuggestedAppsInWindowsInkWorkspace' -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\TabletTip\1.7' -Name 'EnableInkingWithTouch' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Spelling & Typing ===

#[tauri::command]
pub async fn disable_spelling_typing() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\TabletTip\1.7' -Name 'EnableAutocorrection' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\TabletTip\1.7' -Name 'EnableSpellchecking' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Input\Settings' -Name 'InsightsEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\TabletTip\1.7' -Name 'EnableDoubleTapSpace' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\TabletTip\1.7' -Name 'EnablePredictionSpaceInsertion' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\TabletTip\1.7' -Name 'EnableTextPrediction' -Value 0 -Type DWord -Force
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_spelling_typing() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\TabletTip\1.7' -Name 'EnableAutocorrection' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\TabletTip\1.7' -Name 'EnableSpellchecking' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Input\Settings' -Name 'InsightsEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\TabletTip\1.7' -Name 'EnableDoubleTapSpace' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\TabletTip\1.7' -Name 'EnablePredictionSpaceInsertion' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\TabletTip\1.7' -Name 'EnableTextPrediction' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Cloud Clipboard ===

#[tauri::command]
pub async fn disable_cloud_clipboard() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
New-Item -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' -Name 'AllowClipboardHistory' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' -Name 'AllowCrossDeviceClipboard' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Clipboard' -Name 'EnableClipboardHistory' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKLM:\Software\Microsoft\Clipboard' -Name 'EnableClipboardHistory' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_cloud_clipboard() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' -Name 'AllowClipboardHistory' -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' -Name 'AllowCrossDeviceClipboard' -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Clipboard' -Name 'EnableClipboardHistory' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\Software\Microsoft\Clipboard' -Name 'EnableClipboardHistory' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: App Launch Tracking ===

#[tauri::command]
pub async fn disable_app_launch_tracking() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Privacy' -Name 'EnableActivityFeed' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-338389Enabled' -Value 0 -Type DWord -Force
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_app_launch_tracking() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Privacy' -Name 'EnableActivityFeed' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-338389Enabled' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Advertising ID ===

#[tauri::command]
pub async fn disable_advertising_id() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo' -Name 'Enabled' -Value 0 -Type DWord -Force
New-Item -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AdvertisingInfo' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AdvertisingInfo' -Name 'DisabledByGroupPolicy' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_advertising_id() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo' -Name 'Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AdvertisingInfo' -Name 'DisabledByGroupPolicy' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: File System Access ===

#[tauri::command]
pub async fn disable_file_system_access() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
New-Item -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\broadFileSystemAccess' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\broadFileSystemAccess' -Name 'Value' -Value 'Deny' -Type String -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_file_system_access() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-Item -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\broadFileSystemAccess' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Documents Access ===

#[tauri::command]
pub async fn disable_documents_access() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
New-Item -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\documentsLibrary' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\documentsLibrary' -Name 'Value' -Value 'Deny' -Type String -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_documents_access() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-Item -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\documentsLibrary' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Calendar Access ===

#[tauri::command]
pub async fn disable_calendar_access() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
New-Item -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\appointments' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\appointments' -Name 'Value' -Value 'Deny' -Type String -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_calendar_access() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-Item -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\appointments' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Contacts Access ===

#[tauri::command]
pub async fn disable_contacts_access() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
New-Item -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\contacts' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\contacts' -Name 'Value' -Value 'Deny' -Type String -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_contacts_access() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-Item -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\contacts' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Language Tracking ===

#[tauri::command]
pub async fn disable_language_tracking() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKCU:\Control Panel\International\User Profile' -Name 'HttpAcceptLanguageOptOut' -Value 1 -Type DWord -Force
New-Item -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Control Panel\International' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Control Panel\International' -Name 'HttpAcceptLanguageOptOut' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_language_tracking() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKCU:\Control Panel\International\User Profile' -Name 'HttpAcceptLanguageOptOut' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Control Panel\International' -Name 'HttpAcceptLanguageOptOut' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Welcome Experience ===

#[tauri::command]
pub async fn disable_welcome_experience() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-310093Enabled' -Value 0 -Type DWord -Force
New-Item -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\CloudContent' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\CloudContent' -Name 'DisableWindowsConsumerFeatures' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_welcome_experience() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-310093Enabled' -Value 1 -Type DWord -Force
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\CloudContent' -Name 'DisableWindowsConsumerFeatures' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Feedback Frequency ===

#[tauri::command]
pub async fn disable_feedback_frequency() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
New-Item -Path 'HKCU:\Software\Microsoft\Siuf\Rules' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Siuf\Rules' -Name 'NumberOfSIUFInPeriod' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Siuf\Rules' -Name 'PeriodInNanoSeconds' -Value 0 -Type DWord -Force
New-Item -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection' -Name 'DoNotShowFeedbackNotifications' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_feedback_frequency() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Siuf\Rules' -Name 'NumberOfSIUFInPeriod' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Siuf\Rules' -Name 'PeriodInNanoSeconds' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection' -Name 'DoNotShowFeedbackNotifications' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Diagnostic Data ===

#[tauri::command]
pub async fn disable_diagnostic_data() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
New-Item -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection' -Name 'AllowTelemetry' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection' -Name 'MaxTelemetryAllowed' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\DataCollection' -Name 'AllowTelemetry' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-Service -Name 'DiagTrack' -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name 'DiagTrack' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'dmwappushservice' -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name 'dmwappushservice' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_diagnostic_data() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection' -Name 'AllowTelemetry' -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection' -Name 'MaxTelemetryAllowed' -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\DataCollection' -Name 'AllowTelemetry' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'DiagTrack' -StartupType Manual -ErrorAction SilentlyContinue
Set-Service -Name 'dmwappushservice' -StartupType Manual -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Optimizer features: Writing Habits ===

#[tauri::command]
pub async fn disable_writing_habits() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
New-Item -Path 'HKLM:\SOFTWARE\Policies\Microsoft\InputPersonalization' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\InputPersonalization' -Name 'AllowInputPersonalization' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization' -Name 'RestrictImplicitTextCollection' -Value 1 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization' -Name 'RestrictImplicitInkCollection' -Value 1 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization\TrainedDataStore' -Name 'HarvestContacts' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Personalization\Settings' -Name 'AcceptedPrivacyPolicy' -Value 0 -Type DWord -Force
New-Item -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\HandwritingErrorReports' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\HandwritingErrorReports' -Name 'ForceDisableHandwritingErrorReports' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_writing_habits() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\InputPersonalization' -Name 'AllowInputPersonalization' -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization' -Name 'RestrictImplicitTextCollection' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization' -Name 'RestrictImplicitInkCollection' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization\TrainedDataStore' -Name 'HarvestContacts' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Personalization\Settings' -Name 'AcceptedPrivacyPolicy' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\HandwritingErrorReports' -Name 'ForceDisableHandwritingErrorReports' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === System Tweaks: CEIP ===

#[tauri::command]
pub async fn disable_ceip() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\SQMClient\Windows' -Name 'CEIPEnable' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_ceip() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\SQMClient\Windows' -Name 'CEIPEnable' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === System Tweaks: NTFS Link Tracking Service ===

#[tauri::command]
pub async fn disable_trk_wks() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\TrkWks' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue
Stop-Service -Name 'TrkWks' -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_trk_wks() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\TrkWks' -Name 'Start' -Value 3 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === System Tweaks: Auto Maintenance ===

#[tauri::command]
pub async fn disable_auto_maintenance() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Schedule\Maintenance' -Name 'MaintenanceDisabled' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_auto_maintenance() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Schedule\Maintenance' -Name 'MaintenanceDisabled' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === System Tweaks: Large System Cache ===

#[tauri::command]
pub async fn enable_large_sys_cache() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management' -Name 'LargeSystemCache' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn disable_large_sys_cache() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management' -Name 'LargeSystemCache' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === System Tweaks: Spectre/Meltdown Patch ===

#[tauri::command]
pub async fn disable_spectre_patch() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management' -Name 'FeatureSettingsOverride' -Value 3 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management' -Name 'FeatureSettingsOverrideMask' -Value 3 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_spectre_patch() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management' -Name 'FeatureSettingsOverride' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management' -Name 'FeatureSettingsOverrideMask' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === System Tweaks: Auto Debug ===

#[tauri::command]
pub async fn disable_auto_debug() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\CrashControl' -Name 'AutoReboot' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\CrashControl' -Name 'LogEvent' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_auto_debug() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\CrashControl' -Name 'AutoReboot' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\CrashControl' -Name 'LogEvent' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === System Tweaks: Crash Dump ===

#[tauri::command]
pub async fn disable_crash_dump() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\CrashControl' -Name 'CrashDumpEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_crash_dump() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\CrashControl' -Name 'CrashDumpEnabled' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === System Tweaks: Audit Log ===

#[tauri::command]
pub async fn disable_audit_log() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Lsa' -Name 'CrashOnAuditFail' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_audit_log() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Lsa' -Name 'CrashOnAuditFail' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === System Tweaks: WFP Diagnostics ===

#[tauri::command]
pub async fn disable_wfp_diag() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\BFE\Parameters' -Name 'CollectionEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_wfp_diag() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\BFE\Parameters' -Name 'CollectionEnabled' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Privacy Tweaks: Address Book Collection ===

#[tauri::command]
pub async fn disable_address_book_collect() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContactManagement' -Name 'NoContactSharing' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_address_book_collect() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContactManagement' -Name 'NoContactSharing' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Privacy Tweaks: Typing Collection ===

#[tauri::command]
pub async fn disable_typing_collection() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization' -Name 'RestrictImplicitTextCollection' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization' -Name 'RestrictImplicitInkCollection' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_typing_collection() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization' -Name 'RestrictImplicitTextCollection' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization' -Name 'RestrictImplicitInkCollection' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Privacy Tweaks: Silent App Install ===

#[tauri::command]
pub async fn disable_silent_app_install() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SilentInstalledAppsEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_silent_app_install() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SilentInstalledAppsEnabled' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Privacy Tweaks: WiFi Hotspots ===

#[tauri::command]
pub async fn disable_wifi_hotspots() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
New-Item -Path 'HKLM:\SOFTWARE\Microsoft\PolicyManager\default\WiFi\AllowWiFiHotSpotReporting' -Force | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\PolicyManager\default\WiFi\AllowWiFiHotSpotReporting' -Name 'value' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_wifi_hotspots() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\PolicyManager\default\WiFi\AllowWiFiHotSpotReporting' -Name 'value' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Privacy Tweaks: Typing Insights ===

#[tauri::command]
pub async fn disable_typing_insights() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization\TrainedDataStore' -Name 'HarvestContacts' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_typing_insights() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization\TrainedDataStore' -Name 'HarvestContacts' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Privacy Tweaks: Preinstalled Apps ===

#[tauri::command]
pub async fn disable_preinstalled_apps() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\CloudContent' -Name 'DisableWindowsConsumerFeatures' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_preinstalled_apps() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\CloudContent' -Name 'DisableWindowsConsumerFeatures' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Privacy Tweaks: .NET Telemetry ===

#[tauri::command]
pub async fn disable_dotnet_telemetry() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\.NETFramework' -Name 'TelemetryEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_dotnet_telemetry() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\.NETFramework' -Name 'TelemetryEnabled' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Privacy Tweaks: PowerShell Telemetry ===

#[tauri::command]
pub async fn disable_pwsh_telemetry() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
[Environment]::SetEnvironmentVariable('POWERSHELL_TELEMETRY_OPTOUT', '1', 'Machine')
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_pwsh_telemetry() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
[Environment]::SetEnvironmentVariable('POWERSHELL_TELEMETRY_OPTOUT', $null, 'Machine')
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Environment' -Name 'POWERSHELL_TELEMETRY_OPTOUT' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === State detection: single PowerShell to check all tweak states ===

#[tauri::command]
pub async fn check_all_tweak_states() -> Result<HashMap<String, bool>, String> {
    let script = r#"
$ErrorActionPreference = 'SilentlyContinue'
$r = @{}

function GetDWord($path, $name) { $v = (Get-ItemProperty -Path $path -Name $name -ErrorAction SilentlyContinue).$name; if ($null -eq $v) { return $null }; return $v }
function GetStr($path, $name) { $v = (Get-ItemProperty -Path $path -Name $name -ErrorAction SilentlyContinue).$name; if ($null -eq $v) { return $null }; return $v.ToString() }
function GetSvc($name) { $v = (Get-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Services\$name" -Name 'Start' -ErrorAction SilentlyContinue).Start; if ($null -eq $v) { return $null }; return $v }

$v = GetStr 'HKCU:\Control Panel\Desktop' 'AutoEndTasks'; $r['master'] = ($v -eq '1')
$v = GetStr 'HKCU:\Control Panel\Desktop' 'MenuShowDelay'; $r['removeMenuDelay'] = ($v -eq '0')
$v = GetDWord 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' 'NetworkThrottlingIndex'; $r['disableNetworkThrottling'] = ($null -ne $v -and $v -eq -1)
$v = GetSvc 'WerSvc'; $r['disableErrorReporting'] = ($null -ne $v -and $v -eq 4)
$v = GetSvc 'PcaSvc'; $r['disableCompatibilityAssistant'] = ($null -ne $v -and $v -eq 4)
$v = GetSvc 'Spooler'; $r['disablePrintService'] = ($null -ne $v -and $v -eq 3)
$v = GetSvc 'Fax'; $r['disableFaxService'] = ($null -ne $v -and $v -eq 4)
$v = GetStr 'HKCU:\Control Panel\Accessibility\StickyKeys' 'Flags'; $r['disableStickyKeys'] = ($v -eq '506')
$v = GetStr 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer' 'SmartScreenEnabled'; $r['disableSmartScreen'] = ($v -eq 'Off')
$v = GetDWord 'HKLM:\SOFTWARE\Policies\Microsoft\Windows NT\SystemRestore' 'DisableSR'; $r['disableSystemRestore'] = ($null -ne $v -and $v -eq 1)
$v = GetSvc 'SysMain'; $r['disableSuperfetch'] = ($null -ne $v -and $v -eq 4)
$v = GetDWord 'HKLM:\SYSTEM\CurrentControlSet\Control\Power' 'HibernateEnabled'; $r['disableHibernate'] = ($null -ne $v -and $v -eq 0)
$v = (fsutil behavior query disablelastaccess 2>$null) -replace '.*?(\d+).*|.*','$1'; $r['disableNtfsTimestamp'] = ($v -eq '1')
$t = schtasks /query /tn '\Microsoft\Windows\Customer Experience Improvement Program\Consolidator' /v /fo csv 2>$null; $r['disableTelemetryTasks'] = [bool]($t -match 'Disabled')
$v = GetSvc 'WMPNetworkSvc'; $r['disableMediaPlayerSharing'] = ($null -ne $v -and $v -eq 4)
$v = GetDWord 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\HomeGroup' 'DisableHomeGroup'; $r['disableHomeGroup'] = ($null -ne $v -and $v -eq 1)
$v = GetDWord 'HKLM:\SYSTEM\CurrentControlSet\Services\LanmanServer\Parameters' 'SMB1'; $r['disableSmb1'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKLM:\SYSTEM\CurrentControlSet\Services\LanmanServer\Parameters' 'SMB2'; $r['disableSmb2'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKCU:\SOFTWARE\Microsoft\Office\Common\ClientTelemetry' 'DisableTelemetry'; $r['disableOfficeTelemetry'] = ($null -ne $v -and $v -eq 1)
$v = GetDWord 'HKLM:\SOFTWARE\Policies\Mozilla\Firefox' 'DisableTelemetry'; $r['disableFirefoxTelemetry'] = ($null -ne $v -and $v -eq 1)
$v = GetDWord 'HKLM:\SOFTWARE\Policies\Google\Chrome' 'MetricsReportingEnabled'; $r['disableChromeTelemetry'] = ($null -ne $v -and $v -eq 0)
$v = GetSvc 'NvTelemetryContainer'; $r['disableNvidiaTelemetry'] = ($null -ne $v -and $v -eq 4)
$v = GetDWord 'HKCU:\Software\Microsoft\VisualStudio\Telemetry' 'TurnOffSwitch'; $r['disableVsTelemetry'] = ($null -ne $v -and $v -eq 1)

# === New Optimizer features ===
$v = GetSvc 'DiagTrack'; $r['disableTelemetryServices'] = ($null -ne $v -and $v -eq 4)
$v = GetDWord 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' 'AllowCortana'; $r['disableCortana'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKLM:\SOFTWARE\Policies\Microsoft\Dsh' 'AllowNewsAndInterests'; $r['disableNewsInterests'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' 'ContentDeliveryAllowed'; $r['disableStartMenuAds'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' 'MetricsReportingEnabled'; $r['disableEdgeTelemetry'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' 'HubsSidebarEnabled'; $r['disableEdgeDiscoverBar'] = ($null -ne $v -and $v -eq 0)

# === New ZyperWin++ features ===
$v = GetDWord 'HKLM:\SYSTEM\CurrentControlSet\Control' 'SvcHostSplitThresholdInKB'; $r['optimizeProcessCount'] = ($null -ne $v -and $v -eq -1)
$v = GetDWord 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Explorer' 'NoUseStoreOpenWith'; $r['disableStoreSearchApp'] = ($null -ne $v -and $v -eq 1)
$v = GetDWord 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' 'SilentInstalledAppsEnabled'; $r['disableStorePromotions'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKLM:\SOFTWARE\Policies\Microsoft\WindowsStore' 'AutoDownload'; $r['disableStoreAutoUpdate'] = ($null -ne $v -and $v -eq 2)
$v = GetDWord 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' 'RotatingLockScreenEnabled'; $r['disableSpotlightLock'] = ($null -ne $v -and $v -eq 0)

# === New Optimizer features (12 more) ===
$v = GetDWord 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Advanced\People' 'PeopleBand'; $r['disableMyPeople'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKLM:\SYSTEM\Setup\MoSetup' 'AllowUpgradesWithUnsupportedTPMOrCPU'; $r['disableTPMCheck'] = ($null -ne $v -and $v -eq 1)
$v = GetSvc 'SensrSvc'; $r['disableSensorServices'] = ($null -ne $v -and $v -eq 4)
$v = GetStr 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Shell Extensions\Blocked' '{7AD84985-87B4-4a16-BE58-8B72A5B390F7}'; $r['removeCastToDevice'] = ($null -ne $v)
$v = GetDWord 'HKLM:\SYSTEM\CurrentControlSet\Control\DeviceGuard' 'EnableVirtualizationBasedSecurity'; $r['disableVBS'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKLM:\SYSTEM\CurrentControlSet\Control\Power' 'PlatformAoAcOverride'; $r['disableModernStandby'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKLM:\SYSTEM\CurrentControlSet\Control\GraphicsDrivers' 'HwSchMode'; $r['enableGameMode'] = ($null -ne $v -and $v -eq 2)
$v = GetSvc 'XboxNetApiSvc'; $r['disableXboxLive'] = ($null -ne $v -and $v -eq 4)
$v = GetDWord 'HKCU:\Software\Microsoft\Windows\CurrentVersion\GameDVR' 'AppCaptureEnabled'; $r['disableGameBar'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKLM:\SOFTWARE\Policies\Microsoft\WindowsInkWorkspace' 'AllowWindowsInkWorkspace'; $r['disableWindowsInk'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKCU:\SOFTWARE\Microsoft\TabletTip\1.7' 'EnableSpellchecking'; $r['disableSpellCheck'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' 'AllowClipboardHistory'; $r['disableCloudClipboard'] = ($null -ne $v -and $v -eq 0)

# === New privacy features ===
$v = GetDWord 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Privacy' 'EnableActivityFeed'; $r['disableAppLaunchTracking'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKCU:\Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo' 'Enabled'; $r['disableAdvertisingId'] = ($null -ne $v -and $v -eq 0)
$v = GetStr 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\broadFileSystemAccess' 'Value'; $r['disableFileSystemAccess'] = ($v -eq 'Deny')
$v = GetStr 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\documentsLibrary' 'Value'; $r['disableDocumentsAccess'] = ($v -eq 'Deny')
$v = GetStr 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\appointments' 'Value'; $r['disableCalendarAccess'] = ($v -eq 'Deny')
$v = GetStr 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\contacts' 'Value'; $r['disableContactsAccess'] = ($v -eq 'Deny')
$v = GetDWord 'HKCU:\Control Panel\International\User Profile' 'HttpAcceptLanguageOptOut'; $r['disableLanguageTracking'] = ($null -ne $v -and $v -eq 1)
$v = GetDWord 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' 'SubscribedContent-310093Enabled'; $r['disableWelcomeExperience'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKCU:\Software\Microsoft\Siuf\Rules' 'NumberOfSIUFInPeriod'; $r['disableFeedbackFrequency'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection' 'AllowTelemetry'; $r['disableDiagnosticData'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKCU:\Software\Microsoft\InputPersonalization' 'RestrictImplicitTextCollection'; $r['disableWritingHabits'] = ($null -ne $v -and $v -eq 1)

# === ZyperWin++ System ===
$v = GetDWord 'HKLM:\SOFTWARE\Microsoft\SQMClient\Windows' 'CEIPEnable'; $r['disableCEIP'] = ($null -ne $v -and $v -eq 0)
$v = GetSvc 'TrkWks'; $r['disableTrkWks'] = ($null -ne $v -and $v -eq 4)
$v = GetDWord 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Schedule\Maintenance' 'MaintenanceDisabled'; $r['disableAutoMaintenance'] = ($null -ne $v -and $v -eq 1)
$v = GetDWord 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management' 'LargeSystemCache'; $r['enableLargeSysCache'] = ($null -ne $v -and $v -eq 1)
$v = GetDWord 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management' 'FeatureSettingsOverride'; $r['disableSpectrePatch'] = ($null -ne $v -and $v -eq 3)
$v = GetDWord 'HKLM:\SYSTEM\CurrentControlSet\Control\CrashControl' 'AutoReboot'; $r['disableAutoDebug'] = ($null -ne $v -and $v -eq 1)
$v = GetDWord 'HKLM:\SYSTEM\CurrentControlSet\Control\CrashControl' 'CrashDumpEnabled'; $r['disableCrashDump'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKLM:\SYSTEM\CurrentControlSet\Control\Lsa' 'CrashOnAuditFail'; $r['disableAuditLog'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKLM:\SYSTEM\CurrentControlSet\Services\BFE\Parameters' 'CollectionEnabled'; $r['disableWfpDiag'] = ($null -ne $v -and $v -eq 0)
# === ZyperWin++ Privacy ===
$v = GetDWord 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContactManagement' 'NoContactSharing'; $r['disableAddressBookCollect'] = ($null -ne $v -and $v -eq 1)
$v = GetDWord 'HKCU:\Software\Microsoft\InputPersonalization' 'RestrictImplicitTextCollection'; $r['disableTypingCollection'] = ($null -ne $v -and $v -eq 1)
$v = GetDWord 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' 'SilentInstalledAppsEnabled'; $r['disableSilentAppInstall'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKLM:\SOFTWARE\Microsoft\PolicyManager\default\WiFi\AllowWiFiHotSpotReporting' 'value'; $r['disableWiFiHotspots'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKCU:\Software\Microsoft\InputPersonalization\TrainedDataStore' 'HarvestContacts'; $r['disableTypingInsights'] = ($null -ne $v -and $v -eq 0)
$v = GetDWord 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\CloudContent' 'DisableWindowsConsumerFeatures'; $r['disablePreinstalledApps'] = ($null -ne $v -and $v -eq 1)
$v = GetDWord 'HKLM:\SOFTWARE\Policies\Microsoft\.NETFramework' 'TelemetryEnabled'; $r['disableDotNetTelemetry'] = ($null -ne $v -and $v -eq 0)
$v = (Get-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Environment' -Name 'POWERSHELL_TELEMETRY_OPTOUT' -ErrorAction SilentlyContinue).POWERSHELL_TELEMETRY_OPTOUT; $r['disablePwshTelemetry'] = ($null -ne $v -and $v -eq '1')

$r | ConvertTo-Json -Compress
"#;

    let ps_path = get_powershell_path();
    let result = Command::new(&ps_path)
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行命令失败: {}", e))?;

    if !result.status.success() {
        return Err("检测状态失败".to_string());
    }

    let stdout = String::from_utf8_lossy(&result.stdout).to_string();
    serde_json::from_str(&stdout).map_err(|e| format!("解析状态失败: {}", e))
}

// === Wi-Fi Sense ===

#[tauri::command]
pub async fn disable_wifi_sense() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
New-Item -Path 'HKLM:\SOFTWARE\Microsoft\PolicyManager\default\WiFi\AllowAutoConnectToWiFiSenseHotspots' -Force | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\PolicyManager\default\WiFi\AllowAutoConnectToWiFiSenseHotspots' -Name 'value' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\WcmSvc\wifinetworkmanager\config' -Name 'AutoConnectAllowedOEM' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_wifi_sense() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\PolicyManager\default\WiFi\AllowAutoConnectToWiFiSenseHotspots' -Name 'value' -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\WcmSvc\wifinetworkmanager\config' -Name 'AutoConnectAllowedOEM' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Step Recorder ===

#[tauri::command]
pub async fn disable_step_recorder() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
New-Item -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\StepRecorder' -Force | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\StepRecorder' -Name 'DisableStepRecorder' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

#[tauri::command]
pub async fn enable_step_recorder() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\StepRecorder' -Name 'DisableStepRecorder' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// === Batch commands: single PowerShell for all tweaks ===

fn run_batch_script(script: &str, success_msg: &str, admin_msg: &str) -> Result<PerfTweakResult, String> {
    let result = run_ps_script(script)?;

    if result.status.success() {
        Ok(PerfTweakResult { success: true, message: success_msg.to_string() })
    } else {
        let stderr = String::from_utf8_lossy(&result.stderr).to_string();
        let stdout = String::from_utf8_lossy(&result.stdout).to_string();
        let err_msg = if !stderr.trim().is_empty() { stderr.trim().to_string() } else { stdout.trim().to_string() };
        let lower = err_msg.to_lowercase();
        if lower.contains("access denied") || lower.contains("denied") || lower.contains("拒绝访问") || lower.contains("权限不足") {
            Err(admin_msg.to_string())
        } else {
            Err(format!("操作失败: {}", err_msg))
        }
    }
}

#[tauri::command]
pub async fn batch_enable_tweaks() -> Result<PerfTweakResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }
    run_batch_script(r#"
$ErrorActionPreference = 'SilentlyContinue'

# === Master: Registry tweaks ===
Set-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'AutoEndTasks' -Value '1' -Type String -Force
Set-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'HungAppTimeout' -Value '1000' -Type String -Force
Set-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'WaitToKillAppTimeout' -Value '2000' -Type String -Force
Set-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'LowLevelHooksTimeout' -Value '1000' -Type String -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced' -Name 'DisallowShaking' -Value 1 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced' -Name 'HideFileExt' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced' -Name 'Hidden' -Value 1 -Type DWord -Force
$p = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer'
New-Item -Path $p -Force | Out-Null
Set-ItemProperty -Path $p -Name 'NoLowDiskSpaceChecks' -Value 1 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'LinkResolveIgnoreLinkInfo' -Value 1 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'NoResolveSearch' -Value 1 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'NoResolveTrack' -Value 1 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'NoInternetOpenWith' -Value 1 -Type DWord -Force
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\CrashControl' -Name 'CrashDumpEnabled' -Value 3 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Remote Assistance' -Name 'fAllowToGetHelp' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control' -Name 'WaitToKillServiceTimeout' -Value '2000' -Type String -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' -Name 'SystemResponsiveness' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' -Name 'NoLazyMode' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' -Name 'AlwaysOn' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
$games = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games'
New-Item -Path $games -Force | Out-Null
Set-ItemProperty -Path $games -Name 'GPU Priority' -Value 8 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path $games -Name 'Priority' -Value 6 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path $games -Name 'Scheduling Category' -Value 'High' -Type String -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path $games -Name 'SFIO Priority' -Value 'High' -Type String -Force -ErrorAction SilentlyContinue
$ll = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Low Latency'
New-Item -Path $ll -Force | Out-Null
Set-ItemProperty -Path $ll -Name 'GPU Priority' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path $ll -Name 'Priority' -Value 8 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path $ll -Name 'Scheduling Category' -Value 'Medium' -Type String -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows Media Foundation' -Name 'EnableFrameServerMode' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-Service -Name 'DiagTrack' -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name 'DiagTrack' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'diagnosticshub.standardcollector.service' -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name 'diagnosticshub.standardcollector.service' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'dmwappushservice' -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name 'dmwappushservice' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'RemoteRegistry' -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name 'RemoteRegistry' -Force -ErrorAction SilentlyContinue

# === Remove Menu Delay ===
Set-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'MenuShowDelay' -Value '0' -Type String -Force
Set-ItemProperty -Path 'HKCU:\Control Panel\Mouse' -Name 'MouseHoverTime' -Value '0' -Type String -Force

# === Disable Network Throttling ===
$v = [convert]::ToInt32('ffffffff', 16)
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' -Name 'NetworkThrottlingIndex' -Value $v -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Psched' -Name 'NonBestEffortLimit' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue

# === Disable Error Reporting ===
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Error Reporting' -Name 'Disabled' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\PCHealth\ErrorReporting' -Name 'DoReport' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\Windows Error Reporting' -Name 'Disabled' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-Service -Name 'WerSvc' -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name 'WerSvc' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'wercplsupport' -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name 'wercplsupport' -Force -ErrorAction SilentlyContinue

# === Disable Compatibility Assistant ===
Stop-Service -Name 'PcaSvc' -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\PcaSvc' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue

# === Disable Print Service ===
Stop-Service -Name 'Spooler' -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\Spooler' -Name 'Start' -Value 3 -Type DWord -Force -ErrorAction SilentlyContinue

# === Disable Fax Service ===
Stop-Service -Name 'Fax' -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\Fax' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue

# === Disable Sticky Keys ===
Set-ItemProperty -Path 'HKCU:\Control Panel\Accessibility\StickyKeys' -Name 'Flags' -Value '506' -Type String -Force
Set-ItemProperty -Path 'HKCU:\Control Panel\Accessibility\Keyboard Response' -Name 'Flags' -Value '122' -Type String -Force
Set-ItemProperty -Path 'HKCU:\Control Panel\Accessibility\ToggleKeys' -Name 'Flags' -Value '58' -Type String -Force
reg add "HKEY_USERS\.DEFAULT\Control Panel\Accessibility\StickyKeys" /v "Flags" /t REG_SZ /d "506" /f 2>nul
reg add "HKEY_USERS\.DEFAULT\Control Panel\Accessibility\Keyboard Response" /v "Flags" /t REG_SZ /d "122" /f 2>nul
reg add "HKEY_USERS\.DEFAULT\Control Panel\Accessibility\ToggleKeys" /v "Flags" /t REG_SZ /d "58" /f 2>nul

# === Disable SmartScreen ===
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\Attachments' -Name 'SaveZoneInformation' -Value 1 -Type DWord -Force
Set-ItemProperty -Path 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Policies\Attachments' -Name 'ScanWithAntiVirus' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' -Name 'ShellSmartScreenLevel' -Value 'Warn' -Type String -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' -Name 'EnableSmartScreen' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer' -Name 'SmartScreenEnabled' -Value 'Off' -Type String -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Internet Explorer\PhishingFilter' -Name 'EnabledV9' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\AppHost' -Name 'PreventOverride' -Value 0 -Type DWord -Force

# === Disable System Restore ===
vssadmin delete shadows /for=c: /all /quiet 2>nul
Stop-Service -Name 'VSS' -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows NT\SystemRestore' -Name 'DisableSR' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows NT\SystemRestore' -Name 'DisableConfig' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue

# === Disable Superfetch ===
Stop-Service -Name 'SysMain' -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\SysMain' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management\PrefetchParameters' -Name 'EnableSuperfetch' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management\PrefetchParameters' -Name 'EnablePrefetcher' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management\PrefetchParameters' -Name 'SfTracingState' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue

# === Disable Hibernate ===
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Power' -Name 'HibernateEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
powercfg -h off

# === Disable NTFS Timestamp ===
fsutil behavior set disablelastaccess 1

# === Disable Telemetry Tasks ===
$autoLogger = "$env:ProgramData\Microsoft\Diagnosis\ETLLogs\AutoLogger"
if (Test-Path $autoLogger) { icacls $autoLogger /deny SYSTEM:`(OI`)`(CI`)F 2>nul }
$tasks = @(
'\Microsoft\Windows\Customer Experience Improvement Program\Consolidator',
'\Microsoft\Windows\Customer Experience Improvement Program\BthSQM',
'\Microsoft\Windows\Customer Experience Improvement Program\KernelCeipTask',
'\Microsoft\Windows\Customer Experience Improvement Program\UsbCeip',
'\Microsoft\Windows\Customer Experience Improvement Program\Uploader',
'\Microsoft\Windows\Application Experience\Microsoft Compatibility Appraiser',
'\Microsoft\Windows\Application Experience\ProgramDataUpdater',
'\Microsoft\Windows\Application Experience\StartupAppTask',
'\Microsoft\Windows\DiskDiagnostic\Microsoft-Windows-DiskDiagnosticDataCollector',
'\Microsoft\Windows\DiskDiagnostic\Microsoft-Windows-DiskDiagnosticResolver',
'\Microsoft\Windows\Power Efficiency Diagnostics\AnalyzeSystem',
'\Microsoft\Windows\Shell\FamilySafetyMonitor',
'\Microsoft\Windows\Shell\FamilySafetyRefresh',
'\Microsoft\Windows\Shell\FamilySafetyUpload',
'\Microsoft\Windows\Autochk\Proxy',
'\Microsoft\Windows\Maintenance\WinSAT',
'\Microsoft\Windows\Application Experience\AitAgent',
'\Microsoft\Windows\Windows Error Reporting\QueueReporting',
'\Microsoft\Windows\CloudExperienceHost\CreateObjectTask',
'\Microsoft\Windows\DiskFootprint\Diagnostics',
'\Microsoft\Windows\FileHistory\File History (maintenance mode)',
'\Microsoft\Windows\PI\Sqm-Tasks',
'\Microsoft\Windows\NetTrace\GatherNetworkInfo',
'\Microsoft\Windows\AppID\SmartScreenSpecific',
'\Microsoft\Windows\HelloFace\FODCleanupTask',
'\Microsoft\Windows\Feedback\Siuf\DmClient',
'\Microsoft\Windows\Feedback\Siuf\DmClientOnScenarioDownload',
'\Microsoft\Windows\Application Experience\PcaPatchDbTask',
'\Microsoft\Windows\Device Information\Device',
'\Microsoft\Windows\Device Information\Device User'
)
foreach ($t in $tasks) { schtasks /end /tn "$t" /f 2>nul; schtasks /change /disable /tn "$t" 2>nul }

# === Disable Media Player Sharing ===
Stop-Service -Name 'WMPNetworkSvc' -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\WMPNetworkSvc' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue

# === Disable HomeGroup ===
Stop-Service -Name 'HomeGroupListener' -Force -ErrorAction SilentlyContinue
Stop-Service -Name 'HomeGroupProvider' -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\HomeGroup' -Name 'DisableHomeGroup' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\HomeGroupListener' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\HomeGroupProvider' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue

# === Disable SMB1 ===
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\LanmanServer\Parameters' -Name 'SMB1' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue

# === Disable SMB2 ===
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\LanmanServer\Parameters' -Name 'SMB2' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue

# === Disable Office Telemetry ===
schtasks /end /tn '\Microsoft\Office\OfficeTelemetryAgentFallBack2016' /f 2>nul
schtasks /change /disable /tn '\Microsoft\Office\OfficeTelemetryAgentFallBack2016' 2>nul
schtasks /end /tn '\Microsoft\Office\OfficeTelemetryAgentLogOn2016' /f 2>nul
schtasks /change /disable /tn '\Microsoft\Office\OfficeTelemetryAgentLogOn2016' 2>nul
schtasks /end /tn '\Microsoft\Office\OfficeTelemetryAgentFallBack' /f 2>nul
schtasks /change /disable /tn '\Microsoft\Office\OfficeTelemetryAgentFallBack' 2>nul
schtasks /end /tn '\Microsoft\Office\OfficeTelemetryAgentLogOn' /f 2>nul
schtasks /change /disable /tn '\Microsoft\Office\OfficeTelemetryAgentLogOn' 2>nul
$paths = @(
@{Path='HKCU:\SOFTWARE\Microsoft\Office\15.0\Outlook\Options\Mail'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Microsoft\Office\16.0\Outlook\Options\Mail'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Microsoft\Office\15.0\Outlook\Options\Calendar'; Name='EnableCalendarLogging'},
@{Path='HKCU:\SOFTWARE\Microsoft\Office\16.0\Outlook\Options\Calendar'; Name='EnableCalendarLogging'},
@{Path='HKCU:\SOFTWARE\Microsoft\Office\15.0\Word\Options'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Microsoft\Office\16.0\Word\Options'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Policies\Microsoft\Office\15.0\OSM'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Policies\Microsoft\Office\16.0\OSM'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Policies\Microsoft\Office\15.0\OSM'; Name='EnableUpload'},
@{Path='HKCU:\SOFTWARE\Policies\Microsoft\Office\16.0\OSM'; Name='EnableUpload'}
)
foreach ($p in $paths) { New-Item -Path $p.Path -Force | Out-Null; Set-ItemProperty -Path $p.Path -Name $p.Name -Value 0 -Type DWord -Force }
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\Common\ClientTelemetry' -Name 'DisableTelemetry' -Value 1 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\Common\ClientTelemetry' -Name 'VerboseLogging' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\16.0\Common\ClientTelemetry' -Name 'DisableTelemetry' -Value 1 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\16.0\Common\ClientTelemetry' -Name 'VerboseLogging' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\15.0\Common' -Name 'QMEnable' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\16.0\Common' -Name 'QMEnable' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\15.0\Common\Feedback' -Name 'Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\16.0\Common\Feedback' -Name 'Enabled' -Value 0 -Type DWord -Force

# === Disable Firefox Telemetry ===
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Mozilla\Firefox' -Name 'DisableTelemetry' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Mozilla\Firefox' -Name 'DisableDefaultBrowserAgent' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
schtasks /change /disable /tn '\Mozilla\Firefox Default Browser Agent 308046B0AF4A39CB' 2>nul
schtasks /change /disable /tn '\Mozilla\Firefox Default Browser Agent D2CEEC440E2074BD' 2>nul

# === Disable Chrome Telemetry ===
$cp = 'HKLM:\SOFTWARE\Policies\Google\Chrome'
New-Item -Path $cp -Force | Out-Null
Set-ItemProperty -Path $cp -Name 'MetricsReportingEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path $cp -Name 'ChromeCleanupReportingEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path $cp -Name 'ChromeCleanupEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path $cp -Name 'UserFeedbackAllowed' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path $cp -Name 'DeviceMetricsReportingEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path $cp -Name 'ExtensionManifestV2Availability' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue

# === Disable NVIDIA Telemetry ===
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\NvTelemetryContainer' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue
sc.exe config NvTelemetryContainer start= disabled 2>nul
net.exe stop NvTelemetryContainer 2>nul
sc.exe stop NvTelemetryContainer 2>nul
schtasks /change /disable /tn 'NvTmRepOnLogon_{B2FE1952-0186-46C3-BAEC-A80AA35AC5B8}' 2>nul
schtasks /change /disable /tn 'NvTmRep_{B2FE1952-0186-46C3-BAEC-A80AA35AC5B8}' 2>nul
schtasks /change /disable /tn 'NvTmMon_{B2FE1952-0186-46C3-BAEC-A80AA35AC5B8}' 2>nul

# === Disable VS Telemetry ===
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\VisualStudio\Telemetry' -Name 'TurnOffSwitch' -Value 1 -Type DWord -Force
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\Feedback' -Name 'DisableFeedbackDialog' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\Feedback' -Name 'DisableEmailInput' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\Feedback' -Name 'DisableScreenshotCapture' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\Software\Policies\Microsoft\VisualStudio\SQM' -Name 'OptIn' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\Setup' -Name 'ConcurrentDownloads' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\VSCommon\14.0\SQM' -Name 'OptIn' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\VSCommon\15.0\SQM' -Name 'OptIn' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\VSCommon\16.0\SQM' -Name 'OptIn' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
sc.exe config VSStandardCollectorService150 start= disabled 2>nul
net.exe stop VSStandardCollectorService150 2>nul

# === Disable Telemetry Services ===
Stop-Service -Name 'DiagTrack' -Force -ErrorAction SilentlyContinue
Stop-Service -Name 'diagnosticshub.standardcollector.service' -Force -ErrorAction SilentlyContinue
Stop-Service -Name 'dmwappushservice' -Force -ErrorAction SilentlyContinue
Stop-Service -Name 'DcpSvc' -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\DiagTrack' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\diagnosticshub.standardcollector.service' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\dmwappushservice' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\DcpSvc' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'DisableEngine' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'SbEnable' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'AITEnable' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'DisableInventory' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'DisablePCA' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'DisableUAR' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' -Name 'PublishUserActivities' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\SQMClient\Windows' -Name 'CEIPEnable' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Device Metadata' -Name 'PreventDeviceMetadataFromNetwork' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\WMI\AutoLogger\SQMLogger' -Name 'Start' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\PolicyManager\current\device\System' -Name 'AllowExperimentation' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
sc.exe config WdiServiceHost start= disabled 2>nul

# === Disable Cortana ===
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name 'AllowCortana' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name 'DisableWebSearch' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name 'ConnectedSearchUseWeb' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name 'ConnectedSearchUseWebOverMeteredConnections' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name 'AllowCloudSearch' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name 'CortanaConsent' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name 'BingSearchEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name 'AllowSearchToUseLocation' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name 'HistoryViewEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name 'DeviceHistoryEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\SearchSettings' -Name 'IsDeviceSearchHistoryEnabled' -Value 0 -Type DWord -Force

# === Disable News and Interests ===
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Dsh' -Name 'AllowNewsAndInterests' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Feeds' -Name 'EnableFeeds' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\PolicyManager\default\NewsAndInterests\AllowNewsAndInterests' -Name 'value' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue

# === Disable Start Menu Ads ===
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Mobility' -Name 'OptedIn' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Notifications\Settings\Windows.SystemToast.Suggested' -Name 'Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-88000326Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\UserProfileEngagement' -Name 'ScoobeSystemSettingEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'ContentDeliveryAllowed' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'PreInstalledAppsEverEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SilentInstalledAppsEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-314559Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-338387Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-338389Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SystemPaneSuggestionsEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-338393Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-353694Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-353696Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-310093Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-338388Enabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContentEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SoftLandingEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'FeatureManagementEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Policies\Microsoft\Windows\Explorer' -Name 'DisableSearchBoxSuggestions' -Value 1 -Type DWord -Force
Set-ItemProperty -Path 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer' -Name 'AllowOnlineTips' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Explorer' -Name 'DisableSearchBoxSuggestions' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue

# === Disable Edge Telemetry ===
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'PersonalizationReportingEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\Edge' -Name 'PersonalizationReportingEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'UserFeedbackAllowed' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\Edge' -Name 'UserFeedbackAllowed' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'MetricsReportingEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\Edge' -Name 'MetricsReportingEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\MicrosoftEdge\BooksLibrary' -Name 'EnableExtendedBooksTelemetry' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\MicrosoftEdge\BooksLibrary' -Name 'EnableExtendedBooksTelemetry' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Edge\SmartScreenEnabled' -Name '(default)' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Edge\SmartScreenPuaEnabled' -Name '(default)' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'ExtensionManifestV2Availability' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'Edge3PSerpTelemetryEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'SpotlightExperiencesAndRecommendationsEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\Edge' -Name 'SpotlightExperiencesAndRecommendationsEnabled' -Value 0 -Type DWord -Force

# === Disable Edge Discover Bar ===
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'HubsSidebarEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\Edge' -Name 'HubsSidebarEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'WebWidgetAllowed' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue

# === Disable My People ===
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced\People' -Name 'PeopleBand' -Value 0 -Type DWord -Force

# === Remove Cast to Device ===
New-Item -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Shell Extensions\Blocked' -Force -ErrorAction SilentlyContinue | Out-Null
New-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Shell Extensions\Blocked' -Name '{7AD84985-87B4-4a16-BE58-8B72A5B390F7}' -Value 'Play to Menu' -PropertyType String -Force -ErrorAction SilentlyContinue

# === Disable TPM Check ===
New-Item -Path 'HKLM:\SYSTEM\Setup\MoSetup' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\SYSTEM\Setup\MoSetup' -Name 'AllowUpgradesWithUnsupportedTPMOrCPU' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
New-Item -Path 'HKLM:\SYSTEM\Setup\LabConfig' -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path 'HKLM:\SYSTEM\Setup\LabConfig' -Name 'BypassTPMCheck' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\Setup\LabConfig' -Name 'BypassCPUCheck' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\Setup\LabConfig' -Name 'BypassRAMCheck' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\Setup\LabConfig' -Name 'BypassSecureBootCheck' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue

# === Disable Sensor Services ===
Set-Service -Name 'SensrSvc' -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name 'SensrSvc' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'SensorService' -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name 'SensorService' -Force -ErrorAction SilentlyContinue

# === ZyperWin++ System Tweaks ===
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\SQMClient\Windows' -Name 'CEIPEnable' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Stop-Service -Name 'TrkWks' -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\TrkWks' -Name 'Start' -Value 4 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Schedule\Maintenance' -Name 'MaintenanceDisabled' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management' -Name 'LargeSystemCache' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management' -Name 'FeatureSettingsOverride' -Value 3 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management' -Name 'FeatureSettingsOverrideMask' -Value 3 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\CrashControl' -Name 'AutoReboot' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\CrashControl' -Name 'LogEvent' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\CrashControl' -Name 'CrashDumpEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Lsa' -Name 'CrashOnAuditFail' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\BFE\Parameters' -Name 'CollectionEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
# === ZyperWin++ Privacy ===
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContactManagement' -Name 'NoContactSharing' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization' -Name 'RestrictImplicitTextCollection' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization' -Name 'RestrictImplicitInkCollection' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SilentInstalledAppsEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
New-Item -Path 'HKLM:\SOFTWARE\Microsoft\PolicyManager\default\WiFi\AllowWiFiHotSpotReporting' -Force | Out-Null
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\PolicyManager\default\WiFi\AllowWiFiHotSpotReporting' -Name 'value' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization\TrainedDataStore' -Name 'HarvestContacts' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\CloudContent' -Name 'DisableWindowsConsumerFeatures' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\.NETFramework' -Name 'TelemetryEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
[Environment]::SetEnvironmentVariable('POWERSHELL_TELEMETRY_OPTOUT', '1', 'Machine')
# === Privacy (existing items missing from batch) ===
# Disable documents access
$p = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\documentsLibrary'
New-Item -Path $p -Force | Out-Null
Set-ItemProperty -Path $p -Name 'Value' -Value 'Deny' -Type String -Force -ErrorAction SilentlyContinue
# Disable calendar access
$p = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\appointments'
New-Item -Path $p -Force | Out-Null
Set-ItemProperty -Path $p -Name 'Value' -Value 'Deny' -Type String -Force -ErrorAction SilentlyContinue
# Disable contacts access
$p = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\contacts'
New-Item -Path $p -Force | Out-Null
Set-ItemProperty -Path $p -Name 'Value' -Value 'Deny' -Type String -Force -ErrorAction SilentlyContinue
# Disable welcome experience
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-310093Enabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
# Disable feedback frequency
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Siuf\Rules' -Name 'NumberOfSIUFInPeriod' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Siuf\Rules' -Name 'PeriodInNanoSeconds' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Siuf\Rules' -Name 'DoNotShowFeedbackNotifications' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
# Disable writing habits
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization' -Name 'RestrictImplicitTextCollection' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization' -Name 'RestrictImplicitInkCollection' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue

Write-Output 'OK'
"#, "性能调整已全部启用，建议重启系统以完全生效", "需要管理员权限，请以管理员身份运行 NexBox")
}

#[tauri::command]
pub async fn batch_disable_tweaks() -> Result<PerfTweakResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }
    run_batch_script(r#"
$ErrorActionPreference = 'SilentlyContinue'

# === Master: Revert registry tweaks ===
Remove-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'AutoEndTasks' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'HungAppTimeout' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'WaitToKillAppTimeout' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'LowLevelHooksTimeout' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced' -Name 'DisallowShaking' -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced' -Name 'HideFileExt' -Value 1 -Type DWord -Force
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced' -Name 'Hidden' -Value 2 -Type DWord -Force
$p = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer'
Remove-ItemProperty -Path $p -Name 'NoLowDiskSpaceChecks' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'LinkResolveIgnoreLinkInfo' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'NoResolveSearch' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'NoResolveTrack' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'NoInternetOpenWith' -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\CrashControl' -Name 'CrashDumpEnabled' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Remote Assistance' -Name 'fAllowToGetHelp' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control' -Name 'WaitToKillServiceTimeout' -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' -Name 'SystemResponsiveness' -Value 10 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' -Name 'NoLazyMode' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' -Name 'AlwaysOn' -ErrorAction SilentlyContinue
$games = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games'
Remove-ItemProperty -Path $games -Name 'GPU Priority' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $games -Name 'Priority' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $games -Name 'Scheduling Category' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $games -Name 'SFIO Priority' -ErrorAction SilentlyContinue
$ll = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Low Latency'
Remove-ItemProperty -Path $ll -Name 'GPU Priority' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $ll -Name 'Priority' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $ll -Name 'Scheduling Category' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows Media Foundation' -Name 'EnableFrameServerMode' -ErrorAction SilentlyContinue
Set-Service -Name 'DiagTrack' -StartupType Manual -ErrorAction SilentlyContinue
Set-Service -Name 'diagnosticshub.standardcollector.service' -StartupType Manual -ErrorAction SilentlyContinue
Set-Service -Name 'dmwappushservice' -StartupType Manual -ErrorAction SilentlyContinue
Set-Service -Name 'RemoteRegistry' -StartupType Manual -ErrorAction SilentlyContinue

# === Restore Menu Delay ===
Set-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name 'MenuShowDelay' -Value '400' -Type String -Force
Set-ItemProperty -Path 'HKCU:\Control Panel\Mouse' -Name 'MouseHoverTime' -Value '400' -Type String -Force

# === Enable Network Throttling ===
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Psched' -Name 'NonBestEffortLimit' -Value 80 -Type DWord -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile' -Name 'NetworkThrottlingIndex' -ErrorAction SilentlyContinue

# === Enable Error Reporting ===
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Error Reporting' -Name 'Disabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\PCHealth\ErrorReporting' -Name 'DoReport' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\Windows Error Reporting' -Name 'Disabled' -ErrorAction SilentlyContinue
Set-Service -Name 'WerSvc' -StartupType Automatic -ErrorAction SilentlyContinue
Set-Service -Name 'wercplsupport' -StartupType Manual -ErrorAction SilentlyContinue

# === Enable Compatibility Assistant ===
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\PcaSvc' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Start-Service -Name 'PcaSvc' -ErrorAction SilentlyContinue

# === Enable Print Service ===
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\Spooler' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Start-Service -Name 'Spooler' -ErrorAction SilentlyContinue

# === Enable Fax Service ===
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\Fax' -Name 'Start' -Value 3 -Type DWord -Force -ErrorAction SilentlyContinue

# === Enable Sticky Keys ===
Set-ItemProperty -Path 'HKCU:\Control Panel\Accessibility\StickyKeys' -Name 'Flags' -Value '510' -Type String -Force
Set-ItemProperty -Path 'HKCU:\Control Panel\Accessibility\Keyboard Response' -Name 'Flags' -Value '126' -Type String -Force
Set-ItemProperty -Path 'HKCU:\Control Panel\Accessibility\ToggleKeys' -Name 'Flags' -Value '62' -Type String -Force
reg add "HKEY_USERS\.DEFAULT\Control Panel\Accessibility\StickyKeys" /v "Flags" /t REG_SZ /d "510" /f 2>nul
reg add "HKEY_USERS\.DEFAULT\Control Panel\Accessibility\Keyboard Response" /v "Flags" /t REG_SZ /d "126" /f 2>nul
reg add "HKEY_USERS\.DEFAULT\Control Panel\Accessibility\ToggleKeys" /v "Flags" /t REG_SZ /d "62" /f 2>nul

# === Enable SmartScreen ===
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\Attachments' -Name 'SaveZoneInformation' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Policies\Attachments' -Name 'ScanWithAntiVirus' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' -Name 'ShellSmartScreenLevel' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' -Name 'EnableSmartScreen' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer' -Name 'SmartScreenEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Internet Explorer\PhishingFilter' -Name 'EnabledV9' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\AppHost' -Name 'PreventOverride' -ErrorAction SilentlyContinue

# === Enable System Restore ===
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows NT\SystemRestore' -Name 'DisableSR' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows NT\SystemRestore' -Name 'DisableConfig' -ErrorAction SilentlyContinue
Start-Service -Name 'VSS' -ErrorAction SilentlyContinue

# === Enable Superfetch ===
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\SysMain' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management\PrefetchParameters' -Name 'EnableSuperfetch' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management\PrefetchParameters' -Name 'EnablePrefetcher' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management\PrefetchParameters' -Name 'SfTracingState' -ErrorAction SilentlyContinue
Start-Service -Name 'SysMain' -ErrorAction SilentlyContinue

# === Enable Hibernate ===
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Power' -Name 'HibernateEnabled' -ErrorAction SilentlyContinue
powercfg -h on

# === Enable NTFS Timestamp ===
fsutil behavior set disablelastaccess 2

# === Enable Telemetry Tasks ===
$tasks = @(
'\Microsoft\Windows\Customer Experience Improvement Program\Consolidator',
'\Microsoft\Windows\Customer Experience Improvement Program\BthSQM',
'\Microsoft\Windows\Customer Experience Improvement Program\KernelCeipTask',
'\Microsoft\Windows\Customer Experience Improvement Program\UsbCeip',
'\Microsoft\Windows\Customer Experience Improvement Program\Uploader',
'\Microsoft\Windows\Application Experience\Microsoft Compatibility Appraiser',
'\Microsoft\Windows\Application Experience\ProgramDataUpdater',
'\Microsoft\Windows\Application Experience\StartupAppTask',
'\Microsoft\Windows\DiskDiagnostic\Microsoft-Windows-DiskDiagnosticDataCollector',
'\Microsoft\Windows\DiskDiagnostic\Microsoft-Windows-DiskDiagnosticResolver',
'\Microsoft\Windows\Power Efficiency Diagnostics\AnalyzeSystem',
'\Microsoft\Windows\Shell\FamilySafetyMonitor',
'\Microsoft\Windows\Shell\FamilySafetyRefresh',
'\Microsoft\Windows\Shell\FamilySafetyUpload',
'\Microsoft\Windows\Autochk\Proxy',
'\Microsoft\Windows\Maintenance\WinSAT',
'\Microsoft\Windows\Application Experience\AitAgent',
'\Microsoft\Windows\Windows Error Reporting\QueueReporting',
'\Microsoft\Windows\CloudExperienceHost\CreateObjectTask',
'\Microsoft\Windows\DiskFootprint\Diagnostics',
'\Microsoft\Windows\FileHistory\File History (maintenance mode)',
'\Microsoft\Windows\PI\Sqm-Tasks',
'\Microsoft\Windows\NetTrace\GatherNetworkInfo',
'\Microsoft\Windows\AppID\SmartScreenSpecific',
'\Microsoft\Windows\HelloFace\FODCleanupTask',
'\Microsoft\Windows\Feedback\Siuf\DmClient',
'\Microsoft\Windows\Feedback\Siuf\DmClientOnScenarioDownload',
'\Microsoft\Windows\Application Experience\PcaPatchDbTask',
'\Microsoft\Windows\Device Information\Device',
'\Microsoft\Windows\Device Information\Device User'
)
foreach ($t in $tasks) { schtasks /change /enable /tn "$t" 2>nul }

# === Enable Media Player Sharing ===
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\WMPNetworkSvc' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Start-Service -Name 'WMPNetworkSvc' -ErrorAction SilentlyContinue

# === Enable HomeGroup ===
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\HomeGroup' -Name 'DisableHomeGroup' -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\HomeGroupListener' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\HomeGroupProvider' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Start-Service -Name 'HomeGroupListener' -ErrorAction SilentlyContinue
Start-Service -Name 'HomeGroupProvider' -ErrorAction SilentlyContinue

# === Enable SMB1 ===
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\LanmanServer\Parameters' -Name 'SMB1' -ErrorAction SilentlyContinue

# === Enable SMB2 ===
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\LanmanServer\Parameters' -Name 'SMB2' -ErrorAction SilentlyContinue

# === Enable Office Telemetry ===
schtasks /change /enable /tn '\Microsoft\Office\OfficeTelemetryAgentFallBack2016' 2>nul
schtasks /change /enable /tn '\Microsoft\Office\OfficeTelemetryAgentLogOn2016' 2>nul
schtasks /change /enable /tn '\Microsoft\Office\OfficeTelemetryAgentFallBack' 2>nul
schtasks /change /enable /tn '\Microsoft\Office\OfficeTelemetryAgentLogOn' 2>nul
$paths = @(
@{Path='HKCU:\SOFTWARE\Microsoft\Office\15.0\Outlook\Options\Mail'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Microsoft\Office\16.0\Outlook\Options\Mail'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Microsoft\Office\15.0\Outlook\Options\Calendar'; Name='EnableCalendarLogging'},
@{Path='HKCU:\SOFTWARE\Microsoft\Office\16.0\Outlook\Options\Calendar'; Name='EnableCalendarLogging'},
@{Path='HKCU:\SOFTWARE\Microsoft\Office\15.0\Word\Options'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Microsoft\Office\16.0\Word\Options'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Policies\Microsoft\Office\15.0\OSM'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Policies\Microsoft\Office\16.0\OSM'; Name='EnableLogging'},
@{Path='HKCU:\SOFTWARE\Policies\Microsoft\Office\15.0\OSM'; Name='EnableUpload'},
@{Path='HKCU:\SOFTWARE\Policies\Microsoft\Office\16.0\OSM'; Name='EnableUpload'}
)
foreach ($p in $paths) { Remove-ItemProperty -Path $p.Path -Name $p.Name -ErrorAction SilentlyContinue }
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\Common\ClientTelemetry' -Name 'DisableTelemetry' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\Common\ClientTelemetry' -Name 'VerboseLogging' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\16.0\Common\ClientTelemetry' -Name 'DisableTelemetry' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\16.0\Common\ClientTelemetry' -Name 'VerboseLogging' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\15.0\Common' -Name 'QMEnable' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\16.0\Common' -Name 'QMEnable' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\15.0\Common\Feedback' -Name 'Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Office\16.0\Common\Feedback' -Name 'Enabled' -ErrorAction SilentlyContinue

# === Enable Firefox Telemetry ===
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Mozilla\Firefox' -Name 'DisableTelemetry' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Mozilla\Firefox' -Name 'DisableDefaultBrowserAgent' -ErrorAction SilentlyContinue
schtasks /change /enable /tn '\Mozilla\Firefox Default Browser Agent 308046B0AF4A39CB' 2>nul
schtasks /change /enable /tn '\Mozilla\Firefox Default Browser Agent D2CEEC440E2074BD' 2>nul

# === Enable Chrome Telemetry ===
$cp = 'HKLM:\SOFTWARE\Policies\Google\Chrome'
Remove-ItemProperty -Path $cp -Name 'MetricsReportingEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $cp -Name 'ChromeCleanupReportingEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $cp -Name 'ChromeCleanupEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $cp -Name 'UserFeedbackAllowed' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $cp -Name 'DeviceMetricsReportingEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $cp -Name 'ExtensionManifestV2Availability' -ErrorAction SilentlyContinue

# === Enable NVIDIA Telemetry ===
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\NvTelemetryContainer' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
sc.exe config NvTelemetryContainer start= auto 2>nul
net.exe start NvTelemetryContainer 2>nul
sc.exe start NvTelemetryContainer 2>nul
schtasks /change /enable /tn 'NvTmRepOnLogon_{B2FE1952-0186-46C3-BAEC-A80AA35AC5B8}' 2>nul
schtasks /change /enable /tn 'NvTmRep_{B2FE1952-0186-46C3-BAEC-A80AA35AC5B8}' 2>nul
schtasks /change /enable /tn 'NvTmMon_{B2FE1952-0186-46C3-BAEC-A80AA35AC5B8}' 2>nul

# === Enable VS Telemetry ===
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\VisualStudio\Telemetry' -Name 'TurnOffSwitch' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\Feedback' -Name 'DisableFeedbackDialog' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\Feedback' -Name 'DisableEmailInput' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\Feedback' -Name 'DisableScreenshotCapture' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\Software\Policies\Microsoft\VisualStudio\SQM' -Name 'OptIn' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\Setup' -Name 'ConcurrentDownloads' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\VSCommon\14.0\SQM' -Name 'OptIn' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\VSCommon\15.0\SQM' -Name 'OptIn' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\VSCommon\16.0\SQM' -Name 'OptIn' -ErrorAction SilentlyContinue
sc.exe config VSStandardCollectorService150 start= demand 2>nul

# === Enable Telemetry Services ===
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\DiagTrack' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\diagnosticshub.standardcollector.service' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\dmwappushservice' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\DcpSvc' -Name 'Start' -Value 2 -Type DWord -Force -ErrorAction SilentlyContinue
Start-Service -Name 'DiagTrack' -ErrorAction SilentlyContinue
Start-Service -Name 'diagnosticshub.standardcollector.service' -ErrorAction SilentlyContinue
Start-Service -Name 'dmwappushservice' -ErrorAction SilentlyContinue
Start-Service -Name 'DcpSvc' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'DisableEngine' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'SbEnable' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'AITEnable' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'DisableInventory' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'DisablePCA' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\AppCompat' -Name 'DisableUAR' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System' -Name 'PublishUserActivities' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\SQMClient\Windows' -Name 'CEIPEnable' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Device Metadata' -Name 'PreventDeviceMetadataFromNetwork' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\WMI\AutoLogger\SQMLogger' -Name 'Start' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\PolicyManager\current\device\System' -Name 'AllowExperimentation' -ErrorAction SilentlyContinue
sc.exe config WdiServiceHost start= demand 2>nul

# === Enable Cortana ===
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name 'AllowCortana' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name 'DisableWebSearch' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name 'ConnectedSearchUseWeb' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name 'ConnectedSearchUseWebOverMeteredConnections' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search' -Name 'AllowCloudSearch' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name 'CortanaConsent' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name 'BingSearchEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name 'AllowSearchToUseLocation' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name 'HistoryViewEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Search' -Name 'DeviceHistoryEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\SearchSettings' -Name 'IsDeviceSearchHistoryEnabled' -ErrorAction SilentlyContinue

# === Enable News and Interests ===
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Dsh' -Name 'AllowNewsAndInterests' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Feeds' -Name 'EnableFeeds' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\PolicyManager\default\NewsAndInterests\AllowNewsAndInterests' -Name 'value' -ErrorAction SilentlyContinue

# === Enable Start Menu Ads ===
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Mobility' -Name 'OptedIn' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Notifications\Settings\Windows.SystemToast.Suggested' -Name 'Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-88000326Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\UserProfileEngagement' -Name 'ScoobeSystemSettingEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'ContentDeliveryAllowed' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'PreInstalledAppsEverEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SilentInstalledAppsEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-314559Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-338387Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-338389Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SystemPaneSuggestionsEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-338393Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-353694Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-353696Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-310093Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-338388Enabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContentEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SoftLandingEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'FeatureManagementEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Policies\Microsoft\Windows\Explorer' -Name 'DisableSearchBoxSuggestions' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer' -Name 'AllowOnlineTips' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Explorer' -Name 'DisableSearchBoxSuggestions' -ErrorAction SilentlyContinue

# === Enable Edge Telemetry ===
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'PersonalizationReportingEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\Edge' -Name 'PersonalizationReportingEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'UserFeedbackAllowed' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\Edge' -Name 'UserFeedbackAllowed' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'MetricsReportingEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\Edge' -Name 'MetricsReportingEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\MicrosoftEdge\BooksLibrary' -Name 'EnableExtendedBooksTelemetry' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\MicrosoftEdge\BooksLibrary' -Name 'EnableExtendedBooksTelemetry' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Edge\SmartScreenEnabled' -Name '(default)' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Edge\SmartScreenPuaEnabled' -Name '(default)' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'ExtensionManifestV2Availability' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'Edge3PSerpTelemetryEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'SpotlightExperiencesAndRecommendationsEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\Edge' -Name 'SpotlightExperiencesAndRecommendationsEnabled' -ErrorAction SilentlyContinue

# === Enable Edge Discover Bar ===
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'HubsSidebarEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\SOFTWARE\Policies\Microsoft\Edge' -Name 'HubsSidebarEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -Name 'WebWidgetAllowed' -ErrorAction SilentlyContinue

# === Enable My People ===
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced\People' -Name 'PeopleBand' -ErrorAction SilentlyContinue

# === Add Cast to Device ===
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Shell Extensions\Blocked' -Name '{7AD84985-87B4-4a16-BE58-8B72A5B390F7}' -Force -ErrorAction SilentlyContinue

# === Enable TPM Check ===
Remove-ItemProperty -Path 'HKLM:\SYSTEM\Setup\MoSetup' -Name 'AllowUpgradesWithUnsupportedTPMOrCPU' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\Setup\LabConfig' -Name 'BypassTPMCheck' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\Setup\LabConfig' -Name 'BypassCPUCheck' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\Setup\LabConfig' -Name 'BypassRAMCheck' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\Setup\LabConfig' -Name 'BypassSecureBootCheck' -ErrorAction SilentlyContinue

# === Enable Sensor Services ===
Set-Service -Name 'SensrSvc' -StartupType Automatic -ErrorAction SilentlyContinue
Start-Service -Name 'SensrSvc' -ErrorAction SilentlyContinue
Set-Service -Name 'SensorService' -StartupType Automatic -ErrorAction SilentlyContinue
Start-Service -Name 'SensorService' -ErrorAction SilentlyContinue

# === ZyperWin++ System Restore ===
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\SQMClient\Windows' -Name 'CEIPEnable' -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\TrkWks' -Name 'Start' -Value 3 -Type DWord -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Schedule\Maintenance' -Name 'MaintenanceDisabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management' -Name 'LargeSystemCache' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management' -Name 'FeatureSettingsOverride' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management' -Name 'FeatureSettingsOverrideMask' -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\CrashControl' -Name 'AutoReboot' -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\CrashControl' -Name 'LogEvent' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\CrashControl' -Name 'CrashDumpEnabled' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Lsa' -Name 'CrashOnAuditFail' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\BFE\Parameters' -Name 'CollectionEnabled' -ErrorAction SilentlyContinue
# === ZyperWin++ Privacy Restore ===
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContactManagement' -Name 'NoContactSharing' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization' -Name 'RestrictImplicitTextCollection' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization' -Name 'RestrictImplicitInkCollection' -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SilentInstalledAppsEnabled' -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\PolicyManager\default\WiFi\AllowWiFiHotSpotReporting' -Name 'value' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization\TrainedDataStore' -Name 'HarvestContacts' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\CloudContent' -Name 'DisableWindowsConsumerFeatures' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\.NETFramework' -Name 'TelemetryEnabled' -ErrorAction SilentlyContinue
[Environment]::SetEnvironmentVariable('POWERSHELL_TELEMETRY_OPTOUT', $null, 'Machine')
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Environment' -Name 'POWERSHELL_TELEMETRY_OPTOUT' -ErrorAction SilentlyContinue
# === Privacy items that were missing ===
# Restore documents access
Remove-Item -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\documentsLibrary' -Force -ErrorAction SilentlyContinue
# Restore calendar access
Remove-Item -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\appointments' -Force -ErrorAction SilentlyContinue
# Restore contacts access
Remove-Item -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\contacts' -Force -ErrorAction SilentlyContinue
# Restore welcome experience
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager' -Name 'SubscribedContent-310093Enabled' -ErrorAction SilentlyContinue
# Restore feedback frequency
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Siuf\Rules' -Name 'NumberOfSIUFInPeriod' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Siuf\Rules' -Name 'PeriodInNanoSeconds' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Siuf\Rules' -Name 'DoNotShowFeedbackNotifications' -ErrorAction SilentlyContinue
# Restore writing habits
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization' -Name 'RestrictImplicitTextCollection' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\InputPersonalization' -Name 'RestrictImplicitInkCollection' -ErrorAction SilentlyContinue

Write-Output 'OK'
"#, "性能调整已全部还原", "需要管理员权限，请以管理员身份运行 NexBox")
}

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
