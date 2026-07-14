# 系统优化重构方案 — 基于 aq_registry 注册表文件

> **致谢：感谢 1U 工具箱提供系统优化支持**

## 一、方案概述

### 1.1 目标

将当前系统优化页面（`SystemOptimizerPage`）的全部优化项替换为基于 `aq_registry` 注册表文件的优化方案。每个优化项对应一个 `.reg`（应用优化）和一个 `.restore.reg`（恢复默认），共 **67 项**。

### 1.2 核心变更

| 项目 | 变更前 | 变更后 |
|------|--------|--------|
| 优化项数量 | ~60 项（Rust 硬编码） | 67 项（注册表文件驱动） |
| 后端实现 | 每项独立 Rust 函数 + PowerShell 脚本 | 纯 Rust `winreg` crate 直接写注册表 |
| 恢复机制 | 每项独立反向 PowerShell 脚本 | 解析 `.restore.reg` 后用 `winreg` 恢复 |
| 打包方式 | 无（代码内嵌） | Tauri Resources 打包 .reg 文件 |
| 页面标注 | 无 | 「感谢 1U 工具箱提供系统优化支持」 |

---

## 二、注册表文件能否打包进 exe？

### 结论：✅ 可以打包

Tauri 的 `bundle.resources` 配置支持将任意文件打包进安装包。项目已有先例：

```json
"resources": [
  "../nvidiaProfileInspector.exe",
  "../power-plans/*",
  "../monitor/bin/Release/net48/*"
]
```

只需将 `aq_registry` 和 `aq_registry_restore` 目录加入 `resources`，打包后文件会放在 exe 同级目录或 `resources/` 子目录中，运行时通过 `std::env::current_exe()` 解析路径即可访问。

### 两种可选方案

#### 方案一：Tauri Resources 打包（✅ 推荐）

```json
"resources": [
  "../aq_registry/*",
  "../aq_registry_restore/*"
]
```

- **优点**：与现有打包方式一致；文件独立便于维护更新
- **缺点**：.reg 文件解压到磁盘，用户可见（但可接受）
- **运行时**：Rust 读取 .reg 文件内容 → 解析格式 → 通过 `winreg` crate 直接调用 Windows API 写注册表（零外部进程）

#### 方案二：Rust `include_str!` 嵌入

```rust
const TELEMETRY_REG: &str = include_str!("../aq_registry/禁用遥测服务.reg");
```

- **优点**：完全嵌入二进制，用户不可见
- **缺点**：需为 67 个文件逐一编写；运行时需写入临时文件再导入
- **适用**：对文件可见性有严格要求的场景

> **本方案采用方案一**，与项目现有资源管理风格保持一致。

---

## 三、优化项分类（67 项 → 8 大类）

### 分类总览

| 分类 | 数量 | 说明 |
|------|------|------|
| 🎮 游戏与图形优化 | 12 | DirectX、Game DVR、MMCSS 等 |
| 🟢 NVIDIA 显卡优化 | 11 | 延迟、时钟、电源、遥测等 |
| 🔴 AMD 显卡优化 | 1 | Shader Cache |
| ⚡ 系统性能调优 | 9 | 缓存、超时、延迟、NVMe 等 |
| 🔒 隐私与遥测 | 10 | 各类遥测和数据收集 |
| 🔧 系统服务精简 | 11 | 不常用服务禁用 |
| 💾 磁盘与文件系统 | 5 | NTFS、存储保留等 |
| 📱 应用与界面 | 4 | Edge、搜索、后台应用 |

---

### 3.1 🎮 游戏与图形优化（12 项）

| # | 文件名 | 说明 | 需重启 |
|---|--------|------|--------|
| 1 | `MMCSS完整游戏配置.reg` | MMCSS 游戏任务最高优先级、GPU 优先级 8、禁用懒模式 | ✅ |
| 2 | `优先考虑游戏责任.reg` | MMCSS Games 任务优先级提升 | ✅ |
| 3 | `改进网络和 SR 响应能力.reg` | 禁用网络节流、SystemResponsiveness=0 | ❌ |
| 4 | `启用DirectX AutoHDR.reg` | 启用 DirectX 自动高动态范围 | ❌ |
| 5 | `启用DirectX Flip Model.reg` | 强制 Flip Model 呈现，减少帧延迟 | ❌ |
| 6 | `启用DirectX VRR优化.reg` | 启用可变刷新率优化 | ❌ |
| 7 | `禁用DX最大化窗口模式.reg` | 全局禁用最大化窗口模式，避免 DWM 延迟 | ❌ |
| 8 | `禁用 GPU 抢占.reg` | 禁用 GPU 抢占，降低输入延迟 | ❌ |
| 9 | `禁用GameBar提示.reg` | 禁用 GameBar 控制器快捷键和启动面板 | ❌ |
| 10 | `禁用游戏硬盘录像机Game DVR.reg` | 禁用 Game DVR 后台录制 | ❌ |
| 11 | `禁用广播DVR服务.reg` | 禁用 BcastDVRUserService 服务 | ✅ |
| 12 | `关闭自动色彩管理.reg` | 关闭 DirectX 自动色彩管理 | ❌ |

### 3.2 🟢 NVIDIA 显卡优化（11 项）

| # | 文件名 | 说明 | 需重启 |
|---|--------|------|--------|
| 1 | `NVIDIA低延迟阈值优化.reg` | D3PC/F1/3D 延迟阈值设为最低 | ✅ |
| 2 | `启用NVIDIA Per-CPU DPC.reg` | GPU 中断分散到多核处理 | ✅ |
| 3 | `启用NVIDIA锐化.reg` | 启用 Image Sharpening 全局锐化 | ❌ |
| 4 | `禁用NVIDIA GPU电源管理.reg` | 关闭运行时电源管理和 ASPM | ✅ |
| 5 | `禁用NVIDIA HDCP.reg` | 禁用 HDCP 握手开销 | ❌ |
| 6 | `禁用NVIDIA写合并.reg` | 关闭 Write Combining，降低延迟 | ❌ |
| 7 | `禁用NVIDIA时钟门控.reg` | 禁用 BLCG/ELCG/ELPG/FSPG/SLCG | ✅ |
| 8 | `禁用NVIDIA遥测.reg` | 禁用 GeForce Experience 遥测 | ❌ |
| 9 | `禁用NVIDIA驱动日志.reg` | 关闭驱动内部日志 | ❌ |
| 10 | `锁定NVIDIA P-State 0.reg` | 锁定最高性能状态，防降频 | ✅ |
| 11 | `禁用Miracast和Overlay.reg` | 禁用 Miracast 投屏和驱动叠加层 | ✅ |

### 3.3 🔴 AMD 显卡优化（1 项）

| # | 文件名 | 说明 | 需重启 |
|---|--------|------|--------|
| 1 | `强制启用AMD Shader Cache.reg` | 强制开启着色器缓存 | ❌ |

### 3.4 ⚡ 系统性能调优（9 项）

| # | 文件名 | 说明 | 需重启 |
|---|--------|------|--------|
| 1 | `启用大系统缓存.reg` | LargeSystemCache=1，适合 16GB+ 内存 | ✅ |
| 2 | `启用Intel TSX.reg` | 启用事务性同步扩展（仅 Intel） | ✅ |
| 3 | `合并ServiceHost进程.reg` | 增大拆分阈值，减少 svchost 进程数 | ✅ |
| 4 | `缩短任务超时.reg` | HungAppTimeout=1000, WaitToKillApp=2000 | ❌ |
| 5 | `缩短服务超时.reg` | WaitToKillServiceTimeout=2000 | ❌ |
| 6 | `鼠标悬停延迟优化.reg` | MouseHoverTime 从 400ms 降到 10ms | ❌ |
| 7 | `禁用启动延迟.reg` | StartupDelayInMSec=0 | ❌ |
| 8 | `禁用省电模式.reg` | 禁用 PowerThrottling 和休眠 | ✅ |
| 9 | `NVMe调优.reg` | 连续内存分配、禁用日志和空闲节能 | ✅ |

### 3.5 🔒 隐私与遥测（10 项）

| # | 文件名 | 说明 | 需重启 |
|---|--------|------|--------|
| 1 | `禁用遥测服务.reg` | 禁用 DiagTrack 和 dmwappushservice | ✅ |
| 2 | `禁用CEIP-SQM.reg` | 禁用客户体验改善计划 | ❌ |
| 3 | `禁用DotNet遥测.reg` | DOTNET_CLI_TELEMETRY_OPTOUT=1 | ❌ |
| 4 | `禁用应用影响遥测.reg` | 禁用 AITEnable 策略 | ❌ |
| 5 | `禁用应用影响遥测代理.reg` | 禁用 AIT 代理 | ❌ |
| 6 | `禁用许可遥测.reg` | NoGenTicket=1 | ❌ |
| 7 | `禁用计划诊断.reg` | 禁用计划诊断执行 | ❌ |
| 8 | `禁用网络摄像头遥测.reg` | webcam ConsentStore=Deny | ❌ |
| 9 | `禁用写入反馈.reg` | 禁用手写输入反馈收集 | ❌ |
| 10 | `禁用Windows错误报告.reg` | 禁用 WER | ❌ |

### 3.6 🔧 系统服务精简（11 项）

| # | 文件名 | 说明 | 需重启 |
|---|--------|------|--------|
| 1 | `禁用传感器服务.reg` | 禁用 SensrSvc 和 SensorDataService | ✅ |
| 2 | `禁用传真服务.reg` | 禁用 Fax 服务 | ✅ |
| 3 | `禁用打印服务.reg` | 禁用打印后台处理程序 Spooler | ✅ |
| 4 | `禁用下载地图管理器.reg` | 禁用 MapsBroker 服务 | ✅ |
| 5 | `禁用UCPD.reg` | 禁用 User Choice Protection Driver | ✅ |
| 6 | `禁用DCOM.reg` | 禁用分布式组件对象模型 | ❌ |
| 7 | `禁用StorageSense.reg` | 禁用存储感知自动清理 | ❌ |
| 8 | `禁用自动维护.reg` | MaintenanceDisabled=1 | ❌ |
| 9 | `禁用应用程序兼容性.reg` | 禁用兼容性引擎和 PCA | ❌ |
| 10 | `禁用步骤记录器.reg` | DisableUAR=1 | ❌ |
| 11 | `禁用性能提醒.reg` | 禁用性能改进建议 | ❌ |

### 3.7 💾 磁盘与文件系统（5 项）

| # | 文件名 | 说明 | 需重启 |
|---|--------|------|--------|
| 1 | `禁用8.3文件名.reg` | NtfsDisable8dot3NameCreation=1 | ✅ |
| 2 | `禁用NTFS加密.reg` | NtfsDisableEncryption=1 | ❌ |
| 3 | `禁用最后访问更新.reg` | NtfsDisableLastAccessUpdate | ✅ |
| 4 | `禁用更新保留存储.reg` | 释放 ~7GB 保留空间 | ❌ |
| 5 | `禁用搜索全文件系统.reg` | 仅搜索索引位置 | ❌ |

### 3.8 📱 应用与界面（4 项）

| # | 文件名 | 说明 | 需重启 |
|---|--------|------|--------|
| 1 | `禁用后台应用.reg` | GlobalUserDisabled=1 | ❌ |
| 2 | `禁用Edge启动加速.reg` | 禁用 Edge 预启动和后台运行 | ❌ |
| 3 | `精简Edge广告推荐.reg` | 禁用侧边栏、广告、购物助手等 | ❌ |
| 4 | `禁用搜索WebView2.reg` | 禁用 Search WebView2 渲染 | ❌ |

---

## 四、技术实现方案

### 4.1 目录结构调整

```
NexBox/
├── aq_registry/              ← 优化 .reg 文件（67 个）
├── aq_registry_restore/      ← 恢复 .restore.reg 文件（67 个）
├── src/
│   ├── config/
│   │   └── system-optimizer.ts   ← 重写：注册表驱动配置
│   ├── pages/
│   │   └── SystemOptimizerPage.tsx  ← 修改：添加致谢标注
│   └── locales/
│       ├── zh.json           ← 更新：新增 67 项 i18n
│       ├── en.json           ← 更新
│       └── ja.json           ← 更新
└── src-tauri/
    ├── tauri.conf.json       ← 修改：resources 添加 .reg 文件
    └── src/
        ├── lib.rs            ← 修改：注册新命令
        └── optimization.rs   ← 重构：统一 reg 导入/恢复逻辑
```

### 4.2 Tauri 配置修改

**文件：`src-tauri/tauri.conf.json`**

```json
"bundle": {
  "resources": [
    "../nvidiaProfileInspector.exe",
    "../nvidiaProfileInspector.exe.config",
    "../Reference.xml",
    "../power-plans/*",
    "../monitor/bin/Release/net48/*",
    "../aq_registry/*",
    "../aq_registry_restore/*"
  ]
}
```

### 4.3 Rust 后端实现（纯 Rust `winreg`，零外部进程）

> **核心思路**：不用 `reg.exe`、不用 `powershell.exe`，直接在 Rust 里读取 `.reg` 文件内容，
> 解析其格式（键路径 / dword / string / 删除标记），然后通过 `winreg` crate 调用 Windows Registry API 直接写注册表。
>
> 项目已有依赖：`winreg = "0.52"`（`Cargo.toml`），且 `startup_manager.rs`、`nvapi.rs` 已有使用范例。

#### 4.3.1 性能对比

| 方案 | 单项耗时 | 67 项全开耗时 | 外部进程 | 临时文件 |
|------|---------|-------------|---------|---------|
| PowerShell（旧方案） | ~100-300ms | 10-30 秒 | powershell.exe | .ps1 |
| `reg.exe import` | ~30-50ms | 2-4 秒 | reg.exe | 无 |
| **纯 Rust `winreg`** | **<1ms** | **<100ms** | **无** | **无** |

#### 4.3.2 统一命令设计

替换原来 ~120 个独立命令，改为 **4 个通用命令**：

```rust
use winreg::enums::*;
use winreg::RegKey;
use std::fs;
use std::path::PathBuf;

/// 应用单个注册表优化
/// - 读取 `aq_registry/<name>.reg` 文件内容
/// - 解析 .reg 格式，通过 winreg 直接写注册表
#[tauri::command]
pub async fn apply_registry_tweak(name: String) -> Result<(), String> {
    let content = read_reg_file(&name, false)?;
    apply_reg_content(&content)
}

/// 恢复单个注册表优化
/// - 读取 `aq_registry_restore/<name>.restore.reg` 文件内容
/// - 解析 .reg 格式，通过 winreg 直接写注册表
#[tauri::command]
pub async fn restore_registry_tweak(name: String) -> Result<(), String> {
    let content = read_reg_file(&name, true)?;
    apply_reg_content(&content)
}

/// 批量应用所有优化
#[tauri::command]
pub async fn batch_apply_registry_tweaks(names: Vec<String>) -> Result<(), String> {
    for name in &names {
        let content = read_reg_file(name, false)?;
        apply_reg_content(&content)?;
    }
    Ok(())
}

/// 批量恢复所有优化
#[tauri::command]
pub async fn batch_restore_registry_tweaks(names: Vec<String>) -> Result<(), String> {
    for name in &names {
        let content = read_reg_file(name, true)?;
        apply_reg_content(&content)?;
    }
    Ok(())
}
```

#### 4.3.3 .reg 文件读取与路径解析

```rust
/// 读取 .reg 文件内容（自动处理 UTF-8 和 UTF-16LE 编码）
/// - is_restore: true → aq_registry_restore/<name>.restore.reg
/// - is_restore: false → aq_registry/<name>.reg
fn read_reg_file(name: &str, is_restore: bool) -> Result<String, String> {
    let path = resolve_reg_path(name, is_restore)?;
    let bytes = fs::read(&path)
        .map_err(|e| format!("读取注册表文件失败: {}", e))?;
    
    // 检测编码：UTF-16LE BOM (FF FE) 或 UTF-8 BOM (EF BB BF)
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        // UTF-16LE 编码（部分 .reg 文件使用此编码）
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        String::from_utf16_lossy(&u16s)
    } else if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        // UTF-8 BOM
        String::from_utf8_lossy(&bytes[3..]).to_string()
    } else {
        // 纯 UTF-8
        String::from_utf8_lossy(&bytes).to_string()
    }
}

/// 解析 .reg 文件路径
fn resolve_reg_path(name: &str, is_restore: bool) -> Result<PathBuf, String> {
    let exe_dir = std::env::current_exe()
        .map_err(|e| format!("获取程序路径失败: {}", e))?;
    let parent = exe_dir.parent().ok_or("无法获取父目录")?;

    let (dir, suffix) = if is_restore {
        ("aq_registry_restore", ".restore.reg")
    } else {
        ("aq_registry", ".reg")
    };

    // 尝试多个候选路径（适配不同打包模式）
    let candidates = [
        parent.join(dir).join(format!("{}{}", name, suffix)),
        parent.join("_up_").join(dir).join(format!("{}{}", name, suffix)),
        parent.join("resources").join(dir).join(format!("{}{}", name, suffix)),
    ];

    for path in &candidates {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    Err(format!("未找到注册表文件: {}{}", name, suffix))
}
```

#### 4.3.4 .reg 文件解析与 winreg 写入（核心）

```rust
/// 解析 .reg 文件内容并直接通过 winreg 写入注册表
/// 支持：dword / String / 删除值(-) / 创建键
fn apply_reg_content(content: &str) -> Result<(), String> {
    let mut current_key: Option<RegKey> = None;

    for line in content.lines() {
        let line = line.trim();

        // 跳过空行和注释
        if line.is_empty() || line.starts_with(';') || line.starts_with("Windows Registry Editor") {
            continue;
        }

        // [HKEY_LOCAL_MACHINE\SYSTEM\...] — 注册表键路径
        if line.starts_with('[') && line.ends_with(']') {
            let path = &line[1..line.len() - 1];
            current_key = Some(open_or_create_reg_key(path)?);
            continue;
        }

        // "ValueName"=dword:00000001 — DWORD 值
        // "ValueName"="string"        — 字符串值
        // "ValueName"=-                — 删除值
        if let Some(ref key) = current_key {
            if let Some(rest) = line.strip_prefix('"') {
                if let Some(eq_pos) = rest.find("\"=") {
                    let name = &rest[..eq_pos];
                    // 反转义 .reg 中的双引号 \"
                    let name = name.replace("\\\"", "\"");
                    let value = &rest[eq_pos + 2..];

                    if value.starts_with("dword:") {
                        // DWORD 值
                        let hex_str = &value[6..];
                        let val = u32::from_str_radix(hex_str, 16)
                            .map_err(|e| format!("解析 dword 值失败: {}", e))?;
                        key.set_value(&name, &val)
                            .map_err(|e| format!("写入注册表值失败: {}", e))?;
                    } else if value.starts_with('"') {
                        // 字符串值（去掉首尾引号，反转义）
                        let val = &value[1..value.len() - 1];
                        let val = val.replace("\\\"", "\"");
                        key.set_value(&name, &val)
                            .map_err(|e| format!("写入注册表值失败: {}", e))?;
                    } else if value == "-" {
                        // 删除值
                        let _ = key.delete_value(&name);
                    }
                    // hex: 格式（二进制值）暂不支持，aq_registry 中未使用
                }
            }
        }
    }

    Ok(())
}

/// 根据 .reg 文件中的路径打开或创建注册表键
/// 例如: HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Services\DiagTrack
fn open_or_create_reg_key(path: &str) -> Result<RegKey, String> {
    // 分割 root hive 和子路径
    let (root, subpath) = if let Some(sub) = path.strip_prefix("HKEY_LOCAL_MACHINE\\") {
        (RegKey::predef(HKEY_LOCAL_MACHINE), sub)
    } else if let Some(sub) = path.strip_prefix("HKEY_CURRENT_USER\\") {
        (RegKey::predef(HKEY_CURRENT_USER), sub)
    } else if let Some(sub) = path.strip_prefix("HKEY_CLASSES_ROOT\\") {
        (RegKey::predef(HKEY_CLASSES_ROOT), sub)
    } else if let Some(sub) = path.strip_prefix("HKEY_USERS\\") {
        (RegKey::predef(HKEY_USERS), sub)
    } else {
        return Err(format!("不支持的注册表根键: {}", path));
    };

    // create_subkey 会自动创建所有不存在的中间键
    let (key, _) = root
        .create_subkey(subpath)
        .map_err(|e| format!("创建注册表键失败: {} - {}", path, e))?;

    Ok(key)
}
```

#### 4.3.5 已有依赖确认

项目 `Cargo.toml` 中已有 `winreg = "0.52"` 依赖，无需新增任何 crate：

```toml
# src-tauri/src-tauri/Cargo.toml (已存在)
winreg = "0.52"
```

项目已有纯 Rust 注册表操作范例：
- `src-tauri/src/startup_manager.rs` — 使用 `RegKey::predef(HKEY_*)` 读写启动项
- `src-tauri/src/nvapi.rs` — 使用 `RegKey` 查询 NVIDIA 驱动注册表信息

> **管理员权限**：多数 .reg 文件修改 `HKEY_LOCAL_MACHINE`，需要管理员权限运行。
> 应用启动时已有提权逻辑，`winreg` 的 `create_subkey` / `set_value` 在管理员权限下可直接写入 HKLM。

### 4.4 前端配置文件重写

**文件：`src/config/system-optimizer.ts`**

```typescript
export type OptimizerCategory =
  | "gaming"        // 游戏与图形优化
  | "nvidia"        // NVIDIA 显卡优化
  | "amd"           // AMD 显卡优化
  | "performance"   // 系统性能调优
  | "privacy"       // 隐私与遥测
  | "services"      // 系统服务精简
  | "disk"          // 磁盘与文件系统
  | "apps";         // 应用与界面

export interface OptimizerItem {
  id: string;
  regName: string;          // .reg 文件名（不含扩展名）
  category: OptimizerCategory;
  icon: ComponentType<{ size?: number; strokeWidth?: number }>;
  color: string;
  titleKey: string;
  descKey: string;
  requiresReboot: boolean;
}

export const optimizerItems: OptimizerItem[] = [
  // === 游戏与图形优化 ===
  {
    id: "mmcssGameConfig",
    regName: "MMCSS完整游戏配置",
    category: "gaming",
    icon: Gamepad2,
    color: COLORS[0],
    titleKey: "systemOptimizer.gaming.mmcssGameConfig",
    descKey: "systemOptimizer.gaming.mmcssGameConfigDesc",
    requiresReboot: true,
  },
  // ... 其余 66 项
];

export const categoryLabels: Record<OptimizerCategory, string> = {
  gaming: "systemOptimizer.category.gaming",
  nvidia: "systemOptimizer.category.nvidia",
  amd: "systemOptimizer.category.amd",
  performance: "systemOptimizer.category.performance",
  privacy: "systemOptimizer.category.privacy",
  services: "systemOptimizer.category.services",
  disk: "systemOptimizer.category.disk",
  apps: "systemOptimizer.category.apps",
};

export const categoryOrder: OptimizerCategory[] = [
  "gaming",
  "nvidia",
  "amd",
  "performance",
  "privacy",
  "services",
  "disk",
  "apps",
];
```

### 4.5 前端页面修改

**文件：`src/pages/SystemOptimizerPage.tsx`**

#### 4.5.1 命令调用变更

```typescript
// 单个优化项开关
const toggleItem = useCallback(
  async (item: OptimizerItem, enable: boolean) => {
    const cmd = enable ? "apply_registry_tweak" : "restore_registry_tweak";
    setTogglingItems((prev) => new Set(prev).add(item.id));
    try {
      await invoke(cmd, { name: item.regName });
      // ... 更新状态
    } catch (err) {
      // ... 错误处理
    }
  },
  [],
);

// 批量优化
const handleBatchEnable = useCallback(async () => {
  const names = optimizerItems.map((item) => item.regName);
  await invoke("batch_apply_registry_tweaks", { names });
  // ...
}, []);
```

#### 4.5.2 添加致谢标注

在页面底部添加：

```tsx
{/* 致谢标注 */}
<Box w="full" textAlign="center" mt={2}>
  <Text fontSize="xs" color={subTextColor}>
    {t("systemOptimizer.credits")}
  </Text>
</Box>
```

对应 i18n：
```json
{
  "systemOptimizer": {
    "credits": "感谢 1U 工具箱提供系统优化支持"
  }
}
```

#### 4.5.3 状态扫描变更

由于 .reg 文件方案无法像之前那样精确"扫描"每项的当前状态，改为：
- 初始化时读取 `store` 中保存的用户操作记录
- 未操作过的项默认显示为关闭
- 用户操作后即时更新并持久化

### 4.6 i18n 国际化

需要在 `zh.json`、`en.json`、`ja.json` 中为 67 项优化添加标题和描述。

**命名规则**：`systemOptimizer.<category>.<itemId>` 和 `systemOptimizer.<category>.<itemId>Desc`

示例（中文）：
```json
{
  "systemOptimizer": {
    "category": {
      "gaming": "游戏与图形优化",
      "nvidia": "NVIDIA 显卡优化",
      "amd": "AMD 显卡优化",
      "performance": "系统性能调优",
      "privacy": "隐私与遥测",
      "services": "系统服务精简",
      "disk": "磁盘与文件系统",
      "apps": "应用与界面"
    },
    "credits": "感谢 1U 工具箱提供系统优化支持",
    "gaming": {
      "mmcssGameConfig": "MMCSS 完整游戏配置",
      "mmcssGameConfigDesc": "设置游戏任务最高优先级、GPU 优先级 8、禁用懒模式",
      "gamePriority": "优先考虑游戏责任",
      "gamePriorityDesc": "提升 MMCSS Games 任务优先级，确保游戏获得更多 CPU 资源",
      // ...
    },
    "nvidia": {
      "lowLatencyThreshold": "NVIDIA 低延迟阈值优化",
      "lowLatencyThresholdDesc": "调整 GPU 调度器各项延迟阈值到最低值，减少输入延迟",
      // ...
    }
    // ... 其余分类
  }
}
```

---

## 五、实施步骤

### 阶段一：后端重构（预计 2-3 小时）

1. **修改 `tauri.conf.json`**：添加 `aq_registry/*` 和 `aq_registry_restore/*` 到 resources
2. **重写 `optimization.rs`**：
   - 删除原有 ~120 个独立优化命令（PowerShell 脚本）
   - 实现 `.reg` 文件解析器（支持 dword / string / 删除值 / UTF-8 / UTF-16LE 编码）
   - 实现 `apply_registry_tweak`、`restore_registry_tweak`、`batch_apply_registry_tweaks`、`batch_restore_registry_tweaks` 四个通用命令
   - 全部使用 `winreg` crate 直接操作注册表，零外部进程
3. **修改 `lib.rs`**：更新命令注册，移除旧命令，添加新命令

### 阶段二：前端配置重写（预计 1-2 小时）

1. **重写 `system-optimizer.ts`**：
   - 更新 `OptimizerCategory` 类型（8 个分类）
   - 更新 `OptimizerItem` 接口（`regName` 替代 `enableCmd`/`disableCmd`/`stateKey`）
   - 编写 67 项完整配置
2. **更新 i18n 文件**：
   - `zh.json`：67 项中文标题和描述 + 分类标签 + 致谢文案
   - `en.json`：67 项英文翻译
   - `ja.json`：67 项日文翻译

### 阶段三：页面适配（预计 1 小时）

1. **修改 `SystemOptimizerPage.tsx`**：
   - 更新 `toggleItem`：调用 `apply_registry_tweak` / `restore_registry_tweak`
   - 更新 `handleBatchEnable`：调用 `batch_apply_registry_tweaks`
   - 更新 `handleBatchDisable`：遍历调用 `restore_registry_tweak`
   - 简化初始化逻辑：移除 `check_all_tweak_states`，改为纯 store 读取
   - 添加底部致谢标注「感谢 1U 工具箱提供系统优化支持」

### 阶段四：测试与验证（预计 1 小时）

1. 开发模式测试：每个分类抽 1-2 项验证应用/恢复
2. 打包测试：确认 .reg 文件正确打包到 exe 目录
3. 路径解析测试：验证不同安装路径下的文件查找
4. 编码测试：验证 UTF-8 和 UTF-16LE 编码的 .reg 文件均能正确解析
5. 权限测试：确认管理员权限下 `winreg` 写入 HKLM 正常
6. 性能测试：批量应用 67 项，确认 <100ms 完成

---

## 六、清理工作

### 6.1 删除旧代码

- `optimization.rs` 中原有 ~120 个 `enable_xxx` / `disable_xxx` 命令
- `lib.rs` 中对应的命令注册
- `system-optimizer.ts` 中旧的 `enableCmd` / `disableCmd` / `stateKey` 字段
- i18n 文件中旧的优化项文案（保留分类标签结构，更新内容）

### 6.2 保留的代码

- `optimization.rs` 中的电源计划相关命令（`get_system_power_plans` 等）
- `optimization.rs` 中的外设设置相关命令（`get_peripheral_status` 等）
- `optimization.rs` 中的 Windows Update 开关命令（如保留）
- `network_optimize` 模块（网络优化独立模块，不在本次重构范围）

---

## 七、风险与注意事项

### 7.1 管理员权限

多数 .reg 文件修改 `HKEY_LOCAL_MACHINE`，需要管理员权限。确保：
- 应用以管理员身份运行
- `winreg` 的 `create_subkey` / `set_value` 在管理员权限下可直接写入 HKLM，无需额外提权

### 7.2 文件编码问题

部分 .reg 文件（如 `禁用游戏硬盘录像机Game DVR.reg`、`优先考虑游戏责任.reg`）使用 **UTF-16LE BOM** 编码（Windows Registry Editor 标准编码），其余使用 UTF-8。

纯 Rust 解析方案通过 BOM 检测自动处理两种编码：
- `FF FE` → UTF-16LE 解码（`String::from_utf16_lossy`）
- `EF BB BF` → UTF-8 BOM 跳过前 3 字节
- 其他 → 纯 UTF-8 解码

### 7.3 状态追踪

与旧方案不同，.reg 文件方案无法精确"扫描"当前注册表状态。采用方案：
- 使用 `tauri-plugin-store` 持久化用户操作记录
- 用户开启过的项标记为 `true`，关闭后标记为 `false`
- 首次进入页面时所有项默认为关闭状态

### 7.4 文件名包含中文

.reg 文件名包含中文字符，在 Rust 中处理路径时需注意：
- `PathBuf` 自动处理 Unicode 路径，原生支持中文
- 前端传递 `regName` 时确保编码正确（Tauri IPC 使用 JSON，原生支持 Unicode）

### 7.5 .reg 格式解析覆盖度

当前解析器支持 `.reg` 文件中出现的所有格式：
- `[HKEY_...]` — 键路径（自动创建）
- `"Name"=dword:XXXXXXXX` — DWORD 值
- `"Name"="string"` — 字符串值
- `"Name"=-` — 删除值
- `; comment` — 注释行（跳过）

aq_registry 中的 67 个文件均未使用 `hex:` 二进制格式，无需额外支持。

### 7.5 兼容性提示

部分优化项有硬件/软件前提条件：
- **NVIDIA 优化**：仅对 NVIDIA 显卡有效
- **AMD Shader Cache**：仅对 AMD 显卡有效
- **Intel TSX**：仅对支持 TSX 的 Intel CPU 有效
- **禁用省电模式**：不建议笔记本用户使用（会显著缩短续航）

建议在 UI 上为这些项添加条件提示标签。

---

## 八、完整优化项 ID 映射表

> 以下为 `regName`（.reg 文件名，不含扩展名）与 i18n key 的完整映射。

### 游戏与图形优化

| regName | i18n key | 中文名 |
|---------|----------|--------|
| MMCSS完整游戏配置 | gaming.mmcssGameConfig | MMCSS 完整游戏配置 |
| 优先考虑游戏责任 | gaming.gamePriority | 优先考虑游戏责任 |
| 改进网络和 SR 响应能力 | gaming.networkSRResponse | 改进网络和 SR 响应能力 |
| 启用DirectX AutoHDR | gaming.directXAutoHDR | 启用 DirectX AutoHDR |
| 启用DirectX Flip Model | gaming.directXFlipModel | 启用 DirectX Flip Model |
| 启用DirectX VRR优化 | gaming.directXVRR | 启用 DirectX VRR 优化 |
| 禁用DX最大化窗口模式 | gaming.disableDXMaxWindow | 禁用 DX 最大化窗口模式 |
| 禁用 GPU 抢占 | gaming.disableGPUPreemption | 禁用 GPU 抢占 |
| 禁用GameBar提示 | gaming.disableGameBar | 禁用 GameBar 提示 |
| 禁用游戏硬盘录像机Game DVR | gaming.disableGameDVR | 禁用 Game DVR |
| 禁用广播DVR服务 | gaming.disableBcastDVR | 禁用广播 DVR 服务 |
| 关闭自动色彩管理 | gaming.disableAutoColorMgmt | 关闭自动色彩管理 |

### NVIDIA 显卡优化

| regName | i18n key | 中文名 |
|---------|----------|--------|
| NVIDIA低延迟阈值优化 | nvidia.lowLatencyThreshold | NVIDIA 低延迟阈值优化 |
| 启用NVIDIA Per-CPU DPC | nvidia.perCpuDPC | 启用 NVIDIA Per-CPU DPC |
| 启用NVIDIA锐化 | nvidia.imageSharpening | 启用 NVIDIA 锐化 |
| 禁用NVIDIA GPU电源管理 | nvidia.disableGpuPowerMgmt | 禁用 NVIDIA GPU 电源管理 |
| 禁用NVIDIA HDCP | nvidia.disableHDCP | 禁用 NVIDIA HDCP |
| 禁用NVIDIA写合并 | nvidia.disableWriteCombining | 禁用 NVIDIA 写合并 |
| 禁用NVIDIA时钟门控 | nvidia.disableClockGating | 禁用 NVIDIA 时钟门控 |
| 禁用NVIDIA遥测 | nvidia.disableTelemetry | 禁用 NVIDIA 遥测 |
| 禁用NVIDIA驱动日志 | nvidia.disableDriverLog | 禁用 NVIDIA 驱动日志 |
| 锁定NVIDIA P-State 0 | nvidia.lockPState0 | 锁定 NVIDIA P-State 0 |
| 禁用Miracast和Overlay | nvidia.disableMiracastOverlay | 禁用 Miracast 和 Overlay |

### AMD 显卡优化

| regName | i18n key | 中文名 |
|---------|----------|--------|
| 强制启用AMD Shader Cache | amd.shaderCache | 强制启用 AMD Shader Cache |

### 系统性能调优

| regName | i18n key | 中文名 |
|---------|----------|--------|
| 启用大系统缓存 | performance.largeSysCache | 启用大系统缓存 |
| 启用Intel TSX | performance.intelTSX | 启用 Intel TSX |
| 合并ServiceHost进程 | performance.mergeSvcHost | 合并 ServiceHost 进程 |
| 缩短任务超时 | performance.shortenTaskTimeout | 缩短任务超时 |
| 缩短服务超时 | performance.shortenServiceTimeout | 缩短服务超时 |
| 鼠标悬停延迟优化 | performance.mouseHoverDelay | 鼠标悬停延迟优化 |
| 禁用启动延迟 | performance.disableStartupDelay | 禁用启动延迟 |
| 禁用省电模式 | performance.disablePowerSaving | 禁用省电模式 |
| NVMe调优 | performance.nvmeTuning | NVMe 调优 |

### 隐私与遥测

| regName | i18n key | 中文名 |
|---------|----------|--------|
| 禁用遥测服务 | privacy.disableTelemetrySvc | 禁用遥测服务 |
| 禁用CEIP-SQM | privacy.disableCEIP | 禁用 CEIP-SQM |
| 禁用DotNet遥测 | privacy.disableDotNetTelemetry | 禁用 DotNet 遥测 |
| 禁用应用影响遥测 | privacy.disableAppImpactTelemetry | 禁用应用影响遥测 |
| 禁用应用影响遥测代理 | privacy.disableAppImpactTelemetryAgent | 禁用应用影响遥测代理 |
| 禁用许可遥测 | privacy.disableLicenseTelemetry | 禁用许可遥测 |
| 禁用计划诊断 | privacy.disableScheduledDiag | 禁用计划诊断 |
| 禁用网络摄像头遥测 | privacy.disableWebcamTelemetry | 禁用网络摄像头遥测 |
| 禁用写入反馈 | privacy.disableWritingFeedback | 禁用写入反馈 |
| 禁用Windows错误报告 | privacy.disableErrorReporting | 禁用 Windows 错误报告 |

### 系统服务精简

| regName | i18n key | 中文名 |
|---------|----------|--------|
| 禁用传感器服务 | services.disableSensorSvc | 禁用传感器服务 |
| 禁用传真服务 | services.disableFaxSvc | 禁用传真服务 |
| 禁用打印服务 | services.disablePrintSvc | 禁用打印服务 |
| 禁用下载地图管理器 | services.disableMapsBroker | 禁用下载地图管理器 |
| 禁用UCPD | services.disableUCPD | 禁用 UCPD |
| 禁用DCOM | services.disableDCOM | 禁用 DCOM |
| 禁用StorageSense | services.disableStorageSense | 禁用 StorageSense |
| 禁用自动维护 | services.disableAutoMaintenance | 禁用自动维护 |
| 禁用应用程序兼容性 | services.disableAppCompat | 禁用应用程序兼容性 |
| 禁用步骤记录器 | services.disableStepRecorder | 禁用步骤记录器 |
| 禁用性能提醒 | services.disablePerfTips | 禁用性能提醒 |

### 磁盘与文件系统

| regName | i18n key | 中文名 |
|---------|----------|--------|
| 禁用8.3文件名 | disk.disable8dot3 | 禁用 8.3 文件名 |
| 禁用NTFS加密 | disk.disableNtfsEncryption | 禁用 NTFS 加密 |
| 禁用最后访问更新 | disk.disableLastAccess | 禁用最后访问更新 |
| 禁用更新保留存储 | disk.disableReservedStorage | 禁用更新保留存储 |
| 禁用搜索全文件系统 | disk.disableFullFsSearch | 禁用搜索全文件系统 |

### 应用与界面

| regName | i18n key | 中文名 |
|---------|----------|--------|
| 禁用后台应用 | apps.disableBackgroundApps | 禁用后台应用 |
| 禁用Edge启动加速 | apps.disableEdgeStartupBoost | 禁用 Edge 启动加速 |
| 精简Edge广告推荐 | apps.slimEdgeAds | 精简 Edge 广告推荐 |
| 禁用搜索WebView2 | apps.disableSearchWebView2 | 禁用搜索 WebView2 |

---

## 九、预期效果

### 打包后目录结构

```
NexBox.exe
├── aq_registry/
│   ├── MMCSS完整游戏配置.reg
│   ├── NVIDIA低延迟阈值优化.reg
│   └── ... (共 67 个)
├── aq_registry_restore/
│   ├── MMCSS完整游戏配置.restore.reg
│   ├── NVIDIA低延迟阈值优化.restore.reg
│   └── ... (共 67 个)
├── nvidiaProfileInspector.exe
├── power-plans/
└── monitor/
```

### 页面效果

```
┌─────────────────────────────────────────┐
│  ← 系统优化          [全部优化] [全部恢复] │
│                                         │
│  ▎游戏与图形优化                         │
│  ┌─────────────┐  ┌─────────────┐       │
│  │ MMCSS 完整... │  │ DirectX...  │       │
│  └─────────────┘  └─────────────┘       │
│                                         │
│  ▎NVIDIA 显卡优化                       │
│  ┌─────────────┐  ┌─────────────┐       │
│  │ 低延迟阈值... │  │ Per-CPU...  │       │
│  └─────────────┘  └─────────────┘       │
│                                         │
│  ...                                    │
│                                         │
│        感谢 1U 工具箱提供系统优化支持      │
└─────────────────────────────────────────┘
```

---

## 十、总结

| 指标 | 数据 |
|------|------|
| 优化项总数 | 67 项 |
| 分类数 | 8 大类 |
| 后端命令数 | 4 个（从 ~120 个精简） |
| 后端实现 | 纯 Rust `winreg` crate，零外部进程 |
| 批量 67 项耗时 | <100ms（旧方案 PowerShell 需 10-30 秒） |
| .reg 文件总数 | 134 个（67 优化 + 67 恢复） |
| 预计开发时间 | 5-7 小时 |
| 打包方式 | Tauri Resources（.reg 文件打包进 exe 目录） |
| 致谢 | 感谢 1U 工具箱提供系统优化支持 |
