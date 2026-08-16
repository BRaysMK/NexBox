// SMTC 支持：通过 Media Session API 将 NexBox 音乐播放暴露到
// Windows 任务栏媒体控件（System Media Transport Controls）。
//
// 原理：WebView2 (Chromium) 的 HTMLAudioElement 播放时自动注册系统媒体会话，
// 前端通过 navigator.mediaSession 设置元数据（标题/艺术家/封面）和
// 播放控制（播放/暂停/上一首/下一首/seek）。
//
// 注意：SMTC 会话只在「有声音播放」期间激活，暂停后仍保留控制但需及时
// 调用 setPlaybackState 同步状态。

import { convertFileSrc } from "@tauri-apps/api/core";
import type { Song } from "@/types/music";

// 专辑封面 URL → 临时缓存，避免重复请求
const coverCache = new Map<string, string>();

function resolveCoverUrl(song: Song): string {
  const raw = song.cover || "";
  if (!raw) return "";
  if (raw.startsWith("http") || raw.startsWith("data:")) return raw;
  // 相对路径（本地封面），用 convertFileSrc 转换
  return convertFileSrc(raw);
}

async function loadCoverBlob(url: string): Promise<string | undefined> {
  if (coverCache.has(url)) return coverCache.get(url);
  try {
    const res = await fetch(url, { mode: "cors" });
    if (!res.ok) return undefined;
    const blob = await res.blob();
    const dataUrl = await new Promise<string | undefined>((resolve) => {
      const reader = new FileReader();
      reader.onload = () => resolve(typeof reader.result === "string" ? reader.result : undefined);
      reader.onerror = () => resolve(undefined);
      reader.readAsDataURL(blob);
    });
    coverCache.set(url, dataUrl || "");
    return dataUrl;
  } catch {
    return undefined;
  }
}

export async function updateMediaSession(song: Song | null, isPlaying: boolean): Promise<void> {
  const ms = navigator.mediaSession;
  if (!ms) return; // 非 Chromium / 不支持的环境

  if (!song) {
    try {
      ms.metadata = null;
      ms.playbackState = "none";
    } catch { /* ignore */ }
    return;
  }

  // 元数据
  try {
    const coverUrl = resolveCoverUrl(song);
    let artwork: MediaImage[] = [];
    if (coverUrl) {
      const dataUrl = await loadCoverBlob(coverUrl);
      if (dataUrl) {
        artwork = [{ src: dataUrl, sizes: "512x512", type: "image/jpeg" }];
      }
    }
    ms.metadata = new MediaMetadata({
      title: song.name || "未知歌曲",
      artist: song.artist || "未知歌手",
      album: song.album || "",
      artwork,
    });
  } catch { /* 元数据失败不影响播放 */ }

  ms.playbackState = isPlaying ? "playing" : "paused";
}

export function setMediaPlaybackState(isPlaying: boolean): void {
  const ms = navigator.mediaSession;
  if (!ms) return;
  try {
    ms.playbackState = isPlaying ? "playing" : "paused";
  } catch { /* ignore */ }
}

// 注册媒体控制按键回调（播放/暂停/上一首/下一首/seek）
// 返回一个取消注册函数
export function registerMediaActions(handlers: {
  onPlay: () => void;
  onPause: () => void;
  onNext: () => void;
  onPrev: () => void;
  onSeek: (time: number) => void;
}): () => void {
  const ms = navigator.mediaSession;
  if (!ms) return () => {};

  ms.setActionHandler("play", () => handlers.onPlay());
  ms.setActionHandler("pause", () => handlers.onPause());
  ms.setActionHandler("previoustrack", () => handlers.onPrev());
  ms.setActionHandler("nexttrack", () => handlers.onNext());
  ms.setActionHandler("seekto", (details) => {
    if (details.seekTime != null) handlers.onSeek(details.seekTime);
  });

  return () => {
    try {
      ms.setActionHandler("play", null);
      ms.setActionHandler("pause", null);
      ms.setActionHandler("previoustrack", null);
      ms.setActionHandler("nexttrack", null);
      ms.setActionHandler("seekto", null);
    } catch { /* ignore */ }
  };
}
