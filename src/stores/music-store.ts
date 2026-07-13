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

  // 音频元素引用
  audioRef: HTMLAudioElement | null;

  // Actions
  init: () => Promise<void>;
  setAudioRef: (audio: HTMLAudioElement | null) => void;

  search: (keywords: string) => Promise<void>;
  searchArtists: (keywords: string) => Promise<void>;
  loadArtistSongs: (artistId: string, offset?: number) => Promise<void>;
  clearArtistState: () => void;
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
  loadRightPlaylistTracks: (id: string) => Promise<void>;
  loadLikedList: () => Promise<void>;
  toggleLike: (songId: string) => Promise<void>;
  loadLyrics: (songId: string) => Promise<void>;
  loadRecommendations: () => Promise<void>;
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
function stopTimeSync() {
  if (timeSyncTimer) {
    clearInterval(timeSyncTimer);
    timeSyncTimer = null;
  }
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

export const useMusicStore = create<MusicState>((set, get) => ({
  currentSong: null,
  isPlaying: false,
  currentTime: 0,
  duration: 0,
  volume: 0.7,
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
  likedSongIds: new Set(),
  currentLyrics: null,
  recommendations: [],
  recommendSongs: [],

  artistSearchResults: [],
  artistSongs: [],
  selectedArtist: null,
  searchingArtists: false,
  loadingArtistSongs: false,

  playbackQuality: "hires",
  currentQuality: "",
  currentBitrate: 0,
  lyricsFontSize: 18,
  lyricsHighlightColor: "#fff0b8",
  expandedStyle: "modern",
  dynamicEnabled: false,
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
      if (dlFontSize != null) set({ desktopLyricsFontSize: dlFontSize });
      if (dlHighlightColor) set({ desktopLyricsHighlightColor: dlHighlightColor });
      if (dlBaseColor) set({ desktopLyricsBaseColor: dlBaseColor });
      if (dlLineCount) set({ desktopLyricsLineCount: dlLineCount });
      if (dlLocked != null) set({ desktopLyricsLocked: dlLocked });
    } catch {
      // ignore
    }

    // 防止 React Strict Mode 双重调用导致重复注册
    if (listenersRegistered) return;
    listenersRegistered = true;

    // 桌面歌词控制事件监听
    listen<{ action: string }>("desktop-lyrics:control", (event) => {
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
    });

    // 桌面歌词窗口就绪后请求数据
    // 解决窗口首次打开时 emit 早于 listen 注册的时序问题
    listen("desktop-lyrics:request-data", () => {
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
    });

    // 监听登录成功事件 (来自网页登录窗口的 cookie 捕获)
    // 后端会附带 LoginInfo 数据
    listen<LoginInfo>("netease-login-success", async (event) => {
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
    });

    // 监听登录失败事件
    listen<string>("netease-login-failed", (event) => {
      console.error("[Music] Login failed:", event.payload);
    });

    // 检查登录状态
    await get().loginStatus();
    // 如果已登录, 加载歌单和喜欢列表
    if (get().loginInfo?.logged_in) {
      get().loadUserPlaylists();
      get().loadLikedList();
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
    const { audioRef } = get();
    if (audioRef) audioRef.volume = v;
    set({ volume: v });
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
    set({ loadingLeftTracks: true });
    try {
      const result = await invoke<[Playlist, Song[]]>("music_playlist_tracks", { id });
      set({ leftPlaylistMeta: result[0], leftPlaylistTracks: result[1] });
    } catch {
      set({ leftPlaylistTracks: [] });
    } finally {
      set({ loadingLeftTracks: false });
    }
  },

  loadRightPlaylistTracks: async (id) => {
    set({ loadingRightTracks: true });
    try {
      const result = await invoke<[Playlist, Song[]]>("music_playlist_tracks", { id });
      set({ rightPlaylistMeta: result[0], rightPlaylistTracks: result[1] });
    } catch {
      set({ rightPlaylistTracks: [] });
    } finally {
      set({ loadingRightTracks: false });
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
    try {
      await invoke("music_like", { id: songId, like: !liked });
      const newSet = new Set(get().likedSongIds);
      if (liked) {
        newSet.delete(songId);
      } else {
        newSet.add(songId);
      }
      set({ likedSongIds: newSet });
    } catch (e) {
      console.error("Toggle like failed:", e);
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
}));
