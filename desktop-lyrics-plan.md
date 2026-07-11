# 桌面歌词功能开发计划

> 参考网易云桌面歌词交互体验，为 NexBox 音乐播放器新增独立桌面歌词窗口，支持卡拉OK逐字效果、悬浮控制、锁定穿透、设置弹窗等功能。

---

## 一、用户需求

### 1.1 底部播放器入口
- 在底部播放器（`PlayerBar`）增加两个按钮：
  - **桌面歌词开关按钮**：点击打开/关闭桌面歌词独立窗口
  - **设置按钮**：点击弹出桌面歌词设置弹窗

### 1.2 设置弹窗
- 弹窗内可调节：
  - **字体大小**：滑块控制（如 24px ~ 60px）
  - **歌词颜色**：高亮色（已唱）+ 底色（未唱）颜色选择器
  - **显示句数**：单行（仅当前句）/ 双行（当前句 + 下一句）切换

### 1.3 桌面歌词窗口
- **卡拉OK逐字效果**：复用现有 `buildKaraokeLines` + `getLineProgress` + 双层文字叠加方案
- **悬浮控制栏**：鼠标移入时显示控制按钮——上一句、暂停/播放、下一句、随机播放、锁定
- **未锁定状态**：
  - 可自由拖动窗口位置
  - 鼠标不可穿透（窗口区域拦截点击）
  - 悬浮时显示背景轮廓，提示该区域不可点击穿透
- **锁定状态**：
  - 鼠标可穿透（点击穿过窗口到后方应用）
  - 悬浮时仅显示一个解锁按钮
  - 不显示其他控制按钮
- **整体效果对标网易云桌面歌词**

---

## 二、技术架构

### 2.1 窗口架构

参考现有 `widget` 窗口模式，创建独立的桌面歌词 Tauri 窗口：

```
┌─────────────────────────────────────────────────┐
│  主窗口 (main)                                    │
│  - 音频播放 (HTMLAudioElement)                    │
│  - Zustand Store (music-store)                   │
│  - 歌词获取/解析                                   │
│  - 底部播放器 PlayerBar (+桌面歌词入口按钮)         │
│                                                   │
│  ── emit ────────────────────────►  桌面歌词窗口   │
│  ←── listen ◄────────────────────  (desktop-lyrics)│
│  · desktop-lyrics:data (歌词/歌曲信息)              │
│  · desktop-lyrics:time (高频时间同步)               │
│  · desktop-lyrics:state (播放状态/模式)             │
│  · desktop-lyrics:settings (设置变更)               │
│                                                   │
│  ◄── emit ── desktop-lyrics:control ──►           │
│  · { action: "play-pause" | "prev" | "next" |     │
│  ·   "toggle-shuffle" | "lock" | "unlock" }       │
└─────────────────────────────────────────────────┘
```

#### 桌面歌词窗口配置（`tauri.conf.json` 新增）

```jsonc
{
  "title": "NexBox Desktop Lyrics",
  "label": "desktop-lyrics",
  "url": "/desktop-lyrics",
  "width": 800,
  "height": 120,
  "resizable": false,
  "decorations": false,
  "transparent": true,
  "alwaysOnTop": true,
  "visible": false,
  "maximized": false,
  "maximizable": false,
  "skipTaskbar": true,
  "shadow": false
}
```

### 2.2 跨窗口通信

| 方向 | 事件名 | 频率 | 载荷 |
|------|--------|------|------|
| 主→歌词 | `desktop-lyrics:data` | 切歌时 | `{ song, lyrics, karaokeLines }` |
| 主→歌词 | `desktop-lyrics:time` | ~10Hz (100ms) | `{ currentTime, isPlaying }` |
| 主→歌词 | `desktop-lyrics:state` | 状态变更时 | `{ isPlaying, playMode, volume }` |
| 主→歌词 | `desktop-lyrics:settings` | 设置变更时 | `{ fontSize, highlightColor, baseColor, lineCount }` |
| 主→歌词 | `desktop-lyrics:visibility` | 开关时 | `{ visible }` |
| 歌词→主 | `desktop-lyrics:control` | 用户操作时 | `{ action: ControlAction }` |

#### 时间同步策略（关键）

音频元素在主窗口，桌面歌词窗口无法直接读取 `audioRef.currentTime`。采用**低频同步 + 高频插值**方案：

```
主窗口: 每 100ms emit { currentTime, isPlaying }
    ↓
桌面歌词窗口:
  - 收到时间 → 更新 audioTimeRef + lastSyncRealTimeRef
  - RAF 循环:
    if (isPlaying) {
      estimatedTime = audioTimeRef + (now - lastSyncRealTimeRef) / 1000
    } else {
      estimatedTime = audioTimeRef  // 暂停时时间冻结
    }
  - 用 estimatedTime 计算 activeIndex 和卡拉OK进度
```

此方案以 100ms 低频同步 + 60fps RAF 插值，CPU 开销极低，歌词进度视觉无卡顿。

### 2.3 设置持久化

桌面歌词设置存储在已有的 `tauri-plugin-store`（`music-player-settings.json`），两个窗口共享同一持久化文件：

```typescript
// 新增 Store 字段
desktopLyricsFontSize: number;      // 默认 36
desktopLyricsHighlightColor: string; // 默认 "#FFD700" (金色)
desktopLyricsBaseColor: string;      // 默认 "rgba(255,255,255,0.3)"
desktopLyricsLineCount: 1 | 2;       // 默认 2
desktopLyricsLocked: boolean;        // 默认 false
desktopLyricsPosition?: { x: number; y: number }; // 记忆位置
```

桌面歌词窗口启动时从 Store 读取初始值；主窗口设置弹窗修改后，同时写 Store + emit `desktop-lyrics:settings` 事件热更新。

---

## 三、实现方案

### 3.1 文件清单

#### 新增文件

| 文件路径 | 说明 |
|---------|------|
| `src/pages/DesktopLyricsPage.tsx` | 桌面歌词窗口的 React 页面（独立路由 `/desktop-lyrics`） |
| `src/components/DesktopLyricsSettingsModal.tsx` | 桌面歌词设置弹窗组件 |
| `src/components/desktop-lyrics/LyricsCanvas.tsx` | 桌面歌词渲染主体（卡拉OK效果 + 句数控制） |
| `src/components/desktop-lyrics/LyricsControlBar.tsx` | 悬浮控制栏（上一句/播放/下一句/随机/锁定） |
| `src/hooks/useDesktopLyricsSync.ts` | 跨窗口通信 Hook（时间插值 + 事件收发） |
| `src/lib/desktop-lyrics-window.ts` | 桌面歌词窗口管理工具（打开/关闭/拖动/穿透切换） |

#### 修改文件

| 文件路径 | 修改内容 |
|---------|---------|
| `src-tauri/tauri.conf.json` | 新增 `desktop-lyrics` 窗口配置 |
| `src-tauri/capabilities/default.json` | 新增 `core:window:allow-set-ignore-cursor-events` 权限 + `desktop-lyrics` 窗口 |
| `src/stores/music-store.ts` | 新增桌面歌词设置字段 + setter + 持久化 |
| `src/pages/MusicPage.tsx` | `PlayerBar` 增加桌面歌词开关按钮 + 设置按钮 |
| `src/App.tsx` | 新增 `/desktop-lyrics` 独立路由（无主布局） |

### 3.2 底部播放器入口（PlayerBar 改造）

在 `PlayerBar` 右侧控制区（音量滑块之后、播放队列之前）新增两个按钮：

```
[音质] [音量] [音量滑块] [📝歌词开关] [⚙设置] [📋播放队列]
```

```tsx
// PlayerBar 右侧 HStack 内新增
<Tooltip label="桌面歌词">
  <IconButton
    aria-label="Desktop Lyrics"
    icon={<MonitorSpeaker size={16} />}  // 或 Lyrics 图标
    size="sm"
    variant="ghost"
    sx={{
      color: desktopLyricsVisible ? activeColor : subTextColor,
      _hover: { bg: hoverBg }
    }}
    onClick={() => toggleDesktopLyrics()}
  />
</Tooltip>

<Tooltip label="桌面歌词设置">
  <IconButton
    aria-label="Lyrics Settings"
    icon={<Settings size={16} />}
    size="sm"
    variant="ghost"
    sx={{ color: textColor, _hover: { bg: hoverBg } }}
    onClick={() => setSettingsModalOpen(true)}
  />
</Tooltip>
```

### 3.3 设置弹窗（DesktopLyricsSettingsModal）

使用 Chakra UI `Modal` 组件，玻璃拟态风格：

```
┌──────────────────────────────────────┐
│  桌面歌词设置                    [✕]  │
├──────────────────────────────────────┤
│                                      │
│  字体大小        A ━━━●━━━━ A  36px  │
│                                      │
│  高亮颜色（已唱）  [■ #FFD700]       │
│  底色（未唱）     [■ rgba(...)]      │
│                                      │
│  显示行数   [单行] [双行(当前+下一句)]│
│                                      │
│  预览：                               │
│  ┌──────────────────────────────┐   │
│  │  ██████░░░░░░░░░░░░  (当前句) │   │
│  │  ░░░░░░░░░░░░░░░░░░  (下一句) │   │
│  └──────────────────────────────┘   │
│                                      │
│              [确认]  [取消]          │
└──────────────────────────────────────┘
```

- **字体大小**：`<input type="range" min={24} max={60} step={2}>`
- **颜色选择**：复用现有 `CustomColorPicker` 组件
- **行数切换**：Chakra `ButtonGroup` 单选
- **实时预览**：弹窗内渲染一个迷你 `LyricsCanvas` 预览效果
- 修改即生效（onChange 实时更新 Store + emit 事件），无需点确认

### 3.4 桌面歌词页面（DesktopLyricsPage）

独立路由页面，无主布局（同 `/widget` 模式）：

```tsx
// App.tsx 中新增
if (location.pathname === "/desktop-lyrics") {
  return <DesktopLyricsPage />;
}
```

#### 页面结构

```tsx
function DesktopLyricsPage() {
  const { song, karaokeLines, estimatedTime, isPlaying, playMode,
          settings, isLocked, isHovered } = useDesktopLyricsSync();

  return (
    <Box
      w="100%" h="100%"
      transparent
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onMouseDown={handleDrag}         // 未锁定时拖动
      sx={{
        // 未锁定 + 悬浮时显示背景轮廓
        ...(isHovered && !isLocked ? {
          bg: "rgba(0,0,0,0.15)",
          borderRadius: "12px",
          border: "1px solid rgba(255,255,255,0.15)",
        } : {})
      }}
    >
      {/* 歌词渲染 */}
      <LyricsCanvas
        lines={karaokeLines}
        currentTime={estimatedTime}
        fontSize={settings.fontSize}
        highlightColor={settings.highlightColor}
        baseColor={settings.baseColor}
        lineCount={settings.lineCount}
      />

      {/* 悬浮控制栏 */}
      {isHovered && (
        isLocked ? (
          <UnlockButton onClick={unlock} />  // 锁定时仅显示解锁
        ) : (
          <LyricsControlBar                 // 未锁定显示完整控制
            isPlaying={isPlaying}
            playMode={playMode}
            onPrev={...} onPlayPause={...}
            onNext={...} onShuffle={...}
            onLock={lock}
          />
        )
      )}
    </Box>
  );
}
```

### 3.5 卡拉OK逐字效果（LyricsCanvas）

复用现有 `karaoke-lyrics.ts` 的核心算法，针对桌面歌词场景精简渲染：

```tsx
function LyricsCanvas({ lines, currentTime, fontSize, highlightColor,
                         baseColor, lineCount }) {
  const activeIndex = calcActiveIndex(lines, currentTime);
  const currentLine = lines[activeIndex];
  const nextLine = lines[activeIndex + 1];

  return (
    <VStack spacing={2} justify="center" h="100%">
      {/* 当前行：双层叠加卡拉OK效果 */}
      {currentLine && (
        <KaraokeLine
          line={currentLine}
          nextLine={nextLine}
          currentTime={currentTime}
          fontSize={fontSize}
          highlightColor={highlightColor}
          baseColor={baseColor}
          isActive
        />
      )}

      {/* 下一行：双行模式时显示 */}
      {lineCount === 2 && nextLine && (
        <Text
          fontSize={fontSize * 0.7}  // 下一句略小
          color={baseColor}
          opacity={0.5}
          textAlign="center"
        >
          {nextLine.text}
        </Text>
      )}
    </VStack>
  );
}
```

**KaraokeLine 组件**（精简版，基于现有 `KaraokeLyricLine.tsx` 改造）：

```
双层叠加方案（同现有实现）：
┌─────────────────────────────────────────┐
│  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  │ ← 底层：baseColor, opacity 0.3
│  ████████████░░░░░░░░░░░░░░░░░░░░░░░  │ ← 顶层：highlightColor, width% 裁剪
│  你 是 我 最 美 的 意 外                  │
└─────────────────────────────────────────┘
  ↑ RAF 更新顶层 width = progress * 100%
```

- 有 YRC 逐字数据时：精确逐词进度（`getLineProgress`）
- 无 YRC 时：行级 smoothstep 插值
- 超长歌词：`calculateScrollOffset` 水平滚动
- **文字描边/阴影**：桌面歌词需加描边以保证在任意背景上可读
  ```css
  text-shadow:
    -1px -1px 0 #000, 1px -1px 0 #000,
    -1px 1px 0 #000, 1px 1px 0 #000,
    0 0 8px rgba(0,0,0,0.5);
  ```

### 3.6 锁定/穿透机制（核心难点）

#### 方案：动态切换 `setIgnoreCursorEvents`

利用 Windows 平台 `WS_EX_TRANSPARENT` 特性：设置 `ignoreCursorEvents(true)` 后，窗口仍能接收 `WM_MOUSEMOVE`（鼠标移动事件），但点击事件穿透到后方窗口。

```
状态机：

┌─────────────┐  用户点击锁定  ┌──────────────────────┐
│  UNLOCKED   │ ─────────────► │  LOCKED (passthrough) │
│             │                │  ignoreCursor=true    │
│ ignore=false│  ◄───────────  │  仍接收 mousemove     │
│ 显示完整控制 │  点击解锁按钮   │  仅显示解锁按钮        │
└─────────────┘                └──────────────────────┘
                                    │
                              mousemove 检测到
                              光标在解锁按钮区域
                                    │
                                    ▼
                        ┌─────────────────────┐
                        │  LOCKED_HOVER        │
                        │  ignoreCursor=false  │ ← 临时关闭穿透
                        │  解锁按钮高亮可点击    │
                        └─────────────────────┘
                                    │
                              mouseleave 或
                              点击解锁后
                                    │
                                    ▼
                        返回 UNLOCKED 或 LOCKED
```

**实现伪代码**：

```typescript
// desktop-lyrics-window.ts

import { getCurrentWindow } from "@tauri-apps/api/window";

const lyricsWindow = getCurrentWindow();

// 锁定
export async function lockLyrics() {
  await lyricsWindow.setIgnoreCursorEvents(true);
  setLocked(true);
  // 锁定后窗口仍接收 mousemove（Windows WS_EX_TRANSPARENT 特性）
  // mousemove 事件用于检测光标是否在解锁按钮区域
}

// 解锁
export async function unlockLyrics() {
  await lyricsWindow.setIgnoreCursorEvents(false);
  setLocked(false);
}

// 锁定状态下：mousemove 检测光标是否在解锁按钮区域
function onMouseMove(e: MouseEvent) {
  if (!isLocked) return;
  const unlockBtnRect = getUnlockButtonRect();
  const isOverUnlock = isPointInRect(e.clientX, e.clientY, unlockBtnRect);

  if (isOverUnlock && ignoreCursorEvents) {
    // 光标在解锁按钮上 → 临时关闭穿透，让按钮可点击
    lyricsWindow.setIgnoreCursorEvents(false);
    setHoveringUnlock(true);
  } else if (!isOverUnlock && !ignoreCursorEvents && isLocked) {
    // 光标离开解锁按钮 → 恢复穿透
    lyricsWindow.setIgnoreCursorEvents(true);
    setHoveringUnlock(false);
  }
}
```

**跨平台兼容**：
- **Windows**：`WS_EX_TRANSPARENT` 天然支持 mousemove 转发，方案直接可用
- **macOS/Linux**（未来扩展）：`setIgnoreCursorEvents(true)` 后不接收 mousemove，需用轮询方案——定时器通过 `cursorPosition()` + 窗口位置判断光标是否在区域内，动态切换穿透状态
- 当前 NexBox 为 Windows 应用，优先实现 Windows 方案

#### 拖动实现

未锁定状态下，通过 `startDragging` 拖动窗口（同主窗口无边框拖动方案）：

```typescript
function handleMouseDown(e: React.MouseEvent) {
  if (isLocked) return;
  // 仅在非控制按钮区域触发拖动
  if (e.target === dragAreaRef.current) {
    lyricsWindow.startDragging();
  }
}

// 拖动结束时保存位置
function handleMouseUp() {
  const pos = await lyricsWindow.outerPosition();
  saveDesktopLyricsPosition(pos);
}
```

### 3.7 悬浮控制栏（LyricsControlBar）

未锁定 + 鼠标悬浮时显示，半透明玻璃拟态背景，居中浮于歌词上方：

```
┌────────────────────────────────────────────────┐
│  [⏮] [▶/⏸] [⏭] [🔀] ─── [🔒锁定]              │
└────────────────────────────────────────────────┘
```

```tsx
function LyricsControlBar({ isPlaying, playMode, onPrev, onPlayPause,
                             onNext, onShuffle, onLock }) {
  return (
    <HStack
      spacing={2}
      position="absolute"
      top="50%"
      left="50%"
      transform="translate(-50%, -50%)"
      bg="rgba(0,0,0,0.4)"
      backdropFilter="blur(12px)"
      borderRadius="full"
      px={4} py={2}
      sx={{
        transition: "opacity 0.2s ease",
        opacity: 1,
      }}
      // 阻止控制栏区域触发拖动
      onMouseDown={(e) => e.stopPropagation()}
    >
      <ControlButton icon={<SkipBack size={16} />} onClick={onPrev} />
      <ControlButton
        icon={isPlaying ? <Pause size={18} /> : <Play size={18} />}
        onClick={onPlayPause}
        highlight
      />
      <ControlButton icon={<SkipForward size={16} />} onClick={onNext} />
      <ControlButton
        icon={<Shuffle size={16} />}
        onClick={onShuffle}
        active={playMode === "shuffle"}
      />
      <Divider orientation="vertical" h="16px" />
      <ControlButton icon={<Lock size={16} />} onClick={onLock} />
    </HStack>
  );
}
```

每个控制按钮点击后 emit `desktop-lyrics:control` 事件到主窗口：

```typescript
import { emit } from "@tauri-apps/api/event";

emit("desktop-lyrics:control", { action: "play-pause" });
emit("desktop-lyrics:control", { action: "prev" });
emit("desktop-lyrics:control", { action: "next" });
emit("desktop-lyrics:control", { action: "toggle-shuffle" });
```

主窗口监听：

```typescript
// music-store.ts init() 中
listen<ControlPayload>("desktop-lyrics:control", (event) => {
  const { action } = event.payload;
  switch (action) {
    case "play-pause": get().togglePlay(); break;
    case "prev":       get().prevTrack(); break;
    case "next":       get().nextTrack(); break;
    case "toggle-shuffle":
      // 直接切换到 shuffle 模式
      set({ playMode: "shuffle" });
      break;
  }
});
```

### 3.8 时间同步 Hook（useDesktopLyricsSync）

```typescript
function useDesktopLyricsSync() {
  const [song, setSong] = useState<Song | null>(null);
  const [karaokeLines, setKaraokeLines] = useState<KaraokeLine[]>([]);
  const [isPlaying, setIsPlaying] = useState(false);
  const [playMode, setPlayMode] = useState<PlayMode>("list");
  const [settings, setSettings] = useState(DEFAULT_SETTINGS);
  const [isLocked, setIsLocked] = useState(false);

  // 时间插值
  const audioTimeRef = useRef(0);
  const lastSyncRef = useRef(performance.now());
  const isPlayingRef = useRef(false);
  const [estimatedTime, setEstimatedTime] = useState(0);

  // 监听主窗口事件
  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];

    // 歌词/歌曲数据
    unlisteners.push(
      listen("desktop-lyrics:data", (e) => {
        setSong(e.payload.song);
        setKaraokeLines(e.payload.karaokeLines);
      })
    );

    // 高频时间同步 (100ms)
    unlisteners.push(
      listen("desktop-lyrics:time", (e) => {
        audioTimeRef.current = e.payload.currentTime;
        lastSyncRef.current = performance.now();
        isPlayingRef.current = e.payload.isPlaying;
        if (!e.payload.isPlaying) {
          setEstimatedTime(e.payload.currentTime); // 暂停时直接设置
        }
      })
    );

    // 播放状态
    unlisteners.push(
      listen("desktop-lyrics:state", (e) => {
        setIsPlaying(e.payload.isPlaying);
        setPlayMode(e.payload.playMode);
      })
    );

    // 设置
    unlisteners.push(
      listen("desktop-lyrics:settings", (e) => {
        setSettings(e.payload);
      })
    );

    // 从 Store 读取初始设置
    loadSettingsFromStore().then(setSettings);

    return () => unlisteners.forEach((fn) => fn());
  }, []);

  // RAF 时间插值 (60fps)
  useEffect(() => {
    let rafId: number;
    const tick = () => {
      if (isPlayingRef.current) {
        const elapsed = (performance.now() - lastSyncRef.current) / 1000;
        setEstimatedTime(audioTimeRef.current + elapsed);
      }
      rafId = requestAnimationFrame(tick);
    };
    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  }, []);

  return { song, karaokeLines, estimatedTime, isPlaying, playMode,
           settings, isLocked, setIsLocked };
}
```

### 3.9 主窗口事件发射

在 `music-store.ts` 中新增时间同步发射器：

```typescript
// init() 中启动时间同步
let timeSyncTimer: number | null = null;

function startTimeSync() {
  if (timeSyncTimer) return;
  timeSyncTimer = setInterval(() => {
    const { audioRef, isPlaying } = get();
    if (audioRef) {
      emit("desktop-lyrics:time", {
        currentTime: audioRef.currentTime,
        isPlaying,
      });
    }
  }, 100); // 10Hz
}

function stopTimeSync() {
  if (timeSyncTimer) {
    clearInterval(timeSyncTimer);
    timeSyncTimer = null;
  }
}

// 桌面歌词打开时启动同步，关闭时停止
// 切歌时 emit data 事件
async function playSong(song, queue) {
  // ... 现有逻辑 ...
  // 加载歌词后 emit
  const lyrics = await invoke("music_lyric", { id: song.id });
  const karaokeLines = buildKaraokeLines(lyrics);
  emit("desktop-lyrics:data", { song, lyrics, karaokeLines });
}
```

---

## 四、开发步骤

### Phase 1：窗口基础设施
1. `tauri.conf.json` 新增 `desktop-lyrics` 窗口配置
2. `capabilities/default.json` 新增 `core:window:allow-set-ignore-cursor-events` + `desktop-lyrics` 窗口
3. `App.tsx` 新增 `/desktop-lyrics` 独立路由
4. 创建 `DesktopLyricsPage.tsx` 基础空壳页面
5. 创建 `desktop-lyrics-window.ts` 窗口管理工具（open/close/show/hide）
6. 验证：能从主窗口打开/关闭桌面歌词窗口

### Phase 2：跨窗口通信
7. `music-store.ts` 新增桌面歌词设置字段 + setter + 持久化
8. 实现 `useDesktopLyricsSync.ts` Hook（事件监听 + 时间插值）
9. `music-store.ts` 新增时间同步发射器（100ms emit）
10. 切歌时 emit `desktop-lyrics:data`（song + karaokeLines）
11. `music-store.ts` 监听 `desktop-lyrics:control` 事件
12. 验证：桌面歌词窗口能收到歌词数据和实时时间

### Phase 3：歌词渲染
13. 实现 `LyricsCanvas.tsx`（单行/双行 + 卡拉OK效果）
14. 实现桌面歌词版 KaraokeLine（双层叠加 + RAF + 描边）
15. 支持字体大小、高亮色、底色实时调节
16. 验证：歌词随播放进度逐字高亮

### Phase 4：悬浮控制
17. 实现 `LyricsControlBar.tsx`（上一句/播放/下一句/随机/锁定）
18. 悬浮显示/离开隐藏动画
19. 控制按钮 emit `desktop-lyrics:control` 事件
20. 验证：从桌面歌词窗口控制播放

### Phase 5：锁定/穿透
21. 实现锁定/解锁逻辑（`setIgnoreCursorEvents`）
22. 锁定状态下 mousemove 检测 + 动态切换穿透
23. 解锁按钮悬浮显示
24. 未锁定时拖动窗口 + 位置记忆
25. 未锁定悬浮时显示背景轮廓
26. 验证：锁定穿透、解锁按钮、拖动、轮廓

### Phase 6：设置弹窗
27. 实现 `DesktopLyricsSettingsModal.tsx`
28. 字体大小滑块 + 颜色选择器 + 行数切换
29. 实时预览
30. 设置变更 emit `desktop-lyrics:settings` 热更新
31. `PlayerBar` 增加桌面歌词开关按钮 + 设置按钮
32. 验证：设置实时生效

### Phase 7：优化与打磨
33. 窗口位置记忆/恢复
34. 歌词无数据时的占位提示
35. 窗口关闭时清理事件监听
36. 性能优化（RAF 仅在播放时运行）
37. 边缘情况处理（切歌瞬间的歌词闪烁等）

---

## 五、技术难点与解决方案

### 5.1 锁定状态下显示可点击的解锁按钮

**问题**：`setIgnoreCursorEvents(true)` 使整个窗口点击穿透，解锁按钮无法点击。

**方案**：利用 Windows `WS_EX_TRANSPARENT` 仍转发 mousemove 的特性：
1. 锁定时 `ignoreCursorEvents(true)`，窗口仍接收 mousemove
2. mousemove 判断光标是否在解锁按钮区域
3. 在解锁按钮区域时 → 临时 `ignoreCursorEvents(false)` → 按钮可点击
4. 光标离开 → 恢复 `ignoreCursorEvents(true)`

**备选方案**（如果 mousemove 转发不可靠）：使用独立的小型"解锁按钮"窗口（`desktop-lyrics-unlock` label），始终可交互，跟随歌词窗口位置移动。

### 5.2 跨窗口时间同步精度

**问题**：音频在主窗口，桌面歌词窗口无法直接读取 `currentTime`。

**方案**：低频同步（10Hz）+ 高频插值（60fps RAF）：
- 100ms 误差最大 ~50ms（半帧），卡拉OK进度视觉无感知
- RAF 插值基于 `performance.now()` 差值，精度 < 1ms
- 暂停时冻结时间，不进行插值

### 5.3 歌词事件载荷大小

**问题**：完整歌词（含 YRC 逐字数据）可能较大。

**方案**：主窗口完成解析后，发送已解析的 `KaraokeLine[]`（纯文本 + 时间戳），去除原始 YRC/LRC 冗余数据。典型一首歌 50~80 行，JSON 序列化后 < 10KB，事件传输无压力。

### 5.4 透明窗口文字可读性

**问题**：桌面歌词浮在任意背景上，浅色背景下白字不可读。

**方案**：强制文字描边/阴影：
```css
text-shadow:
  -1px -1px 0 #000, 1px -1px 0 #000,
  -1px 1px 0 #000, 1px 1px 0 #000,
  0 2px 4px rgba(0,0,0,0.6);
```
底色（未唱）默认半透明白色，高亮色默认金色，在深浅背景上均可读。

### 5.5 拖动与点击区分

**问题**：未锁定时，鼠标在歌词区域需要同时支持拖动窗口和点击控制按钮。

**方案**：
- 控制栏 `onMouseDown` 调用 `e.stopPropagation()` 阻止冒泡
- 歌词文本区域 `onMouseDown` 触发 `startDragging()`
- 控制栏区域不触发拖动

---

## 六、UI/交互设计

### 6.1 桌面歌词窗口外观

```
╔══════════════════════════════════════════════════════╗
║                                                        ║
║          ████████████░░░░░░░░░░░░░░░░░░░  ← 当前行    ║
║          ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  ← 下一行    ║
║                                                        ║
╠══════════════════════════════════════════════════════╣
║  未锁定悬浮时：                                         ║
║  ┌──────────────────────────────────────────────┐     ║
║  │  [⏮] [▶] [⏭] [🔀] │ [🔒]                    │     ║
║  └──────────────────────────────────────────────┘     ║
║  + 背景轮廓 (rgba(0,0,0,0.15) + border)               ║
╠══════════════════════════════════════════════════════╣
║  锁定悬浮时（仅解锁按钮）：                              ║
║                                              [🔓]      ║
╚══════════════════════════════════════════════════════╝
```

### 6.2 设置弹窗交互

- 所有设置项**实时生效**（onChange 即更新 Store + emit 事件）
- 弹窗内含迷你预览区，所见即所得
- 颜色选择器复用现有 `CustomColorPicker` 组件
- 行数切换用 `ButtonGroup`，选中态高亮

### 6.3 动画

- 桌面歌词窗口出现/消失：淡入淡出（`opacity` 过渡）
- 控制栏出现/消失：`opacity` + `translateY` 弹性过渡
- 锁定/解锁切换：解锁按钮旋转动画
- 歌词切换：当前行 `scale(1.0→1.02)` 微弹 + 下一行淡入

---

## 七、依赖与风险

### 7.1 新增依赖
- 无新增 npm 依赖（全部使用现有 Tauri API + Chakra UI）
- Tauri 权限：仅需新增 `core:window:allow-set-ignore-cursor-events`

### 7.2 风险评估

| 风险 | 影响 | 缓解 |
|------|------|------|
| `ignoreCursorEvents` mousemove 转发在部分 Windows 版本不可靠 | 锁定后无法检测悬浮 | 备选：独立解锁按钮窗口 或 轮询 `cursorPosition()` |
| 100ms 时间同步延迟导致歌词不同步 | 卡拉OK进度有轻微延迟 | RAF 插值补偿，实际感知 < 50ms |
| 透明窗口在某些桌面环境下闪烁 | 视觉体验差 | 设置 `shadow: false` + 双缓冲 |
| Tauri 事件在高频 emit 时丢包 | 歌词时间跳变 | 时间同步用 100ms 低频，丢包可接受；插值平滑 |

---

## 八、验收标准

- [ ] 底部播放器有桌面歌词开关按钮和设置按钮
- [ ] 点击开关按钮打开/关闭桌面歌词独立窗口
- [ ] 设置弹窗可调节字体大小、颜色、显示行数
- [ ] 设置实时生效，无需重启
- [ ] 桌面歌词显示卡拉OK逐字高亮效果
- [ ] 单行模式只显示当前句，双行模式显示当前句+下一句
- [ ] 鼠标悬浮显示控制栏（上一句/播放/下一句/随机/锁定）
- [ ] 控制栏按钮可控制播放
- [ ] 未锁定时可自由拖动窗口
- [ ] 未锁定悬浮时显示背景轮廓
- [ ] 锁定后鼠标可穿透
- [ ] 锁定后悬浮仅显示解锁按钮
- [ ] 解锁按钮可点击解锁
- [ ] 窗口位置记忆
- [ ] 整体体验对标网易云桌面歌词
