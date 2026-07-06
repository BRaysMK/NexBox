---
name: steam-integration
overview: 通过 Steamworks SDK 集成 Steam 账户信息和游戏库到主页右上角。后端使用 `steamworks` Rust crate 读取本地 Steam 客户端数据，前端创建 SteamCard 组件，置于主页右上角，与"今日人气"水平对齐、与"快捷启动"垂直对齐。
design:
  architecture:
    framework: react
  styleKeywords:
    - Glassmorphism
    - 紧凑布局
    - Steam 品牌暗色调
    - 微动画悬停
  fontSystem:
    fontFamily: PingFang SC
    heading:
      size: 14px
      weight: 600
    subheading:
      size: 13px
      weight: 500
    body:
      size: 12px
      weight: 400
  colorSystem:
    primary:
      - "#1A9FFF"
      - "#0077CC"
    background:
      - rgba(20, 30, 50, 0.7)
      - rgba(255, 255, 255, 0.05)
    text:
      - "#E0E0E0"
      - "#888888"
      - "#FFFFFF"
    functional:
      - "#38A169"
      - "#E53E3E"
      - "#718096"
todos:
  - id: create-steam-backend
    content: 创建 Steam 后端模块（src-tauri/src/steam.rs）：实现注册表定位、DLL 动态加载、Steamworks API 初始化、用户信息获取、VDF/ACF 解析器、游戏库扫描，并注册 3 个 Tauri 命令
    status: completed
  - id: register-steam-commands
    content: 在 lib.rs 中声明 steam 模块并注册 get_steam_status / check_steam_running / launch_steam_game 三个命令到 invoke_handler
    status: completed
    dependencies:
      - create-steam-backend
  - id: create-steamcard-component
    content: 创建 SteamCard 前端组件（src/components/SteamCard.tsx）：包含用户信息区、游戏列表区、AvatarRenderer、GameListPopover、三种状态（加载/未运行/正常）、useSteamCardEnabled hook
    status: completed
  - id: integrate-homepage
    content: 修改 HomePage.tsx：引入 SteamCard 组件，添加绝对定位（top=20, right=4），集成显隐开关逻辑（localStorage + CustomEvent）
    status: completed
    dependencies:
      - create-steamcard-component
  - id: add-settings-toggle
    content: 修改 SettingsPage.tsx：在 homepage 区域 homeHardwareModel 之后新增 Steam 卡片显隐开关（state + handler + ThemeSwitch + Divider）
    status: completed
    dependencies:
      - create-steamcard-component
  - id: add-i18n-strings
    content: 在 locales/zh.json 中新增 steam 相关翻译文本（卡片标题、未运行提示、查看全部、设置标签描述等约 8 条）
    status: completed
---

## 用户需求

在 NexBox 主页右上角添加 Steam 集成卡片，使用 Steamworks SDK 获取本地 Steam 账户信息和已安装游戏库。

## 核心功能

- **Steam 用户信息展示**：从本地 Steam 客户端读取当前登录用户（头像、昵称、Steam ID），以紧凑的堆叠布局显示在卡片上方
- **游戏库列表**：显示已安装游戏列表（最多4-5款，可滚动），每款游戏显示名称和图标，底部提供"查看全部"按钮打开总览弹窗
- **本地精准读取**：通过解析 Steam 本地配置文件（libraryfolders.vdf + appmanifest_*.acf）获取完整的已安装游戏列表
- **显隐开关**：遵循现有主页卡片模式（localStorage + CustomEvent），在设置页面中可独立控制 Steam 卡片的显示/隐藏
- **异常状态处理**：Steam 未运行、初始化失败等异常状态下给出清晰的提示信息

## 技术栈

- **后端（Rust/Tauri）**：`steamworks` crate v0.11 + `libloading` 动态加载 `steam_api64.dll`（注册表定位路径）+ 自研 VDF/ACF 文件解析器
- **前端（React/TypeScript）**：Chakra UI + LiquidGlassCard + Lucide 图标库
- **数据交换**：Tauri `invoke` 命令 + `serde` JSON 序列化
- **平台限制**：仅 Windows，依赖 Steam 客户端运行

## 实现方案

### 总体策略

采用**混合方案**——Steamworks SDK 获取用户信息（头像、昵称、Steam ID），本地 VDF/ACF 文件解析获取游戏库列表。Steamworks SDK 的 `ISteamFriends` 接口可直接获取用户名和头像 RGBA 数据，而 `ISteamApps` 接口无法直接枚举所有已安装游戏，因此游戏库部分采用文件解析方案，无需额外依赖。

### Steam API 初始化流程

1. 从 Windows 注册表 `HKEY_CURRENT_USER\Software\Valve\Steam` 读取 `SteamPath` 获取 Steam 安装目录
2. 使用 `libloading` 动态加载 `{SteamPath}/steam_api64.dll`
3. 调用 `SteamAPI_Init()` 初始化（自动使用 AppID 480 = Spacewar，Steamworks 示例应用）
4. 初始化成功后获取 `ISteamFriends` 接口指针，调用 `GetPersonaName()`、`GetFriendPersonaName()`、`GetSmallFriendAvatar()` 获取用户数据
5. 头像返回 RGBA 字节数组（32x32），在前端编码为 base64 data URI

### 游戏库解析

- **libraryfolders.vdf**：包含多个库文件夹路径和每个库中的 AppID 列表
- **appmanifest_*.acf**：每个已安装游戏的清单文件，包含 `appid`、`name`、`installdir` 字段
- 解析策略：递归下降解析器，按行读取，识别缩进层级和引号键值对，构建嵌套结构
- 性能：遍历所有库文件夹和 manifest 文件，O(库数 × manifest数)，通常 < 50 个文件，耗时可忽略

### 关键设计决策

- **动态 DLL 加载而非编译链接**：参考 `nvapi.rs` 的 `libloading` 模式。`steamworks` crate 提供高层 API，但仍依赖本地 `steam_api64.dll`。从注册表动态定位路径，避免硬编码
- **用户信息与游戏库分离**：用户信息来自 SDK（实时、需 Steam 运行），游戏库来自文件解析（可离线展示），通过一次 Tauri 命令调用返回组合结果
- **头像编码在前端完成**：后端返回 `Vec<u8>` 原始 RGBA 数据，前端使用 `canvas` API 转换为 PNG 或直接构建 `data:image/png;base64,...` 显示

## 实现细节

### 性能考量

- **Steam API 初始化**：仅在卡片首次加载时初始化一次，使用 `OnceLock` 缓存初始化状态
- **游戏库扫描**：每次调用时重新扫描，因为游戏可能被安装/卸载。manifest 文件都很小（< 1KB），扫描开销可忽略
- **头像缓存**：前端使用 `useRef` 缓存 base64 编码结果，避免重复编码

### 错误处理

- **Steam 未运行**：`SteamAPI_Init()` 返回 false → 前端展示"Steam 未运行"状态，提供提示
- **DLL 加载失败**：注册表路径不存在或 DLL 缺失 → 返回明确错误信息
- **注册表读取失败**：Steam 未安装 → 返回空结果，前端显示"未检测到 Steam 安装"
- **VDF 解析错误**：个别文件损坏或格式异常 → 跳过该文件，记录警告日志，不影响其他文件解析

### 日志规范

- 使用项目已有的 `log` crate（Rust 侧），info 级别记录初始化/加载事件，warn 级别记录解析异常，不记录用户隐私数据（Steam ID 脱敏处理）
- 前端使用 `console.error` 记录网络/命令异常

### 与现有架构保持一致

- Rust 模块声明：`mod steam;` 放在 `lib.rs` 模块列表末尾，命令注册在 `invoke_handler!` 末尾
- 前端组件：独立 `SteamCard.tsx` 文件，导出默认组件 + `useSteamCardEnabled()` hook
- 设置页开关：在 `SettingsPage.tsx` 的 homepage 区域内 `homeHardwareModel` 之后新增 Steam 开关
- 显隐控制：localStorage key `nexbox_steam_card_enabled`，CustomEvent `steam-card-setting-changed`

## 目录结构

```
src-tauri/
├── Cargo.toml                      # [MODIFY] 添加 steamworks = "0.11" 依赖（可选，如需用 crate 封装的 API）
├── src/
│   ├── lib.rs                      # [MODIFY] 添加 mod steam; + 注册 3 个 Tauri 命令
│   └── steam.rs                    # [NEW] Steam 集成模块（约 350 行）
│       ├── SteamState 结构体        # 全局状态：DLL Library 句柄、函数指针缓存
│       ├── vdf_parse() 函数         # VDF/ACF 递归下降解析器
│       ├── get_steam_install_path() # 从注册表读取 Steam 安装路径
│       ├── init_steam_api()         # 动态加载 DLL + SteamAPI_Init()
│       ├── get_steam_user_info()    # #[tauri::command] 返回用户头像/昵称/Steam ID
│       ├── get_steam_games()        # #[tauri::command] 扫描库文件夹 + 解析 manifest
│       └── check_steam_running()    # #[tauri::command] 快速检查 Steam 是否运行

src/
├── components/
│   └── SteamCard.tsx               # [NEW] Steam 卡片组件（约 280 行）
│       ├── SteamCard 主组件         # LiquidGlassCard 包装，用户信息区 + 游戏列表区
│       ├── useSteamCardEnabled()    # 导出 hook：localStorage + CustomEvent
│       ├── AvatarRenderer           # 子组件：RGBA 字节 → canvas → base64 data URI
│       ├── GameListPopover          # 子组件：Popover/Modal 展示完整游戏列表
│       └── 三种状态                 # 加载中 / Steam未运行 / 正常展示
├── pages/
│   ├── HomePage.tsx                # [MODIFY] 添加 SteamCard（绝对定位 top=20, right=4）+ 显隐逻辑
│   └── SettingsPage.tsx            # [MODIFY] 在 homepage 区域末尾新增 Steam 开关（state + handler + JSX）
└── locales/
    └── zh.json                     # [MODIFY] 新增 steam 相关翻译 key（约 8 条）
```

## 关键代码结构

```rust
// steam.rs 核心类型定义

/// Steam 已安装游戏信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamGame {
    pub appid: u32,
    pub name: String,
    pub install_dir: String,
    pub library_path: String, // 游戏所在库文件夹路径
}

/// Steam 用户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamUserInfo {
    pub steam_id: u64,
    pub persona_name: String,
    pub avatar_rgba: Vec<u8>, // 32x32 RGBA 原始像素数据
    pub avatar_width: u32,
    pub avatar_height: u32,
}

/// Steam 完整状态（单次命令返回）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamStatus {
    pub steam_running: bool,
    pub user_info: Option<SteamUserInfo>,
    pub games: Vec<SteamGame>,
    pub total_game_count: usize,
}
```

```typescript
// SteamCard.tsx 组件接口

interface SteamCardProps {
  // 无外部 props，数据通过 invoke 获取
}

// 导出 hook 供 HomePage 使用
export function useSteamCardEnabled(): boolean {
  // localStorage key: "nexbox_steam_card_enabled"
  // CustomEvent: "steam-card-setting-changed"
}
```

## 设计风格

延续 NexBox 主页卡片的毛玻璃美学风格，Steam 卡片采用 LiquidGlassCard 容器，半透明深色背景配合微妙的蓝色调（Steam 品牌色），营造现代、轻量、沉浸的感觉。

## 页面布局

### Steam 卡片主体

- **位置**：主页右上角，绝对定位 `position="absolute" top="20" right="4"`，与"今日人气"小卡片行垂直对齐，右侧与"快捷启动"水平对齐
- **宽度**：固定 240px，与 GameLauncher 略宽但保持紧凑
- **结构**：自上而下分为三个区域

### 顶部标题栏

- Steam 图标（Lucide `Gamepad2` 或自定义 Steam logo）+ "Steam" 文字
- 紧凑排列，字号 sm（14px），与卡片风格一致

### 中部用户信息区

- 小头像（32px 圆形，带 2px 白色描边）+ 用户名（medium 字重）+ Steam ID（xs 灰色文字）
- 头像使用 RGBA 原始数据通过 canvas 转换为 PNG，确保透明通道正确渲染
- Steam 未运行状态：显示灰色头像占位符 + "Steam 未运行" 提示文字

### 下部游戏库区

- 分隔线（Divider，半透明）
- 游戏列表：每行显示游戏图标（16px）+ 游戏名称（sm，单行省略号），hover 时背景高亮
- 最多显示 4 款游戏，超出部分通过"查看全部（N）"按钮打开 Popover 弹窗
- Popover 弹窗：完整游戏列表，可滚动，支持点击游戏名启动（`steam://rungameid/{appid}`）

### 设置页面开关

- 位于"主页显示设置"区域内，`homeHardwareModel` 开关之后
- 格式跟随现有 pattern：HStack（标签描述 + ThemeSwitch）+ Divider 分隔

### 三种状态

1. **加载中**：居中 Spinner 动画
2. **Steam 未运行**：灰色调卡片，提示"Steam 客户端未运行"，附带"启动 Steam"按钮
3. **正常展示**：亮色调，显示完整用户信息和游戏列表