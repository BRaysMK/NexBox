# 音乐页面性能问题诊断与修复方案

## 问题描述

- **症状**：音乐页面刚打开时非常流畅，使用一段时间后整个页面所有组件都变卡
- **范围**：整个音乐页面所有组件卡顿，非单个组件问题
- **约束**：播放器动态背景和歌曲列表已优化完毕，不在本次修改范围

---

## 诊断结论

经逐文件深度审查，发现 **14 个性能问题**，其中 **5 个是导致"用久了卡"的直接原因**。核心机制是：**资源（事件监听器、GPU 图层、Emotion 样式缓存、Image/Canvas 对象）随时间累积不释放，导致每次渲染的 CPU/GPU 开销逐渐增大。**

---

## 问题分级总览

| 级别 | 数量 | 说明 |
|------|------|------|
| 🔴 严重 | 5 | 直接导致"用久了卡"，必须修复 |
| 🟡 中等 | 5 | 间接加速性能下降，建议修复 |
| 🟢 轻微 | 4 | 小幅影响，有余力时修复 |

---

## 🔴 严重问题（直接导致卡顿）

### 问题 1：Tauri `listen()` 事件监听器永不取消

**文件**：`src/stores/music-store.ts`，第 369-431 行

**现状**：
```typescript
// init() 中注册了 4 个监听器，返回的 UnlistenFn 从未存储或调用
listen<{ action: string }>("desktop-lyrics:control", (event) => { ... });
listen("desktop-lyrics:request-data", () => { ... });
listen<LoginInfo>("netease-login-success", async (event) => { ... });
listen<string>("netease-login-failed", (event) => { ... });
```

**问题**：
- `listen()` 返回 `Promise<UnlistenFn>`，但返回值被丢弃
- `listenersRegistered` 布尔标志只防止 Strict Mode 双注册，不负责清理
- 每次 HMR 或应用热重载时，旧监听器仍留在 Tauri 后端事件系统中
- 监听器回调闭包持续引用 store 函数，阻止 GC
- 事件分发时，所有历史监听器都会被触发，CPU 开销随时间线性增长

**修复方案**：
```typescript
// 在模块级别存储 unlisten 函数
const unlistenFns: (() => void)[] = [];

// 在 init() 中：
if (listenersRegistered) return;
listenersRegistered = true;

const unlisten1 = await listen<{ action: string }>("desktop-lyrics:control", (event) => { ... });
unlistenFns.push(unlisten1);

const unlisten2 = await listen("desktop-lyrics:request-data", () => { ... });
unlistenFns.push(unlisten2);

const unlisten3 = await listen<LoginInfo>("netease-login-success", async (event) => { ... });
unlistenFns.push(unlisten3);

const unlisten4 = await listen<string>("netease-login-failed", (event) => { ... });
unlistenFns.push(unlisten4);

// 导出清理函数，供应用卸载时调用
export function cleanupMusicListeners() {
  unlistenFns.forEach((fn) => fn());
  unlistenFns.length = 0;
  listenersRegistered = false;
}
```

---

### 问题 2：`useCoverColor` Image/Canvas 对象未清理

**文件**：`src/hooks/use-cover-color.ts`，第 74-91 行

**现状**：
```typescript
useEffect(() => {
  if (!coverUrl || coverUrl === lastUrlRef.current) return;
  lastUrlRef.current = coverUrl;

  const img = new Image();
  img.crossOrigin = "anonymous";
  img.onload = () => {
    const c = extractDominantColor(img); // 内部创建 canvas
    setColor(c);
  };
  img.src = coverUrl;
  // ← 没有 return cleanup
}, [coverUrl]);
```

**问题**：
- 每次切歌创建 `new Image()`，`onload` 闭包持有 `setColor` 引用
- 如果组件卸载时图片仍在加载，`onload` 仍会触发，操作已卸载组件状态
- `extractDominantColor` 内部每次创建 `document.createElement("canvas")`，Canvas 的 GPU 纹理不保证立即释放
- 快速切歌时多个 Image + Canvas 同时存在，内存峰值飙升
- 长时间使用后，浏览器 GPU 进程内存持续增长

**修复方案**：
```typescript
export function useCoverColor(coverUrl: string): CoverColor {
  const [color, setColor] = useState<CoverColor>({ hex: "#1a1a2e", isLight: false, rgb: [26, 26, 46] });
  const lastUrlRef = useRef("");

  useEffect(() => {
    if (!coverUrl || coverUrl === lastUrlRef.current) return;
    lastUrlRef.current = coverUrl;

    let cancelled = false;
    const img = new Image();
    img.crossOrigin = "anonymous";

    img.onload = () => {
      if (cancelled) return; // 组件已卸载，丢弃结果
      const c = extractDominantColor(img);
      if (!cancelled) setColor(c);
    };

    img.onerror = () => {
      if (!cancelled) setColor({ hex: "#1a1a2e", isLight: false, rgb: [26, 26, 46] });
    };

    img.src = coverUrl;

    return () => {
      cancelled = true;
      img.onload = null;   // 断开闭包引用
      img.onerror = null;  // 断开闭包引用
      img.src = "";        // 取消正在进行的图片加载
    };
  }, [coverUrl]);

  return color;
}
```

---

### 问题 3：`scrollbarSx(activeColor)` 内联调用 — 每次渲染创建新对象

**文件**：`src/pages/MusicPage.tsx`，至少 9 处调用

**现状**：
```tsx
// 函数定义（每次调用返回新对象）
const scrollbarSx = (color: string) => ({
  "&::-webkit-scrollbar": { width: "4px" },
  "&::-webkit-scrollbar-thumb": { background: color, borderRadius: "2px" },
  "&::-webkit-scrollbar-track": { background: "transparent" },
});

// 使用处（每次渲染创建新对象）
<VirtualList scrollbarSx={scrollbarSx(activeColor)} ... />
<Box sx={scrollbarSx(activeColor)} ... />
```

**问题**：
- `scrollbarSx(activeColor)` 是普通函数调用，每次渲染返回新的 `{}` 对象
- VirtualList 是 `memo` 组件，`scrollbarSx` prop 引用变化导致 memo 失效
- `<Box sx={...}>` 每次渲染创建新 sx 对象，Emotion 需重新序列化+哈希
- 至少 9 处调用，每次父组件重渲染产生 9 个新对象
- ExpandedPlayer 中已用 `useMemo` 优化了 1 处（第 473 行），其余 8 处未优化

**修复方案**：
```tsx
// 在 MusicPage 组件顶层统一 memoize
const memoScrollbarSx = useMemo(() => scrollbarSx(activeColor), [activeColor]);

// 所有使用处改为引用 memoScrollbarSx
<VirtualList scrollbarSx={memoScrollbarSx} ... />
<Box sx={memoScrollbarSx} ... />
```

注意：SearchBox 和 ExpandedPlayer 是独立组件，各自需要内部 memoize。ExpandedPlayer 已有（第 473 行），SearchBox 需补充。

---

### 问题 4：`renderSongRow` 未用 `useCallback` 包装，破坏 VirtualList memo

**文件**：`src/pages/MusicPage.tsx`，第 2237 行 + VirtualList 调用处

**现状**：
```tsx
// renderSongRow 是普通函数，每次渲染创建新引用
const renderSongRow = (song: Song, index: number, queue: Song[]) => (
  <SongRow ... />
);

// 传给 VirtualList 时还套了一层箭头函数，也是新引用
<VirtualList
  renderItem={(song, i) => renderSongRow(song, i, leftPlaylistTracks)}
  ...
/>
```

**问题**：
- `renderSongRow` 不是 `useCallback`，每次 MusicPage 重渲染都创建新函数
- 传给 VirtualList 的 `(song, i) => renderSongRow(...)` 是内联箭头函数，引用每次都变
- VirtualList 的 `memo()` 做浅比较时 `renderItem` prop 不相等 → memo 完全失效
- MusicPage 订阅了 24 个 store 字段，任意一个变化都触发 MusicPage 重渲染
- 每次重渲染 → 所有 VirtualList 重渲染 → 所有可见 SongRow 重渲染
- 这是为什么"用久了越来越卡"的关键：随 store 数据增多，重渲染频率上升

**修复方案**：
```tsx
// 用 useCallback 包装 renderSongRow
const renderSongRow = useCallback((song: Song, index: number, queue: Song[]) => (
  <SongRow
    key={`${song.provider}-${song.id}-${index}`}
    song={song}
    index={index}
    queue={queue}
    isCurrent={currentSong?.id === song.id}
    isPlaying={isPlaying}
    isLiked={likedSongIds.has(song.id)}
    isLoggedIn={!!loginInfo?.logged_in}
    proxyPort={proxyPort}
    activeColor={activeColor}
    hoverBg={hoverBg}
    itemHoverBg={itemHoverBg}
    itemActiveBg={itemActiveBg}
    textColor={textColor}
    subTextColor={subTextColor}
    liquidGlassEnabled={liquidGlassEnabled}
    onPlay={onPlay}
    onTogglePlay={onTogglePlay}
    onToggleLike={onToggleLike}
    onArtistClick={handleArtistClick}
  />
), [currentSong, isPlaying, likedSongIds, loginInfo, proxyPort, activeColor, hoverBg, itemHoverBg, itemActiveBg, textColor, subTextColor, liquidGlassEnabled, onPlay, onTogglePlay, onToggleLike, handleArtistClick]);

// VirtualList 调用处用 useCallback 包装箭头函数
const renderLeftTracks = useCallback(
  (song: Song, i: number) => renderSongRow(song, i, leftPlaylistTracks),
  [renderSongRow, leftPlaylistTracks]
);
// <VirtualList renderItem={renderLeftTracks} ... />
```

---

### 问题 5：LiquidGlassCard `willChange` 导致 GPU 图层累积

**文件**：`src/components/special/liquid-glass-card.tsx`，第 54 行

**现状**：
```tsx
sx: {
  transform: "translateZ(0)",
  WebkitTransform: "translateZ(0)",
  WebkitBackfaceVisibility: "hidden",
  backfaceVisibility: "hidden",
  willChange: "backdrop-filter, transform",  // ← 问题
},
```

**问题**：
- `willChange` 告诉浏览器为该元素创建独立的合成器图层（GPU 内存）
- 页面上同时存在大量 LiquidGlassCard（左侧面板、右侧面板、PlayerBar、搜索结果等）
- 每个 LiquidGlassCard 都创建一个 GPU 图层
- `backdrop-filter` 本身是昂贵的 GPU 操作（需要采样背后内容）
- 组件卸载后 GPU 图层不保证立即释放
- 随页面交互（切换视图、打开/关闭 ExpandedPlayer），图层不断创建/销毁，GPU 内存碎片化
- 长时间使用后 GPU 内存压力增大，合成器帧率下降 → 整页面卡顿

**修复方案**：
```tsx
sx: {
  transform: "translateZ(0)",
  WebkitTransform: "translateZ(0)",
  // 移除 willChange — translateZ(0) 已经创建了合成层
  // willChange 应该只在动画即将开始时设置，动画结束后移除
  // 对于静态卡片，不需要 willChange
},
```

如果确实需要 `backdrop-filter` 的硬件加速，可以只在 `liquidGlassEnabled` 为 true 时设置：
```tsx
sx: {
  transform: "translateZ(0)",
  WebkitTransform: "translateZ(0)",
  ...(liquidGlassEnabled ? { willChange: "backdrop-filter" } : {}),
},
```

---

## 🟡 中等问题（间接加速性能下降）

### 问题 6：`timeSyncTimer` 在页面卸载时不清理

**文件**：`src/stores/music-store.ts`，第 162-182 行

**现状**：
```typescript
let timeSyncTimer: ReturnType<typeof setInterval> | null = null;

function startTimeSync() {
  if (timeSyncTimer) return;
  timeSyncTimer = setInterval(() => {
    const state = useMusicStore.getState();
    if (state.audioRef && state.desktopLyricsVisible) {
      emit("desktop-lyrics:time", { ... });
    }
  }, 100);  // 每 100ms 执行
}
```

**问题**：
- 如果用户离开音乐页面时桌面歌词仍开启，100ms 间隔的定时器持续运行
- 每次 `emit()` 通过 Tauri IPC 分发事件，有序列化+跨进程通信开销
- 即使回到音乐页面，定时器可能已经存在多个实例（如果 `stopTimeSync` 未被正确调用）

**修复方案**：
```typescript
// 导出 stopTimeSync，在 MusicPage 卸载时调用
useEffect(() => {
  return () => {
    // 离开页面时停止时间同步（不关闭桌面歌词）
    stopTimeSync();
  };
}, []);
```

---

### 问题 7：ProgressSection 和 PlayerProgress 重复 `timeupdate` 监听

**文件**：`src/pages/MusicPage.tsx`，第 326 行和第 1199 行

**现状**：
- `ProgressSection`（ExpandedPlayer 内）和 `PlayerProgress`（PlayerBar 内）各自添加 `timeupdate` 监听
- 当 ExpandedPlayer 打开时，PlayerBar 仍然挂载在页面上
- 两个组件同时监听同一 `audioRef` 的 `timeupdate` 事件
- 每 ~250ms 两次 `setLocalCurrentTime` → 两次组件重渲染

**修复方案**：
- 方案 A：当 `expandedPlayer` 为 true 时，不渲染 PlayerBar 的进度条部分
- 方案 B（推荐）：PlayerProgress 内部判断是否被遮挡，如果 ExpandedPlayer 打开则跳过 setState：

```tsx
const onTimeUpdate = () => {
  if (isUserSeekingRef.current) return;
  // 如果 ExpandedPlayer 打开，跳过更新（它的 ProgressSection 在处理）
  if (useMusicStore.getState().audioRef !== audioRef) return;
  setLocalCurrentTime(audioRef.currentTime);
};
```

更简单的方案：在 MusicPage 中当 expandedPlayer 为 true 时给 PlayerBar 传一个 `hidden` prop。

---

### 问题 8：SearchBox debounce 定时器未在卸载时清理

**文件**：`src/pages/MusicPage.tsx`，第 1755、1808 行

**现状**：
```tsx
const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

// 搜索时设置 debounce
debounceRef.current = setTimeout(async () => {
  await storeActions.search(value);
  // ...
}, 300);
// ← 组件卸载时未清除 debounceRef.current
```

**问题**：
- 组件卸载后，挂起的 setTimeout 仍会触发
- 回调中调用 `storeActions.search()` → `set({ searching: true })` → 触发已卸载组件订阅者的重渲染
- React 会警告 "Can't perform a React state update on an unmounted component"

**修复方案**：
```tsx
useEffect(() => {
  return () => {
    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
      debounceRef.current = null;
    }
  };
}, []);
```

---

### 问题 9：`handleCloseWithAnimation` 的 `setTimeout` 未清理

**文件**：`src/pages/MusicPage.tsx`，第 491-494 行

**现状**：
```tsx
const handleCloseWithAnimation = useCallback(() => {
  setIsClosing(true);
  setTimeout(() => onClose(), 300); // ← 未清理
}, [onClose]);
```

**问题**：
- 如果用户在 300ms 动画期间快速操作导致组件卸载，定时器仍触发 `onClose()`
- 操作已卸载组件可能引发警告或异常

**修复方案**：
```tsx
const closeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

const handleCloseWithAnimation = useCallback(() => {
  setIsClosing(true);
  closeTimerRef.current = setTimeout(() => onClose(), 300);
}, [onClose]);

useEffect(() => {
  return () => {
    if (closeTimerRef.current) clearTimeout(closeTimerRef.current);
  };
}, []);
```

---

### 问题 10：KaraokeLyricLine 的 `willChange` 属性

**文件**：`src/components/KaraokeLyricLine.tsx`，第 156、182 行

**现状**：
```tsx
// scrollRef
willChange: "transform",

// overlayRef
willChange: "width",
```

**问题**：
- `willChange` 在当前行创建 2 个 GPU 图层
- 切歌时旧行的 KaraokeLyricLine 变为非激活态，DOM 元素被替换为简单 Text
- 但旧 GPU 图层不保证立即释放
- 频繁切歌导致 GPU 图层数量累积

**修复方案**：
- `willChange: "transform"` 保留（scrollRef 确实需要频繁 transform 动画）
- `willChange: "width"` 移除（width 动画不是 transform/opacity，willChange 对 width 无明显加速效果，反而消耗 GPU 内存）

```tsx
// overlayRef — 移除 willChange
style={{
  ...sharedTextStyle,
  position: "absolute",
  top: 0,
  left: 0,
  width: "0%",
  overflow: "hidden",
  color: highlightColor,
  // willChange: "width",  ← 移除
  maskImage: maskGradient,
  WebkitMaskImage: maskGradient,
}}
```

同理 `LyricsCanvas.tsx` 中的 `willChange: "width"` 也应移除。

---

## 🟢 轻微问题（小幅影响）

### 问题 11：`playQueue` 无限增长

**文件**：`src/stores/music-store.ts`，`batchLoadToQueue` 函数

**问题**：
- 每次加载歌单时，`batchLoadToQueue` 后台加载所有曲目到 `playQueue`
- 浏览多个歌单后，`playQueue` 可能包含数千首歌
- 每次数组变化触发 PlayerBar 和 ExpandedPlayer 重渲染
- VirtualList 的 `items` prop 也变大，虽然 slice 是 O(1)，但内存占用增加

**修复方案**：
- 在 `playSong` 设置新队列时清空旧的 batch load
- 或限制 `playQueue` 最大长度，超出时不再追加
- 或在切换歌单时重置 `playQueue` 为当前歌单的初始曲目

---

### 问题 12：`getBorderGlowStyle(glowColor)` 每次渲染创建新对象

**文件**：`src/components/special/liquid-glass-card.tsx`，第 66 行

**现状**：
```tsx
<Box style={getBorderGlowStyle(glowColor)} ... />
```

**问题**：
- `getBorderGlowStyle` 每次返回新的 `CSSProperties` 对象
- 作为 `style` prop，每次渲染引用都不同

**修复方案**：
```tsx
const borderGlowStyle = useMemo(() => getBorderGlowStyle(glowColor), [glowColor]);
// <Box style={borderGlowStyle} ... />
```

---

### 问题 13：Emotion/ChakraUI 动态样式缓存增长

**问题**：
- 大量内联 `sx` 对象包含动态值（如 `${activeColor}22`、动态 gradient 字符串）
- 每个唯一值组合生成新的 CSS 规则并注入 `<style>` 标签
- 随时间推移，DOM 中 `<style>` 标签增多，浏览器样式重计算变慢
- ChakraUI v2 的 `sx` prop 每次渲染都经过 Emotion 序列化 → 哈希 → 缓存查找

**修复方案**：
- 对动态 `sx` 对象使用 `useMemo` 缓存
- 对完全静态的样式提取为模块级常量
- 长期方案：考虑迁移到 ChakraUI v3 或 vanilla-extract

---

### 问题 14：`formatTime` 在 SongRow 中每次渲染重新创建

**文件**：`src/pages/MusicPage.tsx`，第 1624 行

**现状**：
```tsx
const SongRow = memo(function SongRow({ ... }) {
  const formatTime = (time: number): string => { ... }; // ← 每次渲染创建
  // ...
});
```

**问题**：
- `formatTime` 在组件内部定义，每次渲染创建新函数
- 实际上文件第 1150 行已有一个模块级的同名函数

**修复方案**：
- 删除 SongRow 内部的 `formatTime`，使用模块级函数

---

## 修复优先级排序

建议按以下顺序修复（影响从大到小）：

| 优先级 | 问题编号 | 修复内容 | 预计耗时 |
|--------|----------|----------|----------|
| P0 | #4 | `renderSongRow` 用 `useCallback` 包装 | 15 min |
| P0 | #3 | `scrollbarSx` 调用统一 `useMemo` | 10 min |
| P0 | #5 | LiquidGlassCard 移除/条件化 `willChange` | 5 min |
| P0 | #2 | `useCoverColor` 添加 Image 清理 | 10 min |
| P0 | #1 | Tauri `listen()` 存储 unlisten 并提供清理 | 15 min |
| P1 | #10 | KaraokeLyricLine 移除 `willChange: "width"` | 5 min |
| P1 | #8 | SearchBox debounce 卸载清理 | 5 min |
| P1 | #9 | `handleCloseWithAnimation` 定时器清理 | 5 min |
| P1 | #6 | `timeSyncTimer` 页面卸载时停止 | 5 min |
| P1 | #7 | 重复 `timeupdate` 监听去重 | 10 min |
| P2 | #12 | `getBorderGlowStyle` memoize | 5 min |
| P2 | #14 | `formatTime` 提取为模块级 | 2 min |
| P2 | #11 | `playQueue` 增长限制 | 15 min |
| P2 | #13 | Emotion 动态样式优化 | 持续 |

---

## 涉及文件清单

| 文件路径 | 修改类型 |
|----------|----------|
| `src/stores/music-store.ts` | 事件监听器清理、timeSyncTimer 清理 |
| `src/hooks/use-cover-color.ts` | Image/Canvas 清理 |
| `src/pages/MusicPage.tsx` | useCallback、useMemo、定时器清理、scrollbarSx |
| `src/components/special/liquid-glass-card.tsx` | 移除 willChange、memoize style |
| `src/components/KaraokeLyricLine.tsx` | 移除 willChange: width |
| `src/components/desktop-lyrics/LyricsCanvas.tsx` | 移除 willChange: width |

---

## 验证方法

1. **Chrome DevTools Performance Monitor**：
   - 打开 Performance 面板，录制 5 分钟正常使用（切歌、搜索、打开/关闭播放器）
   - 观察 JS Heap、GPU Memory、DOM Nodes 趋势
   - 修复前：持续上升趋势；修复后：应趋于平稳

2. **React DevTools Profiler**：
   - 录制操作序列，检查重渲染频率
   - 重点关注 MusicPage、VirtualList、PlayerBar 的渲染次数
   - 修复前：MusicPage 每次 store 变化都重渲染，VirtualList 跟着重渲染
   - 修复后：VirtualList 应仅在 items 变化时重渲染

3. **Chrome Task Manager**：
   - 观察 GPU Process 的内存使用
   - 修复前：随时间持续增长；修复后：应趋于稳定

4. **手动测试**：
   - 连续切歌 50 次，观察页面流畅度
   - 打开/关闭 ExpandedPlayer 20 次，观察是否有卡顿
   - 搜索 + 浏览歌单 10 分钟，观察整体响应速度
