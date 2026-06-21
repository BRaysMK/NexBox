//! BLE 心率监控模块
//!
//! 通过 Windows BLE API 连接手环/手表等设备，获取实时心率数据。
//! 支持 Xiaomi Smart Band / Mi Band、Garmin、Polar、Samsung、HUAWEI 等设备。

use std::{
    collections::HashSet,
    sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex},
    time::Duration,
};
use tokio::{sync::mpsc, time};
use serde::Serialize;
use windows::{
    core::GUID,
    Devices::Bluetooth::{
        Advertisement::{
            BluetoothLEAdvertisementReceivedEventArgs,
            BluetoothLEAdvertisementWatcher,
            BluetoothLEScanningMode,
        },
        BluetoothConnectionStatus,
        BluetoothLEDevice,
        GenericAttributeProfile::{
            GattCharacteristic,
            GattClientCharacteristicConfigurationDescriptorValue,
            GattCommunicationStatus,
            GattValueChangedEventArgs,
            GattWriteOption,
        },
    },
    Devices::Enumeration::DeviceInformation,
    Foundation::TypedEventHandler,
    Storage::Streams::{DataReader, DataWriter},
};

// ── UUID 常量 ────────────────────────────────────────────────────────────────

const HR_SERVICE: GUID = GUID {
    data1: 0x0000_180d,
    data2: 0x0000,
    data3: 0x1000,
    data4: [0x80, 0x00, 0x00, 0x80, 0x5f, 0x9b, 0x34, 0xfb],
};
const HR_MEASUREMENT: GUID = GUID {
    data1: 0x0000_2a37,
    data2: 0x0000,
    data3: 0x1000,
    data4: [0x80, 0x00, 0x00, 0x80, 0x5f, 0x9b, 0x34, 0xfb],
};
const HR_CONTROL_POINT: GUID = GUID {
    data1: 0x0000_2a39,
    data2: 0x0000,
    data3: 0x1000,
    data4: [0x80, 0x00, 0x00, 0x80, 0x5f, 0x9b, 0x34, 0xfb],
};

// ── 品牌识别 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Brand {
    Xiaomi,
    Garmin,
    Polar,
    Samsung,
    Huawei,
    Fitbit,
    Amazfit,
    Apple,
    Generic,
}

impl Brand {
    fn detect(name: &str) -> Self {
        let n = name.to_ascii_lowercase();
        if n.contains("xiaomi smart band") || n.contains("mi band") || n.contains("miband")
            || n.contains("mi smart band")
        {
            Brand::Xiaomi
        } else if n.contains("garmin")
            || n.contains("forerunner")
            || n.contains("fenix")
            || n.contains("vivoactive")
            || n.contains("venu")
            || n.contains("instinct")
        {
            Brand::Garmin
        } else if n.contains("polar") {
            Brand::Polar
        } else if n.contains("galaxy watch") || n.contains("galaxy fit") || n.contains("gear fit") {
            Brand::Samsung
        } else if n.contains("huawei band")
            || n.contains("huawei watch")
            || n.contains("honor band")
        {
            Brand::Huawei
        } else if n.contains("versa")
            || n.contains("charge")
            || n.contains("sense")
            || n.contains("inspire")
            || n.contains("fitbit")
        {
            Brand::Fitbit
        } else if n.contains("amazfit") || n.contains("zepp") {
            Brand::Amazfit
        } else if n.contains("apple watch") {
            Brand::Apple
        } else {
            Brand::Generic
        }
    }

    fn needs_activation(&self) -> bool {
        matches!(self, Brand::Xiaomi | Brand::Huawei | Brand::Amazfit)
    }

    fn keepalive_secs(&self) -> Option<u64> {
        match self {
            Brand::Xiaomi => Some(12),
            Brand::Huawei => Some(15),
            Brand::Amazfit => Some(20),
            _ => None,
        }
    }
}

// ── 心率数据解析 ──────────────────────────────────────────────────────────────

fn parse_hr(data: &[u8]) -> Option<u16> {
    if data.len() < 2 {
        return None;
    }
    let flags = data[0];
    let hr_u16 = flags & 0x01 != 0;

    if hr_u16 {
        if data.len() < 3 {
            return None;
        }
        Some(u16::from_le_bytes([data[1], data[2]]))
    } else {
        Some(data[1] as u16)
    }
}

// ── GATT 写入辅助 ─────────────────────────────────────────────────────────────

async fn gatt_write(ch: &GattCharacteristic, data: &[u8]) -> windows::core::Result<()> {
    let writer = DataWriter::new()?;
    writer.WriteBytes(data)?;
    let buf = writer.DetachBuffer()?;
    ch.WriteValueWithOptionAsync(&buf, GattWriteOption::WriteWithoutResponse)?
        .get()?;
    Ok(())
}

// ── MAC 地址格式化 ────────────────────────────────────────────────────────────

fn mac_str(addr: u64) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        (addr >> 40) as u8,
        (addr >> 32) as u8,
        (addr >> 24) as u8,
        (addr >> 16) as u8,
        (addr >> 8) as u8,
        addr as u8,
    )
}

// ── 对外类型 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct BleDeviceInfo {
    pub address: u64,
    pub address_str: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum BleConnectionStatus {
    Disconnected,
    Scanning,
    Connecting,
    Connected,
}

// ── 全局状态 ─────────────────────────────────────────────────────────────────

static CONNECTION_STATUS: AtomicBool = AtomicBool::new(false);
static CACHED_HEART_RATE: Mutex<Option<u16>> = Mutex::new(None);
static CACHED_DEVICE_NAME: Mutex<Option<String>> = Mutex::new(None);
static CONNECTION_STATE: Mutex<BleConnectionStatus> = Mutex::new(BleConnectionStatus::Disconnected);
/// 用于通知后台连接任务停止
static STOP_FLAG: AtomicBool = AtomicBool::new(false);
/// 广播监听任务是否运行
static ADVERT_LISTEN_RUNNING: AtomicBool = AtomicBool::new(false);
/// 广播监听停止标记
static ADVERT_LISTEN_STOP: AtomicBool = AtomicBool::new(false);

// ── 公共接口 ─────────────────────────────────────────────────────────────────

pub fn get_cached_heart_rate() -> Option<u16> {
    *CACHED_HEART_RATE.lock().unwrap()
}

pub fn get_heart_rate_device_name() -> Option<String> {
    CACHED_DEVICE_NAME.lock().unwrap().clone()
}

pub fn get_connection_status() -> BleConnectionStatus {
    *CONNECTION_STATE.lock().unwrap()
}

pub fn disconnect() {
    STOP_FLAG.store(true, Ordering::SeqCst);
    ADVERT_LISTEN_STOP.store(true, Ordering::SeqCst);
    CONNECTION_STATUS.store(false, Ordering::SeqCst);
    *CONNECTION_STATE.lock().unwrap() = BleConnectionStatus::Disconnected;
    *CACHED_HEART_RATE.lock().unwrap() = None;
    *CACHED_DEVICE_NAME.lock().unwrap() = None;
}

pub fn cleanup() {
    disconnect();
}

// ── BLE 扫描 ─────────────────────────────────────────────────────────────────

pub async fn start_scan() -> Result<Vec<BleDeviceInfo>, String> {
    *CONNECTION_STATE.lock().unwrap() = BleConnectionStatus::Scanning;

    let mut seen: HashSet<u64> = HashSet::new();
    let mut devices: Vec<BleDeviceInfo> = Vec::new();

    // 方式1: 枚举系统已知BLE设备
    log::info!("[心率] 枚举系统BLE设备...");
    for selector in [
        BluetoothLEDevice::GetDeviceSelector(),
        BluetoothLEDevice::GetDeviceSelectorFromPairingState(true),
        BluetoothLEDevice::GetDeviceSelectorFromPairingState(false),
    ]
    .iter()
    {
        if let Ok(selector) = selector {
            if let Ok(async_op) = DeviceInformation::FindAllAsyncAqsFilter(selector) {
                if let Ok(info) = async_op.get() {
                    let size = info.Size().unwrap_or(0);
                    for i in 0..size {
                        if let Ok(item) = info.GetAt(i) {
                            let name = item.Name().unwrap_or_default().to_string();
                            let id = item.Id().unwrap_or_default().to_string();
                            if let Some(address) = parse_address_from_id(&id) {
                                if !name.is_empty() && seen.insert(address) {
                                    devices.push(BleDeviceInfo {
                                        address,
                                        address_str: mac_str(address),
                                        name: name.clone(),
                                    });
                                    log::info!("[心率] 枚举设备: {} | {}", name, mac_str(address));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 方式2: BLE广播扫描（同时用 Active + Passive 两轮扫）
    log::info!("[心率] 启动BLE广播扫描(先Active再Passive)...");
    let (tx, mut rx) = mpsc::unbounded_channel::<(u64, String)>();
    let seen_arc: Arc<Mutex<HashSet<u64>>> = Arc::new(Mutex::new(seen));

    // Arc延长扫描器生命周期，防止回调执行前被drop
    let watcher = Arc::new(match BluetoothLEAdvertisementWatcher::new() {
        Ok(w) => w,
        Err(e) => {
            log::error!("[心率] 创建扫描器失败: {}", e);
            *CONNECTION_STATE.lock().unwrap() = BleConnectionStatus::Disconnected;
            return Ok(devices);
        }
    });

    let seen_arc_clone = seen_arc.clone();
    let tx_clone = tx.clone();

    // 注册广播回调 — 记录所有广播用于调试
    if let Err(e) = watcher.Received(&TypedEventHandler::<
        BluetoothLEAdvertisementWatcher,
        BluetoothLEAdvertisementReceivedEventArgs,
    >::new(move |_, args| {
        let args = match args { Some(a) => a, None => return Ok(()) };
        let address = match args.BluetoothAddress() { Ok(a) => a, Err(_) => return Ok(()) };
        let adv = match args.Advertisement() { Ok(a) => a, Err(_) => return Ok(()) };
        let name = adv.LocalName().unwrap_or_default().to_string();
        let rssi = args.RawSignalStrengthInDBm().unwrap_or(0);

        // 收集 UUID 列表用于调试
        let uuid_list: Vec<String> = {
            let mut list = Vec::new();
            if let Ok(uuids) = adv.ServiceUuids() {
                for j in 0..uuids.Size().unwrap_or(0) {
                    if let Ok(uuid) = uuids.GetAt(j) {
                        list.push(format!("{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
                            uuid.data1, uuid.data2, uuid.data3,
                            uuid.data4[0], uuid.data4[1],
                            uuid.data4[2], uuid.data4[3], uuid.data4[4], uuid.data4[5], uuid.data4[6], uuid.data4[7]));
                    }
                }
            }
            list
        };

        // 收集 ManufacturerData 原始字节用于调试
        let mfg_hex: Vec<String> = {
            let mut result = Vec::new();
            if let Ok(data_list) = adv.ManufacturerData() {
                for j in 0..data_list.Size().unwrap_or(0) {
                    if let Ok(item) = data_list.GetAt(j) {
                        if let Ok(stream) = item.Data() {
                            if let Ok(reader) = DataReader::FromBuffer(&stream) {
                                let len = reader.UnconsumedBufferLength().unwrap_or(0) as usize;
                                let mut buf = vec![0u8; len];
                                let _ = reader.ReadBytes(&mut buf);
                                result.push(buf.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" "));
                            }
                        }
                    }
                }
            }
            result
        };

        let has_hr = uuid_list.iter().any(|u| u.starts_with("0000180D"));

        // 记录每一个广播包（调试用）
        log::info!(
            "[心率] RAW广播 {} {}dBm name=\"{}\" UUIDs:{:?} MfgData:{}",
            mac_str(address), rssi, name, uuid_list,
            mfg_hex.iter().map(|h| format!("[{}]", h)).collect::<Vec<_>>().join("")
        );

        // 收录条件：有名称 或 有心率服务UUID
        if name.is_empty() && !has_hr {
            return Ok(());
        }

        let display_name = if name.is_empty() { "心率设备".to_string() } else { name.clone() };

        let mut seen = seen_arc_clone.lock().unwrap();
        if !seen.contains(&address) {
            seen.insert(address);
            log::info!("[心率] ✓ 收录: {} | {}", display_name, mac_str(address));
            let _ = tx_clone.send((address, display_name));
        }
        Ok(())
    })) {
        log::error!("[心率] 注册广播回调失败: {}", e);
    }

    // 第1轮: Active 模式扫描 6 秒
    let _ = watcher.SetScanningMode(BluetoothLEScanningMode::Active);
    log::info!("[心率] → Active 扫描 6s...");
    if watcher.Start().is_err() {
        log::error!("[心率] Active 扫描启动失败");
    } else {
        let _ = time::sleep(Duration::from_secs(6)).await;
        let _ = watcher.Stop();
        log::info!("[心率] → Active 扫描结束");
    }

    // 第2轮: Passive 模式扫描 6 秒
    let _ = watcher.SetScanningMode(BluetoothLEScanningMode::Passive);
    log::info!("[心率] → Passive 扫描 6s...");
    if watcher.Start().is_err() {
        log::error!("[心率] Passive 扫描启动失败");
    } else {
        let _ = time::sleep(Duration::from_secs(6)).await;
        let _ = watcher.Stop();
        log::info!("[心率] → Passive 扫描结束");
    }

    // 收集结果
    while let Ok((address, name)) = rx.try_recv() {
        devices.push(BleDeviceInfo {
            address,
            address_str: mac_str(address),
            name,
        });
    }

    *CONNECTION_STATE.lock().unwrap() = BleConnectionStatus::Disconnected;
    log::info!("[心率] 扫描结束，总计发现 {} 台设备", devices.len());

    Ok(devices)
}

/// 从 Windows 设备 ID 中解析蓝牙 MAC 地址
/// 设备 ID 格式类似: BluetoothLE#BluetoothLE60:ab:67:5a:de:40-xx:xx:xx:xx:xx:xx
fn parse_address_from_id(id: &str) -> Option<u64> {
    let mut mac_candidates = Vec::new();
    let mut buf = String::new();

    for c in id.to_ascii_lowercase().chars() {
        if c.is_ascii_hexdigit() {
            buf.push(c);
        } else {
            if buf.len() == 2 {
                mac_candidates.push(buf.clone());
            }
            buf.clear();
        }
    }
    if buf.len() == 2 {
        mac_candidates.push(buf);
    }

    // 取连续6组两位十六进制 = MAC
    for win in mac_candidates.windows(6) {
        let mut addr = 0u64;
        let mut valid = true;
        for (idx, hex) in win.iter().enumerate() {
            match u8::from_str_radix(hex, 16) {
                Ok(b) => {
                    let shift = 40 - (idx * 8);
                    addr |= (b as u64) << shift;
                }
                Err(_) => {
                    valid = false;
                    break;
                }
            }
        }
        if valid {
            return Some(addr);
        }
    }
    None
}

// ── BLE 连接并订阅心率 ───────────────────────────────────────────────────────

pub async fn connect_device(address: u64) -> Result<(), String> {
    // 先断开已有连接
    if CONNECTION_STATUS.load(Ordering::SeqCst) {
        disconnect();
        time::sleep(Duration::from_millis(500)).await;
    }

    STOP_FLAG.store(false, Ordering::SeqCst);
    *CONNECTION_STATE.lock().unwrap() = BleConnectionStatus::Connecting;

    let device = BluetoothLEDevice::FromBluetoothAddressAsync(address)
        .map_err(|e| format!("创建 BLE 设备失败: {}", e))?
        .get()
        .map_err(|e| format!("等待 BLE 设备失败: {}", e))?;

    let name = device
        .Name()
        .unwrap_or_default()
        .to_string();
    let brand = Brand::detect(&name);

    // 获取 Heart Rate Service (0x180D)
    let (_hr_svc, hr_char, ctrl_char) = {
        let svc_res = device
            .GetGattServicesForUuidAsync(HR_SERVICE)
            .map_err(|e| format!("获取 GATT 服务失败: {}", e))?
            .get()
            .map_err(|e| format!("等待 GATT 服务失败: {}", e))?;

        if svc_res.Status().map_err(|e| format!("服务状态错误: {}", e))?
            != GattCommunicationStatus::Success
        {
            return Err("无法访问 Heart Rate Service — 设备是否已在系统蓝牙中配对？".to_string());
        }
        let services = svc_res
            .Services()
            .map_err(|e| format!("获取服务列表失败: {}", e))?;
        if services.Size().map_err(|e| format!("服务列表大小错误: {}", e))? == 0 {
            return Err("设备不暴露 Heart Rate Service".to_string());
        }
        let hr_svc = services.GetAt(0).map_err(|e| format!("获取服务失败: {}", e))?;

        // 获取 HR Measurement (0x2A37)
        let meas_res = hr_svc
            .GetCharacteristicsForUuidAsync(HR_MEASUREMENT)
            .map_err(|e| format!("获取特征失败: {}", e))?
            .get()
            .map_err(|e| format!("等待特征失败: {}", e))?;
        if meas_res.Status().map_err(|e| format!("特征状态错误: {}", e))?
            != GattCommunicationStatus::Success
        {
            return Err("无法访问 HR Measurement 特征".to_string());
        }
        let hr_char = meas_res
            .Characteristics()
            .map_err(|e| format!("获取特征列表失败: {}", e))?
            .GetAt(0)
            .map_err(|e| format!("获取特征失败: {}", e))?;

        // 获取 HR Control Point (0x2A39)，可选
        let ctrl_char: Option<GattCharacteristic> = {
            match hr_svc
                .GetCharacteristicsForUuidAsync(HR_CONTROL_POINT)
                .map_err(|e| format!("获取控制点特征失败: {}", e))?
                .get()
            {
                Ok(res)
                    if res.Status().map_err(|e| format!("控制点状态错误: {}", e))?
                        == GattCommunicationStatus::Success =>
                {
                    let chars = res
                        .Characteristics()
                        .map_err(|e| format!("获取控制点列表失败: {}", e))?;
                    if chars.Size().map_err(|e| format!("控制点大小错误: {}", e))? > 0 {
                        Some(chars.GetAt(0).map_err(|e| format!("获取控制点失败: {}", e))?)
                    } else {
                        None
                    }
                }
                _ => None,
            }
        };

        (hr_svc, hr_char, ctrl_char)
    }; // drop services, svc_res, meas_res, chars (non-Send types)

    // 品牌特定激活命令
    if brand.needs_activation() {
        if let Some(ref ctrl) = ctrl_char {
            match brand {
                Brand::Xiaomi => {
                    gatt_write(ctrl, &[0x15, 0x02, 0x00])
                        .await
                        .map_err(|e| format!("停止旧测量会话失败: {}", e))?;
                    time::sleep(Duration::from_millis(200)).await;
                    gatt_write(ctrl, &[0x15, 0x01, 0x01])
                        .await
                        .map_err(|e| format!("激活心率测量失败: {}", e))?;
                }
                Brand::Huawei => {
                    gatt_write(ctrl, &[0x01])
                        .await
                        .map_err(|e| format!("激活心率测量失败: {}", e))?;
                }
                Brand::Amazfit => {
                    gatt_write(ctrl, &[0x01, 0x00])
                        .await
                        .map_err(|e| format!("激活心率测量失败: {}", e))?;
                }
                _ => {}
            }
            time::sleep(Duration::from_millis(300)).await;
        }
    }

    // Keep-alive 任务
    if let (Some(secs), Some(ctrl)) = (brand.keepalive_secs(), ctrl_char.clone()) {
        let brand_ka = brand.clone();
        tokio::spawn(async move {
            loop {
                time::sleep(Duration::from_secs(secs)).await;
                if STOP_FLAG.load(Ordering::SeqCst) {
                    break;
                }
                let _ = match brand_ka {
                    Brand::Xiaomi => gatt_write(&ctrl, &[0x16]).await,
                    Brand::Huawei => gatt_write(&ctrl, &[0x01]).await,
                    Brand::Amazfit => gatt_write(&ctrl, &[0x01, 0x00]).await,
                    _ => Ok(()),
                };
            }
        });
    }

    // 注册 ValueChanged 回调
    let device_name = name.clone();
    let _token = hr_char
        .ValueChanged(&TypedEventHandler::<
            GattCharacteristic,
            GattValueChangedEventArgs,
        >::new(move |_, args| {
            let args = match args {
                Some(a) => a,
                None => return Ok(()),
            };
            let buf = args.CharacteristicValue()?;
            let reader = DataReader::FromBuffer(&buf)?;
            let len = reader.UnconsumedBufferLength()? as usize;
            let mut raw = vec![0u8; len];
            reader.ReadBytes(&mut raw)?;

            if let Some(bpm) = parse_hr(&raw) {
                *CACHED_HEART_RATE.lock().unwrap() = Some(bpm);
            }
            Ok(())
        }))
        .map_err(|e| format!("注册回调失败: {}", e))?;

    // 启用 Notify
    hr_char
        .WriteClientCharacteristicConfigurationDescriptorAsync(
            GattClientCharacteristicConfigurationDescriptorValue::Notify,
        )
        .map_err(|e| format!("启用通知失败: {}", e))?
        .get()
        .map_err(|e| format!("等待通知启用失败: {}", e))?;

    CONNECTION_STATUS.store(true, Ordering::SeqCst);
    *CONNECTION_STATE.lock().unwrap() = BleConnectionStatus::Connected;
    *CACHED_DEVICE_NAME.lock().unwrap() = Some(device_name.clone());

    // 后台保持连接，监控连接状态
    tokio::spawn(async move {
        loop {
            if STOP_FLAG.load(Ordering::SeqCst) {
                break;
            }
            time::sleep(Duration::from_secs(2)).await;
            match device.ConnectionStatus() {
                Ok(status) if status != BluetoothConnectionStatus::Connected => {
                    break;
                }
                Err(_) => break,
                _ => {}
            }
        }
        // 连接断开，清理状态
        CONNECTION_STATUS.store(false, Ordering::SeqCst);
        *CONNECTION_STATE.lock().unwrap() = BleConnectionStatus::Disconnected;
        *CACHED_HEART_RATE.lock().unwrap() = None;
        *CACHED_DEVICE_NAME.lock().unwrap() = None;
    });

    Ok(())
}

/// 启动 纯广播心率监听（小米手环心率广播专用，免连接）
pub async fn start_advert_heartrate_listen(target_addr: u64) -> Result<(), String> {
    if ADVERT_LISTEN_RUNNING.load(Ordering::SeqCst) {
        return Err("广播监听已在运行".to_string());
    }
    disconnect();

    ADVERT_LISTEN_STOP.store(false, Ordering::SeqCst);
    ADVERT_LISTEN_RUNNING.store(true, Ordering::SeqCst);
    *CONNECTION_STATE.lock().unwrap() = BleConnectionStatus::Connected;

    let watcher = Arc::new(
        BluetoothLEAdvertisementWatcher::new()
            .map_err(|e| format!("创建监听扫描器失败: {}", e))?,
    );
    let _ = watcher.SetScanningMode(BluetoothLEScanningMode::Passive);

    let watcher_clone = watcher.clone();
    let target = target_addr;

    // 广播回调：解析广播数据中的心率
    let _ = watcher.Received(&TypedEventHandler::<
        BluetoothLEAdvertisementWatcher,
        BluetoothLEAdvertisementReceivedEventArgs,
    >::new(move |_, args: &Option<BluetoothLEAdvertisementReceivedEventArgs>| {
        if ADVERT_LISTEN_STOP.load(Ordering::SeqCst) {
            return Ok(());
        }
        let args = match args { Some(a) => a, None => return Ok(()) };
        let addr = match args.BluetoothAddress() { Ok(a) => a, Err(_) => return Ok(()) };

        // 只监听目标设备
        if addr != target {
            return Ok(());
        }

        // 解析广播负载数据
        let adv = match args.Advertisement() { Ok(a) => a, Err(_) => return Ok(()) };

        let mut hr_payload = Vec::new();

        // 方式1: 尝试从 DataSections 中找心率服务数据
        if let Ok(sections) = adv.DataSections() {
            let size = sections.Size().unwrap_or(0);
            for i in 0..size {
                if let Ok(section) = sections.GetAt(i) {
                    if let Ok(data) = section.Data() {
                        if let Ok(reader) = DataReader::FromBuffer(&data) {
                            let len = reader.UnconsumedBufferLength().unwrap_or(0) as usize;
                            let mut temp = vec![0u8; len];
                            let _ = reader.ReadBytes(&mut temp);
                            // 尝试解析这段数据作为标准心率格式
                            if parse_hr(&temp).is_some() {
                                hr_payload = temp;
                                break;
                            }
                        }
                    }
                }
            }
        }

        // 方式2: 从 ManufacturerData 中提取
        if hr_payload.is_empty() {
            if let Ok(data_list) = adv.ManufacturerData() {
                let mut buf = Vec::new();
                let size = data_list.Size().unwrap_or(0);
                for i in 0..size {
                    if let Ok(item) = data_list.GetAt(i) {
                        if let Ok(stream) = item.Data() {
                            if let Ok(reader) = DataReader::FromBuffer(&stream) {
                                let len = reader.UnconsumedBufferLength().unwrap_or(0) as usize;
                                let mut temp = vec![0u8; len];
                                let _ = reader.ReadBytes(&mut temp);
                                buf.extend(temp);
                            }
                        }
                    }
                }
                // 小米厂商数据可能包含心率，尝试多种偏移量
                if !buf.is_empty() {
                    for offset in 0..buf.len().min(8) {
                        let slice = &buf[offset..];
                        if let Some(bpm) = parse_hr(slice) {
                            *CACHED_HEART_RATE.lock().unwrap() = Some(bpm);
                            return Ok(());
                        }
                    }
                }
            }
        }

        // 标准心率广播解析（复用现有 parse_hr）
        if let Some(bpm) = parse_hr(&hr_payload) {
            *CACHED_HEART_RATE.lock().unwrap() = Some(bpm);
        }
        Ok(())
    }));

    watcher.Start().map_err(|e| format!("启动广播监听失败: {}", e))?;

    // 后台保活监听
    tokio::spawn(async move {
        while !ADVERT_LISTEN_STOP.load(Ordering::SeqCst) {
            time::sleep(Duration::from_secs(1)).await;
        }
        // 停止监听，清理资源
        let _ = watcher_clone.Stop();
        ADVERT_LISTEN_RUNNING.store(false, Ordering::SeqCst);
        *CONNECTION_STATE.lock().unwrap() = BleConnectionStatus::Disconnected;
        *CACHED_HEART_RATE.lock().unwrap() = None;
    });

    Ok(())
}

/// 停止纯广播心率监听
pub fn stop_advert_heartrate_listen() {
    ADVERT_LISTEN_STOP.store(true, Ordering::SeqCst);
}

// ── Tauri 命令 ───────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn scan_ble_devices() -> Result<Vec<BleDeviceInfo>, String> {
    start_scan().await
}

#[tauri::command]
pub async fn connect_ble_device(address: u64) -> Result<(), String> {
    connect_device(address).await
}

#[tauri::command]
pub async fn disconnect_ble_device() -> Result<(), String> {
    disconnect();
    Ok(())
}

#[derive(Serialize)]
pub struct HeartRateData {
    pub heart_rate: Option<u16>,
    pub device_name: Option<String>,
    pub connection_status: BleConnectionStatus,
}

#[tauri::command]
pub async fn get_heart_rate_data() -> Result<HeartRateData, String> {
    Ok(HeartRateData {
        heart_rate: get_cached_heart_rate(),
        device_name: get_heart_rate_device_name(),
        connection_status: get_connection_status(),
    })
}

#[tauri::command]
pub async fn get_ble_connection_status() -> Result<BleConnectionStatus, String> {
    Ok(get_connection_status())
}

#[tauri::command]
pub async fn start_advert_hr_listen(address: u64) -> Result<(), String> {
    start_advert_heartrate_listen(address).await
}

#[tauri::command]
pub async fn stop_advert_hr_listen() -> Result<(), String> {
    stop_advert_heartrate_listen();
    Ok(())
}