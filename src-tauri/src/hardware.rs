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
pub struct SoundCardInfo {
    pub name: String,
    pub manufacturer: String,
    pub status: String,
    pub device_id: String,
    pub pnp_device_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkCardInfo {
    pub name: String,
    pub manufacturer: String,
    pub adapter_type: String,
    pub mac_address: String,
    pub speed_mbps: u64,
    pub connection_name: String,
    pub service_name: String,
    pub index: u32,
    pub max_speed: Option<u64>,
    pub guid: String,
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
    pub sound_card: Vec<SoundCardInfo>,
    pub network_card: Vec<NetworkCardInfo>,
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
struct PsSoundDevice {
    Name: String,
    Manufacturer: Option<String>,
    Status: Option<String>,
    DeviceID: Option<String>,
    PNPDeviceID: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct PsNetworkAdapter {
    Name: String,
    Manufacturer: Option<String>,
    AdapterType: Option<String>,
    MACAddress: Option<String>,
    Speed: Option<u64>,
    NetConnectionID: Option<String>,
    ServiceName: Option<String>,
    Index: Option<u32>,
    MaxSpeed: Option<u64>,
    #[serde(rename = "GUID")]
    GUID: Option<String>,
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

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct PsWmiMonitorId {
    UserFriendlyName: Option<String>,
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

/// Query real monitor model names from EDID via WmiMonitorID (root\wmi namespace)
fn query_edid_monitor_names() -> Vec<String> {
    let cmd = "ConvertTo-Json -Compress @(Get-CimInstance -Namespace root\\wmi WmiMonitorID | ForEach-Object { $friendly = ''; if ($_.UserFriendlyNameLength -gt 0) { $arr = @($_.UserFriendlyName); $max = [Math]::Min($arr.Count, $_.UserFriendlyNameLength); for ($i = 0; $i -lt $max; $i++) { $c = [char]$arr[$i]; if ($c -eq [char]0) { break } $friendly += $c } }; [PSCustomObject]@{ UserFriendlyName = $friendly.Trim() } })";

    match run_powershell::<PsWmiMonitorId>(cmd) {
        Ok(results) => {
            results
                .into_iter()
                .map(|m| m.UserFriendlyName.unwrap_or_default())
                .collect()
        }
        Err(e) => {
            log::warn!("EDID 显示器名称查询失败: {}", e);
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
    sound_card: Vec<SoundCardInfo>,
    network_card: Vec<NetworkCardInfo>,
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
    // 强制所有 PowerShell 输出使用 UTF-8 编码，防止中文乱码
    let full_cmd = format!(
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $OutputEncoding = [System.Text.Encoding]::UTF8; {}",
        command
    );
    let mut cmd = Command::new("powershell");
    cmd.args(&["-Command", &full_cmd]);
    
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

fn video_architecture_name(code: Option<u16>) -> Option<String> {
    match code {
        Some(1) => Some("VGA".into()),
        Some(2) => Some("XGA".into()),
        Some(3) => Some("Other".into()),
        Some(4) => Some("S-Video".into()),
        Some(5) => Some("Composite".into()),
        Some(6) => Some("Component".into()),
        Some(7) => Some("DVI".into()),
        Some(8) => Some("HDMI".into()),
        Some(9) => Some("LVDS".into()),
        Some(10) => Some("D-Jpn".into()),
        Some(11) => Some("SDI".into()),
        Some(12) => Some("DisplayPort (External)".into()),
        Some(13) => Some("DisplayPort (Embedded)".into()),
        Some(14) => Some("UDI (External)".into()),
        Some(15) => Some("UDI (Embedded)".into()),
        Some(16) => Some("SDTV-Dongle".into()),
        Some(17) => Some("Miracast".into()),
        Some(18) => Some("Internal".into()),
        _ => None,
    }
}

fn video_memory_type_name(code: Option<u16>) -> Option<String> {
    match code {
        Some(1) => Some("Other".into()),
        Some(2) => Some("Unknown".into()),
        Some(3) => Some("VRAM".into()),
        Some(4) => Some("DRAM".into()),
        Some(5) => Some("SRAM".into()),
        Some(6) => Some("WRAM".into()),
        Some(7) => Some("EDO RAM".into()),
        Some(8) => Some("Burst Synchronous DRAM".into()),
        Some(9) => Some("Pipelined Burst SRAM".into()),
        Some(10) => Some("CDRAM".into()),
        Some(11) => Some("3DRAM".into()),
        Some(12) => Some("SDRAM".into()),
        Some(13) => Some("SGRAM".into()),
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
    let mut all_gpus: Vec<(PsVideoController, GpuVendor, bool, f64)> = Vec::new();

    let gpu_cmd = "Get-WmiObject Win32_VideoController | Select-Object Name, DriverVersion, AdapterRAM, VideoProcessor, AdapterCompatibility, DriverDate, InstalledDisplayDrivers, VideoModeDescription, CurrentHorizontalResolution, CurrentVerticalResolution, CurrentRefreshRate, DeviceID, PNPDeviceID, Status, InfFilename, VideoArchitecture, VideoMemoryType | ConvertTo-Json -Compress";
    if let Ok(gpu_results) = run_powershell::<PsVideoController>(gpu_cmd) {
        for g in gpu_results {
            let name_lower = g.Name.to_lowercase();
            let vendor = detect_gpu_vendor(&g.Name);
            let memory_gb = g.AdapterRAM
                .map(|ram| ram as f64 / (1024.0 * 1024.0 * 1024.0))
                .unwrap_or(0.0);

            // 判断是否为核显（Intel 集成显卡和 AMD APU）
            let is_integrated = match vendor {
                GpuVendor::Intel => !name_lower.contains("arc"),
                GpuVendor::AMD => {
                    name_lower.contains("radeon") && name_lower.contains("graphics")
                        && !name_lower.contains("rx ")
                }
                _ => false,
            };

            all_gpus.push((g, vendor, is_integrated, memory_gb));
        }

        // 检查是否存在独显（非核显 GPU）
        let has_dgpu = all_gpus.iter().any(|(_, _, is_igpu, _)| !is_igpu);

        let mut gpus = Vec::new();
        for (g, vendor, is_integrated, memory_gb) in all_gpus {
            // 当存在独显时跳过核显，否则保留核显（纯核显电脑）
            if is_integrated && has_dgpu {
                log::info!("跳过核显(WMI): {}, 厂商: {:?}, 显存: {:.1}GB (存在独显)",
                          g.Name, vendor, memory_gb);
                continue;
            }

            if is_integrated {
                log::info!("核显(WMI)(唯一): {}, 厂商: {:?}, 显存: {:.1}GB",
                          g.Name, vendor, memory_gb);
            } else {
                log::info!("显卡(WMI): {}, 厂商: {:?}, 显存: {:.1}GB",
                          g.Name, vendor, memory_gb);
            }

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
                video_architecture: video_architecture_name(g.VideoArchitecture),
                video_memory_type: video_memory_type_name(g.VideoMemoryType),
            });
        }
        return gpus;
    }

    Vec::new()
}

fn get_gpu_info() -> Vec<GpuInfo> {
    // 同时获取WMI扩展信息（所有GPU的详细参数）
    let wmi_gpus = get_gpus_from_wmi();

    // 尝试用NVML获取NVIDIA显卡（最好的方式）
    if let Ok(mut nvml_gpus) = get_nvidia_gpus_with_nvml() {
        if !nvml_gpus.is_empty() {
            // 合并WMI扩展字段到NVML结果
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

    // 然后尝试用nvidia-smi
    let mut smi_gpus = get_nvidia_gpus_with_smi();
    if !smi_gpus.is_empty() {
        // 合并WMI扩展字段
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

    // 最后用WMI（已包含扩展字段）
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

fn architecture_name(code: Option<u16>) -> String {
    match code {
        Some(0) => "x86".into(),
        Some(1) => "MIPS".into(),
        Some(2) => "Alpha".into(),
        Some(3) => "PowerPC".into(),
        Some(5) => "ARM".into(),
        Some(6) => "ia64".into(),
        Some(7) => "Alpha64".into(),
        Some(9) => "x64".into(),
        Some(12) => "ARM64".into(),
        _ => "未知".into(),
    }
}

fn memory_form_factor_name(code: Option<u16>) -> String {
    match code {
        Some(0) => "未知".into(),
        Some(1) => "Other".into(),
        Some(2) => "SIP".into(),
        Some(3) => "DIP".into(),
        Some(4) => "ZIP".into(),
        Some(5) => "SOJ".into(),
        Some(6) => "Proprietary".into(),
        Some(7) => "SIMM".into(),
        Some(8) => "DIMM".into(),
        Some(9) => "TSOP".into(),
        Some(10) => "PGA".into(),
        Some(11) => "RIMM".into(),
        Some(12) => "SODIMM".into(),
        Some(13) => "SRIMM".into(),
        Some(14) => "SMD".into(),
        Some(15) => "SSMP".into(),
        Some(16) => "QFP".into(),
        Some(17) => "TQFP".into(),
        Some(18) => "SOIC".into(),
        Some(19) => "LCC".into(),
        Some(20) => "PLCC".into(),
        Some(21) => "BGA".into(),
        Some(22) => "FPBGA".into(),
        Some(23) => "LGA".into(),
        Some(24) => "FB-DIMM".into(),
        _ => "未知".into(),
    }
}

fn memory_type_name(code: Option<u16>) -> String {
    match code {
        Some(0) => "未知".into(),
        Some(1) => "Other".into(),
        Some(2) => "DRAM".into(),
        Some(3) => "Synchronous DRAM".into(),
        Some(4) => "Cache DRAM".into(),
        Some(5) => "EDO".into(),
        Some(6) => "EDRAM".into(),
        Some(7) => "VRAM".into(),
        Some(8) => "SRAM".into(),
        Some(9) => "RAM".into(),
        Some(10) => "ROM".into(),
        Some(11) => "Flash".into(),
        Some(12) => "EEPROM".into(),
        Some(13) => "FEPROM".into(),
        Some(14) => "EPROM".into(),
        Some(15) => "CDRAM".into(),
        Some(16) => "3DRAM".into(),
        Some(17) => "SDRAM".into(),
        Some(18) => "SGRAM".into(),
        Some(19) => "RDRAM".into(),
        Some(20) => "DDR".into(),
        Some(21) => "DDR2".into(),
        Some(22) => "DDR2 FB-DIMM".into(),
        Some(24) => "DDR3".into(),
        Some(25) => "FBD2".into(),
        Some(26) => "DDR4".into(),
        Some(27) => "LPDDR".into(),
        Some(28) => "LPDDR2".into(),
        Some(29) => "LPDDR3".into(),
        Some(30) => "LPDDR4".into(),
        Some(31) => "Logical non-volatile".into(),
        Some(32) => "HBM".into(),
        Some(33) => "HBM2".into(),
        Some(34) => "DDR5".into(),
        Some(35) => "LPDDR5".into(),
        Some(36) => "HBM3".into(),
        _ => "未知".into(),
    }
}

fn chassis_type_name(codes: &Option<Vec<u16>>) -> String {
    match codes.as_ref().and_then(|v| v.first()).copied() {
        Some(1) => "Other".into(),
        Some(2) => "Unknown".into(),
        Some(3) => "Desktop".into(),
        Some(4) => "Low Profile Desktop".into(),
        Some(5) => "Pizza Box".into(),
        Some(6) => "Mini Tower".into(),
        Some(7) => "Tower".into(),
        Some(8) => "Portable".into(),
        Some(9) => "Laptop".into(),
        Some(10) => "Notebook".into(),
        Some(11) => "Hand Held".into(),
        Some(12) => "Docking Station".into(),
        Some(13) => "All in One".into(),
        Some(14) => "Sub Notebook".into(),
        Some(15) => "Space-Saving".into(),
        Some(16) => "Lunch Box".into(),
        Some(17) => "Main System Chassis".into(),
        Some(18) => "Expansion Chassis".into(),
        Some(19) => "Sub Chassis".into(),
        Some(20) => "Bus Expansion Chassis".into(),
        Some(21) => "Peripheral Chassis".into(),
        Some(22) => "Storage Chassis".into(),
        Some(23) => "Rack Mount Chassis".into(),
        Some(24) => "Sealed-Case PC".into(),
        Some(25) => "Multi-System Chassis".into(),
        Some(26) => "Compact PCI".into(),
        Some(27) => "Advanced TCA".into(),
        Some(28) => "Blade".into(),
        Some(29) => "Blade Enclosure".into(),
        Some(30) => "Tablet".into(),
        Some(31) => "Convertible".into(),
        Some(32) => "Detachable".into(),
        Some(33) => "IoT Gateway".into(),
        Some(34) => "Embedded PC".into(),
        Some(35) => "Mini PC".into(),
        Some(36) => "Stick PC".into(),
        _ => "未知".into(),
    }
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
        let cpu_cmd = "Get-WmiObject Win32_Processor | Select-Object Name, NumberOfCores, NumberOfLogicalProcessors, MaxClockSpeed, L2CacheSize, L2CacheSpeed, L3CacheSize, L3CacheSpeed, LoadPercentage, Manufacturer, Architecture, SocketDesignation, CurrentClockSpeed, ExtClock, ProcessorId, Family, Stepping, Revision, NumberOfEnabledCore, VoltageCaps | ConvertTo-Json -Compress";
        match run_powershell::<PsProcessor>(cpu_cmd) {
            Ok(cpu_results) => {
                log::info!("获取到{}个CPU信息", cpu_results.len());
                cpu_results.into_iter().next().map(|p| {
                    log::info!("CPU型号: {}", p.Name);
                    CpuInfo {
                        name: p.Name,
                        manufacturer: p.Manufacturer.unwrap_or_else(|| "未知".to_string()),
                        cores: p.NumberOfCores,
                        threads: p.NumberOfLogicalProcessors,
                        max_clock_speed: p.MaxClockSpeed,
                        l2_cache_size: p.L2CacheSize.unwrap_or(0),
                        l3_cache_size: p.L3CacheSize.unwrap_or(0),
                        load_percentage: p.LoadPercentage,
                        architecture: architecture_name(p.Architecture),
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
        // 同时查询主板、系统信息、BIOS、机箱
        let mobo_cmd = "Get-WmiObject Win32_BaseBoard | Select-Object Manufacturer, Product, SerialNumber, Version | ConvertTo-Json -Compress";
        let sys_cmd = "Get-WmiObject Win32_ComputerSystem | Select-Object Manufacturer, Model, SystemType | ConvertTo-Json -Compress";
        let bios_cmd = "Get-WmiObject Win32_BIOS | Select-Object SMBIOSBIOSVersion, Manufacturer, Name, ReleaseDate, SerialNumber | ConvertTo-Json -Compress";
        let chassis_cmd = "Get-WmiObject Win32_SystemEnclosure | Select-Object ChassisTypes, Manufacturer, Version, SerialNumber | ConvertTo-Json -Compress";

        let mobo_result = run_powershell::<PsBaseBoard>(mobo_cmd).ok().and_then(|r| r.into_iter().next());
        let sys_result = run_powershell::<PsComputerSystem>(sys_cmd).ok().and_then(|r| r.into_iter().next());
        let bios_result = run_powershell::<PsBios>(bios_cmd).ok().and_then(|r| r.into_iter().next());
        let chassis_result = run_powershell::<PsSystemEnclosure>(chassis_cmd).ok().and_then(|r| r.into_iter().next());

        if let Some(ref m) = mobo_result {
            log::info!("主板: {} {}", m.Manufacturer.as_deref().unwrap_or(""), m.Product.as_deref().unwrap_or(""));
        }
        if let Some(ref b) = bios_result {
            log::info!("BIOS: {}", b.SMBIOSBIOSVersion.as_deref().unwrap_or(""));
        }

        if let Ok(mut errs) = errors_mobo.lock() {
            if mobo_result.is_none() { errs.push("主板: WMI查询失败".to_string()); }
        }

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
            chassis_type: chassis_type_name(&chassis_result.as_ref().and_then(|c| c.ChassisTypes.clone())),
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
                        form_factor: memory_form_factor_name(mem.FormFactor),
                        memory_type: memory_type_name(mem.MemoryType),
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
        let disk_cmd = "Get-WmiObject Win32_DiskDrive | Select-Object Model, Size, InterfaceType, SerialNumber, FirmwareRevision, MediaType, BytesPerSector, Partitions, Status, PNPDeviceID | ConvertTo-Json -Compress";
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

    let errors_sound = errors.clone();
    let sound_handle = thread::spawn(move || {
        let sound_cmd = r#"Get-WmiObject Win32_SoundDevice | Where-Object { $_.Status -eq 'OK' -and $_.PNPDeviceID -notlike 'USB\*' -and $_.PNPDeviceID -notlike 'HID\*' -and $_.PNPDeviceID -notlike 'SWD\*' -and $_.Name -notlike '*Virtual*' -and $_.Name -notlike '*VB-Audio*' -and $_.Name -notlike '*Voicemeeter*' -and $_.Name -notlike '*CABLE*' -and $_.Name -notlike '*Sonic*Studio*' -and $_.Name -notlike '*NVIDIA*Virtual*' -and $_.Name -notlike '*Steam*Streaming*' -and $_.Name -notlike '*Oculus*' -and $_.Name -notlike '*Wave*Link*' -and $_.Name -notlike '*Elgato*Sound*Capture*' -and $_.Name -notlike '*Nahimic*' -and $_.Name -notlike '*DTS*' -and $_.Name -notlike '*Dolby*' -and $_.Name -notlike '*Bluetooth*' -and $_.Name -notlike '*Hands-Free*' -and $_.Name -notlike '*S/PDIF*' } | Select-Object Name, Manufacturer, Status, DeviceID, PNPDeviceID | ConvertTo-Json -Compress"#;
        match run_powershell::<PsSoundDevice>(sound_cmd) {
            Ok(results) => {
                log::info!("获取到{}个声卡信息", results.len());
                results.into_iter().map(|s| {
                    SoundCardInfo {
                        name: s.Name,
                        manufacturer: s.Manufacturer.unwrap_or_else(|| "未知".to_string()),
                        status: s.Status.unwrap_or_else(|| "未知".to_string()),
                        device_id: s.DeviceID.unwrap_or_else(|| "未知".to_string()),
                        pnp_device_id: s.PNPDeviceID.unwrap_or_else(|| "未知".to_string()),
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
        let network_cmd = r#"Get-WmiObject Win32_NetworkAdapter | Where-Object { $_.PhysicalAdapter -eq $true -and $_.NetEnabled -eq $true -and $_.PNPDeviceID -notlike 'SWD\*' -and $_.Name -notlike '*Hyper-V*' -and $_.Name -notlike '*vEthernet*' -and $_.Name -notlike '*Virtual*' -and $_.Name -notlike '*VirtualBox*' -and $_.Name -notlike '*VMware*' -and $_.Name -notlike '*Bluetooth*' -and $_.Name -notlike '*Tailscale*' -and $_.Name -notlike '*ZeroTier*' -and $_.Name -notlike '*WSL*' -and $_.Name -notlike '*Docker*' -and $_.Name -notlike '*Npcap*' -and $_.Name -notlike '*WireGuard*' -and $_.Name -notlike '*OpenVPN*' -and $_.Name -notlike '*TAP-Windows*' -and $_.Name -notlike '*WAN Miniport*' -and $_.Name -notlike '*VPN*' -and $_.Name -notlike '*Proton*' -and $_.Name -notlike '*Nord*' -and $_.Name -notlike '*Cloudflare*WARP*' -and $_.AdapterType -notlike '*Loopback*' } | Select-Object Name, Manufacturer, AdapterType, MACAddress, Speed, NetConnectionID, ServiceName, Index, MaxSpeed, GUID | ConvertTo-Json -Compress"#;
        match run_powershell::<PsNetworkAdapter>(network_cmd) {
            Ok(results) => {
                log::info!("获取到{}个网卡信息", results.len());
                results.into_iter().map(|n| {
                    // Speed is in bits per second, convert to Mbps
                    let speed_mbps = n.Speed.map(|s| s / 1_000_000).unwrap_or(0);
                    let max_speed = n.MaxSpeed.map(|s| s / 1_000_000);
                    NetworkCardInfo {
                        name: n.Name,
                        manufacturer: n.Manufacturer.unwrap_or_else(|| "未知".to_string()),
                        adapter_type: n.AdapterType.unwrap_or_else(|| "未知".to_string()),
                        mac_address: n.MACAddress.unwrap_or_else(|| "未知".to_string()),
                        speed_mbps,
                        connection_name: n.NetConnectionID.unwrap_or_else(|| "未知".to_string()),
                        service_name: n.ServiceName.unwrap_or_else(|| "未知".to_string()),
                        index: n.Index.unwrap_or(0),
                        max_speed,
                        guid: n.GUID.unwrap_or_else(|| "未知".to_string()),
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
                }).collect::<Vec<MonitorInfo>>();

                // EDID 回退：如果显示器名称是通用的（如"通用即插即用显示器"），
                // 尝试从 EDID (WmiMonitorID) 获取真实型号
                let has_generic = monitors.iter().any(|m| is_generic_monitor_name(&m.name));
                if has_generic {
                    log::info!("检测到通用显示器名称，尝试从 EDID 获取真实型号...");
                    let edid_names = query_edid_monitor_names();
                    if !edid_names.is_empty() {
                        for (i, m) in monitors.iter_mut().enumerate() {
                            if is_generic_monitor_name(&m.name) {
                                if let Some(edid_name) = edid_names.get(i) {
                                    if !edid_name.is_empty() {
                                        log::info!("显示器[{}]: EDID 替换 '{}' -> '{}'", i, m.name, edid_name);
                                        m.name = edid_name.clone();
                                    }
                                } else if edid_names.len() == 1 && !edid_names[0].is_empty() {
                                    log::info!("显示器[{}]: EDID 替换(单结果) '{}' -> '{}'", i, m.name, edid_names[0]);
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
    let sound_card = sound_handle.join().unwrap_or_else(|_| Vec::new());
    let network_card = network_handle.join().unwrap_or_else(|_| Vec::new());
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
        sound_card,
        network_card,
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
        sound_card: static_info.sound_card,
        network_card: static_info.network_card,
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

        // NVML/nvidia-smi 失败时（如纯核显系统），回退到 LHML 传感器
        if let Ok(response) = crate::sensor::read_lhm_sensors() {
            let gpu_hardware_types: Vec<_> = response.sensors.iter()
                .filter(|s| s.hardware_type.to_lowercase().starts_with("gpu"))
                .map(|s| s.hardware_type.clone())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();

            let has_dgpu = gpu_hardware_types.iter().any(|t| {
                t.eq_ignore_ascii_case("GpuNvidia") || t.eq_ignore_ascii_case("GpuAmd")
            });

            for hw_type in &gpu_hardware_types {
                if has_dgpu && hw_type.eq_ignore_ascii_case("GpuIntel") {
                    continue;
                }
                let temp = response.sensors.iter()
                    .filter(|s| s.hardware_type == *hw_type
                        && s.sensor_type == "Temperature"
                        && s.name == "GPU Core")
                    .map(|s| s.value)
                    .next();
                let usage = response.sensors.iter()
                    .filter(|s| s.hardware_type == *hw_type
                        && s.sensor_type == "Load"
                        && s.name == "GPU Core")
                    .map(|s| s.value as u32)
                    .next();
                return GpuStatus { temperature: temp, usage };
            }
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
