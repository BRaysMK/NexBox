/**
 * RustAudio —— HTMLAudioElement 的前端适配器
 *
 * 背景：主窗口销毁(托盘)后 WebView2 随之销毁，HTMLAudioElement 播放的音乐会中断。
 * 方案：播放引擎移到 Rust（src-tauri/src/player.rs, rodio），前端用本类模拟
 * HTMLAudioElement 的接口（src/volume/currentTime/play/pause/事件），
 * 使 music-store.ts / MusicPage.tsx 几乎不用改动即可切换到后端播放。
 *
 * 事件映射：
 *   Rust player-tick     → 更新 currentTime/duration，触发 timeupdate / loadedmetadata
 *   Rust player-ended    → 触发 ended
 *   Rust player-error    → 触发 error
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** 播放器最小接口（兼容 HTMLAudioElement 子集，供 store / 歌词组件使用） */
export interface PlaybackAudio {
  src: string;
  volume: number;
  currentTime: number;
  duration: number;
  paused: boolean;
  play(): Promise<void>;
  pause(): void;
  addEventListener(type: string, listener: EventListenerOrEventListenerObject): void;
  removeEventListener(type: string, listener: EventListenerOrEventListenerObject): void;
  /**
   * 用后端引擎的真实状态同步本地镜像（不触发 seek/播放命令）。
   * 窗口重建恢复时调用，避免暂停状态下没有 player-tick 导致进度/状态显示错误。
   */
  syncState?(position: number, duration: number, isPlaying: boolean): void;
}

type ListenerMap = Map<string, Set<EventListenerOrEventListenerObject>>;

let globalListenersRegistered = false;
let unlistenFns: UnlistenFn[] = [];
const instances = new Set<RustAudio>();

async function ensureGlobalListeners() {
  if (globalListenersRegistered) return;
  globalListenersRegistered = true;
  try {
    unlistenFns.push(
      await listen<{ position: number; duration: number; isPlaying: boolean; loading: boolean }>(
        "player-tick",
        (e) => {
          const { position, duration, isPlaying, loading } = e.payload ?? {};
          for (const inst of instances) {
            inst._onTick(position ?? 0, duration ?? 0, isPlaying ?? false, loading ?? false);
          }
        }
      )
    );
    unlistenFns.push(
      await listen("player-ended", () => {
        for (const inst of instances) inst._onEnded();
      })
    );
    unlistenFns.push(
      await listen<{ message: string }>("player-error", (e) => {
        const msg = e.payload?.message ?? "播放失败";
        for (const inst of instances) inst._onError(msg);
      })
    );
  } catch (err) {
    console.error("[RustAudio] register listeners failed:", err);
    globalListenersRegistered = false;
  }
}

export class RustAudio implements PlaybackAudio {
  private _src = "";
  private _volume = 0.7;
  private _currentTime = 0;
  private _duration = 0;
  private _paused = true;
  private listeners: ListenerMap = new Map();
  private _lastDuration = 0;
  private _loaded = false;

  constructor() {
    instances.add(this);
    void ensureGlobalListeners();
  }

  // ---------- 属性 ----------

  get src(): string {
    return this._src;
  }

  /**
   * 设置播放源。支持：
   *  - 本地绝对路径（C:\\...）→ player_play kind=local
   *  - 网络 URL → player_play kind=url
   *  - 本机音频代理 URL（http://127.0.0.1:{port}/audio?url=...）→ 提取原始 URL 后播放
   *  - 空字符串 → 停止播放
   */
  set src(value: string) {
    const v = value ?? "";
    if (v === this._src && v !== "") return;
    const isNewSource = v !== "" && v !== this._src;
    this._src = v;
    if (!v) {
      void invoke("player_stop").catch(() => {});
      this._paused = true;
      this._currentTime = 0;
      this._duration = 0;
      this._loaded = false;
      return;
    }
    const { kind, src } = resolveSource(v);
    // 新播放源一律从头开始（seek 0）：避免切歌时沿用上一首歌的位置，
    // 否则新歌会从旧歌的进度处开始播放（错误进度/跳播）。
    // 需要续播的场景由调用方在设置 src 后显式赋值 currentTime（走 player_seek）。
    void invoke("player_play", { kind, src, seek: isNewSource ? 0 : this._currentTime }).catch((e) =>
      console.error("[RustAudio] player_play failed:", e)
    );
    this._paused = false;
    this._loaded = false;
    if (isNewSource) {
      this._currentTime = 0;
      this._duration = 0;
    }
  }

  get volume(): number {
    return this._volume;
  }

  set volume(v: number) {
    this._volume = v;
    void invoke("player_set_volume", { volume: v }).catch(() => {});
  }

  get currentTime(): number {
    return this._currentTime;
  }

  set currentTime(t: number) {
    this._currentTime = t;
    void invoke("player_seek", { seconds: t }).catch(() => {});
  }

  get duration(): number {
    return this._duration;
  }

  get paused(): boolean {
    return this._paused;
  }

  // ---------- 控制 ----------

  async play(): Promise<void> {
    this._paused = false;
    try {
      await invoke("player_resume");
    } catch (e) {
      console.error("[RustAudio] resume failed:", e);
    }
  }

  pause(): void {
    this._paused = true;
    void invoke("player_pause").catch(() => {});
  }

  // ---------- 后端状态同步 ----------

  /**
   * 用后端引擎真实状态同步本地镜像（不触发任何引擎命令）。
   * 用于主窗口重建后的恢复：暂停/无 tick 时也能显示正确进度与状态。
   */
  syncState(position: number, duration: number, isPlaying: boolean): void {
    this._currentTime = position;
    const durChanged = duration > 0 && Math.abs(duration - this._duration) > 0.1;
    if (durChanged) this._duration = duration;
    this._paused = !isPlaying;
    if (durChanged && duration > 0 && !this._loaded) {
      this._loaded = true;
      this._lastDuration = duration;
      this._dispatch("loadedmetadata");
    }
    this._dispatch("timeupdate");
  }

  // ---------- 事件 ----------

  addEventListener(type: string, listener: EventListenerOrEventListenerObject): void {
    if (!this.listeners.has(type)) this.listeners.set(type, new Set());
    this.listeners.get(type)!.add(listener);
  }

  removeEventListener(type: string, listener: EventListenerOrEventListenerObject): void {
    this.listeners.get(type)?.delete(listener);
  }

  destroy(): void {
    instances.delete(this);
  }

  // ---------- 内部：处理 Rust 事件 ----------

  _onTick(position: number, duration: number, isPlaying: boolean, loading: boolean): void {
    const timeChanged = Math.abs(position - this._currentTime) > 0.02;
    this._currentTime = position;
    const durChanged = duration > 0 && Math.abs(duration - this._duration) > 0.1;
    if (durChanged) this._duration = duration;
    this._paused = !isPlaying;
    if (durChanged && duration > 0 && !this._loaded) {
      this._loaded = true;
      this._lastDuration = duration;
      this._dispatch("loadedmetadata");
    }
    if (timeChanged || durChanged) {
      this._dispatch("timeupdate");
    }
  }

  _onEnded(): void {
    this._paused = true;
    this._dispatch("ended");
  }

  _onError(message: string): void {
    console.error("[RustAudio] error:", message);
    this._dispatch("error", message);
  }

  private _dispatch(type: string, detail?: string): void {
    const set = this.listeners.get(type);
    if (!set || set.size === 0) return;
    const evt = new Event(type);
    for (const l of [...set]) {
      try {
        if (typeof l === "function") (l as EventListener).call(this, evt);
        else (l as EventListenerObject).handleEvent(evt);
      } catch (e) {
        console.error("[RustAudio] listener error:", e);
      }
    }
  }
}

/** 解析播放源：本地路径 / 网络 URL / 本机代理 URL */
function resolveSource(src: string): { kind: "local" | "url"; src: string } {
  // 本机音频代理 URL：http://127.0.0.1:{port}/audio?url=xxx
  if (/^https?:\/\/127\.0\.0\.1(?::\d+)?\/audio\?/.test(src)) {
    try {
      const u = new URL(src);
      const raw = u.searchParams.get("url");
      if (raw) return { kind: "url", src: raw };
    } catch {
      // fall through
    }
  }
  // 网络 URL
  if (/^https?:\/\//.test(src)) {
    return { kind: "url", src };
  }
  // 其余视为本地路径
  return { kind: "local", src };
}

/** 供非组件代码直接使用（如 store 内部） */
export function createRustAudio(): RustAudio {
  return new RustAudio();
}
