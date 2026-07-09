# NexBox 音乐播放器开发计划

> 基于 Mineradio 开源项目分析，在 NexBox (Tauri 2 + React 19) 中实现完整的在线音乐播放器

---

## 一、Mineradio 项目分析

### 1.1 项目概述

Mineradio 是一个基于 **Electron** 的桌面音乐播放器，核心功能包括：

- 网易云音乐 / QQ 音乐双平台搜索与播放
- 扫码登录 + Cookie 持久化
- 用户歌单同步、红心喜欢、收藏到歌单
- 歌词显示（含翻译、罗马音）
- 多音质探测（标准 → 极高 → 无损 → 高清臻音 → 超清母带）
- 音频/封面代理（解决跨域和 Referer 限制）
- 播客、推荐、每日推荐等发现页功能

### 1.2 技术架构

```
┌─────────────────────────────────────────┐
│            Electron 主窗口               │
│         public/index.html               │
│    (前端 UI + CSS + JS，单文件)          │
└──────────────┬──────────────────────────┘
               │ fetch /api/*
┌──────────────▼──────────────────────────┐
│         Node.js 本地服务器               │
│            server.js (端口 3000)          │
│                                          │
│  ┌─────────────┐  ┌──────────────────┐  │
│  │ NeteaseCloud│  │  QQ Music        │  │
│  │ MusicApi    │  │  (自定义 HTTP)    │  │
│  │ (npm 包)    │  │                  │  │
│  └─────────────┘  └──────────────────┘  │
│                                          │
│  Cookie 持久化 (.cookie / .qq-cookie)    │
│  音频/封面代理 (/api/audio, /api/cover)  │
└──────────────────────────────────────────┘
```

### 1.3 网易云音乐 API 调用方式

Mineradio 使用 [`NeteaseCloudMusicApi`](https://github.com/Binaryify/NeteaseCloudMusicApi) npm 包（v4.32.0），这是一个逆向工程的网易云 API 封装库。

#### 核心功能与对应 API

| 功能 | API 函数 | 关键参数 |
|------|----------|----------|
| 搜索 | `cloudsearch` | `keywords`, `limit`, `cookie` |
| 歌曲详情 | `song_detail` | `ids` (逗号分隔), `cookie` |
| 播放地址 | `song_url_v1` / `song_url` | `id`, `level`/`br`, `cookie` |
| 二维码登录 | `login_qr_key` → `login_qr_create` → `login_qr_check` | `key`, 轮询间隔 2s |
| 登录状态 | `login_status` | `cookie` |
| 用户信息 | `user_account` | `cookie` |
| 用户歌单 | `user_playlist` | `uid`, `cookie` |
| 歌单曲目 | `playlist_track_all` / `playlist_detail` | `id`, `limit`, `offset`, `cookie` |
| 喜欢列表 | `likelist` | `uid`, `cookie` |
| 喜欢检查 | `song_like_check` | `ids` (逗号分隔), `cookie` |
| 红心/取消 | `like` | `id`, `like: true/false`, `cookie` |
| 收藏到歌单 | `playlist_track_add` | `op: 'add'`, `pid`, `tracks`, `cookie` |
| 创建歌单 | `playlist_create` | `name`, `cookie` |
| 歌词 | `lyric_new` / `lyric` | `id`, `cookie` |
| 推荐 | `personalized`, `recommend_resource`, `recommend_songs` | `cookie` |
| 评论 | `comment_music` | `id`, `cookie` |
| 歌手 | `artist_detail`, `artist_songs`, `artist_top_song` | `id`, `cookie` |

#### 音质探测逻辑

```javascript
const NETEASE_QUALITY_CANDIDATES = [
  { level: 'jymaster', br: 1999000, label: '超清母带', svip: true },
  { level: 'hires',    br: 1999000, label: '高清臻音' },
  { level: 'lossless', br: 1411000, label: '无损' },
  { level: 'exhigh',   br: 999000,  label: '极高' },
  { level: 'standard', br: 128000,  label: '标准' },
];
```

- 从用户请求的音质开始，依次尝试更低的音质
- 检测 `freeTrialInfo` 字段，判断是否只有试听片段
- 如果只有试听，标记为 `trial: true` 并回退使用
- 所有 API 调用都携带用户 Cookie（`MUSIC_U` 是核心鉴权字段）

#### 登录流程

**方式一：二维码登录**
1. 调用 `login_qr_key` 获取 key
2. 调用 `login_qr_create` 获取二维码图片
3. 每 2 秒轮询 `login_qr_check`
   - 801: 等待扫码
   - 802: 已扫码待确认
   - 803: 授权成功（返回 Cookie）
   - 800: 二维码过期
4. 成功后保存 Cookie 到 `.cookie` 文件

**方式二：网页登录（推荐，更稳定）**
1. Electron 打开 `BrowserWindow`，加载 `https://music.163.com/#/login`
2. 每 1.2 秒检查 `session.cookies`
3. 检测到 `MUSIC_U` Cookie 即登录成功
4. 提取并保存完整 Cookie 字符串

### 1.4 QQ 音乐 API 调用方式

QQ 音乐没有现成的 npm 包，Mineradio 通过直接 HTTP 请求实现：

#### 核心 API

| 功能 | URL / 模块 | 方式 |
|------|-----------|------|
| 搜索 | `https://c.y.qq.com/splcloud/fcgi-bin/smartbox_new.fcg` | GET |
| 歌曲详情 | `music.pf_song_detail_svr.get_song_detail_yqq` | POST musicu.fcg |
| 播放地址 | `vkey.GetVkeyServer.CgiGetVkey` | POST musicu.fcg |
| 用户歌单 | `fcg_get_profile_order_asset.fcg` | GET |
| 歌单曲目 | `fcg_ucc_getcdinfo_by_ids_cp.fcg` | GET |
| 歌词 | `music.musichallSong.PlayLyricInfo.GetPlayLyricInfo` | POST musicu.fcg |
| 用户资料 | `fcg_get_profile_homepage.fcg` | GET |

- 统一入口：`https://u.y.qq.com/cgi-bin/musicu.fcg`（POST JSON）
- 关键 Cookie：`uin`（账号）、`qm_keyst` / `qqmusic_key` / `music_key`（播放授权）
- `p_skey` 只能证明网页登录态，不等于播放授权

#### QQ 音质模板

```javascript
const QQ_QUALITY_CANDIDATE_TEMPLATES = [
  { prefix: 'RS01', ext: '.flac', level: 'hires',    label: 'Hi-Res FLAC' },
  { prefix: 'F000', ext: '.flac', level: 'lossless', label: '无损 FLAC' },
  { prefix: 'M800', ext: '.mp3',  level: 'exhigh',   label: '320k MP3' },
  { prefix: 'M500', ext: '.mp3',  level: 'standard', label: '128k MP3' },
  { prefix: 'C400', ext: '.m4a',  level: 'aac',      label: 'AAC/M4A' },
];
```

### 1.5 音频/封面代理

网易云和 QQ 音乐的音频/图片 URL 有 **Referer 限制**和 **跨域限制**，必须通过本地代理转发：

```
/api/audio?url=<音频URL>&range=<bytes范围>
  → 设置 Referer: https://music.163.com/ 或 https://y.qq.com/
  → 支持 Range 请求（分段加载）
  → 返回正确的 Content-Type

/api/cover?url=<图片URL>
  → 设置 Referer
  → 添加 CORS 头
  → 缓存 24 小时
```

---

## 二、NexBox 现状分析

### 2.1 技术栈

| 项目 | NexBox | Mineradio |
|------|--------|-----------|
| 框架 | **Tauri 2.x** | Electron |
| 前端 | **React 19 + TypeScript** | 原生 HTML/JS |
| UI 库 | **Chakra UI + Ant Design** | 自定义 CSS |
| 后端 | **Rust** | Node.js |
| 状态管理 | **Zustand** | 全局变量 |
| HTTP 客户端 | **reqwest (Rust) + tauri-plugin-http** | Node.js http/https |

### 2.2 现有音乐功能

NexBox 已有一个**基础本地音乐播放器**：

- `src-tauri/src/music.rs` — 仅列出 `public/music/` 目录下的本地音频文件
- `src/components/MusicPlayer.tsx` — 播放器 UI 组件
- `src/components/MiniMusicPlayer.tsx` — 迷你播放器
- `src/contexts/music-context.tsx` — 播放状态管理（播放/暂停/上下首/音量/模式）

**现状限制**：只能播放本地文件，无在线搜索、无登录、无歌单、无歌词。

### 2.3 已有可复用基础设施

| 组件 | 文件 | 说明 |
|------|------|------|
| 网易云 EAPI 加密 | `src-tauri/src/netease_lyrics.rs` | AES-128-ECB + MD5，已实现完整 EAPI 加密 |
| 网易云搜索 | `src-tauri/src/netease_lyrics.rs` | 已实现搜索+匹配+歌词获取 |
| HTTP 客户端 | `Cargo.toml` | `reqwest` 已引入 |
| AES/MD5 加密 | `Cargo.toml` | `aes`, `md-5`, `cipher` 已引入 |
| 封面获取 | `src-tauri/src/island.rs` | 已实现网易云搜索获取封面 |
| Tauri 窗口管理 | `tauri.conf.json` | 已有多窗口配置（main, widget, tray-menu） |
| 本地存储 | `tauri-plugin-store` | 已引入，可用于存储 Cookie/设置 |

---

## 三、开发计划

### 3.1 架构设计

#### 推荐方案：纯 Rust 后端 + React 前端

```
┌──────────────────────────────────────────────────┐
│              React 前端 (TypeScript)              │
│                                                   │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌────────┐ │
│  │ 搜索页   │ │ 歌单页   │ │ 播放器   │ │ 登录页  │ │
│  └────┬────┘ └────┬────┘ └────┬────┘ └───┬────┘ │
│       │           │           │          │       │
│       └───── invoke() / emit() ─────────┘       │
└───────────────────────┬──────────────────────────┘
                        │
┌───────────────────────▼──────────────────────────┐
│              Tauri Rust 后端                      │
│                                                   │
│  ┌─────────────────────────────────────────┐     │
│  │        music_api.rs (核心模块)           │     │
│  │                                          │     │
│  │  ┌──────────────┐  ┌─────────────────┐  │     │
│  │  │ 网易云 API    │  │ QQ 音乐 API     │  │     │
│  │  │ (EAPI 加密)   │  │ (HTTP 直连)     │  │     │
│  │  └──────────────┘  └─────────────────┘  │     │
│  │                                          │     │
│  │  Cookie 管理 / 音质探测 / 代理转发       │     │
│  └─────────────────────────────────────────┘     │
│                                                   │
│  ┌──────────────┐  ┌────────────────────────┐    │
│  │ audio_proxy  │  │ login_window           │    │
│  │ (axum 路由)  │  │ (WebviewWindow)        │    │
│  └──────────────┘  └────────────────────────┘    │
└───────────────────────────────────────────────────┘
```

**为什么不用 Node.js Sidecar？**
- Tauri 的优势就是轻量，引入 Node.js 运行时违背初衷
- NexBox 已有 Rust 的 EAPI 加密实现，可直接复用
- Rust 的 `reqwest` 完全可以替代 Node.js 的 HTTP 请求
- 减少打包体积和依赖复杂度

### 3.2 Rust 后端模块划分

#### 新建文件结构

```
src-tauri/src/
├── music_api/              # 音乐 API 核心模块
│   ├── mod.rs              # 模块入口 + 通用工具
│   ├── netease.rs          # 网易云音乐 API
│   ├── qqmusic.rs          # QQ 音乐 API
│   ├── models.rs           # 数据结构定义
│   ├── crypto.rs           # 加密工具（从 netease_lyrics.rs 提取）
│   ├── cookie.rs           # Cookie 存储/读取
│   └── audio_proxy.rs      # 音频/封面代理服务器
├── music.rs                # 保留：本地音乐文件
├── netease_lyrics.rs       # 保留：网易云歌词（独立功能）
└── lib.rs                  # 注册新的 Tauri commands
```

#### 核心数据结构 (`models.rs`)

```rust
use serde::{Deserialize, Serialize};

/// 统一歌曲结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    pub provider: String,       // "netease" | "qq" | "local"
    pub id: String,
    pub mid: Option<String>,    // QQ 音乐用
    pub media_mid: Option<String>,
    pub name: String,
    pub artist: String,
    pub artists: Vec<Artist>,
    pub album: String,
    pub cover: String,
    pub duration: u64,          // 毫秒
    pub fee: i32,               // 0=免费, 1=VIP, 4=购买
    pub playable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artist {
    pub id: Option<String>,
    pub mid: Option<String>,
    pub name: String,
}

/// 歌单
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub provider: String,
    pub id: String,
    pub name: String,
    pub cover: String,
    pub track_count: u32,
    pub creator: String,
}

/// 播放地址结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SongUrlResult {
    pub url: Option<String>,
    pub playable: bool,
    pub trial: bool,
    pub level: String,
    pub quality: String,
    pub br: u64,
    pub reason: Option<String>,
    pub message: Option<String>,
}

/// 登录信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginInfo {
    pub provider: String,
    pub logged_in: bool,
    pub user_id: String,
    pub nickname: String,
    pub avatar: String,
    pub vip_type: i32,
    pub vip_level: String,      // "none" | "vip" | "svip"
    pub is_vip: bool,
    pub is_svip: bool,
}

/// 歌词
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lyrics {
    pub lyric: String,
    pub translation: Option<String>,
    pub roma: Option<String>,   // 罗马音
}
```

#### 网易云 API 实现 (`netease.rs`)

复用 `netease_lyrics.rs` 中已有的 EAPI 加密基础设施：

```rust
// 核心函数清单
pub async fn search(keywords: &str, limit: u32, cookie: &str) -> Result<Vec<Song>>;
pub async fn song_detail(ids: &[String], cookie: &str) -> Result<Vec<Song>>;
pub async fn song_url(id: &str, level: &str, cookie: &str) -> Result<SongUrlResult>;
pub async fn login_qr_key() -> Result<String>;
pub async fn login_qr_create(key: &str) -> Result<String>;  // 返回 base64 图片
pub async fn login_qr_check(key: &str) -> Result<QrCheckResult>;
pub async fn login_status(cookie: &str) -> Result<LoginInfo>;
pub async fn user_playlist(uid: &str, cookie: &str) -> Result<Vec<Playlist>>;
pub async fn playlist_tracks(id: &str, cookie: &str) -> Result<Vec<Song>>;
pub async fn likelist(uid: &str, cookie: &str) -> Result<Vec<String>>;
pub async fn like(id: &str, like: bool, cookie: &str) -> Result<()>;
pub async fn playlist_add_song(pid: &str, track_id: &str, cookie: &str) -> Result<()>;
pub async fn playlist_create(name: &str, cookie: &str) -> Result<Playlist>;
pub async fn lyric(id: &str, cookie: &str) -> Result<Lyrics>;
pub async fn personalized(cookie: &str) -> Result<Vec<Playlist>>;
pub async fn recommend_songs(cookie: &str) -> Result<Vec<Song>>;
```

**EAPI 加密流程**（已在 `netease_lyrics.rs` 中实现）：

```
1. 构造请求参数 JSON
2. 计算 digest = MD5("nobody{api_path}use{payload}md5forencrypt")
3. 拼接 data = "{api_path}-36cd479b6b5-{payload}-36cd479b6b5-{digest}"
4. AES-128-ECB 加密 data，密钥 = b"e82ckenh8dichen8"
5. 十六进制大写编码
6. POST 发送 params={encrypted_hex}
```

**API 端点**：

| 功能 | URL | API Path |
|------|-----|----------|
| 搜索 | `https://interface3.music.163.com/eapi/cloudsearch/get/web` | `/api/cloudsearch/get/web` |
| 歌曲详情 | `https://interface3.music.163.com/eapi/v3/song/detail` | `/api/v3/song/detail` |
| 播放地址 | `https://interface3.music.163.com/eapi/song/enhance/player/url/v1` | `/api/song/enhance/player/url/v1` |
| 二维码 key | `https://interface3.music.163.com/eapi/login/qrcode/unikey` | `/api/login/qrcode/unikey` |
| 二维码生成 | `https://interface3.music.163.com/eapi/login/qrcode/client/login` | `/api/login/qrcode/client/login` |
| 二维码检查 | `https://interface3.music.163.com/eapi/login/qrcode/client/login` | `/api/login/qrcode/client/login` |
| 用户歌单 | `https://interface3.music.163.com/eapi/user/playlist` | `/api/user/playlist` |
| 歌单曲目 | `https://interface3.music.163.com/eapi/v6/playlist/detail` | `/api/v6/playlist/detail` |
| 歌词 | `https://interface3.music.163.com/eapi/song/lyric/v1` | `/api/song/lyric/v1` |
| 喜欢列表 | `https://interface3.music.163.com/eapi/likelist/get` | `/api/likelist/get` |
| 红心 | `https://interface3.music.163.com/eapi/song/like` | `/api/song/like` |
| 推荐 | `https://interface3.music.163.com/eapi/personalized/playlist` | `/api/personalized/playlist` |

#### QQ 音乐 API 实现 (`qqmusic.rs`)

```rust
// 核心函数清单
pub async fn qq_search(keywords: &str, limit: u32, cookie: &str) -> Result<Vec<Song>>;
pub async fn qq_song_detail(mid: &str, cookie: &str) -> Result<Song>;
pub async fn qq_song_url(mid: &str, media_mid: &str, quality: &str, cookie: &str) -> Result<SongUrlResult>;
pub async fn qq_login_info(cookie: &str) -> Result<LoginInfo>;
pub async fn qq_user_playlists(cookie: &str) -> Result<Vec<Playlist>>;
pub async fn qq_playlist_tracks(id: &str, cookie: &str) -> Result<Vec<Song>>;
pub async fn qq_lyric(mid: &str, id: &str, cookie: &str) -> Result<Lyrics>;
```

**QQ 音乐请求格式**：

```json
POST https://u.y.qq.com/cgi-bin/musicu.fcg
Content-Type: application/json

{
  "comm": { "uin": "<uin>", "format": "json", "ct": 19, "cv": 0 },
  "req_0": {
    "module": "vkey.GetVkeyServer",
    "method": "CgiGetVkey",
    "param": { ... }
  }
}
```

#### Cookie 管理 (`cookie.rs`)

```rust
use tauri_plugin_store::Store;

pub async fn save_cookie(app: &tauri::AppHandle, provider: &str, cookie: &str) -> Result<()>;
pub async fn load_cookie(app: &tauri::AppHandle, provider: &str) -> Result<String>;
pub async fn clear_cookie(app: &tauri::AppHandle, provider: &str) -> Result<()>;
pub fn parse_cookie_string(cookie: &str) -> HashMap<String, String>;
pub fn normalize_cookie_header(raw: &str) -> String;
```

使用 `tauri-plugin-store` 持久化 Cookie，存储路径如 `cookies.json`。

#### 音频/封面代理 (`audio_proxy.rs`)

使用已有的 `axum` 启动本地 HTTP 代理服务器：

```rust
// 启动在 127.0.0.1:随机端口
pub async fn start_audio_proxy() -> Result<u16>;

// 路由
// GET /audio?url=<音频URL>  → 代理音频流，支持 Range
// GET /cover?url=<图片URL>  → 代理封面图片，添加 CORS 头
```

**关键逻辑**：
- 根据音频 URL 域名设置正确的 `Referer`
  - `music.163.com` → `Referer: https://music.163.com/`
  - `qq.com` / `qpic.cn` → `Referer: https://y.qq.com/`
- 支持 `Range` 请求头（分段加载/拖动进度条）
- 返回正确的 `Content-Type`（flac/mp3/m4a/ogg/wav）

### 3.3 Tauri Commands 注册

在 `lib.rs` 中注册所有音乐相关的 Tauri 命令：

```rust
.invoke_handler(tauri::generate_handler![
    // === 搜索 ===
    music_api::netease::cmd_search,
    music_api::qqmusic::cmd_qq_search,

    // === 播放地址 ===
    music_api::netease::cmd_song_url,
    music_api::qqmusic::cmd_qq_song_url,

    // === 登录 ===
    music_api::netease::cmd_login_qr_key,
    music_api::netease::cmd_login_qr_create,
    music_api::netease::cmd_login_qr_check,
    music_api::netease::cmd_login_status,
    music_api::netease::cmd_login_cookie,       // 手动导入 Cookie
    music_api::netease::cmd_logout,
    music_api::qqmusic::cmd_qq_login_status,
    music_api::qqmusic::cmd_qq_login_cookie,
    music_api::qqmusic::cmd_qq_logout,
    music_api::cmd_open_login_window,            // 打开网页登录窗口

    // === 歌单 ===
    music_api::netease::cmd_user_playlist,
    music_api::netease::cmd_playlist_tracks,
    music_api::netease::cmd_playlist_add_song,
    music_api::netease::cmd_playlist_create,
    music_api::qqmusic::cmd_qq_user_playlists,
    music_api::qqmusic::cmd_qq_playlist_tracks,

    // === 喜欢 ===
    music_api::netease::cmd_likelist,
    music_api::netease::cmd_like,
    music_api::netease::cmd_song_like_check,

    // === 歌词 ===
    music_api::netease::cmd_lyric,
    music_api::qqmusic::cmd_qq_lyric,

    // === 推荐 ===
    music_api::netease::cmd_personalized,
    music_api::netease::cmd_recommend_songs,

    // === 代理 ===
    music_api::audio_proxy::cmd_get_proxy_port,

    // === 本地音乐（保留原有） ===
    music::get_music_files,
])
```

### 3.4 登录窗口实现

利用 Tauri 的 `WebviewWindow` 打开网易云/QQ 音乐登录页：

```rust
use tauri::WebviewWindow;

#[tauri::command]
pub async fn cmd_open_login_window(
    app: tauri::AppHandle,
    provider: String,  // "netease" | "qq"
) -> Result<LoginResult, String> {
    let (url, label, cookie_key) = match provider.as_str() {
        "netease" => ("https://music.163.com/#/login", "netease-login", "MUSIC_U"),
        "qq" => ("https://y.qq.com/n/ryqq/profile", "qq-login", "qm_keyst"),
        _ => return Err("Unknown provider".into()),
    };

    // 创建独立 WebviewWindow
    let window = WebviewWindow::builder(
        &app,
        label,
        tauri::WebviewUrl::External(url.parse().unwrap())
    )
    .title("登录")
    .inner_size(940.0, 760.0)
    .build()
    .map_err(|e| e.to_string())?;

    // 轮询检查 Cookie（通过 JS 注入或 webview cookie API）
    // 检测到目标 Cookie 后关闭窗口，保存 Cookie，返回登录信息
    // ...

    Ok(LoginResult { ok: true, cookie: "..." })
}
```

**注意事项**：
- Tauri 2.x 的 `WebviewWindow` 支持 cookie 隔离（不同 label 独立）
- 可以通过 `window.eval()` 注入 JS 检查 `document.cookie`
- 也可以监听导航事件，检测 URL 变化判断登录成功

### 3.5 前端模块划分

#### 新建文件结构

```
src/
├── pages/
│   └── MusicPage.tsx              # 音乐播放器主页面
├── components/
│   ├── music/
│   │   ├── MusicSearchPanel.tsx   # 搜索面板
│   │   ├── MusicPlaylistPanel.tsx # 歌单列表面板
│   │   ├── MusicPlayerBar.tsx     # 底部播放控制栏
│   │   ├── MusicLyrics.tsx        # 歌词显示组件
│   │   ├── MusicLoginModal.tsx    # 登录弹窗
│   │   ├── MusicQueue.tsx         # 播放队列
│   │   └── MusicDiscover.tsx      # 发现/推荐页
│   ├── MusicPlayer.tsx            # 保留：重构为本地+在线
│   └── MiniMusicPlayer.tsx        # 保留：重构
├── contexts/
│   └── music-context.tsx          # 重构：支持在线音乐
├── stores/
│   └── music-store.ts             # Zustand 状态管理
└── hooks/
    └── useMusicApi.ts             # 音乐 API 调用 hooks
```

#### Zustand 状态管理 (`music-store.ts`)

```typescript
interface MusicState {
  // 播放状态
  currentSong: Song | null;
  isPlaying: boolean;
  currentTime: number;
  duration: number;
  volume: number;
  playMode: 'list' | 'shuffle' | 'one';
  playQueue: Song[];
  currentIndex: number;

  // 登录状态
  neteaseLogin: LoginInfo | null;
  qqLogin: LoginInfo | null;
  activeProvider: 'netease' | 'qq' | 'local';

  // 数据
  searchResults: Song[];
  userPlaylists: Playlist[];
  currentPlaylistTracks: Song[];
  likedSongIds: Set<string>;
  currentLyrics: Lyrics | null;

  // 音质
  playbackQuality: 'jymaster' | 'hires' | 'lossless' | 'exhigh' | 'standard';

  // 代理端口
  proxyPort: number;

  // Actions
  play: (song: Song, queue?: Song[]) => void;
  togglePlay: () => void;
  nextTrack: () => void;
  prevTrack: () => void;
  seekTo: (time: number) => void;
  setVolume: (v: number) => void;
  search: (keywords: string) => Promise<void>;
  loadUserPlaylists: () => Promise<void>;
  loadPlaylistTracks: (playlistId: string) => Promise<void>;
  loginNetease: (method: 'qr' | 'web' | 'cookie') => Promise<void>;
  loginQQ: (method: 'web' | 'cookie') => Promise<void>;
  toggleLike: (songId: string) => Promise<void>;
  loadLyrics: (songId: string) => Promise<void>;
  // ...
}
```

#### 前端音频播放逻辑

```typescript
// 播放一首在线歌曲
async function playOnlineSong(song: Song) {
  // 1. 获取播放地址
  const result = await invoke<SongUrlResult>('cmd_song_url', {
    id: song.id,
    quality: playbackQuality,
  });

  if (!result.playable || !result.url) {
    // 处理不可播放情况（VIP/版权/试听限制）
    showToast(result.message || '无法播放');
    return;
  }

  // 2. 通过代理 URL 播放（解决跨域和 Referer）
  const audioUrl = `http://127.0.0.1:${proxyPort}/audio?url=${encodeURIComponent(result.url)}`;

  // 3. 设置 audio 元素并播放
  audio.src = audioUrl;
  audio.play();

  // 4. 加载歌词
  if (song.provider === 'netease') {
    const lyrics = await invoke<Lyrics>('cmd_lyric', { id: song.id });
    set({ currentLyrics: lyrics });
  }
}
```

#### 搜索面板 UI (`MusicSearchPanel.tsx`)

```tsx
function MusicSearchPanel() {
  const [keywords, setKeywords] = useState('');
  const [results, setResults] = useState<Song[]>([]);
  const [searching, setSearching] = useState(false);
  const [provider, setProvider] = useState<'netease' | 'qq' | 'both'>('netease');

  const handleSearch = async () => {
    setSearching(true);
    try {
      if (provider === 'netease' || provider === 'both') {
        const songs = await invoke<Song[]>('cmd_search', { keywords, limit: 30 });
        // ...
      }
      if (provider === 'qq' || provider === 'both') {
        const songs = await invoke<Song[]>('cmd_qq_search', { keywords, limit: 30 });
        // 合并去重，按相关度排序
      }
    } finally {
      setSearching(false);
    }
  };

  return (
    <VStack>
      <HStack>
        <Input value={keywords} onChange={(e) => setKeywords(e.target.value)} />
        <Select value={provider} onChange={(e) => setProvider(e.target.value)}>
          <option value="netease">网易云</option>
          <option value="qq">QQ 音乐</option>
          <option value="both">双平台</option>
        </Select>
        <Button onClick={handleSearch}>搜索</Button>
      </HStack>
      {/* 搜索结果列表 */}
    </VStack>
  );
}
```

### 3.6 开发阶段划分

#### 第一阶段：基础搜索与播放（MVP）

**目标**：能搜索网易云歌曲并播放

1. **Rust 后端**
   - [ ] 从 `netease_lyrics.rs` 提取 EAPI 加密工具到 `music_api/crypto.rs`
   - [ ] 实现 `music_api/netease.rs` 中的 `search` 和 `song_url` 函数
   - [ ] 实现 `music_api/audio_proxy.rs` 音频代理服务器
   - [ ] 注册 Tauri commands: `cmd_search`, `cmd_song_url`, `cmd_get_proxy_port`
   - [ ] 在 `lib.rs` 中启动音频代理服务器（setup 阶段）

2. **React 前端**
   - [ ] 创建 `stores/music-store.ts` Zustand 状态管理
   - [ ] 创建 `pages/MusicPage.tsx` 主页面
   - [ ] 实现 `MusicSearchPanel.tsx` 搜索面板
   - [ ] 实现 `MusicPlayerBar.tsx` 播放控制栏
   - [ ] 重构 `music-context.tsx` 支持在线播放
   - [ ] 在侧边栏添加音乐入口

#### 第二阶段：登录与歌单

**目标**：登录网易云，同步用户歌单

1. **Rust 后端**
   - [ ] 实现 `music_api/cookie.rs` Cookie 持久化
   - [ ] 实现二维码登录：`login_qr_key`, `login_qr_create`, `login_qr_check`
   - [ ] 实现网页登录窗口（WebviewWindow）
   - [ ] 实现 `login_status`, `user_playlist`, `playlist_tracks`
   - [ ] 实现 `likelist`, `like`, `song_like_check`

2. **React 前端**
   - [ ] 实现 `MusicLoginModal.tsx` 登录弹窗（二维码 + 网页登录）
   - [ ] 实现 `MusicPlaylistPanel.tsx` 歌单列表
   - [ ] 实现歌单曲目展示
   - [ ] 实现红心喜欢功能
   - [ ] 实现收藏到歌单

#### 第三阶段：歌词与发现页

**目标**：歌词显示、推荐内容

1. **Rust 后端**
   - [ ] 实现歌词获取（复用已有 `netease_lyrics.rs` 逻辑，适配新接口）
   - [ ] 实现 `personalized`, `recommend_songs` 推荐接口

2. **React 前端**
   - [ ] 实现 `MusicLyrics.tsx` 歌词显示（逐行高亮、翻译、罗马音）
   - [ ] 实现 `MusicDiscover.tsx` 发现页
   - [ ] 实现桌面歌词（可选，Tauri 透明窗口）

#### 第四阶段：QQ 音乐集成

**目标**：双平台搜索与播放

1. **Rust 后端**
   - [ ] 实现 `music_api/qqmusic.rs` 全部 QQ 音乐 API
   - [ ] 实现 QQ 音乐登录窗口
   - [ ] 实现 QQ 歌词（含 QRC 逐字歌词、Base64 解码）

2. **React 前端**
   - [ ] 双平台搜索合并与排序
   - [ ] QQ 歌单展示
   - [ ] 播放失败自动换源（QQ → 网易云 或 反之）

#### 第五阶段：体验优化

**目标**：完善细节，提升体验

1. **功能完善**
   - [ ] 音质切换（播放中无缝切换）
   - [ ] 搜索历史
   - [ ] 播放队列管理
   - [ ] 歌手详情页
   - [ ] 评论展示
   - [ ] 每日推荐 / 私人 FM

2. **UI 优化**
   - [ ] 封面显示与模糊背景
   - [ ] 歌词逐字滚动（QRC）
   - [ ] 播放动画
   - [ ] 暗色/亮色主题适配
   - [ ] 国际化（i18n）

3. **性能优化**
   - [ ] 封面图片缓存
   - [ ] 搜索结果虚拟滚动
   - [ ] 歌词预加载
   - [ ] 音频预加载（下一首）

---

## 四、关键技术难点与解决方案

### 4.1 EAPI 加密

NexBox 已在 `netease_lyrics.rs` 中完整实现了网易云 EAPI 加密：

```rust
const NETEASE_EAPI_KEY: &[u8; 16] = b"e82ckenh8dichen8";

fn encrypt_eapi_payload(api_path: &str, payload_text: &str) -> Result<String, String> {
    let digest_source = format!("nobody{api_path}use{payload_text}md5forencrypt");
    let digest = hex::encode(md5_bytes(digest_source.as_bytes()));
    let data = format!("{api_path}-36cd479b6b5-{payload_text}-36cd479b6b5-{digest}");
    let encrypted = aes_ecb_encrypt_pkcs7(data.as_bytes(), NETEASE_EAPI_KEY);
    Ok(hex::encode_upper(encrypted))
}
```

**复用方案**：将此代码提取到 `music_api/crypto.rs`，供所有网易云 API 调用使用。

### 4.2 音频跨域与 Referer 限制

网易云和 QQ 音乐的音频 URL 存在 Referer 校验，前端 `<audio>` 直接播放会被拒绝。

**解决方案**：在 Rust 后端启动 axum HTTP 代理：

```rust
async fn audio_proxy_handler(uri: Query<AudioQuery>, req: Request) -> Response {
    let audio_url = &uri.url;
    let range = req.headers().get("range").cloned();

    // 设置正确的 Referer
    let referer = if audio_url.contains("qq.com") || audio_url.contains("qpic.cn") {
        "https://y.qq.com/"
    } else {
        "https://music.163.com/"
    };

    // 转发请求
    let mut request = client.get(audio_url)
        .header("Referer", referer)
        .header("User-Agent", UA);
    if let Some(r) = range {
        request = request.header("Range", r);
    }

    let resp = request.send().await?;
    // 流式转发响应...
}
```

### 4.3 登录窗口 Cookie 获取

Tauri 2.x 的 WebviewWindow 不像 Electron 那样直接暴露 `session.cookies` API。

**解决方案**：

方案 A：通过 JS 注入读取 `document.cookie`
```rust
let cookies: String = window.eval("document.cookie").await?;
```
- 缺点：`HttpOnly` Cookie 无法通过 JS 读取

方案 B：使用 Tauri 的 `webview-allows` 和导航拦截
- 监听导航事件，在登录成功跳转后，通过 Rust 的 HTTP 客户端模拟登录请求获取 Cookie

方案 C（推荐）：**二维码登录为主，网页登录为辅**
- 二维码登录完全通过 EAPI 实现，不依赖 WebviewWindow
- 网页登录作为备选，通过 JS 注入获取非 HttpOnly 的 `MUSIC_U` Cookie

### 4.4 QQ 音乐播放授权

QQ 音乐的 `p_skey` 只能证明网页登录态，播放需要 `qm_keyst` / `qqmusic_key` / `music_key`。

**解决方案**：
- 登录窗口加载 QQ 音乐播放器页面进行 warmup
- 等待获取播放授权 Cookie
- 如果只拿到 `p_skey`，标记为 `playbackKeyReady: false`
- 播放失败时自动换源到网易云同名歌曲

### 4.5 音质探测与降级

```rust
pub async fn song_url_with_fallback(id: &str, preferred: &str, cookie: &str) -> SongUrlResult {
    let candidates = quality_candidates_from(preferred);
    let mut trial_fallback = None;

    for q in &candidates {
        match song_url(id, &q.level, cookie).await {
            Ok(result) if result.url.is_some() && !result.trial => {
                return result; // 完整可播放
            }
            Ok(result) if result.url.is_some() && result.trial => {
                if trial_fallback.is_none() {
                    trial_fallback = Some(result);
                }
            }
            _ => {}
        }
    }

    trial_fallback.unwrap_or(SongUrlResult {
        url: None,
        playable: false,
        reason: "url_unavailable".into(),
        ..Default::default()
    })
}
```

---

## 五、依赖与配置变更

### 5.1 Cargo.toml 新增依赖

```toml
[dependencies]
# 已有
# reqwest = { version = "0.11", features = ["stream", "json", "blocking"] }
# aes = "0.8"
# md-5 = "0.10"
# cipher = "0.4"
# axum = "0.7"
# tower-http = { version = "0.5", features = ["fs", "cors", "trace"] }

# 需要新增
hex = "0.4"              # 十六进制编码（EAPI 加密用）
qrcode = "0.14"          # 二维码生成（如果不使用网易云返回的图片）
image = "0.25"           # 图片处理（可选，封面裁剪）
```

### 5.2 package.json 新增依赖

```json
{
  "dependencies": {
    "qrcode.react": "^4.2.0"
  }
}
```

### 5.3 tauri.conf.json 配置

需要在 `capabilities/default.json` 中添加权限：

```json
{
  "permissions": [
    "core:default",
    "core:webview:allow-create-webview-window",
    "core:webview:allow-internal-toggle-devtools",
    "http:default",
    "http:allow-fetch",
    "http:allow-fetch-send",
    "http:allow-fetch-read",
    "http:allow-fetch-cancel"
  ]
}
```

### 5.4 路由配置

在 React Router 中添加音乐页面路由：

```tsx
<Route path="/music" element={<MusicPage />} />
```

---

## 六、API 接口文档

### 6.1 网易云 API（Rust Tauri Commands）

#### 搜索
```typescript
invoke<Song[]>('cmd_search', { keywords: string, limit: number })
```

#### 获取播放地址
```typescript
invoke<SongUrlResult>('cmd_song_url', { id: string, quality: string })
```

#### 二维码登录
```typescript
// 1. 获取 key
invoke<{ key: string }>('cmd_login_qr_key')
// 2. 获取二维码
invoke<{ qrcode: string }>('cmd_login_qr_create', { key: string })
// 3. 轮询检查
invoke<{ code: number, cookie?: string }>('cmd_login_qr_check', { key: string })
// code: 801=等待扫码, 802=待确认, 803=成功, 800=过期
```

#### 网页登录
```typescript
invoke<{ ok: boolean, cookie?: string }>('cmd_open_login_window', { provider: 'netease' })
```

#### 登录状态
```typescript
invoke<LoginInfo>('cmd_login_status')
```

#### 用户歌单
```typescript
invoke<Playlist[]>('cmd_user_playlist')
```

#### 歌单曲目
```typescript
invoke<{ playlist: Playlist, tracks: Song[] }>('cmd_playlist_tracks', { id: string })
```

#### 歌词
```typescript
invoke<Lyrics>('cmd_lyric', { id: string })
```

#### 喜欢/取消
```typescript
invoke<{ success: boolean }>('cmd_like', { id: string, like: boolean })
```

### 6.2 QQ 音乐 API

```typescript
invoke<Song[]>('cmd_qq_search', { keywords: string, limit: number })
invoke<SongUrlResult>('cmd_qq_song_url', { mid: string, mediaMid: string, quality: string })
invoke<LoginInfo>('cmd_qq_login_status')
invoke<Playlist[]>('cmd_qq_user_playlists')
invoke<{ playlist: Playlist, tracks: Song[] }>('cmd_qq_playlist_tracks', { id: string })
invoke<Lyrics>('cmd_qq_lyric', { mid: string, id: string })
```

---

## 七、风险与注意事项

### 7.1 法律与合规

- 本项目仅用于个人学习和研究目的
- 音乐 API 属于逆向工程，平台随时可能变更接口或增加风控
- 不应绕过付费/版权保护机制，播放能力受用户账号权限限制
- VIP/付费歌曲仅在有相应权限的账号下才能播放完整版

### 7.2 接口稳定性

- 网易云 EAPI 接口可能随版本更新而变化
- QQ 音乐的 `musicu.fcg` 接口可能增加签名校验
- 需要定期维护和更新 API 端点
- 建议实现接口降级策略（如 EAPI 失败时尝试 WEAPI）

### 7.3 性能考虑

- 音频代理服务器需要高效流式转发，避免内存堆积
- 搜索结果合并去重需要合理的算法（标题+歌手相似度匹配）
- 封面图片应缓存到本地，避免重复请求
- 歌词解析应在 Rust 端完成，前端只接收结构化数据

### 7.4 用户体验

- 未登录状态下应允许试听（部分歌曲有免费试听片段）
- 播放失败时提供清晰的原因（未登录/VIP/版权/网络）
- 双平台搜索结果合并时需要智能排序（相关度+平台偏好）
- 音质切换应尽量无缝（保存当前播放位置）

---

## 八、开发优先级与时间估算

| 阶段 | 内容 | 预估工时 | 优先级 |
|------|------|----------|--------|
| 1 | 基础搜索与播放（MVP） | 3-5 天 | P0 |
| 2 | 登录与歌单 | 3-5 天 | P0 |
| 3 | 歌词与发现页 | 2-3 天 | P1 |
| 4 | QQ 音乐集成 | 3-5 天 | P1 |
| 5 | 体验优化 | 3-5 天 | P2 |

**总计**：约 14-23 个工作日

---

## 九、参考资源

- [NeteaseCloudMusicApi](https://github.com/Binaryify/NeteaseCloudMusicApi) — 网易云 API 文档
- [NexBox 已有的 EAPI 实现](src-tauri/src/netease_lyrics.rs) — 可直接复用
- [Mineradio server.js](Mineradio-main/server.js) — 完整的 Node.js 实现参考
- [Mineradio QQ 音乐排障记录](Mineradio-main/docs/QQ_MUSIC_INTERFACE_NOTES.md) — QQ 音乐接口注意事项
- [Tauri 2.x WebviewWindow 文档](https://tauri.app/v2/guides/window-customization/) — 登录窗口实现
- [Axum 文档](https://docs.rs/axum) — 音频代理服务器

---

*最后更新：2026-07-09*
