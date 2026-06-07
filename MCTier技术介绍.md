# MCTier 技术架构介绍

## 概述

MCTier 是 NexBox 中的 P2P 联机功能模块，基于 WebRTC 技术实现了一套完整的游戏联机、语音通话、文件共享和屏幕共享解决方案。

---

## 技术栈

- **前端框架**: React + TypeScript + Vite
- **UI 组件库**: Chakra UI + Ant Design
- **状态管理**: Zustand (轻量级状态管理)
- **P2P 技术**: WebRTC + EasyTier (虚拟局域网)
- **桌面应用**: Tauri (Rust 后端 + Web 前端)
- **信令服务器**: 自定义 WebSocket 服务

---

## 核心架构

### 整体架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                           用户界面层                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │  MainWindow  │  │  MiniWindow  │  │  StatusWindow│          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                         状态管理层 (Zustand)                       │
│                  useAppStore (全局状态管理)                       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                          服务层                                   │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────┐  │
│  │  WebRTCClient   │  │  AudioService    │  │ HotkeyMgr   │  │
│  └──────────────────┘  └──────────────────┘  └──────────────┘  │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────┐  │
│  │ FileShareService │  │ScreenShareService│  │ VersionCheck│  │
│  └──────────────────┘  └──────────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                        Tauri 后端层 (Rust)                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │  EasyTier    │  │  文件系统    │  │  音频采集/播放     │  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                        网络通信层                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │  WebSocket   │  │   WebRTC     │  │   虚拟 IP 网络      │  │
│  │  信令服务器   │  │   P2P 连接    │  │   (EasyTier)        │  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 核心模块详解

### 1. 状态管理 - Zustand Store

**文件**: [src/mctier/stores/appStore.ts](file:///d:/NexBox/src/mctier/stores/appStore.ts)

Zustand 是一个轻量级的状态管理库，MCTier 使用它来管理全局应用状态。

#### 核心状态结构

```typescript
interface AppStore {
  // 应用状态
  appState: 'idle' | 'connecting' | 'in-lobby' | 'error';
  
  // 大厅信息
  lobby: Lobby | null;
  
  // 玩家列表
  currentPlayerId: string | null;
  players: Player[];
  
  // 语音状态
  micEnabled: boolean;
  mutedPlayers: Set<string>;
  globalMuted: boolean;
  playerVolumes: Map<string, number>;
  
  // 聊天消息
  chatMessages: ChatMessage[];
  
  // 配置管理
  config: UserConfig;
  
  // UI 状态
  statusWindowCollapsed: boolean;
  miniMode: boolean;
}
```

#### 状态流转

1. **idle** 状态: 初始状态，显示主界面
2. **connecting** 状态: 正在连接信令服务器
3. **in-lobby** 状态: 已进入大厅，显示迷你界面
4. **error** 状态: 发生错误，显示错误信息

---

### 2. WebRTC 客户端 - WebRTCClient

**文件**: [src/mctier/services/webrtc/WebRTCClient.ts](file:///d:/NexBox/src/mctier/services/webrtc/WebRTCClient.ts)

WebRTC 客户端是整个 MCTier 的核心模块，负责处理所有 P2P 连接和信令通信。

#### 核心功能

1. **信令服务器连接**
   - 通过 WebSocket 连接到信令服务器
   - 心跳保活机制
   - 断线自动重连

2. **P2P 连接管理**
   - 创建和管理与每个玩家的 `RTCPeerConnection`
   - 使用字典序 ID 比较决定谁主动发起连接，避免同时发起 Offer
   - 支持 ICE 候选队列和远程描述设置状态管理

3. **媒体流传输**
   - 本地麦克风流采集
   - 远程音频流接收和播放
   - 音量控制

4. **数据通道**
   - 聊天消息通道
   - 文件传输通道 (已迁移至 HTTP 方案)
   - 状态同步通道

5. **事件系统**
   - 玩家加入/离开
   - 远程流到达
   - 状态更新
   - 聊天消息接收

#### 关键数据结构

```typescript
interface PeerConnection {
  id: string;
  connection: RTCPeerConnection;
  dataChannel?: RTCDataChannel;
  fileTransferChannel?: RTCDataChannel;
  audioStream?: MediaStream;
  audioElement?: HTMLAudioElement;
  iceCandidateQueue: RTCIceCandidate[];
  remoteDescriptionSet: boolean;
  connectionTimeout?: number;
  isNegotiating: boolean;
  createdAt: number;
}
```

#### WebSocket 信令消息类型

| 消息类型 | 用途 |
|---------|------|
| `register` | 客户端注册到服务器 |
| `offer` | 发起 WebRTC 连接请求 |
| `answer` | 响应 WebRTC 连接请求 |
| `ice-candidate` | 交换 ICE 候选 |
| `player-joined` | 新玩家加入通知 |
| `player-left` | 玩家离开通知 |
| `status-update` | 玩家状态变更（如麦克风开关） |
| `chat-message` | 聊天消息 |
| `heartbeat` / `pong` | 心跳保活 |
| `screen-share-*` | 屏幕共享相关消息 |
| `file-share-*` | 文件共享相关消息 |

---

### 3. 大厅管理

#### 大厅创建流程

1. 用户输入大厅名称和可选密码
2. 初始化 EasyTier 虚拟局域网
3. 获取虚拟 IP 地址
4. 连接 WebSocket 信令服务器
5. 注册到大厅
6. 初始化 WebRTC 客户端
7. 显示迷你界面

#### 大厅加入流程

1. 用户输入大厅名称和密码
2. 初始化 EasyTier 虚拟局域网
3. 获取虚拟 IP 地址
4. 连接 WebSocket 信令服务器
5. 发送加入请求
6. 接收现有玩家列表
7. 与每个玩家建立 P2P 连接
8. 显示迷你界面

#### 虚拟域名系统

MCTier 支持为玩家分配虚拟域名，方便访问：

```typescript
interface Lobby {
  virtualDomain?: string;    // 虚拟域名
  useDomain?: boolean;       // 是否使用域名访问
}
```

当玩家加入时，如果配置了虚拟域名，会通过 Tauri 后端调用 `add_player_domain` 命令，将域名映射到对应玩家的虚拟 IP 上。

---

### 4. 语音通话系统

#### 音频采集与播放

- 使用浏览器原生的 `getUserMedia` API 采集麦克风
- 通过 WebRTC `MediaStream` 传输音频
- 使用 `HTMLAudioElement` 播放远程音频流

#### 语音控制功能

| 功能 | 说明 |
|------|------|
| 麦克风开关 | 本地麦克风启停，状态同步到所有玩家 |
| 全局静音 | 本地静音/取消静音所有远程玩家 |
| 单独静音 | 对特定玩家单独静音/取消静音 |
| 音量调节 | 为每个玩家独立设置音量 (0.0-1.0) |
| 快捷键 | 支持自定义全局快捷键 |

#### 音频流处理流程

```
本地麦克风 → MediaStream → WebRTC → P2P → 远程播放器
                                                 ↑
远程麦克风 → MediaStream ← WebRTC ← P2P ← 本地播放器
```

---

### 5. 文件共享系统

#### 架构演进

MCTier 的文件共享经历了两次重大架构调整：

1. **第一版**: 通过 WebSocket 信令服务器中继传输（延迟高、慢）
2. **第二版**: 通过 WebRTC DataChannel P2P 传输（延迟低，但实现复杂）
3. **第三版**: 基于 EasyTier 虚拟 IP + HTTP 服务器传输（当前方案）

#### 当前实现

- **本地 HTTP 服务器**: 每个玩家启动一个本地 HTTP 服务器（端口随机）
- **虚拟 IP 通信**: 通过 EasyTier 虚拟 IP 直接访问其他玩家的 HTTP 服务器
- **文件浏览**: 提供目录列表 API，其他玩家可以浏览共享的文件
- **文件下载**: 通过 HTTP GET 直接下载，支持断点续传
- **密码保护**: 共享目录可以设置密码，需要验证才能访问

#### 文件共享服务接口

```typescript
interface FileShare {
  shareId: string;        // 共享 ID
  shareName: string;      // 共享名称
  path: string;          // 本地路径
  hasPassword: boolean;  // 是否需要密码
  password?: string;     // 密码（如果需要）
}
```

#### 事件驱动更新

通过 WebSocket 信令服务器实时同步共享列表变化：
- `file-share-added`: 新共享添加
- `file-share-removed`: 共享被移除
- `file-share-list-request`: 请求共享列表
- `file-share-list-response`: 响应共享列表

---

### 6. 屏幕共享系统

#### 屏幕采集

使用 `getDisplayMedia` API 采集屏幕内容。

#### 屏幕共享流程

1. **发起共享**
   - 选择要共享的屏幕/窗口/标签页
   - 可选设置访问密码
   - 启动共享
   - 通过信令服务器广播共享信息

2. **加入观看**
   - 在共享列表中看到可用的屏幕共享
   - 点击观看（如果需要密码，输入密码）
   - 建立独立的 WebRTC 连接
   - 接收和显示屏幕流

3. **结束共享**
   - 停止屏幕采集
   - 关闭 WebRTC 连接
   - 广播共享结束通知

#### 屏幕共享特性

- 单用户限制: 同一时间只允许一个玩家观看
- 密码保护: 可以设置访问密码
- 事件通知: 开始/结束/状态变化实时通知
- 独立连接: 使用独立的 WebRTC 连接，不占用语音带宽

---

### 7. 聊天系统

**文件**: [src/mctier/services/chat/P2PChatService.ts](file:///d:/NexBox/src/mctier/services/chat/P2PChatService.ts)

#### 聊天消息传输

虽然叫 P2PChatService，但实际聊天消息通过 WebSocket 信令服务器广播，而不是通过 WebRTC DataChannel，原因是：
- 所有玩家都能看到消息（广播需求）
- 消息量小，不占用太多带宽
- 实现简单，不需要每个玩家都发送一次

#### 消息格式

```typescript
interface ChatMessage {
  id: string;           // 消息 ID
  playerId: string;     // 发送者 ID
  playerName: string;  // 发送者名称
  content: string;     // 消息内容
  timestamp: number;   // 时间戳
  type?: 'text' | 'image';  // 消息类型
  imageData?: string;  // 图片数据（Base64）
}
```

#### 新消息提醒

当有新消息且用户不在聊天界面时，播放通知音效。

---

### 8. 快捷键系统

**文件**: [src/mctier/services/hotkey/HotkeyManager.ts](file:///d:/NexBox/src/mctier/services/hotkey/HotkeyManager.ts)

#### 全局快捷键

通过 Tauri 的全局快捷键 API 实现，支持在后台运行时也能响应。

| 功能 | 默认快捷键 |
|------|-----------|
| 麦克风开关 | Ctrl + M |
| 全局静音 | Ctrl + T |
| 按住说话 | F2 |

#### 快捷键记录器

提供了可视化的快捷键设置界面，用户可以自定义每个功能的快捷键。

---

### 9. UI 组件层

#### 主窗口 (MainWindow)

初始界面，包含：
- 创建大厅表单
- 加入大厅表单
- 收藏房间列表
- 大厅历史记录
- 设置窗口
- 关于窗口

#### 迷你窗口 (MiniWindow)

进入大厅后显示的紧凑界面，包含：
- 当前玩家列表
- 语音控制按钮
- 状态显示
- 退出大厅按钮
- 聊天按钮
- 文件共享按钮
- 屏幕共享按钮

#### 状态窗口 (StatusWindow)

悬浮窗显示当前状态，包含：
- 玩家列表（可展开/收起）
- 每个玩家的语音状态
- 音量调节滑块
- 静音按钮

#### 其他组件

- ChatRoom: 聊天室界面
- FileShareManager: 文件共享管理
- ScreenShareManager: 屏幕共享管理
- ScreenViewer: 屏幕查看器
- MinecraftHelper: Minecraft 联机辅助
- SettingsWindow: 设置窗口
- AboutWindow: 关于窗口

---

## 网络架构

### EasyTier 虚拟局域网

EasyTier 是一个基于 WireGuard 的虚拟局域网（VLAN）解决方案，为 MCTier 提供：

1. **虚拟 IP 分配**: 每个玩家获得一个虚拟 IP
2. **NAT 穿透**: 使用 ICE 技术穿透 NAT，无需公网 IP
3. **点对点通信**: 玩家之间直接通信，不经过服务器中转
4. **虚拟域名**: 支持将虚拟 IP 映射到域名

### 信令服务器

信令服务器的作用：
1. 维护房间信息和玩家列表
2. 转发 WebRTC 信令消息 (Offer/Answer/ICE Candidate)
3. 转发聊天消息和状态更新
4. 心跳保活和断线检测

信令服务器不处理：
- 音频/视频数据（通过 WebRTC P2P）
- 文件传输（通过虚拟 IP HTTP）
- 屏幕共享（通过 WebRTC P2P）

### WebRTC ICE 配置

使用公共 STUN 服务器：
- `stun:stun.l.google.com:19302`
- `stun:stun1.l.google.com:19302`

也支持用户配置自定义 TURN 服务器（当前未实现）。

---

## 安全特性

1. **大厅密码**: 房间可以设置密码，只有知道密码的人才能加入
2. **文件共享密码**: 共享的目录可以设置访问密码
3. **屏幕共享密码**: 屏幕共享可以设置访问密码
4. **麦克风默认关闭**: 为了保护隐私，麦克风默认关闭，需要用户手动开启
5. **版本检查**: 客户端版本过低时会提示升级

---

## 版本管理

**文件**: [src/mctier/services/version/VersionCheckService.ts](file:///d:/NexBox/src/mctier/services/version/VersionCheckService.ts)

- 启动时自动检查更新
- 支持版本号比较
- 下载和安装更新
- 版本过低时阻止连接（服务端强制）

---

## 数据流示例

### 玩家加入流程

```
玩家 A (房间创建者)          信令服务器              玩家 B (新加入)
    |                          |                          |
    |-- 注册到服务器 -------->|                          |
    |                          |-- 注册成功 ------------>|
    |                          |                          |
    |                          |-- players-list -------->|
    |                          |   (包含所有在线玩家)    |
    |                          |                          |
    |-- 创建 Offer ----------->|                          |
    |   (因为 A 的 ID > B 的 ID)|                          |
    |                          |-- 转发 Offer ---------->|
    |                          |                          |
    |                          |<-- 发送 Answer ----------|
    |<-- 转发 Answer ----------|                          |
    |                          |                          |
    |-- 发送 ICE Candidate --->|                          |
    |                          |-- 转发 ICE Candidate -->|
    |                          |                          |
    |                          |<-- 发送 ICE Candidate ---|
    |<-- 转发 ICE Candidate ---|                          |
    |                          |                          |
    |<-- P2P 连接建立 -------->|--> P2P 连接建立 -------->|
    |                          |                          |
    |<-- 音频流传输 ---------->|<-- 音频流传输 --------->|
```

### 聊天消息流程

```
发送者                       信令服务器                  所有接收者
   |                            |                            |
   |-- chat-message ----------->|                            |
   |                            |-- 广播给所有其他玩家 --->|
   |                            |                            |
   |                            |                            |-- 显示消息
   |                            |                            |-- 播放提示音(如果不在聊天界面)
```

---

## 关键技术点

### 1. 避免同时发起连接

通过比较玩家 ID 的字典序决定谁主动发起连接：

```typescript
if (localPlayerId > remotePlayerId) {
  // 本地 ID 较大，主动发起连接
  createPeerConnection(remotePlayerId);
  sendOffer(remotePlayerId);
} else {
  // 等待对方发起连接
}
```

### 2. 短时断线检测

使用定时器判断是短时断线还是真正离开：

```typescript
// 收到 player-left 时，不立即处理，等待 3 秒
const leaveTimer = setTimeout(() => {
  // 如果 3 秒内没有收到玩家重新加入的消息，才真正处理离开
  removePlayer(playerId);
}, 3000);

// 如果收到 players-list 或 player-joined 中包含该玩家，取消定时器
clearTimeout(leaveTimer);
```

### 3. 重新协商处理

当网络变化需要重新建立连接时，处理信令状态冲突（Glare）：

```typescript
if (signalingState === 'have-local-offer') {
  // 如果本地已经有一个 Offer，执行 rollback 再处理远程 Offer
  await peerConnection.setLocalDescription({ type: 'rollback' });
}
await peerConnection.setRemoteDescription(offer);
```

### 4. ICE 候选队列

在远程描述未设置之前，将收到的 ICE 候选放入队列：

```typescript
if (!peer.remoteDescriptionSet) {
  peer.iceCandidateQueue.push(candidate);
} else {
  await peer.connection.addIceCandidate(candidate);
}

// 远程描述设置后，处理队列中的候选
for (const candidate of peer.iceCandidateQueue) {
  await peer.connection.addIceCandidate(candidate);
}
peer.iceCandidateQueue = [];
```

---

## 未来优化方向

1. **TURN 服务器**: 当 P2P 无法直接连接时提供中继
2. **视频通话**: 在语音基础上增加视频支持
3. **更多音频编码**: 支持 Opus 等更高效的音频编码
4. **文件传输加密**: 对传输的文件进行加密
5. **多人屏幕共享**: 允许同时观看多个屏幕
6. **群聊改进**: 支持@提及、回复等功能
7. **性能优化**: 减少内存占用，提高连接速度

---

---

## 后端架构详解

### Rust 后端模块组织

MCTier 的后端采用 Rust 语言实现，使用 Tauri 框架提供桌面应用能力，模块结构如下：

```
src-tauri/src/mctier_modules/
├── mod.rs                    # 模块入口与导出
├── app_core.rs               # 应用核心逻辑
├── config_manager.rs         # 配置管理
├── error.rs                  # 错误类型定义
├── lobby_manager.rs          # 大厅管理
├── hosts_manager.rs          # Magic DNS (hosts 文件管理)
├── voice_service.rs          # 语音服务
├── p2p_signaling.rs          # P2P 信令服务
├── websocket_signaling.rs    # WebSocket 信令服务器
├── network_service.rs        # EasyTier 网络服务
├── file_transfer.rs          # HTTP 文件传输服务
├── chat_service.rs           # P2P 聊天服务
├── easytier_advanced_commands.rs # EasyTier 高级配置
├── resource_manager.rs       # 资源管理（二进制文件提取）
├── tauri_commands.rs         # Tauri 命令接口（前端调用入口）
└── tauri_events.rs           # Tauri 事件推送
```

### 核心模块详解

#### 1. WebSocket 信令服务器 (websocket_signaling.rs)

**功能**: 实现本地 WebSocket 信令服务器，用于转发 WebRTC 信令消息

**核心数据结构**:
- `SignalingMessage`: 信令消息枚举
  - `Register`: 客户端注册
  - `PlayersList`: 玩家列表
  - `PlayerJoined`: 玩家加入
  - `PlayerLeft`: 玩家离开
  - `Offer`: WebRTC Offer
  - `Answer`: WebRTC Answer
  - `IceCandidate`: ICE 候选
- `WebSocketSignalingServer`: 服务器主结构体

**关键特性**:
- 支持客户端重连检测（相同 clientId 视为重连，不重复广播加入）
- 消息转发机制（一对一转发 Offer/Answer/ICE）
- 广播机制（玩家加入/离开通知）

#### 2. 网络服务 (network_service.rs)

**功能**: 管理 EasyTier 子进程，提供虚拟局域网连接

**核心流程**:
1. 权限检查（Windows 需要管理员权限创建虚拟网卡）
2. 防火墙规则配置（子网级规则，允许虚拟局域网通信）
3. 提取必要的二进制文件（Packet.dll, wintun.dll, WinDivert64.sys）
4. 查找可用的 RPC 端口
5. 启动 EasyTier 进程
6. 监控标准输出解析虚拟 IP
7. 监控进程健康状态

**关键配置选项**:
- 无 TUN 模式（`--no-tun`）
- DHCP 模式
- SOCKS5 代理
- 端口转发
- 出口节点
- 多线程模式
- 延迟优先模式
- KCP/QUIC 代理
- 加密算法
- 手动路由
- 压缩算法
- STUN 服务器
- 私有模式

#### 3. 文件传输服务 (file_transfer.rs)

**功能**: 基于 HTTP 的高性能文件共享服务

**技术选型**:
- Web 框架: Axum
- 跨域支持: tower-http::CorsLayer

**API 端点**:
- `GET /api/shares`: 获取共享列表
- `GET /api/shares/:share_id/files`: 列出文件（支持路径参数）
- `POST /api/shares/:share_id/verify`: 验证密码
- `GET /api/shares/:share_id/download/*file_path`: 下载文件（支持 Range 请求断点续传）
- `POST /api/shares/:share_id/batch-download`: 批量打包下载（先压后发）

**安全机制**:
- 密码保护（HTTP Header: `x-share-password`）
- 路径安全检查（确保不越界访问共享目录外的文件）

#### 4. 大厅管理 (lobby_manager.rs)

**功能**: 创建/加入/退出大厅，管理玩家列表

**核心数据结构**:
- `Lobby`: 大厅信息
  - 名称
  - 密码（加密存储）
  - 服务器节点地址
  - 信令服务器地址
  - 虚拟 IP
  - 虚拟域名
- `Player`: 玩家信息

#### 5. Tauri 命令接口 (tauri_commands.rs)

**功能**: 提供前端可调用的所有命令，是前后端交互的桥梁

**主要命令分类**:
1. **大厅操作**: `create_lobby`, `join_lobby`, `leave_lobby`
2. **语音控制**: `toggle_mic`, `mute_player`, `mute_all`
3. **配置管理**: `get_config`, `update_config`, `save_opacity`
4. **系统信息**: `get_audio_devices`, `get_app_state`, `get_current_lobby`, `get_players`
5. **窗口控制**: `set_always_on_top`, `toggle_mini_mode`, `set_window_opacity`
6. **WebRTC 信令**: `send_signaling_message`, `broadcast_status_update`, `send_heartbeat`
7. **网络管理**: `force_stop_easytier`
8. **网络诊断**: `check_virtual_adapter`, `check_firewall_rules`, `ping_virtual_ip`, `check_udp_port`
9. **系统设置**: `set_auto_start`, `check_auto_start`
10. **Magic DNS**: `add_player_domain`, `remove_player_domain`
11. **文件共享**: `get_folder_name`, `get_folder_info`, `list_directory_files`, `read_file_bytes`, `write_file_bytes`

---

## 完整数据流示例

### 1. 创建大厅完整流程

```
用户点击"创建大厅"
    ↓
前端调用 Tauri 命令 `create_lobby`
    ↓
后端:
  1. 更新应用状态为 Connecting
  2. 读取全局和大厅配置
  3. 获取 Lock -> LobbyManager, NetworkService
  4. 调用 lobby_manager.create_lobby_with_config()
     ├─ 停止旧的 EasyTier 进程
     ├─ 启动新的 EasyTier 进程
     │   ├─ 检查管理员权限
     │   ├─ 添加防火墙规则
     │   ├─ 提取必要的 DLL 文件
     │   ├─ 查找可用 RPC 端口
     │   ├─ 构建并启动 EasyTier 命令
     │   └─ 监控输出解析虚拟 IP
     ├─ 创建大厅记录
     └─ 返回 Lobby 对象
  5. 设置文件服务虚拟 IP（不自动启动服务器）
  6. 启动 P2P 聊天服务器
  7. 更新应用状态为 InLobby
    ↓
前端:
  1. 连接到信令服务器
  2. 初始化 WebRTC 客户端
  3. 显示迷你窗口
```

### 2. 玩家加入大厅完整流程

```
玩家 B 点击"加入大厅"
    ↓
前端调用 `join_lobby`
    ↓
后端:
  1. 更新状态为 Connecting
  2. 启动 EasyTier 网络服务
  3. 加入大厅
  4. 初始化语音服务
  5. 获取虚拟 IP
  6. 启动 P2P 信令服务
  7. 设置文件服务 IP（不自动启动服务器）
  8. 启动 P2P 聊天服务器
  9. 更新状态为 InLobby
    ↓
前端:
  1. 连接 WebSocket 信令服务器
  2. 注册玩家信息
  3. 收到 PlayersList（已有玩家列表）
  4. 对每个已有玩家:
     ├─ 比较 ID 字典序
     ├─ 如果本地 ID > 远程 ID，主动发起 Offer
     └─ 建立 WebRTC PeerConnection
  5. 建立语音通信
  6. 显示迷你窗口
```

### 3. 文件共享流程

```
玩家 A 添加共享目录
    ↓
前端调用相应命令
    ↓
后端:
  1. FileTransferService.add_share()
  2. 按需启动 HTTP 服务器（第一次添加共享时）
  3. 服务器绑定 0.0.0.0:14539
    ↓
玩家 B 访问共享:
  1. 调用 GET /api/shares 获取共享列表
  2. 选择一个共享
  3. 如有密码，调用 POST /api/shares/:id/verify 验证
  4. 调用 GET /api/shares/:id/files 浏览文件
  5. 调用 GET /api/shares/:id/download/*path 下载文件
     (支持 Range 请求实现断点续传)
```

---

## 关键技术实现细节

### 1. 防火墙规则配置

为了支持虚拟局域网通信和 Minecraft 联机，MCTier 自动配置 Windows 防火墙：

- **入站规则**（子网名 "MCTier P2P Network"）:
  - 允许来自 10.126.126.0/24 的所有协议（TCP/UDP/ICMP）
  - 覆盖所有网络配置文件

- **出站规则**:
  - 允许从虚拟子网（10.126.126.0/24）发出的所有流量
  - 支持 Minecraft LAN 发现（UDP 组播）

### 2. Magic DNS 实现

MCTier 支持虚拟域名功能：
- 使用 `HostsManager` 管理系统 hosts 文件
- 当玩家加入启用了虚拟域名的大厅时
- 自动将玩家名映射到其虚拟 IP（如 `player1.mct.net` -> `10.126.126.5`）
- 退出大厅时自动清理 hosts 文件

### 3. 进程管理与资源清理

- EasyTier 进程使用 `tokio::process::Child` 管理
- 支持优雅关闭与强制终止（Windows 使用 `taskkill /F`）
- 退出大厅时自动清理：
  - 停止 EasyTier 进程
  - 停止 HTTP 文件服务器
  - 停止 P2P 信令服务
  - 清理语音服务资源
  - 恢复 hosts 文件

### 4. 配置管理架构

- 配置分为全局配置和大厅配置
- 大厅配置可选择是否继承全局配置（`use_global_config`）
- 支持 EasyTier 高级配置（20+ 个选项）
- 配置持久化存储在用户数据目录

---

## 总结

MCTier 是一个功能完整、架构清晰的 P2P 联机解决方案：

**前端技术栈**: React + TypeScript + Zustand + WebRTC
**后端技术栈**: Rust + Tauri + Tokio + Axum
**网络技术**: EasyTier (WireGuard-based) + WebRTC + WebSocket

**核心优势**:
1. 完整的虚拟局域网支持，游戏兼容性好
2. 低延迟语音通话
3. 高性能 HTTP 文件共享（支持断点续传）
4. 屏幕共享功能
5. 模块化架构，易于维护和扩展
6. 完善的错误处理和诊断功能

代码采用清晰的分层设计：UI 层 → 状态管理层 → 服务层 → 后端层 → 网络层，各模块职责明确，协作高效。
