# NexBox - 游戏工具箱

[![Version](https://img.shields.io/badge/version-2.5.7-blue.svg)](https://github.com/your-repo/nexbox)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2.10-orange.svg)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-19.0-blue.svg)](https://reactjs.org/)
[![Rust](https://img.shields.io/badge/Rust-1.77.2-red.svg)](https://www.rust-lang.org/)

NexBox 是一款专为游戏玩家打造的桌面工具箱，集成了硬件监控、系统优化、游戏辅助等多种实用功能，帮助你获得更流畅的游戏体验。

***

## 📋 目录

- [用户指南](#用户指南)
  - [核心功能](#核心功能)
  - [特色亮点](#特色亮点)
  - [适用场景](#适用场景)
  - [安装使用](#安装使用)
- [开发者文档](#开发者文档)
  - [技术栈](#技术栈)
  - [项目结构](#项目结构)
  - [开发环境搭建](#开发环境搭建)
  - [构建发布](#构建发布)
  - [贡献指南](#贡献指南)

***

## 用户指南

### 核心功能

#### 🖥️ 硬件监控

实时监控电脑硬件状态，让你随时掌握系统运行情况：

- **CPU 监控** - 实时显示处理器占用率、温度、核心数、主频等信息
- **GPU 监控** - 显示显卡占用率、温度、显存、驱动版本
- **内存监控** - 实时内存使用率、已用/总量显示
- **存储监控** - 硬盘使用情况、读写状态
- **主板信息** - 显示主板型号等硬件详情

所有数据以可视化图表形式呈现，支持历史趋势曲线，让你一目了然。

#### ⚡ 系统优化

一键优化系统性能，释放更多资源给游戏：

- **临时文件清理** - 清理系统临时文件，释放磁盘空间
- **内存优化** - 释放被占用的内存，提升系统响应速度
- **隐私服务优化** - 关闭不必要的后台服务
- **高性能电源计划** - 自动切换到高性能模式
- **DNS 刷新** - 优化网络连接
- **Wallpaper Engine 管理** - 游戏时关闭动态壁纸节省资源

支持一键全选优化，也可根据需要单独选择优化项目。

#### 🎯 准星工具

为射击游戏玩家量身定制的准星覆盖工具：

- **多种准星样式** - 十字、圆点、圆形、十字圆点、圆形十字等 5 种样式
- **自定义颜色** - 9 种预设颜色可选（红、绿、蓝、青、品红、黄、白、橙、粉）
- **参数调节** - 大小、粗细、间隙、中心点大小、透明度全可调
- **实时预览** - 调整参数时即时看到效果
- **一键开关** - 快速启用或禁用准星显示

#### 📊 悬浮面板

游戏内实时显示硬件信息的悬浮窗口：

- **CPU 占用率** - 实时处理器使用情况
- **GPU 温度/占用** - 显卡温度和使用率
- **内存占用** - 内存使用百分比
- **三角洲密码** - 自动获取并显示游戏密码
- **透明度调节** - 可调整悬浮窗透明度，不影响游戏视野

#### 🎨 显示滤镜

屏幕色彩调节工具，打造舒适的视觉体验：

- **多种预设模式**
  - 标准 - 默认显示效果
  - 鲜艳 - 增强色彩饱和度
  - 电影 - 电影级色彩调校
  - 高亮 - 提升画面亮度
  - 柔和 - 降低对比度，护眼模式
  - 游戏 - 优化游戏画面
  - 阅读 - 适合长时间阅读
- **参数调节** - 色温、亮度、对比度、饱和度可精细调整

#### 🎮 三角洲行动专属

为《三角洲行动》玩家提供的专属功能：

- **每日密码** - 自动获取并显示游戏每日密码，一键复制
- **DLSS 模型预设** - 一键切换 DLSS 模型（A-M 共 11 种），优化游戏画质
- **改枪平台入口** - 快速访问 ATGUNS、码枪堂等改枪网站
- **官方地图工具** - 内置游戏官方地图工具窗口

#### 🧰 工具箱

集成常用工具，一键下载安装：

- **硬件工具** - CPU-Z、GPU-Z 等硬件检测工具
- **优化工具** - Mem Reduct 内存清理、Optimizer 系统优化
- **网络工具** - Clash Verge 等网络代理工具
- **游戏助手** - GamePP 游戏加速器

工具自动检测安装状态，已安装的工具可直接运行，未安装的一键下载。

#### ⚙️ 设置中心

个性化你的使用体验：

- **语言切换** - 支持中文/英文/德文/法文/日文界面
- **主题切换** - 亮色/暗色主题自由选择
- **主题色定制** - 多种主题色可选
- **背景设置** - 支持自定义背景图片
- **快捷工具** - 首页快捷工具开关
- **自定义小组件** - 支持添加自定义 HTML 小组件

### 特色亮点

✨ **界面美观** - 采用现代化 UI 设计，支持毛玻璃效果，视觉体验出色

🚀 **轻量高效** - 基于 Tauri 框架开发，内存占用低，启动速度快

🔒 **安全可靠** - 本地运行，无需联网即可使用大部分功能

🌐 **多语言支持** - 内置多语言界面，自动识别系统语言

### 适用场景

- 🎯 **FPS 游戏玩家** - 准星工具、硬件监控助你精准射击
- 🖥️ **硬件发烧友** - 实时监控硬件状态，掌握系统性能
- ⚡ **追求极致性能** - 一键优化释放系统资源
- 🎮 **三角洲行动玩家** - 专属功能提升游戏体验

### 安装使用

#### 下载安装

1. 访问 [Releases 页面](https://github.com/your-repo/nexbox/releases) 下载最新版本
2. 运行安装程序，按照提示完成安装
3. 启动 NexBox 即可使用

#### 系统要求

- **操作系统**: Windows 10 或更高版本
- **内存**: 建议 4GB 以上
- **磁盘空间**: 至少 100MB 可用空间

***

## 开发者文档

### 技术栈

#### 前端

- **框架**: React 19.0
- **构建工具**: Vite 6.0
- **UI 组件库**: Chakra UI 2.10
- **语言**: TypeScript 5.8
- **路由**: React Router 7.1
- **国际化**: i18next
- **动画**: Framer Motion
- **图标**: Lucide React + React Icons

#### 后端

- **框架**: Tauri 2.10
- **语言**: Rust 1.77.2
- **主要依赖**:
  - sysinfo (系统信息获取)
  - nvml-wrapper (NVIDIA GPU 监控)
  - wmi + winreg (Windows API)
  - reqwest (HTTP 客户端)
  - tokio (异步运行时)

### 项目结构

```
nexbox/
├── src/                      # 前端源代码
│   ├── pages/                # 页面组件
│   ├── components/           # 可复用组件
│   │   ├── special/          # 特殊组件（毛玻璃效果等）
│   │   └── ui/               # UI 组件
│   ├── contexts/             # React Context
│   ├── hooks/                # 自定义 Hooks
│   ├── lib/                  # 工具函数
│   ├── locales/              # 国际化资源
│   └── assets/               # 静态资源
├── src-tauri/                # Tauri 后端
│   ├── src/                  # Rust 源代码
│   │   ├── main.rs           # 主入口
│   │   ├── lib.rs            # 库文件
│   │   ├── hardware.rs       # 硬件监控
│   │   ├── optimization.rs   # 系统优化
│   │   └── ...               # 其他模块
│   ├── Cargo.toml            # Rust 依赖配置
│   └── tauri.conf.json       # Tauri 配置
├── public/                   # 公共静态资源
├── package.json              # Node.js 依赖配置
├── tsconfig.json             # TypeScript 配置
├── vite.config.ts            # Vite 配置
└── README.md                 # 项目说明
```

### 开发环境搭建

#### 前置要求

1. **Node.js**: 建议 18.x 或更高版本
2. **Rust**: 1.77.2 或更高版本
3. **Windows 开发工具**: Visual Studio Build Tools (Windows 专用)

#### 安装步骤

1. 克隆仓库
   ```bash
   git clone https://github.com/MuLiuSaMa/NexBox/nexbox.git
   cd nexbox
   ```
2. 安装 Node.js 依赖
   ```bash
   npm install
   ```
3. 安装 Rust 依赖（首次构建时自动完成）
4. 启动开发服务器
   ```bash
   npm run tauri:dev
   ```

#### 可用脚本

```bash
# 前端开发服务器
npm run dev

# Tauri 开发模式（推荐）
npm run tauri:dev

# 构建前端
npm run build

# 构建 Tauri 应用
npm run tauri:build

# 代码检查
npm run lint

# 代码格式化
npm run format
```

### 构建发布

#### 构建 Release 版本

```bash
npm run tauri:build
```

构建完成后，可执行文件将位于 `src-tauri/target/release/` 目录。

#### 打包安装程序

项目使用 Inno Setup 打包 Windows 安装程序，配置文件为 `nexbox.iss`。

### 贡献指南

我们欢迎任何形式的贡献！

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 开启 Pull Request

#### 代码规范

- 前端代码遵循 ESLint 配置
- 后端代码遵循 Rust 官方代码风格
- 提交前请运行 `npm run lint` 和 `npm run format`

***

## 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件

## 致谢

感谢所有为 NexBox 做出贡献的开发者！

***

**NexBox，你的游戏好帮手！**
