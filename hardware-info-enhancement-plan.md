# 硬件信息增强方案 — WMI 详细规格采集

## 一、方案概述

### 1.1 目标

在现有硬件信息采集框架基础上，利用 WMI 获取更丰富的硬件规格信息，并通过 **点击卡片展开详情** 的方式呈现。仅展示型号/规格/参数，不展示状态/占用/温度等实时数据。

### 1.2 核心交互变更

| 变更 | 说明 |
|------|------|
| 已有 7 类卡片 | **点击弹出详情浮层**，展示完整规格参数 |
| 新增 **显示器** 卡片 | 第二个独立 DetailCard，显示显示器名称、分辨率、刷新率等 |
| 删除 BIOS/系统/机箱 独立卡片 | 不新增独立卡片，其信息合并入"主板"和"处理器"的展开详情中 |
| 状态/占用/温度 | 仅展示规格，不展示实时监控数据 |

### 1.3 交互流程

```
┌─────────────────────────────────────────┐
│  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐     │
│  │ CPU  │ │ GPU  │ │ RAM  │ │Storage│     │
│  │ xx%  │ │ xx%  │ │ xx%  │ │ xx%  │     │
│  └──╋───┘ └──╋───┘ └──╋───┘ └──╋───┘     │
│     ┃        ┃        ┃        ┃          │
│  ┌──╋────────╋────────╋────────╋──────┐   │
│  │ ┃ 处理   ┃ 显卡   ┃ 内存   ┃ 磁盘 │   │
│  │ ┃ 主板   ┃ 声卡   ┃ 网卡   ┃ 显示器│   │
│  └──╋────────╋────────╋────────╋──────┘   │
│     ┃                                        │
│     ┃ 点击任意卡片                          │
│     ▼                                        │
│  ┌──────────────────────┐                    │
│  │  展开详情浮层/侧栏    │                    │
│  │  ─────────────────── │                    │
│  │  处理器 (Intel Core  │                    │
│  │  i7-14700K)          │                    │
│  │  • 制造商: Intel      │                    │
│  │  • 架构: x64          │                    │
│  │  • 插槽: LGA1700     │                    │
│  │  • 核心: 20 (8P+12E) │                    │
│  │  • 线程: 28          │                    │
│  │  • 基础频率: 3.4 GHz │                    │
│  │  • 最大睿频: 5.6 GHz │                    │
│  │  • L2 缓存: 28 MB    │                    │
│  │  • L3 缓存: 33 MB    │                    │
│  │  • 步进/修订: C0     │                    │
│  │  • 节: 关闭         │                    │
│  └──────────────────────┘                    │
└─────────────────────────────────────────┘
```

---

## 二、数据来源

| 数据源 | 采集方式 | 用途 |
|--------|---------|------|
| **WMI (PowerShell)** | 现有 `run_powershell<T>()` 框架 | 静态规格信息（CPU/GPU/内存/主板/存储/声卡/网卡/显示器/BIOS） |
| **NVML (nvidia-ml)** | 现有 `nvml_wrapper` crate | NVIDIA GPU 名称、显存、驱动版本（规格部分） |
| **nvidia-smi** | 现有命令行降级方案 | NVML 降级 |
| **LHML (NexBoxMonitor 子进程)** | 现有管道 JSON 通信 | 已有传感器中可用于辨识硬件规格的数据（如 CPU 名称、GPU 名称、主板型号、内存 SPD 信息等） |
| **sysinfo crate** | 现有 Rust 库 | 操作系统版本、CPU 核心数 |

---

## 三、WMI 扩展字段方案

### 3.1 CPU — 扩展 Win32_Processor 查询

**当前查询属性：**
```
Name, NumberOfCores, NumberOfLogicalProcessors, MaxClockSpeed, L3CacheSize, LoadPercentage
```

**扩展后查询属性：**
```
Name, NumberOfCores, NumberOfLogicalProcessors, MaxClockSpeed, L3CacheSize, LoadPercentage,
Manufacturer, Architecture, SocketDesignation, L2CacheSize, L2CacheSpeed,
L3CacheSpeed, CurrentClockSpeed, ExtClock, ProcessorId,
Family, Stepping, Revision, NumberOfEnabledCore, VoltageCaps
```

**CpuInfo 结构体新增字段：**

| 新字段 | 类型 | 说明 |
|--------|------|------|
| `manufacturer` | `String` | 制造商（Intel / AMD） |
| `architecture` | `String` | 架构（x64 / ARM64 / x86） |
| `socket` | `String` | 插槽类型（LGA1700 / AM5） |
| `l2_cache_size` | `u32` | L2 缓存大小（KB） |
| `l2_cache_speed` | `Option<u32>` | L2 缓存速度 |
| `l3_cache_speed` | `Option<u32>` | L3 缓存速度 |
| `current_clock_speed` | `Option<u32>` | 当前频率（MHz） |
| `ext_clock` | `Option<u32>` | 外频/总线频率（MHz） |
| `processor_id` | `String` | 处理器 ID |
| `family` | `u32` | 处理器系列 |
| `stepping` | `String` | 步进号 |
| `revision` | `String` | 修订版本 |
| `enabled_cores` | `Option<u32>` | 已启用核心数 |
| `voltage_caps` | `Option<String>` | 电压能力 |

### 3.2 GPU — 扩展 Win32_VideoController 查询

**当前查询属性：**
```
Name, DriverVersion, AdapterRAM
```

**扩展后查询属性：**
```
Name, DriverVersion, AdapterRAM,
VideoProcessor, AdapterCompatibility, DriverDate,
InstalledDisplayDrivers, VideoModeDescription,
CurrentHorizontalResolution, CurrentVerticalResolution, CurrentRefreshRate,
DeviceID, PNPDeviceID, Status, InfFilename, VideoArchitecture, VideoMemoryType
```

**GpuInfo 结构体新增字段：**

| 新字段 | 类型 | 说明 |
|--------|------|------|
| `video_processor` | `String` | GPU 核心架构名（如 "GeForce RTX 4080"） |
| `adapter_compatibility` | `String` | 适配器兼容性（如 "NVIDIA"） |
| `driver_date` | `String` | 驱动发布日期 |
| `installed_drivers` | `String` | 已安装驱动文件 |
| `video_mode` | `String` | 当前视频模式描述 |
| `resolution_width` | `Option<u32>` | 当前水平分辨率 |
| `resolution_height` | `Option<u32>` | 当前垂直分辨率 |
| `refresh_rate` | `Option<u32>` | 当前刷新率 |
| `device_id` | `String` | 设备 ID |
| `pnp_device_id` | `String` | PNP 设备 ID |
| `status` | `String` | 设备状态 |
| `inf_filename` | `String` | INF 文件名 |
| `video_architecture` | `Option<String>` | 视频架构类型 |
| `video_memory_type` | `Option<String>` | 显存类型 |

### 3.3 主板 — 扩展 Win32_BaseBoard + Win32_ComputerSystem

**当前：** 仅 `Win32_BaseBoard` 的 `Manufacturer, Product`

**扩展为联合查询（主板 + 系统 + BIOS + 机箱）：**

```
Win32_BaseBoard:    Manufacturer, Product, SerialNumber, Version
Win32_ComputerSystem: Manufacturer, Model, SystemType, TotalPhysicalMemory, Domain, NumberOfProcessors
Win32_BIOS:          SMBIOSBIOSVersion, Manufacturer, Name, ReleaseDate, SerialNumber
Win32_SystemEnclosure: ChassisTypes, Manufacturer, Version, SerialNumber
```

**MotherboardInfo 结构体（从 `String` 升级为结构体）：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `product` | `String` | 主板型号（现有） |
| `manufacturer` | `String` | 主板制造商 |
| `serial_number` | `String` | 主板序列号 |
| `version` | `String` | 主板版本 |
| `bios_vendor` | `String` | BIOS 制造商 |
| `bios_version` | `String` | BIOS 版本 |
| `bios_release_date` | `String` | BIOS 发布日期 |
| `system_manufacturer` | `String` | 系统制造商（品牌机厂商） |
| `system_model` | `String` | 系统型号 |
| `system_type` | `String` | 系统类型（x64/ARM） |
| `chassis_type` | `String` | 机箱类型（Desktop/Notebook/Server） |

### 3.4 内存 — 扩展 Win32_PhysicalMemory 查询

**当前查询属性：**
```
Manufacturer, PartNumber, Capacity, Speed, BankLabel
```

**扩展后查询属性：**
```
Manufacturer, PartNumber, Capacity, Speed, BankLabel,
FormFactor, MemoryType, ConfiguredClockSpeed, ConfiguredVoltage,
DataWidth, TotalWidth, TypeDetail, SerialNumber, Tag
```

**MemoryInfo 结构体新增字段：**

| 新字段 | 类型 | 说明 |
|--------|------|------|
| `form_factor` | `String` | 外形规格（DIMM / SODIMM / RDIMM） |
| `memory_type` | `String` | 内存类型（DDR4 / DDR5 / DDR3） |
| `configured_clock_speed` | `Option<u32>` | 配置时钟速度 |
| `configured_voltage` | `Option<u32>` | 配置电压（mV） |
| `data_width` | `Option<u32>` | 数据宽度（位） |
| `total_width` | `Option<u32>` | 总宽度（位） |
| `serial_number` | `String` | 序列号 |
| `type_detail` | `String` | 类型详情 |

### 3.5 存储 — 结构体升级 + WMI 字段扩展

**当前：** `disk: Vec<String>` — 仅有 `Model (SizeGB)` 字符串

**改为：** `disk: Vec<DiskDetailInfo>`

**DiskDetailInfo 结构体：**

| 字段 | 类型 | 来源 |
|------|------|------|
| `model` | `String` | Win32_DiskDrive.Model |
| `size_gb` | `f64` | Win32_DiskDrive.Size |
| `interface_type` | `String` | Win32_DiskDrive.InterfaceType |
| `serial_number` | `String` | Win32_DiskDrive.SerialNumber |
| `firmware_revision` | `String` | Win32_DiskDrive.FirmwareRevision |
| `media_type` | `String` | Win32_DiskDrive.MediaType |
| `bytes_per_sector` | `Option<u32>` | Win32_DiskDrive.BytesPerSector |
| `partitions` | `u32` | Win32_DiskDrive.Partitions |
| `status` | `String` | Win32_DiskDrive.Status |
| `is_ssd` | `bool` | 根据 MediaType 判断 |

### 3.6 网卡 — 扩展 Win32_NetworkAdapter 查询

**当前查询属性：**
```
Name, Manufacturer, AdapterType, MACAddress, Speed
```

**扩展后查询属性：**
```
Name, Manufacturer, AdapterType, MACAddress, Speed,
NetConnectionID, ServiceName, Index, MaxSpeed, NetEnabled, GUID
```

**NetworkCardInfo 结构体新增字段：**

| 新字段 | 类型 | 说明 |
|--------|------|------|
| `connection_name` | `String` | 网络连接名称（如 "以太网"、"Wi-Fi"） |
| `service_name` | `String` | 服务名 |
| `index` | `u32` | 索引号 |
| `max_speed` | `Option<u64>` | 最大速度（bps） |
| `guid` | `String` | 适配器 GUID |

### 3.7 新增显示器信息 — Win32_DesktopMonitor

**MonitorInfo 结构体：**

| 字段 | 类型 | WMI 类 / 属性 |
|------|------|--------------|
| `name` | `String` | Win32_DesktopMonitor.Name |
| `manufacturer` | `String` | Win32_DesktopMonitor.MonitorManufacturerName |
| `screen_width` | `Option<u32>` | Win32_DesktopMonitor.ScreenWidth |
| `screen_height` | `Option<u32>` | Win32_DesktopMonitor.ScreenHeight |
| `refresh_rate` | `Option<u32>` | Win32_DesktopMonitor.DisplayFrequency |
| `pnp_device_id` | `String` | Win32_DesktopMonitor.PNPDeviceID |
| `status` | `String` | Win32_DesktopMonitor.Status |
| `availability` | `Option<u32>` | Win32_DesktopMonitor.Availability |

> **补充：** 也可通过 `Win32_DisplayConfiguration` 或 `Win32_DisplayDevice` 获取当前连接的显示器名称。

### 3.8 声卡 — 扩展 Win32_SoundDevice

**当前：** `Name, Manufacturer`

**扩展：** 仅增加 `Status`, `DeviceID`, `PNPDeviceID` — 信息已较完整，补充用于辨识。

### 3.9 新增硬件类别汇总到 HardwareInfo

```rust
pub struct HardwareInfo {
    pub cpu: CpuInfo,
    pub gpu: Vec<GpuInfo>,
    pub memory: Vec<MemoryInfo>,
    pub motherboard: MotherboardInfo,    // String → struct
    pub disk: Vec<DiskDetailInfo>,       // Vec<String> → Vec<struct>
    pub sound_card: Vec<SoundCardInfo>,
    pub network_card: Vec<NetworkCardInfo>,
    pub monitor: Vec<MonitorInfo>,       // 🆕 新增
}
```

> **注意**：前端保持 `get_hardware` 命令不变（无需新命令），仅扩展数据。

---

## 四、LHML 补充方案

LiberHardwareMonitor 通过 `NexBoxMonitor.exe` 子进程管道通信，返回 `SensorReading[]`。其中可用于补充规格信息的数据：

| 硬件 | LHML 传感器 | 可补充信息 |
|------|------------|-----------|
| CPU | `CPU Core #` / `CPU Package` 等硬件名称 | 验证 CPU 型号全称 |
| GPU | `GPU Core` 硬件节点，含名称 | 验证 GPU 型号 |
| 主板 | `Motherboard` 硬件节点，含 `Identifier` | 主板厂商/型号全称 |
| 内存 | `Memory` 节点传感器 | 内存 SPD 信息（容量、频率） |
| 存储 | `Storage` 节点，含 `Model`、`Name` | 磁盘型号确认 |

> LHML 数据已在 `overlay_panel.rs` 的后台轮询器中持续获取，可以直接利用现有缓存。

---

## 五、前端实现方案

### 5.1 数据类型扩展（src/lib/hardware.ts）

```typescript
export interface MotherboardInfo {
  product: string;
  manufacturer: string;
  serial_number: string;
  version: string;
  bios_vendor: string;
  bios_version: string;
  bios_release_date: string;
  system_manufacturer: string;
  system_model: string;
  system_type: string;
  chassis_type: string;
}

export interface DiskDetailInfo {
  model: string;
  size_gb: number;
  interface_type: string;
  serial_number: string;
  firmware_revision: string;
  media_type: string;
  bytes_per_sector: number | null;
  partitions: number;
  status: string;
  is_ssd: boolean;
}

export interface MonitorInfo {
  name: string;
  manufacturer: string;
  screen_width: number | null;
  screen_height: number | null;
  refresh_rate: number | null;
  pnp_device_id: string;
  status: string;
}

// 其余接口同步扩展字段...
```

### 5.2 交互设计 — 点击展开详情浮层

每张 DetailCard 增加点击响应，点击后打开 Modal / Drawer / Popover 显示完整规格。

```tsx
// DetailCard 新增 onClick 属性
<DetailCard
  title={t("hardware.processor")}
  icon={Cpu}
  info={cpuDisplayInfo}      // 卡片上只显示基本信息
  type="cpu"
  onClick={() => setExpandedCard("cpu")}
/>

// 展开后弹出的详情浮层
<HardwareDetailModal
  isOpen={expandedCard === "cpu"}
  onClose={() => setExpandedCard(null)}
  title="处理器"
  icon={Cpu}
  type="cpu"
  specs={[
    { label: "型号", value: "Intel Core i7-14700K" },
    { label: "制造商", value: "Intel Corporation" },
    { label: "架构", value: "x64" },
    { label: "插槽", value: "LGA1700" },
    { label: "核心数", value: "20 (8P + 12E)" },
    { label: "线程数", value: "28" },
    { label: "基础频率", value: "3.4 GHz" },
    { label: "最大睿频", value: "5.6 GHz" },
    { label: "外频", value: "100 MHz" },
    { label: "L2 缓存", value: "28 MB" },
    { label: "L3 缓存", value: "33 MB" },
    { label: "L3 缓存速度", value: "—" },
    { label: "步进", value: "C0" },
    { label: "修订", value: "1" },
    { label: "已启用核心", value: "20" },
  ]}
/>
```

**浮层组件要点：**

- **`HardwareDetailModal`**：使用 Chakra UI 的 `Modal` 或 `Drawer` 实现
- **规格列表**：`{ label: string, value: string }[]` — 纯键值对，不显示任何状态
- **自适应布局**：若规格项较多（如内存 4 条、GPU 2 块），使用可滚动区域 + 分组
- **关闭方式**：点击遮罩层关闭 / ESC 关闭 / 右上角 X 按钮

### 5.3 页面布局

```
┌────────────────────────────────────────────┐
│  硬件信息                         [清除] [导出] │
├────────────────────────────────────────────┤
│ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐       │
│ │ CPU  │ │ GPU  │ │ RAM  │ │Storage│      │
│ │ xx%  │ │ xx%  │ │ xx%  │ │ xx%  │      │
│ └──────┘ └──────┘ └──────┘ └──────┘       │
│                                            │
│ ┌─────────┐ ┌─────────┐ ┌─────────┐       │
│ │ 处理器  │ │ 显卡    │ │ 内存    │  ← 可点击  │
│ │ • 型号  │ │ • 型号  │ │ • 总容量│       │
│ │ • 核心  │ │ • 显存  │ │ • 频率  │       │
│ └─────╋───┘ └─────╋───┘ └─────╋───┘       │
│ ┌─────╋───┐ ┌─────╋───┐ ┌─────╋───┐       │
│ │ 主板    │ │ 存储  │ │ 声卡    │  ← 可点击  │
│ │ • 型号  │ │ • 型1  │ │ • 型1  │       │
│ │         │ │ • 型2  │ │ • 型2  │       │
│ └─────╋───┘ └─────╋───┘ └─────╋───┘       │
│ ┌─────╋───┐ ┌─────╋───┐                   │
│ │ 网卡    │ │ 显示器│  ← 可点击             │
│ │ • 型1  │ │ • 名1  │                      │
│ │ • 型2  │ │ • 分1  │                      │
│ └────────┘ └────────┘                      │
│                                            │
│ 点击任一卡片 → 弹出详情浮层                  │
└────────────────────────────────────────────┘
```

### 5.4 各卡片展开详情内容

#### 处理器展开详情

| 标签 | 值来源 |
|------|--------|
| 型号 | CpuInfo.name |
| 制造商 | CpuInfo.manufacturer |
| 架构 | CpuInfo.architecture |
| 插槽 | CpuInfo.socket |
| 核心数 | CpuInfo.cores |
| 线程数 | CpuInfo.threads |
| 已启用核心 | CpuInfo.enabled_cores （若不同） |
| 基础频率 | CpuInfo.max_clock_speed |
| 当前频率 | CpuInfo.current_clock_speed |
| 外频(总线) | CpuInfo.ext_clock |
| L2 缓存 | CpuInfo.l2_cache_size |
| L3 缓存 | CpuInfo.l3_cache_size |
| 步进 | CpuInfo.stepping |
| 修订 | CpuInfo.revision |
| 处理器 ID | CpuInfo.processor_id |
| 系列/家族 | CpuInfo.family |
| 电压能力 | CpuInfo.voltage_caps |

#### 显卡展开详情

| 标签 | 值来源 |
|------|--------|
| 型号 | GpuInfo.name |
| 核心架构 | GpuInfo.video_processor |
| 制造商 | GpuInfo.adapter_compatibility |
| 显存 | GpuInfo.memory_gb |
| 显存类型 | GpuInfo.video_memory_type |
| 驱动版本 | GpuInfo.driver_version |
| 驱动日期 | GpuInfo.driver_date |
| 驱动文件 | GpuInfo.installed_drivers |
| INF 文件 | GpuInfo.inf_filename |
| 设备 ID | GpuInfo.device_id |
| PNP ID | GpuInfo.pnp_device_id |
| 当前分辨率 | width x height |
| 当前刷新率 | refresh_rate |
| 状态 | GpuInfo.status |

> 每个 GPU 独立一个展开卡片（支持多 GPU），在浮层内用 Tab 或分段展示。

#### 内存展开详情

首行显示汇总：总容量、总条数、总频率

| 标签 | 值来源 |
|------|--------|
| 插槽 | MemoryInfo.bank_label |
| 型号 | MemoryInfo.part_number |
| 制造商 | MemoryInfo.manufacturer |
| 容量 | MemoryInfo.capacity_gb |
| 频率 | MemoryInfo.speed_mhz |
| 配置频率 | MemoryInfo.configured_clock_speed |
| 类型 | MemoryInfo.memory_type |
| 外形规格 | MemoryInfo.form_factor |
| 数据宽度 | MemoryInfo.data_width |
| 总宽度 | MemoryInfo.total_width |
| 配置电压 | MemoryInfo.configured_voltage |
| 序列号 | MemoryInfo.serial_number |

> 多条内存时在浮层内以表格/列表形式展示。

#### 主板/Motherboard 展开详情

| 标签 | 值来源 |
|------|--------|
| 型号 | MotherboardInfo.product |
| 制造商 | MotherboardInfo.manufacturer |
| 序列号 | MotherboardInfo.serial_number |
| 版本 | MotherboardInfo.version |
| BIOS 制造商 | MotherboardInfo.bios_vendor |
| BIOS 版本 | MotherboardInfo.bios_version |
| BIOS 发布日期 | MotherboardInfo.bios_release_date |
| 系统制造商 | MotherboardInfo.system_manufacturer |
| 系统型号 | MotherboardInfo.system_model |
| 系统类型 | MotherboardInfo.system_type |
| 机箱类型 | MotherboardInfo.chassis_type |

#### 存储展开详情

每块磁盘独立条目：

| 标签 | 值来源 |
|------|--------|
| 型号 | DiskDetailInfo.model |
| 容量 | DiskDetailInfo.size_gb |
| 类型 | DiskDetailInfo.media_type (SSD/HDD) |
| 接口 | DiskDetailInfo.interface_type |
| 序列号 | DiskDetailInfo.serial_number |
| 固件版本 | DiskDetailInfo.firmware_revision |
| 分区数 | DiskDetailInfo.partitions |
| 扇区大小 | DiskDetailInfo.bytes_per_sector |
| 状态 | DiskDetailInfo.status |

#### 声卡展开详情

| 标签 | 值来源 |
|------|--------|
| 名称 | SoundCardInfo.name |
| 制造商 | SoundCardInfo.manufacturer |
| 状态 | SoundCardInfo.status |
| 设备 ID | SoundCardInfo.device_id |

#### 网卡展开详情

| 标签 | 值来源 |
|------|--------|
| 名称 | NetworkCardInfo.name |
| 连接名称 | NetworkCardInfo.connection_name |
| 制造商 | NetworkCardInfo.manufacturer |
| 服务名 | NetworkCardInfo.service_name |
| 适配器类型 | NetworkCardInfo.adapter_type |
| MAC 地址 | NetworkCardInfo.mac_address |
| 链路速度 | NetworkCardInfo.speed_mbps |
| 最大速度 | NetworkCardInfo.max_speed |
| GUID | NetworkCardInfo.guid |

#### 显示器展开详情

| 标签 | 值来源 |
|------|--------|
| 名称 | MonitorInfo.name |
| 制造商 | MonitorInfo.manufacturer |
| 分辨率 | width x height |
| 刷新率 | MonitorInfo.refresh_rate |
| PNP ID | MonitorInfo.pnp_device_id |
| 状态 | MonitorInfo.status |

> 支持多显示器，每个独立一个条目。

---

## 六、涉及文件清单

| 文件 | 操作 | 说明 |
|------|------|------|
| `src-tauri/src/hardware.rs` | [MODIFY] | 扩展全部结构体 + WMI 查询（主文件） |
| `src-tauri/src-tauri/src/hardware.rs` | [MODIFY] | 同步更新（旧版文件） |
| `src/lib/hardware.ts` | [MODIFY] | 同步 TS 接口类型 |
| `src/pages/HardwarePage.tsx` | [MODIFY] | 添加点击展开逻辑 + HardwareDetailModal 组件 |
| `src/components/HardwareDetailModal.tsx` | [ADD] | 新建详情浮层组件 |
| `src/locales/zh.json` | [MODIFY] | 新增翻译键 |
| `src/locales/en.json` | [MODIFY] | 新增翻译键 |
| `src/locales/ja.json` | [MODIFY] | 新增翻译键 |

---

## 七、与现有架构的兼容性

| 方面 | 说明 |
|------|------|
| Tauri 命令 | `get_hardware` 接口不变，仅 payload 扩展字段 |
| 前端调用 | 现有 `useAppStartup()` 获取 `hardwareInfo` 不变 |
| 后端缓存 | `StaticHardwareCache` 不变，仅数据结构扩展 |
| 旧版文件 | `src-tauri/src-tauri/src/hardware.rs` 需同步结构体变更 |
| LHML 整合 | 轮询器不变，数据已在 `overlay_hardware_data` 中可用 |
| HardwareModelCard | 存储展示从 `Vec<String>` 改为 `DiskDetailInfo[]`，需调整 |
| 首页摘要 | 存储摘要行从字符串改为结构化解析 |

---

## 八、实施步骤

### 阶段一：后端 Rust 扩展

1. 扩展 `PsProcessor` → 新增全部 CPU WMI 属性字段
2. 扩展 `PsVideoController` → 新增全部 GPU WMI 属性字段
3. 将 `motherboard: String` 改为 `MotherboardInfo` 结构体
4. 扩展 `PsPhysicalMemory` → 新增内存类型/规格字段
5. 将 `disk: Vec<String>` 改为 `Vec<DiskDetailInfo>`（新建结构体）
6. 扩展 `PsNetworkAdapter` → 新增连接名/服务名等
7. 新增 `MonitorInfo` 结构体 + `PsDesktopMonitor`
8. 新增显示器 WMI 查询线程（`Win32_DesktopMonitor`）
9. BIOS/系统/机箱信息并入主板查询线程
10. 同步更新旧版 `src-tauri/src-tauri/src/hardware.rs`

### 阶段二：前端类型 & 组件

11. 更新 `src/lib/hardware.ts` 所有 TS 接口
12. 新建 `src/components/HardwareDetailModal.tsx` 浮层组件
13. 修改 `HardwarePage.tsx`：为每个 DetailCard 添加 `onClick`
14. 管理 `expandedCard` 状态（哪个卡片展开）
15. 构建各硬件类别的完整规格列表
16. 调整 HardwareModelCard 存储展示方式

### 阶段三：国际化

17. 为所有新增字段添加翻译键

### 阶段四：测试

18. 验证 WMI 解析正确性
19. 验证浮层交互流程
20. 验证多显示器/多 GPU/多内存条场景

---

## 九、预期效果

```
┌────────────────────────────────────────────┐
│  硬件信息                         [清除] [导出] │
├────────────────────────────────────────────┤
│ CPU 45%  │ GPU 67%  │ RAM 62%  │ Disk 32% │
├────────────────────────────────────────────┤
│ ┌───────────┐ ┌───────────┐ ┌───────────┐ │
│ │ 处理器    │ │ 显卡      │ │ 内存      │ │
│ │ i7-14700K│ │ RTX 4080 │ │ 32GB DDR5│ │
│ │ 20核28线  │ │ 16GB GDDR6│ │ 5600MHz  │ │
│ └─────click─┘ └─────click─┘ └─────click─┘ │
│ ┌───────────┐ ┌───────────┐ ┌───────────┐ │
│ │ 主板      │ │ 存储      │ │ 声卡      │ │
│ │ ROG Z790 │ │ 1TB SSD  │ │ ALC4080  │ │
│ │ BIOS 1661│ │ 2TB HDD  │ │ ─────    │ │
│ └─────click─┘ └─────click─┘ └─────click─┘ │
│ ┌───────────┐ ┌───────────┐               │
│ │ 网卡      │ │ 显示器    │               │
│ │ I225-V   │ │ DElL S27  │               │
│ │ 2.5Gbps  │ │ 2560x1440 │               │
│ └─────click─┘ └─────click─┘               │
└────────────────────────────────────────────┘

[点击 "处理器" → 弹窗]
┌────────────────────────────────────┐
│ 🔲 处理器                        ✕ │
├────────────────────────────────────┤
│ 型号: Intel Core i7-14700K         │
│ 制造商: Intel Corporation          │
│ 架构: x64                          │
│ 插槽: LGA1700                      │
│ 核心数: 20 (8P + 12E)              │
│ 线程数: 28                         │
│ 基础频率: 3.4 GHz                  │
│ 最大睿频: 5.6 GHz                  │
│ 外频: 100 MHz                      │
│ L2 缓存: 28 MB                     │
│ L3 缓存: 33 MB                     │
│ 步进: C0                           │
│ 修订: 1                            │
│ 已启用核心: 20                     │
│ 处理器 ID: BFEBFBFF000A06F2        │
│ 电压能力: —                        │
└────────────────────────────────────┘
```
