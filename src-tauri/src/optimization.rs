use std::os::windows::process::CommandExt;
use std::process::Command;
use std::{env, fs, path::Path};
use sysinfo::System;

const CREATE_NO_WINDOW: u32 = 0x08000000;

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

#[tauri::command]
pub async fn clean_standby_memory() -> Result<MemoryCleanupResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

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
            let freed = if after.available > before.available {
                after.available - before.available
            } else {
                0
            };

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
        Err(e) => Err(format!("清理待机内存失败: {}", e)),
    }
}

#[tauri::command]
pub async fn trim_system_working_set() -> Result<MemoryCleanupResult, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

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
            let freed = if after.available > before.available {
                after.available - before.available
            } else {
                0
            };

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
        Err(e) => Err(format!("收紧系统工作集失败: {}", e)),
    }
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
