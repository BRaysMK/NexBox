/**
 * 系统媒体控制（Windows SMTC）—— 走 Chromium MediaSession
 *
 * 设计取舍：
 * 之前是 Rust 侧用 WinRT 自建 `SystemMediaTransportControls` 会话，为了在 WebView2
 * 聚焦时也能拿到物理媒体键，又叠了一套 `RegisterHotKey` 全局热键 + 主窗口
 * `WM_APPCOMMAND` 子类化。结果是**只要程序在跑就全局抢媒体键**，与本应用是否在放歌无关，
 * 还会和别的播放器抢注册（实测 4 个媒体 VK 全部 `ERROR_HOTKEY_ALREADY_REGISTERED`）。
 *
 * 现在改由 Chromium 内置的 MediaSession 桥接系统 SMTC：
 *   - 只有**真的有 `<audio>` 在播**时系统浮层才出现本应用，暂停/停止即消失；
 *   - 媒体键由 Windows 按「当前在播会话」原生路由，本应用不注册任何全局热键，
 *     因此不放歌时绝不会吃掉别的软件的媒体键；
 *   - 进度条由 Chromium 依据 `<audio>` 元素自行推导，前端不再每秒 IPC 推位置。
 *
 * 本模块是这套集成的唯一出口：元数据、播放状态、系统下发的控制动作都在这里。
 */

import type { Song } from "@/types/music";

/** 系统可下发的媒体控制动作（MediaSessionAction 的子集，只列我们真正处理的） */
type SystemAction = "play" | "pause" | "previoustrack" | "nexttrack" | "stop" | "seekto";

/** 供 store 注入的动作回调集合 */
export interface MediaSessionActions {
  play: () => void;
  pause: () => void;
  prev: () => void;
  next: () => void;
  stop: () => void;
  /** 跳转到指定位置（秒） */
  seek: (seconds: number) => void;
}

/** 当前是否允许新境盒接管系统媒体控制（对应设置「键盘媒体键控制音乐」） */
let enabled = true;
/** 动作回调（store 就绪后注册一次） */
let actions: MediaSessionActions | null = null;
/** 上一次设置过的元数据 key，避免每帧重建 MediaMetadata */
let lastMetadataKey: string | null = null;
/** 已注册过 action handler 的标记（重复注册无意义且浪费） */
let handlersInstalled = false;

/** 浏览器是否支持 MediaSession（非 Tauri/旧 WebView2 环境下静默降级） */
function supported(): boolean {
  return typeof navigator !== "undefined" && "mediaSession" in navigator;
}

/** 取 mediaSession 对象（不支持时返回 null） */
function session(): MediaSession | null {
  return supported() ? navigator.mediaSession : null;
}

/**
 * 注册系统下发的控制动作。幂等，只需在 store 初始化时调用一次。
 *
 * 注意 `play` / `pause` 必须分开实现而不是都塞 `togglePlay`：
 * 系统可能在状态切换过程中重复下发同一动作，用 toggle 会翻转两次（表现为按一次没反应）。
 */
export function installMediaSessionActions(next: MediaSessionActions): void {
  actions = next;
  const ms = session();
  if (!ms || handlersInstalled) return;
  handlersInstalled = true;

  const handlers: [SystemAction, MediaSessionActionHandler][] = [
    ["play", () => guard(() => actions?.play())],
    ["pause", () => guard(() => actions?.pause())],
    ["previoustrack", () => guard(() => actions?.prev())],
    ["nexttrack", () => guard(() => actions?.next())],
    ["stop", () => guard(() => actions?.stop())],
    [
      "seekto",
      (details) => {
        // seekOffset 是相对量，seekTime 是绝对秒数；只处理绝对跳转
        const time = (details as MediaSessionActionDetails).seekTime;
        if (typeof time === "number" && Number.isFinite(time)) {
          guard(() => actions?.seek(time));
        }
      },
    ],
  ];

  for (const [action, handler] of handlers) {
    try {
      ms.setActionHandler(action, handler);
    } catch {
      // 个别 action 在旧版 Chromium 上不支持：忽略即可，其余动作仍可用
    }
  }
}

/** 统一前置判断：开关关闭时系统下发的任何动作都不响应 */
function guard(fn: () => void): void {
  if (!enabled) return;
  try {
    fn();
  } catch (e) {
    console.warn("[MediaSession] 处理系统媒体控制失败:", e);
  }
}

/** 切换「是否允许接管系统媒体控制」；关闭时立即把会话从浮层摘掉 */
export function setMediaSessionEnabled(value: boolean): void {
  enabled = value;
  if (!value) {
    clearMediaSession();
  }
}

/**
 * 同步一次系统媒体会话状态。
 *
 * - `song` 为空 → 清空会话（浮层条目消失）
 * - 元数据仅在歌曲/封面变化时重建（避免每次进度更新都分配对象）
 * - `playbackState` 每次都要同步（系统据此决定媒体键路由目标）
 *
 * @param song     当前歌曲（null 表示没有在播内容）
 * @param playing  是否正在播放
 * @param artwork  封面绝对 URL（本地代理地址），空字符串表示无封面
 */
export function syncMediaSession(
  song: Song | null,
  playing: boolean,
  artwork: string
): void {
  const ms = session();
  if (!ms) return;

  if (!enabled || !song) {
    clearMediaSession();
    return;
  }

  const title = song.name || "";
  const artist =
    song.artist || song.artists?.map((a) => a.name).filter(Boolean).join(" / ") || "";
  const album = song.album || "";

  const metadataKey = `${title}\u0000${artist}\u0000${album}\u0000${artwork}`;
  if (metadataKey !== lastMetadataKey) {
    lastMetadataKey = metadataKey;
    try {
      ms.metadata = new MediaMetadata({
        title,
        artist,
        album,
        artwork: artwork ? [{ src: artwork, sizes: "512x512" }] : [],
      });
    } catch (e) {
      console.warn("[MediaSession] 设置元数据失败:", e);
    }
  }

  try {
    ms.playbackState = playing ? "playing" : "paused";
  } catch {
    // 老版本只读，忽略
  }
}

/**
 * 清空系统媒体会话：无歌 / 关闭播放 / 开关关闭时调用。
 *
 * `playbackState = "none"` + `metadata = null` 会让 Windows 立即移除音量浮层与锁屏里的
 * 本应用条目，并把媒体键交还给其他软件（这是「不全局拦截」的关键一步）。
 */
export function clearMediaSession(): void {
  const ms = session();
  if (!ms) return;
  lastMetadataKey = null;
  try {
    ms.metadata = null;
  } catch {
    // ignore
  }
  try {
    ms.playbackState = "none";
  } catch {
    // ignore
  }
}
