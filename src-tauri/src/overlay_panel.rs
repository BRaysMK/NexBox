use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::{Emitter, Manager};

static OVERLAY_ACTIVE: AtomicBool = AtomicBool::new(false);
static OVERLAY_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
static DRAG_MODE: AtomicBool = AtomicBool::new(false);
static POSITION_CHANGED: AtomicBool = AtomicBool::new(false);

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct DisplayItem {
    pub id: String,
    pub label: String,
    pub enabled: bool,
}

pub type DisplayItems = Vec<DisplayItem>;

fn default_style() -> String {
    "default".to_string()
}

fn default_font() -> String {
    "MiSans Medium".to_string()
}

fn default_display_items() -> DisplayItems {
        vec![
            DisplayItem { id: "fps".to_string(), label: "FPS".to_string(), enabled: false },
            DisplayItem { id: "cpu_temp".to_string(), label: "CPU温度".to_string(), enabled: false },
            DisplayItem { id: "cpu_usage".to_string(), label: "CPU占用".to_string(), enabled: true },
            DisplayItem { id: "cpu_clock".to_string(), label: "CPU频率".to_string(), enabled: false },
            DisplayItem { id: "cpu_voltage".to_string(), label: "CPU电压".to_string(), enabled: false },
            DisplayItem { id: "cpu_power".to_string(), label: "CPU功耗".to_string(), enabled: false },
            DisplayItem { id: "gpu_temp".to_string(), label: "GPU温度".to_string(), enabled: true },
            DisplayItem { id: "gpu_usage".to_string(), label: "GPU占用".to_string(), enabled: true },
            DisplayItem { id: "gpu_fan_speed".to_string(), label: "GPU风扇转速".to_string(), enabled: false },
            DisplayItem { id: "gpu_power".to_string(), label: "GPU功耗".to_string(), enabled: false },
            DisplayItem { id: "gpu_clock".to_string(), label: "GPU频率".to_string(), enabled: false },
            DisplayItem { id: "gpu_voltage".to_string(), label: "GPU电压".to_string(), enabled: false },
            DisplayItem { id: "gpu_vram".to_string(), label: "GPU显存占用".to_string(), enabled: false },
            DisplayItem { id: "gpu_memory_clock".to_string(), label: "GPU显存频率".to_string(), enabled: false },
            DisplayItem { id: "memory_usage".to_string(), label: "内存占用".to_string(), enabled: true },
            DisplayItem { id: "ssd_temp".to_string(), label: "硬盘温度".to_string(), enabled: false },
            DisplayItem { id: "game_ping".to_string(), label: "游戏延迟".to_string(), enabled: true },
            DisplayItem { id: "delta_password".to_string(), label: "三角洲密码".to_string(), enabled: true },
            DisplayItem { id: "netease_lyric".to_string(), label: "网易云歌词".to_string(), enabled: false },
        ]
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CustomOverlayItem {
    pub id: String,
    pub text: String,
    pub color: String,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct OverlaySettings {
    #[serde(default = "default_display_items")]
    pub display_items: DisplayItems,
    #[serde(default)]
    pub custom_items: Vec<CustomOverlayItem>,
    pub opacity: u8,
    #[serde(default = "default_style")]
    pub style: String,
    #[serde(default = "default_font")]
    pub font: String,
    #[serde(default)]
    pub position_x: Option<i32>,
    #[serde(default)]
    pub position_y: Option<i32>,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            display_items: default_display_items(),
            custom_items: Vec::new(),
            opacity: 255,
            style: "default".to_string(),
            font: "MiSans Medium".to_string(),
            position_x: None,
            position_y: None,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct OverlayResult {
    pub success: bool,
    pub message: String,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct OverlayHardwareData {
    fps: Option<u32>,
    cpu_usage: Option<u16>,
    cpu_temp: Option<f64>,
    cpu_clock: Option<u32>,
    gpu_temp: Option<f64>,
    gpu_usage: Option<u32>,
    memory_usage: Option<f64>,
    delta_password: Option<String>,
    game_ping: Option<u32>,
    heart_rate: Option<u16>,
    heart_rate_device: Option<String>,
    gpu_fan_speed: Option<u32>,
    gpu_power: Option<u32>,
    gpu_clock: Option<u32>,
    gpu_vram_used: Option<u32>,
    gpu_vram_total: Option<u32>,
    gpu_memory_clock: Option<u32>,
    cpu_voltage: Option<f64>,
    gpu_voltage: Option<f64>,
    cpu_power: Option<f64>,
    ssd_temp: Option<f64>,
    netease_current_lyric: Option<String>,
    netease_song_title: Option<String>,
    netease_song_artist: Option<String>,
}

impl Default for OverlayHardwareData {
    fn default() -> Self {
        Self {
            fps: None,
            cpu_usage: None,
            cpu_temp: None,
            cpu_clock: None,
            gpu_temp: None,
            gpu_usage: None,
            memory_usage: None,
            delta_password: None,
            game_ping: None,
            heart_rate: None,
            heart_rate_device: None,
            gpu_fan_speed: None,
            gpu_power: None,
            gpu_clock: None,
            gpu_vram_used: None,
            gpu_vram_total: None,
            gpu_memory_clock: None,
            cpu_voltage: None,
            gpu_voltage: None,
            cpu_power: None,
            ssd_temp: None,
            netease_current_lyric: None,
            netease_song_title: None,
            netease_song_artist: None,
        }
    }
}

static CURRENT_SETTINGS: Mutex<Option<OverlaySettings>> = Mutex::new(None);
static CURRENT_HARDWARE_DATA: Mutex<Option<OverlayHardwareData>> = Mutex::new(None);
static MISANS_FONT_PATH: Mutex<Option<String>> = Mutex::new(None);

fn get_or_init_settings() -> OverlaySettings {
    let mut settings_lock = CURRENT_SETTINGS.lock().unwrap();
    if settings_lock.is_none() {
        *settings_lock = Some(OverlaySettings::default());
    }
    settings_lock.as_ref().unwrap().clone()
}

/// 从 LHML 传感器列表中提取指定类型的传感器值（多模式匹配）
fn extract_sensor(
    sensors: &[crate::sensor::SensorReading],
    sensor_type: &str,
    hardware_type: &str,
    names: &[&str],
    skip_zero: bool,
) -> Option<(f64, String)> {
    for name in names {
        if let Some(s) = sensors.iter().find(|s| {
            s.sensor_type == sensor_type
                && s.hardware_type.eq_ignore_ascii_case(hardware_type)
                && s.name.contains(name)
                && (!skip_zero || s.value > 0.0)
        }) {
            return Some((s.value, s.name.clone()));
        }
    }
    None
}

/// 从 LHML 传感器列表中提取所有匹配传感器值的平均值（用于风扇转速等）
fn extract_all_avg(
    sensors: &[crate::sensor::SensorReading],
    sensor_type: &str,
    hardware_type: &str,
    name_prefixes: &[&str],
) -> Option<f64> {
    let values: Vec<f64> = sensors
        .iter()
        .filter(|s| {
            s.sensor_type == sensor_type
                && s.hardware_type.eq_ignore_ascii_case(hardware_type)
                && name_prefixes.iter().any(|p| s.name.starts_with(p))
        })
        .map(|s| s.value)
        .collect();
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_overlay_text(value: &str, max_chars: usize) -> String {
    let trimmed = collapse_whitespace(value);
    let char_count = trimmed.chars().count();
    if char_count <= max_chars {
        return trimmed;
    }

    let truncated: String = trimmed.chars().take(max_chars).collect();
    format!("{truncated}...")
}

fn build_netease_song_text(title: Option<&str>, artist: Option<&str>) -> Option<String> {
    let title = title
        .map(collapse_whitespace)
        .filter(|value| !value.is_empty());
    let artist = artist
        .map(collapse_whitespace)
        .filter(|value| !value.is_empty());

    match (title, artist) {
        (Some(title), Some(artist)) => Some(format!("{title} - {artist}")),
        (Some(title), None) => Some(title),
        (None, Some(artist)) => Some(artist),
        (None, None) => None,
    }
}

static LAST_LHML_UPDATE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn collect_hardware_data() -> OverlayHardwareData {
    let fps = crate::game_fps::get_cached_fps();

    let delta_password = crate::delta_force::get_cached_delta_password();

    let game_ping = crate::game_ping::get_cached_ping();

    let lyrics_enabled = get_or_init_settings()
        .display_items
        .iter()
        .any(|item| item.id == "netease_lyric" && item.enabled);

    let heart_rate = crate::heart_rate::get_cached_heart_rate();
    let heart_rate_device = crate::heart_rate::get_heart_rate_device_name();

    let current_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
    let last_time = LAST_LHML_UPDATE.load(std::sync::atomic::Ordering::Relaxed);
    
    let use_cached_lhml = if current_time - last_time < 1000 {
        true
    } else {
        LAST_LHML_UPDATE.store(current_time, std::sync::atomic::Ordering::Relaxed);
        false
    };

    // 从 LHML (NexBoxMonitor) 获取硬件传感器数据
    let (cpu_usage, cpu_temp, cpu_clock, cpu_voltage, cpu_power, ssd_temp, memory_usage, gpu_temp, gpu_usage, gpu_fan_speed, gpu_power, gpu_clock, gpu_vram_used, gpu_vram_total, gpu_memory_clock, gpu_voltage) =
        if use_cached_lhml {
            let prev = CURRENT_HARDWARE_DATA.lock().unwrap().clone().unwrap_or_default();
            (
                prev.cpu_usage, prev.cpu_temp, prev.cpu_clock, prev.cpu_voltage, prev.cpu_power, prev.ssd_temp, prev.memory_usage, prev.gpu_temp, prev.gpu_usage, prev.gpu_fan_speed, prev.gpu_power, prev.gpu_clock, prev.gpu_vram_used, prev.gpu_vram_total, prev.gpu_memory_clock, prev.gpu_voltage
            )
        } else {
        match crate::sensor::read_lhm_sensors() {
            Ok(response) => {
                // CPU 占用 (Load 类型)
                let (cpu_usage, cpu_usage_name) = extract_sensor(
                    &response.sensors,
                    "Load",
                    "CPU",
                    &["CPU Total", "Total"],
                    false,
                ).unzip();
                let cpu_usage = cpu_usage.map(|v| v as u16);
                // CPU 温度 (AMD: Core (Tctl/Tdie), Intel: CPU Package)
                let (cpu_temp, cpu_name) = extract_sensor(
                    &response.sensors,
                    "Temperature",
                    "CPU",
                    &["Core (Tctl/Tdie)", "CPU Package", "Tctl", "Core"],
                    false,
                ).unzip();
                // CPU 频率
                let (cpu_clock, cpu_clock_name) = extract_sensor(
                    &response.sensors,
                    "Clock",
                    "CPU",
                    &["Cores (Average)", "Core #1", "Bus Speed"],
                    true,
                ).unzip();
                let cpu_clock = cpu_clock.map(|v| v as u32);
                // CPU 电压
                let cpu_voltage_result = extract_sensor(
                    &response.sensors,
                    "Voltage",
                    "CPU",
                    &["CPU Core", "Vcore", "Core", "CPU VCore"],
                    false,
                );
                let cpu_voltage = cpu_voltage_result.as_ref().map(|(v, _)| *v);
                // CPU 功耗
                let cpu_power_result = extract_sensor(
                    &response.sensors,
                    "Power",
                    "CPU",
                    &["Package", "CPU Package", "CPU Cores", "CPU Core"],
                    false,
                );
                let cpu_power = cpu_power_result.as_ref().map(|(v, _)| *v);
                // SSD 温度 (跳过 0 值)
                let (ssd_temp, ssd_name) = extract_sensor(
                    &response.sensors,
                    "Temperature",
                    "Storage",
                    &["Composite Temperature", "Temperature #1", "Temperature"],
                    true,
                ).unzip();

                // 内存占用 (从 LHML Memory 硬件获取)
                let memory_usage = extract_sensor(
                    &response.sensors,
                    "Load",
                    "Memory",
                    &["Memory"],
                    false,
                ).map(|(v, _)| v);

                // 调试：打印所有 RAM 传感器
                let memory_sensors: Vec<_> = response.sensors.iter()
                    .filter(|s| s.hardware_type.eq_ignore_ascii_case("Memory"))
                    .map(|s| format!("{}|{}|{}={}", s.hardware_type, s.sensor_type, s.name, s.value))
                    .collect();
                log::info!("LHML Memory sensors: {:?}", memory_sensors);

                // 调试：打印所有 CPU 和主板传感器
                let cpu_sensors: Vec<_> = response.sensors.iter()
                    .filter(|s| s.hardware_type.eq_ignore_ascii_case("CPU"))
                    .map(|s| format!("{}|{}|{}={}", s.hardware_type, s.sensor_type, s.name, s.value))
                    .collect();
                log::info!("LHML CPU sensors: {:?}", cpu_sensors);
                // 打印所有非 GPU/CPU/HDD 的硬件类型（用于排查主板传感器）
                let other_types: std::collections::BTreeSet<_> = response.sensors.iter()
                    .filter(|s| !s.hardware_type.to_lowercase().starts_with("gpu")
                        && !s.hardware_type.eq_ignore_ascii_case("CPU")
                        && !s.hardware_type.eq_ignore_ascii_case("Storage")
                        && !s.hardware_type.eq_ignore_ascii_case("RAM"))
                    .map(|s| format!("{}|{}|{}={}", s.hardware_type, s.sensor_type, s.name, s.value))
                    .collect();
                log::info!("LHML other sensors: {:?}", other_types);

                // 调试：打印所有 GPU 传感器
                let gpu_sensors: Vec<_> = response.sensors.iter()
                    .filter(|s| s.hardware_type.to_lowercase().starts_with("gpu"))
                    .map(|s| format!("{}|{}|{}={}", s.hardware_type, s.sensor_type, s.name, s.value))
                    .collect();
                log::info!("LHML GPU sensors: {:?}", gpu_sensors);

                // GPU 温度
                let gpu_temp_result = extract_sensor(
                    &response.sensors,
                    "Temperature",
                    "GpuNvidia",
                    &["GPU Core"],
                    false,
                )
                    .or_else(|| extract_sensor(&response.sensors, "Temperature", "GpuAmd", &["GPU Core"], false))
                    .or_else(|| extract_sensor(&response.sensors, "Temperature", "GpuIntel", &["GPU Core"], false));
                let gpu_temp = gpu_temp_result.as_ref().map(|(v, _)| *v);
                let gpu_temp_name = gpu_temp_result.as_ref().map(|(_, n)| n.clone());

                // GPU 占用
                let gpu_usage_result = extract_sensor(
                    &response.sensors,
                    "Load",
                    "GpuNvidia",
                    &["GPU Core"],
                    false,
                )
                    .or_else(|| extract_sensor(&response.sensors, "Load", "GpuAmd", &["GPU Core"], false))
                    .or_else(|| extract_sensor(&response.sensors, "Load", "GpuIntel", &["GPU Core"], false));
                let gpu_usage = gpu_usage_result.as_ref().map(|(v, _)| *v as u32);
                let gpu_usage_name = gpu_usage_result.as_ref().map(|(_, n)| n.clone());

                // GPU 风扇（所有风扇平均，RPM）
                // 尝试多种前缀：NVIDIA 常见 "GPU Fan #0", "GPU"，AMD 常见 "Fans"
                let gpu_fan_speed = extract_all_avg(&response.sensors, "Fan", "GpuNvidia", &["GPU Fan", "GPU"])
                    .or_else(|| extract_all_avg(&response.sensors, "Fan", "GpuAmd", &["GPU Fan", "GPU", "Fans"]))
                    .map(|v| v as u32);

                // GPU 功耗
                let gpu_power_result = extract_sensor(
                    &response.sensors,
                    "Power",
                    "GpuNvidia",
                    &["GPU Power", "GPU Package", "GPU Chip Power", "Power"],
                    false,
                )
                    .or_else(|| {
                        extract_sensor(
                            &response.sensors,
                            "Power",
                            "GpuAmd",
                            &["GPU Power", "GPU Package", "Power"],
                            false,
                        )
                    });
                let gpu_power = gpu_power_result.as_ref().map(|(v, _)| *v as u32);
                let gpu_power_name = gpu_power_result.as_ref().map(|(_, n)| n.clone());

                // GPU 频率
                let gpu_clock_result = extract_sensor(
                    &response.sensors,
                    "Clock",
                    "GpuNvidia",
                    &["GPU Core"],
                    false,
                )
                    .or_else(|| extract_sensor(&response.sensors, "Clock", "GpuAmd", &["GPU Core"], false))
                    .or_else(|| extract_sensor(&response.sensors, "Clock", "GpuIntel", &["GPU Core"], false));
                let gpu_clock = gpu_clock_result.as_ref().map(|(v, _)| *v as u32);
                let gpu_clock_name = gpu_clock_result.as_ref().map(|(_, n)| n.clone());

                // 显存占用 (MB)
                let gpu_vram_used_result = extract_sensor(
                    &response.sensors,
                    "SmallData",
                    "GpuNvidia",
                    &["GPU Memory Used", "D3D Shared Memory Used"],
                    false,
                )
                    .or_else(|| extract_sensor(&response.sensors, "SmallData", "GpuAmd", &["GPU Memory Used"], false))
                    .or_else(|| extract_sensor(&response.sensors, "SmallData", "GpuIntel", &["GPU Memory Used"], false));
                let gpu_vram_used = gpu_vram_used_result.as_ref().map(|(v, _)| *v as u32);

                // 显存总量 (MB)
                let gpu_vram_total_result = extract_sensor(
                    &response.sensors,
                    "SmallData",
                    "GpuNvidia",
                    &["GPU Memory Total"],
                    false,
                )
                    .or_else(|| extract_sensor(&response.sensors, "SmallData", "GpuAmd", &["GPU Memory Total"], false))
                    .or_else(|| extract_sensor(&response.sensors, "SmallData", "GpuIntel", &["GPU Memory Total"], false));
                let gpu_vram_total = gpu_vram_total_result.as_ref().map(|(v, _)| *v as u32);

                // 显存频率
                let gpu_memory_clock_result = extract_sensor(
                    &response.sensors,
                    "Clock",
                    "GpuNvidia",
                    &["GPU Memory"],
                    false,
                )
                    .or_else(|| extract_sensor(&response.sensors, "Clock", "GpuAmd", &["GPU Memory"], false))
                    .or_else(|| extract_sensor(&response.sensors, "Clock", "GpuIntel", &["GPU Memory"], false));
                let gpu_memory_clock = gpu_memory_clock_result.as_ref().map(|(v, _)| *v as u32);

                // GPU 电压
                let gpu_voltage_result = extract_sensor(
                    &response.sensors, "Voltage", "GpuNvidia",
                    &["GPU Core Voltage", "GPU Core"],
                    false,
                )
                    .or_else(|| extract_sensor(&response.sensors, "Voltage", "GpuAmd", &["GPU Core Voltage", "GPU Core"], false))
                    .or_else(|| extract_sensor(&response.sensors, "Voltage", "GpuIntel", &["GPU Core Voltage", "GPU Core"], false));
                let gpu_voltage = gpu_voltage_result.as_ref().map(|(v, _)| *v);

                log::info!(
                    "LHML: CPU占用={:?}({}) CPU温度={:?}({}) CPU频率={:?}({}) CPU电压={:?}V CPU功耗={:?}W SSD温度={:?}({}) 内存占用={:?}% GPU温度={:?}({}) GPU占用={:?}({}) GPU风扇={:?}RPM GPU功耗={:?}({}) GPU频率={:?}({}) 显存={:?}/{:?}MB 显存频率={:?}MHz GPU电压={:?}V",
                    cpu_usage,
                    cpu_usage_name.as_deref().unwrap_or("N/A"),
                    cpu_temp,
                    cpu_name.as_deref().unwrap_or("N/A"),
                    cpu_clock,
                    cpu_clock_name.as_deref().unwrap_or("N/A"),
                    cpu_voltage,
                    cpu_power,
                    ssd_temp,
                    ssd_name.as_deref().unwrap_or("N/A"),
                    memory_usage,
                    gpu_temp,
                    gpu_temp_name.as_deref().unwrap_or("N/A"),
                    gpu_usage,
                    gpu_usage_name.as_deref().unwrap_or("N/A"),
                    gpu_fan_speed,
                    gpu_power,
                    gpu_power_name.as_deref().unwrap_or("N/A"),
                    gpu_clock,
                    gpu_clock_name.as_deref().unwrap_or("N/A"),
                    gpu_vram_used,
                    gpu_vram_total,
                    gpu_memory_clock,
                    gpu_voltage,
                );

                (cpu_usage, cpu_temp, cpu_clock, cpu_voltage, cpu_power, ssd_temp, memory_usage, gpu_temp, gpu_usage, gpu_fan_speed, gpu_power, gpu_clock, gpu_vram_used, gpu_vram_total, gpu_memory_clock, gpu_voltage)
            }
            Err(e) => {
                log::warn!("LHML 传感器读取失败: {e}");
                (None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None)
            }
        }
    };

    let netease_snapshot = if lyrics_enabled {
        crate::netease_lyrics::collect_snapshot()
    } else {
        crate::netease_lyrics::Snapshot::default()
    };

    let new_data = OverlayHardwareData {
        fps,
        cpu_usage,
        cpu_temp,
        cpu_clock,
        gpu_temp,
        gpu_usage,
        memory_usage,
        delta_password,
        game_ping,
        heart_rate,
        heart_rate_device,
        gpu_fan_speed,
        gpu_power,
        gpu_clock,
        gpu_vram_used,
        gpu_vram_total,
        gpu_memory_clock,
        cpu_voltage,
        gpu_voltage,
        cpu_power,
        ssd_temp,
        netease_current_lyric: netease_snapshot.current_lyric,
        netease_song_title: netease_snapshot.song_title,
        netease_song_artist: netease_snapshot.song_artist,
    };

    let prev_data = CURRENT_HARDWARE_DATA.lock().unwrap().clone();
    let result = if let Some(prev) = prev_data {
        OverlayHardwareData {
            fps: new_data.fps.or(prev.fps),
            cpu_usage: new_data.cpu_usage.or(prev.cpu_usage),
            cpu_temp: new_data.cpu_temp.or(prev.cpu_temp),
            cpu_clock: new_data.cpu_clock.or(prev.cpu_clock),
            gpu_temp: new_data.gpu_temp.or(prev.gpu_temp),
            gpu_usage: new_data.gpu_usage.or(prev.gpu_usage),
            memory_usage: new_data.memory_usage.or(prev.memory_usage),
            delta_password: new_data.delta_password.or_else(|| prev.delta_password.clone()),
            game_ping: new_data.game_ping.or(prev.game_ping),
            heart_rate: new_data.heart_rate.or(prev.heart_rate),
            heart_rate_device: new_data
                .heart_rate_device
                .or_else(|| prev.heart_rate_device.clone()),
            gpu_fan_speed: new_data.gpu_fan_speed.or(prev.gpu_fan_speed),
            gpu_power: new_data.gpu_power.or(prev.gpu_power),
            gpu_clock: new_data.gpu_clock.or(prev.gpu_clock),
            gpu_vram_used: new_data.gpu_vram_used.or(prev.gpu_vram_used),
            gpu_vram_total: new_data.gpu_vram_total.or(prev.gpu_vram_total),
            gpu_memory_clock: new_data.gpu_memory_clock.or(prev.gpu_memory_clock),
            cpu_voltage: new_data.cpu_voltage.or(prev.cpu_voltage),
            gpu_voltage: new_data.gpu_voltage.or(prev.gpu_voltage),
            cpu_power: new_data.cpu_power.or(prev.cpu_power),
            ssd_temp: new_data.ssd_temp.or(prev.ssd_temp),
            netease_current_lyric: new_data.netease_current_lyric,
            netease_song_title: new_data.netease_song_title,
            netease_song_artist: new_data.netease_song_artist,
        }
    } else {
        new_data
    };

    result
}

#[cfg(target_os = "windows")]
mod win32 {
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::Graphics::GdiPlus::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    use windows_sys::Win32::UI::Accessibility::*;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use std::ptr;
    use std::sync::Mutex;
    use std::result::Result::Ok;

    static GDIPLUS_TOKEN: Mutex<Option<usize>> = Mutex::new(None);
    static WIN_EVENT_HOOK: Mutex<Option<usize>> = Mutex::new(None);

    unsafe extern "system" fn win_event_proc(
        _h_win_event_hook: *mut std::ffi::c_void,
        _event: u32,
        hwnd: HWND,
        id_object: i32,
        _id_child: i32,
        _dw_event_thread: u32,
        _dwms_event_time: u32,
    ) {
        if id_object != 0 || hwnd.is_null() {
            return;
        }
        let overlay_hwnd = super::OVERLAY_HANDLE.load(std::sync::atomic::Ordering::SeqCst);
        if overlay_hwnd.is_null() {
            return;
        }
        if hwnd != overlay_hwnd {
            SetWindowPos(
                overlay_hwnd,
                HWND_TOPMOST,
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }

    pub unsafe fn install_topmost_guard() {
        let hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            std::ptr::null_mut(),
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        );
        if !hook.is_null() {
            let mut lock = WIN_EVENT_HOOK.lock().unwrap();
            *lock = Some(hook as usize);
        }
    }

    pub unsafe fn uninstall_topmost_guard() {
        let mut lock = WIN_EVENT_HOOK.lock().unwrap();
        if let Some(hook) = lock.take() {
            UnhookWinEvent(hook as *mut std::ffi::c_void);
        }
    }

    pub unsafe fn init_gdiplus() -> bool {
        let mut token = GDIPLUS_TOKEN.lock().unwrap();
        if token.is_some() {
            return true;
        }

        let mut input = GdiplusStartupInput {
            GdiplusVersion: 1,
            DebugEventCallback: 0,
            SuppressBackgroundThread: 0,
            SuppressExternalCodecs: 0,
        };

        let mut token_value: usize = 0;
        let result = GdiplusStartup(&mut token_value, &mut input, ptr::null_mut());

        if result == 0 {
            *token = Some(token_value);
            true
        } else {
            log::error!("GDI+ 初始化失败: {}", result);
            false
        }
    }

    pub unsafe fn shutdown_gdiplus() {
        let mut token = GDIPLUS_TOKEN.lock().unwrap();
        if let Some(t) = token.take() {
            GdiplusShutdown(t);
        }
    }

    fn calculate_window_width(settings: &super::OverlaySettings) -> i32 {
        // 使用较小的单项宽度以缩小悬浮框
        let normal_item_width = 130;
        // 默认密码项宽度（逻辑像素）
        let mut password_item_width = 220;
        let mut lyric_item_width = 280;

        // 如果已启用密码显示，尝试基于当前缓存的密码测量实际宽度
        if settings.display_items.iter().any(|item| item.id == "delta_password" && item.enabled) {
            if let Ok(lock) = super::CURRENT_HARDWARE_DATA.lock() {
                if let Some(ref data) = *lock {
                    if let Some(ref pwd) = data.delta_password {
                        unsafe {
                            // 使用屏幕 DC 和字体测量文本宽度，按 DPI 缩放
                            let screen_dc = GetDC(ptr::null_mut());
                            if !screen_dc.is_null() {
                                let dpi_x = GetDeviceCaps(screen_dc, 88);
                                let dpi_scale = dpi_x as f32 / 96.0;
                                let hfont = create_compatible_font(dpi_scale, &settings.font);
                                if !hfont.is_null() {
                                    let val_w = measure_text_width(screen_dc, hfont, pwd);
                                    let est = val_w + (12.0 * dpi_scale) as i32 + 20;
                                                if est > password_item_width {
                                                    password_item_width = est;
                                                }
                                    DeleteObject(hfont as _);
                                }
                                ReleaseDC(ptr::null_mut(), screen_dc);
                            }
                        }
                    }
                }
            }
        }

        if settings.display_items.iter().any(|item| item.id == "netease_lyric" && item.enabled) {
            if let Ok(lock) = super::CURRENT_HARDWARE_DATA.lock() {
                if let Some(ref data) = *lock {
                    let lyric_text = data
                        .netease_current_lyric
                        .clone()
                        .or_else(|| super::build_netease_song_text(
                            data.netease_song_title.as_deref(),
                            data.netease_song_artist.as_deref(),
                        ))
                        .unwrap_or_else(|| "未检测到网易云播放".to_string());

                    unsafe {
                        let screen_dc = GetDC(ptr::null_mut());
                        if !screen_dc.is_null() {
                            let dpi_x = GetDeviceCaps(screen_dc, 88);
                            let dpi_scale = dpi_x as f32 / 96.0;
                            let hfont = create_compatible_font(dpi_scale, &settings.font);
                            if !hfont.is_null() {
                                let val_w = measure_text_width(screen_dc, hfont, &super::truncate_overlay_text(&lyric_text, 26));
                                let est = (val_w + (34.0 * dpi_scale) as i32 + 24).clamp(220, 460);
                                if est > lyric_item_width {
                                    lyric_item_width = est;
                                }
                                DeleteObject(hfont as _);
                            }
                            ReleaseDC(ptr::null_mut(), screen_dc);
                        }
                    }
                }
            }
        }

        let mut width = 0i32;
        let mut enabled_count = 0i32;
        for item in &settings.display_items {
            if item.enabled {
                enabled_count += 1;
                match item.id.as_str() {
                    "delta_password" => { width += password_item_width; }
                    "netease_lyric" => { width += lyric_item_width; }
                    _ => { width += normal_item_width; }
                }
            }
        }

        // 自定义项宽度（各 150px 基础宽度）
        let custom_item_width = 150;
        let mut custom_count = 0i32;
        for custom in &settings.custom_items {
            if custom.enabled && !custom.text.is_empty() {
                width += custom_item_width;
                custom_count += 1;
            }
        }

        if width == 0 { return 200; }
        enabled_count += custom_count;
        let sep_count = if enabled_count > 1 { enabled_count - 1 } else { 0 };
        width + 32 + sep_count * 16
    }

    pub unsafe fn create_overlay_window(
        settings: &super::OverlaySettings,
    ) -> Result<HWND, String> {
        init_gdiplus();

        let h_instance = GetModuleHandleW(ptr::null());
        if h_instance.is_null() {
            return Err("无法获取模块句柄".to_string());
        }

        let class_name = windows_sys::core::w!("NexBoxOverlayPanel");

        let wnd_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: h_instance,
            hIcon: LoadIconW(h_instance, IDI_APPLICATION),
            hCursor: LoadCursorW(h_instance, IDC_ARROW),
            hbrBackground: CreateSolidBrush(0),
            lpszMenuName: ptr::null(),
            lpszClassName: class_name,
        };

        if RegisterClassW(&wnd_class) == 0 {
            let error = GetLastError();
            if error != 1410 {
                return Err(format!("注册窗口类失败: {}", error));
            }
        }

        let screen_dc = GetDC(ptr::null_mut());
        let dpi_x = if screen_dc.is_null() { 96 } else { GetDeviceCaps(screen_dc, 88) };
        if !screen_dc.is_null() {
            ReleaseDC(ptr::null_mut(), screen_dc);
        }
        let dpi_scale = dpi_x as f32 / 96.0;

        let logical_width = calculate_window_width(settings);
        let logical_height = if settings.style == "dynamic_island" { 36 } else { 28 };
        let physical_width = (logical_width as f32 * dpi_scale) as i32;
        let physical_height = (logical_height as f32 * dpi_scale) as i32;

        // 使用保存的位置，或使用默认位置
        let (x, y) = if let (Some(px), Some(py)) = (settings.position_x, settings.position_y) {
            (px, py)
        } else {
            let screen_width = GetSystemMetrics(SM_CXSCREEN);
            let default_x = (screen_width - physical_width) / 2;
            let default_y = if settings.style == "dynamic_island" { 4 } else { 0 };
            (default_x, default_y)
        };

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT,
            class_name,
            windows_sys::core::w!("NexBox Overlay Panel"),
            WS_POPUP,
            x,
            y,
            physical_width,
            physical_height,
            ptr::null_mut(),
            ptr::null_mut(),
            h_instance,
            ptr::null_mut(),
        );

        if hwnd.is_null() {
            return Err("创建窗口失败".to_string());
        }

        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);

        Ok(hwnd)
    }

    pub unsafe fn destroy_overlay_window(hwnd: HWND) -> bool {
        if hwnd.is_null() {
            return false;
        }
        KillTimer(hwnd, 1);
        DestroyWindow(hwnd) != 0
    }

    unsafe fn create_compatible_font(dpi_scale: f32, font_name: &str) -> HFONT {
        let font_height = -(13.0 * dpi_scale).round() as i32;
        let wide_name: Vec<u16> = font_name.encode_utf16().chain(std::iter::once(0)).collect();
        CreateFontW(
            font_height,
            0,
            0,
            0,
            FW_NORMAL as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            (DEFAULT_PITCH | FF_DONTCARE) as u32,
            wide_name.as_ptr(),
        )
    }

    pub unsafe fn register_custom_font(path: &str) -> bool {
        let wide_path: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let result = AddFontResourceExW(wide_path.as_ptr(), FR_PRIVATE, std::ptr::null_mut());
        result > 0
    }

    pub unsafe fn unregister_custom_font(path: &str) -> bool {
        let wide_path: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let result = RemoveFontResourceExW(wide_path.as_ptr(), FR_PRIVATE, std::ptr::null_mut());
        result > 0
    }

    unsafe fn measure_text_width(hdc: HDC, hfont: HFONT, text: &str) -> i32 {
        let old_font = SelectObject(hdc, hfont as _);
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let mut size = SIZE { cx: 0, cy: 0 };
        GetTextExtentPoint32W(hdc, wide.as_ptr(), (wide.len() - 1) as i32, &mut size);
        SelectObject(hdc, old_font);
        size.cx
    }

    struct DisplayItem {
        label: String,
        value: String,
        label_width: i32,
        value_width: i32,
        total_width: i32,
        custom_color: Option<u32>,
    }

    fn parse_hex_color(hex: &str) -> u32 {
        let hex = hex.trim_start_matches('#');
        if let Ok(val) = u32::from_str_radix(hex, 16) {
            // 前端使用 #RRGGBB 格式，GDI+ 颜色格式为 0x00BBGGRR
            let r = (val >> 16) & 0xFF;
            let g = (val >> 8) & 0xFF;
            let b = val & 0xFF;
            (b << 16) | (g << 8) | r
        } else {
            0x00FFFFFF
        }
    }

    fn build_display_items(
        settings: &super::OverlaySettings,
        data: &super::OverlayHardwareData,
    ) -> Vec<DisplayItem> {
        let mut items = Vec::new();
        for display_item in &settings.display_items {
            if !display_item.enabled {
                continue;
            }
            match display_item.id.as_str() {
                "cpu_usage" => {
                    let val = data.cpu_usage.map(|v| format!("{}%", v)).unwrap_or_else(|| "--%".to_string());
                    items.push(DisplayItem { label: "CPU".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: None });
                }
                "gpu_temp" => {
                    let val = data.gpu_temp.map(|v| format!("{:.0}°C", v)).unwrap_or_else(|| "--°C".to_string());
                    items.push(DisplayItem { label: "GPU".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: None });
                }
                "gpu_usage" => {
                    let val = data.gpu_usage.map(|v| format!("{}%", v)).unwrap_or_else(|| "--%".to_string());
                    items.push(DisplayItem { label: "GPU".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: None });
                }
                "memory_usage" => {
                    let val = data.memory_usage.map(|v| format!("{}%", v.round() as i32)).unwrap_or_else(|| "--%".to_string());
                    items.push(DisplayItem { label: "RAM".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: None });
                }
                "delta_password" => {
                    let val = data.delta_password.as_deref().unwrap_or("--").to_string();
                    items.push(DisplayItem { label: "".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: None });
                }
                "game_ping" => {
                    let val = data.game_ping.map(|v| format!("{}ms", v)).unwrap_or_else(|| "--ms".to_string());
                    items.push(DisplayItem { label: "PING".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: None });
                }
                "netease_lyric" => {
                    let val = data
                        .netease_current_lyric
                        .clone()
                        .map(|text| super::truncate_overlay_text(&text, 26))
                        .or_else(|| {
                            super::build_netease_song_text(
                                data.netease_song_title.as_deref(),
                                data.netease_song_artist.as_deref(),
                            )
                            .map(|text| super::truncate_overlay_text(&text, 28))
                        })
                        .unwrap_or_else(|| "未检测到网易云播放".to_string());
                    items.push(DisplayItem { label: "♪".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: Some(0x00D7F5FFu32) });
                }
                "fps" => {
                    let (val, color) = match data.fps {
                        Some(v) => {
                            let c = if v < 30 {
                                0x000000FFu32
                            } else if v < 60 {
                                0x0000FFFFu32
                            } else {
                                0x0000FF00u32
                            };
                            (format!("{}", v), Some(c))
                        }
                        None => ("--".to_string(), None),
                    };
                    items.push(DisplayItem { label: "FPS".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: color });
                }
                "heart_rate" => {
                    let val = data.heart_rate.map(|v| format!("{}", v)).unwrap_or_else(|| "--".to_string());
                    let color = match data.heart_rate {
                        Some(v) if v < 60 => Some(0x0000FFFFu32),
                        Some(v) if v < 100 => Some(0x0000FF00u32),
                        Some(v) if v < 140 => Some(0x0000FFFFu32),
                        Some(_) => Some(0x000000FFu32),
                        None => None,
                    };
                    items.push(DisplayItem { label: "❤".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: color });
                }
                "gpu_fan_speed" => {
                    let val = data.gpu_fan_speed.map(|v| format!("{}RPM", v)).unwrap_or_else(|| "--RPM".to_string());
                    items.push(DisplayItem { label: "GPU".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: Some(0x0000FF00u32) });
                }
                "gpu_power" => {
                    let val = data.gpu_power.map(|v| format!("{}W", v)).unwrap_or_else(|| "--W".to_string());
                    items.push(DisplayItem { label: "GPU".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: Some(0x0000FF00u32) });
                }
                "gpu_clock" => {
                    let val = data.gpu_clock.map(|v| format!("{}MHz", v)).unwrap_or_else(|| "--MHz".to_string());
                    items.push(DisplayItem { label: "GPU".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: Some(0x0000FF00u32) });
                }
                "gpu_vram" => {
                    let val = match (data.gpu_vram_used, data.gpu_vram_total) {
                        (Some(used), Some(total)) => {
                            let used_gb = used as f64 / 1024.0;
                            let total_gb = total as f64 / 1024.0;
                            format!("{:.1}G/{:.1}G", used_gb, total_gb)
                        }
                        _ => "--G/--G".to_string(),
                    };
                    items.push(DisplayItem { label: "GPU".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: Some(0x0000FF00u32) });
                }
                "gpu_memory_clock" => {
                    let val = data.gpu_memory_clock.map(|v| format!("{}MHz", v)).unwrap_or_else(|| "--MHz".to_string());
                    items.push(DisplayItem { label: "GPU".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: Some(0x0000FF00u32) });
                }
                "gpu_voltage" => {
                    let val = data.gpu_voltage.map(|v| format!("{:.3}V", v)).unwrap_or_else(|| "--V".to_string());
                    items.push(DisplayItem { label: "GPU".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: Some(0x0000FF00u32) });
                }
                "cpu_temp" => {
                    let val = data.cpu_temp.map(|v| format!("{:.0}°C", v)).unwrap_or_else(|| "--°C".to_string());
                    items.push(DisplayItem { label: "CPU".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: None });
                }
                "cpu_clock" => {
                    let val = data.cpu_clock.map(|v| format!("{}MHz", v)).unwrap_or_else(|| "--MHz".to_string());
                    items.push(DisplayItem { label: "CPU".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: Some(0x0000FF00u32) });
                }
                "cpu_voltage" => {
                    let val = data.cpu_voltage.map(|v| format!("{:.3}V", v)).unwrap_or_else(|| "--V".to_string());
                    items.push(DisplayItem { label: "CPU".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: None });
                }
                "cpu_power" => {
                    let val = data.cpu_power.map(|v| format!("{:.1}W", v)).unwrap_or_else(|| "--W".to_string());
                    items.push(DisplayItem { label: "CPU".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: None });
                }
                "ssd_temp" => {
                    let val = data.ssd_temp.map(|v| format!("{:.0}°C", v)).unwrap_or_else(|| "--°C".to_string());
                    items.push(DisplayItem { label: "硬盘".to_string(), value: val, label_width: 0, value_width: 0, total_width: 0, custom_color: None });
                }
                _ => {}
            }
        }
        for custom in &settings.custom_items {
            if custom.enabled && !custom.text.is_empty() {
                let color = parse_hex_color(&custom.color);
                items.push(DisplayItem {
                    label: String::new(),
                    value: custom.text.clone(),
                    label_width: 0,
                    value_width: 0,
                    total_width: 0,
                    custom_color: Some(color),
                });
            }
        }
        items
    }

    unsafe fn measure_and_layout_items(
        hdc: HDC,
        hfont: HFONT,
        items: &mut [DisplayItem],
        dpi_scale: f32,
    ) -> i32 {
        let gap = (10.0 * dpi_scale) as i32;
        let mut total = 0i32;
        for item in items.iter_mut() {
            item.label_width = measure_text_width(hdc, hfont, &item.label);
            item.value_width = measure_text_width(hdc, hfont, &item.value);
            if item.label.is_empty() {
                item.total_width = item.value_width;
            } else {
                item.total_width = item.label_width + gap + item.value_width;
            }
            total += item.total_width;
        }
        total
    }

    pub unsafe fn draw_overlay_content(
        hwnd: HWND,
        settings: &super::OverlaySettings,
        data: &super::OverlayHardwareData,
    ) {
        let dpi_scale = {
            let dc = GetDC(hwnd);
            let dpi = if dc.is_null() { 96 } else { GetDeviceCaps(dc, 88) };
            if !dc.is_null() {
                ReleaseDC(hwnd, dc);
            }
            dpi as f32 / 96.0
        };

        let hfont = create_compatible_font(dpi_scale, &settings.font);
        if hfont.is_null() {
            return;
        }

        let temp_dc = GetDC(ptr::null_mut());
        let mut items = build_display_items(settings, data);
        let padding = (16.0 * dpi_scale) as i32;
        let item_gap = (16.0 * dpi_scale) as i32;
        let content_width = measure_and_layout_items(temp_dc, hfont, &mut items, dpi_scale);
        ReleaseDC(ptr::null_mut(), temp_dc);
        let sep_count = if items.len() > 1 { items.len() as i32 - 1 } else { 0 };
        let total_content_width = content_width + sep_count * item_gap + padding * 2;
        let logical_height = 28;
        let physical_height = (logical_height as f32 * dpi_scale) as i32;

        let dib_width = total_content_width;
        let dib_height = physical_height;

        let screen_dc = GetDC(ptr::null_mut());
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = dib_width;
        bmi.bmiHeader.biHeight = -dib_height;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;

        let mut bits: *mut std::ffi::c_void = ptr::null_mut();
        let hbitmap = CreateDIBSection(screen_dc, &bmi, DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0);
        ReleaseDC(ptr::null_mut(), screen_dc);

        if hbitmap.is_null() {
            DeleteObject(hfont as _);
            return;
        }

        let mem_dc = CreateCompatibleDC(ptr::null_mut());
        let old_bmp = SelectObject(mem_dc, hbitmap as HGDIOBJ);

        let mut graphics: *mut GpGraphics = ptr::null_mut();
        if GdipCreateFromHDC(mem_dc, &mut graphics) != 0 {
            SelectObject(mem_dc, old_bmp);
            DeleteObject(hbitmap as HGDIOBJ);
            DeleteDC(mem_dc);
            DeleteObject(hfont as _);
            return;
        }

        GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias);

        let mut clear_brush: *mut GpSolidFill = ptr::null_mut();
        GdipCreateSolidFill(0x00000000, &mut clear_brush);
        GdipFillRectangle(graphics, clear_brush as *mut GpBrush, 0.0, 0.0, dib_width as f32, dib_height as f32);
        GdipDeleteBrush(clear_brush as *mut GpBrush);

        let bg_argb: u32 = ((settings.opacity as u32) << 24) | 0x00111111;
        let mut bg_brush: *mut GpSolidFill = ptr::null_mut();
        GdipCreateSolidFill(bg_argb, &mut bg_brush);
        GdipFillRectangle(graphics, bg_brush as *mut GpBrush, 0.0, 0.0, dib_width as f32, dib_height as f32);
        GdipDeleteBrush(bg_brush as *mut GpBrush);
        GdipDeleteGraphics(graphics);

        let old_font = SelectObject(mem_dc, hfont as _);
        SetBkMode(mem_dc, TRANSPARENT as i32);

        let gap = (10.0 * dpi_scale) as i32;
        let mut current_x: i32 = padding;
        let win_height_i32 = dib_height;

        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                current_x += item_gap;
            }

            if !item.label.is_empty() {
                let wide_label: Vec<u16> = item.label.encode_utf16().chain(std::iter::once(0)).collect();
                let mut label_rect = RECT {
                    left: current_x,
                    top: 0,
                    right: current_x + item.label_width,
                    bottom: win_height_i32,
                };
                SetTextColor(mem_dc, 0x00FFFFFF);
                DrawTextW(
                    mem_dc,
                    wide_label.as_ptr(),
                    (wide_label.len() - 1) as i32,
                    &mut label_rect,
                    DT_RIGHT | DT_VCENTER | DT_SINGLELINE,
                );
            }

            let value_x = if item.label.is_empty() {
                current_x
            } else {
                current_x + item.label_width + gap
            };
            let wide_value: Vec<u16> = item.value.encode_utf16().chain(std::iter::once(0)).collect();
            let mut value_rect = RECT {
                left: value_x,
                top: 0,
                right: value_x + item.value_width,
                bottom: win_height_i32,
            };

            let mut color: u32 = 0x00FFFFFF;
            if let Some(custom_color) = item.custom_color {
                color = custom_color;
            } else if !item.label.is_empty() && !item.value.contains("--") {
                let mut num_str = String::new();
                for ch in item.value.chars() {
                    if ch.is_ascii_digit() || ch == '.' {
                        num_str.push(ch);
                    } else if !num_str.is_empty() {
                        break;
                    }
                }
                if !num_str.is_empty() {
                    if let Ok(nf) = num_str.parse::<f32>() {
                        let nv = nf as i32;
                        if nv < 50 {
                            color = 0x0000FF00;
                        } else if nv < 80 {
                            color = 0x0000FFFF;
                        } else {
                            color = 0x000000FF;
                        }
                    }
                }
            }

            SetTextColor(mem_dc, color);
            DrawTextW(
                mem_dc,
                wide_value.as_ptr(),
                (wide_value.len() - 1) as i32,
                &mut value_rect,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            current_x += item.total_width;
        }

        SelectObject(mem_dc, old_font);

        if !bits.is_null() {
            let pixels = std::slice::from_raw_parts_mut(
                bits as *mut u32,
                (dib_width * dib_height) as usize,
            );
            for pixel in pixels.iter_mut() {
                let alpha = (*pixel >> 24) & 0xFF;
                let rgb = *pixel & 0x00FFFFFF;
                if alpha == 0 && rgb != 0 {
                    *pixel = 0xFF000000 | rgb;
                }
            }
        }

        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let default_x = (screen_width - dib_width) / 2;
        let use_x = settings.position_x.unwrap_or(default_x);
        let use_y = settings.position_y.unwrap_or(0);

        let ppt_dst = POINT { x: use_x, y: use_y };
        let psize = SIZE { cx: dib_width, cy: dib_height };
        let ppt_src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };

        UpdateLayeredWindow(
            hwnd,
            ptr::null_mut(),
            &ppt_dst,
            &psize,
            mem_dc,
            &ppt_src,
            0,
            &blend,
            ULW_ALPHA,
        );

        SelectObject(mem_dc, old_bmp);
        DeleteObject(hbitmap as HGDIOBJ);
        DeleteDC(mem_dc);
        DeleteObject(hfont as _);
    }

    pub unsafe fn draw_overlay_content_dynamic_island(
        hwnd: HWND,
        settings: &super::OverlaySettings,
        data: &super::OverlayHardwareData,
    ) {
        let dpi_scale = {
            let dc = GetDC(hwnd);
            let dpi = if dc.is_null() { 96 } else { GetDeviceCaps(dc, 88) };
            if !dc.is_null() {
                ReleaseDC(hwnd, dc);
            }
            dpi as f32 / 96.0
        };

        let hfont = create_compatible_font(dpi_scale, &settings.font);
        if hfont.is_null() {
            return;
        }

        // Build a temp DC just for measurement
        let temp_dc = GetDC(ptr::null_mut());
        let mut items = build_display_items(settings, data);
        let padding = (16.0 * dpi_scale) as i32;
        let item_gap = (16.0 * dpi_scale) as i32;
        let content_width = measure_and_layout_items(temp_dc, hfont, &mut items, dpi_scale);
        ReleaseDC(ptr::null_mut(), temp_dc);
        let sep_count = if items.len() > 1 { items.len() as i32 - 1 } else { 0 };
        let total_content_width = content_width + sep_count * item_gap + padding * 2;
        let logical_height = 36;
        let physical_height = (logical_height as f32 * dpi_scale) as i32;

        let dib_width = total_content_width;
        let dib_height = physical_height;

        // --- Create 32-bit ARGB DIB section (like crosshair) ---
        let screen_dc = GetDC(ptr::null_mut());
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = dib_width;
        bmi.bmiHeader.biHeight = -dib_height;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;

        let mut bits: *mut std::ffi::c_void = ptr::null_mut();
        let hbitmap = CreateDIBSection(screen_dc, &bmi, DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0);
        ReleaseDC(ptr::null_mut(), screen_dc);

        if hbitmap.is_null() {
            DeleteObject(hfont as _);
            return;
        }

        let mem_dc = CreateCompatibleDC(ptr::null_mut());
        let old_bmp = SelectObject(mem_dc, hbitmap as HGDIOBJ);

        // --- GDI+ anti-aliased rounded rect background ---
        let mut graphics: *mut GpGraphics = ptr::null_mut();
        if GdipCreateFromHDC(mem_dc, &mut graphics) != 0 {
            SelectObject(mem_dc, old_bmp);
            DeleteObject(hbitmap as HGDIOBJ);
            DeleteDC(mem_dc);
            DeleteObject(hfont as _);
            return;
        }

        GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias);

        // Clear to fully transparent
        let mut clear_brush: *mut GpSolidFill = ptr::null_mut();
        GdipCreateSolidFill(0x00000000, &mut clear_brush);
        GdipFillRectangle(graphics, clear_brush as *mut GpBrush, 0.0, 0.0, dib_width as f32, dib_height as f32);
        GdipDeleteBrush(clear_brush as *mut GpBrush);

        // Draw rounded rect with GDI+ (proper per-pixel alpha anti-aliasing)
        let bg_argb: u32 = ((settings.opacity as u32) << 24) | 0x00111111;
        let corner_r = dib_height as f32 * 0.5;
        let mut bg_brush: *mut GpSolidFill = ptr::null_mut();
        GdipCreateSolidFill(bg_argb, &mut bg_brush);

        let mut path: *mut GpPath = ptr::null_mut();
        GdipCreatePath(FillModeAlternate, &mut path);
        if !path.is_null() {
            let w = dib_width as f32;
            let h = dib_height as f32;
            let r = corner_r;
            GdipAddPathArc(path, 0.0, 0.0, r * 2.0, r * 2.0, 180.0, 90.0);
            GdipAddPathLine(path, r, 0.0, w - r, 0.0);
            GdipAddPathArc(path, w - r * 2.0, 0.0, r * 2.0, r * 2.0, 270.0, 90.0);
            GdipAddPathLine(path, w, r, w, h - r);
            GdipAddPathArc(path, w - r * 2.0, h - r * 2.0, r * 2.0, r * 2.0, 0.0, 90.0);
            GdipAddPathLine(path, w - r, h, r, h);
            GdipAddPathArc(path, 0.0, h - r * 2.0, r * 2.0, r * 2.0, 90.0, 90.0);
            GdipAddPathLine(path, 0.0, h - r, 0.0, r);
            GdipClosePathFigure(path);
            GdipFillPath(graphics, bg_brush as *mut GpBrush, path);
            GdipDeletePath(path);
        }
        GdipDeleteBrush(bg_brush as *mut GpBrush);
        GdipDeleteGraphics(graphics);

        // --- Draw text using GDI ---
        let old_font = SelectObject(mem_dc, hfont as _);
        SetBkMode(mem_dc, TRANSPARENT as i32);

        let gap = (10.0 * dpi_scale) as i32;
        let mut current_x: i32 = padding;
        let win_height_i32 = dib_height;

        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                current_x += item_gap;
            }

            if !item.label.is_empty() {
                let wide_label: Vec<u16> = item.label.encode_utf16().chain(std::iter::once(0)).collect();
                let mut label_rect = RECT {
                    left: current_x,
                    top: 0,
                    right: current_x + item.label_width,
                    bottom: win_height_i32,
                };
                SetTextColor(mem_dc, 0x00FFFFFF);
                DrawTextW(
                    mem_dc,
                    wide_label.as_ptr(),
                    (wide_label.len() - 1) as i32,
                    &mut label_rect,
                    DT_RIGHT | DT_VCENTER | DT_SINGLELINE,
                );
            }

            let value_x = if item.label.is_empty() {
                current_x
            } else {
                current_x + item.label_width + gap
            };
            let wide_value: Vec<u16> = item.value.encode_utf16().chain(std::iter::once(0)).collect();
            let mut value_rect = RECT {
                left: value_x,
                top: 0,
                right: value_x + item.value_width,
                bottom: win_height_i32,
            };

            let mut color: u32 = 0x00FFFFFF;
            if let Some(custom_color) = item.custom_color {
                color = custom_color;
            } else if !item.label.is_empty() && !item.value.contains("--") {
                let mut num_str = String::new();
                for ch in item.value.chars() {
                    if ch.is_ascii_digit() || ch == '.' {
                        num_str.push(ch);
                    } else if !num_str.is_empty() {
                        break;
                    }
                }
                if !num_str.is_empty() {
                    if let Ok(nf) = num_str.parse::<f32>() {
                        let nv = nf as i32;
                        if nv < 50 {
                            color = 0x0000FF00;
                        } else if nv < 80 {
                            color = 0x0000FFFF;
                        } else {
                            color = 0x000000FF;
                        }
                    }
                }
            }

            SetTextColor(mem_dc, color);
            DrawTextW(
                mem_dc,
                wide_value.as_ptr(),
                (wide_value.len() - 1) as i32,
                &mut value_rect,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            current_x += item.total_width;
        }

        SelectObject(mem_dc, old_font);

        // Fix alpha for text pixels: GDI sets RGB but alpha stays 0
        if !bits.is_null() {
            let pixels = std::slice::from_raw_parts_mut(
                bits as *mut u32,
                (dib_width * dib_height) as usize,
            );
            for pixel in pixels.iter_mut() {
                let alpha = (*pixel >> 24) & 0xFF;
                let rgb = *pixel & 0x00FFFFFF;
                if alpha == 0 && rgb != 0 {
                    *pixel = 0xFF000000 | rgb;
                }
            }
        }

        // --- Position and composite via UpdateLayeredWindow ---
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let default_x = (screen_width - dib_width) / 2;
        let use_x = settings.position_x.unwrap_or(default_x);
        let use_y = settings.position_y.unwrap_or(4);

        let ppt_dst = POINT { x: use_x, y: use_y };
        let psize = SIZE { cx: dib_width, cy: dib_height };
        let ppt_src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };

        UpdateLayeredWindow(
            hwnd,
            ptr::null_mut(),
            &ppt_dst,
            &psize,
            mem_dc,
            &ppt_src,
            0,
            &blend,
            ULW_ALPHA,
        );

        SelectObject(mem_dc, old_bmp);
        DeleteObject(hbitmap as HGDIOBJ);
        DeleteDC(mem_dc);
        DeleteObject(hfont as _);
    }

    pub unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT {
                    hdc: ptr::null_mut(),
                    fErase: 0,
                    rcPaint: RECT {
                        left: 0,
                        top: 0,
                        right: 0,
                        bottom: 0,
                    },
                    fRestore: 0,
                    fIncUpdate: 0,
                    rgbReserved: [0u8; 32],
                };
                BeginPaint(hwnd, &mut ps);
                EndPaint(hwnd, &ps);
                0
            }
            WM_TIMER => {
                // 定时器重置为 100ms 刷新以保证歌词等数据同步的实时性
                SetTimer(hwnd, 1, 100, None);
                let data = super::collect_hardware_data();
                *super::CURRENT_HARDWARE_DATA.lock().unwrap() = Some(data.clone());
                let settings = super::get_or_init_settings();
                if settings.style == "dynamic_island" {
                    draw_overlay_content_dynamic_island(hwnd, &settings, &data);
                } else {
                    draw_overlay_content(hwnd, &settings, &data);
                }
                0
            }
            WM_NCHITTEST => {
                // 拖动模式下返回 HTCAPTION 允许拖动
                if super::DRAG_MODE.load(std::sync::atomic::Ordering::SeqCst) {
                    HTCAPTION as LRESULT
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
            WM_EXITSIZEMOVE => {
                // 拖动结束后只保存位置，不退出拖动模式
                // 退出由前端按钮控制，避免样式切换导致位置重置
                if super::DRAG_MODE.load(std::sync::atomic::Ordering::SeqCst) {
                    // 获取当前窗口位置
                    let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
                    GetWindowRect(hwnd, &mut rect);

                    // 保存位置到设置
                    {
                        let mut settings_lock = super::CURRENT_SETTINGS.lock().unwrap();
                        if let Some(ref mut settings) = *settings_lock {
                            settings.position_x = Some(rect.left);
                            settings.position_y = Some(rect.top);
                        }
                    }

                    // 标记位置已变更
                    super::POSITION_CHANGED.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

// 设置拖动模式
pub fn set_drag_mode(enabled: bool) {
    DRAG_MODE.store(enabled, Ordering::SeqCst);

    #[cfg(target_os = "windows")]
    unsafe {
        let hwnd = OVERLAY_HANDLE.load(Ordering::SeqCst);
        if !hwnd.is_null() {
            use windows_sys::Win32::UI::WindowsAndMessaging::*;

            // 获取当前窗口样式
            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;

            if enabled {
                // 进入拖动模式：移除 WS_EX_TRANSPARENT
                let new_style = ex_style & !WS_EX_TRANSPARENT;
                SetWindowLongW(hwnd, GWL_EXSTYLE, new_style as i32);
            } else {
                // 退出拖动模式：恢复 WS_EX_TRANSPARENT
                let new_style = ex_style | WS_EX_TRANSPARENT;
                SetWindowLongW(hwnd, GWL_EXSTYLE, new_style as i32);
            }

            // 刷新窗口样式
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }
}

#[cfg(target_os = "windows")]
pub fn start_overlay(settings: OverlaySettings) -> Result<OverlayResult, String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    if OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        return Ok(OverlayResult {
            success: true,
            message: "悬浮框已处于启用状态".to_string(),
        });
    }

    OVERLAY_ACTIVE.store(true, Ordering::SeqCst);

    {
        let mut settings_lock = CURRENT_SETTINGS.lock().unwrap();
        *settings_lock = Some(settings.clone());
    }

    // Register MiSans font if selected
    if settings.font == "MiSans Medium" {
        if let Ok(path_lock) = MISANS_FONT_PATH.lock() {
            if let Some(ref path) = *path_lock {
                unsafe {
                    win32::register_custom_font(path);
                }
            }
        }
    }

    thread::spawn(move || {
        use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

        crate::game_ping::start_ping_thread();
        crate::game_fps::start_fps_monitor();

        let com_initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).is_ok() };
        if !com_initialized {
            log::warn!("悬浮框线程初始化 COM 失败，网易云歌词功能可能不可用");
        }

        unsafe {
            match win32::create_overlay_window(&settings) {
                std::result::Result::Ok(hwnd) => {
                    OVERLAY_HANDLE.store(hwnd, Ordering::SeqCst);
                    crate::game_fps::set_overlay_hwnd(hwnd as u64);

                    let data = OverlayHardwareData::default();
                    *CURRENT_HARDWARE_DATA.lock().unwrap() = Some(data.clone());

                    if settings.style == "dynamic_island" {
                        win32::draw_overlay_content_dynamic_island(hwnd, &settings, &data);
                    } else {
                        win32::draw_overlay_content(hwnd, &settings, &data);
                    }

                    SetTimer(hwnd, 1, 100, None);
                    win32::install_topmost_guard();

                    let mut msg: MSG = std::mem::zeroed();
                    while OVERLAY_ACTIVE.load(Ordering::SeqCst) {
                        while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                            if msg.message == WM_QUIT {
                                break;
                            }
                            TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                        }

                        if !OVERLAY_ACTIVE.load(Ordering::SeqCst) {
                            break;
                        }

                        thread::sleep(Duration::from_millis(50));
                    }

                    win32::uninstall_topmost_guard();
                    win32::destroy_overlay_window(hwnd);
                    crate::game_fps::clear_overlay_hwnd();
                    OVERLAY_HANDLE.store(std::ptr::null_mut(), Ordering::SeqCst);
                }
                std::result::Result::Err(e) => {
                    log::error!("创建悬浮框窗口失败: {}", e);
                    OVERLAY_ACTIVE.store(false, Ordering::SeqCst);
                }
            }
        }

        if com_initialized {
            unsafe {
                CoUninitialize();
            }
        }
    });

    Ok(OverlayResult {
        success: true,
        message: "悬浮框已启动".to_string(),
    })
}

#[cfg(target_os = "windows")]
pub fn stop_overlay() -> Result<OverlayResult, String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW;
    use windows_sys::Win32::UI::WindowsAndMessaging::WM_CLOSE;

    if !OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        return Ok(OverlayResult {
            success: true,
            message: "悬浮框已处于关闭状态".to_string(),
        });
    }

    OVERLAY_ACTIVE.store(false, Ordering::SeqCst);

    crate::game_ping::stop_ping_thread();
    crate::game_fps::stop_fps_monitor();

    unsafe {
        let hwnd = OVERLAY_HANDLE.load(Ordering::SeqCst);
        if !hwnd.is_null() {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
    }

    Ok(OverlayResult {
        success: true,
        message: "悬浮框已关闭".to_string(),
    })
}

#[cfg(not(target_os = "windows"))]
pub fn start_overlay(_settings: OverlaySettings) -> Result<OverlayResult, String> {
    Err("此功能仅支持 Windows 系统".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn stop_overlay() -> Result<OverlayResult, String> {
    Err("此功能仅支持 Windows 系统".to_string())
}

/// Toggle overlay on/off. Used by global hotkey.
pub fn toggle_overlay(app_handle: &tauri::AppHandle) -> Result<OverlayResult, String> {
    let result = if OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        stop_overlay()
    } else {
        let settings = get_or_init_settings();
        start_overlay(settings)
    };

    if result.is_ok() {
        let _ = app_handle.emit("overlay-status-changed", ());
    }

    result
}

#[tauri::command]
pub async fn start_overlay_panel(settings: Option<OverlaySettings>) -> Result<OverlayResult, String> {
    let settings = settings.unwrap_or_default();
    start_overlay(settings)
}

#[tauri::command]
pub async fn stop_overlay_panel() -> Result<OverlayResult, String> {
    stop_overlay()
}

#[tauri::command]
pub async fn toggle_overlay_panel(app_handle: tauri::AppHandle) -> Result<OverlayResult, String> {
    toggle_overlay(&app_handle)
}

#[tauri::command]
pub async fn get_overlay_panel_status() -> Result<bool, String> {
    Ok(OVERLAY_ACTIVE.load(Ordering::SeqCst))
}

#[tauri::command]
pub async fn get_overlay_hardware_data() -> Result<OverlayHardwareData, String> {
    let data = CURRENT_HARDWARE_DATA
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_default();
    Ok(data)
}

#[tauri::command]
pub async fn update_overlay_settings(settings: OverlaySettings) -> Result<OverlayResult, String> {
    let (old_style, old_font) = {
        let lock = CURRENT_SETTINGS.lock().unwrap();
        let s = lock.as_ref();
        (s.map(|s| s.style.clone()), s.map(|s| s.font.clone()))
    };
    let new_style = settings.style.clone();
    let new_font = settings.font.clone();

    {
        let mut settings_lock = CURRENT_SETTINGS.lock().unwrap();
        *settings_lock = Some(settings);
    }

    if OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        let style_changed = old_style.as_deref() != Some(&new_style);
        let font_changed = old_font.as_deref() != Some(&new_font);
        if style_changed || font_changed {
            let new_settings = CURRENT_SETTINGS.lock().unwrap().clone().unwrap_or_default();
            stop_overlay()?;
            std::thread::sleep(std::time::Duration::from_millis(200));
            start_overlay(new_settings)?;
        } else {
            #[cfg(target_os = "windows")]
            unsafe {
                let hwnd = OVERLAY_HANDLE.load(Ordering::SeqCst);
                if !hwnd.is_null() {
                    let data = CURRENT_HARDWARE_DATA.lock().unwrap().clone().unwrap_or_default();
                    let current_settings = CURRENT_SETTINGS.lock().unwrap().clone().unwrap_or_default();
                    if new_style == "dynamic_island" {
                        win32::draw_overlay_content_dynamic_island(hwnd, &current_settings, &data);
                    } else {
                        win32::draw_overlay_content(hwnd, &current_settings, &data);
                    }
                }
            }
        }
    }

    Ok(OverlayResult {
        success: true,
        message: "设置已更新".to_string(),
    })
}

#[tauri::command]
pub async fn set_overlay_drag_mode(enabled: bool) -> Result<OverlayResult, String> {
    if !OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        return Err("悬浮框未启用".to_string());
    }

    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        use windows_sys::Win32::Foundation::RECT;
        
        let hwnd = OVERLAY_HANDLE.load(Ordering::SeqCst);
        if hwnd.is_null() {
            return Err("悬浮框窗口不存在".to_string());
        }

        if !enabled {
            // 退出拖动模式时，先保存当前位置
            let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            GetWindowRect(hwnd, &mut rect);
            
            {
                let mut settings_lock = CURRENT_SETTINGS.lock().unwrap();
                if let Some(ref mut settings) = *settings_lock {
                    settings.position_x = Some(rect.left);
                    settings.position_y = Some(rect.top);
                }
            }
        }

        // 切换拖动模式
        set_drag_mode(enabled);

        if !enabled {
            // 退出拖动模式后，恢复窗口到保存的位置
            let (saved_x, saved_y) = {
                let settings_lock = CURRENT_SETTINGS.lock().unwrap();
                if let Some(ref settings) = *settings_lock {
                    (settings.position_x, settings.position_y)
                } else {
                    (None, None)
                }
            };
            
            // 获取当前位置
            let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            GetWindowRect(hwnd, &mut rect);
            
            // 如果位置发生变化，恢复到保存的位置
            if let (Some(sx), Some(sy)) = (saved_x, saved_y) {
                if rect.left != sx || rect.top != sy {
                    SetWindowPos(
                        hwnd,
                        HWND_TOPMOST,
                        sx,
                        sy,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                }
            }
            
            POSITION_CHANGED.store(false, Ordering::SeqCst);
        }
    }

    let message = if enabled { 
        "已进入拖动模式".to_string()
    } else {
        "已退出拖动模式".to_string()
    };

    Ok(OverlayResult {
        success: true,
        message,
    })
}

#[tauri::command]
pub async fn get_overlay_current_settings() -> Result<OverlaySettings, String> {
    let mut settings = CURRENT_SETTINGS.lock().unwrap().clone().unwrap_or_default();
    // 合并默认项：新增的显示项自动追加到已有设置中
    let defaults = default_display_items();
    let mut merged = false;
    for default_item in &defaults {
        if !settings.display_items.iter().any(|i| i.id == default_item.id) {
            settings.display_items.push(default_item.clone());
            merged = true;
        }
    }
    drop(defaults); // 释放借用
    if merged {
        let mut lock = CURRENT_SETTINGS.lock().unwrap();
        *lock = Some(settings.clone());
    }
    Ok(settings)
}

#[tauri::command]
pub async fn check_drag_mode_status() -> Result<bool, String> {
    // 返回当前拖动模式状态
    Ok(DRAG_MODE.load(Ordering::SeqCst))
}

#[tauri::command]
pub async fn reset_overlay_position() -> Result<OverlayResult, String> {
    if !OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        return Err("悬浮框未启用".to_string());
    }

    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;

        let hwnd = OVERLAY_HANDLE.load(Ordering::SeqCst);
        if hwnd.is_null() {
            return Err("悬浮框窗口不存在".to_string());
        }

        // 清除已保存的位置，恢复默认居中
        {
            let mut settings_lock = CURRENT_SETTINGS.lock().unwrap();
            if let Some(ref mut settings) = *settings_lock {
                settings.position_x = None;
                settings.position_y = None;
            }
        }

        // 获取当前窗口大小
        let mut rect = std::mem::zeroed();
        GetWindowRect(hwnd, &mut rect);
        let win_w = rect.right - rect.left;
        let win_h = rect.bottom - rect.top;

        // 计算居中位置
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);
        let new_x = (screen_width - win_w) / 2;
        let new_y = (screen_height - win_h) / 2;

        // 移动窗口到居中位置
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            new_x,
            new_y,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }

    Ok(OverlayResult {
        success: true,
        message: "位置已重置为默认".to_string(),
    })
}

pub fn cleanup() {
    if OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        let _ = stop_overlay();
    }
    crate::game_ping::cleanup();
    crate::game_fps::cleanup();
    crate::heart_rate::cleanup();
    #[cfg(target_os = "windows")]
    unsafe {
        // Unregister MiSans font
        if let Ok(path_lock) = MISANS_FONT_PATH.lock() {
            if let Some(ref path) = *path_lock {
                win32::unregister_custom_font(path);
            }
        }
        win32::shutdown_gdiplus();
    }
}

#[tauri::command]
pub async fn run_pawnio_setup() -> Result<String, String> {
    let exe_dir = std::env::current_exe()
        .map_err(|e| format!("获取程序路径失败: {}", e))?;
    let parent_dir = exe_dir.parent().ok_or("无法获取父目录")?;

    let candidates = [
        parent_dir.join("PawnIO_setup.exe"),
        parent_dir.join("_up_").join("PawnIO_setup.exe"),
        parent_dir.join("resources").join("PawnIO_setup.exe"),
    ];

    for path in &candidates {
        if path.exists() {
            match std::process::Command::new(path).spawn() {
                Ok(_) => return Ok("安装程序已启动".to_string()),
                Err(e) => return Err(format!("启动安装程序失败: {}", e)),
            }
        }
    }

    Err("未找到 PawnIO_setup.exe，请确保已将其放在程序目录下".to_string())
}

#[tauri::command]
pub async fn get_misans_font_path(app_handle: tauri::AppHandle) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let resource_dir = app_handle
            .path()
            .resource_dir()
            .map_err(|e| format!("Failed to get resource dir: {}", e))?;

        let font_path = resource_dir.join("MiSans-Medium.ttf");
        let path_str = font_path.to_string_lossy().to_string();

        // Cache the path for later use by start_overlay/cleanup
        if let Ok(mut lock) = MISANS_FONT_PATH.lock() {
            *lock = Some(path_str.clone());
        }

        Ok(path_str)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(String::new())
    }
}
