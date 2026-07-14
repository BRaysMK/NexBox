import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen, emit } from "@tauri-apps/api/event";
import { Store } from "@tauri-apps/plugin-store";
import type {
  Song,
  Artist,
  Playlist,
  LoginInfo,
  Lyrics,
  PlayMode,
  PlaybackQuality,
  SongUrlResult,
  KaraokeLine,
} from "@/types/music";
import { buildKaraokeLines } from "@/lib/karaoke-lyrics";

interface MusicState {
  // 播放状态
  currentSong: Song | null;
  isPlaying: boolean;
  currentTime: number;
  duration: number;
  volume: number;
  prevVolume: number;
  playMode: PlayMode;
  playQueue: Song[];
  currentIndex: number;

  // 登录状态
  loginInfo: LoginInfo | null;

  // 数据
  searchResults: Song[];
  userPlaylists: Playlist[];
  // 左侧「我的歌单」面板的曲目
  leftPlaylistTracks: Song[];
  leftPlaylistMeta: Playlist | null;
  // 右侧「推荐歌单」面板的曲目
  rightPlaylistTracks: Song[];
  rightPlaylistMeta: Playlist | null;
  // 歌单分页
  leftPlaylistTotalTrackIds: string[];
  rightPlaylistTotalTrackIds: string[];
  leftPlaylistLoadingMore: boolean;
  rightPlaylistLoadingMore: boolean;
  likedSongIds: Set<string>;
  currentLyrics: Lyrics | null;
  recommendations: Playlist[];
  recommendSongs: Song[];

  // 歌手搜索
  artistSearchResults: Artist[];
  artistSongs: Song[];
  selectedArtist: Artist | null;
  searchingArtists: boolean;
  loadingArtistSongs: boolean;

  // 歌单搜索
  playlistSearchResults: Playlist[];
  searchingPlaylists: boolean;

  // 官方榜单
  officialCharts: Playlist[];

  // 音质
  playbackQuality: PlaybackQuality;
  currentQuality: string;
  currentBitrate: number;

  // 代理端口
  proxyPort: number;

  // 歌词字体大小
  lyricsFontSize: number;
  // 歌词高亮颜色
  lyricsHighlightColor: string;

  // 桌面歌词设置
  desktopLyricsVisible: boolean;
  desktopLyricsFontSize: number;
  desktopLyricsHighlightColor: string;
  desktopLyricsBaseColor: string;
  desktopLyricsLineCount: 1 | 2;
  desktopLyricsLocked: boolean;

  // UI 状态
  searching: boolean;
  loadingPlaylists: boolean;
  loadingLeftTracks: boolean;
  loadingRightTracks: boolean;
  loadingLyrics: boolean;
  expandedStyle: "glass" | "modern";
  dynamicEnabled: boolean;
  coverFilmEffect: boolean;

  // 音频元素引用
  audioRef: HTMLAudioElement | null;

  // Actions
  init: () => Promise<void>;
  setAudioRef: (audio: HTMLAudioElement | null) => void;

  search: (keywords: string) => Promise<void>;
  searchArtists: (keywords: string) => Promise<void>;
  loadArtistSongs: (artistId: string, offset?: number) => Promise<void>;
  clearArtistState: () => void;
  searchPlaylists: (keywords: string) => Promise<void>;
  playSong: (song: Song, queue?: Song[]) => Promise<void>;
  togglePlay: () => void;
  nextTrack: () => void;
  prevTrack: () => void;
  seekTo: (time: number) => void;
  setVolume: (v: number) => void;
  togglePlayMode: () => void;
  setPlaybackQuality: (quality: PlaybackQuality) => Promise<void>;
  setLyricsFontSize: (size: number) => Promise<void>;
  setLyricsHighlightColor: (color: string) => Promise<void>;
  setExpandedStyle: (style: "glass" | "modern") => Promise<void>;
  setDynamicEnabled: (enabled: boolean) => Promise<void>;
  setCoverFilmEffect: (enabled: boolean) => Promise<void>;
  setCurrentTime: (t: number) => void;
  setDuration: (d: number) => void;

  // 桌面歌词 Actions
  toggleDesktopLyrics: () => Promise<void>;
  setDesktopLyricsVisible: (visible: boolean) => Promise<void>;
  setDesktopLyricsFontSize: (size: number) => Promise<void>;
  setDesktopLyricsHighlightColor: (color: string) => Promise<void>;
  setDesktopLyricsBaseColor: (color: string) => Promise<void>;
  setDesktopLyricsLineCount: (count: 1 | 2) => Promise<void>;
  setDesktopLyricsLocked: (locked: boolean) => void;
  emitDesktopLyricsSettings: () => void;
  emitDesktopLyricsData: () => void;

  loginStatus: () => Promise<void>;
  loginWithCookie: (cookie: string) => Promise<boolean>;
  logout: () => Promise<void>;
  openLoginWindow: () => Promise<void>;

  loadUserPlaylists: () => Promise<void>;
  loadLeftPlaylistTracks: (id: string) => Promise<void>;
  loadMoreLeftPlaylistTracks: () => Promise<void>;
  loadRightPlaylistTracks: (id: string) => Promise<void>;
  loadMoreRightPlaylistTracks: () => Promise<void>;
  loadLikedList: () => Promise<void>;
  toggleLike: (songId: string) => Promise<void>;
  loadLyrics: (songId: string) => Promise<void>;
  loadRecommendations: () => Promise<void>;
  loadOfficialCharts: () => Promise<void>;
  togglePlaylistSubscribe: (playlistId: string, currentSubscribed: boolean) => Promise<void>;
}

let storeInstance: Store | null = null;
const getStore = async (): Promise<Store> => {
  if (!storeInstance) {
    storeInstance = await Store.load("music-player-settings.json");
  }
  return storeInstance;
};

// 桌面歌词时间同步定时器
let timeSyncTimer: ReturnType<typeof setInterval> | null = null;
// 防止 React Strict Mode 双重调用 init 导致重复注册 listener
let listenersRegistered = false;
// 存储 Tauri 事件监听器的取消函数，防止内存泄漏
const unlistenFns: (() => void)[] = [];

function startTimeSync() {
  if (timeSyncTimer) return;
  timeSyncTimer = setInterval(() => {
    const state = useMusicStore.getState();
    if (state.audioRef && state.desktopLyricsVisible) {
      emit("desktop-lyrics:time", {
        currentTime: state.audioRef.currentTime,
        isPlaying: state.isPlaying,
      });
    }
  }, 100);
}
export function stopTimeSync() {
  if (timeSyncTimer) {
    clearInterval(timeSyncTimer);
    timeSyncTimer = null;
  }
}

/** 清理所有 Tauri 事件监听器，防止内存泄漏 */
export function cleanupMusicListeners() {
  unlistenFns.forEach((fn) => fn());
  unlistenFns.length = 0;
  listenersRegistered = false;
}

async function getProxyAudioUrl(rawUrl: string, proxyPort: number): Promise<string> {
  if (!proxyPort) {
    proxyPort = await invoke<number>("cmd_get_proxy_port");
  }
  return `http://127.0.0.1:${proxyPort}/audio?url=${encodeURIComponent(rawUrl)}`;
}

export function coverProxyUrl(url: string, proxyPort: number): string {
  if (!url) return "";
  if (url.startsWith("data:") || url.startsWith("blob:")) return url;
  if (!proxyPort) return url;
  return `http://127.0.0.1:${proxyPort}/cover?url=${encodeURIComponent(url)}`;
}

/// 后台批量加载歌单剩余曲目到播放队列（不加入歌单列表）
/// 优化：先在本地累积所有批次，最后做一次去重 setState，避免重复歌曲和频繁 re-render
/// 限制：播放队列最大 2000 首，超出部分不再追加，防止内存无限增长
const MAX_PLAY_QUEUE = 2000;
let batchLoadGuard: string | null = null;
async function batchLoadToQueue(playlistId: string, initialSongs: Song[], totalCount: number) {
  if (initialSongs.length >= totalCount) return;
  // 防止并发执行同一歌单的后台加载
  if (batchLoadGuard === playlistId) return;
  batchLoadGuard = playlistId;

  // 本地累积，仅在结束时做一次 setState
  const collected: Song[] = [];
  const seenIds = new Set(initialSongs.map((s) => s.id));
  let offset = initialSongs.length;

  try {
    while (offset < totalCount) {
      const batch = await invoke<Song[]>("music_playlist_tracks_range", { id: playlistId, start: offset, count: 200 });
      if (batch.length === 0) break;
      // 检查用户是否已切换到其他歌单
      const state = useMusicStore.getState();
      if (state.leftPlaylistMeta?.id !== playlistId && state.rightPlaylistMeta?.id !== playlistId) break;
      // 去重：跳过已收集的歌曲
      for (const song of batch) {
        if (!seenIds.has(song.id)) {
          seenIds.add(song.id);
          collected.push(song);
        }
      }
      offset += 200;
      // 队列已达上限，停止加载
      if (initialSongs.length + collected.length >= MAX_PLAY_QUEUE) break;
    }
  } catch {
    // 网络错误中断，已收集的部分仍然写入
  } finally {
    batchLoadGuard = null;
  }

  if (collected.length === 0) return;

  // 单次 setState，并对当前 playQueue 去重
  const state = useMusicStore.getState();
  const isSameList = state.playQueue.length > 0
    && state.currentSong
    && state.playQueue.some((s) => s.id === state.currentSong!.id);
  if (isSameList) {
    const queueIds = new Set(state.playQueue.map((s) => s.id));
    const unique = collected.filter((s) => !queueIds.has(s.id));
    // 截断到最大队列长度
    const remaining = MAX_PLAY_QUEUE - state.playQueue.length;
    const toAdd = unique.slice(0, Math.max(0, remaining));
    if (toAdd.length > 0) {
      useMusicStore.setState({ playQueue: [...state.playQueue, ...toAdd] });
    }
  }
}

export const useMusicStore = create<MusicState>((set, get) => ({
  currentSong: null,
  isPlaying: false,
  currentTime: 0,
  duration: 0,
  volume: 0.7,
  prevVolume: 0.7,
  playMode: "list",
  playQueue: [],
  currentIndex: -1,

  loginInfo: null,

  searchResults: [],
  userPlaylists: [],
  leftPlaylistTracks: [],
  leftPlaylistMeta: null,
  rightPlaylistTracks: [],
  rightPlaylistMeta: null,
  leftPlaylistTotalTrackIds: [],
  rightPlaylistTotalTrackIds: [],
  leftPlaylistLoadingMore: false,
  rightPlaylistLoadingMore: false,
  likedSongIds: new Set(),
  currentLyrics: null,
  recommendations: [],
  recommendSongs: [],

  artistSearchResults: [],
  artistSongs: [],
  selectedArtist: null,
  searchingArtists: false,
  loadingArtistSongs: false,

  playlistSearchResults: [],
  searchingPlaylists: false,

  officialCharts: [],

  playbackQuality: "hires",
  currentQuality: "",
  currentBitrate: 0,
  lyricsFontSize: 18,
  lyricsHighlightColor: "#fff0b8",
  expandedStyle: "modern",
  dynamicEnabled: false,
  coverFilmEffect: false,
  proxyPort: 0,

  desktopLyricsVisible: false,
  desktopLyricsFontSize: 36,
  desktopLyricsHighlightColor: "#FFD700",
  desktopLyricsBaseColor: "rgba(255,255,255,0.35)",
  desktopLyricsLineCount: 2,
  desktopLyricsLocked: false,

  searching: false,
  loadingPlaylists: false,
  loadingLeftTracks: false,
  loadingRightTracks: false,
  loadingLyrics: false,

  audioRef: null,

  init: async () => {
    try {
      const port = await invoke<number>("cmd_get_proxy_port");
      set({ proxyPort: port });
    } catch {
      console.warn("Failed to get proxy port");
    }

    // 加载设置
    try {
      const store = await getStore();
      const vol = await store.get<number>("volume");
      const mode = await store.get<PlayMode>("playMode");
      const quality = await store.get<PlaybackQuality>("quality");
      const fontSize = await store.get<number>("lyricsFontSize");
      const highlightColor = await store.get<string>("lyricsHighlightColor");
      const dlFontSize = await store.get<number>("desktopLyricsFontSize");
      const dlHighlightColor = await store.get<string>("desktopLyricsHighlightColor");
      const dlBaseColor = await store.get<string>("desktopLyricsBaseColor");
      const dlLineCount = await store.get<1 | 2>("desktopLyricsLineCount");
      const dlLocked = await store.get<boolean>("desktopLyricsLocked");
      if (vol != null) set({ volume: vol });
      if (mode) set({ playMode: mode });
      if (quality) set({ playbackQuality: quality });
      if (fontSize != null) set({ lyricsFontSize: fontSize });
      const expStyle = await store.get<string>("expandedStyle");
      if (highlightColor) set({ lyricsHighlightColor: highlightColor });
      if (expStyle === "modern") set({ expandedStyle: "modern" });
      const dynamic = await store.get<boolean>("dynamicEnabled");
      if (dynamic) set({ dynamicEnabled: true });
      const filmEffect = await store.get<boolean>("coverFilmEffect");
      if (filmEffect) set({ coverFilmEffect: true });
      if (dlFontSize != null) set({ desktopLyricsFontSize: dlFontSize });
      if (dlHighlightColor) set({ desktopLyricsHighlightColor: dlHighlightColor });
      if (dlBaseColor) set({ desktopLyricsBaseColor: dlBaseColor });
      if (dlLineCount) set({ desktopLyricsLineCount: dlLineCount });
      if (dlLocked != null) set({ desktopLyricsLocked: dlLocked });
    } catch {
      // ignore
    }

    // 加载缓存的官方榜单（优先显示缓存，后台刷新）
    try {
      const s = await getStore();
      const cached = await s.get<Playlist[]>("officialCharts");
      if (cached && cached.length > 0) {
        set({ officialCharts: cached });
      }
    } catch {}
    get().loadOfficialCharts();

    // 防止 React Strict Mode 双重调用导致重复注册
    if (listenersRegistered) return;
    listenersRegistered = true;

    // 桌面歌词控制事件监听
    unlistenFns.push(
      await listen<{ action: string }>("desktop-lyrics:control", (event) => {
        const { action } = event.payload;
        switch (action) {
          case "play-pause":
            get().togglePlay();
            break;
          case "prev":
            get().prevTrack();
            break;
          case "next":
            get().nextTrack();
            break;
          case "toggle-shuffle":
            get().togglePlayMode();
            break;
          case "lock":
            get().setDesktopLyricsLocked(true);
            break;
          case "unlock":
            get().setDesktopLyricsLocked(false);
            break;
        }
      })
    );

    // 桌面歌词窗口就绪后请求数据
    // 解决窗口首次打开时 emit 早于 listen 注册的时序问题
    unlistenFns.push(
      await listen("desktop-lyrics:request-data", () => {
        // 小延时确保请求方 listener 完全就绪
        setTimeout(() => {
          get().emitDesktopLyricsData();
          get().emitDesktopLyricsSettings();
          emit("desktop-lyrics:state", {
            isPlaying: get().isPlaying,
            playMode: get().playMode,
            volume: get().volume,
          });
        }, 50);
      })
    );

    // 监听登录成功事件 (来自网页登录窗口的 cookie 捕获)
    // 后端会附带 LoginInfo 数据
    unlistenFns.push(
      await listen<LoginInfo>("netease-login-success", async (event) => {
        console.log("[Music] Login success event received", event.payload);
        const info = event.payload;
        if (info && info.logged_in) {
          // 直接使用后端返回的登录信息
          set({ loginInfo: info });
          get().loadUserPlaylists();
          get().loadLikedList();
        } else {
          // 后端没带数据，手动刷新
          await get().loginStatus();
          if (get().loginInfo?.logged_in) {
            get().loadUserPlaylists();
            get().loadLikedList();
          }
        }
      })
    );

    // 监听登录失败事件
    unlistenFns.push(
      await listen<string>("netease-login-failed", (event) => {
        console.error("[Music] Login failed:", event.payload);
      })
    );

    // 检查登录状态
    await get().loginStatus();
    // 如果已登录, 加载歌单和喜欢列表
    if (get().loginInfo?.logged_in) {
      get().loadUserPlaylists();
      await get().loadLikedList();
    }
  },

  setAudioRef: (audio) => set({ audioRef: audio }),

  search: async (keywords) => {
    if (!keywords.trim()) return;
    set({ searching: true });
    try {
      const results = await invoke<Song[]>("music_search", { keywords, limit: 30 });
      set({ searchResults: results });
    } catch (e) {
      console.error("Search failed:", e);
    } finally {
      set({ searching: false });
    }
  },

  searchArtists: async (keywords) => {
    if (!keywords.trim()) return;
    set({ searchingArtists: true, artistSearchResults: [], selectedArtist: null, artistSongs: [] });
    try {
      const results = await invoke<Artist[]>("music_artist_search", { keywords, limit: 30 });
      set({ artistSearchResults: results });
    } catch (e) {
      console.error("Artist search failed:", e);
      set({ artistSearchResults: [] });
    } finally {
      set({ searchingArtists: false });
    }
  },

  loadArtistSongs: async (artistId, offset = 0) => {
    set({ loadingArtistSongs: true });
    try {
      const songs = await invoke<Song[]>("music_artist_songs", { artistId, limit: 50, offset });
      set((state) => ({
        artistSongs: offset === 0 ? songs : [...state.artistSongs, ...songs],
      }));
    } catch (e) {
      console.error("Load artist songs failed:", e);
    } finally {
      set({ loadingArtistSongs: false });
    }
  },

  clearArtistState: () => {
    set({ artistSearchResults: [], artistSongs: [], selectedArtist: null });
  },

  searchPlaylists: async (keywords) => {
    if (!keywords.trim()) return;
    set({ searchingPlaylists: true, playlistSearchResults: [] });
    try {
      const results = await invoke<Playlist[]>("music_playlist_search", { keywords, limit: 30 });
      // 同步已收藏状态
      const subscribedIds = new Set(get().userPlaylists.filter((pl) => pl.subscribed).map((pl) => pl.id));
      set({
        playlistSearchResults: results.map((pl) =>
          ({ ...pl, subscribed: subscribedIds.has(pl.id) })
        ),
      });
    } catch (e) {
      console.error("Playlist search failed:", e);
      set({ playlistSearchResults: [] });
    } finally {
      set({ searchingPlaylists: false });
    }
  },

  playSong: async (song, queue) => {
    const state = get();
    const audio = state.audioRef;
    if (!audio) return;

    // 设置播放队列
    if (queue) {
      const idx = queue.findIndex((s) => s.id === song.id);
      set({ playQueue: queue, currentIndex: idx >= 0 ? idx : 0 });
    } else {
      // 手动切歌/上下首：在已有队列中找到对应位置
      const idx = state.playQueue.findIndex((s) => s.id === song.id);
      if (idx >= 0) set({ currentIndex: idx });
    }

    set({ currentSong: song, currentTime: 0, duration: 0, isPlaying: false, currentLyrics: null });

    try {
      const result = await invoke<SongUrlResult>("music_song_url", {
        id: song.id,
        quality: state.playbackQuality,
      });

      if (!result.playable || !result.url) {
        console.warn("Cannot play:", result.message);
        set({ isPlaying: false });
        return;
      }

      const audioUrl = await getProxyAudioUrl(result.url, state.proxyPort);
      audio.src = audioUrl;
      audio.volume = state.volume;
      await audio.play();
      set({ isPlaying: true, proxyPort: state.proxyPort || get().proxyPort, currentQuality: result.quality, currentBitrate: result.br });
      // 推送歌曲数据到桌面歌词
      if (get().desktopLyricsVisible) {
        get().emitDesktopLyricsData();
        emit("desktop-lyrics:state", { isPlaying: true, playMode: get().playMode, volume: get().volume });
      }
      // 异步加载歌词（不阻塞播放）
      get().loadLyrics(song.id);
    } catch (e) {
      console.error("Play failed:", e);
    }
  },

  togglePlay: async () => {
    const { audioRef, isPlaying } = get();
    if (!audioRef) return;
    if (isPlaying) {
      audioRef.pause();
      set({ isPlaying: false });
      if (get().desktopLyricsVisible) {
        emit("desktop-lyrics:state", { isPlaying: false, playMode: get().playMode, volume: get().volume });
      }
    } else {
      try {
        await audioRef.play();
        set({ isPlaying: true });
        if (get().desktopLyricsVisible) {
          emit("desktop-lyrics:state", { isPlaying: true, playMode: get().playMode, volume: get().volume });
        }
      } catch {
        // 播放失败，URL 可能已过期，尝试重新获取
        const state = get();
        const song = state.currentSong;
        if (song) {
          const savedTime = audioRef.currentTime;
          try {
            const result = await invoke<SongUrlResult>("music_song_url", {
              id: song.id,
              quality: state.playbackQuality,
            });
            if (result.playable && result.url) {
              const audioUrl = await getProxyAudioUrl(result.url, state.proxyPort);
              audioRef.src = audioUrl;
              audioRef.currentTime = savedTime;
              await audioRef.play();
              set({ isPlaying: true });
              return;
            }
          } catch {}
        }
        set({ isPlaying: false });
        if (get().desktopLyricsVisible) {
          emit("desktop-lyrics:state", { isPlaying: false, playMode: get().playMode, volume: get().volume });
        }
      }
    }
  },

  nextTrack: () => {
    const { playQueue, currentIndex, playMode, audioRef } = get();
    if (playQueue.length === 0) return;

    // 单曲循环：重新播放当前歌曲
    if (playMode === "one") {
      if (audioRef) {
        audioRef.currentTime = 0;
        audioRef.play().catch(() => {});
      }
      return;
    }

    let next: number;
    if (playMode === "shuffle") {
      next = Math.floor(Math.random() * playQueue.length);
    } else {
      next = currentIndex + 1;
      if (next >= playQueue.length) next = 0;
    }

    const song = playQueue[next];
    if (song) {
      get().playSong(song);
    }
  },

  prevTrack: () => {
    const { playQueue, currentIndex } = get();
    if (playQueue.length === 0) return;
    let prev = currentIndex - 1;
    if (prev < 0) prev = playQueue.length - 1;
    const song = playQueue[prev];
    if (song) {
      get().playSong(song);
    }
  },

  seekTo: (time) => {
    const { audioRef, isPlaying } = get();
    if (audioRef) {
      audioRef.currentTime = time;
      set({ currentTime: time });
      // 在线音频 seek 后需要重新缓冲，浏览器可能自动暂停
      // 如果之前是在播放状态，确保 seek 后继续播放
      if (isPlaying) {
        audioRef.play().catch(() => {});
      }
    }
  },

  setVolume: (v) => {
    const { audioRef, prevVolume } = get();
    if (audioRef) audioRef.volume = v;
    set({ volume: v, prevVolume: v > 0 ? v : prevVolume });
    getStore().then((s) => s.set("volume", v).then(() => s.save()));
  },

  togglePlayMode: () => {
    const modes: PlayMode[] = ["list", "shuffle", "one"];
    const current = modes.indexOf(get().playMode);
    const next = modes[(current + 1) % modes.length];
    set({ playMode: next });
    getStore().then((s) => s.set("playMode", next).then(() => s.save()));
    if (get().desktopLyricsVisible) {
      emit("desktop-lyrics:state", { isPlaying: get().isPlaying, playMode: next, volume: get().volume });
    }
  },

  setCurrentTime: (t) => set({ currentTime: t }),
  setDuration: (d) => set({ duration: d }),

  setPlaybackQuality: async (quality) => {
    const state = get();
    set({ playbackQuality: quality });
    getStore().then((s) => s.set("quality", quality).then(() => s.save()));

    if (state.currentSong && state.audioRef && state.audioRef.src) {
      try {
        const result = await invoke<SongUrlResult>("music_song_url", {
          id: state.currentSong.id,
          quality,
        });
        if (result.playable && result.url) {
          const resumeAt = state.audioRef.currentTime;
          const wasPlaying = state.isPlaying;
          const audioUrl = await getProxyAudioUrl(result.url, state.proxyPort);
          state.audioRef.src = audioUrl;
          state.audioRef.currentTime = resumeAt;
          if (wasPlaying) state.audioRef.play().catch(() => {});
          set({ currentQuality: result.quality, currentBitrate: result.br });
        }
      } catch (e) {
        console.error("Failed to switch quality:", e);
      }
    }
  },

  setLyricsFontSize: async (size) => {
    set({ lyricsFontSize: size });
    getStore().then((s) => s.set("lyricsFontSize", size).then(() => s.save()));
  },

  setLyricsHighlightColor: async (color) => {
    set({ lyricsHighlightColor: color });
    getStore().then((s) => s.set("lyricsHighlightColor", color).then(() => s.save()));
  },

  setExpandedStyle: async (style) => {
    set({ expandedStyle: style });
    getStore().then((s) => s.set("expandedStyle", style).then(() => s.save()));
  },

  setDynamicEnabled: async (enabled) => {
    set({ dynamicEnabled: enabled });
    getStore().then((s) => s.set("dynamicEnabled", enabled).then(() => s.save()));
  },

  setCoverFilmEffect: async (enabled) => {
    set({ coverFilmEffect: enabled });
    getStore().then((s) => s.set("coverFilmEffect", enabled).then(() => s.save()));
  },

  // ══ 桌面歌词 Actions ══
  toggleDesktopLyrics: async () => {
    const visible = !get().desktopLyricsVisible;
    await get().setDesktopLyricsVisible(visible);
  },

  setDesktopLyricsVisible: async (visible) => {
    set({ desktopLyricsVisible: visible });
    try {
      const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
      const win = await WebviewWindow.getByLabel("desktop-lyrics");
      if (win) {
        if (visible) {
          await win.show();
          await win.setFocus();
          startTimeSync();
          // 发送当前歌曲数据（此时新窗口可能还没挂载，listener 尚未注册）
          get().emitDesktopLyricsData();
          get().emitDesktopLyricsSettings();
          emit("desktop-lyrics:state", {
            isPlaying: get().isPlaying,
            playMode: get().playMode,
            volume: get().volume,
          });
          // 延迟重推：给桌面歌词窗口足够时间完成 React 挂载和 listener 注册
          [1500, 3000].forEach((ms) => {
            setTimeout(() => {
              if (get().desktopLyricsVisible) {
                get().emitDesktopLyricsData();
                get().emitDesktopLyricsSettings();
                emit("desktop-lyrics:state", {
                  isPlaying: get().isPlaying,
                  playMode: get().playMode,
                  volume: get().volume,
                });
              }
            }, ms);
          });
        } else {
          await win.hide();
          stopTimeSync();
        }
      }
    } catch (e) {
      console.error("[DesktopLyrics] toggle failed:", e);
    }
  },

  setDesktopLyricsFontSize: async (size) => {
    set({ desktopLyricsFontSize: size });
    getStore().then((s) => s.set("desktopLyricsFontSize", size).then(() => s.save()));
    get().emitDesktopLyricsSettings();
  },

  setDesktopLyricsHighlightColor: async (color) => {
    set({ desktopLyricsHighlightColor: color });
    getStore().then((s) => s.set("desktopLyricsHighlightColor", color).then(() => s.save()));
    get().emitDesktopLyricsSettings();
  },

  setDesktopLyricsBaseColor: async (color) => {
    set({ desktopLyricsBaseColor: color });
    getStore().then((s) => s.set("desktopLyricsBaseColor", color).then(() => s.save()));
    get().emitDesktopLyricsSettings();
  },

  setDesktopLyricsLineCount: async (count) => {
    set({ desktopLyricsLineCount: count });
    getStore().then((s) => s.set("desktopLyricsLineCount", count).then(() => s.save()));
    get().emitDesktopLyricsSettings();
  },

  setDesktopLyricsLocked: (locked) => {
    set({ desktopLyricsLocked: locked });
    getStore().then((s) => s.set("desktopLyricsLocked", locked).then(() => s.save()));
  },

  emitDesktopLyricsSettings: () => {
    const s = get();
    emit("desktop-lyrics:settings", {
      fontSize: s.desktopLyricsFontSize,
      highlightColor: s.desktopLyricsHighlightColor,
      baseColor: s.desktopLyricsBaseColor,
      lineCount: s.desktopLyricsLineCount,
      isLocked: s.desktopLyricsLocked,
    });
  },

  emitDesktopLyricsData: () => {
    const s = get();
    const karaokeLines = buildKaraokeLines(s.currentLyrics);
    emit("desktop-lyrics:data", {
      song: s.currentSong,
      karaokeLines,
      currentTime: s.audioRef?.currentTime ?? 0,
      isPlaying: s.isPlaying,
    });
  },

  loginStatus: async () => {
    try {
      const info = await invoke<LoginInfo>("music_login_status");
      set({ loginInfo: info });
    } catch {
      set({ loginInfo: null });
    }
  },

  loginWithCookie: async (cookie) => {
    try {
      const info = await invoke<LoginInfo>("music_login_cookie", { cookie });
      set({ loginInfo: info });
      if (info.logged_in) {
        get().loadUserPlaylists();
        get().loadLikedList();
      }
      return info.logged_in;
    } catch {
      return false;
    }
  },

  logout: async () => {
    try {
      await invoke("music_logout");
      set({ loginInfo: null, userPlaylists: [], likedSongIds: new Set() });
    } catch {
      // ignore
    }
  },

  openLoginWindow: async () => {
    // 如果已登录，先清除 cookie，让登录页显示全新的登录界面
    if (get().loginInfo?.logged_in) {
      await get().logout();
    }
    try {
      await invoke("music_open_login_window");
    } catch (e) {
      console.error("Failed to open login window:", e);
    }
  },

  loadUserPlaylists: async () => {
    set({ loadingPlaylists: true });
    try {
      const playlists = await invoke<Playlist[]>("music_user_playlist");
      set({ userPlaylists: playlists });
    } catch {
      set({ userPlaylists: [] });
    } finally {
      set({ loadingPlaylists: false });
    }
  },

  loadLeftPlaylistTracks: async (id) => {
    set({ loadingLeftTracks: true, leftPlaylistTracks: [], leftPlaylistMeta: null });
    try {
      const [meta, songs] = await invoke<[Playlist, Song[]]>("music_playlist_tracks", { id });
      set({ leftPlaylistMeta: meta, leftPlaylistTracks: songs });
      // 后台加载全部剩余 → 只追加到播放列表，不塞进歌单
      batchLoadToQueue(id, songs, meta.track_count);
    } catch {
      set({ leftPlaylistTracks: [] });
    } finally {
      set({ loadingLeftTracks: false });
    }
  },


  loadMoreLeftPlaylistTracks: async () => {
    const state = get();
    const id = state.leftPlaylistMeta?.id;
    const total = state.leftPlaylistMeta?.track_count ?? 0;
    if (!id || state.leftPlaylistLoadingMore) return;
    const start = state.leftPlaylistTracks.length;
    if (start >= total) return;
    set({ leftPlaylistLoadingMore: true });
    try {
      const songs = await invoke<Song[]>("music_playlist_tracks_range", { id, start, count: 50 });
      set((s) => {
        // 如果当前播放队列是从左侧歌单播放的，同步追加（去重）
        const shouldSync = s.playQueue.length > 0
          && s.playQueue[s.currentIndex]?.id === s.currentSong?.id
          && s.leftPlaylistTracks.length > 0
          && s.leftPlaylistTracks[0]?.id === s.playQueue[0]?.id;
        if (shouldSync) {
          const queueIds = new Set(s.playQueue.map((q) => q.id));
          const unique = songs.filter((song) => !queueIds.has(song.id));
          return {
            leftPlaylistTracks: [...s.leftPlaylistTracks, ...songs],
            playQueue: unique.length > 0 ? [...s.playQueue, ...unique] : s.playQueue,
          };
        }
        return {
          leftPlaylistTracks: [...s.leftPlaylistTracks, ...songs],
        };
      });
    } catch (e) {
      console.error("loadMore left failed:", e);
    } finally {
      set({ leftPlaylistLoadingMore: false });
    }
  },

  loadRightPlaylistTracks: async (id) => {
    set({ loadingRightTracks: true, rightPlaylistTracks: [], rightPlaylistMeta: null });
    try {
      const [meta, songs] = await invoke<[Playlist, Song[]]>("music_playlist_tracks", { id });
      set({ rightPlaylistMeta: meta, rightPlaylistTracks: songs });
      batchLoadToQueue(id, songs, meta.track_count);
    } catch {
      set({ rightPlaylistTracks: [] });
    } finally {
      set({ loadingRightTracks: false });
    }
  },

  loadMoreRightPlaylistTracks: async () => {
    const state = get();
    const id = state.rightPlaylistMeta?.id;
    const total = state.rightPlaylistMeta?.track_count ?? 0;
    if (!id || state.rightPlaylistLoadingMore) return;
    const start = state.rightPlaylistTracks.length;
    if (start >= total) return;
    set({ rightPlaylistLoadingMore: true });
    try {
      const songs = await invoke<Song[]>("music_playlist_tracks_range", { id, start, count: 50 });
      set((s) => {
        const shouldSync = s.playQueue.length > 0
          && s.playQueue[s.currentIndex]?.id === s.currentSong?.id
          && s.rightPlaylistTracks.length > 0
          && s.rightPlaylistTracks[0]?.id === s.playQueue[0]?.id;
        if (shouldSync) {
          const queueIds = new Set(s.playQueue.map((q) => q.id));
          const unique = songs.filter((song) => !queueIds.has(song.id));
          return {
            rightPlaylistTracks: [...s.rightPlaylistTracks, ...songs],
            playQueue: unique.length > 0 ? [...s.playQueue, ...unique] : s.playQueue,
          };
        }
        return {
          rightPlaylistTracks: [...s.rightPlaylistTracks, ...songs],
        };
      });
    } catch (e) {
      console.error("loadMore right failed:", e);
    } finally {
      set({ rightPlaylistLoadingMore: false });
    }
  },

  loadLikedList: async () => {
    try {
      const ids = await invoke<string[]>("music_likelist");
      console.log("[Music] liked songs loaded:", ids.length);
      set({ likedSongIds: new Set(ids) });
    } catch (e) {
      console.error("[Music] loadLikedList failed:", e);
    }
  },

  toggleLike: async (songId) => {
    const liked = get().likedSongIds.has(songId);
    // 乐观更新：先改 UI，API 在后台执行
    const newSet = new Set(get().likedSongIds);
    if (liked) {
      newSet.delete(songId);
    } else {
      newSet.add(songId);
    }
    set({ likedSongIds: newSet });
    try {
      await invoke("music_like", { id: songId, like: !liked });
    } catch (e) {
      // 回滚
      console.error("Toggle like failed:", e);
      const rollback = new Set(get().likedSongIds);
      if (liked) {
        rollback.add(songId);
      } else {
        rollback.delete(songId);
      }
      set({ likedSongIds: rollback });
    }
  },

  loadLyrics: async (songId) => {
    set({ loadingLyrics: true });
    try {
      const lyrics = await invoke<Lyrics>("music_lyric", { id: songId });
      set({ currentLyrics: lyrics });
      // 桌面歌词可见时推送歌词数据
      if (get().desktopLyricsVisible) {
        get().emitDesktopLyricsData();
      }
    } catch {
      set({ currentLyrics: null });
      if (get().desktopLyricsVisible) {
        get().emitDesktopLyricsData();
      }
    } finally {
      set({ loadingLyrics: false });
    }
  },

  loadRecommendations: async () => {
    try {
      const [playlists, songs] = await Promise.all([
        invoke<Playlist[]>("music_personalized"),
        invoke<Song[]>("music_recommend_songs").catch(() => []),
      ]);
      set({ recommendations: playlists, recommendSongs: songs });
    } catch {
      // ignore
    }
  },

  loadOfficialCharts: async () => {
    const chartIds = ["3778678", "19723756", "3779629", "6723173524", "5453912201", "6886768100"];
    const chartNames = ["热歌榜", "飙升榜", "新歌榜", "网络热歌榜", "VIP热歌榜", "中文DJ榜"];
    try {
      const results = await Promise.all(
        chartIds.map((id) =>
          invoke<Playlist>("music_playlist_detail", { id })
            .catch(() => ({
              provider: "netease" as const,
              id,
              name: chartNames[chartIds.indexOf(id)] || "",
              cover: "",
              track_count: 0,
              creator: "网易云音乐",
              subscribed: false,
            }))
        )
      );
      set({ officialCharts: results });
      // 持久化缓存
      try {
        const s = await getStore();
        await s.set("officialCharts", results);
        await s.save();
      } catch {}
    } catch {}
  },

  togglePlaylistSubscribe: async (playlistId, currentSubscribed) => {
    const newSubscribed = !currentSubscribed;
    try {
      await invoke("music_playlist_subscribe", { id: playlistId, subscribe: newSubscribed });
      // 刷新我的歌单列表
      await get().loadUserPlaylists();
      // 获取已收藏的歌单 ID 集合，同步到所有列表
      const subscribedIds = new Set(get().userPlaylists.filter((pl) => pl.subscribed).map((pl) => pl.id));
      set((state) => ({
        playlistSearchResults: state.playlistSearchResults.map((pl) =>
          ({ ...pl, subscribed: subscribedIds.has(pl.id) })
        ),
        recommendations: state.recommendations.map((pl) =>
          ({ ...pl, subscribed: subscribedIds.has(pl.id) })
        ),
        officialCharts: state.officialCharts.map((pl) =>
          ({ ...pl, subscribed: subscribedIds.has(pl.id) })
        ),
        leftPlaylistMeta: state.leftPlaylistMeta?.id === playlistId
          ? { ...state.leftPlaylistMeta, subscribed: newSubscribed } : state.leftPlaylistMeta,
        rightPlaylistMeta: state.rightPlaylistMeta?.id === playlistId
          ? { ...state.rightPlaylistMeta, subscribed: newSubscribed } : state.rightPlaylistMeta,
      }));
    } catch (e) {
      console.error("Playlist subscribe failed:", e);
    }
  },
}));
