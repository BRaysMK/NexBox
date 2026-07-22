# NexBox EQ 调音功能 — 完整实现方案

> 基于 FxSound 虚拟声卡驱动的系统级 EQ 调音功能

---

## 一、项目概述

在 NexBox 内置工具中新增 **EQ 调音** 功能页面，利用 FxSound 的虚拟声卡驱动 (`fxvad`) 实现系统级音频均衡器调音。

### 核心功能
1. **虚拟声卡安装/卸载** — 一键安装/卸载 FxSound 虚拟音频驱动
2. **完整 EQ 调音** — 10 频段参数均衡器，支持实时调节
3. **预设管理** — 内置多种音频预设（音乐、游戏、电影、人声等），支持自定义保存
4. **搜索集成** — 在全局搜索中加入 EQ 调音入口

---

## 二、FxSound 虚拟声卡源码深度分析

### 2.1 源码结构

```
fxsound-driver-main/
├── fxvad/
│   ├── adapter.cpp          # 驱动入口 DriverEntry / AddDevice / StartDevice / PnP 处理
│   ├── common.cpp/h         # CAdapterCommon 类 — 驱动核心逻辑（设备实例化/插拔模拟/混音器）
│   ├── hw.cpp/h             # CMSVADHW 类 — 虚拟硬件寄存器（音量/静音/Mux 数组）
│   ├── savedata.cpp/h       # CSaveData 类 — 数据保存（所有方法已 stub 为空实现）
│   ├── basedma.cpp          # IDmaChannel 实现 — DMA 缓冲区管理（CopyFrom/CopyTo 为空/stub）
│   ├── basewave.cpp/h       # CMiniportWaveCyclicMSVAD — 波形迷你端口基类
│   ├── basetopo.cpp/h       # CMiniportTopologyMSVAD — 拓扑迷你端口基类（Volume/Mute/Mux 属性处理）
│   ├── kshelper.cpp/h       # KS 属性辅助函数（格式验证/属性参数验证）
│   ├── msvad.h              # 公共定义（DMA缓冲区大小/设备上下文/调试宏）
│   ├── fxvad.rc             # 资源文件（版本信息：14.1.0.0, Copyright 2021）
│   ├── fxvad.sln            # Visual Studio 解决方案
│   ├── fxvad.ico            # 驱动图标
│   └── pcmex/
│       ├── fxvad.inf        # 驱动安装 INF 文件
│       ├── pcmex.h          # ★ 关键：音频格式定义与流限制
│       ├── mintopo.cpp/h    # 拓扑迷你端口实现 + 物理连接表
│       ├── minwave.cpp/h    # 波形迷你端口实现（格式验证/通道配置）
│       ├── wavtable.h       # 波形引脚/节点/连接描述符表
│       ├── toptable.h       # 拓扑引脚/节点/连接描述符表
│       └── vadpcmex.vcxproj # VS 项目文件
├── README.md
└── LICENSE                  # AGPL v3.0
```

### 2.2 ★ 关键发现：驱动仅支持渲染（Render-Only）

**这是最重要的发现，直接影响音频引擎架构设计。**

通过分析 `pcmex.h` 中的流限制定义：

```c
// pcmex.h — 流数量限制
#define MAX_OUTPUT_STREAMS          0       // 捕获流数量 = 0（禁用捕获！）
#define MAX_INPUT_STREAMS           1       // 渲染流数量 = 1
#define MAX_TOTAL_STREAMS           MAX_OUTPUT_STREAMS + MAX_INPUT_STREAMS
```

通过分析 `fxvad.inf` 中的接口注册：

```inf
[DFX_Device.NT.Interfaces]
AddInterface=%KSCATEGORY_AUDIO%,%KSNAME_Wave%,DFX_Device.I.Wave
AddInterface=%KSCATEGORY_RENDER%,%KSNAME_Wave%,DFX_Device.I.Wave
;AddInterface=%KSCATEGORY_CAPTURE%,%KSNAME_Wave%,DFX_Device.I.Wave   ← 捕获接口被注释掉！
AddInterface=%KSCATEGORY_AUDIO%,%KSNAME_Topology%,DFX_Device.I.Topo
```

通过分析 `mintopo.cpp` 中的物理连接表：

```c
// mintopo.cpp — 拓扑/波形桥接连接
//  Capture <---|0    1|<===|4    1|<--- Synth
//  Render  --->|2    3|===>|0     |
PHYSICALCONNECTIONTABLE TopologyPhysicalConnections =
{
    KSPIN_TOPO_WAVEOUT_SOURCE,  // TopologyIn  — 渲染路径连接
    (ULONG)-1,                  // TopologyOut = -1 — 捕获路径未连接！
    (ULONG)-1,                  // WaveIn = -1      — 捕获路径未连接！
    KSPIN_WAVE_RENDER_SOURCE    // WaveOut         — 渲染路径连接
};
```

**结论：** 该驱动源码编译后**仅支持音频渲染（播放）**，不支持捕获（录音）。应用程序可以将音频播放到虚拟设备，但无法从虚拟设备捕获音频流。

### 2.3 ★ 关键发现：音频格式约束

从 `pcmex.h` 中提取的精确音频格式限制：

```c
// pcmex.h — PCM 格式约束
#define MIN_CHANNELS                2       // 最小声道数
#define MAX_CHANNELS_PCM            8       // 最大声道数
#define MIN_BITS_PER_SAMPLE_PCM     24      // 最小位深 = 24-bit！
#define MAX_BITS_PER_SAMPLE_PCM     24      // 最大位深 = 24-bit！
#define MIN_SAMPLE_RATE             44100   // 最小采样率
#define MAX_SAMPLE_RATE             48000   // 最大采样率
```

从 `minwave.cpp` 的 `DataRangeIntersection` 中确认支持的声道配置：
- **2 声道**（立体声，`KSAUDIO_SPEAKER_STEREO`）
- **6 声道**（5.1 环绕，`KSAUDIO_SPEAKER_5POINT1` / `KSAUDIO_SPEAKER_5POINT1_SURROUND`）
- **8 声道**（7.1 环绕，`KSAUDIO_SPEAKER_7POINT1` / `KSAUDIO_SPEAKER_7POINT1_SURROUND`）

从 `msvad.h` 中的 DMA 缓冲区大小：
```c
#define DMA_BUFFER_SIZE             0x16000  // 88 KB
```

**格式约束总结：**

| 参数 | 值 | 说明 |
|------|-----|------|
| 位深 | **仅 24-bit PCM** | 不支持 16-bit 或 32-bit float |
| 采样率 | 44100 ~ 48000 Hz | 标准音频范围 |
| 声道 | 2 / 6 / 8 | 立体声 / 5.1 / 7.1 |
| 渲染流 | 最多 1 个 | 同时只能有一个播放流 |
| 捕获流 | 0 个 | **不支持捕获** |

### 2.4 ★ 关键发现：驱动不处理任何音频数据

通过分析 `basedma.cpp` 中的 DMA 数据拷贝：

```cpp
// CopyFrom — 从 DMA 缓冲区读取数据（空实现，什么也不做）
STDMETHODIMP_(void) CMiniportWaveCyclicStreamMSVAD::CopyFrom(...)
{
    UNREFERENCED_PARAMETER(Destination);
    UNREFERENCED_PARAMETER(Source);
    UNREFERENCED_PARAMETER(ByteCount);
    // 完全空实现 — 不拷贝任何数据
}

// CopyTo — 向 DMA 缓冲区写入数据（调用 stub 的 WriteData）
STDMETHODIMP_(void) CMiniportWaveCyclicStreamMSVAD::CopyTo(...)
{
    UNREFERENCED_PARAMETER(Destination);
    m_SaveData.WriteData((PBYTE) Source, ByteCount);
    // WriteData 也是空实现 — 不保存任何数据
}
```

`savedata.cpp` 中的 `WriteData`：
```cpp
void CSaveData::WriteData(PBYTE pBuffer, ULONG ulByteCount)
{
    UNREFERENCED_PARAMETER(pBuffer);
    UNREFERENCED_PARAMETER(ulByteCount);
    // 完全空实现
}
```

**结论：** 驱动是一个纯虚拟音频设备，不进行任何音频数据处理。所有 DSP（EQ、音效）必须在用户空间实现。

### 2.5 驱动拓扑结构详解

从 `toptable.h` 和 `basetopo.cpp` 分析的拓扑节点：

```
拓扑连接图（来自 toptable.h）:

  WaveOut ──→ [Volume] ──→ [Mute] ──→ ┐
                                       ├──→ [Sum] ──→ [Volume] ──→ LineOut (扬声器)
  SynthOut ──→ [Volume] ──→ [Mute] ──→ ┘
  
  SynthIn ───→ [Volume] ──────────────→ [Mux] ──→ WaveIn
  Mic ───────→ [Volume] ──────────────→ [Mux] ──→ WaveIn
```

拓扑节点属性（来自 `basetopo.cpp`）：
- **Volume 节点**：范围 -96 dB ~ 0 dB，步进 0.5 dB
- **Mute 节点**：BOOL 类型（开/关）
- **Mux 节点**：选择输入源（默认 Mic）
- **Sum 节点**：多路混音

驱动初始化时默认选择 Mic 输入（`mintopo.cpp`：`m_AdapterCommon->MixerMuxWrite(KSPIN_TOPO_MIC_SOURCE)`）

### 2.6 驱动工作原理总结

```
┌─────────────────────────────────────────────────────────────────┐
│                    Windows 音频子系统                             │
│                                                                 │
│  ┌──────────┐    WASAPI 渲染     ┌──────────────────────┐       │
│  │ 应用程序  │ ──────────────►   │ fxvad.sys            │       │
│  │ (播放器)  │                   │ (虚拟渲染设备)         │       │
│  └──────────┘                   │                      │       │
│                                 │ ⚠ 仅渲染，无捕获接口    │       │
│                                 │ ⚠ 不处理任何音频数据    │       │
│                                 │ ⚠ 24-bit PCM only     │       │
│                                 └──────────────────────┘       │
│                                            │                    │
│                                   WASAPI Loopback 捕获           │
│                                            ▼                    │
│  ┌──────────────────────────────────────────────────────┐       │
│  │     NexBox 用户空间音频处理引擎                        │       │
│  │                                                      │       │
│  │  Loopback 捕获虚拟设备音频 → EQ 滤波处理 → 输出到真实设备 │      │
│  └──────────────────────────────────────────────────────┘       │
│                                          │                      │
│                                  WASAPI 渲染                     │
│                                          ▼                      │
│                            ┌──────────────────────┐             │
│                            │   真实声卡/扬声器      │             │
│                            └──────────────────────┘             │
└─────────────────────────────────────────────────────────────────┘
```

### 2.7 是否需要重新编译驱动？

**结论：不需要重新编译。使用官方已签名驱动即可。**

**详细原因分析：**

| 方面 | 分析 |
|------|------|
| **音频处理** | 驱动不包含任何音频处理逻辑（`CopyFrom`/`CopyTo`/`WriteData` 全部空实现），编译后与源码功能完全一致 |
| **驱动签名** | Windows 10/11 64位要求内核驱动有 WHQL 签名。自行编译需要 EV 证书 + 微软门户提交，成本极高且周期长 |
| **构建环境** | 需要 Visual Studio 2022 + Windows WDK，环境搭建复杂 |
| **Render-Only 限制** | 即使重新编译，源码中 `MAX_OUTPUT_STREAMS=0` 且 INF 中捕获接口被注释，仍然不支持捕获。要启用捕获需修改源码（改宏定义、取消 INF 注释、修改连接表），但这会引入未知稳定性风险 |
| **Loopback 替代** | 使用 WASAPI Loopback 捕获可以完美绕过驱动的 Render-Only 限制，无需修改驱动 |
| **合规性** | 驱动源码为 AGPL v3.0，使用官方预编译驱动更合规 |

**推荐方案：** 从 FxSound 官方安装包中提取已签名的驱动文件，打包到 NexBox 的资源目录中。使用 WASAPI Loopback 实现音频捕获。

### 2.8 驱动文件说明

| 文件 | 说明 | 目标路径 |
|------|------|----------|
| `fxvad.sys` | 驱动二进制文件（版本 14.1.0.0） | `%SystemRoot%\System32\drivers\fxvad.sys` |
| `fxvad.inf` | 驱动安装信息文件 | 安装时引用 |
| `fxvad.cat` | 驱动目录文件（包含数字签名） | 安装时引用 |

驱动设备信息（来自 `fxvad.inf` 和 `fxvad.rc`）：
- **设备名称**: `FxSound Audio Enhancer`
- **硬件 ID**: `Root\FXVAD`
- **服务名称**: `FXVAD`
- **设备类**: `MEDIA` (ClassGUID: `{4d36e96c-e325-11ce-bfc1-08002be10318}`)
- **启动类型**: `3` (手动启动 / Service Demand Start)
- **驱动版本**: `14.1.0.0` (2021年)
- **暴露接口**: `KSCATEGORY_AUDIO` + `KSCATEGORY_RENDER`（无 `KSCATEGORY_CAPTURE`）

---

## 三、技术方案设计

### 3.1 整体架构

```
┌──────────────────────────────────────────────────────────────┐
│                        NexBox 应用                            │
│                                                              │
│  ┌─────────────────┐  ┌──────────────────────────────────┐  │
│  │   前端 (React)   │  │         后端 (Rust/Tauri)         │  │
│  │                 │  │                                  │  │
│  │  EQ调音页面      │  │  ┌────────────────────────────┐  │  │
│  │  ├─ 声卡管理按钮  │◄─►│  │   audio_eq.rs (新增)       │  │  │
│  │  ├─ EQ频段滑块   │  │  │   ├─ 驱动安装/卸载           │  │  │
│  │  ├─ 预设选择     │  │  │   ├─ WASAPI Loopback 捕获    │  │  │
│  │  └─ 实时可视化   │  │  │   ├─ EQ Biquad 滤波处理     │  │  │
│  │                 │  │  │   ├─ WASAPI 渲染输出         │  │  │
│  └─────────────────┘  │  │   └─ 预设管理                │  │  │
│                       │  └────────────────────────────┘  │  │
│  搜索索引 (search-index.ts)  │  资源文件                      │  │
│  └─ 新增 EQ 调音入口  │  └─ resources/binaries/fxvad/    │  │
│                       └──────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
                              │
                    ┌─────────┴─────────┐
                    │  Windows 音频系统   │
                    │                   │
                    │  fxvad.sys (虚拟)  │ ← 仅渲染端点
                    │  真实声卡 (输出)    │
                    └───────────────────┘
```

### 3.2 ★ 核心技术：WASAPI Loopback 捕获

由于驱动是 **Render-Only**（不支持捕获接口），必须使用 **WASAPI Loopback** 来捕获虚拟设备播放的音频。

**WASAPI Loopback 原理：**
- Loopback 是 Windows 提供的标准 API，允许捕获某个**渲染端点**正在播放的音频
- 不需要驱动支持捕获接口
- 捕获的音频数据与渲染的音频数据完全一致
- FxSound 官方应用也是使用此方式

**Loopback 捕获流程：**
```
1. 枚举音频端点，找到 "FxSound Audio Enhancer" 渲染端点
2. 以 Loopback 模式打开该端点的捕获客户端
   → AUDCLNT_STREAMFLAGS_LOOPBACK
3. 循环读取捕获缓冲区 → 获得 PCM 音频数据
4. 将数据送入 EQ 滤波器处理
5. 将处理后的数据写入真实设备的渲染客户端
```

**Rust 伪代码（使用 windows crate）：**
```rust
use windows::Win32::Media::Audio::*;

fn start_loopback_capture(virtual_device_id: &str) -> Result<()> {
    // 1. 获取虚拟设备的 IMMDevice
    let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
    let virtual_device = enumerator.GetDevice(virtual_device_id)?;

    // 2. 以 Loopback 模式激活音频客户端
    let audio_client: IAudioClient = virtual_device.Activate(CLSCTX_ALL, None)?;
    
    // 3. 初始化 — 关键：使用 LOOPBACK 标志
    audio_client.Initialize(
        AUDCLNT_SHAREMODE_SHARED,
        AUDCLNT_STREAMFLAGS_LOOPBACK,  // ← Loopback 捕获！
        0,                              // 缓冲区时长（0=自动）
        0,
        &WAVEFORMATEX { /* 48kHz, 24-bit, 2ch */ },
        ptr::null(),
    )?;

    // 4. 获取捕获客户端
    let capture_client: IAudioCaptureClient = audio_client.GetService()?;

    // 5. 启动
    audio_client.Start()?;

    // 6. 主循环 — 读取数据
    loop {
        let mut packet_size = 0;
        let mut buffer = ptr::null();
        let mut flags = 0;

        capture_client.GetNextPacketSize(&mut packet_size)?;
        while packet_size > 0 {
            capture_client.GetBuffer(&mut buffer, &mut frames, &mut flags, None, None)?;
            
            // 处理音频数据 → EQ 滤波
            let processed = eq_chain.process(buffer, frames);
            
            // 写入真实设备
            render_client.write_buffer(&processed)?;
            
            capture_client.ReleaseBuffer(frames)?;
            capture_client.GetNextPacketSize(&mut packet_size)?;
        }
        
        // 等待下一批数据
        sleep(Duration::from_millis(1));
    }
}
```

### 3.3 虚拟声卡安装/卸载方案

#### 安装流程

```
用户点击"安装声卡"
    │
    ├─► 检查管理员权限 (如无则 UAC 提权)
    │
    ├─► 检查驱动是否已安装 (查询设备管理器/服务)
    │   └─ 已安装 → 提示用户，返回
    │
    ├─► 将驱动文件释放到临时目录
    │   ├─ fxvad.sys
    │   ├─ fxvad.inf
    │   └─ fxvad.cat
    │
    ├─► 使用 pnputil 安装驱动
    │   └─ pnputil /add-driver fxvad.inf /install
    │
    ├─► 使用 SetupAPI 创建 Root 设备实例
    │   └─ 创建 Root\FXVAD 设备节点
    │
    ├─► 等待设备初始化完成
    │
    └─► 返回安装结果
```

**安装方式选择：**

| 方式 | 优点 | 缺点 | 选择 |
|------|------|------|------|
| `pnputil` | 系统自带，兼容性好 | 仅安装驱动包，不创建设备实例 | ✅ 主选 |
| `SetupAPI` | 可创建 Root 设备 | 实现复杂 | ✅ 配合使用 |
| `devcon` | 功能全面 | 需要额外分发 | ❌ 不选 |
| `FxSound 安装包静默安装` | 最简单 | 安装整个 FxSound 应用 | ❌ 不选 |

**Rust 实现要点：**
- 使用 `windows` crate 的 `SetupAPI` 接口创建 Root 设备
- 使用 `std::process::Command` 调用 `pnputil` 安装驱动
- 使用 `runas` 或 `elevate` 机制处理 UAC 提权
- 安装后通过 `SetupDiGetDeviceProperty` 检查设备状态

#### 卸载流程

```
用户点击"卸载声卡"
    │
    ├─► 检查管理员权限
    │
    ├─► 停止音频处理引擎 (如果正在运行)
    │
    ├─► 恢复默认音频设备 (如果之前被切换)
    │
    ├─► 查找 fxvad 设备实例
    │   └─ SetupDiGetClassDevs + SetupDiEnumDeviceInfo
    │
    ├─► 移除设备
    │   └─ SetupDiCallClassInstaller (DIF_REMOVE)
    │
    ├─► 删除驱动包
    │   └─ pnputil /delete-driver oem*.inf /uninstall /force
    │
    ├─► 删除驱动文件
    │   └─ 删除 system32\drivers\fxvad.sys
    │
    └─► 返回卸载结果
```

#### 状态检测

```
检测声卡是否已安装：
    │
    ├─► 方法1: 查询 Windows 服务
    │   └─ OpenSCManager → OpenService("FXVAD")
    │
    ├─► 方法2: 查询设备管理器
    │   └─ SetupDiGetClassDevs (MEDIA 类) + 匹配硬件 ID "Root\FXVAD"
    │
    └─► 方法3: 枚举音频端点
        └─ IMMDeviceEnumerator → 查找 "FxSound Audio Enhancer" 渲染端点
```

### 3.4 EQ 调音引擎方案

#### 3.4.1 音频处理流程

```
┌──────────────┐  WASAPI 渲染   ┌──────────────┐  WASAPI Loopback  ┌──────────────┐
│  应用程序     │ ──────────►  │  fxvad 虚拟   │ ──────────────►  │  NexBox 音频  │
│  (播放器等)   │              │  渲染端点     │                  │  处理引擎     │
└──────────────┘              └──────────────┘                  └──────┬───────┘
                                                                   │
                                                              EQ 滤波链
                                                             ┌───────┴───────┐
                                                             │  10 频段 Biquad │
                                                             │  滤波器组       │
                                                             └───────┬───────┘
                                                                     │
                                                              WASAPI 渲染
                                                                     ▼
                                                             ┌──────────────┐
                                                             │  真实声卡     │
                                                             │  (扬声器)     │
                                                             └──────────────┘
```

#### 3.4.2 ★ 音频格式处理

由于驱动**仅支持 24-bit PCM**，音频引擎需要处理格式转换：

```
                    24-bit PCM (驱动格式)
                           │
                     ┌─────┴─────┐
                     │  格式转换   │
                     └─────┬─────┘
                           │
                    32-bit float (内部处理)
                           │
                     ┌─────┴─────┐
                     │  EQ 滤波   │
                     └─────┬─────┘
                           │
                    32-bit float (处理后)
                           │
                     ┌─────┴─────┐
                     │  格式转换   │
                     └─────┬─────┘
                           │
              匹配真实设备格式 (16/24/32-bit)
```

**格式转换说明：**
- **24-bit → float**：将 24-bit PCM 样本（3字节）转换为 `f32`（-1.0 ~ 1.0）
- **float → 输出格式**：根据真实设备支持的格式进行转换
- **采样率**：驱动支持 44100/48000，真实设备通常也支持，如不匹配需要重采样

#### 3.4.3 EQ 滤波器实现

**选用 Biquad 滤波器（双二阶滤波器）** — 业界标准 EQ 实现方案

**10 频段配置：**

| 频段 | 中心频率 | 类型 | 增益范围 |
|------|----------|------|----------|
| 1 | 32 Hz | 低频架搁 (Low Shelf) | -12 ~ +12 dB |
| 2 | 64 Hz | 峰值 (Peaking) | -12 ~ +12 dB |
| 3 | 125 Hz | 峰值 (Peaking) | -12 ~ +12 dB |
| 4 | 250 Hz | 峰值 (Peaking) | -12 ~ +12 dB |
| 5 | 500 Hz | 峰值 (Peaking) | -12 ~ +12 dB |
| 6 | 1 kHz | 峰值 (Peaking) | -12 ~ +12 dB |
| 7 | 2 kHz | 峰值 (Peaking) | -12 ~ +12 dB |
| 8 | 4 kHz | 峰值 (Peaking) | -12 ~ +12 dB |
| 9 | 8 kHz | 峰值 (Peaking) | -12 ~ +12 dB |
| 10 | 16 kHz | 高频架搁 (High Shelf) | -12 ~ +12 dB |

**Biquad 滤波器系数计算（Rust 伪代码）：**

```rust
// Peaking EQ 滤波器系数（参考 Audio EQ Cookbook）
fn compute_peaking_coefficients(
    sample_rate: f64,
    freq: f64,      // 中心频率
    gain_db: f64,   // 增益 (dB)
    q: f64,         // Q 值 (带宽)
) -> BiquadCoeffs {
    let a = 10.0_f64.powf(gain_db / 40.0);
    let w0 = 2.0 * std::f64::consts::PI * freq / sample_rate;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / (2.0 * q);

    let b0 = 1.0 + alpha * a;
    let b1 = -2.0 * cos_w0;
    let b2 = 1.0 - alpha * a;
    let a0 = 1.0 + alpha / a;
    let a1 = -2.0 * cos_w0;
    let a2 = 1.0 - alpha / a;

    BiquadCoeffs {
        b0: b0 / a0, b1: b1 / a0, b2: b2 / a0,
        a1: a1 / a0, a2: a2 / a0,
    }
}
```

**滤波器实现使用 Direct Form II Transposed 结构**（数值稳定性最佳）：

```rust
struct BiquadFilter {
    coeffs: BiquadCoeffs,
    z1: f64,  // 状态变量 1
    z2: f64,  // 状态变量 2
}

impl BiquadFilter {
    fn process_sample(&mut self, x: f64) -> f64 {
        let c = &self.coeffs;
        let y = c.b0 * x + self.z1;
        self.z1 = c.b1 * x - c.a1 * y + self.z2;
        self.z2 = c.b2 * x - c.a2 * y;
        y
    }
}
```

#### 3.4.4 WASAPI 音频捕获/渲染

**音频引擎核心循环：**

```rust
async fn audio_processing_loop(
    virtual_device_id: String,   // fxvad 虚拟设备 ID
    real_device_id: String,      // 真实声卡设备 ID
    eq_settings: Arc<RwLock<EqSettings>>,
) -> Result<()> {
    // 1. 初始化 WASAPI Loopback 捕获客户端 (从虚拟设备)
    let capture_client = wasapi::loopback_capture_client(&virtual_device_id)?;

    // 2. 初始化 WASAPI 渲染客户端 (到真实设备)
    let render_client = wasapi::render_client(&real_device_id, SAMPLE_RATE, CHANNELS)?;

    // 3. 初始化 EQ 滤波器链
    let mut eq_chain = EqChain::new(SAMPLE_RATE, CHANNELS);

    // 4. 主循环
    loop {
        // 从虚拟设备 Loopback 读取音频帧 (24-bit PCM)
        let buffer = capture_client.read_buffer()?;

        // 24-bit PCM → f32 转换
        let float_buffer = pcm24_to_float(&buffer);

        // 应用 EQ 滤波
        {
            let settings = eq_settings.read().unwrap();
            eq_chain.update_coefficients(&settings);
        }
        let processed = eq_chain.process(&float_buffer);

        // f32 → 输出格式转换
        let output_buffer = float_to_output_format(&processed, &render_client.format());

        // 写入真实设备
        render_client.write_buffer(&output_buffer)?;

        // 检查停止信号
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }
    }

    Ok(())
}
```

**技术选型：**
- 使用 `windows` crate (已在依赖中) 直接调用 WASAPI — 不引入额外依赖
- 或使用 `cpal` crate (Rust 音频库) 简化 WASAPI 操作 — 但 cpal 对 Loopback 支持有限
- **推荐直接使用 `windows` crate**，因为 Loopback 是 WASAPI 特有功能
- 采样率：48000 Hz (与驱动默认一致)
- 位深：24-bit PCM (驱动) ↔ 32-bit float (内部处理)
- 声道：2 (立体声，EQ 默认配置)

#### 3.4.5 默认设备切换

```
启用 EQ 时：
    1. 记录当前默认音频渲染设备 (IMMDeviceEnumerator::GetDefaultAudioEndpoint)
    2. 将默认播放设备切换为 fxvad 虚拟设备
       → 使用 IPolicyConfig::SetDefaultEndpoint (Vista+ 非公开接口)
    3. 启动音频处理引擎 (Loopback 捕获 fxvad → EQ → 渲染到真实设备)

禁用 EQ 时：
    1. 停止音频处理引擎
    2. 将默认播放设备恢复为原始真实设备
    3. (可选) 保留虚拟设备已安装状态
```

**默认设备切换接口：**
- `IPolicyConfig` / `IPolicyConfigVista` — Windows 未公开但稳定的 COM 接口
- 所有主流音频软件（FxSound、Voicemeeter 等）均使用此接口
- 需要手动定义 COM 接口的 vtable

### 3.5 预设管理

#### 内置预设

| 预设名称 | 频段增益 (dB) | 适用场景 |
|----------|--------------|----------|
| 平坦 (Flat) | 全部 0 | 默认/参考 |
| 音乐 (Music) | V 型曲线，增强低频和高频 | 听音乐 |
| 游戏 (Gaming) | 增强低频和高中频，提升脚步声和枪声 | FPS 游戏 |
| 电影 (Movie) | 增强低频和中低频，提升爆炸和配乐 | 看电影 |
| 人声 (Vocal) | 增强中频 (1-4kHz) | 听播客/人声 |
| 低音增强 (Bass Boost) | 32-125Hz 大幅增强 | 电子/嘻哈 |
| 古典 (Classical) | 温和提升高低频，中频微降 | 古典音乐 |
| 摇滚 (Rock) | 典型 V 型曲线，更激进 | 摇滚音乐 |

#### 预设数据结构

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct EqPreset {
    pub id: String,
    pub name: String,
    pub name_key: Option<String>,    // i18n key
    pub bands: Vec<BandGain>,         // 10 个频段增益
    pub is_builtin: bool,
    pub created_at: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BandGain {
    pub freq: u32,      // 中心频率
    pub gain_db: f32,   // 增益
    pub q: f32,         // Q 值
}

#[derive(Serialize, Deserialize, Clone)]
pub struct EqSettings {
    pub enabled: bool,
    pub preset_id: String,
    pub bands: Vec<BandGain>,
    pub master_volume: f32,    // 主音量补偿
    pub device_name: String,   // 目标输出设备
}
```

**存储位置：** `%LOCALAPPDATA%/NexBox/eq_settings.json` (使用 tauri-plugin-store)

### 3.6 前端页面设计

#### 页面结构

```
EQ 调音页面 (/audio-eq)
├── 顶部导航栏
│   ├── 返回按钮
│   └── 标题 "EQ 调音"
│
├── 声卡管理区域
│   ├── 状态指示器 (已安装/未安装/处理中)
│   ├── 安装/卸载按钮
│   └── 当前设备信息
│
├── EQ 调音控制区域
│   ├── EQ 开关 (总开关)
│   ├── 预设选择器 (下拉/卡片)
│   ├── 10 频段 EQ 滑块
│   │   ├── 频率标签 (32Hz ~ 16kHz)
│   │   ├── 增益滑块 (-12 ~ +12 dB)
│   │   └── 实时频率响应曲线
│   ├── 主音量补偿
│   └── 保存/重置按钮
│
├── 自定义预设管理
│   ├── 保存当前设置为新预设
│   ├── 已保存预设列表
│   └── 删除/重命名预设
│
└── 频率响应可视化
    ├── 实时频谱图 (可选)
    └── EQ 曲线显示
```

#### UI 组件

```
┌─────────────────────────────────────────────────┐
│  ←  EQ 调音                                      │
├─────────────────────────────────────────────────┤
│                                                 │
│  ┌─ 虚拟声卡 ────────────────────────────────┐  │
│  │  ● 已安装  FxSound Audio Enhancer         │  │
│  │                            [卸载声卡]      │  │
│  └───────────────────────────────────────────┘  │
│                                                 │
│  ┌─ EQ 均衡器 ───────────────────────────────┐  │
│  │                                           │  │
│  │  预设: [音乐 ▼]    EQ: [ON ●]            │  │
│  │                                           │  │
│  │  ┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐│  │
│  │  │ +6│ +3│ 0 │-3 │ 0 │+3 │+6 │+4 │+2 │ 0 ││  │
│  │  │ │ │ │ │ │ │ │ │ │ │ │ │ │ │ │ │ │ │ ││  │
│  │  │-12│-12│-12│-12│-12│-12│-12│-12│-12│-12││  │
│  │  └───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘│  │
│  │  32  64 125 250 500 1k  2k  4k  8k 16k Hz │  │
│  │                                           │  │
│  │  ╔═════════════════════════════════════╗  │  │
│  │  ║     EQ 频率响应曲线 (Canvas)        ║  │  │
│  │  ╚═════════════════════════════════════╝  │  │
│  │                                           │  │
│  │  主音量补偿: [─────●──] +0 dB            │  │
│  │                                           │  │
│  │  [重置] [保存为预设]                      │  │
│  └───────────────────────────────────────────┘  │
│                                                 │
│  ┌─ 我的预设 ───────────────────────────────┐  │
│  │  ○ 游戏增强 (2024-01-15)        [删除]   │  │
│  │  ○ 自定义1 (2024-01-10)         [删除]   │  │
│  └───────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
```

### 3.7 搜索集成

在 `src/config/search-index.ts` 中新增搜索项：

```typescript
{
  id: "audio-eq",
  nameKey: "sidebar.audioEq",
  path: "/audio-eq",
  icon: Volume2,
  category: "builtin-tool",
  keywords: ["EQ", "调音", "均衡器", "音频", "声卡", "equalizer", "audio", "fxsound", "音效"],
},
```

### 3.8 内置工具页面集成

在 `src/pages/BuiltinToolsPage.tsx` 的 `tools` 数组中新增：

```typescript
{
  id: "audio-eq",
  path: "/audio-eq",
  icon: Volume2,
  titleKey: "sidebar.audioEq",
  descriptionKey: "builtinTools.audioEqDesc",
  color: "#E74C3C",
},
```

---

## 四、文件变更清单

### 4.1 新增文件

| 文件路径 | 说明 |
|----------|------|
| `src-tauri/src/audio_eq.rs` | Rust 后端：驱动安装/卸载、WASAPI Loopback、EQ 处理 |
| `src/pages/AudioEqPage.tsx` | 前端：EQ 调音页面 |
| `src-tauri/resources/binaries/fxvad/fxvad.sys` | 虚拟声卡驱动文件（预编译签名版，v14.1.0.0） |
| `src-tauri/resources/binaries/fxvad/fxvad.inf` | 驱动安装信息文件 |
| `src-tauri/resources/binaries/fxvad/fxvad.cat` | 驱动签名目录文件 |

### 4.2 修改文件

| 文件路径 | 修改内容 |
|----------|----------|
| `src-tauri/src/lib.rs` | 注册 `audio_eq` 模块，添加 Tauri 命令 |
| `src/App.tsx` | 添加 `/audio-eq` 路由 |
| `src/pages/BuiltinToolsPage.tsx` | 在工具列表中添加 EQ 调音入口 |
| `src/config/search-index.ts` | 在搜索索引中添加 EQ 调音项 |
| `src/locales/zh.json` | 添加中文翻译 |
| `src/locales/en.json` | 添加英文翻译 |
| `src/locales/zh-TW.json` | 添加繁体中文翻译 |
| `src/locales/ja.json` | 添加日文翻译 |
| `src/locales/de.json` | 添加德文翻译 |
| `src/locales/fr.json` | 添加法文翻译 |

### 4.3 依赖变更

无需新增外部依赖。直接使用已有的 `windows` crate (v0.58) 调用 WASAPI。

需要在 `Cargo.toml` 的 `windows` features 中确保包含：
```toml
"Win32_Media_Audio",      # 已包含
"Win32_Devices_DeviceAndDriverInstallation",  # SetupAPI (新增)
"Win32_Devices_Properties",                    # 设备属性 (已包含)
```

---

## 五、Tauri 命令设计

```rust
// src-tauri/src/audio_eq.rs

/// 检查虚拟声卡驱动是否已安装
#[tauri::command]
pub fn check_virtual_audio_driver() -> Result<DriverStatus, String>;

/// 安装虚拟声卡驱动
#[tauri::command]
pub async fn install_virtual_audio_driver() -> Result<(), String>;

/// 卸载虚拟声卡驱动
#[tauri::command]
pub async fn uninstall_virtual_audio_driver() -> Result<(), String>;

/// 启动 EQ 音频处理引擎
#[tauri::command]
pub async fn start_eq_engine(settings: EqSettings) -> Result<(), String>;

/// 停止 EQ 音频处理引擎
#[tauri::command]
pub async fn stop_eq_engine() -> Result<(), String>;

/// 更新 EQ 设置（实时调节，无需重启引擎）
#[tauri::command]
pub fn update_eq_settings(settings: EqSettings) -> Result<(), String>;

/// 获取当前 EQ 设置
#[tauri::command]
pub fn get_eq_settings() -> Result<EqSettings, String>;

/// 获取内置预设列表
#[tauri::command]
pub fn get_eq_presets() -> Result<Vec<EqPreset>, String>;

/// 保存自定义预设
#[tauri::command]
pub fn save_eq_preset(preset: EqPreset) -> Result<(), String>;

/// 删除自定义预设
#[tauri::command]
pub fn delete_eq_preset(preset_id: String) -> Result<(), String>;

/// 获取可用音频输出设备列表
#[tauri::command]
pub fn get_audio_output_devices() -> Result<Vec<AudioDevice>, String>;

/// 获取当前默认音频设备
#[tauri::command]
pub fn get_default_audio_device() -> Result<AudioDevice, String>;

/// 设置默认音频设备
#[tauri::command]
pub fn set_default_audio_device(device_id: String) -> Result<(), String>;

/// 获取 EQ 引擎运行状态 (CPU占用、延迟、缓冲区)
#[tauri::command]
pub fn get_eq_engine_status() -> Result<EngineStatus, String>;
```

---

## 六、技术风险与应对

### 6.1 驱动签名问题

**风险：** 自行提取的驱动文件可能因签名问题无法在 Windows 10/11 上安装

**应对：**
- 使用 FxSound 官方安装包中的已签名驱动文件（v14.1.0.0）
- 官方驱动已通过 WHQL 认证，可正常安装
- 如遇签名问题，提示用户启用测试签名模式 (`bcdedit /set testsigning on`)

### 6.2 ★ WASAPI Loopback 兼容性

**风险：** Loopback 捕获依赖 Windows 的混音引擎，某些独占模式场景可能不工作

**应对：**
- Loopback 仅在共享模式下工作，确保虚拟设备处于共享模式
- 文档中说明：EQ 调音在应用使用共享模式播放时生效
- 如果应用使用独占模式（如某些游戏），音频不会经过虚拟设备，EQ 不生效
- 提供设备模式检测和提示

### 6.3 音频延迟

**风险：** Loopback 捕获 → 用户空间处理 → 渲染输出的链路可能引入音频延迟

**应对：**
- WASAPI 共享模式延迟通常 10-30ms，可接受
- 缓冲区大小优化：目标总延迟 < 50ms
- 使用事件驱动模式 (Event-Driven) 减少轮询延迟
- 提供延迟模式选择 (低延迟/高质量)

### 6.4 24-bit 格式处理

**风险：** 驱动仅支持 24-bit PCM，格式转换可能引入精度损失

**应对：**
- 24-bit → float32 转换无损（24-bit 精度 < float32 尾数 23-bit + 符号位）
- 内部使用 float64 进行滤波器计算（最大精度）
- 输出格式匹配真实设备，避免二次转换
- 测试不同位深组合的音质

### 6.5 音频设备切换冲突

**风险：** 切换默认设备时可能与其他音频软件冲突

**应对：**
- 启用 EQ 前记录原始默认设备
- 禁用 EQ 时恢复原始设备
- 监听设备变化事件，处理热插拔
- 提供设备选择，允许用户指定输出设备

### 6.6 驱动安装权限

**风险：** 驱动安装需要管理员权限

**应对：**
- 使用 UAC 提权弹窗
- 安装/卸载操作通过独立的提权进程执行
- 前端显示明确的权限请求提示

### 6.7 系统兼容性

**风险：** 不同 Windows 版本的驱动兼容性

**应对：**
- 驱动支持 Windows 7/8/10/11 (x64)
- 在安装前检测系统版本和架构
- 仅支持 x64 系统 (现代 PC 标准)

### 6.8 单渲染流限制

**风险：** 驱动仅支持 1 个渲染流（`MAX_INPUT_STREAMS = 1`），多应用同时播放可能有问题

**应对：**
- Windows 音频引擎会自动混音多个应用的音频流
- 虚拟设备作为默认设备时，所有应用音频都会经过 Windows 混音器
- Loopback 捕获的是混音后的结果，包含所有应用的音频
- 不会出现单流限制问题

---

## 七、TODO List

### Phase 1: 驱动管理 (核心)

- [ ] **1.1** 从 FxSound 官方安装包提取已签名驱动文件 (`fxvad.sys` v14.1.0.0、`fxvad.inf`、`fxvad.cat`)
- [ ] **1.2** 将驱动文件放入 `src-tauri/resources/binaries/fxvad/` 目录
- [ ] **1.3** 创建 `src-tauri/src/audio_eq.rs` 模块文件
- [ ] **1.4** 实现驱动状态检测函数 `check_virtual_audio_driver()`
  - [ ] 通过 Windows 服务管理器查询 FXVAD 服务 (`OpenSCManager` → `OpenService`)
  - [ ] 通过 SetupAPI 查询设备实例 (`SetupDiGetClassDevs` + 匹配 `Root\FXVAD`)
  - [ ] 通过 `IMMDeviceEnumerator` 查询 "FxSound Audio Enhancer" 渲染端点
- [ ] **1.5** 实现驱动安装函数 `install_virtual_audio_driver()`
  - [ ] UAC 提权处理 (启动提权子进程执行安装命令)
  - [ ] 释放驱动文件到临时目录
  - [ ] 调用 `pnputil /add-driver fxvad.inf /install` 安装驱动
  - [ ] 使用 SetupAPI 创建 `Root\FXVAD` 设备实例 (`SetupDiCreateDeviceInfoList` → `SetupDiCreateDeviceInfo` → `SetupDiCallClassInstaller`)
  - [ ] 等待设备初始化并验证安装
- [ ] **1.6** 实现驱动卸载函数 `uninstall_virtual_audio_driver()`
  - [ ] 查找并移除设备实例 (`SetupDiCallClassInstaller` + `DIF_REMOVE`)
  - [ ] 调用 `pnputil /delete-driver oem*.inf /uninstall /force`
  - [ ] 清理 `system32\drivers\fxvad.sys` 文件
- [ ] **1.7** 在 `Cargo.toml` 中添加 `Win32_Devices_DeviceAndDriverInstallation` feature
- [ ] **1.8** 在 `lib.rs` 中注册 `audio_eq` 模块和 Tauri 命令

### Phase 2: 前端页面基础

- [ ] **2.1** 创建 `src/pages/AudioEqPage.tsx` 页面组件
- [ ] **2.2** 实现页面布局（参考 `DiskHealthPage.tsx` 风格）
  - [ ] 顶部导航栏（返回按钮 + 标题）
  - [ ] 声卡管理区域
  - [ ] EQ 调音区域
  - [ ] 预设管理区域
- [ ] **2.3** 实现声卡管理 UI
  - [ ] 驱动状态指示器（已安装/未安装/安装中/卸载中）
  - [ ] 安装按钮（调用 `install_virtual_audio_driver`）
  - [ ] 卸载按钮（调用 `uninstall_virtual_audio_driver`）
  - [ ] 安装/卸载进度反馈（Loading 状态 + 成功/失败 Toast）
  - [ ] 错误处理与提示（权限不足、签名问题等）
- [ ] **2.4** 在 `App.tsx` 中注册 `/audio-eq` 路由
- [ ] **2.5** 在 `BuiltinToolsPage.tsx` 中添加 EQ 调音工具卡片
- [ ] **2.6** 在 `search-index.ts` 中添加搜索索引项

### Phase 3: WASAPI Loopback 音频引擎

- [ ] **3.1** 实现 WASAPI 初始化
  - [ ] COM 初始化 (`CoInitializeEx`)
  - [ ] `IMMDeviceEnumerator` 枚举音频端点
  - [ ] 查找 "FxSound Audio Enhancer" 渲染端点
  - [ ] 查找真实音频输出设备
- [ ] **3.2** 实现 WASAPI Loopback 捕获
  - [ ] 以 `AUDCLNT_SHAREMODE_SHARED` + `AUDCLNT_STREAMFLAGS_LOOPBACK` 初始化捕获客户端
  - [ ] 获取捕获缓冲区 (`IAudioCaptureClient`)
  - [ ] 事件驱动模式 (`SetEventHandle`) 或定时轮询模式
  - [ ] 处理 24-bit PCM 数据读取
- [ ] **3.3** 实现 24-bit PCM ↔ f32 格式转换
  - [ ] 24-bit little-endian → f32 解码
  - [ ] f32 → 24-bit / 16-bit / 32-bit 编码（匹配输出设备）
  - [ ] 多声道交错/平面格式处理
- [ ] **3.4** 实现 Biquad 滤波器
  - [ ] Peaking EQ 滤波器系数计算
  - [ ] Low Shelf 滤波器系数计算
  - [ ] High Shelf 滤波器系数计算
  - [ ] Direct Form II Transposed 实现
  - [ ] 多声道处理 (立体声 / 5.1 / 7.1)
- [ ] **3.5** 实现 EQ 滤波器链
  - [ ] 10 频段串联滤波
  - [ ] 系数实时更新（线程安全 — `Arc<RwLock>`）
  - [ ] 平滑过渡（避免参数突变产生爆音）
- [ ] **3.6** 实现 WASAPI 渲染输出
  - [ ] 初始化渲染客户端 (`IAudioRenderClient`)
  - [ ] 获取真实设备支持的格式
  - [ ] 缓冲区写入与同步
  - [ ] 采样率匹配（必要时重采样）
- [ ] **3.7** 实现音频处理主循环
  - [ ] Loopback 捕获 → 格式转换 → EQ 滤波 → 格式转换 → 渲染
  - [ ] 独立音频线程（高优先级）
  - [ ] 停止信号处理（`AtomicBool`）
  - [ ] 错误恢复（设备断开重连）
- [ ] **3.8** 实现默认设备切换
  - [ ] 定义 `IPolicyConfig` COM 接口 vtable
  - [ ] 记录原始默认设备 (`GetDefaultAudioEndpoint`)
  - [ ] 切换默认设备到 fxvad (`SetDefaultEndpoint`)
  - [ ] 恢复原始设备
- [ ] **3.9** 实现 Tauri 命令
  - [ ] `start_eq_engine` — 启动处理引擎
  - [ ] `stop_eq_engine` — 停止处理引擎
  - [ ] `update_eq_settings` — 实时更新 EQ 参数
  - [ ] `get_eq_engine_status` — 获取引擎状态

### Phase 4: EQ 调音 UI

- [ ] **4.1** 实现 EQ 开关组件（总开关）
- [ ] **4.2** 实现 10 频段 EQ 滑块组件
  - [ ] 垂直滑块 (-12 ~ +12 dB)
  - [ ] 频率标签显示 (32/64/125/250/500/1k/2k/4k/8k/16k)
  - [ ] 增益数值显示
  - [ ] 拖动时实时更新音频参数 (debounce 50ms)
- [ ] **4.3** 实现频率响应曲线可视化
  - [ ] Canvas 绘制 EQ 曲线
  - [ ] 实时反映滑块变化
  - [ ] 频率/增益坐标轴
  - [ ] 对数频率轴
- [ ] **4.4** 实现主音量补偿滑块
- [ ] **4.5** 实现重置按钮（恢复平坦响应）
- [ ] **4.6** 实现音频输出设备选择器
  - [ ] 列出可用输出设备
  - [ ] 显示当前设备
  - [ ] 切换输出设备

### Phase 5: 预设管理

- [ ] **5.1** 定义预设数据结构 (`EqPreset`、`EqSettings`)
- [ ] **5.2** 创建内置预设数据
  - [ ] 平坦 (Flat) — 全部 0 dB
  - [ ] 音乐 (Music) — V 型曲线
  - [ ] 游戏 (Gaming) — 增强脚步声频段
  - [ ] 电影 (Movie) — 增强低频
  - [ ] 人声 (Vocal) — 中频增强
  - [ ] 低音增强 (Bass Boost) — 32-125Hz +8~12 dB
  - [ ] 古典 (Classical) — 温和曲线
  - [ ] 摇滚 (Rock) — 激进 V 型
- [ ] **5.3** 实现预设持久化存储
  - [ ] 使用 tauri-plugin-store 保存设置
  - [ ] 保存路径：`%LOCALAPPDATA%/NexBox/eq_settings.json`
- [ ] **5.4** 实现预设选择器 UI
  - [ ] 预设下拉菜单 / 卡片网格
  - [ ] 预览预设效果
  - [ ] 应用预设
- [ ] **5.5** 实现自定义预设管理
  - [ ] 保存当前设置为新预设
  - [ ] 预设列表展示
  - [ ] 删除自定义预设
  - [ ] 预设命名

### Phase 6: 国际化

- [ ] **6.1** 添加中文翻译 (`zh.json`)
  - [ ] 侧边栏：`sidebar.audioEq`
  - [ ] 工具描述：`builtinTools.audioEqDesc`
  - [ ] EQ 页面所有文案
  - [ ] 预设名称
- [ ] **6.2** 添加英文翻译 (`en.json`)
- [ ] **6.3** 添加繁体中文翻译 (`zh-TW.json`)
- [ ] **6.4** 添加日文翻译 (`ja.json`)
- [ ] **6.5** 添加德文翻译 (`de.json`)
- [ ] **6.6** 添加法文翻译 (`fr.json`)

### Phase 7: 优化与完善

- [ ] **7.1** 音频延迟优化
  - [ ] WASAPI 事件驱动模式
  - [ ] 缓冲区大小调优
  - [ ] 延迟测量与显示
- [ ] **7.2** 实时音频频谱可视化 (可选)
  - [ ] FFT 频谱分析
  - [ ] Canvas 实时绘制频谱
- [ ] **7.3** 设备热插拔处理
  - [ ] 监听 `IMMNotificationClient` 设备变化事件
  - [ ] 自动恢复/重连
- [ ] **7.4** 异常处理与恢复
  - [ ] 驱动安装失败处理
  - [ ] 音频引擎崩溃恢复
  - [ ] 设备断开重连
- [ ] **7.5** 性能监控
  - [ ] CPU 占用显示
  - [ ] 延迟显示
  - [ ] 缓冲区使用率

### Phase 8: 测试与发布

- [ ] **8.1** 功能测试
  - [ ] 驱动安装/卸载测试 (多系统版本)
  - [ ] EQ 调音效果测试 (A/B 对比)
  - [ ] 预设切换测试
  - [ ] 设备切换测试
  - [ ] Loopback 捕获正确性测试
- [ ] **8.2** 兼容性测试
  - [ ] Windows 10 测试
  - [ ] Windows 11 测试
  - [ ] 不同声卡硬件测试 (Realtek / Creative / USB DAC)
  - [ ] 与其他音频软件共存测试 (FxSound / Voicemeeter / 系统音量混合器)
- [ ] **8.3** 性能测试
  - [ ] 延迟测量 (目标 < 50ms)
  - [ ] CPU 占用测量 (目标 < 5%)
  - [ ] 内存占用测量
  - [ ] 长时间运行稳定性测试
- [ ] **8.4** 用户体验优化
  - [ ] 安装流程简化
  - [ ] 错误提示优化
  - [ ] 帮助文档

---

## 八、关键技术参考

### 8.1 相关文档

- [FxSound 驱动源码](https://github.com/fxsound2/fxsound-driver)
- [FxSound 应用源码](https://github.com/fxsound2/fxsound-app) — 音频处理逻辑参考
- [MSVAD 示例文档](https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/sample-audio-drivers)
- [WASAPI 文档](https://learn.microsoft.com/en-us/windows/win32/coreaudio/wasapi)
- [WASAPI Loopback 捕获](https://learn.microsoft.com/en-us/windows/win32/coreaudio/capturing-a-stream) — 关键参考
- [SetupAPI 文档](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/setupapi)
- [Biquad 滤波器公式 (Audio EQ Cookbook)](https://www.w3.org/TR/audio-eq-cookbook/)
- [IPolicyConfig 接口参考](https://stackoverflow.com/questions/25808830/how-to-change-default-audio-playback-device-programmatically)

### 8.2 Rust 音频库

- **windows crate WASAPI** (推荐) — 直接使用 Windows API，已在依赖中 (v0.58)
  - 模块：`windows::Win32::Media::Audio`
  - 已包含 `Win32_Media_Audio` feature
- [cpal](https://github.com/RustAudio/cpal) — 跨平台音频库，但 Loopback 支持有限

### 8.3 驱动安装命令参考

```powershell
# 安装驱动
pnputil /add-driver fxvad.inf /install

# 查找已安装的驱动
pnputil /enum-drivers | findstr -i fxsound

# 删除驱动
pnputil /delete-driver oemXX.inf /uninstall /force

# 查看设备服务
sc query FXVAD

# 启动/停止服务
sc start FXVAD
sc stop FXVAD
```

### 8.4 WASAPI Loopback 关键 API

```rust
// 关键 Windows API (windows crate)
use windows::Win32::Media::Audio::{
    IMMDeviceEnumerator, MMDeviceEnumerator,
    IAudioClient, IAudioCaptureClient, IAudioRenderClient,
    AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_LOOPBACK,    // ← Loopback 标志
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, // 事件驱动模式
};
use windows::Win32::System::Com::{CoInitializeEx, CoCreateInstance, CLSCTX_ALL};
```

### 8.5 源码文件与关键定义对照表

| 源码文件 | 关键内容 | 对方案的影响 |
|----------|----------|------------|
| `pcmex.h` | `MAX_OUTPUT_STREAMS=0`, `MAX_INPUT_STREAMS=1` | 驱动仅支持渲染，需用 Loopback |
| `pcmex.h` | `MIN/MAX_BITS_PER_SAMPLE_PCM=24` | 音频引擎需处理 24-bit 格式 |
| `pcmex.h` | `MIN/MAX_SAMPLE_RATE=44100/48000` | 固定采样率范围 |
| `fxvad.inf` | 捕获接口被注释 | 确认 Render-Only |
| `mintopo.cpp` | `TopologyPhysicalConnections` 连接表 | 确认仅渲染路径连接 |
| `basedma.cpp` | `CopyFrom`/`CopyTo` 空实现 | 驱动不处理数据 |
| `savedata.cpp` | 所有方法 stub | 驱动不保存数据 |
| `msvad.h` | `DMA_BUFFER_SIZE=0x16000` | DMA 缓冲区 88KB |
| `basetopo.cpp` | Volume 范围 -96~0 dB | 拓扑节点属性参考 |
| `fxvad.rc` | 版本 14.1.0.0 | 驱动版本信息 |

---

## 九、开发优先级

```
Phase 1 (驱动管理)          ████░░░░░░  核心 — 最高优先级
Phase 2 (前端基础)          ████░░░░░░  核心 — 最高优先级
Phase 3 (WASAPI Loopback)   ████░░░░░░  核心 — 最高优先级
Phase 4 (EQ UI)            ███░░░░░░░  重要 — 高优先级
Phase 5 (预设管理)          ██░░░░░░░░  重要 — 中优先级
Phase 6 (国际化)           █░░░░░░░░░  必要 — 中优先级
Phase 7 (优化完善)          █░░░░░░░░░  增强 — 低优先级
Phase 8 (测试发布)          █░░░░░░░░░  必要 — 最后进行
```

**建议开发顺序：** Phase 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8

Phase 1-3 为 MVP（最小可行产品），完成后即可使用基本的 EQ 调音功能。
