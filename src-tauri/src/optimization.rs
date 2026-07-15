use std::os::windows::process::CommandExt;
use std::panic;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::{env, fs, path::Path, path::PathBuf};
use sysinfo::System;
use tauri::Manager;
use winreg::enums::*;
use winreg::RegKey;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
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
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const IDLE_PRIORITY_CLASS: u32 = 0x00000040;
const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x00004000;
const PROCESS_MODE_BACKGROUND_BEGIN: u32 = 0x00100000;
const IO_PRIORITY_VERY_LOW: u32 = 0;
const PROCESS_MEMORY_PRIORITY_NEW: u32 = 0;
const PROCESS_MEMORY_PRIORITY_OLD: u32 = 11;
const MEMORY_PRIORITY_VERY_LOW: u32 = 1;

#[link(name = "ntdll")]
extern "system" {
    fn NtSetInformationProcess(
        ProcessHandle: HANDLE,
        ProcessInformationClass: u32,
        ProcessInformation: *const std::ffi::c_void,
        ProcessInformationSize: u32,
    ) -> i32;
}

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
        let handle = OpenProcess(
            PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            pid,
        );
        if handle.is_null() {
            return false;
        }

        let mut applied = false;

        // 1) CPU: BELOW_NORMAL 优先级
        if SetPriorityClass(handle, BELOW_NORMAL_PRIORITY_CLASS) != 0 {
            applied = true;
        }

        // 2) 后台 I/O + 内存：先尝试 PROCESS_MODE_BACKGROUND_BEGIN（旧版 Win11）
        if SetPriorityClass(handle, PROCESS_MODE_BACKGROUND_BEGIN) != 0 {
            SetPriorityClass(handle, BELOW_NORMAL_PRIORITY_CLASS);
        } else {
            // Build 26200+：独立 API 逐项设置
            // I/O 优先级 → VeryLow
            let io: u32 = IO_PRIORITY_VERY_LOW;
            let nt = NtSetInformationProcess(
                handle,
                33,
                &io as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
            );
            if nt != 0 {
                log::warn!("I/O priority failed (pid={}, nt={})", pid, nt);
            }

            // 内存优先级 → VeryLow：新 SDK class=0，回退旧 SDK class=11
            let mem: u32 = MEMORY_PRIORITY_VERY_LOW;
            let mut mem_ok = false;
            for cls in &[PROCESS_MEMORY_PRIORITY_NEW, PROCESS_MEMORY_PRIORITY_OLD] {
                if SetProcessInformation(
                    handle,
                    *cls,
                    &mem as *const _ as *const std::ffi::c_void,
                    std::mem::size_of::<u32>() as u32,
                ) != 0
                {
                    mem_ok = true;
                    break;
                }
            }
            if !mem_ok {
                let err = GetLastError();
                log::warn!("Memory priority failed (pid={}, err={})", pid, err);
            }
        }

        CloseHandle(handle);
        applied
    }
}

fn set_process_low_priority(pid: u32) -> bool {
    unsafe {
        let handle = OpenProcess(PROCESS_SET_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }

        let ok = if SetPriorityClass(handle, IDLE_PRIORITY_CLASS) == 0 {
            false
        } else {
            true
        };

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
            id: "7.9gb".to_string(),
            label: "7.9 GB".to_string(),
            limit_gb: 7.9,
            min_physical_gb: 0.0,
        },
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

use tauri_plugin_store::StoreExt;

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

// ===== ACE 自动检测与优化 =====

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct AceAutoDetectConfig {
    pub enabled: bool,
}

#[derive(serde::Serialize, Clone, Debug, Default)]
pub struct AceAutoDetectStats {
    pub is_running: bool,
    pub last_check: Option<String>,
    pub total_optimized: u32,
    pub currently_optimized: Vec<String>,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct AceAutoDetectStatus {
    pub enabled: bool,
    pub is_running: bool,
    pub last_check: Option<String>,
    pub total_optimized: u32,
    pub currently_optimized: Vec<String>,
}

static AUTO_DETECT_CONFIG: Mutex<Option<AceAutoDetectConfig>> = Mutex::new(None);
static AUTO_DETECT_GENERATION: AtomicU64 = AtomicU64::new(0);
static AUTO_DETECT_STATS: Mutex<Option<AceAutoDetectStats>> = Mutex::new(None);

// 内存中的 enabled 状态，避免 store 读取竞争
static AUTO_DETECT_ENABLED: AtomicBool = AtomicBool::new(false);

const ACE_PROCESS_NAMES: &[&str] = &["ACE-Tray.exe", "SGuard64.exe", "SGuardSvc64.exe"];
const ACE_DETECT_INTERVAL_SECS: u64 = 5;

fn update_auto_detect_stats(optimized: Vec<String>) {
    let mut stats = AUTO_DETECT_STATS.lock().unwrap();
    if stats.is_none() {
        *stats = Some(AceAutoDetectStats::default());
    }
    if let Some(ref mut s) = *stats {
        s.is_running = true;
        s.last_check = Some(
            chrono::Local::now()
                .to_rfc3339()
        );
        s.total_optimized = s.total_optimized.saturating_add(optimized.len() as u32);
        s.currently_optimized = optimized;
    }
}

fn set_auto_detect_running(running: bool) {
    let mut stats = AUTO_DETECT_STATS.lock().unwrap();
    if let Some(ref mut s) = *stats {
        s.is_running = running;
    }
}

fn detect_and_optimize_ace_processes() -> Vec<String> {
    let mut optimized = Vec::new();
    
    let mut system = System::new();
    system.refresh_processes();
    
    for (_, process) in system.processes() {
        let name = process.name().to_string();
        let name_lower = name.to_lowercase();
        
        if ACE_PROCESS_NAMES.iter().any(|n| n.to_lowercase() == name_lower) {
            let mut this_optimized = false;
            
            // 1. 启用调度优化（BELOW_NORMAL 优先级 + I/O VeryLow + 低内存优先级）
            if enable_process_efficiency_mode(process.pid().as_u32()) || set_process_low_priority(process.pid().as_u32()) {
                this_optimized = true;
            }
            
            // 2. 限制亲和性为 CPU0 (affinity = 1)
            if restrict_process_affinity_powershell(&name) {
                this_optimized = true;
            }
            
            if this_optimized {
                optimized.push(name);
            }
        }
    }
    
    optimized
}

fn restrict_process_affinity_powershell(process_name: &str) -> bool {
    let name_without_ext = process_name.trim_end_matches(".exe");
    let ps_script = format!(
        r#"
        $proc = Get-Process -Name "{}" -ErrorAction SilentlyContinue
        if ($proc) {{
            foreach ($p in $proc) {{
                try {{
                    $p.ProcessorAffinity = 1
                    Write-Host "AFFINITY_SET:{}"
                }} catch {{
                    Write-Host "AFFINITY_FAILED:{}"
                }}
            }}
            exit 0
        }} else {{
            Write-Host "NOT_FOUND"
            exit 1
        }}
        "#,
        name_without_ext, name_without_ext, name_without_ext
    );
    
    let result = Command::new("powershell")
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    
    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            stdout.contains("AFFINITY_SET")
        }
        Err(_) => false,
    }
}

fn ace_auto_detect_loop(config: AceAutoDetectConfig, generation: u64) {
    // 启动时立即标记为运行中
    set_auto_detect_running(true);
    
    loop {
        if AUTO_DETECT_GENERATION.load(Ordering::Relaxed) != generation {
            break;
        }
        
        thread::sleep(Duration::from_secs(ACE_DETECT_INTERVAL_SECS));
        
        if AUTO_DETECT_GENERATION.load(Ordering::Relaxed) != generation {
            break;
        }
        
        if !config.enabled {
            continue;
        }
        
        let optimized = detect_and_optimize_ace_processes();
        update_auto_detect_stats(optimized);
    }
    set_auto_detect_running(false);
}

async fn load_persisted_config(app: &tauri::AppHandle) -> AceAutoDetectConfig {
    match app.store("ace_auto_detect.json") {
        Ok(store) => {
            if let Some(value) = store.get("config") {
                if let Ok(config) = serde_json::from_value::<AceAutoDetectConfig>(value) {
                    return config;
                }
            }
        }
        Err(e) => {
            log::warn!("Failed to open ace_auto_detect store: {}", e);
        }
    }
    AceAutoDetectConfig::default()
}

async fn save_persisted_config(app: &tauri::AppHandle, config: &AceAutoDetectConfig) {
    match app.store("ace_auto_detect.json") {
        Ok(store) => {
            store.set("config", serde_json::to_value(config).unwrap());
            if let Err(e) = store.save() {
                log::error!("Failed to save ace_auto_detect config: {}", e);
            }
        }
        Err(e) => {
            log::error!("Failed to open ace_auto_detect store for saving: {}", e);
        }
    }
}

#[tauri::command]
pub async fn set_ace_auto_detect(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    // 读取当前内存状态，避免重复操作
    let current = AUTO_DETECT_ENABLED.load(Ordering::Relaxed);
    if current == enabled {
        return Ok(()); // 状态未变，无需处理
    }
    
    let gen = AUTO_DETECT_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    
    let config = AceAutoDetectConfig { enabled };
    
    {
        let mut cfg = AUTO_DETECT_CONFIG.lock().map_err(|e| e.to_string())?;
        *cfg = Some(config.clone());
    }
    
    // 更新内存状态（立即生效，供 status 读取）
    AUTO_DETECT_ENABLED.store(enabled, Ordering::Relaxed);
    
    // 持久化保存（异步，不阻塞）
    save_persisted_config(&app, &config).await;
    
    if enabled {
        thread::spawn(move || {
            // 捕获 panic，防止线程意外退出
            let _ = panic::catch_unwind(|| {
                ace_auto_detect_loop(config, gen);
            });
            set_auto_detect_running(false);
        });
    } else {
        set_auto_detect_running(false);
    }
    
    Ok(())
}

#[tauri::command]
pub async fn get_ace_auto_detect_status(_app: tauri::AppHandle) -> Result<AceAutoDetectStatus, String> {
    // 直接读取内存状态，避免 store 读取竞争
    let enabled = AUTO_DETECT_ENABLED.load(Ordering::Relaxed);
    
    let stats = AUTO_DETECT_STATS.lock().map_err(|e| e.to_string())?;
    let stats = stats.clone().unwrap_or_default();
    
    Ok(AceAutoDetectStatus {
        enabled,
        is_running: stats.is_running && enabled,
        last_check: stats.last_check,
        total_optimized: stats.total_optimized,
        currently_optimized: stats.currently_optimized,
    })
}

#[tauri::command]
pub async fn init_ace_auto_detect(app: tauri::AppHandle) -> Result<(), String> {
    let config = load_persisted_config(&app).await;
    
    // 初始化内存状态
    AUTO_DETECT_ENABLED.store(config.enabled, Ordering::Relaxed);
    
    if config.enabled {
        let gen = AUTO_DETECT_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
        
        {
            let mut cfg = AUTO_DETECT_CONFIG.lock().map_err(|e| e.to_string())?;
            *cfg = Some(config.clone());
        }
        
        thread::spawn(move || {
            let _ = panic::catch_unwind(|| {
                ace_auto_detect_loop(config, gen);
            });
            set_auto_detect_running(false);
        });
    }
    
    Ok(())
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

    let process_names = ["ACE-Tray.exe", "SGuard64.exe", "SGuardSvc64.exe"];
    let mut count = 0u32;

    let mut system = System::new();
    system.refresh_processes();

    for (_, process) in system.processes() {
        let name = process.name().to_string();
        let name_lower = name.to_lowercase();
        if process_names.iter().any(|n| n.to_lowercase() == name_lower) {
            if set_process_low_priority(process.pid().as_u32()) {
                count += 1;
            }
        }
    }

    Ok(AcePartialResult {
        success: count > 0,
        message: if count > 0 {
            format!("已限制 {} 个 ACE 进程优先级", count)
        } else {
            "未找到运行中的 ACE 进程".to_string()
        },
        count,
    })
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

#[tauri::command]
pub async fn boost_delta_force_affinity_with_mask(mask: u64) -> Result<PriorityResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let ps_script = format!(
        r#"
        $proc = Get-Process -Name "DeltaForceClient-Win64-Shipping" -ErrorAction SilentlyContinue
        if ($proc) {{
            $proc.ProcessorAffinity = {}
            Write-Host "AFFINITY_SET"
            exit 0
        }} else {{
            Write-Host "NOT_FOUND"
            exit 1
        }}
        "#,
        mask
    );

    let result = Command::new("powershell")
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            if stdout.contains("AFFINITY_SET") {
                Ok(PriorityResult {
                    success: true,
                    message: "三角洲进程已设置为使用指定处理器核心".to_string(),
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

#[tauri::command]
pub async fn restrict_ace_affinity_with_mask(mask: u64) -> Result<AcePartialResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let ps_script = format!(
        r#"
        $count = 0
        $processNames = @("ACE-Tray", "SGuard64", "SGuardSvc64")
        foreach ($name in $processNames) {{
            $processes = Get-Process -Name $name -ErrorAction SilentlyContinue
            if ($processes) {{
                foreach ($proc in $processes) {{
                    try {{
                        $proc.ProcessorAffinity = {}
                        $count++
                    }} catch {{}}
                }}
            }}
        }}
        Write-Host "RESTRICTED:$count"
        if ($count -gt 0) {{ exit 0 }} else {{ exit 1 }}
        "#,
        mask
    );

    let result = Command::new("powershell")
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_script])
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
                message: if count > 0 { format!("已限制 {} 个 ACE 进程使用指定核心", count) } else { "未找到运行中的 ACE 进程".to_string() },
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

/// 通用 PowerShell 脚本执行工具：将脚本写入临时 .ps1 文件并通过 -File 参数执行，
/// 避免 Windows 命令行长度限制（错误 206）。
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

/// 执行 PowerShell 脚本并返回统一结果，自动处理权限错误
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

// === Windows Update Disable/Enable (Pure Rust, no PowerShell) ===

/// Convert a Rust string to a null-terminated wide string for Windows API calls.
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Helper: clear failure actions for a service (prevents auto-restart/reboot).
/// Uses sc.exe to reset failure actions.
fn clear_service_failure_actions(service_name: &str) -> Result<(), String> {
    // sc.exe failure <svc> reset=0 actions=""
    let result = std::process::Command::new("sc.exe")
        .args(&["failure", service_name, "reset=", "0", "actions=", ""])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行 sc.exe 失败: {}", e))?;

    if !result.status.success() {
        let err = String::from_utf8_lossy(&result.stderr);
        if !err.trim().is_empty() && !err.contains("FAIL") {
            // Non-fatal for some services
            log::info!("sc.exe failure {} 输出: {}", service_name, err.trim());
        }
    }
    Ok(())
}

/// Helper: grant Administrators write access to a protected service registry key.
/// Uses PowerShell .NET API to take ownership and grant access (the only reliable way
/// for ACL-protected keys like wuauserv).
fn grant_service_key_access(service_name: &str) -> Result<(), String> {
    let ps_script = format!(
        r#"
        $key = [Microsoft.Win32.Registry]::LocalMachine.OpenSubkey('SYSTEM\CurrentControlSet\Services\{}', [Microsoft.Win32.RegistryKeyPermissionCheck]::ReadWriteSubTree, [System.Security.AccessControl.RegistryRights]::TakeOwnership)
        $acl = $key.GetAccessControl()
        $me = [System.Security.Principal.NTAccount]'Administrators'
        $acl.SetOwner($me)
        $key.SetAccessControl($acl)
        $acl = $key.GetAccessControl()
        $rule = New-Object System.Security.AccessControl.RegistryAccessRule($me, 'FullControl', 'ContainerInherit', 'None', 'Allow')
        $acl.SetAccessRule($rule)
        $key.SetAccessControl($acl)
        $key.Close()
        "#,
        service_name
    );

    let output = std::process::Command::new("powershell.exe")
        .args(&["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行 PowerShell 权限获取失败: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let out = String::from_utf8_lossy(&output.stdout);
        let msg = if err.trim().is_empty() { out.trim() } else { err.trim() };
        log::warn!("grant_service_key_access {} 输出: {}", service_name, msg);
        // Don't fail — try reg add anyway
    }
    Ok(())
}

/// Helper: set a service start type via reg.exe with PowerShell ownership fallback.
/// start_type: 2=auto, 3=demand, 4=disabled
fn set_service_start_reg(service_name: &str, start_type: u32) -> Result<(), String> {
    // First try: direct reg add
    let cmd = format!(
        "reg add \"HKLM\\SYSTEM\\CurrentControlSet\\Services\\{}\" /v Start /t REG_DWORD /d {} /f",
        service_name, start_type
    );
    let output = std::process::Command::new("cmd.exe")
        .args(&["/c", &cmd])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行 cmd/reg 失败: {}", e))?;

    if output.status.success() {
        return Ok(());
    }

    // First try failed — take ownership via PowerShell .NET API and retry
    log::info!("reg add 直接写入失败，尝试获取注册表键所有权...");
    let _ = grant_service_key_access(service_name);

    // Retry reg add after taking ownership
    let output2 = std::process::Command::new("cmd.exe")
        .args(&["/c", &cmd])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行 cmd/reg (retry) 失败: {}", e))?;

    if !output2.status.success() {
        let err = String::from_utf8_lossy(&output2.stderr);
        let out = String::from_utf8_lossy(&output2.stdout);
        let msg = if err.trim().is_empty() { out.trim() } else { err.trim() };
        log::error!("设置服务 {} Start={} 最终失败: {}", service_name, start_type, msg);
        // Return Ok anyway — non-fatal, the policy + schtasks still apply
        return Ok(());
    }
    log::info!("服务 {} Start={} 设置成功 (retry)", service_name, start_type);
    Ok(())
}

/// Helper: control a service (stop/start).
unsafe fn control_service(service_name: &str, control: u32) -> Result<(), String> {
    use windows_sys::Win32::System::Services::{
        OpenSCManagerW, OpenServiceW, ControlService, CloseServiceHandle, StartServiceW,
        QueryServiceStatus,
        SC_MANAGER_CONNECT, SERVICE_STOP, SERVICE_START,
        SERVICE_QUERY_STATUS, SERVICE_STOPPED,
        SERVICE_CONTROL_STOP,
    };

    let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT);
    if scm.is_null() {
        return Err(format!("无法打开 SCM (服务控制)"));
    }

    let svc_name = to_wide(service_name);
    let access = if control == SERVICE_CONTROL_STOP { SERVICE_STOP | SERVICE_QUERY_STATUS }
                 else { SERVICE_START | SERVICE_QUERY_STATUS };
    let svc = OpenServiceW(scm, svc_name.as_ptr(), access);
    if svc.is_null() {
        CloseServiceHandle(scm);
        // Service not found or not accessible – not fatal for disable
        return Ok(());
    }

    if control == SERVICE_CONTROL_STOP {
        // Query status first to see if it's running
        let mut status: windows_sys::Win32::System::Services::SERVICE_STATUS = std::mem::zeroed();
        let qs_ret = QueryServiceStatus(svc, &mut status);
        if qs_ret != 0 && status.dwCurrentState != SERVICE_STOPPED {
            let mut s = std::mem::zeroed();
            ControlService(svc, SERVICE_CONTROL_STOP, &mut s);
        }
    } else {
        // Start
        StartServiceW(svc, 0, std::ptr::null_mut());
    }

    CloseServiceHandle(svc);
    CloseServiceHandle(scm);
    Ok(())
}

/// Helper: kill a process by name.
fn kill_process(name: &str) {
    let _ = std::process::Command::new("taskkill")
        .args(&["/f", "/im", name])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

/// Helper: run schtasks to disable or enable Windows Update scheduled tasks.
fn schtasks_wu_tasks(enable: bool) -> Result<(), String> {
    let action = if enable { "/enable" } else { "/disable" };

    // Dynamically enumerate all tasks under \Microsoft\Windows\WindowsUpdate
    let output = std::process::Command::new("cmd.exe")
        .args(&["/c", "schtasks /query /fo csv /nh"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行 schtasks /query 失败: {}", e))?;

    let out = String::from_utf8_lossy(&output.stdout);
    let mut task_names: Vec<String> = Vec::new();

    for line in out.lines() {
        // CSV format: "TaskName","Next Run Time","Status"
        if line.to_lowercase().contains("windowsupdate") {
            // Extract task name from CSV (first field)
            if let Some(name) = line.split(',').next() {
                let name = name.trim().trim_matches('"');
                if !name.is_empty() {
                    task_names.push(name.to_string());
                }
            }
        }
    }

    log::info!("找到 {} 个 WindowsUpdate 计划任务: {:?}", task_names.len(), task_names);

    if task_names.is_empty() {
        log::warn!("未找到任何 WindowsUpdate 计划任务，跳过");
        return Ok(());
    }

    for task_path in &task_names {
        let result = std::process::Command::new("schtasks")
            .args(&["/change", "/tn", task_path, action])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        match result {
            Ok(output) => {
                let out_str = String::from_utf8_lossy(&output.stdout);
                let err_str = String::from_utf8_lossy(&output.stderr);
                if output.status.success() {
                    log::info!("schtasks {} {} -> 成功", action, task_path);
                } else {
                    log::warn!("schtasks {} {} -> {} {}", action, task_path, out_str.trim(), err_str.trim());
                }
            }
            Err(e) => {
                log::error!("schtasks 调用失败: {}", e);
            }
        }
    }

    Ok(())
}

/// Check if any WU scheduled task is disabled.
fn check_schtasks_wu_disabled() -> bool {
    // Use PowerShell Get-ScheduledTask — State enum values are always English
    let ps_script = r#"
        $tasks = Get-ScheduledTask -TaskPath '\Microsoft\Windows\WindowsUpdate\*' -ErrorAction SilentlyContinue
        $disabled = $tasks | Where-Object { $_.State -eq 'Disabled' }
        if ($disabled) { Write-Output 'YES' } else { Write-Output 'NO' }
    "#;
    let output = std::process::Command::new("powershell.exe")
        .args(&["-NoProfile", "-NonInteractive", "-Command", ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        return text.trim().eq_ignore_ascii_case("YES");
    }
    false
}

#[tauri::command]
pub async fn disable_windows_update() -> Result<String, String> {
    log::info!("开始关闭 Windows Update...");

    // 1. Stop services
    let services = ["wuauserv", "UsoSvc", "WaaSMedicSvc"];
    for svc in &services {
        log::info!("停止服务: {}", svc);
        unsafe {
            control_service(svc, windows_sys::Win32::System::Services::SERVICE_CONTROL_STOP)
                .unwrap_or_else(|e| log::error!("停止服务 {} 失败: {}", svc, e));
        }
    }

    // 2. Kill UsoClient.exe
    log::info!("终止 UsoClient.exe 进程");
    kill_process("UsoClient.exe");

    // 3. Set service start types to disabled (4) via registry (bypass SCM protection)
    for svc in &services {
        log::info!("禁用服务启动: {}", svc);
        set_service_start_reg(svc, 4)
            .unwrap_or_else(|e| log::error!("禁用 {} 服务启动失败: {}", svc, e));
        // Verify the write
        let after = get_service_start(svc);
        log::info!("服务 {} 写入后 Start = {:?}", svc, after);
        clear_service_failure_actions(svc)
            .unwrap_or_else(|e| log::error!("清空失败恢复 {} 失败: {}", svc, e));
    }

    // 4. Registry: Set NoAutoUpdate=1, AUOptions=1
    log::info!("设置注册表策略: NoAutoUpdate, AUOptions");
    let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
    let (wu_key, _) = hklm.create_subkey(
        r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU"
    ).map_err(|e| format!("创建/打开注册表键失败: {}", e))?;
    wu_key.set_value("NoAutoUpdate", &1u32).map_err(|e| format!("设置 NoAutoUpdate 失败: {}", e))?;
    wu_key.set_value("AUOptions", &1u32).map_err(|e| format!("设置 AUOptions 失败: {}", e))?;

    // Optional: DisableWindowsUpdateAccess for Pro/Enterprise
    let (wu_policy_key, _) = hklm.create_subkey(
        r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate"
    ).map_err(|e| format!("创建 WindowsUpdate 策略键失败: {}", e))?;
    wu_policy_key.set_value("DisableWindowsUpdateAccess", &1u32)
        .unwrap_or_else(|e| log::info!("DisableWindowsUpdateAccess 设置跳过（非 Pro/Enterprise 系统）: {}", e));

    // 5. Disable scheduled tasks
    log::info!("禁用 Windows Update 计划任务");
    schtasks_wu_tasks(false)?;

    log::info!("Windows Update 关闭完成");
    Ok("ok".to_string())
}

#[tauri::command]
pub async fn enable_windows_update() -> Result<String, String> {
    log::info!("开始恢复 Windows Update...");

    // 1. Restore service start types via registry: wuauserv=3 (demand), UsoSvc=2 (auto), WaaSMedicSvc=2 (auto)
    let services_config = [
        ("wuauserv", 3u32),
        ("UsoSvc", 2u32),
        ("WaaSMedicSvc", 2u32),
    ];

    for (svc, start_type) in &services_config {
        log::info!("恢复服务启动类型: {} -> {}", svc, start_type);
        set_service_start_reg(svc, *start_type)
            .unwrap_or_else(|e| log::error!("恢复 {} 服务启动类型失败: {}", svc, e));
    }

    // 2. Registry: Delete policy keys
    log::info!("删除注册表策略键值");
    let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);

    // Delete NoAutoUpdate and AUOptions
    if let Ok(wu_key) = hklm.open_subkey_with_flags(
        r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU",
        winreg::enums::KEY_SET_VALUE
    ) {
        let _ = wu_key.delete_value("NoAutoUpdate");
        let _ = wu_key.delete_value("AUOptions");
    }

    // Delete DisableWindowsUpdateAccess
    if let Ok(wu_policy_key) = hklm.open_subkey_with_flags(
        r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate",
        winreg::enums::KEY_SET_VALUE
    ) {
        let _ = wu_policy_key.delete_value("DisableWindowsUpdateAccess");
    }

    // 3. Enable scheduled tasks
    log::info!("启用 Windows Update 计划任务");
    schtasks_wu_tasks(true)?;

    log::info!("Windows Update 恢复完成");
    Ok("ok".to_string())
}

/// Check service Start value via registry (simpler and more reliable than SCM query).
fn get_service_start(service_name: &str) -> Option<u32> {
    // Use reg query via cmd.exe for reliable reading of ACL-protected keys
    let output = std::process::Command::new("cmd.exe")
        .args(&[
            "/c",
            &format!(
                "reg query \"HKLM\\SYSTEM\\CurrentControlSet\\Services\\{}\" /v Start",
                service_name
            ),
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;

    let out = String::from_utf8_lossy(&output.stdout);
    // Parse: "    Start    REG_DWORD    0x4"
    for line in out.lines() {
        let line = line.trim();
        if line.contains("Start") && line.contains("REG_DWORD") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(last) = parts.last() {
                if let Ok(v) = u32::from_str_radix(last.trim_start_matches("0x"), 16) {
                    return Some(v);
                }
            }
        }
    }
    None
}

#[tauri::command]
pub async fn check_windows_update_state() -> Result<serde_json::Value, String> {
    let services_to_check = ["wuauserv", "UsoSvc", "WaaSMedicSvc"];
    let services_disabled = services_to_check.iter().all(|svc| {
            get_service_start(svc).map_or(false, |st| st == 4)
        });

    // Check registry: NoAutoUpdate == 1?
    let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
    let policy_set = hklm.open_subkey(
        r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU"
    ).and_then(|key| {
        key.get_value::<u32, _>("NoAutoUpdate")
    }).map_or(false, |v| v == 1);

    let scheduler_disabled = check_schtasks_wu_disabled();
    let all_disabled = services_disabled && policy_set && scheduler_disabled;

    let result = serde_json::json!({
        "services_disabled": services_disabled,
        "policy_set": policy_set,
        "scheduler_disabled": scheduler_disabled,
        "all_disabled": all_disabled,
    });

    Ok(result)
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

#[derive(serde::Serialize)]
pub struct PeripheralStatus {
    pub mouse_value: Option<i32>,
    pub keyboard_value: Option<i32>,
}

#[tauri::command]
pub async fn get_peripheral_status() -> Result<PeripheralStatus, String> {
    let script = r#"
$ErrorActionPreference = 'SilentlyContinue'
$mouse = Get-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\PriorityControl' -Name 'Win32PrioritySeparation' -ErrorAction SilentlyContinue
$keyboard = Get-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\Kbdclass\Parameters' -Name 'KeyboardDataQueueSize' -ErrorAction SilentlyContinue
if ($mouse -ne $null) { Write-Output "MOUSE:$($mouse.Win32PrioritySeparation)" } else { Write-Output "MOUSE:null" }
if ($keyboard -ne $null) { Write-Output "KEYBOARD:$($keyboard.KeyboardDataQueueSize)" } else { Write-Output "KEYBOARD:null" }
"#;
    let result = run_ps_script(script)?;
    let stdout = String::from_utf8_lossy(&result.stdout).to_string();
    let mut mouse_value: Option<i32> = None;
    let mut keyboard_value: Option<i32> = None;
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("MOUSE:") {
            mouse_value = if val == "null" { None } else { val.trim().parse().ok() };
        } else if let Some(val) = line.strip_prefix("KEYBOARD:") {
            keyboard_value = if val == "null" { None } else { val.trim().parse().ok() };
        }
    }
    Ok(PeripheralStatus { mouse_value, keyboard_value })
}

#[tauri::command]
pub async fn set_peripheral_settings(mouse_value: u32, keyboard_value: u32) -> Result<PerfTweakResult, String> {
    let script = format!(r#"
$ErrorActionPreference = 'SilentlyContinue'
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\PriorityControl' -Name 'Win32PrioritySeparation' -Value {mouse} -Type DWord -Force -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\Kbdclass\Parameters' -Name 'KeyboardDataQueueSize' -Value {keyboard} -Type DWord -Force -ErrorAction SilentlyContinue
Write-Output 'OK'
"#, mouse = mouse_value, keyboard = keyboard_value);
    run_simple_feature(&script)
}

#[tauri::command]
pub async fn reset_peripheral_settings() -> Result<PerfTweakResult, String> {
    run_simple_feature(r#"
$ErrorActionPreference = 'SilentlyContinue'
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\PriorityControl' -Name 'Win32PrioritySeparation' -ErrorAction SilentlyContinue
Remove-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\Kbdclass\Parameters' -Name 'KeyboardDataQueueSize' -ErrorAction SilentlyContinue
Write-Output 'OK'
"#)
}

// ========== AQ_REGISTRY 模块 - 纯 Rust 注册表操作（零外部进程） ==========
// 感谢 1U 工具箱提供系统优化支持

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
        Ok(String::from_utf16_lossy(&u16s))
    } else if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        // UTF-8 BOM
        Ok(String::from_utf8_lossy(&bytes[3..]).to_string())
    } else {
        // 纯 UTF-8
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }
}

/// 解析 .reg 文件路径（适配开发模式和打包模式）
fn resolve_reg_path(name: &str, is_restore: bool) -> Result<PathBuf, String> {
    let (dir, suffix) = if is_restore {
        ("aq_registry_restore", ".restore.reg")
    } else {
        ("aq_registry", ".reg")
    };

    // 开发模式下从项目根目录查找
    if let Ok(cwd) = std::env::current_dir() {
        let dev_candidates = [
            cwd.join(dir).join(format!("{}{}", name, suffix)),
            cwd.join("..").join(dir).join(format!("{}{}", name, suffix)),
            cwd.join("..").join("..").join(dir).join(format!("{}{}", name, suffix)),
        ];
        for path in &dev_candidates {
            if path.exists() {
                return Ok(path.clone());
            }
        }
    }

    // 打包模式下从 exe 同级目录查找
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
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
        }
    }

    Err(format!("未找到注册表文件: {}{}", name, suffix))
}

/// 解析 .reg 文件内容并直接通过 winreg 写入注册表
/// 支持：[HKEY_...] 键路径 / "Name"=dword:XXX / "Name"="string" / "Name"=- 删除值
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

    let (key, _) = root
        .create_subkey(subpath)
        .map_err(|e| format!("创建注册表键失败: {} - {}", path, e))?;

    Ok(key)
}

/// 应用单个注册表优化（读取 aq_registry/<name>.reg 并通过 winreg 写入）
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

/// 恢复单个注册表优化（读取 aq_registry_restore/<name>.restore.reg 并通过 winreg 写入）
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

/// 批量应用注册表优化
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
        let failed_names: Vec<String> = failed.iter().map(|(n, _)| n.clone()).collect();
        Ok(PerfTweakResult {
            success: failed.len() < names.len(),
            message: format!(
                "成功 {} 项，失败 {} 项: {}",
                success_count,
                failed.len(),
                failed_names.join(", ")
            ),
        })
    }
}

/// 批量恢复注册表优化
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
        let failed_names: Vec<String> = failed.iter().map(|(n, _)| n.clone()).collect();
        Ok(PerfTweakResult {
            success: failed.len() < names.len(),
            message: format!(
                "成功 {} 项，失败 {} 项: {}",
                success_count,
                failed.len(),
                failed_names.join(", ")
            ),
        })
    }
}

/// 重启显卡驱动（模拟 Win+Ctrl+Shift+B）
/// 该快捷键会触发 Windows 图形栈重置，适用于网吧用户需要快速恢复显示异常的场景
#[tauri::command]
pub fn restart_graphics_driver() -> Result<PerfTweakResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    unsafe {
        use winapi::um::winuser::{
            keybd_event, KEYEVENTF_KEYUP,
            VK_LCONTROL, VK_LSHIFT, VK_LWIN,
        };

        const VK_B: u8 = 0x42;

        // 按下组合键：Win + Ctrl + Shift + B
        keybd_event(VK_LWIN as u8, 0, 0, 0);
        keybd_event(VK_LCONTROL as u8, 0, 0, 0);
        keybd_event(VK_LSHIFT as u8, 0, 0, 0);
        keybd_event(VK_B, 0, 0, 0);

        // 短暂延迟确保系统注册该组合键
        std::thread::sleep(std::time::Duration::from_millis(100));

        // 释放按键（逆序）：B, Shift, Ctrl, Win
        keybd_event(VK_B, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_LSHIFT as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_LCONTROL as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_LWIN as u8, 0, KEYEVENTF_KEYUP, 0);
    }

    Ok(PerfTweakResult {
        success: true,
        message: "已发送重启显卡驱动指令，屏幕可能会短暂闪烁".to_string(),
    })
}
