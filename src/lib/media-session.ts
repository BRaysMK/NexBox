// SMTC 支持：通过 Web Media Session API 将 NexBox 内置播放器暴露到 Windows 任务栏媒体控件。
//
// 原理：WebView2(Chromium) 的 HTMLAudioElement 播放时自动注册系统媒体会话，
// 前端通过 navigator.mediaSession 设置元数据（标题/歌手/专辑/封面）、播放状态与进度，
// 并响应任务栏媒体控件上的播放/暂停/上一曲/下一曲/进度拖动等按钮。
//
// 注意：本模块仅在主窗口 WebView2 内生效（播放器 Audio 元素驻留于主窗口的 music store），
// 与桌面歌词窗口通过事件通信，不在此处处理。

import { invoke } from "@tauri-apps/api/core";
import { coverProxyUrl, useMusicStore } from "@/stores/music-store";
import type { Song } from "@/types/music";

let setupDone = false;
let lastMetadataKey = "";
let lastPlaybackState = "";
let lastPositionKey = "";

function session(): MediaSession | null {
  return typeof navigator !== "undefined" && "mediaSession" in navigator
    ? navigator.mediaSession
    : null;
}

function buildArtwork(song: Song, proxyPort: number): MediaImage[] {
  const cover = song.cover;
  if (!cover) return [];
  // data URL 可直接交给系统媒体控件展示
  if (cover.startsWith("data:")) return [{ src: cover }];
  // convertFileSrc 生成的本地文件直链仅在 WebView 内可访问，系统控件拉取不到，跳过封面
  if (cover.startsWith("http://asset.localhost/") || cover.startsWith("https://asset.localhost/"))
    return [];
  // 远程封面走本地代理端口（绕过防盗链），系统可直接访问 127.0.0.1
  const resolved = coverProxyUrl(cover, proxyPort);
  return resolved ? [{ src: resolved }] : [];
}

function updateMetadata(song: Song | null, isPlaying: boolean, proxyPort: number) {
  const ms = session();
  if (!ms) return;

  const key = song ? `${song.provider}:${song.id}` : "";
  if (key !== lastMetadataKey) {
    lastMetadataKey = key;
    // 告知 Rust 侧音乐会话活跃状态（用于物理媒体键钩子的消费开关）
    invoke("set_music_session_active", { active: key !== "" }).catch(() => {});
    try {
      if (!song) {
        ms.metadata = null;
      } else {
        ms.metadata = new MediaMetadata({
          title: song.name,
          artist: song.artist || song.artists?.[0]?.name || "",
          album: song.album || "",
          artwork: buildArtwork(song, proxyPort),
        });
      }
    } catch {
      // 会话未激活等场景静默忽略
    }
  }

  const state: MediaSessionPlaybackState = song ? (isPlaying ? "playing" : "paused") : "none";
  if (state !== lastPlaybackState) {
    lastPlaybackState = state;
    try {
      ms.playbackState = state;
    } catch {
      // ignore
    }
  }
}

function syncPosition() {
  const ms = session();
  if (!ms) return;
  const s = useMusicStore.getState();
  if (!s.currentSong || !s.audioRef) return;
  const audioRef = s.audioRef;
  const duration = audioRef.duration && isFinite(audioRef.duration) ? audioRef.duration : 0;
  if (!duration) return;
  const position = Math.min(audioRef.currentTime || 0, duration);
  const key = `${Math.round(position)}:${Math.round(duration)}`;
  if (key === lastPositionKey) return;
  lastPositionKey = key;
  try {
    ms.setPositionState({ duration, playbackRate: audioRef.playbackRate || 1, position });
  } catch {
    // 会话未激活时 setPositionState 会抛错，静默忽略
  }
}

function registerHandlers() {
  const ms = session();
  if (!ms) return;
  const bind = (action: MediaSessionAction, handler: MediaSessionActionHandler) => {
    try {
      ms.setActionHandler(action, handler);
    } catch {
      // 当前环境不支持的按钮动作，忽略
    }
  };

  bind("play", () => {
    const s = useMusicStore.getState();
    if (!s.isPlaying) s.togglePlay();
  });
  bind("pause", () => {
    const s = useMusicStore.getState();
    if (s.isPlaying) s.togglePlay();
  });
  bind("previoustrack", () => useMusicStore.getState().prevTrack());
  bind("nexttrack", () => useMusicStore.getState().nextTrack());
  bind("seekto", (details) => {
    if (typeof details.seekTime === "number") {
      useMusicStore.getState().seekTo(details.seekTime);
    }
  });
  bind("seekbackward", () => {
    const s = useMusicStore.getState();
    s.seekTo(Math.max(0, (s.audioRef?.currentTime ?? 0) - 10));
  });
  bind("seekforward", () => {
    const s = useMusicStore.getState();
    s.seekTo((s.audioRef?.currentTime ?? 0) + 10);
  });
}

/**
 * 幂等初始化系统媒体会话同步：注册按钮事件 + 订阅 store 状态变化 + 定时同步播放进度。
 * 播放器 Audio 元素由主窗口 MusicPage 创建后调用本函数（app 生命周期内只需一次）。
 */
export function setupMediaSessionSync() {
  if (setupDone) return;
  setupDone = true;

  registerHandlers();

  const s = useMusicStore.getState();
  updateMetadata(s.currentSong, s.isPlaying, s.proxyPort);

  useMusicStore.subscribe((state) => {
    updateMetadata(state.currentSong, state.isPlaying, state.proxyPort);
    syncPosition();
  });

  setInterval(syncPosition, 1000);
}
