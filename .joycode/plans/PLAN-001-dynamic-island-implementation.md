# 灵动岛 (Dynamic Island) 功能移植方案

## 任务概述
将 NetSpeed-Dynamic-main 项目的灵动岛功能移植到 NexBox，从 Vue+Tauri 技术栈转换为 React(Chakra UI)+Tauri 技术栈。灵动岛是一个悬浮在屏幕顶部的半透明小组件，可显示网速、硬件监控、音乐控制、消息通知等信息。

## 技术栈对比

| 维度 | NetSpeed-Dynamic | NexBox |
|------|-----------------|--------|
| 前端框架 | Vue 3 + vue-router | React 19 + react-router |
| UI 库 | 原生 CSS | Chakra UI + Antd |
| 动画 | Vue transition + JS | framer-motion |
| 状态管理 | Vue ref/localStorage | zustand |
| 后端 | Tauri 2 + winapi + windows crate | Tauri 2 + sysinfo + windows-sys |
| 路由 | vue-router (多窗口) | react-router (单窗口) |

## 可行性分析

### 完全可行 - NexBox 已有的基础设施
- ✅ Tauri 2 多窗口支持 (只需在 tauri.conf.json 新增 widget 窗口)
- ✅ sysinfo 库 (已安装，可直接获取 CPU/GPU/RAM 数据)
- ✅ 硬件监控模块 (`hardware.rs` + `sensor.rs` + LibreHardwareMonitor)
- ✅ 音乐相关模块 (`music.rs` + `netease_lyrics.rs`)
- ✅ reqwest + urlencoding (封面获取)
- ✅ framer-motion (弹簧动画替代 Vue transition)

### 需要新增的 Rust 后端能力
- 🔧 `get_network_stats` - 网速统计 (需新增，参考 NSD 的 Networks 实现)
- 🔧 `get_network_latency` - 网络延迟检测 (需新增)
- 🔧 `control_system_media` - 系统媒体控制 (需新增 winapi keybd_event)
- 🔧 `force_window_topmost` - 窗口强制置顶 (需新增 winapi SetWindowPos)
- 🔧 `fetch_latest_notification` - 系统通知读取 (需新增 windows crate)
- 🔧 Widget 窗口圆角裁剪 (DWM API，可从 NSD 复制)

## TODO: Phase 1 - Rust 后端扩展

### 1.1 新增灵动岛后端模块
- [ ] 创建 `src-tauri/src/island.rs` 模块
- [ ] 实现 `get_network_stats` (使用 sysinfo::Networks)
- [ ] 实现 `get_network_latency` (TcpStream 超时检测)
- [ ] 实现 `control_system_media` (winapi keybd_event)
- [ ] 实现 `force_window_topmost` (winapi SetWindowPos + 全屏检测)
- [ ] 实现 `fetch_latest_notification` (windows crate UserNotificationListener)
- [ ] 实现 `open_app_by_aumid` (ShellExecuteW 协议唤醒)
- [ ] 在 `lib.rs` 注册所有新 command

### 1.2 Widget 窗口配置与裁剪
- [ ] 在 `tauri.conf.json` 新增 widget 窗口配置 (transparent, decorations:false, alwaysOnTop, skipTaskbar, shadow:false)
- [ ] 在 setup 中添加 widget 窗口的 DWM 圆角裁剪逻辑 (可从 NSD `lib.rs:551-591` 复制)
- [ ] 添加 widget 窗口的关闭拦截 (hide 代替 close)

### 1.3 Cargo.toml 依赖
- [ ] 确认 winapi crate 已添加 (当前未添加，需新增 winapi 依赖)
- [ ] 确认 windows crate 通知相关 feature 已添加

## TODO: Phase 2 - 前端灵动岛组件 (React 重写)

### 2.1 灵动岛路由与窗口
- [ ] 新增 `/widget` 路由指向 WidgetIsland 组件
- [ ] 创建 `src/pages/WidgetIslandPage.tsx` (核心灵动岛页面)
- [ ] 创建 `src/components/island/` 目录存放子组件

### 2.2 WidgetIsland 核心组件 (Vue→React 转换)
- [ ] `IslandContainer` - 外层容器 (弹簧入场/离场动画，framer-motion 替代 Vue transition)
- [ ] `SpeedBox` - 网速显示 (上传/下载 + 高流量高亮)
- [ ] `HardwareMonitor` - CPU/GPU/RAM 监控 (复用现有 sensor 数据)
- [ ] `MusicControlBox` - 音乐控制 (专辑封面旋转 + 播放控制)
- [ ] `MsgNotificationBox` - 消息通知展示
- [ ] `StatusDot` - 网络状态指示灯 (绿/黄/红)
- [ ] `RainbowBorderGlow` - 流光边框特效

### 2.3 动画系统 (framer-motion 替代手写 JS 动画)
- [ ] 弹簧动画：使用 `framer-motion` 的 `spring` 类型替代手写 cos*exp 衰减
- [ ] 灵动岛尺寸变化动画：`animateIslandSize` → `useAnimation` + `motion.div`
- [ ] 内容切换动画：`AnimatePresence` + `motion.div` 替代 Vue `<transition>`

### 2.4 窗口交互
- [ ] 拖拽：`getCurrentWindow().startDragging()`
- [ ] 右键菜单：`@tauri-apps/api/menu` MenuItem
- [ ] 位置记忆与吸附：localStorage + PhysicalPosition
- [ ] 置顶与任务栏模式

## TODO: Phase 3 - 灵动岛设置页面 (入口: 内置工具页面)

### 3.1 在 BuiltinToolsPage 添加入口
- [ ] 在 `src/pages/BuiltinToolsPage.tsx` 的 tools 数组中新增灵动岛 ViewItem:
  ```ts
  { id: "dynamic-island", path: "/dynamic-island", icon: Smartphone,
    titleKey: "sidebar.dynamicIsland", descriptionKey: "builtinTools.dynamicIslandDesc",
    color: "#E91E63" }
  ```
- [ ] 从 lucide-react 引入 Smartphone 图标 (或选更合适的图标如 Activity/Wifi)

### 3.2 灵动岛设置页面组件
- [ ] 创建 `src/pages/DynamicIslandPage.tsx` (设置页主组件)
- [ ] 开关控制 (灵动岛显隐、音乐控制、硬件监控、消息通知、流光边框)
- [ ] 透明度滑块、主题切换 (黑/白)
- [ ] 置于任务栏选项
- [ ] 通过 Tauri event 与 widget 窗口通信

### 3.3 路由与国际化
- [ ] 在 `src/App.tsx` 的两套路由中新增 `/dynamic-island` 路由
- [ ] 在国际化文件中添加 sidebar.dynamicIsland / builtinTools.dynamicIslandDesc 等键值

## TODO: Phase 4 - 集成与优化

### 4.1 测试与调优
- [ ] 全屏游戏检测 (避免灵动岛抢占焦点)
- [ ] 窗口置顶稳定性测试
- [ ] 内存泄漏检查 (定时器、事件监听)
- [ ] 多显示器支持验证

## 可直接复制的文件 (Rust 侧)
以下代码逻辑可直接从 NSD 复制并适配到 NexBox：
1. `NetSpeed-Dynamic-main/src-tauri/src/lib.rs:1-183` → `island.rs` (音乐信息+媒体控制)
2. `NetSpeed-Dynamic-main/src-tauri/src/lib.rs:185-291` → `island.rs` (通知读取)
3. `NetSpeed-Dynamic-main/src-tauri/src/lib.rs:293-409` → `island.rs` (app唤醒+窗口置顶)
4. `NetSpeed-Dynamic-main/src-tauri/src/lib.rs:411-453` → `island.rs` (硬件/网速统计)
5. `NetSpeed-Dynamic-main/src-tauri/src/lib.rs:551-591` → `lib.rs setup` (DWM裁剪)

## CSS 样式参考
`WidgetIsland.vue` 的 `<style scoped>` (L979-1462) 中的样式需完全用 React CSS-in-JS (Chakra UI + emotion) 重写，不能直接复制。