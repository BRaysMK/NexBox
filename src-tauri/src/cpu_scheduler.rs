use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessRefreshKind, System};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;
use log;

// LOGICAL_PROCESSOR_RELATIONSHIP 常量
const RELATION_PROCESSOR_CORE: i32 = 0;

// ── Win32 常量 ──────────────────────────────────────────────
const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
const PROCESS_SET_INFORMATION: u32 = 0x0200;

// ── 数据结构 ────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum CoreType {
    Performance,
    Efficiency,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PhysicalCore {
    pub core_index: u32,
    pub core_type: CoreType,
    pub logical_processors: Vec<u32>,
    /// 该核心在亲和性掩码中的位组合 (1 << lp for each lp)
    pub affinity_mask: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CpuTopology {
    pub cpu_name: String,
    pub total_physical_cores: u32,
    pub total_logical_processors: u32,
    pub has_hybrid_architecture: bool,
    pub physical_cores: Vec<PhysicalCore>,
    /// 系统全部可用逻辑处理器的掩码
    pub system_affinity_mask: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub memory_mb: f64,
    pub cpu_usage: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProcessAffinityInfo {
    pub pid: u32,
    pub process_name: String,
    pub affinity_mask: u64,
    pub system_mask: u64,
    pub assigned_logical_processors: Vec<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SchedulerRule {
    pub process_name: String,
    pub mask: u64,
    pub preset: String,
    pub description: String,
}

// ── CPU 拓扑 ────────────────────────────────────────────────

/// 读取 LE u32
fn read_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}
/// 读取 LE u16
fn read_u16(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}
/// 读取 LE u64
fn read_u64(buf: &[u8], off: usize) -> u64 {
    u64::from_le_bytes([
        buf[off], buf[off + 1], buf[off + 2], buf[off + 3],
        buf[off + 4], buf[off + 5], buf[off + 6], buf[off + 7],
    ])
}

#[cfg(target_os = "windows")]
fn get_cpu_topology_win32() -> Result<CpuTopology, String> {
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::System::SystemInformation::{
        GetLogicalProcessorInformationEx, SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX,
    };

    let cpu_name = get_cpu_name();

    // 调用 GetLogicalProcessorInformationEx 获取核心拓扑
    let relationship: i32 = RELATION_PROCESSOR_CORE;
    let mut buffer_size: u32 = 0;
    unsafe {
        GetLogicalProcessorInformationEx(relationship, std::ptr::null_mut(), &mut buffer_size);
    }
    if buffer_size == 0 {
        return Err("无法获取CPU处理器信息".to_string());
    }

    // 分配 8 字节对齐的 buffer
    let alloc_units = ((buffer_size as usize) + 7) / 8;
    let mut buffer_aligned: Vec<u64> = vec![0u64; alloc_units];
    let success = unsafe {
        GetLogicalProcessorInformationEx(
            relationship,
            buffer_aligned.as_mut_ptr() as *mut SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX,
            &mut buffer_size,
        )
    };
    if success == 0 {
        let err = unsafe { GetLastError() };
        return Err(format!("GetLogicalProcessorInformationEx 失败, 错误码: {}", err));
    }

    // 转为字节切片
    let buf: &[u8] = unsafe {
        std::slice::from_raw_parts(buffer_aligned.as_ptr() as *const u8, buffer_size as usize)
    };

    // ── 按 Windows SDK 精确字节偏移解析 ─────────────────────
    //
    // SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX:
    //   +0  Relationship  DWORD  (4)
    //   +4  Size           DWORD  (4)
    //   +8  union data
    //
    // PROCESSOR_RELATIONSHIP (从 entry+8 开始):
    //   +0  Flags           BYTE   (1)
    //   +1  EfficiencyClass BYTE   (1)
    //   +2  Reserved        BYTE[20] (20)
    //   +22 GroupCount      WORD   (2)
    //   +24 GroupMask[]     GROUP_AFFINITY[] (每个 16 bytes)
    //
    // GROUP_AFFINITY:
    //   +0  Mask    ULONG_PTR (8 on 64-bit)
    //   +8  Group   WORD      (2)
    //   +10 Reserved WORD[3]  (6)
    //   Total: 16 bytes

    let mut physical_cores: Vec<PhysicalCore> = Vec::new();
    let mut total_logical: u32 = 0;
    let mut system_mask: u64 = 0;
    let mut has_efficiency = false;

    let mut offset = 0usize;
    while offset + 8 <= buf.len() {
        let rel = read_u32(buf, offset) as i32;
        let entry_size = read_u32(buf, offset + 4) as usize;

        if entry_size == 0 || offset + entry_size > buf.len() {
            break;
        }

        // 只处理 RelationProcessorCore (0)
        if rel == 0 {
            let p = offset + 8; // union 起点

            let efficiency_class = buf[p + 1];
            let group_count = read_u16(buf, p + 22) as usize;

            // 读取该核心的逻辑处理器掩码
            let mut core_mask: u64 = 0;
            for g in 0..group_count {
                let gm = p + 24 + g * 16;
                if gm + 10 > buf.len() {
                    break;
                }
                let mask = read_u64(buf, gm);
                let group = read_u16(buf, gm + 8);
                if group == 0 {
                    core_mask |= mask;
                }
            }

            // EfficiencyClass 含义：值越小性能越高，值越大能效越高
            //   1  = 最高性能核心 (P-core / Performance)
            //   >=2 = 能效核心 (E-core / Efficiency)
            //   参考: https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-processor_relationship
            let core_type = match efficiency_class {
                1 => CoreType::Performance,                          // P核：最高性能等级
                ec if ec >= 2 => { has_efficiency = true; CoreType::Efficiency }, // E核：更高的能效等级
                _ => CoreType::Unknown,
            };

            // 从掩码中提取逻辑处理器编号
            let mut logical_processors: Vec<u32> = Vec::new();
            for bit in 0..64u32 {
                if (core_mask >> bit) & 1 == 1 {
                    logical_processors.push(bit);
                }
            }

            total_logical += logical_processors.len() as u32;
            system_mask |= core_mask;

            physical_cores.push(PhysicalCore {
                core_index: physical_cores.len() as u32,
                core_type,
                logical_processors,
                affinity_mask: core_mask,
            });
        }

        offset += entry_size;
    }

    if physical_cores.is_empty() {
        return Err("未能解析到任何物理核心信息".to_string());
    }

    // 非混合架构（无 E核，如 AMD）: 所有 Unknown 核心视为 Performance 核心
    if !has_efficiency {
        for core in &mut physical_cores {
            if core.core_type == CoreType::Unknown {
                core.core_type = CoreType::Performance;
            }
        }
    }

    Ok(CpuTopology {
        cpu_name,
        total_physical_cores: physical_cores.len() as u32,
        total_logical_processors: total_logical,
        has_hybrid_architecture: has_efficiency,
        physical_cores,
        system_affinity_mask: system_mask,
    })
}

#[cfg(not(target_os = "windows"))]
fn get_cpu_topology_win32() -> Result<CpuTopology, String> {
    Err("此功能仅支持 Windows 系统".to_string())
}

fn get_cpu_name() -> String {
    let sys = System::new_all();
    let cpus = sys.cpus();
    if !cpus.is_empty() {
        return cpus[0].brand().to_string();
    }
    "未知 CPU".to_string()
}

#[tauri::command]
pub async fn get_cpu_topology() -> Result<CpuTopology, String> {
    get_cpu_topology_win32()
}

// ── 进程列表 ────────────────────────────────────────────────

/// 系统关键进程白名单 — 禁止用户操作
const PROTECTED_PROCESSES: &[&str] = &[
    "System",
    "Registry",
    "smss.exe",
    "csrss.exe",
    "wininit.exe",
    "services.exe",
    "lsass.exe",
    "svchost.exe",
    "fontdrvhost.exe",
    "dwm.exe",
    "winlogon.exe",
    "MemCompression",
    "kthreadd",
    "Idle",
];

fn is_protected_process(name: &str) -> bool {
    let lower = name.to_lowercase();
    PROTECTED_PROCESSES.iter().any(|p| p.to_lowercase() == lower)
}

#[tauri::command]
pub async fn get_process_list() -> Result<Vec<ProcessInfo>, String> {
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessRefreshKind::everything().with_cpu().with_memory(),
    );

    let mut processes: Vec<ProcessInfo> = Vec::new();

    for (_, proc_) in sys.processes() {
        let name = proc_.name().to_string();
        if name.is_empty() || is_protected_process(&name) {
            continue;
        }
        let memory_mb = proc_.memory() as f64 / 1024.0 / 1024.0;
        processes.push(ProcessInfo {
            pid: proc_.pid().as_u32(),
            name,
            memory_mb,
            cpu_usage: proc_.cpu_usage(),
        });
    }

    // 按内存占用降序
    processes.sort_by(|a, b| {
        b.memory_mb
            .partial_cmp(&a.memory_mb)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(processes)
}

// ── 进程亲和性 ──────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn open_process_for_affinity(pid: u32) -> Result<windows_sys::Win32::Foundation::HANDLE, String> {
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::System::Threading::OpenProcess;

    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_SET_INFORMATION,
            0,
            pid,
        )
    };

    if handle.is_null() {
        let err = unsafe { GetLastError() };
        // 尝试以有限权限打开
        let handle_limited = unsafe {
            OpenProcess(
                PROCESS_QUERY_INFORMATION,
                0,
                pid,
            )
        };
        if handle_limited.is_null() {
            return Err(format!("无法打开进程 (PID: {}), 错误码: {}", pid, err));
        }
        return Ok(handle_limited);
    }

    Ok(handle)
}

#[tauri::command]
pub async fn get_process_affinity(pid: u32) -> Result<ProcessAffinityInfo, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::GetProcessAffinityMask;

        let handle = open_process_for_affinity(pid)?;
        let mut process_mask: usize = 0;
        let mut system_mask: usize = 0;
        let success = unsafe {
            GetProcessAffinityMask(handle, &mut process_mask, &mut system_mask)
        };

        let proc_name = get_process_name_by_pid(pid);
        let pm = process_mask as u64;
        let sm = system_mask as u64;

        unsafe { CloseHandle(handle); }

        if success == 0 {
            return Err(format!("获取进程亲和性失败 (PID: {})", pid));
        }

        let mut assigned: Vec<u32> = Vec::new();
        for bit in 0..64u32 {
            if (pm >> bit) & 1 == 1 {
                assigned.push(bit);
            }
        }

        Ok(ProcessAffinityInfo {
            pid,
            process_name: proc_name,
            affinity_mask: pm,
            system_mask: sm,
            assigned_logical_processors: assigned,
        })
    }

    #[cfg(not(target_os = "windows"))]
    Err("此功能仅支持 Windows 系统".to_string())
}

#[tauri::command]
pub async fn set_process_affinity(pid: u32, mask: u64) -> Result<bool, String> {
    if !cfg!(target_os = "windows") {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
        use windows_sys::Win32::System::Threading::SetProcessAffinityMask;

        let handle = open_process_for_affinity(pid)?;
        let success = unsafe { SetProcessAffinityMask(handle, mask as usize) };
        let err = unsafe { GetLastError() };
        unsafe { CloseHandle(handle); }

        if success == 0 {
            return Err(format!("设置进程亲和性失败, 错误码: {} (可能需要管理员权限)", err));
        }

        Ok(true)
    }

    #[cfg(not(target_os = "windows"))]
    Err("此功能仅支持 Windows 系统".to_string())
}

#[tauri::command]
pub async fn restore_process_affinity(pid: u32) -> Result<bool, String> {
    // 恢复 = 设置为系统全部可用核心
    let topology = get_cpu_topology().await?;
    set_process_affinity(pid, topology.system_affinity_mask).await
}

fn get_process_name_by_pid(pid: u32) -> String {
    let mut sys = System::new();
    sys.refresh_process_specifics(Pid::from_u32(pid), ProcessRefreshKind::everything());
    sys.process(Pid::from_u32(pid))
        .map(|p| p.name().to_string())
        .unwrap_or_else(|| "未知".to_string())
}

// ── 规则持久化 ──────────────────────────────────────────────

const STORE_FILE: &str = "cpu-scheduler-rules.json";

#[tauri::command]
pub async fn get_saved_rules(app: AppHandle) -> Result<Vec<SchedulerRule>, String> {
    let store = app.store(STORE_FILE).map_err(|e| format!("无法打开规则存储: {}", e))?;
    let mut rules: Vec<SchedulerRule> = Vec::new();

    for (key, value) in store.entries() {
        if let Some(obj) = value.as_object() {
            let mask = obj
                .get("mask")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let preset = obj
                .get("preset")
                .and_then(|v| v.as_str())
                .unwrap_or("custom")
                .to_string();
            let description = obj
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            rules.push(SchedulerRule {
                process_name: key,
                mask,
                preset,
                description,
            });
        }
    }

    rules.sort_by(|a, b| a.process_name.cmp(&b.process_name));
    Ok(rules)
}

#[tauri::command]
pub async fn save_rule(
    app: AppHandle,
    process_name: String,
    mask: u64,
    preset: String,
    description: String,
) -> Result<bool, String> {
    let store = app.store(STORE_FILE).map_err(|e| format!("无法打开规则存储: {}", e))?;
    let rule = serde_json::json!({
        "mask": mask,
        "preset": preset,
        "description": description,
    });
    store.set(&process_name, rule);
    store
        .save()
        .map_err(|e| format!("保存规则失败: {}", e))?;
    Ok(true)
}

#[tauri::command]
pub async fn delete_rule(app: AppHandle, process_name: String) -> Result<bool, String> {
    let store = app.store(STORE_FILE).map_err(|e| format!("无法打开规则存储: {}", e))?;
    store.delete(&process_name);
    store
        .save()
        .map_err(|e| format!("删除规则失败: {}", e))?;
    Ok(true)
}

#[tauri::command]
pub async fn apply_rule_by_name(app: AppHandle, process_name: String) -> Result<(bool, u32), String> {
    let store = app.store(STORE_FILE).map_err(|e| format!("无法打开规则存储: {}", e))?;
    let value = store
        .get(&process_name)
        .ok_or_else(|| format!("未找到进程 {} 的规则", process_name))?;

    let mask = value
        .as_object()
        .and_then(|o| o.get("mask"))
        .and_then(|v| v.as_u64())
        .ok_or_else(|| format!("未找到进程 {} 的规则", process_name))?;

    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessRefreshKind::new());

    let mut count = 0u32;
    for (_, proc_) in sys.processes() {
        if proc_.name() == process_name {
            match set_process_affinity(proc_.pid().as_u32(), mask).await {
                Ok(_) => count += 1,
                Err(_) => {}
            }
        }
    }

    if count == 0 {
        return Err(format!("未找到运行中的进程: {}", process_name));
    }

    Ok((true, count))
}

// ── 启动时自动应用所有已保存规则 ──────────────────────────────

pub async fn apply_all_saved_rules(app: &AppHandle) {
    let store = match app.store(STORE_FILE) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("[CPU调度] 无法打开规则存储: {}", e);
            return;
        }
    };

    let entries = store.entries();
    if entries.is_empty() {
        return;
    }

    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessRefreshKind::new());

    let mut applied = 0u32;
    let mut skipped = 0u32;

    for (process_name, value) in entries {
        let mask = match value
            .as_object()
            .and_then(|o| o.get("mask"))
            .and_then(|v| v.as_u64())
        {
            Some(m) => m,
            None => continue,
        };

        let mut found = false;
        for (_, proc_) in sys.processes() {
            if proc_.name() == process_name.as_str() {
                found = true;
                match set_process_affinity(proc_.pid().as_u32(), mask).await {
                    Ok(_) => applied += 1,
                    Err(e) => log::warn!("[CPU调度] 应用规则 {} 失败: {}", process_name, e),
                }
            }
        }
        if !found {
            skipped += 1;
        }
    }

    if applied > 0 || skipped > 0 {
        log::info!(
            "[CPU调度] 启动时自动应用规则完成: {} 条已应用, {} 条进程未运行",
            applied,
            skipped
        );
    }
}
