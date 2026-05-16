use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::Mutex;
use thiserror::Error;
use sysinfo::System;

#[derive(Error, Debug)]
pub enum HardwareError {
    #[error("PowerShell执行失败: {0}")]
    PowerShellError(String),
    #[error("JSON解析失败: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("NVML错误: {0}")]
    NvmlError(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CpuInfo {
    pub name: String,
    pub cores: u32,
    pub threads: u32,
    pub max_clock_speed: u32,
    pub l3_cache_size: u32,
    pub load_percentage: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GpuInfo {
    pub name: String,
    pub memory_gb: f64,
    pub driver_version: String,
    pub temperature: Option<u32>,
    pub usage: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryInfo {
    pub manufacturer: String,
    pub part_number: String,
    pub capacity_gb: f64,
    pub speed_mhz: u32,
    pub bank_label: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HardwareInfo {
    pub cpu: CpuInfo,
    pub gpu: Vec<GpuInfo>,
    pub memory: Vec<MemoryInfo>,
    pub motherboard: String,
    pub disk: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct PsProcessor {
    Name: String,
    NumberOfCores: u32,
    NumberOfLogicalProcessors: u32,
    MaxClockSpeed: u32,
    L3CacheSize: Option<u32>,
    LoadPercentage: Option<u16>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct PsVideoController {
    Name: String,
    DriverVersion: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case, dead_code)]
struct PsBaseBoard {
    Manufacturer: String,
    Product: String,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct PsPhysicalMemory {
    Manufacturer: Option<String>,
    PartNumber: Option<String>,
    Capacity: u64,
    Speed: Option<u32>,
    BankLabel: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct PsDiskDrive {
    Model: String,
    Size: u64,
}

// 静态硬件信息缓存（不会变化的部分）
#[derive(Debug, Clone)]
struct StaticHardwareInfo {
    cpu: CpuInfo,
    gpu_static: Vec<GpuStaticInfo>,
    motherboard: String,
    memory: Vec<MemoryInfo>,
    disk: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GpuStaticInfo {
    name: String,
    memory_gb: f64,
    driver_version: String,
}

static STATIC_HARDWARE_CACHE: Mutex<Option<StaticHardwareInfo>> = Mutex::new(None);
static CPU_SYSTEM: Mutex<Option<System>> = Mutex::new(None);

fn run_powershell<T: for<'de> Deserialize<'de>>(command: &str) -> Result<Vec<T>, HardwareError> {
    let mut cmd = Command::new("powershell");
    cmd.args(&["-Command", command]);
    
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    
    let output = cmd.output()
        .map_err(|e| HardwareError::PowerShellError(e.to_string()))?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(HardwareError::PowerShellError(error.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // 尝试解析为数组，如果失败则尝试解析为单个对象并包装到数组中
    match serde_json::from_str::<Vec<T>>(&stdout) {
        Ok(results) => Ok(results),
        Err(_) => {
            if let Ok(single) = serde_json::from_str::<T>(&stdout) {
                Ok(vec![single])
            } else {
                Ok(vec![])
            }
        }
    }
}

fn get_nvidia_gpus_with_nvml() -> Result<Vec<GpuInfo>, HardwareError> {
    use nvml_wrapper::Nvml;

    let nvml = Nvml::init().map_err(|e| HardwareError::NvmlError(e.to_string()))?;
    let device_count = nvml
        .device_count()
        .map_err(|e| HardwareError::NvmlError(e.to_string()))?;

    let mut gpus = Vec::new();

    for i in 0..device_count {
        let device = nvml
            .device_by_index(i)
            .map_err(|e| HardwareError::NvmlError(e.to_string()))?;

        let name = device
            .name()
            .map_err(|e| HardwareError::NvmlError(e.to_string()))?;
        let memory_info = device
            .memory_info()
            .map_err(|e| HardwareError::NvmlError(e.to_string()))?;
        let memory_gb = memory_info.total as f64 / (1024.0 * 1024.0 * 1024.0);

        let temperature = device
            .temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
            .ok();
        let utilization = device.utilization_rates().ok();
        let usage = utilization.map(|u| u.gpu);

        let driver_version = nvml
            .sys_driver_version()
            .map_err(|e| HardwareError::NvmlError(e.to_string()))?;

        log::info!(
            "NVIDIA GPU (NVML): {}, 显存: {:.1}GB, 温度: {:?}°C, 占用: {:?}%",
            name,
            memory_gb,
            temperature,
            usage
        );

        gpus.push(GpuInfo {
            name,
            memory_gb,
            driver_version,
            temperature,
            usage,
        });
    }

    Ok(gpus)
}

fn get_nvidia_gpus_with_smi() -> Vec<GpuInfo> {
    let mut gpus = Vec::new();

    let mut cmd = Command::new("nvidia-smi");
    cmd.args(&[
        "--query-gpu=name,memory.total,temperature.gpu,utilization.gpu,driver_version",
        "--format=csv,noheader,nounits",
    ]);
    
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    if let Ok(output) = cmd.output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                if parts.len() >= 5 {
                    let name = parts[0].to_string();
                    let memory_mb: f64 = parts[1].parse().unwrap_or(0.0);
                    let memory_gb = memory_mb / 1024.0;
                    let temperature: Option<u32> = parts[2].parse().ok();
                    let usage: Option<u32> = parts[3].parse().ok();
                    let driver_version = parts[4].to_string();

                    log::info!(
                        "NVIDIA GPU (nvidia-smi): {}, 显存: {:.1}GB, 温度: {:?}°C, 占用: {:?}%",
                        name,
                        memory_gb,
                        temperature,
                        usage
                    );
                    gpus.push(GpuInfo {
                        name,
                        memory_gb,
                        driver_version,
                        temperature,
                        usage,
                    });
                }
            }
        }
    }

    gpus
}

fn get_gpus_from_wmi() -> Vec<GpuInfo> {
    let mut gpus = Vec::new();

    let gpu_cmd = "Get-WmiObject Win32_VideoController | Select-Object Name, DriverVersion | ConvertTo-Json -Compress";
    if let Ok(gpu_results) = run_powershell::<PsVideoController>(gpu_cmd) {
        for g in gpu_results {
            // 过滤掉集成显卡
            let name_lower = g.Name.to_lowercase();
            let is_integrated = name_lower.contains("intel")
                && (g.Name.contains("HD") || g.Name.contains("UHD") || g.Name.contains("Iris"));

            if !is_integrated {
                log::info!("独立显卡(WMI): {}", g.Name);
                gpus.push(GpuInfo {
                    name: g.Name.clone(),
                    memory_gb: 0.0,
                    driver_version: g.DriverVersion.unwrap_or_else(|| "未知".to_string()),
                    temperature: None,
                    usage: None,
                });
            }
        }
    }

    gpus
}

fn get_gpu_info() -> Vec<GpuInfo> {
    // 首先尝试用NVML获取NVIDIA显卡（最好的方式）
    if let Ok(nvml_gpus) = get_nvidia_gpus_with_nvml() {
        if !nvml_gpus.is_empty() {
            return nvml_gpus;
        }
    }

    // 然后尝试用nvidia-smi
    let smi_gpus = get_nvidia_gpus_with_smi();
    if !smi_gpus.is_empty() {
        return smi_gpus;
    }

    // 最后用WMI
    get_gpus_from_wmi()
}

// 只获取GPU的动态数据（温度、占用）
fn get_gpu_dynamic_info(gpu_static: &[GpuStaticInfo]) -> Vec<(Option<u32>, Option<u32>)> {
    let mut dynamic_info = Vec::new();

    // 尝试用NVML
    if let Ok(nvml_gpus) = get_nvidia_gpus_with_nvml() {
        for gpu in nvml_gpus {
            dynamic_info.push((gpu.temperature, gpu.usage));
        }
        return dynamic_info;
    }

    // 尝试用nvidia-smi
    let smi_gpus = get_nvidia_gpus_with_smi();
    for gpu in smi_gpus {
        dynamic_info.push((gpu.temperature, gpu.usage));
    }

    // 如果没有实时数据，填充None
    while dynamic_info.len() < gpu_static.len() {
        dynamic_info.push((None, None));
    }

    dynamic_info
}

// 获取CPU的动态数据（占用）- 使用 sysinfo 库
fn get_cpu_dynamic_info() -> Option<u16> {
    use sysinfo::CpuRefreshKind;
    
    let mut cpu_system = CPU_SYSTEM.lock().unwrap();
    
    if cpu_system.is_none() {
        let mut sys = System::new();
        sys.refresh_cpu_specifics(CpuRefreshKind::everything());
        *cpu_system = Some(sys);
        return Some(0);
    }
    
    let sys = cpu_system.as_mut().unwrap();
    sys.refresh_cpu_specifics(CpuRefreshKind::everything());
    
    let cpus = sys.cpus();
    if cpus.is_empty() {
        return None;
    }
    
    let total_usage: f32 = cpus.iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / cpus.len() as f32;
    let usage = total_usage.round() as u16;
    
    log::info!("CPU占用 (sysinfo): {}%", usage);
    Some(usage)
}

fn get_static_hardware_info() -> Result<StaticHardwareInfo, HardwareError> {
    // 首先尝试从缓存获取
    {
        let cache = STATIC_HARDWARE_CACHE.lock().unwrap();
        if let Some(ref info) = *cache {
            log::info!("从缓存获取静态硬件信息");
            return Ok(info.clone());
        }
    }

    log::info!("开始获取静态硬件信息...");

    // 获取CPU信息（包括静态和初始动态数据）
    let cpu_cmd = "Get-WmiObject Win32_Processor | Select-Object Name, NumberOfCores, NumberOfLogicalProcessors, MaxClockSpeed, L3CacheSize, LoadPercentage | ConvertTo-Json -Compress";
    let cpu_results: Vec<PsProcessor> = run_powershell(cpu_cmd)?;
    log::info!("获取到{}个CPU信息", cpu_results.len());
    let cpu = cpu_results
        .first()
        .map(|p| {
            log::info!("CPU型号: {}", p.Name);
            log::info!(
                "核心数: {}, 线程数: {}",
                p.NumberOfCores,
                p.NumberOfLogicalProcessors
            );
            log::info!("最大时钟速度: {} MHz", p.MaxClockSpeed);
            log::info!("CPU占用: {:?}%", p.LoadPercentage);
            CpuInfo {
                name: p.Name.clone(),
                cores: p.NumberOfCores,
                threads: p.NumberOfLogicalProcessors,
                max_clock_speed: p.MaxClockSpeed,
                l3_cache_size: p.L3CacheSize.unwrap_or(0),
                load_percentage: p.LoadPercentage,
            }
        })
        .unwrap_or_else(|| CpuInfo {
            name: "未知CPU".to_string(),
            cores: 0,
            threads: 0,
            max_clock_speed: 0,
            l3_cache_size: 0,
            load_percentage: None,
        });

    // 获取GPU信息
    let gpu = get_gpu_info();
    let gpu_static: Vec<GpuStaticInfo> = gpu
        .iter()
        .map(|g| GpuStaticInfo {
            name: g.name.clone(),
            memory_gb: g.memory_gb,
            driver_version: g.driver_version.clone(),
        })
        .collect();

    // 获取主板信息
    let mobo_cmd = "Get-WmiObject Win32_BaseBoard | Select-Object Manufacturer, Product | ConvertTo-Json -Compress";
    let mobo_results: Vec<PsBaseBoard> = run_powershell(mobo_cmd)?;
    log::info!("获取到{}个主板信息", mobo_results.len());
    let motherboard = mobo_results
        .first()
        .map(|m| {
            log::info!("主板: {}", m.Product);
            m.Product.clone()
        })
        .unwrap_or_else(|| "未知主板".to_string());

    // 获取内存信息
    let mem_cmd = "Get-WmiObject Win32_PhysicalMemory | Select-Object Manufacturer, PartNumber, Capacity, Speed, BankLabel | ConvertTo-Json -Compress";
    let mem_results: Vec<PsPhysicalMemory> = run_powershell(mem_cmd)?;
    log::info!("获取到{}个内存条信息", mem_results.len());
    let mut memory = Vec::new();
    for mem in mem_results {
        let capacity_gb = mem.Capacity as f64 / (1024.0 * 1024.0 * 1024.0);
        let memory_info = MemoryInfo {
            manufacturer: mem.Manufacturer.unwrap_or_else(|| "未知".to_string()),
            part_number: mem
                .PartNumber
                .unwrap_or_else(|| "未知".to_string())
                .trim()
                .to_string(),
            capacity_gb,
            speed_mhz: mem.Speed.unwrap_or(0),
            bank_label: mem.BankLabel.unwrap_or_else(|| "未知".to_string()),
        };
        log::info!(
            "内存: {} {} {}GB {}MHz {}",
            memory_info.manufacturer,
            memory_info.part_number,
            memory_info.capacity_gb,
            memory_info.speed_mhz,
            memory_info.bank_label
        );
        memory.push(memory_info);
    }

    // 获取硬盘信息
    let disk_cmd =
        "Get-WmiObject Win32_DiskDrive | Select-Object Model, Size | ConvertTo-Json -Compress";
    let disk_results: Vec<PsDiskDrive> = run_powershell(disk_cmd)?;
    log::info!("获取到{}个硬盘信息", disk_results.len());
    let disk: Vec<String> = disk_results
        .iter()
        .map(|d| {
            let size_gb = d.Size / (1024 * 1024 * 1024);
            let disk_info = format!("{} ({}GB)", d.Model, size_gb);
            log::info!("硬盘: {}", disk_info);
            disk_info
        })
        .collect();

    let static_info = StaticHardwareInfo {
        cpu,
        gpu_static,
        motherboard,
        memory,
        disk,
    };

    // 缓存静态硬件信息
    {
        let mut cache = STATIC_HARDWARE_CACHE.lock().unwrap();
        *cache = Some(static_info.clone());
    }

    log::info!("静态硬件信息获取完成");
    Ok(static_info)
}

pub fn get_hardware_info() -> Result<HardwareInfo, HardwareError> {
    let static_info = get_static_hardware_info()?;

    // 获取动态数据
    let cpu_load = get_cpu_dynamic_info();
    let gpu_dynamic = get_gpu_dynamic_info(&static_info.gpu_static);

    // 组合完整信息
    let mut cpu = static_info.cpu;
    // 仅在成功读取到动态 CPU 占用时覆盖静态值，避免在失败时将已有值清空
    if let Some(load) = cpu_load {
        cpu.load_percentage = Some(load);
    }

    let gpu: Vec<GpuInfo> = static_info
        .gpu_static
        .iter()
        .enumerate()
        .map(|(i, gs)| {
            let (temp, usage) = gpu_dynamic.get(i).copied().unwrap_or((None, None));
            GpuInfo {
                name: gs.name.clone(),
                memory_gb: gs.memory_gb,
                driver_version: gs.driver_version.clone(),
                temperature: temp,
                usage,
            }
        })
        .collect();

    Ok(HardwareInfo {
        cpu,
        gpu,
        motherboard: static_info.motherboard,
        memory: static_info.memory,
        disk: static_info.disk,
    })
}

#[tauri::command]
pub async fn get_hardware() -> Result<HardwareInfo, String> {
    match tauri::async_runtime::spawn_blocking(|| get_hardware_info()).await {
        Ok(Ok(info)) => Ok(info),
        Ok(Err(e)) => Err(e.to_string()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn get_cpu_load() -> Result<Option<u16>, String> {
    match tauri::async_runtime::spawn_blocking(|| get_cpu_dynamic_info()).await {
        Ok(load) => Ok(load),
        Err(e) => Err(e.to_string()),
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GpuStatus {
    pub temperature: Option<u32>,
    pub usage: Option<u32>,
}

#[tauri::command]
pub async fn get_gpu_status(index: usize) -> Result<GpuStatus, String> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        if let Ok(nvml_gpus) = get_nvidia_gpus_with_nvml() {
            if let Some(gpu) = nvml_gpus.get(index) {
                return GpuStatus {
                    temperature: gpu.temperature,
                    usage: gpu.usage,
                };
            }
        }
        
        let smi_gpus = get_nvidia_gpus_with_smi();
        if let Some(gpu) = smi_gpus.get(index) {
            return GpuStatus {
                temperature: gpu.temperature,
                usage: gpu.usage,
            };
        }
        
        GpuStatus {
            temperature: None,
            usage: None,
        }
    }).await;
    
    match result {
        Ok(status) => Ok(status),
        Err(e) => Err(e.to_string()),
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiskInfo {
    pub name: String,
    pub total_gb: f64,
    pub available_gb: f64,
    pub used_gb: f64,
    pub usage_percent: f64,
}

#[tauri::command]
pub async fn get_disk_status() -> Result<DiskInfo, String> {
    let result = tauri::async_runtime::spawn_blocking(|| {
        use sysinfo::Disks;
        
        let disks = Disks::new_with_refreshed_list();
        
        let mut total_space: u64 = 0;
        let mut available_space: u64 = 0;
        
        for disk in disks.iter() {
            let mount_point = disk.mount_point().to_string_lossy();
            if mount_point.is_empty() {
                continue;
            }
            total_space = total_space.saturating_add(disk.total_space());
            available_space = available_space.saturating_add(disk.available_space());
        }
        
        let used_space = total_space.saturating_sub(available_space);
        let usage_percent = if total_space > 0 {
            (used_space as f64 / total_space as f64) * 100.0
        } else {
            0.0
        };
        
        let total_gb = total_space as f64 / (1024.0 * 1024.0 * 1024.0);
        let available_gb = available_space as f64 / (1024.0 * 1024.0 * 1024.0);
        let used_gb = used_space as f64 / (1024.0 * 1024.0 * 1024.0);
        
        DiskInfo {
            name: String::from("All Disks"),
            total_gb,
            available_gb,
            used_gb,
            usage_percent,
        }
    }).await;
    
    match result {
        Ok(info) => Ok(info),
        Err(e) => Err(e.to_string()),
    }
}

// 清理硬件信息缓存
pub fn get_overlay_cpu_usage() -> Option<u16> {
    get_cpu_dynamic_info()
}

pub fn get_overlay_gpu_info() -> (Option<u32>, Option<u32>) {
    let cache = STATIC_HARDWARE_CACHE.lock().unwrap();
    let gpu_static: &[GpuStaticInfo] = cache.as_ref().map(|c| c.gpu_static.as_slice()).unwrap_or(&[]);
    let dynamic = get_gpu_dynamic_info(gpu_static);
    drop(cache);
    dynamic.first().cloned().unwrap_or((None, None))
}

pub fn cleanup_hardware_cache() {
    let mut cache = STATIC_HARDWARE_CACHE.lock().unwrap();
    *cache = None;

    let mut cpu_system = CPU_SYSTEM.lock().unwrap();
    *cpu_system = None;
    
    log::info!("硬件信息缓存已清理");
}
