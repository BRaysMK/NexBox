use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{App, AppHandle, Manager};

pub struct SensorChild(pub Mutex<Option<Child>>);

/// LHML 传感器单条数据（与 C# SensorReading 对齐）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReading {
    pub hardware: String,
    #[serde(rename = "hardwareType")]
    pub hardware_type: String,
    #[serde(rename = "subHardware")]
    pub sub_hardware: Option<String>,
    pub name: String,
    #[serde(rename = "sensorType")]
    pub sensor_type: String,
    pub value: f64,
    pub unit: Option<String>,
}

/// LHML 传感器响应（与 C# SensorsResponse 对齐）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorsResponse {
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub sensors: Vec<SensorReading>,
}

/// 管道桥接：管理 NexBoxMonitor 子进程的 stdin/stdout
pub struct SensorBridge {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
    writer: std::process::ChildStdin,
}

impl SensorBridge {
    /// 发送读取命令，返回传感器数据
    pub fn read_sensors(&mut self) -> Result<SensorsResponse, String> {
        // 发送命令
        writeln!(self.writer, r#"{{"cmd":"read"}}"#)
            .map_err(|e| format!("写入管道失败: {}", e))?;
        self.writer
            .flush()
            .map_err(|e| format!("刷新管道失败: {}", e))?;

        // 读取响应
        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .map_err(|e| format!("读取管道失败: {}", e))?;

        if line.trim().is_empty() {
            return Err("子进程返回空响应".to_string());
        }

        serde_json::from_str::<SensorsResponse>(&line)
            .map_err(|e| format!("解析传感器JSON失败: {}", e))
    }

    /// 检查子进程是否仍然存活
    pub fn is_alive(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => true,
            _ => false,
        }
    }

    /// 优雅关闭子进程
    pub fn shutdown(&mut self) {
        let _ = writeln!(self.writer, r#"{{"cmd":"exit"}}"#);
        let _ = self.writer.flush();
        let _ = self.child.wait();
    }
}

/// 全局传感器桥接
static SENSOR_BRIDGE: Mutex<Option<SensorBridge>> = Mutex::new(None);

/// 启动传感器子进程
pub fn start_sensor_process(app: &App) {
    match spawn_sensor() {
        Ok(Some(bridge)) => {
            log::info!("已启动 NexBoxMonitor 子进程 (pid={})", bridge.child.id());
            *SENSOR_BRIDGE.lock().unwrap() = Some(bridge);
            app.manage(SensorChild(Mutex::new(None))); // 保持兼容
        }
        Ok(None) => {
            log::info!("NexBoxMonitor 未找到，跳过启动");
            app.manage(SensorChild(Mutex::new(None)));
        }
        Err(e) => {
            log::warn!("启动 NexBoxMonitor 失败: {e}");
            app.manage(SensorChild(Mutex::new(None)));
        }
    }
}

/// 停止传感器子进程
pub fn stop_sensor_process(app: &AppHandle) {
    // 先处理旧的 SensorChild（兼容）
    if let Some(state) = app.try_state::<SensorChild>() {
        let child = {
            let mut guard = state
                .0
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.take()
        };
        if let Some(mut child) = child {
            log::info!("正在停止传感器子进程 (pid={})", child.id());
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    // 关闭新的 SensorBridge
    if let Some(mut bridge) = SENSOR_BRIDGE.lock().unwrap().take() {
        log::info!("正在关闭 NexBoxMonitor 子进程 (pid={})", bridge.child.id());
        bridge.shutdown();
    }
}

#[tauri::command]
pub async fn get_lhm_cpu_load() -> Result<Option<u16>, String> {
    match read_lhm_sensors() {
        Ok(response) => {
            for s in &response.sensors {
                if s.hardware_type.eq_ignore_ascii_case("CPU")
                    && s.sensor_type == "Load"
                    && (s.name == "CPU Total" || s.name == "Total")
                {
                    return Ok(Some(s.value as u16));
                }
            }
            Ok(None)
        }
        Err(e) => {
            log::warn!("LHML CPU load 读取失败: {e}");
            Ok(None)
        }
    }
}

#[tauri::command]
pub async fn get_lhm_cpu_status() -> Result<(Option<u16>, Option<f64>), String> {
    match read_lhm_sensors() {
        Ok(response) => {
            let mut load = None;
            let mut temp = None;
            for s in &response.sensors {
                if s.hardware_type.eq_ignore_ascii_case("CPU") {
                    if s.sensor_type == "Load" && (s.name == "CPU Total" || s.name == "Total") {
                        load = Some(s.value as u16);
                    }
                    if s.sensor_type == "Temperature"
                        && (s.name == "Core (Tctl/Tdie)" || s.name == "CPU Package" || s.name == "Tctl" || s.name == "Core")
                    {
                        temp = Some(s.value);
                    }
                }
            }
            Ok((load, temp))
        }
        Err(e) => {
            log::warn!("LHML CPU status 读取失败: {e}");
            Ok((None, None))
        }
    }
}

#[tauri::command]
pub async fn get_lhm_gpu_status() -> Result<Vec<(Option<f64>, Option<u32>)>, String> {
    match read_lhm_sensors() {
        Ok(response) => {
            let gpu_hardware_types: Vec<_> = {
                let mut types: Vec<_> = response.sensors.iter()
                    .filter(|s| s.hardware_type.to_lowercase().starts_with("gpu"))
                    .map(|s| s.hardware_type.clone())
                    .collect();
                types.dedup();
                types
            };

            let mut results = Vec::new();
            for hw_type in &gpu_hardware_types {
                if hw_type.eq_ignore_ascii_case("GpuIntel") { continue; }
                let temp = response.sensors.iter()
                    .filter(|s| s.hardware_type == *hw_type && s.sensor_type == "Temperature" && s.name == "GPU Core")
                    .map(|s| s.value)
                    .next();
                let usage = response.sensors.iter()
                    .filter(|s| s.hardware_type == *hw_type && s.sensor_type == "Load" && s.name == "GPU Core")
                    .map(|s| s.value as u32)
                    .next();
                results.push((temp, usage));
            }
            Ok(results)
        }
        Err(e) => {
            log::warn!("LHML GPU status 读取失败: {e}");
            Ok(Vec::new())
        }
    }
}

/// 从 LHML 读取传感器数据（供 overlay_panel.rs 调用）
pub fn read_lhm_sensors() -> Result<SensorsResponse, String> {
    let mut guard = SENSOR_BRIDGE
        .lock()
        .map_err(|e| format!("锁获取失败: {}", e))?;

    match guard.as_mut() {
        Some(bridge) => {
            if !bridge.is_alive() {
                log::warn!("NexBoxMonitor 子进程已退出，尝试重启...");
                *guard = None;
                drop(guard);
                match spawn_sensor() {
                    Ok(Some(new_bridge)) => {
                        log::info!("NexBoxMonitor 重启成功 (pid={})", new_bridge.child.id());
                        *SENSOR_BRIDGE.lock().unwrap() = Some(new_bridge);
                        return Err("子进程已重启，请重试".to_string());
                    }
                    _ => return Err("NexBoxMonitor 不可用".to_string()),
                }
            }
            bridge.read_sensors()
        }
        None => {
            // 尝试启动
            drop(guard);
            match spawn_sensor() {
                Ok(Some(bridge)) => {
                    log::info!("延迟启动 NexBoxMonitor (pid={})", bridge.child.id());
                    *SENSOR_BRIDGE.lock().unwrap() = Some(bridge);
                    Err("子进程已启动，请重试".to_string())
                }
                _ => Err("NexBoxMonitor 不可用".to_string()),
            }
        }
    }
}

/// 查找 NexBoxMonitor.exe 路径
fn find_monitor_exe() -> Option<std::path::PathBuf> {
    // 1. 首先检查当前 exe 所在目录下的 monitor 路径（安装版）
    if let Ok(exe_path) = std::env::current_exe() {
        let exe_dir = exe_path.parent().unwrap_or(std::path::Path::new("."));
        let monitor_path = exe_dir
            .join("monitor")
            .join("bin")
            .join("Release")
            .join("net10.0")
            .join("win-x64")
            .join("NexBoxMonitor.exe");
        if monitor_path.exists() {
            log::info!("找到 NexBoxMonitor: {}", monitor_path.display());
            return Some(monitor_path);
        }
    }

    // 2. 检查开发环境路径（cargo run 时）
    let dev_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("monitor")
        .join("bin")
        .join("Release")
        .join("net10.0")
        .join("win-x64")
        .join("NexBoxMonitor.exe");
    if dev_path.exists() {
        log::info!("找到 NexBoxMonitor (dev): {}", dev_path.display());
        return Some(dev_path);
    }

    // 3. 检查 publish 目录
    let publish_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("monitor")
        .join("bin")
        .join("Release")
        .join("net10.0")
        .join("win-x64")
        .join("publish")
        .join("NexBoxMonitor.exe");
    if publish_path.exists() {
        log::info!("找到 NexBoxMonitor (publish): {}", publish_path.display());
        return Some(publish_path);
    }

    log::warn!("未找到 NexBoxMonitor.exe");
    None
}

fn spawn_sensor() -> std::io::Result<Option<SensorBridge>> {
    let exe_path = match find_monitor_exe() {
        Some(p) => p,
        None => return Ok(None),
    };

    let mut cmd = Command::new(&exe_path);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    // Windows 下隐藏控制台窗口
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn()?;

    let stdin = child.stdin.take().expect("无法获取子进程 stdin");
    let stdout = child.stdout.take().expect("无法获取子进程 stdout");

    let bridge = SensorBridge {
        child,
        reader: BufReader::new(stdout),
        writer: stdin,
    };

    Ok(Some(bridge))
}