import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Store } from "@tauri-apps/plugin-store";
import type {
  Song,
  Playlist,
  LoginInfo,
  Lyrics,
  PlayMode,
  PlaybackQuality,
  SongUrlResult,
} from "@/types/music";

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
  currentPlaylistTracks: Song[];
  currentPlaylistMeta: Playlist | null;
  likedSongIds: Set<string>;
  currentLyrics: Lyrics | null;
  recommendations: Playlist[];
  recommendSongs: Song[];

  // 音质
  playbackQuality: PlaybackQuality;
  currentQuality: string;
  currentBitrate: number;

  // 代理端口
  proxyPort: number;

  // 歌词字体大小
  lyricsFontSize: number;

  // UI 状态
  searching: boolean;
  loadingPlaylists: boolean;
  loadingTracks: boolean;
  loadingLyrics: boolean;

  // 音频元素引用
  audioRef: HTMLAudioElement | null;

  // Actions
  init: () => Promise<void>;
  setAudioRef: (audio: HTMLAudioElement | null) => void;

  search: (keywords: string) => Promise<void>;
  playSong: (song: Song, queue?: Song[]) => Promise<void>;
  togglePlay: () => void;
  nextTrack: () => void;
  prevTrack: () => void;
  seekTo: (time: number) => void;
  setVolume: (v: number) => void;
  togglePlayMode: () => void;
  setPlaybackQuality: (quality: PlaybackQuality) => Promise<void>;
  setLyricsFontSize: (size: number) => Promise<void>;
  setCurrentTime: (t: number) => void;
  setDuration: (d: number) => void;

  loginStatus: () => Promise<void>;
  loginWithCookie: (cookie: string) => Promise<boolean>;
  logout: () => Promise<void>;
  openLoginWindow: () => Promise<void>;

  loadUserPlaylists: () => Promise<void>;
  loadPlaylistTracks: (id: string) => Promise<void>;
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
  currentPlaylistTracks: [],
  currentPlaylistMeta: null,
  likedSongIds: new Set(),
  currentLyrics: null,
  recommendations: [],
  recommendSongs: [],

  playbackQuality: "hires",
  currentQuality: "",
  currentBitrate: 0,
  lyricsFontSize: 18,
  proxyPort: 0,

  searching: false,
  loadingPlaylists: false,
  loadingTracks: false,
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
      if (vol != null) set({ volume: vol });
      if (mode) set({ playMode: mode });
      if (quality) set({ playbackQuality: quality });
      if (fontSize != null) set({ lyricsFontSize: fontSize });
    } catch {
      // ignore
    }

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

    set({ currentSong: song, currentTime: 0, duration: 0, isPlaying: false });

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
    } catch (e) {
      console.error("Play failed:", e);
    }
  },

  togglePlay: () => {
    const { audioRef, isPlaying } = get();
    if (!audioRef) return;
    if (isPlaying) {
      audioRef.pause();
      set({ isPlaying: false });
    } else {
      audioRef.play().catch(() => {});
      set({ isPlaying: true });
    }
  },

  nextTrack: () => {
    const { playQueue, currentIndex, playMode } = get();
    if (playQueue.length === 0) return;

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

  loadPlaylistTracks: async (id) => {
    set({ loadingTracks: true });
    try {
      const result = await invoke<[Playlist, Song[]]>("music_playlist_tracks", { id });
      set({ currentPlaylistMeta: result[0], currentPlaylistTracks: result[1] });
    } catch {
      set({ currentPlaylistTracks: [] });
    } finally {
      set({ loadingTracks: false });
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
    } catch {
      set({ currentLyrics: null });
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
