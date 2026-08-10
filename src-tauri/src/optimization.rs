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
use windows_sys::Win32::System::ProcessStatus::{
    EmptyWorkingSet, K32EnumProcesses, K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
};
use windows_sys::Win32::System::Memory::SetSystemFileCacheSize;
use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, SetPriorityClass, SetProcessAffinityMask,
    SetProcessWorkingSetSize, PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA, PROCESS_VM_READ,
};

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
const REALTIME_PRIORITY_CLASS: u32 = 0x00000100;
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

/// 设置进程优先级为实时（对应 .NET ProcessPriorityClass::RealTime）
fn set_process_realtime_priority(pid: u32) -> bool {
    unsafe {
        let handle = OpenProcess(PROCESS_SET_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let ok = SetPriorityClass(handle, REALTIME_PRIORITY_CLASS) != 0;
        CloseHandle(handle);
        ok
    }
}

/// 设置进程 CPU 亲和性掩码（直接 Win32 API，无需 PowerShell）
fn set_process_affinity(pid: u32, mask: u64) -> bool {
    unsafe {
        let handle = OpenProcess(PROCESS_SET_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        // SetProcessAffinityMask 第二参数为 usize（ULONG_PTR），64 位系统为 u64
        let ok = SetProcessAffinityMask(handle, mask as usize) != 0;
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
    let mut sys = System::new();
    sys.refresh_memory();

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
        // 原生并行清理：待机缓存 + 全进程工作集收紧，无需 PowerShell
        thread::scope(|s| {
            s.spawn(|| {
                clean_standby_memory_inner();
            });
            s.spawn(|| {
                trim_working_set_inner();
            });
        });

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

/// 已知的高性能/Normal GUID（系统内置计划）。
const KNOWN_HIGH_PERF_GUID: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";

/// 通过名称关键词匹配，在系统电源计划中查找高性能方案。
fn find_high_performance_guid(plans: &[(String, String, bool)]) -> Option<String> {
    // 优先级：卓越性能 > 高性能/Ultimate
    let candidates: &[&[&str]] = &[
        &["卓越性能", "Ultimate Performance"],
        &["高性能", "High performance", "Ultimate"],
    ];
    for keywords in candidates {
        for (guid, name, _) in plans {
            let lower = name.to_lowercase();
            if keywords.iter().any(|kw| lower.contains(&kw.to_lowercase())) {
                return Some(guid.clone());
            }
        }
    }
    None
}

#[tauri::command]
pub async fn set_high_performance_power_plan() -> Result<PowerPlanResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    // 1. 记录当前计划（直接调 powercfg，不经过 PowerShell）
    let previous_plan = get_active_plan_internal()
        .map(|(guid, name)| format!("{} ({})", name, guid));

    // 2. 枚举所有系统计划，尝试按名称匹配高性能方案
    let system_plans = get_system_plans_internal();
    let target_guid = find_high_performance_guid(&system_plans)
        .unwrap_or_else(|| KNOWN_HIGH_PERF_GUID.to_string());

    // 3. 直接调用 powercfg /setactive
    let result = run_powercfg(&["/setactive", &target_guid]);

    match result {
        Ok(output) if output.status.success() => {
            // 4. 验证切换结果（直接调 powercfg）
            let current_plan = match get_active_plan_internal() {
                Some((guid, name)) => format!("{} ({})", name, guid),
                None => "高性能".to_string(),
            };

            Ok(PowerPlanResult {
                success: true,
                message: "已切换到高性能电源计划".to_string(),
                previous_plan,
                current_plan,
            })
        }
        Ok(output) => {
            let error_msg = String::from_utf8_lossy(&output.stderr).to_string();
            Err(format!("切换电源计划失败: {}", error_msg))
        }
        Err(e) => Err(format!("执行电源计划切换命令失败: {}", e)),
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
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let mut optimized_processes = Vec::new();
    let mut system = System::new();
    system.refresh_processes();

    for (_, process) in system.processes() {
        let name = process.name().to_string();
        let name_lower = name.to_lowercase();
        if ACE_PROCESS_NAMES.iter().any(|n| n.to_lowercase() == name_lower) {
            let pid = process.pid().as_u32();
            let priority_ok = set_process_low_priority(pid);
            let affinity_ok = set_process_affinity(pid, 1);
            if priority_ok || affinity_ok {
                optimized_processes.push(name);
            }
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

#[derive(serde::Serialize)]
pub struct AceEfficiencyResult {
    pub success: bool,
    pub message: String,
    pub count: u32,
    pub found_count: u32,
}

#[tauri::command]
pub async fn set_ace_efficiency_mode() -> Result<AceEfficiencyResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let mut found = 0u32;
    let mut count = 0u32;

    let mut system = System::new();
    system.refresh_processes();

    for (_, process) in system.processes() {
        let name = process.name().to_string();
        let name_lower = name.to_lowercase();
        if ACE_PROCESS_NAMES.iter().any(|n| n.to_lowercase() == name_lower) {
            found += 1;
            if enable_process_efficiency_mode(process.pid().as_u32()) {
                count += 1;
            }
        }
    }

    Ok(AceEfficiencyResult {
        success: count > 0,
        message: ace_message(found, count, "已为 {} 个 ACE 进程开启效能模式"),
        count,
        found_count: found,
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
    let mut sys = System::new();
    sys.refresh_memory();

    let physical_total = sys.total_memory() / 1024 / 1024;
    let physical_available = sys.available_memory() / 1024 / 1024;
    let physical_used = physical_total - physical_available;

    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    // 原生获取虚拟内存 + 全进程工作集总和（GlobalMemoryStatusEx + EnumProcesses），无需 PowerShell
    let mut virtual_total: u64 = 0;
    let mut virtual_available: u64 = 0;
    let mut working_set_used: u64 = 0;

    unsafe {
        let mut status: MEMORYSTATUSEX = std::mem::zeroed();
        status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut status) != 0 {
            virtual_total = status.ullTotalPageFile / 1024 / 1024;
            virtual_available = status.ullAvailPageFile / 1024 / 1024;
        }

        let mut pids: [u32; 8192] = [0; 8192];
        let mut needed: u32 = 0;
        if K32EnumProcesses(
            pids.as_mut_ptr(),
            std::mem::size_of_val(&pids) as u32,
            &mut needed,
        ) != 0
        {
            let count = ((needed as usize) / std::mem::size_of::<u32>()).min(pids.len());
            let access = PROCESS_QUERY_INFORMATION | PROCESS_VM_READ;
            let pmc_size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            for &pid in &pids[..count] {
                if pid == 0 {
                    continue;
                }
                let handle = OpenProcess(access, 0, pid);
                if handle.is_null() {
                    continue;
                }
                let mut pmc: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
                if K32GetProcessMemoryInfo(handle, &mut pmc, pmc_size) != 0 {
                    working_set_used += (pmc.WorkingSetSize / 1024 / 1024) as u64;
                }
                CloseHandle(handle);
            }
        }
    }

    let virtual_used = virtual_total.saturating_sub(virtual_available);
    let working_set_total = physical_total;
    let working_set_available = working_set_total.saturating_sub(working_set_used);

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

#[repr(C)]
struct MemoryPurgeStandbyListCommand {
    next: *mut std::ffi::c_void,
    command: u32,
}

/// 原生清空待机列表（standby list），需要管理员权限，失败返回 false
fn purge_standby_list_native() -> bool {
    #[link(name = "ntdll")]
    extern "system" {
        fn NtSetSystemInformation(
            InformationClass: u32,
            Information: *const std::ffi::c_void,
            Length: u32,
        ) -> i32;
    }
    unsafe {
        const SYSTEM_MEMORY_LIST_INFORMATION: u32 = 80;
        const MEMORY_PURGE_STANDBY_LIST: u32 = 4;
        let mut cmd = MemoryPurgeStandbyListCommand {
            next: std::ptr::null_mut(),
            command: MEMORY_PURGE_STANDBY_LIST,
        };
        NtSetSystemInformation(
            SYSTEM_MEMORY_LIST_INFORMATION,
            &mut cmd as *mut _ as *const std::ffi::c_void,
            std::mem::size_of::<MemoryPurgeStandbyListCommand>() as u32,
        ) == 0
    }
}

/// 原生清理待机内存（standby 文件缓存 + 待机列表），无需 PowerShell
fn clean_standby_memory_inner() -> u64 {
    let before = get_memory_info();

    // 1) 清空待机列表（管理员权限下生效）
    let purged = purge_standby_list_native();

    unsafe {
        // 2) 临时把系统文件缓存上限压到最低，强制回收文件缓存页（usize::MAX 即 -1，表示恢复系统默认）
        SetSystemFileCacheSize(usize::MAX, usize::MAX, 0);
        // 给系统一点时间回收
        thread::sleep(Duration::from_millis(400));
        // 恢复默认文件缓存上限
        SetSystemFileCacheSize(usize::MAX, usize::MAX, 1);

        if !purged {
            // 权限不足时回退：收紧当前进程工作集（原逻辑兜底，保证至少执行一次清理动作）
            let handle = GetCurrentProcess();
            SetProcessWorkingSetSize(handle, usize::MAX, usize::MAX);
        }
    }

    let after = get_memory_info();
    if after.available > before.available {
        after.available - before.available
    } else {
        0
    }
}

/// 原生遍历所有进程并 EmptyWorkingSet（收紧工作集），无需 PowerShell
fn trim_working_set_inner() -> u64 {
    unsafe {
        let mut pids: [u32; 4096] = [0; 4096];
        let mut needed: u32 = 0;
        if K32EnumProcesses(
            pids.as_mut_ptr(),
            std::mem::size_of_val(&pids) as u32,
            &mut needed,
        ) == 0
        {
            return 0;
        }
        let count = ((needed as usize) / std::mem::size_of::<u32>()).min(pids.len());
        let access = PROCESS_QUERY_INFORMATION | PROCESS_SET_QUOTA | PROCESS_VM_READ;
        let pmc_size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let self_pid = std::process::id();
        let mut freed_mb: u64 = 0;

        for &pid in &pids[..count] {
            if pid == 0 || pid == self_pid {
                continue;
            }
            let handle = OpenProcess(access, 0, pid);
            if handle.is_null() {
                continue;
            }
            let mut pmc_before: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
            if K32GetProcessMemoryInfo(handle, &mut pmc_before, pmc_size) == 0 {
                CloseHandle(handle);
                continue;
            }
            EmptyWorkingSet(handle);
            let mut pmc_after: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
            K32GetProcessMemoryInfo(handle, &mut pmc_after, pmc_size);
            CloseHandle(handle);
            if pmc_before.WorkingSetSize > pmc_after.WorkingSetSize {
                freed_mb += ((pmc_before.WorkingSetSize - pmc_after.WorkingSetSize) / 1024 / 1024) as u64;
            }
        }
        freed_mb
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
                    // 原生并行：待机缓存 + 工作集收紧
                    thread::scope(|s| {
                        s.spawn(|| {
                            clean_standby_memory_inner();
                        });
                        s.spawn(|| {
                            trim_working_set_inner();
                        });
                    });
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

const ACE_PROCESS_NAMES: &[&str] = &[
    "ACE-Tray.exe",
    "ACE-BASE.exe",
    "ACE-GAME.exe",
    "ACE-Client.exe",
    "SGuard64.exe",
    "SGuardSvc64.exe",
    "SGuardLite64.exe",
    "SGuardLite.exe",
    "SGuardSvc.exe",
];
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
    let mut found_unauthorized = 0u32;

    let mut system = System::new();
    system.refresh_processes();

    for (_, process) in system.processes() {
        let name = process.name().to_string();
        let name_lower = name.to_lowercase();

        if ACE_PROCESS_NAMES.iter().any(|n| n.to_lowercase() == name_lower) {
            let pid = process.pid().as_u32();
            let mut this_optimized = false;

            // 1. 启用调度优化（BELOW_NORMAL 优先级 + I/O VeryLow + 低内存优先级）
            if enable_process_efficiency_mode(pid) || set_process_low_priority(pid) {
                this_optimized = true;
            }

            // 2. 限制亲和性为 CPU0 (affinity = 1)，直接 Win32 API
            if set_process_affinity(pid, 1) {
                this_optimized = true;
            }

            if this_optimized {
                optimized.push(name);
            } else {
                found_unauthorized += 1;
            }
        }
    }

    if found_unauthorized > 0 {
        log::warn!(
            "[ACE auto-detect] 发现 {} 个 ACE 进程无法修改（需管理员权限），已跳过",
            found_unauthorized
        );
    }

    optimized
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

    let target = "DeltaForceClient-Win64-Shipping.exe";
    let mut system = System::new();
    system.refresh_processes();

    let mut boosted = false;
    for (_, process) in system.processes() {
        if process.name().eq_ignore_ascii_case(target) {
            if set_process_realtime_priority(process.pid().as_u32()) {
                boosted = true;
            }
        }
    }

    if boosted {
        Ok(ProcessOptimizeResult {
            success: true,
            message: "三角洲进程优先级已提升为「超高」（实时）".to_string(),
            process_name: target.to_string(),
            was_running: true,
        })
    } else {
        // 检查是否找到了进程但改不动
        let found = system
            .processes()
            .values()
            .any(|p| p.name().eq_ignore_ascii_case(target));
        Ok(ProcessOptimizeResult {
            success: false,
            message: if found {
                "三角洲进程已运行，但优先级修改失败（请以管理员身份运行本应用）".to_string()
            } else {
                "三角洲游戏未运行，请先启动游戏".to_string()
            },
            process_name: target.to_string(),
            was_running: found,
        })
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

    let target = "DeltaForceClient-Win64-Shipping.exe";
    // 默认掩码：使用除 CPU0 外的所有核心（与原 PowerShell 版一致）
    let num_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let all_cores_mask: u64 = if num_cores >= 64 { u64::MAX } else { (1u64 << num_cores) - 1 };
    let mask = all_cores_mask ^ 1;

    let mut system = System::new();
    system.refresh_processes();

    let mut found = false;
    let mut applied = false;
    for (_, process) in system.processes() {
        if process.name().eq_ignore_ascii_case(target) {
            found = true;
            if set_process_affinity(process.pid().as_u32(), mask) {
                applied = true;
            }
        }
    }

    Ok(PriorityResult {
        success: applied,
        message: if applied {
            "三角洲进程已设置为使用所有处理器核心".to_string()
        } else if found {
            "三角洲进程已运行，但核心分配修改失败（请以管理员身份运行本应用）".to_string()
        } else {
            "三角洲游戏未运行，请先启动游戏".to_string()
        },
        process_name: target.to_string(),
        was_running: found,
    })
}

#[derive(serde::Serialize)]
pub struct AcePartialResult {
    pub success: bool,
    pub message: String,
    pub count: u32,
    pub found_count: u32,
}

/// 根据 found/count 生成统一文案，区分"未找到"、"需要管理员权限"、"部分成功"、"全部成功"
fn ace_message(found: u32, count: u32, ok_template: &str) -> String {
    if found == 0 {
        return "未找到运行中的 ACE 进程".to_string();
    }
    if count == 0 {
        return format!(
            "发现 {} 个 ACE 进程，但无法修改（请以管理员身份运行本应用）",
            found
        );
    }
    if count < found {
        return format!(
            "{}（另有 {} 个需管理员权限）",
            ok_template.replace("{}", &count.to_string()),
            found - count
        );
    }
    ok_template.replace("{}", &count.to_string())
}

#[tauri::command]
pub async fn limit_ace_priority() -> Result<AcePartialResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let mut found = 0u32;
    let mut count = 0u32;

    let mut system = System::new();
    system.refresh_processes();

    for (_, process) in system.processes() {
        let name = process.name().to_string();
        let name_lower = name.to_lowercase();
        if ACE_PROCESS_NAMES.iter().any(|n| n.to_lowercase() == name_lower) {
            found += 1;
            if set_process_low_priority(process.pid().as_u32()) {
                count += 1;
            }
        }
    }

    Ok(AcePartialResult {
        success: count > 0,
        message: ace_message(found, count, "已限制 {} 个 ACE 进程优先级"),
        count,
        found_count: found,
    })
}

#[tauri::command]
pub async fn restrict_ace_affinity() -> Result<AcePartialResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    // 默认掩码 = 1，只使用 CPU0
    restrict_ace_affinity_impl(1, "已限制 {} 个 ACE 进程使用单核心")
}

#[tauri::command]
pub async fn restrict_ace_affinity_with_mask(mask: u64) -> Result<AcePartialResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    restrict_ace_affinity_impl(mask, "已限制 {} 个 ACE 进程使用指定核心")
}

/// ACE 亲和性限制的统一实现：直接 Win32 API，不走 PowerShell
fn restrict_ace_affinity_impl(mask: u64, ok_template: &str) -> Result<AcePartialResult, String> {
    let mut found = 0u32;
    let mut count = 0u32;

    let mut system = System::new();
    system.refresh_processes();

    for (_, process) in system.processes() {
        let name = process.name().to_string();
        let name_lower = name.to_lowercase();
        if ACE_PROCESS_NAMES.iter().any(|n| n.to_lowercase() == name_lower) {
            found += 1;
            if set_process_affinity(process.pid().as_u32(), mask) {
                count += 1;
            }
        }
    }

    Ok(AcePartialResult {
        success: count > 0,
        message: ace_message(found, count, ok_template),
        count,
        found_count: found,
    })
}

#[tauri::command]
pub async fn boost_delta_force_affinity_with_mask(mask: u64) -> Result<PriorityResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let target = "DeltaForceClient-Win64-Shipping.exe";
    let mut system = System::new();
    system.refresh_processes();

    let mut found = false;
    let mut applied = false;
    for (_, process) in system.processes() {
        if process.name().eq_ignore_ascii_case(target) {
            found = true;
            if set_process_affinity(process.pid().as_u32(), mask) {
                applied = true;
            }
        }
    }

    Ok(PriorityResult {
        success: applied,
        message: if applied {
            "三角洲进程已设置为使用指定处理器核心".to_string()
        } else if found {
            "三角洲进程已运行，但核心分配修改失败（请以管理员身份运行本应用）".to_string()
        } else {
            "三角洲游戏未运行，请先启动游戏".to_string()
        },
        process_name: target.to_string(),
        was_running: found,
    })
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

    let delta_target = "DeltaForceClient-Win64-Shipping.exe";

    let mut system = System::new();
    system.refresh_processes();

    // 1) DeltaForce: 提升优先级为 RealTime
    let mut delta_boosted = false;
    for (_, process) in system.processes() {
        if process.name().eq_ignore_ascii_case(delta_target) {
            if set_process_realtime_priority(process.pid().as_u32()) {
                delta_boosted = true;
            }
        }
    }

    // 2) ACE: 优先级降为 Idle + 限制亲和性为 CPU0
    let mut ace_found: u32 = 0;
    let mut ace_count: u32 = 0;
    for (_, process) in system.processes() {
        let name = process.name().to_string();
        let name_lower = name.to_lowercase();
        if ACE_PROCESS_NAMES.iter().any(|n| n.to_lowercase() == name_lower) {
            ace_found += 1;
            let pid = process.pid().as_u32();
            let priority_ok = set_process_low_priority(pid);
            let affinity_ok = set_process_affinity(pid, 1);
            if priority_ok || affinity_ok {
                ace_count += 1;
            }
        }
    }

    let ace_limited = ace_count > 0;

    let mut msgs: Vec<String> = Vec::new();
    if delta_boosted {
        msgs.push("三角洲: 已优化".to_string());
    } else {
        msgs.push("三角洲: 未运行".to_string());
    }
    if ace_found == 0 {
        msgs.push("ACE: 未运行".to_string());
    } else if ace_count == 0 {
        msgs.push(format!("ACE: 发现 {} 个进程，需管理员权限", ace_found));
    } else if ace_count < ace_found {
        msgs.push(format!(
            "ACE: 已限制 {} 个进程（另有 {} 个需管理员权限）",
            ace_count,
            ace_found - ace_count
        ));
    } else {
        msgs.push(format!("ACE: 已限制 {} 个进程", ace_count));
    }

    Ok(AllGameOptimizeResult {
        success: delta_boosted || ace_limited,
        message: msgs.join(" | "),
        delta_boosted,
        ace_limited,
        ace_count,
    })
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

#[derive(serde::Serialize)]
pub struct LaptopPowerLockStatus {
    /// 是否已解锁（PlatformAoAcOverride == 0）
    pub unlocked: bool,
    /// 当前注册表值（None 表示未设置）
    pub value: Option<u32>,
}

/// 读取 PlatformAoAcOverride 注册表值。
/// 该值用于覆盖平台 AoAc（Always On Always Connected）能力：
/// 现代待机（Modern Standby）笔记本厂商通过它锁定电源计划，
/// 设为 0 可解锁，使系统可自由导入/激活电源计划（需重启生效）。
fn read_platform_aoac_override() -> Option<u32> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let power = hklm
        .open_subkey(r"System\CurrentControlSet\Control\Power")
        .ok()?;
    power.get_value("PlatformAoAcOverride").ok()
}

/// 直接写入 PlatformAoAcOverride=0（需要管理员权限）
fn write_platform_aoac_override() -> Result<(), String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let (power, _) = hklm
        .create_subkey(r"System\CurrentControlSet\Control\Power")
        .map_err(|e| format!("打开注册表键失败: {}", e))?;
    power
        .set_value("PlatformAoAcOverride", &0u32)
        .map_err(|e| format!("写入注册表失败: {}", e))?;
    Ok(())
}

/// 通过 ShellExecuteEx 提权运行 reg.exe 写入（无 PowerShell，启动开销小）。
/// 应用非管理员时弹出 UAC，等待提权进程结束。
fn run_reg_add_elevated() -> Result<(), String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
    use windows::Win32::UI::Shell::{
        ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
    };
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

    let system_root = env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    let reg_path = format!(r"{}\System32\reg.exe", system_root);

    let to_w = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
    let verb_w = to_w("runas");
    let file_w = to_w(&reg_path);
    let args_w = to_w(
        "add HKLM\\System\\CurrentControlSet\\Control\\Power /v PlatformAoAcOverride /t REG_DWORD /d 0 /f",
    );

    let mut sei: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    sei.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    sei.fMask = SEE_MASK_NOCLOSEPROCESS;
    sei.lpVerb = PCWSTR(verb_w.as_ptr());
    sei.lpFile = PCWSTR(file_w.as_ptr());
    sei.lpParameters = PCWSTR(args_w.as_ptr());
    sei.nShow = SW_HIDE.0;

    if unsafe { ShellExecuteExW(&mut sei) }.is_err() {
        let code = unsafe { GetLastError() };
        return Err(format!(
            "需要管理员权限：提权失败（错误码 {}），可能是用户取消了授权",
            code
        ));
    }

    // 等待提权的 reg.exe 执行完毕
    unsafe { WaitForSingleObject(sei.hProcess, u32::MAX) };

    let mut exit_code: u32 = 0;
    if unsafe { GetExitCodeProcess(sei.hProcess, &mut exit_code) }.is_err() {
        let _ = unsafe { CloseHandle(sei.hProcess) };
        return Err("无法获取 reg.exe 执行结果".to_string());
    }
    let _ = unsafe { CloseHandle(sei.hProcess) };

    if exit_code != 0 {
        return Err(format!("reg.exe 写入失败（退出码 {}）", exit_code));
    }
    Ok(())
}

/// 获取笔记本电源计划锁定状态
#[tauri::command]
pub async fn get_laptop_power_lock_status() -> Result<LaptopPowerLockStatus, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }
    let value = read_platform_aoac_override();
    Ok(LaptopPowerLockStatus {
        unlocked: value == Some(0),
        value,
    })
}

/// 解锁笔记本电源计划（写入 PlatformAoAcOverride=0，需管理员权限，重启后生效）
#[tauri::command]
pub async fn unlock_laptop_power_plan() -> Result<LaptopPowerLockStatus, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    // 已解锁则直接返回
    if read_platform_aoac_override() == Some(0) {
        return Ok(LaptopPowerLockStatus {
            unlocked: true,
            value: Some(0),
        });
    }

    // 1) 应用以管理员运行时直接写入（纯 winreg，零子进程，速度快）
    if write_platform_aoac_override().is_err() {
        // 2) 非管理员：ShellExecuteEx 提权运行 reg.exe（弹 UAC）
        run_reg_add_elevated()?;
    }

    // 3) 回读验证，以注册表实际值为准
    match read_platform_aoac_override() {
        Some(0) => Ok(LaptopPowerLockStatus {
            unlocked: true,
            value: Some(0),
        }),
        other => Err(format!(
            "解锁未生效（当前注册表值: {:?}）。请以管理员身份运行 NexBox 后重试",
            other
        )),
    }
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
        "PowerX-v2" => ("PowerX v2".to_string(), "极致性能电源方案，最大化系统响应与游戏帧率".to_string()),
        "卓越性能" => ("卓越性能".to_string(), "Windows 卓越性能电源计划，解锁最高性能模式".to_string()),
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

/// 直接调用 powercfg.exe（通过 cmd 设置 UTF-8 代码页），避免 PowerShell 启动开销。
/// `chcp 65001` 确保中文输出不乱码。
fn run_powercfg(args: &[&str]) -> std::io::Result<std::process::Output> {
    let powercfg_args = args.join(" ");
    let full_cmd = format!("chcp 65001 >nul && powercfg {}", powercfg_args);
    Command::new("cmd")
        .args(&["/C", &full_cmd])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
}

fn get_system_plans_internal() -> Vec<(String, String, bool)> {
    let result = run_powercfg(&["/list"]);

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            parse_powercfg_list(&stdout)
        }
        Err(_) => Vec::new(),
    }
}

fn get_active_plan_internal() -> Option<(String, String)> {
    let result = run_powercfg(&["/getactivescheme"]);

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
        // 去掉末尾括号内的作者/后缀信息再匹配
        // 例如 "英特尔-KF系列提升平均帧计划(毒药制作" -> "英特尔-KF系列提升平均帧计划"
        if let Some(pos) = name.rfind('(') {
            let base_name = name[..pos].trim();
            if base_name.contains(plan_name) || plan_name.contains(base_name) {
                return Some(guid.clone());
            }
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

    let builtin_ids = ["ACMEPCAMD", "AMD电源计划", "ggOSDesktopGaming", "Intel大核心电源计划", "PowerX-v2", "卓越性能"];

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
                return Err(format!("导入电源计划失败: {}\n如果您是笔记本，请先点击「笔记本电源计划解锁」，然后重启电脑重试", err_msg));
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
                Err(format!("电源计划 '{}' 导入后未在系统中找到，可能导入失败。\n如果您是笔记本，请先点击「笔记本电源计划解锁」，然后重启电脑重试", display_name))
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
                Err(format!("电源计划激活失败: {}\n如果您是笔记本，请先点击「笔记本电源计划解锁」，然后重启电脑重试", err_msg))
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
                        return Err(format!("导入失败: {}\n如果您是笔记本，请先点击「笔记本电源计划解锁」，然后重启电脑重试", err_msg));
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
                        return Err(format!("电源计划 '{}' 导入后未在系统中找到，可能导入失败。\n如果您是笔记本，请先点击「笔记本电源计划解锁」，然后重启电脑重试", display_name));
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
/// 直接读 TaskCache 注册表（State=1 表示已禁用），无需启动 PowerShell，毫秒级完成。
fn check_schtasks_wu_disabled() -> bool {
    const TREE_PATH: &str =
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Schedule\TaskCache\Tree\Microsoft\Windows\WindowsUpdate";
    const TASKS_PATH: &str =
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Schedule\TaskCache\Tasks";
    const STATE_DISABLED: u32 = 1;

    fn task_state_disabled(hklm: &RegKey, tasks_path: &str, key: &RegKey) -> bool {
        // 1) 任务键自身的 State
        if let Ok(v) = key.get_value::<u32, _>("State") {
            if v == STATE_DISABLED {
                return true;
            }
        }
        // 2) 通过 Id 定位到 TaskCache\Tasks\{guid} 读取 State
        if let Ok(id) = key.get_value::<String, _>("Id") {
            if let Ok(task_key) = hklm.open_subkey(format!(r"{}\{}", tasks_path, id)) {
                if let Ok(v) = task_key.get_value::<u32, _>("State") {
                    if v == STATE_DISABLED {
                        return true;
                    }
                }
            }
        }
        // 3) 递归子键
        for child in key.enum_keys().flatten() {
            if let Ok(sub) = key.open_subkey(child) {
                if task_state_disabled(hklm, tasks_path, &sub) {
                    return true;
                }
            }
        }
        false
    }

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    match hklm.open_subkey(TREE_PATH) {
        Ok(root) => task_state_disabled(&hklm, TASKS_PATH, &root),
        Err(_) => false,
    }
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
/// 优先直接读注册表（毫秒级），仅当 ACL 阻止读取时回退到 reg query 进程。
fn get_service_start(service_name: &str) -> Option<u32> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let path = format!(r"SYSTEM\CurrentControlSet\Services\{}", service_name);
    if let Ok(key) = hklm.open_subkey(path) {
        if let Ok(v) = key.get_value::<u32, _>("Start") {
            return Some(v);
        }
    }

    // Fallback: use reg query via cmd.exe for reliable reading of ACL-protected keys
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

    // 开发模式后备：通过编译时 CARGO_MANIFEST_DIR 定位项目根目录
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let manifest_candidates = [
        Path::new(manifest_dir).join(dir).join(format!("{}{}", name, suffix)),
        Path::new(manifest_dir).join("..").join(dir).join(format!("{}{}", name, suffix)),
    ];
    for path in &manifest_candidates {
        if path.exists() {
            return Ok(path.clone());
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

/// 检查 Windows 更新暂停状态
#[tauri::command]
pub fn check_pause_update_state() -> Result<bool, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm
        .open_subkey(r"SOFTWARE\Microsoft\WindowsUpdate\UX\Settings")
        .map_err(|e| format!("无法打开注册表键: {}", e))?;

    // 检查 PauseUpdatesExpiryTime 值是否存在
    let paused: bool = key
        .get_value::<String, _>("PauseUpdatesExpiryTime")
        .map(|_| true)
        .unwrap_or(false);

    Ok(paused)
}
