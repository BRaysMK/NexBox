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
    pub manufacturer: String,
    pub cores: u32,
    pub threads: u32,
    pub max_clock_speed: u32,
    pub l2_cache_size: u32,
    pub l3_cache_size: u32,
    pub load_percentage: Option<u16>,
    pub architecture: String,
    pub socket: String,
    pub l2_cache_speed: Option<u32>,
    pub l3_cache_speed: Option<u32>,
    pub current_clock_speed: Option<u32>,
    pub ext_clock: Option<u32>,
    pub processor_id: String,
    pub family: u32,
    pub stepping: String,
    pub revision: String,
    pub enabled_cores: Option<u32>,
    pub voltage_caps: Option<String>,
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
    pub video_processor: String,
    pub adapter_compatibility: String,
    pub driver_date: String,
    pub installed_drivers: String,
    pub video_mode: String,
    pub resolution_width: Option<u32>,
    pub resolution_height: Option<u32>,
    pub refresh_rate: Option<u32>,
    pub device_id: String,
    pub pnp_device_id: String,
    pub status: String,
    pub inf_filename: String,
    pub video_architecture: Option<String>,
    pub video_memory_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryInfo {
    pub manufacturer: String,
    pub part_number: String,
    pub capacity_gb: f64,
    pub speed_mhz: u32,
    pub bank_label: String,
    pub form_factor: String,
    pub memory_type: String,
    pub configured_clock_speed: Option<u32>,
    pub configured_voltage: Option<u32>,
    pub data_width: Option<u32>,
    pub total_width: Option<u32>,
    pub serial_number: String,
    pub type_detail: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MotherboardInfo {
    pub product: String,
    pub manufacturer: String,
    pub serial_number: String,
    pub version: String,
    pub bios_vendor: String,
    pub bios_version: String,
    pub bios_release_date: String,
    pub system_manufacturer: String,
    pub system_model: String,
    pub system_type: String,
    pub chassis_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiskDetailInfo {
    pub model: String,
    pub size_gb: f64,
    pub interface_type: String,
    pub serial_number: String,
    pub firmware_revision: String,
    pub media_type: String,
    pub bytes_per_sector: Option<u32>,
    pub partitions: u32,
    pub status: String,
    pub is_ssd: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MonitorInfo {
    pub name: String,
    pub manufacturer: String,
    pub screen_width: Option<u32>,
    pub screen_height: Option<u32>,
    pub refresh_rate: Option<u32>,
    pub pnp_device_id: String,
    pub status: String,
    pub availability: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HardwareInfo {
    pub cpu: CpuInfo,
    pub gpu: Vec<GpuInfo>,
    pub memory: Vec<MemoryInfo>,
    pub motherboard: MotherboardInfo,
    pub disk: Vec<DiskDetailInfo>,
    pub monitor: Vec<MonitorInfo>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct PsProcessor {
    Name: String,
    NumberOfCores: u32,
    NumberOfLogicalProcessors: u32,
    MaxClockSpeed: u32,
    L2CacheSize: Option<u32>,
    L2CacheSpeed: Option<u32>,
    L3CacheSize: Option<u32>,
    L3CacheSpeed: Option<u32>,
    LoadPercentage: Option<u16>,
    Manufacturer: Option<String>,
    Architecture: Option<u16>,
    SocketDesignation: Option<String>,
    CurrentClockSpeed: Option<u32>,
    ExtClock: Option<u32>,
    ProcessorId: Option<String>,
    Family: Option<u32>,
    Stepping: Option<String>,
    Revision: Option<u16>,
    NumberOfEnabledCore: Option<u32>,
    VoltageCaps: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct PsVideoController {
    Name: String,
    DriverVersion: Option<String>,
    AdapterRAM: Option<u64>,
    VideoProcessor: Option<String>,
    AdapterCompatibility: Option<String>,
    DriverDate: Option<String>,
    InstalledDisplayDrivers: Option<String>,
    VideoModeDescription: Option<String>,
    CurrentHorizontalResolution: Option<u32>,
    CurrentVerticalResolution: Option<u32>,
    CurrentRefreshRate: Option<u32>,
    DeviceID: Option<String>,
    PNPDeviceID: Option<String>,
    Status: Option<String>,
    InfFilename: Option<String>,
    VideoArchitecture: Option<u16>,
    VideoMemoryType: Option<u16>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case, dead_code)]
struct PsBaseBoard {
    Manufacturer: Option<String>,
    Product: Option<String>,
    SerialNumber: Option<String>,
    Version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct PsComputerSystem {
    Manufacturer: Option<String>,
    Model: Option<String>,
    SystemType: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct PsBios {
    SMBIOSBIOSVersion: Option<String>,
    Manufacturer: Option<String>,
    ReleaseDate: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct PsSystemEnclosure {
    ChassisTypes: Option<Vec<u16>>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct PsPhysicalMemory {
    Manufacturer: Option<String>,
    PartNumber: Option<String>,
    Capacity: u64,
    Speed: Option<u32>,
    BankLabel: Option<String>,
    FormFactor: Option<u16>,
    MemoryType: Option<u16>,
    ConfiguredClockSpeed: Option<u32>,
    ConfiguredVoltage: Option<u32>,
    DataWidth: Option<u32>,
    TotalWidth: Option<u32>,
    SerialNumber: Option<String>,
    TypeDetail: Option<u16>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct PsDiskDrive {
    Model: String,
    Size: u64,
    InterfaceType: Option<String>,
    SerialNumber: Option<String>,
    FirmwareRevision: Option<String>,
    MediaType: Option<String>,
    BytesPerSector: Option<u32>,
    Partitions: Option<u32>,
    Status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct PsDesktopMonitor {
    Name: Option<String>,
    MonitorManufacturerName: Option<String>,
    ScreenWidth: Option<u32>,
    ScreenHeight: Option<u32>,
    DisplayFrequency: Option<u32>,
    PNPDeviceID: Option<String>,
    Status: Option<String>,
    Availability: Option<u16>,
}

/// EDID-based monitor name from WmiMonitorID (root\wmi)
#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct PsWmiMonitorId {
    UserFriendlyName: Option<String>,
    ManufacturerName: Option<String>,
}

/// Check if a monitor name is a generic/placeholder (any language variant)
fn is_generic_monitor_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("generic")
        || lower.contains("即插即用")
        || lower.contains("通用")
        || lower.contains("pnp")
        || lower.contains("standard monitor")
        || lower.contains("digital display")
        || lower.contains("analog display")
}

/// Query EDID-based monitor names via WMI WmiMonitorID from root\\wmi namespace.
/// This reads the real monitor model name directly from EDID, bypassing
/// driver-level limitations that cause "Generic PnP Monitor" fallback.
/// WmiMonitorID stores names as uint16[] arrays (Unicode code points),
/// so we iterate and cast each element to [char]. Stop at the first
/// null character (0x0000) to avoid trailing garbage displayed as □.
fn query_edid_monitor_names() -> Vec<String> {
    let cmd = "Get-WmiObject -Namespace root\\wmi WmiMonitorID | ForEach-Object { $friendly = ''; if ($_.UserFriendlyNameLength -gt 0) { $arr = @($_.UserFriendlyName); $max = [Math]::Min($arr.Count, $_.UserFriendlyNameLength); for ($i = 0; $i -lt $max; $i++) { $c = [char]$arr[$i]; if ($c -eq [char]0) { break } $friendly += $c } }; $mfr = ''; if ($_.ManufacturerNameLength -gt 0) { $arr2 = @($_.ManufacturerName); $max2 = [Math]::Min($arr2.Count, $_.ManufacturerNameLength); for ($i = 0; $i -lt $max2; $i++) { $c2 = [char]$arr2[$i]; if ($c2 -eq [char]0) { break } $mfr += $c2 } }; [PSCustomObject]@{ UserFriendlyName = $friendly.Trim(); ManufacturerName = $mfr.Trim() } } | ConvertTo-Json -Compress";

    match run_powershell::<PsWmiMonitorId>(cmd) {
        Ok(results) => {
            results
                .into_iter()
                .map(|m| m.UserFriendlyName.unwrap_or_default())
                .collect()
        }
        Err(e) => {
            log::warn!("EDID显示器名称查询失败: {}", e);
            Vec::new()
        }
    }
}

// 静态硬件信息缓存（不会变化的部分）
#[derive(Debug, Clone)]
struct StaticHardwareInfo {
    cpu: CpuInfo,
    gpu_static: Vec<GpuStaticInfo>,
    motherboard: MotherboardInfo,
    memory: Vec<MemoryInfo>,
    disk: Vec<DiskDetailInfo>,
    monitor: Vec<MonitorInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GpuStaticInfo {
    name: String,
    vendor: GpuVendor,
    memory_gb: f64,
    driver_version: String,
    video_processor: String,
    adapter_compatibility: String,
    driver_date: String,
    installed_drivers: String,
    video_mode: String,
    resolution_width: Option<u32>,
    resolution_height: Option<u32>,
    refresh_rate: Option<u32>,
    device_id: String,
    pnp_device_id: String,
    status: String,
    inf_filename: String,
    video_architecture: Option<String>,
    video_memory_type: Option<String>,
}

static STATIC_HARDWARE_CACHE: Mutex<Option<StaticHardwareInfo>> = Mutex::new(None);
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
            video_processor: String::new(),
            adapter_compatibility: "NVIDIA".to_string(),
            driver_date: String::new(),
            installed_drivers: String::new(),
            video_mode: String::new(),
            resolution_width: None,
            resolution_height: None,
            refresh_rate: None,
            device_id: String::new(),
            pnp_device_id: String::new(),
            status: String::new(),
            inf_filename: String::new(),
            video_architecture: None,
            video_memory_type: None,
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
                        video_processor: String::new(),
                        adapter_compatibility: "NVIDIA".to_string(),
                        driver_date: String::new(),
                        installed_drivers: String::new(),
                        video_mode: String::new(),
                        resolution_width: None,
                        resolution_height: None,
                        refresh_rate: None,
                        device_id: String::new(),
                        pnp_device_id: String::new(),
                        status: String::new(),
                        inf_filename: String::new(),
                        video_architecture: None,
                        video_memory_type: None,
                    });
                }
            }
        }
    }

    gpus
}

fn video_architecture_name_old(code: Option<u16>) -> Option<String> {
    match code {
        Some(1) => Some("VGA".into()),
        Some(7) => Some("DVI".into()),
        Some(8) => Some("HDMI".into()),
        Some(12) => Some("DisplayPort (External)".into()),
        Some(13) => Some("DisplayPort (Embedded)".into()),
        _ => None,
    }
}

fn video_memory_type_name_old(code: Option<u16>) -> Option<String> {
    match code {
        Some(14) => Some("GDDR3".into()),
        Some(15) => Some("GDDR4".into()),
        Some(16) => Some("GDDR5".into()),
        Some(17) => Some("HBM".into()),
        Some(18) => Some("HBM2".into()),
        Some(19) => Some("GDDR5X".into()),
        Some(20) => Some("GDDR6".into()),
        Some(21) => Some("GDDR6X".into()),
        Some(22) => Some("GDDR7".into()),
        Some(23) => Some("HBM3".into()),
        _ => None,
    }
}

fn get_gpus_from_wmi() -> Vec<GpuInfo> {
    let mut gpus = Vec::new();

    let gpu_cmd = "Get-WmiObject Win32_VideoController | Select-Object Name, DriverVersion, AdapterRAM, VideoProcessor, AdapterCompatibility, DriverDate, InstalledDisplayDrivers, VideoModeDescription, CurrentHorizontalResolution, CurrentVerticalResolution, CurrentRefreshRate, DeviceID, PNPDeviceID, Status, InfFilename, VideoArchitecture, VideoMemoryType | ConvertTo-Json -Compress";
    if let Ok(gpu_results) = run_powershell::<PsVideoController>(gpu_cmd) {
        for g in gpu_results {
            let name_lower = g.Name.to_lowercase();
            let vendor = detect_gpu_vendor(&g.Name);
            let memory_gb = g.AdapterRAM
                .map(|ram| ram as f64 / (1024.0 * 1024.0 * 1024.0))
                .unwrap_or(0.0);

            let is_integrated = match vendor {
                GpuVendor::Intel => !name_lower.contains("arc"),
                GpuVendor::AMD => {
                    name_lower.contains("radeon") && name_lower.contains("graphics")
                        && !name_lower.contains("rx ")
                }
                _ => false,
            };

            if is_integrated {
                log::info!("跳过核显(WMI): {}, 厂商: {:?}, 显存: {:.1}GB", g.Name, vendor, memory_gb);
                continue;
            }

            log::info!("显卡(WMI): {}, 厂商: {:?}, 显存: {:.1}GB", g.Name, vendor, memory_gb);
            
            gpus.push(GpuInfo {
                name: g.Name.clone(),
                vendor,
                memory_gb,
                driver_version: g.DriverVersion.unwrap_or_else(|| "未知".to_string()),
                temperature: None,
                usage: None,
                video_processor: g.VideoProcessor.unwrap_or_else(|| "未知".to_string()),
                adapter_compatibility: g.AdapterCompatibility.unwrap_or_else(|| "未知".to_string()),
                driver_date: g.DriverDate.unwrap_or_else(|| "未知".to_string()),
                installed_drivers: g.InstalledDisplayDrivers.unwrap_or_else(|| "未知".to_string()),
                video_mode: g.VideoModeDescription.unwrap_or_else(|| "未知".to_string()),
                resolution_width: g.CurrentHorizontalResolution,
                resolution_height: g.CurrentVerticalResolution,
                refresh_rate: g.CurrentRefreshRate,
                device_id: g.DeviceID.unwrap_or_else(|| "未知".to_string()),
                pnp_device_id: g.PNPDeviceID.unwrap_or_else(|| "未知".to_string()),
                status: g.Status.unwrap_or_else(|| "未知".to_string()),
                inf_filename: g.InfFilename.unwrap_or_else(|| "未知".to_string()),
                video_architecture: video_architecture_name_old(g.VideoArchitecture),
                video_memory_type: video_memory_type_name_old(g.VideoMemoryType),
            });
        }
    }

    gpus
}

fn get_gpu_info() -> Vec<GpuInfo> {
    let wmi_gpus = get_gpus_from_wmi();

    if let Ok(mut nvml_gpus) = get_nvidia_gpus_with_nvml() {
        if !nvml_gpus.is_empty() {
            for gpu in nvml_gpus.iter_mut() {
                if let Some(wmi_match) = wmi_gpus.iter().find(|w| w.vendor == GpuVendor::NVIDIA) {
                    gpu.video_processor = wmi_match.video_processor.clone();
                    gpu.adapter_compatibility = wmi_match.adapter_compatibility.clone();
                    gpu.driver_date = wmi_match.driver_date.clone();
                    gpu.installed_drivers = wmi_match.installed_drivers.clone();
                    gpu.video_mode = wmi_match.video_mode.clone();
                    gpu.resolution_width = wmi_match.resolution_width;
                    gpu.resolution_height = wmi_match.resolution_height;
                    gpu.refresh_rate = wmi_match.refresh_rate;
                    gpu.device_id = wmi_match.device_id.clone();
                    gpu.pnp_device_id = wmi_match.pnp_device_id.clone();
                    gpu.status = wmi_match.status.clone();
                    gpu.inf_filename = wmi_match.inf_filename.clone();
                    gpu.video_architecture = wmi_match.video_architecture.clone();
                    gpu.video_memory_type = wmi_match.video_memory_type.clone();
                    break;
                }
            }
            return nvml_gpus;
        }
    }

    let mut smi_gpus = get_nvidia_gpus_with_smi();
    if !smi_gpus.is_empty() {
        for gpu in smi_gpus.iter_mut() {
            if let Some(wmi_match) = wmi_gpus.iter().find(|w| w.vendor == GpuVendor::NVIDIA) {
                gpu.video_processor = wmi_match.video_processor.clone();
                gpu.adapter_compatibility = wmi_match.adapter_compatibility.clone();
                gpu.driver_date = wmi_match.driver_date.clone();
                gpu.installed_drivers = wmi_match.installed_drivers.clone();
                gpu.video_mode = wmi_match.video_mode.clone();
                gpu.resolution_width = wmi_match.resolution_width;
                gpu.resolution_height = wmi_match.resolution_height;
                gpu.refresh_rate = wmi_match.refresh_rate;
                gpu.device_id = wmi_match.device_id.clone();
                gpu.pnp_device_id = wmi_match.pnp_device_id.clone();
                gpu.status = wmi_match.status.clone();
                gpu.inf_filename = wmi_match.inf_filename.clone();
                gpu.video_architecture = wmi_match.video_architecture.clone();
                gpu.video_memory_type = wmi_match.video_memory_type.clone();
                break;
            }
        }
        return smi_gpus;
    }

    wmi_gpus
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

fn architecture_name_old(code: Option<u16>) -> String {
    match code {
        Some(0) => "x86".into(),
        Some(9) => "x64".into(),
        Some(12) => "ARM64".into(),
        _ => "未知".into(),
    }
}

fn memory_form_factor_name_old(code: Option<u16>) -> String {
    match code {
        Some(8) => "DIMM".into(),
        Some(12) => "SODIMM".into(),
        Some(24) => "FB-DIMM".into(),
        _ => "未知".into(),
    }
}

fn memory_type_name_old(code: Option<u16>) -> String {
    match code {
        Some(20) => "DDR".into(),
        Some(21) => "DDR2".into(),
        Some(24) => "DDR3".into(),
        Some(26) => "DDR4".into(),
        Some(34) => "DDR5".into(),
        _ => "未知".into(),
    }
}

fn chassis_type_name_old(codes: &Option<Vec<u16>>) -> String {
    match codes.as_ref().and_then(|v| v.first()).copied() {
        Some(3) => "Desktop".into(),
        Some(8) => "Portable".into(),
        Some(9) => "Laptop".into(),
        Some(10) => "Notebook".into(),
        Some(13) => "All in One".into(),
        Some(30) => "Tablet".into(),
        Some(35) => "Mini PC".into(),
        _ => "未知".into(),
    }
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

    log::info!("开始并行获取静态硬件信息...");

    let errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let errors_cpu = errors.clone();
    let cpu_handle = thread::spawn(move || {
        let cpu_cmd = "Get-WmiObject Win32_Processor | Select-Object Name, NumberOfCores, NumberOfLogicalProcessors, MaxClockSpeed, L2CacheSize, L2CacheSpeed, L3CacheSize, L3CacheSpeed, LoadPercentage, Manufacturer, Architecture, SocketDesignation, CurrentClockSpeed, ExtClock, ProcessorId, Family, Stepping, Revision, NumberOfEnabledCore, VoltageCaps | ConvertTo-Json -Compress";
        match run_powershell::<PsProcessor>(cpu_cmd) {
            Ok(cpu_results) => {
                log::info!("获取到{}个CPU信息", cpu_results.len());
                cpu_results.into_iter().next().map(|p| {
                    CpuInfo {
                        name: p.Name,
                        manufacturer: p.Manufacturer.unwrap_or_else(|| "未知".to_string()),
                        cores: p.NumberOfCores,
                        threads: p.NumberOfLogicalProcessors,
                        max_clock_speed: p.MaxClockSpeed,
                        l2_cache_size: p.L2CacheSize.unwrap_or(0),
                        l3_cache_size: p.L3CacheSize.unwrap_or(0),
                        load_percentage: p.LoadPercentage,
                        architecture: architecture_name_old(p.Architecture),
                        socket: p.SocketDesignation.unwrap_or_else(|| "未知".to_string()),
                        l2_cache_speed: p.L2CacheSpeed,
                        l3_cache_speed: p.L3CacheSpeed,
                        current_clock_speed: p.CurrentClockSpeed,
                        ext_clock: p.ExtClock,
                        processor_id: p.ProcessorId.unwrap_or_else(|| "未知".to_string()),
                        family: p.Family.unwrap_or(0),
                        stepping: p.Stepping.unwrap_or_else(|| "未知".to_string()),
                        revision: p.Revision.map(|r| r.to_string()).unwrap_or_else(|| "未知".to_string()),
                        enabled_cores: p.NumberOfEnabledCore,
                        voltage_caps: p.VoltageCaps.map(|v| format!("{} mV", v)),
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
            video_processor: g.video_processor,
            adapter_compatibility: g.adapter_compatibility,
            driver_date: g.driver_date,
            installed_drivers: g.installed_drivers,
            video_mode: g.video_mode,
            resolution_width: g.resolution_width,
            resolution_height: g.resolution_height,
            refresh_rate: g.refresh_rate,
            device_id: g.device_id,
            pnp_device_id: g.pnp_device_id,
            status: g.status,
            inf_filename: g.inf_filename,
            video_architecture: g.video_architecture,
            video_memory_type: g.video_memory_type,
        }).collect::<Vec<GpuStaticInfo>>()
    });

    let errors_mobo = errors.clone();
    let mobo_handle = thread::spawn(move || {
        let mobo_cmd = "Get-WmiObject Win32_BaseBoard | Select-Object Manufacturer, Product, SerialNumber, Version | ConvertTo-Json -Compress";
        let sys_cmd = "Get-WmiObject Win32_ComputerSystem | Select-Object Manufacturer, Model, SystemType | ConvertTo-Json -Compress";
        let bios_cmd = "Get-WmiObject Win32_BIOS | Select-Object SMBIOSBIOSVersion, Manufacturer, Name, ReleaseDate | ConvertTo-Json -Compress";
        let chassis_cmd = "Get-WmiObject Win32_SystemEnclosure | Select-Object ChassisTypes, Manufacturer, Version | ConvertTo-Json -Compress";

        let mobo_result = run_powershell::<PsBaseBoard>(mobo_cmd).ok().and_then(|r| r.into_iter().next());
        let sys_result = run_powershell::<PsComputerSystem>(sys_cmd).ok().and_then(|r| r.into_iter().next());
        let bios_result = run_powershell::<PsBios>(bios_cmd).ok().and_then(|r| r.into_iter().next());
        let chassis_result = run_powershell::<PsSystemEnclosure>(chassis_cmd).ok().and_then(|r| r.into_iter().next());

        mobo_result.map(|m| MotherboardInfo {
            product: m.Product.unwrap_or_else(|| "未知".to_string()),
            manufacturer: m.Manufacturer.unwrap_or_else(|| "未知".to_string()),
            serial_number: m.SerialNumber.unwrap_or_else(|| "未知".to_string()),
            version: m.Version.unwrap_or_else(|| "未知".to_string()),
            bios_vendor: bios_result.as_ref().and_then(|b| b.Manufacturer.clone()).unwrap_or_else(|| "未知".to_string()),
            bios_version: bios_result.as_ref().and_then(|b| b.SMBIOSBIOSVersion.clone()).unwrap_or_else(|| "未知".to_string()),
            bios_release_date: bios_result.as_ref().and_then(|b| b.ReleaseDate.clone()).unwrap_or_else(|| "未知".to_string()),
            system_manufacturer: sys_result.as_ref().and_then(|s| s.Manufacturer.clone()).unwrap_or_else(|| "未知".to_string()),
            system_model: sys_result.as_ref().and_then(|s| s.Model.clone()).unwrap_or_else(|| "未知".to_string()),
            system_type: sys_result.as_ref().and_then(|s| s.SystemType.clone()).unwrap_or_else(|| "未知".to_string()),
            chassis_type: chassis_type_name_old(&chassis_result.as_ref().and_then(|c| c.ChassisTypes.clone())),
        })
    });

    let errors_mem = errors.clone();
    let mem_handle = thread::spawn(move || {
        let mem_cmd = "Get-WmiObject Win32_PhysicalMemory | Select-Object Manufacturer, PartNumber, Capacity, Speed, BankLabel, FormFactor, MemoryType, ConfiguredClockSpeed, ConfiguredVoltage, DataWidth, TotalWidth, SerialNumber, TypeDetail | ConvertTo-Json -Compress";
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
                        form_factor: memory_form_factor_name_old(mem.FormFactor),
                        memory_type: memory_type_name_old(mem.MemoryType),
                        configured_clock_speed: mem.ConfiguredClockSpeed,
                        configured_voltage: mem.ConfiguredVoltage,
                        data_width: mem.DataWidth,
                        total_width: mem.TotalWidth,
                        serial_number: mem.SerialNumber.unwrap_or_else(|| "未知".to_string()),
                        type_detail: mem.TypeDetail.map(|d| d.to_string()).unwrap_or_else(|| "未知".to_string()),
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
        let disk_cmd = "Get-WmiObject Win32_DiskDrive | Select-Object Model, Size, InterfaceType, SerialNumber, FirmwareRevision, MediaType, BytesPerSector, Partitions, Status | ConvertTo-Json -Compress";
        match run_powershell::<PsDiskDrive>(disk_cmd) {
            Ok(results) => {
                log::info!("获取到{}个硬盘信息", results.len());
                results.into_iter().map(|d| {
                    let size_gb = d.Size as f64 / (1024.0 * 1024.0 * 1024.0);
                    let media_type = d.MediaType.as_deref().unwrap_or("").to_string();
                    let is_ssd = media_type.to_lowercase().contains("ssd") || media_type.to_lowercase().contains("solid state");
                    DiskDetailInfo {
                        model: d.Model,
                        size_gb,
                        interface_type: d.InterfaceType.unwrap_or_else(|| "未知".to_string()),
                        serial_number: d.SerialNumber.unwrap_or_else(|| "未知".to_string()),
                        firmware_revision: d.FirmwareRevision.unwrap_or_else(|| "未知".to_string()),
                        media_type: if media_type.is_empty() { "未知".to_string() } else { media_type },
                        bytes_per_sector: d.BytesPerSector,
                        partitions: d.Partitions.unwrap_or(0),
                        status: d.Status.unwrap_or_else(|| "未知".to_string()),
                        is_ssd,
                    }
                }).collect::<Vec<DiskDetailInfo>>()
            }
            Err(e) => {
                if let Ok(mut errs) = errors_disk.lock() {
                    errs.push(format!("硬盘: {}", e));
                }
                Vec::new()
            }
        }
    });

    let errors_monitor = errors.clone();
    let monitor_handle = thread::spawn(move || {
        let monitor_cmd = "Get-WmiObject Win32_DesktopMonitor | Where-Object { $_.Name -ne $null -and $_.PNPDeviceID -ne $null } | Select-Object Name, MonitorManufacturerName, ScreenWidth, ScreenHeight, DisplayFrequency, PNPDeviceID, Status, Availability | ConvertTo-Json -Compress";
        match run_powershell::<PsDesktopMonitor>(monitor_cmd) {
            Ok(results) => {
                log::info!("获取到{}个显示器信息", results.len());
                let mut monitors: Vec<MonitorInfo> = results.into_iter().map(|m| {
                    MonitorInfo {
                        name: m.Name.unwrap_or_else(|| "未知".to_string()),
                        manufacturer: m.MonitorManufacturerName.unwrap_or_else(|| "未知".to_string()),
                        screen_width: m.ScreenWidth,
                        screen_height: m.ScreenHeight,
                        refresh_rate: m.DisplayFrequency,
                        pnp_device_id: m.PNPDeviceID.unwrap_or_else(|| "未知".to_string()),
                        status: m.Status.unwrap_or_else(|| "未知".to_string()),
                        availability: m.Availability,
                    }
                }).collect();

                // Fallback: if any monitor name is generic, query EDID for real model name
                let has_generic = monitors.iter().any(|m| is_generic_monitor_name(&m.name));
                if has_generic {
                    log::info!("检测到通用显示器名称，尝试从EDID获取真实型号...");
                    let edid_names = query_edid_monitor_names();
                    if !edid_names.is_empty() {
                        for (i, m) in monitors.iter_mut().enumerate() {
                            if is_generic_monitor_name(&m.name) {
                                if let Some(edid_name) = edid_names.get(i) {
                                    if !edid_name.is_empty() {
                                        log::info!("显示器[{}]: EDID替换 '{}' -> '{}'", i, m.name, edid_name);
                                        m.name = edid_name.clone();
                                    }
                                } else if edid_names.len() == 1 && !edid_names[0].is_empty() {
                                    log::info!("显示器[{}]: EDID替换(单显示器) '{}' -> '{}'", i, m.name, edid_names[0]);
                                    m.name = edid_names[0].clone();
                                }
                            }
                        }
                    }
                }
                monitors
            }
            Err(e) => {
                if let Ok(mut errs) = errors_monitor.lock() {
                    errs.push(format!("显示器: {}", e));
                }
                Vec::new()
            }
        }
    });

    let cpu = cpu_handle.join().unwrap_or_else(|_| None).unwrap_or_else(|| CpuInfo {
        name: "未知CPU".to_string(),
        manufacturer: "未知".to_string(),
        cores: 0,
        threads: 0,
        max_clock_speed: 0,
        l2_cache_size: 0,
        l3_cache_size: 0,
        load_percentage: None,
        architecture: "未知".to_string(),
        socket: "未知".to_string(),
        l2_cache_speed: None,
        l3_cache_speed: None,
        current_clock_speed: None,
        ext_clock: None,
        processor_id: "未知".to_string(),
        family: 0,
        stepping: "未知".to_string(),
        revision: "未知".to_string(),
        enabled_cores: None,
        voltage_caps: None,
    });

    let gpu_static = gpu_handle.join().unwrap_or_else(|_| Vec::new());
    let motherboard = mobo_handle.join().unwrap_or_else(|_| None).unwrap_or_else(|| MotherboardInfo {
        product: "未知".to_string(),
        manufacturer: "未知".to_string(),
        serial_number: "未知".to_string(),
        version: "未知".to_string(),
        bios_vendor: "未知".to_string(),
        bios_version: "未知".to_string(),
        bios_release_date: "未知".to_string(),
        system_manufacturer: "未知".to_string(),
        system_model: "未知".to_string(),
        system_type: "未知".to_string(),
        chassis_type: "未知".to_string(),
    });
    let memory = mem_handle.join().unwrap_or_else(|_| Vec::new());
    let disk = disk_handle.join().unwrap_or_else(|_| Vec::new());
    let mut monitor = monitor_handle.join().unwrap_or_else(|_| Vec::new());
    // Fallback: fill monitor resolution/refresh from GPU output if WMI didn't provide it
    if !gpu_static.is_empty() && !monitor.is_empty() {
        let gpu = &gpu_static[0];
        for m in monitor.iter_mut() {
            if m.screen_width.is_none() { m.screen_width = gpu.resolution_width; }
            if m.screen_height.is_none() { m.screen_height = gpu.resolution_height; }
            if m.refresh_rate.is_none() { m.refresh_rate = gpu.refresh_rate; }
        }
    }

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
        monitor,
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

    let cpu_load = get_cpu_dynamic_info();
    let gpu_dynamic = get_gpu_dynamic_info(&static_info.gpu_static);

    let mut cpu = static_info.cpu;
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
                video_processor: gs.video_processor.clone(),
                adapter_compatibility: gs.adapter_compatibility.clone(),
                driver_date: gs.driver_date.clone(),
                installed_drivers: gs.installed_drivers.clone(),
                video_mode: gs.video_mode.clone(),
                resolution_width: gs.resolution_width,
                resolution_height: gs.resolution_height,
                refresh_rate: gs.refresh_rate,
                device_id: gs.device_id.clone(),
                pnp_device_id: gs.pnp_device_id.clone(),
                status: gs.status.clone(),
                inf_filename: gs.inf_filename.clone(),
                video_architecture: gs.video_architecture.clone(),
                video_memory_type: gs.video_memory_type.clone(),
            }
        })
        .collect();

    Ok(HardwareInfo {
        cpu,
        gpu,
        motherboard: static_info.motherboard,
        memory: static_info.memory,
        disk: static_info.disk,
        monitor: static_info.monitor,
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

pub fn cleanup_hardware_cache() {
    let mut cache = STATIC_HARDWARE_CACHE.lock().unwrap();
    *cache = None;

    let mut cpu_system = CPU_SYSTEM.lock().unwrap();
    *cpu_system = None;
    
    log::info!("硬件信息缓存已清理");
}
