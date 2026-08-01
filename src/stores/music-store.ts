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
  MusicProvider,
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

  // 登录状态 (多平台)
  loginInfo: LoginInfo | null; // 当前播放源的登录信息 (向后兼容)
  loginInfos: Record<MusicProvider, LoginInfo | null>; // 所有平台登录信息
  playbackSource: MusicProvider; // 当前播放源

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

  // Toast 通知
  musicToast: { type: "warning"; message: string } | null;

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
  setDesktopLyricsLocked: (locked: boolean) => Promise<void>;
  emitDesktopLyricsSettings: () => void;
  emitDesktopLyricsData: () => void;

  loginStatus: () => Promise<void>;
  loginStatusFor: (provider: MusicProvider) => Promise<void>;
  loginWithCookie: (cookie: string) => Promise<boolean>;
  logout: () => Promise<void>;
  logoutFor: (provider: MusicProvider) => Promise<void>;
  openLoginWindow: (provider?: MusicProvider) => Promise<void>;
  switchPlaybackSource: (provider: MusicProvider) => Promise<void>;
  loadAllLoginStatuses: () => Promise<void>;

  loadUserPlaylists: () => Promise<void>;
  loadUserPlaylistsFor: (provider: MusicProvider) => Promise<void>;
  loadLeftPlaylistTracks: (id: string) => Promise<void>;
  loadMoreLeftPlaylistTracks: () => Promise<void>;
  loadRightPlaylistTracks: (id: string) => Promise<void>;
  loadMoreRightPlaylistTracks: () => Promise<void>;
  loadLikedList: () => Promise<void>;
  toggleLike: (songId: string) => Promise<void>;
  loadLyrics: (songId: string) => Promise<void>;
  loadLyricsForSong: (song: Song) => Promise<void>;
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
    // 仅桌面歌词可见且正在播放时同步时间，暂停时跳过以节省 CPU
    if (state.audioRef && state.desktopLyricsVisible && state.isPlaying) {
      emit("desktop-lyrics:time", {
        currentTime: state.audioRef.currentTime,
        isPlaying: state.isPlaying,
      });
    }
  }, 200);
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

let playSongSeq = 0;

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
  loginInfos: { netease: null, kugou: null, qqmusic: null },
  playbackSource: "netease",

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
  musicToast: null,
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
      if (vol != null) set({ volume: vol, prevVolume: vol > 0 ? vol : 0.7 });
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
      if (dlLocked != null) {
        // 锁定状态仅在当前会话有效，启动时始终重置为 false
        // 防止跨会话残留导致桌面歌词未开但解锁按钮仍在的问题
        if (dlLocked) {
          await store.set("desktopLyricsLocked", false);
          await store.save();
        }
      }

      // 确保内存状态与持久化一致
      set({ desktopLyricsLocked: false });
    } catch {
      // ignore
    }

    // 加载缓存的官方榜单（优先显示缓存）
    try {
      const s = await getStore();
      const cached = await s.get<Playlist[]>("officialCharts");
      if (cached && cached.length > 0) {
        set({ officialCharts: cached });
      }
    } catch {}

    // 加载播放源 (重启后恢复上次使用的平台)
    try {
      const source = await invoke<MusicProvider>("music_get_playback_source");
      if (source) set({ playbackSource: source });
    } catch {}

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
          case "close":
            get().setDesktopLyricsVisible(false);
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

    // 监听网易云登录成功事件
    unlistenFns.push(
      await listen<LoginInfo>("netease-login-success", async (event) => {
        console.log("[Music] Netease login success", event.payload);
        const info = event.payload;
        if (info && info.logged_in) {
          // 如果当前播放源未登录, 自动切换到网易云
          const currentInfo = get().loginInfos[get().playbackSource];
          const shouldSwitch = !currentInfo?.logged_in;
          if (shouldSwitch) {
            set({ playbackSource: "netease" });
            try { await invoke("music_switch_provider", { provider: "netease" }); } catch {}
          }
          set((s) => ({
            loginInfo: (shouldSwitch || s.playbackSource === "netease") ? info : s.loginInfo,
            loginInfos: { ...s.loginInfos, netease: info }
          }));
          get().loadUserPlaylists();
          get().loadLikedList();
          get().loadOfficialCharts();
          get().loadRecommendations();
        }
      })
    );

    // 监听酷狗登录成功事件
    unlistenFns.push(
      await listen<LoginInfo>("kugou-login-success", async (event) => {
        console.log("[Music] Kugou login success", event.payload);
        const info = event.payload;
        if (info && info.logged_in) {
          // 如果当前播放源未登录, 自动切换到酷狗
          const currentInfo = get().loginInfos[get().playbackSource];
          const shouldSwitch = !currentInfo?.logged_in;
          if (shouldSwitch) {
            set({ playbackSource: "kugou" });
            try { await invoke("music_switch_provider", { provider: "kugou" }); } catch {}
          }
          set((s) => ({
            loginInfo: (shouldSwitch || s.playbackSource === "kugou") ? info : s.loginInfo,
            loginInfos: { ...s.loginInfos, kugou: info }
          }));
          // 加载歌单等数据
          get().loadUserPlaylists();
          get().loadLikedList();
          get().loadRecommendations();
          get().loadOfficialCharts();
        }
      })
    );

    // 监听登录失败事件
    unlistenFns.push(
      await listen<string>("netease-login-failed", (event) => {
        console.error("[Music] Netease login failed:", event.payload);
      })
    );
    unlistenFns.push(
      await listen<string>("kugou-login-failed", (event) => {
        console.error("[Music] Kugou login failed:", event.payload);
      })
    );

    // 监听 QQ 音乐登录成功事件
    unlistenFns.push(
      await listen<LoginInfo>("qqmusic-login-success", async (event) => {
        console.log("[Music] QQ music login success", event.payload);
        const info = event.payload;
        if (info && info.logged_in) {
          const currentInfo = get().loginInfos[get().playbackSource];
          const shouldSwitch = !currentInfo?.logged_in;
          if (shouldSwitch) {
            set({ playbackSource: "qqmusic" });
            try { await invoke("music_switch_provider", { provider: "qqmusic" }); } catch {}
          }
          set((s) => ({
            loginInfo: (shouldSwitch || s.playbackSource === "qqmusic") ? info : s.loginInfo,
            loginInfos: { ...s.loginInfos, qqmusic: info }
          }));
          get().loadUserPlaylists();
          get().loadLikedList();
          get().loadOfficialCharts();
        }
      })
    );
    unlistenFns.push(
      await listen<string>("qqmusic-login-failed", (event) => {
        console.error("[Music] QQ music login failed:", event.payload);
      })
    );

    // 加载所有平台登录状态
    await get().loadAllLoginStatuses();
    // 加载当前播放源的歌单
    if (get().loginInfo?.logged_in) {
      get().loadUserPlaylists();
      if (get().playbackSource === "netease") {
        await get().loadLikedList();
        get().loadOfficialCharts();
        get().loadRecommendations();
      } else if (get().playbackSource === "kugou") {
        await get().loadLikedList();
        get().loadOfficialCharts();
        get().loadRecommendations();
      } else if (get().playbackSource === "qqmusic") {
        await get().loadLikedList();
        get().loadOfficialCharts();
      }
    }
  },

  setAudioRef: (audio) => set({ audioRef: audio }),

  search: async (keywords) => {
    if (!keywords.trim()) return;
    set({ searching: true });
    try {
      const provider = get().playbackSource;
      const cmd = provider === "kugou" ? "kugou_search"
        : provider === "qqmusic" ? "qq_search"
        : "music_search";
      const results = await invoke<Song[]>(cmd, { keywords, limit: 30 });
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
            const provider = get().playbackSource;
      const cmd = provider === "kugou" ? "kugou_artist_search"
        : provider === "qqmusic" ? "qq_artist_search"
        : "music_artist_search";
      const results = await invoke<Artist[]>(cmd, { keywords, limit: 30 });
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
            const provider = get().playbackSource;
      const cmd = provider === "kugou" ? "kugou_artist_songs"
        : provider === "qqmusic" ? "qq_artist_songs"
        : "music_artist_songs";
      const songs = await invoke<Song[]>(cmd, { artistId, limit: 50, offset });
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
            const provider = get().playbackSource;
      const cmd = provider === "kugou" ? "kugou_playlist_search"
        : provider === "qqmusic" ? "qq_playlist_search"
        : "music_playlist_search";
      const results = await invoke<Playlist[]>(cmd, { keywords, limit: 30 });
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

    // 立即停止当前播放，防止旧歌在新 URL 获取期间播完并触发 ended → nextTrack 竞态
    // 同时递增序列号，使任何正在飞行中的 playSong 调用被忽略
    const mySeq = ++playSongSeq;
    audio.pause();
    audio.src = "";

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
      // 根据歌曲 provider 调用对应 API
      const result = song.provider === "kugou"
        ? await invoke<SongUrlResult>("kugou_song_url", {
            hash: song.hash || song.id,
            albumId: song.album_id,
            albumAudioId: song.album_audio_id,
            quality: state.playbackQuality,
            hqHash: song.hq_hash,
            sqHash: song.sq_hash,
            resHash: song.res_hash,
          })
        : song.provider === "qqmusic"
        ? await invoke<SongUrlResult>("qq_song_url", {
            mid: song.mid || song.id,
            mediaMid: song.media_mid,
            quality: state.playbackQuality,
          })
        : await invoke<SongUrlResult>("music_song_url", {
            id: song.id,
            quality: state.playbackQuality,
          });

      // 检查是否有更新的 playSong 调用覆盖了本次请求
      if (mySeq !== playSongSeq) return;

      if (!result.playable || !result.url) {
        console.warn("Cannot play:", result.message);
        set({ isPlaying: false });
        return;
      }

      const audioUrl = await getProxyAudioUrl(result.url, state.proxyPort);

      if (mySeq !== playSongSeq) return;

      audio.src = audioUrl;
      audio.volume = state.volume;
      await audio.play();

      if (mySeq !== playSongSeq) return;

      set({ isPlaying: true, proxyPort: state.proxyPort || get().proxyPort, currentQuality: result.quality, currentBitrate: result.br });
      // 推送歌曲数据到桌面歌词
      if (get().desktopLyricsVisible) {
        get().emitDesktopLyricsData();
        emit("desktop-lyrics:state", { isPlaying: true, playMode: get().playMode, volume: get().volume });
      }
      // 异步加载歌词（不阻塞播放）
      get().loadLyricsForSong(song);
    } catch (e) {
      if (mySeq !== playSongSeq) return;
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
            const result = song.provider === "kugou"
              ? await invoke<SongUrlResult>("kugou_song_url", {
                  hash: song.hash || song.id,
                  albumId: song.album_id,
                  albumAudioId: song.album_audio_id,
                  quality: state.playbackQuality,
                  hqHash: song.hq_hash,
                  sqHash: song.sq_hash,
                  resHash: song.res_hash,
                })
              : song.provider === "qqmusic"
              ? await invoke<SongUrlResult>("qq_song_url", {
                  mid: song.mid || song.id,
                  mediaMid: song.media_mid,
                  quality: state.playbackQuality,
                })
              : await invoke<SongUrlResult>("music_song_url", {
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
        const song = state.currentSong;
        const result = song.provider === "kugou"
          ? await invoke<SongUrlResult>("kugou_song_url", {
              hash: song.hash || song.id,
              albumId: song.album_id,
              albumAudioId: song.album_audio_id,
              quality,
              hqHash: song.hq_hash,
              sqHash: song.sq_hash,
              resHash: song.res_hash,
            })
          : song.provider === "qqmusic"
          ? await invoke<SongUrlResult>("qq_song_url", {
              mid: song.mid || song.id,
              mediaMid: song.media_mid,
              quality,
            })
          : await invoke<SongUrlResult>("music_song_url", {
              id: song.id,
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
          // 关闭桌面歌词时同时重置锁定状态，防止重启后残留锁定状态
          await get().setDesktopLyricsLocked(false);
          // 通知桌面歌词页面解锁并停止轮询，防止在隐藏窗口后仍显示解锁按钮
          emit("desktop-lyrics:settings", {
            fontSize: get().desktopLyricsFontSize,
            highlightColor: get().desktopLyricsHighlightColor,
            baseColor: get().desktopLyricsBaseColor,
            lineCount: get().desktopLyricsLineCount,
            isLocked: false,
          });
          try {
            await invoke("hide_lyrics_unlock_btn");
          } catch {
            // ignore
          }
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

  setDesktopLyricsLocked: async (locked) => {
    set({ desktopLyricsLocked: locked });
    const s = await getStore();
    await s.set("desktopLyricsLocked", locked);
    await s.save();
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
    await get().loginStatusFor(get().playbackSource);
  },

  loginStatusFor: async (provider) => {
    try {
      const cmd = provider === "kugou" ? "kugou_login_status" : "music_login_status";
      const info = await invoke<LoginInfo>(cmd);
      set((s) => ({
        loginInfos: { ...s.loginInfos, [provider]: info },
        loginInfo: s.playbackSource === provider ? info : s.loginInfo,
      }));
    } catch {
      set((s) => ({
        loginInfos: { ...s.loginInfos, [provider]: null },
        loginInfo: s.playbackSource === provider ? null : s.loginInfo,
      }));
    }
  },

  loadAllLoginStatuses: async () => {
    try {
      const statuses = await invoke<Record<string, LoginInfo>>("music_get_login_statuses");
      const currentSource = get().playbackSource;
      const loginInfos: Record<MusicProvider, LoginInfo | null> = {
        netease: statuses.netease || null,
        kugou: statuses.kugou || null,
        qqmusic: statuses.qqmusic || null,
      };
      set({
        loginInfos,
        loginInfo: loginInfos[currentSource],
      });
    } catch {
      // fallback to individual calls
      await get().loginStatusFor("netease");
      await get().loginStatusFor("kugou");
    }
  },

  loginWithCookie: async (cookie) => {
    try {
      const info = await invoke<LoginInfo>("music_login_cookie", { cookie });
      set((s) => ({
        loginInfos: { ...s.loginInfos, netease: info },
        loginInfo: s.playbackSource === "netease" ? info : s.loginInfo,
      }));
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
    await get().logoutFor(get().playbackSource);
  },

  logoutFor: async (provider) => {
    try {
    const cmd = provider === "kugou" ? "kugou_logout"
      : provider === "qqmusic" ? "qq_logout"
      : "music_logout";
    await invoke(cmd);
      set((s) => ({
        loginInfos: { ...s.loginInfos, [provider]: null },
        loginInfo: s.playbackSource === provider ? null : s.loginInfo,
        userPlaylists: s.playbackSource === provider ? [] : s.userPlaylists,
        likedSongIds: s.playbackSource === provider ? new Set() : s.likedSongIds,
      }));
    } catch {
      // ignore
    }
  },

  openLoginWindow: async (provider) => {
    const target = provider || get().playbackSource;
    // 如果已登录该平台，先退出
    if (get().loginInfos[target]?.logged_in) {
      await get().logoutFor(target);
    }
    try {
      await invoke("music_open_login_window", { provider: target });
    } catch (e) {
      console.error(`Failed to open ${target} login window:`, e);
    }
  },

  switchPlaybackSource: async (provider) => {
    set({ playbackSource: provider });
    try {
      await invoke("music_switch_provider", { provider });
    } catch {}
    // 更新 loginInfo 为当前平台的登录状态
    const info = get().loginInfos[provider];
    // 切换平台时立即清空榜单/推荐，避免旧平台数据残留闪烁
    set({ loginInfo: info, userPlaylists: [], officialCharts: [], recommendations: [], recommendSongs: [] });
    // 重新加载当前平台的歌单
    if (info?.logged_in) {
      get().loadUserPlaylists();
      if (provider === "netease") {
        await get().loadLikedList();
        get().loadOfficialCharts();
        get().loadRecommendations();
      } else if (provider === "kugou") {
        await get().loadLikedList();
        get().loadRecommendations();
        get().loadOfficialCharts();
      } else if (provider === "qqmusic") {
        await get().loadLikedList();
        get().loadOfficialCharts();
      }
    }
  },

  loadUserPlaylists: async () => {
    await get().loadUserPlaylistsFor(get().playbackSource);
  },

  loadUserPlaylistsFor: async (provider) => {
    set({ loadingPlaylists: true });
    try {
      const cmd = provider === "kugou" ? "kugou_user_playlists"
        : provider === "qqmusic" ? "qq_user_playlists"
        : "music_user_playlist";
      const playlists = await invoke<Playlist[]>(cmd);
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
      const provider = get().playbackSource;
      const cmd = provider === "kugou" ? "kugou_playlist_tracks"
        : provider === "qqmusic" ? "qq_playlist_tracks"
        : "music_playlist_tracks";
      const [meta, songs] = await invoke<[Playlist, Song[]]>(cmd, { id });
      set({ leftPlaylistMeta: meta, leftPlaylistTracks: songs });
      // 后台加载全部剩余 → 只追加到播放列表，不塞进歌单
      if (provider === "netease") {
        batchLoadToQueue(id, songs, meta.track_count);
      }
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
      const provider = get().playbackSource;
      const cmd = provider === "kugou" ? "kugou_playlist_tracks_range"
        : provider === "qqmusic" ? "qq_playlist_tracks_range"
        : "music_playlist_tracks_range";
      const songs = await invoke<Song[]>(cmd, { id, start, count: 50 });
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
      const provider = get().playbackSource;
      const cmd = provider === "kugou" ? "kugou_playlist_tracks"
        : provider === "qqmusic" ? "qq_playlist_tracks"
        : "music_playlist_tracks";
      const [meta, songs] = await invoke<[Playlist, Song[]]>(cmd, { id });
      set({ rightPlaylistMeta: meta, rightPlaylistTracks: songs });
      if (provider === "netease") {
        batchLoadToQueue(id, songs, meta.track_count);
      }
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
      const provider = get().playbackSource;
      const cmd = provider === "kugou" ? "kugou_playlist_tracks_range"
        : provider === "qqmusic" ? "qq_playlist_tracks_range"
        : "music_playlist_tracks_range";
      const songs = await invoke<Song[]>(cmd, { id, start, count: 50 });
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
      const provider = get().playbackSource;
      if (provider === "kugou") {
        // 酷狗: 从"我喜欢"歌单获取已喜欢的歌曲 hash 列表
        const hashes = await invoke<string[]>("kugou_liked_hashes").catch(() => []);
        console.log("[Music] kugou liked songs loaded:", hashes.length);
        set({ likedSongIds: new Set(hashes) });
      } else if (provider === "qqmusic") {
        // QQ 音乐: 从"我喜欢"歌单获取已喜欢的歌曲 mid 列表
        const mids = await invoke<string[]>("qq_liked_hashes").catch(() => []);
        console.log("[Music] qq liked songs loaded:", mids.length);
        set({ likedSongIds: new Set(mids) });
      } else {
        const ids = await invoke<string[]>("music_likelist");
        console.log("[Music] liked songs loaded:", ids.length);
        set({ likedSongIds: new Set(ids) });
      }
    } catch (e) {
      console.error("[Music] loadLikedList failed:", e);
    }
  },

  toggleLike: async (songId) => {
    const provider = get().playbackSource;
    // QQ 音乐暂不支持写回红心
    if (provider === "qqmusic") {
      set({ musicToast: { type: "warning", message: "QQ 音乐当前仅支持读取账号收藏，暂不支持写回" } });
      return;
    }
    const liked = get().likedSongIds.has(songId);
    console.log("[Music] toggleLike: provider=", provider, "songId=", songId, "liked=", liked);
    // 乐观更新：先改 UI，API 在后台执行
    const newSet = new Set(get().likedSongIds);
    if (liked) {
      newSet.delete(songId);
    } else {
      newSet.add(songId);
    }
    set({ likedSongIds: newSet });
    try {
      if (provider === "kugou") {
        // 酷狗: 需要完整的歌曲对象来执行喜欢/取消喜欢
        // 从多个来源查找完整歌曲对象 (currentSong → playQueue → searchResults → 歌单列表 → 推荐)
        let song = get().currentSong;
        if (!song || song.id !== songId) {
          song = get().playQueue.find((s) => s.id === songId) || null;
        }
        if (!song) {
          song = get().searchResults.find((s) => s.id === songId) || null;
        }
        if (!song) {
          song = get().leftPlaylistTracks.find((s) => s.id === songId) || null;
        }
        if (!song) {
          song = get().rightPlaylistTracks.find((s) => s.id === songId) || null;
        }
        if (!song) {
          song = get().recommendSongs.find((s) => s.id === songId) || null;
        }
        if (!song) {
          song = get().artistSongs.find((s) => s.id === songId) || null;
        }
        if (!song) {
          console.warn("[Music] toggleLike: song not found in any list, using minimal object");
          // 构造最小歌曲对象，QQ音乐需要 mid，酷狗需要 hash
          song = { provider, id: songId, hash: songId, mid: songId, name: "", artist: "", artists: [], album: "", cover: "", duration: 0, fee: 0, playable: true, language: 0 };
        }
        const likeCmd = provider === "kugou" ? "kugou_like_toggle" : "qq_like_toggle";
        await invoke(likeCmd, { song, like: !liked });
        // 刷新喜欢列表, 确保与服务器同步
        await get().loadLikedList();
        // 后台异步刷新歌单列表，更新"我喜欢"的歌单曲目数量
        get().loadUserPlaylists();
      } else {
        await invoke("music_like", { id: songId, like: !liked });
        // 刷新喜欢列表和歌单列表, 确保与服务器同步
        await get().loadLikedList();
        get().loadUserPlaylists();
      }
    } catch (e) {
      // 回滚
      console.error("Toggle like failed:", e, "provider:", provider, "songId:", songId, "like:", !liked);
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

  loadLyricsForSong: async (song) => {
    set({ loadingLyrics: true });
    try {
      const lyrics = song.provider === "kugou"
        ? await invoke<Lyrics>("kugou_lyric", {
            hash: song.hash || song.id,
            albumAudioId: song.album_audio_id,
            duration: Math.floor(song.duration / 1000),
          })
        : song.provider === "qqmusic"
        ? await invoke<Lyrics>("qq_lyric", {
            mid: song.mid || song.id,
            id: song.id,
          })
        : await invoke<Lyrics>("music_lyric", { id: song.id });
      set({ currentLyrics: lyrics });
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
      const provider = get().playbackSource;
      if (provider === "kugou" || provider === "qqmusic") {
        // 酷狗/QQ 没有推荐歌单 API, 清空推荐
        set({ recommendations: [], recommendSongs: [] });
      } else {
        const [playlists, songs] = await Promise.all([
          invoke<Playlist[]>("music_personalized"),
          invoke<Song[]>("music_recommend_songs").catch(() => []),
        ]);
        set({ recommendations: playlists, recommendSongs: songs });
      }
    } catch {
      // ignore
    }
  },

  loadOfficialCharts: async () => {
    // 酷狗平台: 加载酷狗官方排行榜
    if (get().playbackSource === "kugou") {
      try {
        const charts = await invoke<Playlist[]>("kugou_rank_list").catch(() => []);
        set({ officialCharts: charts });
        // 持久化缓存
        try {
          const s = await getStore();
          await s.set("officialCharts", charts);
          await s.save();
        } catch {}
      } catch {
        set({ officialCharts: [] });
      }
      return;
    }
    // QQ 音乐: 榜单 API 已失效，直接显示空
    if (get().playbackSource === "qqmusic") {
      set({ officialCharts: [] });
      return;
    }
    const chartIds = [
      "3778678",      // 热歌榜
      "19723756",     // 飙升榜
      "3779629",      // 新歌榜
      "2884035",      // 原创榜
      "112504",       // 抖音排行榜
      "6723173524",   // 网络热歌榜
      "5453912201",   // VIP热歌榜
      "6886768100",   // 中文DJ榜
      "1978921795",   // 电音榜
      "2809513713",   // 说唱榜
      "71384707",     // 古典榜
    ];
    const chartNames = [
      "热歌榜", "飙升榜", "新歌榜", "原创榜",
      "抖音排行榜", "网络热歌榜", "VIP热歌榜", "中文DJ榜",
      "电音榜", "说唱榜", "古典榜",
    ];
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
