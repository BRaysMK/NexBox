# 音乐搜索增强 — 歌单搜索 + 搜索标签导航

## 概述

为 NexBox 音乐页面增加**歌单搜索**功能，并在搜索结果页顶部添加**「单曲 | 歌单 | 歌手」**三标签导航栏。

## 修改的文件

| 文件 | 变更说明 |
|------|----------|
| `src-tauri/src/music_api/netease.rs` | 新增 `playlist_search()` 函数 |
| `src-tauri/src/music_api/mod.rs` | 注册 `music_playlist_search` Tauri 命令 |
| `src-tauri/src/lib.rs` | 将新命令注册到 Tauri builder |
| `src/stores/music-store.ts` | 新增 `playlistSearchResults`、`searchingPlaylists` 状态和 `searchPlaylists()` action |
| `src/pages/MusicPage.tsx` | 搜索结果页添加三标签导航栏，歌单结果卡片展示 |

---

## 1. 后端实现

### `netease.rs` — `playlist_search()`

- 调用网易云音乐 `cloudsearch/get/web` API，参数 `type=1000`（歌单类型）
- 返回 `Vec<Playlist>`，字段映射：
  - `id`、`name`、`coverImgUrl`（封面）、`trackCount`（曲目数）、`creator.nickname`（创建者）

```rust
pub async fn playlist_search(keywords: &str, limit: u32, cookie: &str) -> Result<Vec<Playlist>, String>
```

### `mod.rs` — 命令注册

```rust
#[tauri::command]
pub async fn music_playlist_search(keywords: String, limit: Option<u32>) -> Result<Vec<Playlist>, String>
```

---

## 2. 前端 Store

### `music-store.ts`

新增状态：

```ts
playlistSearchResults: Playlist[];  // 默认 []
searchingPlaylists: boolean;        // 默认 false
```

新增 action：

```ts
searchPlaylists(keywords: string) => Promise<void>
// 调用 invoke<Playlist[]>("music_playlist_search", { keywords, limit: 30 })
```

---

## 3. UI 变更

### 搜索流程

用户输入关键词 → 回车/点击搜索 → **并行调用三种搜索**：

```ts
Promise.all([
  storeActions.search(value),        // type=1  歌曲
  storeActions.searchArtists(value),  // type=100 歌手
  storeActions.searchPlaylists(value), // type=1000 歌单 ← 新增
])
```

### 搜索结果页布局

```
┌─────────────────────────────────────┐
│ ← 搜索 "关键词"                     │
├─────────────────────────────────────┤
│ [🎵 单曲 (12)] [📋 歌单 (5)] [👤 歌手 (3)]  │  ← 新增标签栏
├─────────────────────────────────────┤
│                                     │
│ 根据标签展示对应结果：               │
│ - 单曲：歌曲行列表                   │
│ - 歌单：封面卡片列表（封面/名称/曲目数/创建者）│
│   点击 → 加载歌单曲目 → 返回主视图   │
│ - 歌手：歌手卡片网格                 │
│                                     │
├─────────────────────────────────────┤
│ PlayerBar                           │
└─────────────────────────────────────┘
```

**注意：** 标签导航栏**仅在搜索结果页显示**，音乐主页保持原有布局不变。

### 标签切换

- 点击标签切换显示对应结果，**不重新搜索**（三种结果已在搜索时一次性获取）
- 每个标签显示该类结果的数量
- 当前选中标签高亮

---

## 4. 交互细节

- **歌单卡片点击**：在搜索结果页内加载该歌单的曲目列表，显示歌单头部信息 + 完整曲目，可点击 "← 返回搜索结果" 回到搜索结果
- **歌单播放按钮**：与卡片点击行为一致（在搜索结果页内展开曲目）
- **搜索下拉预览**：保持不变（仅显示歌曲预览，因为预览是边输入边触发，不触发歌单搜索）

---

## 5. 官方榜单入口

在音乐主页右侧面板的推荐歌单上方，新增 **「官方榜单」** 快捷入口：

| 榜单 | 歌单 ID | 说明 |
|------|---------|------|
| 🔥 热歌榜 | `3778678` | 网易云官方热歌榜 |
| 📈 飙升榜 | `19723756` | 网易云官方飙升榜 |
| 🎵 新歌榜 | `3779629` | 网易云官方新歌榜 |

- 以紧凑按钮组展示，点击后在右侧面板加载对应榜单的曲目列表
- 榜单入口无需登录即可使用

---

## 6. 原有功能影响

| 原有功能 | 是否影响 |
|----------|----------|
| 歌曲搜索 (type=1) | 无影响 |
| 歌手搜索 (type=100) | 无影响 |
| 音乐主页布局 | 无影响（标签栏仅搜索结果页显示） |
| 搜索下拉预览 | 无影响（仅歌曲预览） |
| 播放控制 | 无影响 |

---

## 6. 参考

- Mineradio-main 开源项目：`type=1000` 参数搜索歌单
- 网易云 API：`https://music.163.com/api/cloudsearch/get/web`
