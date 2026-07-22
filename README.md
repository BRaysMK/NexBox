<p align="center">
  <img src="src-tauri/icons/128x128.png" alt="NexBox Logo" width="128" />
</p>

<h1 align="center">NexBox <sub>新境盒</sub></h1>

<p align="center">
  <img src="https://img.shields.io/badge/version-5.8.4-2dd4bf?style=flat-square" alt="Version" />
  <img src="https://img.shields.io/badge/Tauri-2.10-ffc131?style=flat-square&logo=tauri" alt="Tauri" />
  <img src="https://img.shields.io/badge/React-19.0-61dafb?style=flat-square&logo=react" alt="React" />
  <img src="https://img.shields.io/badge/Rust-1.77.2-dea584?style=flat-square&logo=rust" alt="Rust" />
  <img src="https://img.shields.io/badge/license-GPL--3.0-green?style=flat-square" alt="License" />
</p>

<p align="center">
  <strong>为 PC 游戏玩家打造的一站式性能工具箱</strong><br />
  硬件监控 · 系统优化 · 显示增强 · 游戏辅助
</p>

***

## 🧭 目录

- [为什么选择 NexBox](#-为什么选择-nexbox)
- [核心功能](#-核心功能)
  - [硬件监控](#硬件监控)
  - [系统优化](#系统优化)
  - [内置工具](#内置工具)
  - [能力测试](#能力测试)
  - [Delta Force 专区](#delta-force-专区)
  - [更多功能](#更多功能)
- [安装](#-安装)
- [从源码构建](#-从源码构建)
- [项目结构](#-项目结构)
- [贡献指南](#-贡献指南)
- [许可证](#-许可证)

***

## 🎯 为什么选择 NexBox

PC 玩家在游戏中常常需要**同时运行七八个工具软件**——看帧率需要 MSI Afterburner、调色彩需要 DisplayCAL、清内存需要 Mem Reduct……安装繁琐、切换低效，部分工具还会被反作弊系统误判。

**NexBox 把这一切装进一个盒子。**

- ✅ **零性能干扰** — 基于 Tauri v2 构建，内存占用极低，游戏时近乎零开销
- ✅ **纯本地运行** — 核心功能无需联网，不上传任何隐私数据
- ✅ **完全免费开源** — GPL-3.0 协议，代码透明可审计
- ✅ **持续迭代** — 社区驱动，功能随玩家需求不断进化

***

## 🧩 核心功能

### 硬件监控

实时监测系统硬件运行状态，支持在游戏内叠加显示。

| 监控项     | 详情                     |
| ------- | ---------------------- |
| **CPU** | 使用率、温度、频率、电压、功耗        |
| **GPU** | 使用率、温度、风扇转速、功耗、频率、显存用量 |
| **内存**  | 已用 / 总量、实时占比           |
| **磁盘**  | 各分区已用 / 总量             |
| **主板**  | 型号识别                   |
| **FPS** | 实时帧率采集                 |

支持迷你趋势图、多 GPU 同时显示、自定义排序与外观样式。

### 系统优化

10 项系统级优化工具，一键释放游戏性能。

- **内存清理** — 释放物理内存与工作集，支持自动清理
- **内存限制** — 设置页面文件上限，防止虚拟内存膨胀
- **ACE 优化** — 优化腾讯反作弊引擎进程优先级与 CPU 亲和性
- **着色器缓存清理** — NVIDIA / AMD 双平台支持
- **电源管理** — 内置定制高性能电源方案，一键切换
- **启动项管理** — 扫描并清理注册表与启动文件夹中的自启项
- **网络优化器** — 多套 DNS 预设（114 / 阿里 / Cloudflare / 腾讯），TCP 参数调优
- **外设优化** — 鼠标 / 键盘 USB 轮询率调节
- **存储清理** — 系统临时文件、缓存、日志智能扫描与清理
- **系统优化器** — 包含 6 大类共计 50+ 项 Windows 深度调整（性能 / 隐私 / 网络 / 游戏 / 触控 / 应用）

### 内置工具

| 工具            | 说明                                           |
| ------------- | -------------------------------------------- |
| **显示滤镜**      | 屏幕色彩调节：色温、亮度、对比度、饱和度独立控制；ICC 配置文件管理；多显示器独立调校 |
| **准星叠加**      | 自定义游戏辅助准心：十字、圆点、圆形等样式；支持自定义 PNG 图片；颜色取色器     |
| **叠加面板**      | 游戏内悬浮硬件信息面板：FPS / CPU / GPU / 内存等多指标自由拖拽排序   |
| **显卡改写**      | 修改注册表 GPU 名称展示（纯娱乐功能）                        |
| **分辨率转换器**    | 不同分辨率与宽高比之间的快速换算参考                           |
| **DLSS 预设管理** | 基于 DLSSTweaks 数据，为支持的游戏单独或批量设置 DLSS 预设       |
| **灵动岛**       | 屏幕顶部信息栏：音乐控制、Windows 通知镜像、迷你硬件监控             |

### 能力测试

7 种认知反应测试，均带计时与高分记录。

| 测试     | 测量维度              |
| ------ | ----------------- |
| 反应测试   | 延迟反应时间            |
| 瞄准测试   | 30 秒内命中率与反应速度     |
| 专注测试   | 移动目标追踪精度          |
| 选择测试   | 决策速度              |
| 抑制测试   | Go / No-Go 冲动控制能力 |
| 舒尔特方格  | 5×5 专注力与视觉搜索速度    |
| CPS 测试 | 每秒点击次数与峰值         |

### Delta Force 专区

为《三角洲行动》玩家打造的专属功能：

- **新境盒改枪码平台 **— 分类浏览、关键词搜索、一键复制代码、点赞互动、提交分享
- **密码显示** — 从游戏内存读取每日密码
- **快捷跳转** — 码枪堂、ANXU 改枪码、主播改枪码、小涛查等外部平台

### 更多功能

- **Epic 免费游戏** — 卡片展示、限免倒计时、一键跳转领取
- **全局搜索** — 跨页面 / 工具 / 设置快速搜索（`Ctrl+K`）
- **6 语言支持** — 简体中文、繁體中文、English、Français、日本語、Deutsch
- **主题定制** — 深色 / 浅色模式、自定义主题色、毛玻璃效果、视频壁纸
- **全局热键** — 准星叠加、叠加面板、灵动岛均支持自定义快捷键
- **自动更新** — 启动时自动检查版本，应用内下载安装
- **系统托盘** — 最小化到托盘，自定义关闭行为

***

## 📦 安装

### 系统要求

- **操作系统** — Windows 10 22H2 或更高版本（仅 64 位）
- **内存** — 建议 4 GB 以上
- **磁盘** — 至少 200 MB 可用空间

### 下载

前往 [Releases](https://github.com/MuLiuSaMa/NexBox/releases) 页面下载最新版安装程序，运行后按提示完成安装。

***

## 🔧 从源码构建

### 前置要求

| 工具                            | 版本要求                     |
| ----------------------------- | ------------------------ |
| **Node.js**                   | ≥ 18.x                   |
| **Rust**                      | ≥ 1.77.2                 |
| **Visual Studio Build Tools** | Windows 专用（C++ 桌面开发工作负载） |

### 构建步骤

```bash
# 1. 克隆仓库
git clone https://github.com/MuLiuSaMa/NexBox.git
cd NexBox

# 2. 安装前端依赖
npm install

# 3. 启动开发模式（带热重载）
npm run tauri:dev

# 4. 构建生产版本
npm run tauri:build
```

构建产物位于 `src-tauri/target/release/`。若需生成 Windows 安装程序，请使用随附的 `nexbox.iss`（Inno Setup 脚本）。

### 可用命令

| 命令                    | 说明                      |
| --------------------- | ----------------------- |
| `npm run dev`         | 启动 Vite 前端开发服务器         |
| `npm run tauri:dev`   | 启动完整 Tauri 开发环境         |
| `npm run build`       | TypeScript 检查 + Vite 构建 |
| `npm run tauri:build` | 构建 Tauri 桌面应用           |
| `npm run lint`        | ESLint 代码检查             |
| `npm run format`      | Prettier 代码格式化          |

***

## 📁 项目结构

```
nexbox/
├── src/                        # React 前端源码
│   ├── pages/                  # 页面级组件
│   ├── components/             # 可复用 UI 组件
│   ├── contexts/               # React Context 状态管理
│   ├── hooks/                  # 自定义 Hooks
│   ├── lib/                    # 工具函数与常量
│   ├── locales/                # 国际化语言包
│   └── assets/                 # 静态资源
├── src-tauri/                  # Tauri + Rust 后端
│   ├── src/                    # Rust 源码（硬件监控、系统优化等）
│   ├── Cargo.toml              # Rust 依赖清单
│   └── tauri.conf.json         # Tauri 应用配置
├── public/                     # 前端公共资源
├── power-plans/                # 定制电源计划文件 (.pow)
├── monitor/                    # 硬件监控辅助程序 (C#)
├── logo/                       # 应用图标资源
├── nexbox.iss                  # Inno Setup 安装包脚本
└── package.json                # Node.js 项目配置
```

### 技术栈

**前端**

- React 19 + TypeScript 5.8
- Vite 6（构建工具）
- Chakra UI + Ant Design（UI 组件）
- Zustand（状态管理）
- React Router 7（路由）
- i18next（国际化）
- Framer Motion（动画）
- DnD Kit（拖拽排序）

**后端**

- Tauri 2.10（桌面框架）
- Rust 1.77.2
- NVML Wrapper（NVIDIA GPU 监控）
- WMI（Windows 硬件信息）
- Axum + Tower（嵌入式 HTTP 服务）
- Tokio（异步运行时）

***

## 🤝 贡献指南

欢迎以任何形式参与贡献！

1. Fork 本仓库
2. 创建特性分支：`git checkout -b feature/amazing-feature`
3. 提交更改：`git commit -m 'feat: add amazing feature'`
4. 推送分支：`git push origin feature/amazing-feature`
5. 发起 Pull Request

提交前请确保代码通过 lint 检查：

```bash
npm run lint && npm run format
```

***

## 📜 许可证

本项目采用 [GPL-3.0](LICENSE) 许可证。

***

<p align="center">
  <sub>Made with ❤️ by <a href="https://github.com/MuLiuSaMa">MuLiu_SaMa</a> & the NexBox community</sub>
</p>
