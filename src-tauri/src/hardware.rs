use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum GpuVendor {
    NVIDIA,
    AMD,
    Intel,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: GpuVendor,
    pub memory_gb: f64,
    pub driver_version: String,
    pub temperature: Option<f64>,
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
pub struct SoundCardInfo {
    pub name: String,
    pub manufacturer: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkCardInfo {
    pub name: String,
    pub manufacturer: String,
    pub adapter_type: String,
    pub mac_address: String,
    pub speed_mbps: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HardwareInfo {
    pub cpu: CpuInfo,
    pub gpu: Vec<GpuInfo>,
    pub memory: Vec<MemoryInfo>,
    pub motherboard: String,
    pub disk: Vec<String>,
    pub sound_card: Vec<SoundCardInfo>,
    pub network_card: Vec<NetworkCardInfo>,
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
    AdapterRAM: Option<u64>,
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

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct PsSoundDevice {
    Name: String,
    Manufacturer: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct PsNetworkAdapter {
    Name: String,
    Manufacturer: Option<String>,
    AdapterType: Option<String>,
    MACAddress: Option<String>,
    Speed: Option<u64>,
}

// 静态硬件信息缓存（不会变化的部分）
#[derive(Debug, Clone)]
struct StaticHardwareInfo {
    cpu: CpuInfo,
    gpu_static: Vec<GpuStaticInfo>,
    motherboard: String,
    memory: Vec<MemoryInfo>,
    disk: Vec<String>,
    sound_card: Vec<SoundCardInfo>,
    network_card: Vec<NetworkCardInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GpuStaticInfo {
    name: String,
    vendor: GpuVendor,
    memory_gb: f64,
    driver_version: String,
}

static STATIC_HARDWARE_CACHE: Mutex<Option<StaticHardwareInfo>> = Mutex::new(None);
static HARDWARE_INIT_LOCK: Mutex<()> = Mutex::new(());
static CPU_SYSTEM: Mutex<Option<System>> = Mutex::new(None);

fn detect_gpu_vendor(name: &str) -> GpuVendor {
    let name_lower = name.to_lowercase();
    if name_lower.contains("nvidia") || name_lower.contains("geforce") || 
       name_lower.contains("gtx") || name_lower.contains("rtx") {
        GpuVendor::NVIDIA
    } else if name_lower.contains("amd") || name_lower.contains("radeon") || 
              name_lower.contains("rx ") {
        GpuVendor::AMD
    } else if name_lower.contains("intel") {
        GpuVendor::Intel
    } else {
        GpuVendor::Unknown
    }
}

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
            .ok()
            .map(|t| if t > 200 { t as f64 / 10.0 } else { t as f64 });
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
            vendor: GpuVendor::NVIDIA,
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
                    let temperature: Option<f64> = parts[2].parse().ok().map(|v: u32| v as f64);
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
                        vendor: GpuVendor::NVIDIA,
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

    let gpu_cmd = "Get-WmiObject Win32_VideoController | Select-Object Name, DriverVersion, AdapterRAM | ConvertTo-Json -Compress";
    if let Ok(gpu_results) = run_powershell::<PsVideoController>(gpu_cmd) {
        for g in gpu_results {
            let name_lower = g.Name.to_lowercase();
            let vendor = detect_gpu_vendor(&g.Name);
            let memory_gb = g.AdapterRAM
                .map(|ram| ram as f64 / (1024.0 * 1024.0 * 1024.0))
                .unwrap_or(0.0);

            // 过滤核显（Intel 集成显卡和 AMD APU）
            let is_integrated = match vendor {
                GpuVendor::Intel => !name_lower.contains("arc"),
                GpuVendor::AMD => {
                    name_lower.contains("radeon") && name_lower.contains("graphics")
                        && !name_lower.contains("rx ")
                }
                _ => false,
            };

            if is_integrated {
                log::info!("跳过核显(WMI): {}, 厂商: {:?}, 显存: {:.1}GB",
                          g.Name, vendor, memory_gb);
                continue;
            }

            log::info!("显卡(WMI): {}, 厂商: {:?}, 显存: {:.1}GB", 
                      g.Name, vendor, memory_gb);
            
            gpus.push(GpuInfo {
                name: g.Name.clone(),
                vendor,
                memory_gb,
                driver_version: g.DriverVersion.unwrap_or_else(|| "未知".to_string()),
                temperature: None,
                usage: None,
            });
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
fn get_gpu_dynamic_info(gpu_static: &[GpuStaticInfo]) -> Vec<(Option<f64>, Option<u32>)> {
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
    use std::thread;
    use std::time::Duration;
    
    let mut cpu_system = CPU_SYSTEM.lock().unwrap();
    
    if cpu_system.is_none() {
        let mut sys = System::new();
        // 第一次刷新：初始化
        sys.refresh_cpu_specifics(CpuRefreshKind::everything());
        // 短暂等待，让 sysinfo 有时间采集第一个样本
        thread::sleep(Duration::from_millis(50));
        // 第二次刷新：获取准确的 CPU 使用率
        sys.refresh_cpu_specifics(CpuRefreshKind::everything());
        *cpu_system = Some(sys);
    } else {
        // 正常情况下只需要刷新一次
        let sys = cpu_system.as_mut().unwrap();
        sys.refresh_cpu_specifics(CpuRefreshKind::everything());
    }
    
    let sys = cpu_system.as_ref().unwrap();
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
    // Fast path: 先检查缓存，不加初始化锁
    {
        let cache = STATIC_HARDWARE_CACHE.lock().unwrap();
        if let Some(ref info) = *cache {
            log::info!("从缓存获取静态硬件信息");
            return Ok(info.clone());
        }
    }

    // 序列化首次初始化，防止并发重复获取
    let _init_guard = HARDWARE_INIT_LOCK.lock().unwrap();

    // Double-check: 获取锁后再次检查缓存
    {
        let cache = STATIC_HARDWARE_CACHE.lock().unwrap();
        if let Some(ref info) = *cache {
            log::info!("从缓存获取静态硬件信息");
            return Ok(info.clone());
        }
    }

    log::info!("开始并行获取静态硬件信息...");

    let errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let errors_cpu = errors.clone();
    let cpu_handle = thread::spawn(move || {
        let cpu_cmd = "Get-WmiObject Win32_Processor | Select-Object Name, NumberOfCores, NumberOfLogicalProcessors, MaxClockSpeed, L3CacheSize, LoadPercentage | ConvertTo-Json -Compress";
        match run_powershell::<PsProcessor>(cpu_cmd) {
            Ok(cpu_results) => {
                log::info!("获取到{}个CPU信息", cpu_results.len());
                cpu_results.into_iter().next().map(|p| {
                    log::info!("CPU型号: {}", p.Name);
                    CpuInfo {
                        name: p.Name,
                        cores: p.NumberOfCores,
                        threads: p.NumberOfLogicalProcessors,
                        max_clock_speed: p.MaxClockSpeed,
                        l3_cache_size: p.L3CacheSize.unwrap_or(0),
                        load_percentage: p.LoadPercentage,
                    }
                })
            }
            Err(e) => {
                if let Ok(mut errs) = errors_cpu.lock() {
                    errs.push(format!("CPU: {}", e));
                }
                None
            }
        }
    });

    let gpu_handle = thread::spawn(move || {
        let gpu = get_gpu_info();
        gpu.into_iter().map(|g| GpuStaticInfo {
            name: g.name,
            vendor: g.vendor,
            memory_gb: g.memory_gb,
            driver_version: g.driver_version,
        }).collect::<Vec<GpuStaticInfo>>()
    });

    let errors_mobo = errors.clone();
    let mobo_handle = thread::spawn(move || {
        let mobo_cmd = "Get-WmiObject Win32_BaseBoard | Select-Object Manufacturer, Product | ConvertTo-Json -Compress";
        match run_powershell::<PsBaseBoard>(mobo_cmd) {
            Ok(results) => {
                log::info!("获取到{}个主板信息", results.len());
                results.into_iter().next().map(|m| {
                    log::info!("主板: {}", m.Product);
                    m.Product
                })
            }
            Err(e) => {
                if let Ok(mut errs) = errors_mobo.lock() {
                    errs.push(format!("主板: {}", e));
                }
                None
            }
        }
    });

    let errors_mem = errors.clone();
    let mem_handle = thread::spawn(move || {
        let mem_cmd = "Get-WmiObject Win32_PhysicalMemory | Select-Object Manufacturer, PartNumber, Capacity, Speed, BankLabel | ConvertTo-Json -Compress";
        match run_powershell::<PsPhysicalMemory>(mem_cmd) {
            Ok(results) => {
                log::info!("获取到{}个内存条信息", results.len());
                results.into_iter().map(|mem| {
                    let capacity_gb = mem.Capacity as f64 / (1024.0 * 1024.0 * 1024.0);
                    MemoryInfo {
                        manufacturer: mem.Manufacturer.unwrap_or_else(|| "未知".to_string()),
                        part_number: mem.PartNumber.unwrap_or_else(|| "未知".to_string()).trim().to_string(),
                        capacity_gb,
                        speed_mhz: mem.Speed.unwrap_or(0),
                        bank_label: mem.BankLabel.unwrap_or_else(|| "未知".to_string()),
                    }
                }).collect::<Vec<MemoryInfo>>()
            }
            Err(e) => {
                if let Ok(mut errs) = errors_mem.lock() {
                    errs.push(format!("内存: {}", e));
                }
                Vec::new()
            }
        }
    });

    let errors_disk = errors.clone();
    let disk_handle = thread::spawn(move || {
        let disk_cmd = "Get-WmiObject Win32_DiskDrive | Select-Object Model, Size | ConvertTo-Json -Compress";
        match run_powershell::<PsDiskDrive>(disk_cmd) {
            Ok(results) => {
                log::info!("获取到{}个硬盘信息", results.len());
                results.into_iter().map(|d| {
                    let size_gb = d.Size / (1024 * 1024 * 1024);
                    format!("{} ({}GB)", d.Model, size_gb)
                }).collect::<Vec<String>>()
            }
            Err(e) => {
                if let Ok(mut errs) = errors_disk.lock() {
                    errs.push(format!("硬盘: {}", e));
                }
                Vec::new()
            }
        }
    });

    let errors_sound = errors.clone();
    let sound_handle = thread::spawn(move || {
        let sound_cmd = r#"Get-WmiObject Win32_SoundDevice | Where-Object { $_.Status -eq 'OK' -and $_.PNPDeviceID -notlike 'USB\*' -and $_.PNPDeviceID -notlike 'HID\*' -and $_.Name -notlike '*Virtual*' -and $_.Name -notlike '*VB-Audio*' -and $_.Name -notlike '*Voicemeeter*' -and $_.Name -notlike '*CABLE*' -and $_.Name -notlike '*Sonic Studio*' -and $_.Name -notlike '*NVIDIA Virtual Audio*' -and $_.Name -notlike '*Steam Streaming*' -and $_.Name -notlike '*Oculus Virtual*' -and $_.Name -notlike '*Wave Link*' -and $_.Name -notlike '*Elgato Sound Capture*' } | Select-Object Name, Manufacturer | ConvertTo-Json -Compress"#;
        match run_powershell::<PsSoundDevice>(sound_cmd) {
            Ok(results) => {
                log::info!("获取到{}个声卡信息", results.len());
                results.into_iter().map(|s| {
                    SoundCardInfo {
                        name: s.Name,
                        manufacturer: s.Manufacturer.unwrap_or_else(|| "未知".to_string()),
                    }
                }).collect::<Vec<SoundCardInfo>>()
            }
            Err(e) => {
                if let Ok(mut errs) = errors_sound.lock() {
                    errs.push(format!("声卡: {}", e));
                }
                Vec::new()
            }
        }
    });

    let errors_network = errors.clone();
    let network_handle = thread::spawn(move || {
        let network_cmd = r#"[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Get-WmiObject Win32_NetworkAdapter | Where-Object { $_.PhysicalAdapter -eq $true -and $_.NetEnabled -eq $true -and $_.Name -notlike '*Hyper-V*' -and $_.Name -notlike '*vEthernet*' -and $_.Name -notlike '*VirtualBox*' -and $_.Name -notlike '*VMware*' -and $_.Name -notlike '*Bluetooth*' -and $_.AdapterType -notlike '*Loopback*' } | Select-Object Name, Manufacturer, AdapterType, MACAddress, Speed | ConvertTo-Json -Compress"#;
        match run_powershell::<PsNetworkAdapter>(network_cmd) {
            Ok(results) => {
                log::info!("获取到{}个网卡信息", results.len());
                results.into_iter().map(|n| {
                    // Speed is in bits per second, convert to Mbps
                    let speed_mbps = n.Speed.map(|s| s / 1_000_000).unwrap_or(0);
                    NetworkCardInfo {
                        name: n.Name,
                        manufacturer: n.Manufacturer.unwrap_or_else(|| "未知".to_string()),
                        adapter_type: n.AdapterType.unwrap_or_else(|| "未知".to_string()),
                        mac_address: n.MACAddress.unwrap_or_else(|| "未知".to_string()),
                        speed_mbps,
                    }
                }).collect::<Vec<NetworkCardInfo>>()
            }
            Err(e) => {
                if let Ok(mut errs) = errors_network.lock() {
                    errs.push(format!("网卡: {}", e));
                }
                Vec::new()
            }
        }
    });

    let cpu = cpu_handle.join().unwrap_or_else(|_| None).unwrap_or_else(|| CpuInfo {
        name: "未知CPU".to_string(),
        cores: 0,
        threads: 0,
        max_clock_speed: 0,
        l3_cache_size: 0,
        load_percentage: None,
    });

    let gpu_static = gpu_handle.join().unwrap_or_else(|_| Vec::new());
    let motherboard = mobo_handle.join().unwrap_or_else(|_| None).unwrap_or_else(|| "未知主板".to_string());
    let memory = mem_handle.join().unwrap_or_else(|_| Vec::new());
    let disk = disk_handle.join().unwrap_or_else(|_| Vec::new());
    let sound_card = sound_handle.join().unwrap_or_else(|_| Vec::new());
    let network_card = network_handle.join().unwrap_or_else(|_| Vec::new());

    if let Ok(errs) = errors.lock() {
        for e in errs.iter() {
            log::warn!("硬件获取警告: {}", e);
        }
    }

    let static_info = StaticHardwareInfo {
        cpu,
        gpu_static,
        motherboard,
        memory,
        disk,
        sound_card,
        network_card,
    };

    {
        let mut cache = STATIC_HARDWARE_CACHE.lock().unwrap();
        *cache = Some(static_info.clone());
    }

    log::info!("静态硬件信息并行获取完成");
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
                vendor: gs.vendor.clone(),
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
        sound_card: static_info.sound_card,
        network_card: static_info.network_card,
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
    pub temperature: Option<f64>,
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

#[tauri::command]
pub fn is_nvidia_gpu() -> bool {
    let cache = STATIC_HARDWARE_CACHE.lock().unwrap();
    cache
        .as_ref()
        .and_then(|c| c.gpu_static.first())
        .map(|g| g.vendor == GpuVendor::NVIDIA)
        .unwrap_or(false)
}

#[tauri::command]
pub fn get_os_version() -> Result<String, String> {
    sysinfo::System::long_os_version().ok_or_else(|| "无法获取操作系统版本".to_string())
}

// ─── Disk Health ───

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PartitionInfo {
    pub drive_letter: String,
    pub total_gb: f64,
    pub available_gb: f64,
    pub used_gb: f64,
    pub usage_percent: f64,
    pub filesystem: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiskHealthInfo {
    pub index: u32,
    pub model: String,
    pub media_type: String,
    pub size_gb: f64,
    pub interface_type: String,
    pub health_status: String,
    pub operational_status: String,
    pub temperature_c: Option<f64>,
    pub wear_percentage: Option<f64>,
    pub power_on_hours: Option<u64>,
    pub read_errors: Option<u64>,
    pub write_errors: Option<u64>,
    pub status: String,
    pub partition_count: u32,
    pub serial_number: String,
    pub partition_style: String,
    pub is_boot_disk: bool,
    pub partitions: Vec<PartitionInfo>,
    pub total_usage_gb: f64,
    pub total_capacity_gb: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiskHealthResponse {
    pub disks: Vec<DiskHealthInfo>,
    pub total_count: u32,
    pub healthy_count: u32,
    pub warning_count: u32,
    pub unhealthy_count: u32,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case, dead_code)]
struct PsDiskHealthRaw {
    DeviceId: String,
    FriendlyName: String,
    Model: String,
    MediaType: Option<String>,
    Size: Option<u64>,
    BusType: String,
    HealthStatus: String,
    OperationalStatus: String,
    SerialNumber: Option<String>,
    NumberOfPartitions: Option<u32>,
    Temperature: Option<f64>,
    WearPercentage: Option<f64>,
    PowerOnHours: Option<u64>,
    ReadErrorsTotal: Option<u64>,
    WriteErrorsTotal: Option<u64>,
    WmiStatus: Option<String>,
    WmiInterfaceType: Option<String>,
    PartitionStyle: Option<String>,
    IsBoot: Option<bool>,
    Partitions: Option<Vec<PsPartitionRaw>>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct PsPartitionRaw {
    DriveLetter: Option<String>,
    SizeRemaining: Option<u64>,
    Size: Option<u64>,
    FileSystem: Option<String>,
}

fn get_disk_health_info_inner() -> Result<DiskHealthResponse, String> {
    let ps_script = r#"
$disks = Get-PhysicalDisk -ErrorAction SilentlyContinue
$wmiDisks = Get-WmiObject Win32_DiskDrive -ErrorAction SilentlyContinue
$diskLayout = Get-WmiObject -Namespace root\Microsoft\Windows\Storage -Class MSFT_Disk -ErrorAction SilentlyContinue

# Try multiple methods to get reliability/SMART data
$reliabilityMap = @{}
# Method 1: Standard PowerShell pipeline
try {
    $relData = Get-PhysicalDisk -ErrorAction SilentlyContinue | Get-StorageReliabilityCounter -ErrorAction SilentlyContinue
    if ($relData) {
        foreach ($r in $relData) {
            $reliabilityMap[$r.DeviceId] = $r
        }
    }
} catch {}
# Method 2: WMI MSFT_StorageReliabilityCounter
if ($reliabilityMap.Count -eq 0) {
    try {
        $relWmi = Get-WmiObject -Namespace root\Microsoft\Windows\Storage -Class MSFT_StorageReliabilityCounter -ErrorAction SilentlyContinue
        if ($relWmi) {
            foreach ($r in $relWmi) {
                $did = if ($r.DeviceId) { $r.DeviceId } else { "PhysicalDrive$($r.PSComputerName)" }
                $reliabilityMap[$did] = $r
            }
        }
    } catch {}
}
# Method 3: CIM MSFT_StorageReliabilityCounter
if ($reliabilityMap.Count -eq 0) {
    try {
        $relCim = Get-CimInstance -Namespace root\Microsoft\Windows\Storage -ClassName MSFT_StorageReliabilityCounter -ErrorAction SilentlyContinue
        if ($relCim) {
            foreach ($r in $relCim) {
                $reliabilityMap[$r.DeviceId] = $r
            }
        }
    } catch {}
}

$result = $disks | ForEach-Object {
    $d = $_
    $rel = $reliabilityMap[$d.DeviceId]
    $diskNum = [int]($d.DeviceId -replace '.*?(\d+)$', '$1')

    $wmiMatch = $wmiDisks | Where-Object { $_.Model -eq $d.Model -or $_.Model -eq $d.FriendlyName } | Select-Object -First 1

    $layout = $diskLayout | Where-Object { $_.Number -eq $diskNum } | Select-Object -First 1

    $parts = Get-Partition -DiskNumber $diskNum -ErrorAction SilentlyContinue | Where-Object { $_.DriveLetter } | ForEach-Object {
        $vol = Get-Volume -DriveLetter $_.DriveLetter -ErrorAction SilentlyContinue
        [PSCustomObject]@{
            DriveLetter = if ($vol) { $_.DriveLetter.ToString() } else { $null }
            SizeRemaining = if ($vol) { [long]$vol.SizeRemaining } else { $null }
            Size = if ($vol) { [long]$vol.Size } else { $null }
            FileSystem = if ($vol -and $vol.FileSystemType) { $vol.FileSystemType.ToString() } else { $null }
        }
    }

    [PSCustomObject]@{
        DeviceId = $d.DeviceId
        FriendlyName = $d.FriendlyName
        Model = if ($d.Model) { $d.Model } else { $d.FriendlyName }
        MediaType = if ($d.MediaType) { $d.MediaType.ToString() } else { $null }
        Size = if ($d.Size) { [long]$d.Size } else { $null }
        BusType = $d.BusType.ToString()
        HealthStatus = $d.HealthStatus.ToString()
        OperationalStatus = $d.OperationalStatus.ToString()
        SerialNumber = $d.SerialNumber
        NumberOfPartitions = if ($d.NumberOfPartitions -ne $null) { [int]$d.NumberOfPartitions } else { $null }
        Temperature = if ($rel -and $rel.Temperature -ne $null) { [double]$rel.Temperature } else { $null }
        WearPercentage = if ($rel -and $rel.WearPercentage -ne $null) { [double]$rel.WearPercentage } else { $null }
        PowerOnHours = if ($rel -and $rel.PowerOnHours -ne $null) { [long]$rel.PowerOnHours } else { $null }
        ReadErrorsTotal = if ($rel -and $rel.ReadErrorsTotal -ne $null) { [long]$rel.ReadErrorsTotal } else { $null }
        WriteErrorsTotal = if ($rel -and $rel.WriteErrorsTotal -ne $null) { [long]$rel.WriteErrorsTotal } else { $null }
        WmiStatus = if ($wmiMatch) { $wmiMatch.Status } else { $null }
        WmiInterfaceType = if ($wmiMatch) { $wmiMatch.InterfaceType } else { $null }
        PartitionStyle = if ($layout) { if ($layout.PartitionStyle -eq 1) { "MBR" } elseif ($layout.PartitionStyle -eq 2) { "GPT" } elseif ($layout.PartitionStyle -eq 3) { "RAW" } else { "Unknown" } } else { $null }
        IsBoot = if ($layout) { [bool]$layout.IsBoot } else { $false }
        Partitions = @($parts)
    }
}

if ($result) { $result | ConvertTo-Json -Depth 3 -Compress } else { '[]' }
"#;

    let raw_disks = run_powershell::<PsDiskHealthRaw>(ps_script)
        .map_err(|e| format!("获取磁盘信息失败: {}", e))?;

    if raw_disks.is_empty() {
        return Ok(DiskHealthResponse {
            disks: vec![],
            total_count: 0,
            healthy_count: 0,
            warning_count: 0,
            unhealthy_count: 0,
        });
    }

    let mut disk_infos = Vec::new();
    let mut healthy = 0u32;
    let mut warning = 0u32;
    let mut unhealthy = 0u32;

    for (i, d) in raw_disks.iter().enumerate() {
        let media_type = d.MediaType.as_deref().unwrap_or("Unknown").to_string();
        let size_gb = d.Size.map(|s| s as f64 / 1_000_000_000.0).unwrap_or(0.0);
        let health_status = d.HealthStatus.clone();
        let partition_count = d.NumberOfPartitions.unwrap_or(0);
        let serial = d.SerialNumber.as_deref().unwrap_or("").to_string();
        let interface_type = d.WmiInterfaceType.as_deref().unwrap_or(&d.BusType).to_string();

        match health_status.to_lowercase().as_str() {
            "healthy" => healthy += 1,
            "warning" => warning += 1,
            _ => unhealthy += 1,
        }

        let partitions: Vec<PartitionInfo> = d.Partitions.as_ref().map(|parts| {
            parts.iter().filter_map(|p| {
                let letter = p.DriveLetter.as_deref()?;
                let total = p.Size.unwrap_or(0) as f64;
                let available = p.SizeRemaining.unwrap_or(0) as f64;
                let used = total - available;
                let usage_pct = if total > 0.0 { (used / total) * 100.0 } else { 0.0 };
                let fs = p.FileSystem.as_deref().unwrap_or("").to_string();
                Some(PartitionInfo {
                    drive_letter: letter.to_string(),
                    total_gb: total / 1_000_000_000.0,
                    available_gb: available / 1_000_000_000.0,
                    used_gb: used / 1_000_000_000.0,
                    usage_percent: usage_pct,
                    filesystem: fs,
                })
            }).collect()
        }).unwrap_or_default();

        let total_capacity_gb: f64 = partitions.iter().map(|p| p.total_gb).sum();
        let total_usage_gb: f64 = partitions.iter().map(|p| p.used_gb).sum();

        disk_infos.push(DiskHealthInfo {
            index: i as u32,
            model: d.FriendlyName.clone(),
            media_type,
            size_gb,
            interface_type,
            health_status,
            operational_status: d.OperationalStatus.clone(),
            temperature_c: d.Temperature,
            wear_percentage: d.WearPercentage,
            power_on_hours: d.PowerOnHours,
            read_errors: d.ReadErrorsTotal,
            write_errors: d.WriteErrorsTotal,
            status: d.WmiStatus.as_deref().unwrap_or("").to_string(),
            partition_count,
            serial_number: serial,
            partition_style: d.PartitionStyle.as_deref().unwrap_or("").to_string(),
            is_boot_disk: d.IsBoot.unwrap_or(false),
            partitions,
            total_usage_gb,
            total_capacity_gb,
        });
    }

    Ok(DiskHealthResponse {
        disks: disk_infos,
        total_count: raw_disks.len() as u32,
        healthy_count: healthy,
        warning_count: warning,
        unhealthy_count: unhealthy,
    })
}

#[tauri::command]
pub async fn get_disk_health_info() -> Result<DiskHealthResponse, String> {
    match tauri::async_runtime::spawn_blocking(|| get_disk_health_info_inner()).await {
        Ok(Ok(info)) => Ok(info),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(format!("异步任务失败: {}", e)),
    }
}

pub fn cleanup_hardware_cache() {
    let mut cache = STATIC_HARDWARE_CACHE.lock().unwrap();
    *cache = None;

    let mut cpu_system = CPU_SYSTEM.lock().unwrap();
    *cpu_system = None;
    
    log::info!("硬件信息缓存已清理");
}
