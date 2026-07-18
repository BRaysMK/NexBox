# NexBox 竖排悬浮框（Tauri 窗口方案）开发计划

> **文档版本**: v1.0  
> **创建日期**: 2026-07-16  
> **方案类型**: 新增悬浮框样式 — 基于 Tauri Webview 窗口  
> **核心需求**: 所有项竖向排列、完整显示项名、带图标

---

## 一、背景与现状分析

### 1.1 现有悬浮框架构

当前 NexBox 的悬浮框（Overlay Panel）采用 **纯 Win32 API + GDI/GDI+** 原生渲染方案：

| 文件 | 作用 |
|------|------|
| `src-tauri/src/overlay_panel.rs` | 核心逻辑：窗口创建、GDI+ 绘制、硬件数据采集、定时刷新 |
| `src/pages/OverlayPanelPage.tsx` | 前端设置页面：样式选择、显示项配置、不透明度等 |
| `src-tauri/tauri.conf.json` | Tauri 窗口配置 |

现有两种样式：
- **`default`** — 水平条形，所有项横向排列，居中屏幕顶部
- **`dynamic_island`** — 灵动岛风格，圆角水平条形，屏幕顶部

#### 现有样式特点与局限

| 特性 | default | dynamic_island |
|------|---------|----------------|
| 排列方向 | 水平 | 水平 |
| 窗口高度 | 28px | 36px |
| 项名显示 | 完整显示 | 完整显示 |
| 图标 | ❌ 无 | ❌ 无 |
| 圆角 | ❌ 无 | ✅ 有 |
| 渲染技术 | GDI+ | GDI+ |
| 自定义颜色 | ✅ 按数值变色 | ✅ 按数值变色 |

**核心局限**：
1. 仅支持水平排列，项数多时窗口过宽，遮挡游戏画面
2. 无图标支持（GDI+ 绘制图标成本高、效果差）
3. 样式定制困难（GDI+ 代码冗长，修改成本高）
4. 无法使用 CSS 动画、毛玻璃等现代 UI 效果

### 1.2 现有 Tauri 窗口资源

项目已有多个 Tauri Webview 窗口，可作为技术参考：

| 窗口 label | 尺寸 | 特性 | 用途 |
|------------|------|------|------|
| `main` | 1230×771 | 可调整大小 | 主窗口 |
| `widget` | 260×42 | 透明、置顶、跳过任务栏 | 小组件 |
| `tray-menu` | 190×140 | 透明、跳过任务栏 | 托盘菜单 |
| `desktop-lyrics` | 800×200 | 透明、置顶、跳过任务栏 | 桌面歌词 |
| `lyrics-unlock-btn` | 48×48 | 透明、置顶、跳过任务栏 | 歌词解锁按钮 |

这些窗口已验证 Tauri 透明置顶窗口方案的可行性，尤其是 `desktop-lyrics` 和 `widget` 窗口，与目标竖排悬浮框的需求高度吻合。

### 1.3 硬件数据流

```
Rust 后端 (collect_hardware_data)
    ├── game_fps::get_cached_fps()        → FPS
    ├── game_ping::get_cached_ping()      → 游戏延迟
    ├── delta_force::get_cached_delta_password() → 三角洲密码
    ├── sensor 模块                       → CPU/GPU 温度、占用等
    └── netease_lyrics 模块               → 网易云歌词
         │
         ▼
    CURRENT_HARDWARE_DATA (Mutex<Option<OverlayHardwareData>>)
         │
         ├── [Win32 方案] WM_TIMER (100ms) → draw_overlay_content()
         └── [Tauri 方案] app.emit("overlay-hardware-data", data) → 前端监听
```

---

## 二、方案概述

### 2.1 核心思路

新增第三种悬浮框样式 **`vertical_panel`**（竖排面板），采用 **Tauri Webview 窗口 + React 前端渲染** 替代 GDI+ 绘制：

```
┌─────────────────────────┐
│  🎮 FPS          144    │
│  🌡️ CPU温度      65°C   │
│  ⚡ CPU占用      45%    │
│  🔧 CPU频率      4200MHz │
│  🌡️ GPU温度      72°C   │
│  ⚡ GPU占用      88%    │
│  💾 内存占用     12.4GB  │
│  📶 游戏延迟     23ms   │
└─────────────────────────┘
```

### 2.2 方案优势

| 对比项 | Win32 GDI+ 方案 | Tauri 窗口方案（本方案） |
|--------|----------------|------------------------|
| 开发效率 | 低（GDI+ API 冗长） | **高**（React + CSS） |
| 图标支持 | ❌ 困难 | **✅ 原生支持**（SVG/Icon） |
| 样式定制 | 困难 | **灵活**（CSS/Tailwind） |
| 动画效果 | ❌ 无 | **✅ 支持**（CSS transition） |
| 字体渲染 | GDI 清晰度一般 | **WebView 清晰度高** |
| 性能开销 | 极低 | 中等（WebView 进程） |
| 透明度控制 | 像素级 | **窗口级 + CSS** |
| 跨平台潜力 | ❌ Windows 专用 | **✅ 可扩展** |

### 2.3 与现有系统的关系

- **并存而非替代**：`default` 和 `dynamic_island` 样式保留不变，`vertical_panel` 作为第三种选项
- **共享数据源**：复用现有 `collect_hardware_data()` 和 `OverlayHardwareData` 结构
- **共享设置结构**：复用 `OverlaySettings`，新增 `style: "vertical_panel"` 取值
- **独立窗口**：新建 Tauri 窗口 `vertical-overlay`，与现有 Win32 overlay 窗口互斥运行

---

## 三、技术架构设计

### 3.1 整体架构

```
┌─────────────────────────────────────────────────┐
│                   Rust 后端                      │
│                                                  │
│  ┌──────────────┐     ┌───────────────────────┐ │
│  │ HardwareData  │     │  vertical_overlay.rs  │ │
│  │ Poller (1s)   │────▶│  (新增模块)            │ │
│  │ (已有)        │     │  - 创建/销毁 Tauri 窗口 │ │
│  └──────────────┘     │  - emit 硬件数据事件    │ │
│                       │  - 接收前端拖动位置     │ │
│                       └───────────┬───────────┘ │
│                                   │ emit        │
│                                   ▼             │
│  ┌──────────────────────────────────────────────┐│
│  │         Tauri Webview Window                  ││
│  │         (label: "vertical-overlay")           ││
│  │         transparent + alwaysOnTop             ││
│  │                                               ││
│  │  ┌─────────────────────────────────────────┐  ││
│  │  │         React 前端 (VerticalOverlay)     │  ││
│  │  │                                         │  ││
│  │  │  listen("overlay-hardware-data") ──────▶│  ││
│  │  │                                         │  ││
│  │  │  ┌─────┐ ┌──────┐ ┌──────┐             │  ││
│  │  │  │Icon │ │Label │ │Value │  ← 每行     │  ││
│  │  │  └─────┘ └──────┘ └──────┘             │  ││
│  │  │  ┌─────┐ ┌──────┐ ┌──────┐             │  ││
│  │  │  │Icon │ │Label │ │Value │  ← 每行     │  ││
│  │  │  └─────┘ └──────┘ └──────┘             │  ││
│  │  │  ...                                    │  ││
│  │  └─────────────────────────────────────────┘  ││
│  └──────────────────────────────────────────────┘│
└─────────────────────────────────────────────────┘
```

### 3.2 窗口配置

在 `tauri.conf.json` 的 `app.windows` 数组中新增窗口：

```json
{
  "title": "NexBox Vertical Overlay",
  "width": 220,
  "height": 400,
  "resizable": false,
  "decorations": false,
  "transparent": true,
  "alwaysOnTop": true,
  "visible": false,
  "maximized": false,
  "maximizable": false,
  "skipTaskbar": true,
  "shadow": false,
  "label": "vertical-overlay",
  "url": "/vertical-overlay"
}
```

**窗口参数说明**：
- `width: 220` — 固定宽度，足够显示"GPU风扇转速"等长项名
- `height: 400` — 初始高度，运行时根据启用项数量动态调整
- `transparent: true` — 透明背景，实现无边框悬浮效果
- `alwaysOnTop: true` — 置顶显示
- `skipTaskbar: true` — 不在任务栏显示
- `shadow: false` — 无窗口阴影（由 CSS 控制）
- `visible: false` — 启动时隐藏，由后端控制显示

### 3.3 权限配置

在 `src-tauri/capabilities/default.json` 的 `windows` 数组中添加：

```json
{
  "windows": [
    "main",
    "widget",
    "tray-menu",
    "netease-login",
    "desktop-lyrics",
    "vertical-overlay"
  ]
}
```

### 3.4 数据流设计

```
┌─────────────┐         ┌──────────────────┐         ┌─────────────────┐
│  Rust 后端   │         │  Tauri 事件系统    │         │  React 前端      │
│             │         │                  │         │                 │
│  硬件数据    │────────▶│  emit            │────────▶│  listen          │
│  采集(1s)   │         │  "overlay-       │         │  "overlay-       │
│             │         │   hardware-data" │         │   hardware-data" │
└─────────────┘         └──────────────────┘         └─────────────────┘
                                                              │
                                                              ▼
┌─────────────┐         ┌──────────────────┐         ┌─────────────────┐
│  Rust 后端   │         │  Tauri 事件系统    │         │  React 前端      │
│             │         │                  │         │                 │
│  保存位置    │◀────────│  invoke          │◀────────│  拖动结束        │
│             │         │  save_overlay_   │         │  getCurrentPos  │
│             │         │  position        │         │  + invoke       │
└─────────────┘         └──────────────────┘         └─────────────────┘
```

**关键事件**：

| 事件/命令 | 方向 | 数据 | 说明 |
|-----------|------|------|------|
| `overlay-hardware-data` | Rust → 前端 | `OverlayHardwareData` | 每秒推送硬件数据 |
| `overlay-settings-updated` | Rust → 前端 | `OverlaySettings` | 设置变更通知 |
| `save_overlay_position` | 前端 → Rust | `{x, y}` | 保存拖动后的位置 |
| `set_overlay_click_through` | 前端 → Rust | `bool` | 设置鼠标穿透 |
| `start_vertical_overlay` | 前端 → Rust | `Option<OverlaySettings>` | 启动竖排悬浮框 |
| `stop_vertical_overlay` | 前端 → Rust | — | 关闭竖排悬浮框 |

---

## 四、详细设计

### 4.1 Rust 后端新增模块

#### 4.1.1 新增文件：`src-tauri/src/vertical_overlay.rs`

```rust
//! 竖排悬浮框模块 — 基于 Tauri Webview 窗口

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::{Emitter, Manager, AppHandle};

static VERTICAL_OVERLAY_ACTIVE: AtomicBool = AtomicBool::new(false);
static VERTICAL_OVERLAY_HANDLE: Mutex<Option<AppHandle>> = Mutex::new(None);

/// 启动竖排悬浮框
#[tauri::command]
pub async fn start_vertical_overlay(
    app_handle: tauri::AppHandle,
    settings: Option<crate::overlay_panel::OverlaySettings>,
) -> Result<crate::overlay_panel::OverlayResult, String> {
    // 1. 检查是否已启用
    if VERTICAL_OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        return Ok(crate::overlay_panel::OverlayResult {
            success: true,
            message: "竖排悬浮框已处于启用状态".to_string(),
        });
    }

    // 2. 如果 Win32 overlay 正在运行，先停止
    if crate::overlay_panel::is_overlay_active() {
        crate::overlay_panel::stop_overlay()?;
        std::thread::sleep(Duration::from_millis(200));
    }

    // 3. 标记激活
    VERTICAL_OVERLAY_ACTIVE.store(true, Ordering::SeqCst);
    *VERTICAL_OVERLAY_HANDLE.lock().unwrap() = Some(app_handle.clone());

    // 4. 显示 Tauri 窗口
    if let Some(window) = app_handle.get_webview_window("vertical-overlay") {
        // 恢复保存的位置或使用默认位置
        let settings = settings.unwrap_or_default();
        if let (Some(x), Some(y)) = (settings.position_x, settings.position_y) {
            let _ = window.set_position(tauri::PhysicalPosition { x, y });
        } else {
            // 默认：屏幕右上角
            // TODO: 根据屏幕尺寸计算位置
        }
        let _ = window.show();
        let _ = window.set_always_on_top(true);

        // 5. 启动数据推送线程
        let handle_clone = app_handle.clone();
        thread::spawn(move || {
            while VERTICAL_OVERLAY_ACTIVE.load(Ordering::SeqCst) {
                let data = crate::overlay_panel::collect_hardware_data();
                let _ = handle_clone.emit("overlay-hardware-data", &data);

                // 同时更新 CURRENT_HARDWARE_DATA 供报告使用
                *crate::overlay_panel::CURRENT_HARDWARE_DATA.lock().unwrap() = Some(data);

                thread::sleep(Duration::from_millis(1000));
            }
        });

        let _ = app_handle.emit("overlay-status-changed", ());
        Ok(crate::overlay_panel::OverlayResult {
            success: true,
            message: "竖排悬浮框已启动".to_string(),
        })
    } else {
        VERTICAL_OVERLAY_ACTIVE.store(false, Ordering::SeqCst);
        Err("找不到 vertical-overlay 窗口".to_string())
    }
}

/// 停止竖排悬浮框
#[tauri::command]
pub async fn stop_vertical_overlay(
    app_handle: tauri::AppHandle,
) -> Result<crate::overlay_panel::OverlayResult, String> {
    if !VERTICAL_OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        return Ok(crate::overlay_panel::OverlayResult {
            success: true,
            message: "竖排悬浮框已处于关闭状态".to_string(),
        });
    }

    VERTICAL_OVERLAY_ACTIVE.store(false, Ordering::SeqCst);

    if let Some(window) = app_handle.get_webview_window("vertical-overlay") {
        let _ = window.hide();
    }

    let _ = app_handle.emit("overlay-status-changed", ());
    Ok(crate::overlay_panel::OverlayResult {
        success: true,
        message: "竖排悬浮框已关闭".to_string(),
    })
}

/// 保存悬浮框位置
#[tauri::command]
pub async fn save_vertical_overlay_position(
    x: i32,
    y: i32,
) -> Result<crate::overlay_panel::OverlayResult, String> {
    let mut settings_lock = crate::overlay_panel::CURRENT_SETTINGS.lock().unwrap();
    if let Some(ref mut settings) = *settings_lock {
        settings.position_x = Some(x);
        settings.position_y = Some(y);
    }
    Ok(crate::overlay_panel::OverlayResult {
        success: true,
        message: "位置已保存".to_string(),
    })
}

/// 设置鼠标穿透
#[tauri::command]
pub async fn set_vertical_overlay_click_through(
    app_handle: tauri::AppHandle,
    enabled: bool,
) -> Result<crate::overlay_panel::OverlayResult, String> {
    if let Some(window) = app_handle.get_webview_window("vertical-overlay") {
        let _ = window.set_ignore_cursor_events(enabled);
    }
    Ok(crate::overlay_panel::OverlayResult {
        success: true,
        message: if enabled { "已开启鼠标穿透" } else { "已关闭鼠标穿透" }.to_string(),
    })
}

pub fn is_vertical_overlay_active() -> bool {
    VERTICAL_OVERLAY_ACTIVE.load(Ordering::SeqCst)
}

pub fn cleanup(app_handle: &AppHandle) {
    if VERTICAL_OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        VERTICAL_OVERLAY_ACTIVE.store(false, Ordering::SeqCst);
        if let Some(window) = app_handle.get_webview_window("vertical-overlay") {
            let _ = window.hide();
        }
    }
}
```

#### 4.1.2 修改 `src-tauri/src/lib.rs`

```rust
// 在 mod 声明区域新增
mod vertical_overlay;

// 在 setup 闭包中新增（清理逻辑）
// 在 invoke_handler 中注册新命令
// 修改快捷键处理逻辑
```

#### 4.1.3 快捷键处理修改

修改 `lib.rs` 中的全局快捷键处理器，根据当前 `style` 决定启动哪种悬浮框：

```rust
if shortcut.id() == hotkey::get_overlay_shortcut_id() {
    let settings = overlay_panel::get_or_init_settings();
    if settings.style == "vertical_panel" {
        if vertical_overlay::is_vertical_overlay_active() {
            let _ = vertical_overlay::stop_vertical_overlay(app.clone());
        } else {
            let _ = vertical_overlay::start_vertical_overlay(app.clone(), Some(settings));
        }
    } else {
        let _ = overlay_panel::toggle_overlay(app);
    }
}
```

#### 4.1.4 修改 `overlay_panel.rs`

在 `update_overlay_settings` 命令中增加对 `vertical_panel` 样式的处理：

```rust
// 如果切换到 vertical_panel 样式，停止 Win32 overlay 并启动 Tauri 窗口
// 如果从 vertical_panel 切换到其他样式，停止 Tauri 窗口
```

### 4.2 前端新增页面

#### 4.2.1 新增文件：`src/pages/VerticalOverlayPage.tsx`

```tsx
import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

// 硬件数据接口（复用现有定义）
interface HardwareData {
  fps: number | null;
  cpu_usage: number | null;
  cpu_temp: number | null;
  // ... 其他字段
}

// 显示项配置（复用现有定义）
interface DisplayItemConfig {
  id: string;
  label: string;
  enabled: boolean;
}

// 图标映射
const ITEM_ICONS: Record<string, string> = {
  fps: "🎮",
  cpu_temp: "🌡️",
  cpu_usage: "⚡",
  cpu_clock: "🔧",
  cpu_voltage: "🔋",
  cpu_power: "💡",
  gpu_temp: "🌡️",
  gpu_usage: "⚡",
  gpu_fan_speed: "🌀",
  gpu_power: "💡",
  gpu_clock: "🔧",
  gpu_voltage: "🔋",
  gpu_vram: "💾",
  gpu_memory_clock: "🔧",
  memory_usage: "💾",
  ssd_temp: "🌡️",
  game_ping: "📶",
  delta_password: "🔑",
  netease_lyric: "🎵",
};

function VerticalOverlayPage() {
  const [hardwareData, setHardwareData] = useState<HardwareData | null>(null);
  const [settings, setSettings] = useState<OverlaySettings | null>(null);
  const [isDragging, setIsDragging] = useState(false);

  // 监听硬件数据
  useEffect(() => {
    let unlisten: UnlistenFn;
    (async () => {
      unlisten = await listen<HardwareData>("overlay-hardware-data", (event) => {
        setHardwareData(event.payload);
      });

      // 获取当前设置
      const stored = await invoke<OverlaySettings>("get_overlay_settings");
      setSettings(stored);
    })();
    return () => { unlisten?.(); };
  }, []);

  // 拖动逻辑
  const handleDragToggle = async () => {
    const newDragState = !isDragging;
    setIsDragging(newDragState);
    await invoke("set_vertical_overlay_click_through", { enabled: !newDragState });

    if (!newDragState) {
      // 保存位置
      const win = getCurrentWebviewWindow();
      const pos = await win.outerPosition();
      await invoke("save_vertical_overlay_position", {
        x: pos.x,
        y: pos.y,
      });
    }
  };

  // 根据 item.id 和 hardwareData 获取显示值
  const getItemValue = (item: DisplayItemConfig): string => {
    if (!hardwareData) return "--";
    switch (item.id) {
      case "fps": return hardwareData.fps?.toString() ?? "--";
      case "cpu_temp": return hardwareData.cpu_temp ? `${hardwareData.cpu_temp.toFixed(0)}°C` : "--";
      case "cpu_usage": return hardwareData.cpu_usage ? `${hardwareData.cpu_usage}%` : "--";
      // ... 其他项
      default: return "--";
    }
  };

  // 获取数值颜色（复用现有颜色逻辑）
  const getValueColor = (value: string): string => {
    const num = parseFloat(value);
    if (isNaN(num)) return "#ffffff";
    if (num < 50) return "#00ff00";
    if (num < 80) return "#ffff00";
    return "#ff0000";
  };

  if (!settings) return null;

  const enabledItems = settings.display_items.filter(i => i.enabled);

  return (
    <div className="vertical-overlay-container">
      {/* 拖动按钮 */}
      <button className="drag-btn" onClick={handleDragToggle}>
        {isDragging ? "📌" : "✋"}
      </button>

      {/* 项列表 */}
      <div className="overlay-items">
        {enabledItems.map((item) => {
          const value = getItemValue(item);
          return (
            <div className="overlay-item" key={item.id}>
              <span className="item-icon">{ITEM_ICONS[item.id] ?? "📋"}</span>
              <span className="item-label">{item.label}</span>
              <span className="item-value" style={{ color: getValueColor(value) }}>
                {value}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

export default VerticalOverlayPage;
```

#### 4.2.2 路由注册

在 `src/App.tsx` 中新增独立路由（与 desktop-lyrics 同级，不使用 MainLayout）：

```tsx
// 竖排悬浮框窗口：独立渲染，不使用主布局
if (location.pathname === "/vertical-overlay") {
  return <VerticalOverlayPage />;
}
```

#### 4.2.3 CSS 样式设计

```css
/* VerticalOverlayPage.css */

.vertical-overlay-container {
  width: 100vw;
  height: 100vh;
  background: rgba(17, 17, 17, var(--overlay-opacity, 0.8));
  border-radius: 12px;
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  font-family: "Microsoft YaHei", sans-serif;
  font-size: 13px;
  color: #ffffff;
  overflow: hidden;
  user-select: none;
  -webkit-user-select: none;
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
  border: 1px solid rgba(255, 255, 255, 0.08);
}

/* 拖动按钮 */
.drag-btn {
  position: absolute;
  top: 4px;
  right: 4px;
  width: 20px;
  height: 20px;
  border: none;
  background: transparent;
  cursor: pointer;
  opacity: 0.4;
  font-size: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: opacity 0.2s;
}

.drag-btn:hover {
  opacity: 1;
}

/* 项列表 */
.overlay-items {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

/* 每一行 */
.overlay-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 0;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}

.overlay-item:last-child {
  border-bottom: none;
}

/* 图标 */
.item-icon {
  width: 20px;
  text-align: center;
  font-size: 14px;
  flex-shrink: 0;
}

/* 标签（完整显示项名） */
.item-label {
  flex: 1;
  white-space: nowrap;
  overflow: visible;
  color: rgba(255, 255, 255, 0.7);
}

/* 数值 */
.item-value {
  text-align: right;
  font-weight: 600;
  min-width: 60px;
  white-space: nowrap;
}
```

### 4.3 前端设置页面修改

#### 4.3.1 修改 `src/pages/OverlayPanelPage.tsx`

在样式选择区域新增第三个选项：

```tsx
{/* 竖排面板样式 */}
<Box
  as="button"
  onClick={() => updateSetting("style", "vertical_panel")}
  bg={settings.style === "vertical_panel" ? hexToRgba(getActiveColor(), 0.12) : "transparent"}
  border="2px solid"
  borderColor={settings.style === "vertical_panel" ? getActiveColor() : "gray.600"}
  borderRadius="xl"
  p={3}
  cursor="pointer"
  textAlign="center"
  transition="all 0.2s"
  _hover={{
    borderColor: getActiveColor(),
    bg: settings.style === "vertical_panel"
      ? hexToRgba(getActiveColor(), 0.12)
      : hexToRgba(getActiveColor(), 0.08),
  }}
>
  <VStack spacing={2}>
    {/* 迷你预览图：竖排小方块 */}
    <Box display="flex" flexDirection="column" gap="3px" alignItems="center">
      <Box w="48px" h="6px" bg="gray.500" borderRadius="sm" opacity={0.6} />
      <Box w="48px" h="6px" bg="gray.500" borderRadius="sm" opacity={0.6} />
      <Box w="48px" h="6px" bg="gray.500" borderRadius="sm" opacity={0.6} />
    </Box>
    <Text fontSize="sm" fontWeight="medium" color={subTextColor}>
      {t("overlayPanel.styles.verticalPanel") || "竖排面板"}
    </Text>
  </VStack>
</Box>
```

#### 4.3.2 启停逻辑修改

修改启动/停止悬浮框的逻辑，根据 `style` 调用不同的命令：

```tsx
const toggleOverlay = async () => {
  const isActive = await invoke<boolean>("get_overlay_panel_status");
  // 注意：需要同时检查 vertical overlay 状态

  if (settings.style === "vertical_panel") {
    if (isActive) {
      await invoke("stop_vertical_overlay");
    } else {
      await invoke("start_vertical_overlay", { settings });
    }
  } else {
    // 原有逻辑
    if (isActive) {
      await invoke("stop_overlay_panel");
    } else {
      await invoke("start_overlay_panel", { settings });
    }
  }
  // 刷新状态
  await refreshStatus();
};
```

### 4.4 图标方案

#### 4.4.1 图标选型

推荐使用 **Lucide React 图标库**（项目已集成），比 Emoji 更美观统一：

```tsx
import {
  Gauge,        // FPS
  Thermometer,  // 温度类
  Cpu,          // CPU 占用/频率
  Zap,          // 电压/功耗
  HardDrive,    // 硬盘
  MemoryStick,  // 内存
  Wifi,         // 网络延迟
  Key,          // 三角洲密码
  Music,        // 网易云歌词
  Fan,          // 风扇转速
  Activity,     // GPU 占用
  Battery,      // 电压
} from "lucide-react";

const ITEM_ICONS: Record<string, LucideIcon> = {
  fps: Gauge,
  cpu_temp: Thermometer,
  cpu_usage: Cpu,
  cpu_clock: Cpu,
  cpu_voltage: Zap,
  cpu_power: Zap,
  gpu_temp: Thermometer,
  gpu_usage: Activity,
  gpu_fan_speed: Fan,
  gpu_power: Zap,
  gpu_clock: Activity,
  gpu_voltage: Battery,
  gpu_vram: MemoryStick,
  gpu_memory_clock: Activity,
  memory_usage: MemoryStick,
  ssd_temp: Thermometer,
  game_ping: Wifi,
  delta_password: Key,
  netease_lyric: Music,
};
```

#### 4.4.2 图标渲染

```tsx
const IconComponent = ITEM_ICONS[item.id] ?? Gauge;
<IconComponent size={14} className="item-icon-svg" />
```

```css
.item-icon-svg {
  flex-shrink: 0;
  color: rgba(255, 255, 255, 0.5);
}
```

---

## 五、实现计划

### 5.1 任务分解

| 阶段 | 任务 | 涉及文件 | 预估工时 |
|------|------|----------|----------|
| **阶段一：后端基础** | | | |
| 1.1 | 创建 `vertical_overlay.rs` 模块 | `src-tauri/src/vertical_overlay.rs` | 2h |
| 1.2 | 注册模块和 Tauri 命令 | `src-tauri/src/lib.rs` | 0.5h |
| 1.3 | Tauri 窗口配置 | `src-tauri/tauri.conf.json` | 0.5h |
| 1.4 | 权限配置 | `src-tauri/capabilities/default.json` | 0.2h |
| 1.5 | 修改快捷键处理逻辑 | `src-tauri/src/lib.rs` | 0.5h |
| 1.6 | 修改 `overlay_panel.rs` 样式切换逻辑 | `src-tauri/src/overlay_panel.rs` | 1h |
| **阶段二：前端页面** | | | |
| 2.1 | 创建 `VerticalOverlayPage.tsx` | `src/pages/VerticalOverlayPage.tsx` | 3h |
| 2.2 | 创建 CSS 样式文件 | `src/pages/VerticalOverlayPage.css` | 1h |
| 2.3 | 路由注册 | `src/App.tsx` | 0.2h |
| 2.4 | 图标映射与渲染 | `src/pages/VerticalOverlayPage.tsx` | 1h |
| **阶段三：设置页面** | | | |
| 3.1 | 新增样式选择按钮 | `src/pages/OverlayPanelPage.tsx` | 1h |
| 3.2 | 修改启停逻辑 | `src/pages/OverlayPanelPage.tsx` | 1h |
| 3.3 | 状态同步（Win32/Tauri 互斥） | `src/pages/OverlayPanelPage.tsx` | 1h |
| **阶段四：交互功能** | | | |
| 4.1 | 拖动功能 | `src/pages/VerticalOverlayPage.tsx` | 1.5h |
| 4.2 | 位置保存/恢复 | `src-tauri/src/vertical_overlay.rs` | 0.5h |
| 4.3 | 鼠标穿透切换 | `src-tauri/src/vertical_overlay.rs` | 0.5h |
| 4.4 | 重置位置功能 | `src/pages/VerticalOverlayPage.tsx` | 0.5h |
| **阶段五：优化打磨** | | | |
| 5.1 | 窗口高度自适应（根据启用项数量） | `src-tauri/src/vertical_overlay.rs` | 1h |
| 5.2 | 不透明度联动 | `src/pages/VerticalOverlayPage.tsx` | 0.5h |
| 5.3 | 字体选择联动 | `src/pages/VerticalOverlayPage.tsx` | 0.5h |
| 5.4 | 数值颜色渐变 | `src/pages/VerticalOverlayPage.tsx` | 0.5h |
| 5.5 | 自定义项支持 | `src/pages/VerticalOverlayPage.tsx` | 1h |
| **阶段六：测试** | | | |
| 6.1 | 功能测试 | — | 2h |
| 6.2 | 性能测试 | — | 1h |
| 6.3 | 兼容性测试 | — | 1h |

**总预估工时：约 24h（3 个工作日）**

### 5.2 里程碑

| 里程碑 | 交付物 | 预计时间 |
|--------|--------|----------|
| M1 - 后端基础完成 | Tauri 命令可用，窗口可创建/销毁 | 第 1 天上午 |
| M2 - 前端页面完成 | 竖排悬浮框可显示硬件数据 | 第 1 天下午 |
| M3 - 设置页集成 | 可在设置页选择竖排样式并启停 | 第 2 天上午 |
| M4 - 交互完成 | 拖动、穿透、位置保存可用 | 第 2 天下午 |
| M5 - 优化完成 | 自适应高度、颜色渐变、自定义项 | 第 3 天上午 |
| M6 - 测试通过 | 全功能测试通过 | 第 3 天下午 |

---

## 六、关键技术点与风险

### 6.1 窗口高度自适应

**问题**：Tauri 窗口高度固定，但启用项数量可变。

**方案**：前端计算实际内容高度后，通过 `invoke` 通知 Rust 调整窗口大小：

```tsx
useEffect(() => {
  const container = document.querySelector('.vertical-overlay-container');
  if (container) {
    const height = container.scrollHeight;
    // 通知 Rust 调整窗口大小
    invoke("resize_vertical_overlay", { height });
  }
}, [enabledItems.length]);
```

```rust
#[tauri::command]
pub async fn resize_vertical_overlay(
    app_handle: tauri::AppHandle,
    height: u32,
) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("vertical-overlay") {
        let _ = window.set_size(tauri::LogicalSize {
            width: 220,
            height: height as f64,
        });
    }
    Ok(())
}
```

### 6.2 Win32 Overlay 与 Tauri Overlay 互斥

**问题**：两种悬浮框不能同时运行。

**方案**：
- 在 `start_vertical_overlay` 中先检查并停止 Win32 overlay
- 在 `start_overlay`（Win32）中先检查并停止 vertical overlay
- 前端状态检查需同时查询两种状态
- 快捷键 toggle 逻辑根据 `settings.style` 分流

### 6.3 鼠标穿透

**问题**：悬浮框需要既能拖动又不遮挡游戏操作。

**方案**：
- 默认状态：鼠标穿透（`set_ignore_cursor_events(true)`）
- 拖动模式：关闭穿透，允许鼠标交互
- 前端提供拖动按钮切换模式
- 拖动结束后自动恢复穿透并保存位置

### 6.4 数据推送频率

**问题**：Tauri 事件推送频率过高可能影响性能。

**方案**：
- 硬件数据推送间隔：**1000ms**（相比 Win32 方案的 100ms，WebView 方案对实时性要求可适当降低）
- FPS 数据可单独高频推送（500ms）如果需要
- 前端使用 `requestAnimationFrame` 优化渲染

### 6.5 WebView 性能考量

**风险**：WebView 进程比 GDI+ 渲染开销更大。

**缓解措施**：
- 窗口尺寸小（220×自适应），渲染面积有限
- 使用 CSS `will-change: transform` 优化重绘
- 避免不必要的 re-render（React.memo / useMemo）
- 数据推送间隔 1s 而非 100ms
- 硬件数据采集本身开销不变（复用现有 poller）

### 6.6 游戏全屏兼容性

**风险**：部分全屏游戏可能遮挡 Tauri 窗口。

**方案**：
- 设置 `alwaysOnTop: true`
- 后端可通过 Win32 API `SetWindowPos` + `HWND_TOPMOST` 强制置顶（参考现有 `force_topmost` 实现）
- 可选：添加 topmost guard 定时器（参考现有 `install_topmost_guard`）

---

## 七、文件变更清单

### 7.1 新增文件

| 文件路径 | 说明 |
|----------|------|
| `src-tauri/src/vertical_overlay.rs` | Rust 后端模块 |
| `src/pages/VerticalOverlayPage.tsx` | 前端悬浮框页面 |
| `src/pages/VerticalOverlayPage.css` | 前端样式文件 |

### 7.2 修改文件

| 文件路径 | 修改内容 |
|----------|----------|
| `src-tauri/tauri.conf.json` | 新增 `vertical-overlay` 窗口配置 |
| `src-tauri/capabilities/default.json` | `windows` 数组添加 `vertical-overlay` |
| `src-tauri/src/lib.rs` | 注册 `vertical_overlay` 模块、命令、快捷键、清理逻辑 |
| `src-tauri/src/overlay_panel.rs` | `update_overlay_settings` 增加样式切换逻辑、暴露 `is_overlay_active` |
| `src/App.tsx` | 新增 `/vertical-overlay` 路由 |
| `src/pages/OverlayPanelPage.tsx` | 新增竖排面板样式选项、修改启停逻辑 |

---

## 八、测试计划

### 8.1 功能测试

| 测试项 | 预期结果 |
|--------|----------|
| 选择竖排面板样式并启用 | 竖排悬浮框显示在屏幕上 |
| 各硬件项数据正确显示 | 数值与主界面硬件页一致 |
| 项名完整显示 | "GPU风扇转速"等长名称不被截断 |
| 图标正确显示 | 每项前有对应图标 |
| 拖动悬浮框 | 可拖动到任意位置 |
| 拖动后位置保存 | 重启后恢复到上次位置 |
| 鼠标穿透 | 默认状态下鼠标可穿透点击下方内容 |
| 快捷键切换 | 全局快捷键可开关竖排悬浮框 |
| 样式切换 | 从竖排切换到默认/灵动岛时，正确切换窗口 |
| 不透明度调整 | 滑动滑块实时调整背景透明度 |
| 窗口高度自适应 | 启用/禁用项后窗口高度自动调整 |

### 8.2 性能测试

| 测试项 | 预期结果 |
|--------|----------|
| CPU 占用 | 悬浮框运行时 CPU 增量 < 2% |
| 内存占用 | WebView 进程内存 < 50MB |
| 游戏帧率影响 | 开启悬浮框后游戏帧率下降 < 3% |
| 数据延迟 | 硬件数据从采集到显示延迟 < 1.5s |

### 8.3 兼容性测试

| 测试项 | 预期结果 |
|--------|----------|
| 窗口模式游戏 | 悬浮框正常置顶显示 |
| 无边框窗口游戏 | 悬浮框正常置顶显示 |
| 全屏独占游戏 | 悬浮框可能被遮挡（已知限制） |
| 多显示器 | 悬浮框在主显示器显示 |
| DPI 缩放 125%/150% | 文字和图标清晰不模糊 |

---

## 九、后续扩展方向

1. **主题色支持**：跟随主界面主题色，图标和数值使用主题色
2. **动画效果**：数值变化时的过渡动画、进度条样式
3. **多列布局**：项数过多时支持两列甚至三列
4. **迷你图表**：在数值旁显示小型趋势图（sparkline）
5. **背景毛玻璃**：利用 CSS `backdrop-filter` 实现毛玻璃效果
6. **自定义图标**：允许用户为自定义项选择图标
7. **预设布局**：提供"精简版"（仅 FPS/CPU/GPU）、"完整版"等预设
8. **跨平台**：Tauri WebView 方案天然支持 macOS/Linux 扩展
