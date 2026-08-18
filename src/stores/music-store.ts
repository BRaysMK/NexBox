import { create } from "zustand";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen, emit } from "@tauri-apps/api/event";
import type { PlaybackAudio } from "@/lib/rust-audio";
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
  CommentPage,
  Album,
  Mv,
  ArtistDetail,
} from "@/types/music";
import { buildKaraokeLines } from "@/lib/karaoke-lyrics";
import { updateMediaSession, setMediaPlaybackState, registerMediaActions } from "@/lib/media-session";

// 模块级：无版权自动跳过控制
let isAutoSkipping = false;
let unplayableSkipCount = 0;

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
  // 心动模式：基于当前歌曲的相似歌曲动态队列（仅网易云源）
  heartbeatQueue: Song[];
  heartbeatLoading: boolean;
  // 心动模式已播放过的歌曲 ID（用于去重，避免相似歌曲重复播放）
  heartbeatPlayedIds: Set<string>;

  // 本地导入歌曲
  localSongs: Song[];
  // 本地导入进行中（文件夹/多文件导入耗时长，用于 UI 反馈避免误以为卡死）
  importingLocal: boolean;
  // 本地导入进度（分批事件推送）
  importProgress: { done: number; total: number };

  // 登录状态 (多平台)
  loginInfo: LoginInfo | null; // 当前播放源的登录信息 (向后兼容)
  loginInfos: Record<MusicProvider, LoginInfo | null>; // 所有平台登录信息
  playbackSource: MusicProvider; // 当前播放源

  // 数据
  searchResults: Song[];
  userPlaylists: Playlist[];
  userPlaylistsError: string;
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
  dailyRecommendPlaylists: Playlist[];

  // 歌手搜索
  artistSearchResults: Artist[];
  artistSongs: Song[];
  selectedArtist: Artist | null;
  searchingArtists: boolean;
  loadingArtistSongs: boolean;
  artistDetail: ArtistDetail | null;
  artistAlbums: Album[];
  artistMvs: Mv[];
  albumDetailSongs: Song[];
  albumDetailMeta: Album | null;
  loadingArtistDetail: boolean;
  loadingArtistAlbums: boolean;
  loadingArtistMvs: boolean;
  loadingAlbumDetail: boolean;

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
  desktopLyricsShowTranslation: boolean;
  desktopLyricsHideUnlockBtn: boolean;

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

  // 音频元素引用（RustAudio 适配器，模拟 HTMLAudioElement 接口）
  audioRef: PlaybackAudio | null;

  // Actions
  init: () => Promise<void>;
  setAudioRef: (audio: PlaybackAudio | null) => void;

  // 本地导入歌曲 Actions
  loadLocalSongs: () => Promise<void>;
  importLocalSongs: (paths: string[]) => Promise<{ count: number; noCoverCount: number }>;
  importLocalFolder: (folder: string) => Promise<{ count: number; noCoverCount: number }>;
  setImportingLocal: (importing: boolean) => void;
  removeLocalSong: (id: string) => Promise<void>;
  clearLocalSongs: () => Promise<void>;

  search: (keywords: string) => Promise<void>;
  searchArtists: (keywords: string) => Promise<void>;
  loadArtistSongs: (artistId: string, offset?: number) => Promise<void>;
  loadArtistDetail: (artistId: string) => Promise<void>;
  loadArtistAlbums: (artistId: string, offset?: number) => Promise<void>;
  loadArtistMvs: (artistId: string, offset?: number) => Promise<void>;
  loadAlbumDetail: (albumId: string) => Promise<void>;
  clearArtistState: () => void;
  searchPlaylists: (keywords: string) => Promise<void>;
  playSong: (song: Song, queue?: Song[]) => Promise<void>;
  togglePlay: () => void;
  nextTrack: () => void;
  prevTrack: () => void;
  seekTo: (time: number) => void;
  setVolume: (v: number) => void;
  togglePlayMode: () => void;
  loadHeartbeatSongs: (baseSong: Song | null) => Promise<void>;
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
  setDesktopLyricsShowTranslation: (show: boolean) => Promise<void>;
  setDesktopLyricsHideUnlockBtn: (hide: boolean) => Promise<void>;
  toggleDesktopLyricsHideUnlockBtn: () => Promise<void>;
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
  loadRightRankTracks: (rankId: string) => Promise<void>;
  loadMoreRightPlaylistTracks: () => Promise<void>;
  loadLikedList: () => Promise<void>;
  toggleLike: (songId: string) => Promise<void>;
  loadLyrics: (songId: string) => Promise<void>;
  loadLyricsForSong: (song: Song) => Promise<void>;
  loadRecommendations: () => Promise<void>;
  loadOfficialCharts: () => Promise<void>;
  togglePlaylistSubscribe: (playlistId: string, currentSubscribed: boolean) => Promise<void>;

  // 评论系统
  currentComments: CommentPage | null;
  loadingComments: boolean;
  sendingComment: boolean;
  commentError: string;
  loadComments: (songId: string, page?: number) => Promise<void>;
  sendComment: (songId: string, content: string) => Promise<boolean>;
  clearComments: () => void;
}

let storeInstance: Store | null = null;
const getStore = async (): Promise<Store> => {
  if (!storeInstance) {
    storeInstance = await Store.load("music-player-settings.json");
  }
  return storeInstance;
};

// 序列化本地歌曲对象用于持久化，确保字段完整
function serializeLocalSong(song: Song): Record<string, unknown> {
  return {
    provider: "local",
    id: song.id,
    name: song.name,
    artist: song.artist,
    artists: song.artists || [],
    album: song.album,
    // 封面 base64 可能极大（1910 首 × 几十 KB → 几十 MB），
    // 写入 store 会卡死/崩溃。持久化时不保存封面，重启后按需从文件加载。
    cover: "",
    duration: song.duration,
    fee: song.fee ?? 0,
    playable: song.playable ?? true,
    language: song.language ?? 0,
    hash: song.hash,
    _localPath: song._localPath,
  };
}

/// 后端 import_local_music / import_local_music_folder 返回的单首歌曲元信息
interface LocalSongInfoPayload {
  id: string;
  name: string;
  path: string;
  size: number;
  extension: string;
  title: string;
  artist: string;
  album: string;
  duration_ms: number;
  cover: string;
  cover_source: string;
}

/// 把后端返回的本地歌曲元信息合并进 localSongs 并持久化（供单文件/文件夹导入复用）
async function mergeLocalSongInfos(
  infos: LocalSongInfoPayload[],
  set: (partial: (state: unknown) => unknown) => void,
  get: () => { localSongs: Song[] }
): Promise<{ count: number; noCoverCount: number }> {
  if (infos.length === 0) return { count: 0, noCoverCount: 0 };

  const newSongs: Song[] = infos.map((info) => ({
    provider: "local",
    id: info.id,
    // 优先使用音频标签中的标题，回退到文件名
    name: info.title || info.name,
    artist: info.artist || "本地音乐",
    artists: info.artist ? [{ name: info.artist }] : [],
    album: info.album,
    // 内嵌封面（data URI 或空字符串）
    cover: info.cover || "",
    duration: info.duration_ms,
    fee: 0,
    playable: true,
    language: 0,
    // 本地歌曲专用：文件绝对路径，用于 convertFileSrc 播放
    // 复用 hash 字段存放绝对路径，避免修改 Song 结构
    hash: info.path,
    _localPath: info.path,
  }));

  set((state) => {
    const st = state as { localSongs: Song[] };
    // 用 Map 以 id 为键做去重合并，O(n) 而非 O(n²)，避免大列表导入时卡顿
    const merged = new Map<string, Song>();
    for (const song of st.localSongs) {
      merged.set(song.id, song);
    }
    for (const song of newSongs) {
      merged.set(song.id, song);
    }
    return { localSongs: Array.from(merged.values()) };
  });

  // 持久化完整 Song 对象，确保重启后 provider 等字段不丢失
  const s = await getStore();
  const finalList = get().localSongs.map(serializeLocalSong);
  await s.set("localSongs", finalList);
  await s.save();

  return { count: newSongs.length, noCoverCount: infos.filter((info) => !info.cover).length };
}

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


// ── 播放状态持久化：主窗口销毁(托盘)重建后恢复播放 UI ──
// 原理：Rust 播放引擎(rodio)不随窗口销毁，音乐继续；前端状态丢失后
// 从 localStorage 恢复 currentSong/队列，再通过 player_get_state 恢复进度。
const PLAYBACK_STATE_KEY = "nexbox.music.playbackState.v1";

interface PersistedPlayback {
  song: Song | null;
  queue: Song[];
  index: number;
  at: number;
  playing: boolean;
}

/// 当前实际交给 Rust 引擎的播放源（在线=原始音频 URL，本地=文件路径）。
/// 同步给引擎队列快照时，当前曲目用这个值标记已解析源。
let currentEngineSrc = "";

function savePlaybackState() {
  try {
    const s = useMusicStore.getState();
    const data: PersistedPlayback = {
      song: s.currentSong,
      queue: s.playQueue,
      index: s.currentIndex,
      at: s.currentTime,
      playing: s.isPlaying,
    };
    localStorage.setItem(PLAYBACK_STATE_KEY, JSON.stringify(data));
  } catch {
    // ignore quota errors
  }
}

function loadPlaybackState(): PersistedPlayback | null {
  try {
    const raw = localStorage.getItem(PLAYBACK_STATE_KEY);
    if (!raw) return null;
    return JSON.parse(raw) as PersistedPlayback;
  } catch {
    return null;
  }
}

/// 供窗口重建后调用：恢复 UI 状态（不重复播放，由 Rust 引擎继续）。
///
/// 关键点：
/// 1. 歌曲信息以「引擎实际播放的歌曲」为准（current_song），而不是 localStorage 快照，
///    避免最小化期间引擎已自动切歌/被 SMTC 控制切歌后，前端恢复出旧歌名。
/// 2. 进度一律用引擎的 position（暂停时引擎也知道真实位置），避免显示 0:00。
/// 3. 返回 { playing, hasSource } 供 initAndResume 判断：引擎还有源（播放中/暂停）
///    时不重新 playSong，避免重取 URL 变慢、进度归零。
async function restorePlaybackFromBackend(): Promise<{ playing: boolean; hasSource: boolean; engineSong: boolean }> {
  const result = { playing: false, hasSource: false, engineSong: false };
  try {
    const saved = loadPlaybackState();
    const state = await invoke<{
      is_playing: boolean;
      position: number;
      duration: number;
      current_src: string;
      current_song: Song | null;
    }>("player_get_state");
    const hasSource = !!state.current_src;
    result.hasSource = hasSource;
    const playing = !!state.is_playing && hasSource;
    result.playing = playing;
    // 引擎是否记录过“本会话播放过歌曲”（用于区分冷启动与隐藏期间歌曲播完）
    result.engineSong = !!state.current_song;

    // 引擎真实歌曲优先；引擎空闲时回退到本地快照
    const engineSong = state.current_song as Song | null;
    const song = engineSong ?? saved?.song ?? null;
    if (song) {
      const queue = saved?.queue ?? [];
      const idx = queue.findIndex((s) => s.id === song.id);
      useMusicStore.setState({
        currentSong: song,
        playQueue: queue,
        currentIndex: idx >= 0 ? idx : (saved?.index ?? 0),
        // 引擎有源时用引擎位置（播放中/暂停都准确）；空闲时用快照时间
        currentTime: hasSource ? (state.position ?? saved?.at ?? 0) : (saved?.at ?? 0),
        duration: state.duration ?? 0,
        isPlaying: playing,
      });
      // 同步 RustAudio 镜像，暂停状态下没有 player-tick 也能显示正确进度
      const audio = useMusicStore.getState().audioRef;
      audio?.syncState?.(
        hasSource ? (state.position ?? saved?.at ?? 0) : (saved?.at ?? 0),
        state.duration ?? 0,
        playing
      );
      // SMTC：窗口重建后重新推送元数据与封面（会话常驻，需恢复展示）。
      // 暂停状态也重推，避免引擎侧切歌后飞控面板停留在旧曲的元数据/封面
      if (hasSource) {
        if (playing) useMusicStore.getState().loadLyricsForSong(song);
        invoke("smtc_update_metadata", {
          title: song.name || "未知歌曲",
          artist: song.artist || "未知歌手",
          album: song.album || "",
          cover: song.cover || "",
        }).catch(() => {});
      }
    }
  } catch {
    // ignore
  }
  return result;
}

/// 把 Song 转换为引擎队列快照条目（player_set_queue 的参数）
function buildQueueEntry(song: Song, currentSrc: string) {
  const isLocal = song.provider === "local";
  const isCurrent = song.id === useMusicStore.getState().currentSong?.id;
  return {
    id: song.id || "",
    name: song.name || "",
    artist: song.artist || "",
    album: song.album || "",
    // 封面随队列快照同步给引擎：主窗口销毁后引擎侧切歌时能恢复/更新 SMTC 封面
    cover: song.cover || "",
    provider: song.provider || "",
    kind: isLocal ? "local" : "url",
    // 仅本地歌曲与当前歌曲带已解析的 src；在线队列其余曲目由引擎按 provider 字段在线解析
    src: isLocal ? (song._localPath || song.hash || "") : isCurrent ? currentSrc : "",
    duration: song.duration ? song.duration / 1000 : 0,
    quality: useMusicStore.getState().playbackQuality || "standard",
    hash: song.hash || "",
    album_id: song.album_id || "",
    album_audio_id: song.album_audio_id || "",
    hq_hash: song.hq_hash || "",
    sq_hash: song.sq_hash || "",
    res_hash: song.res_hash || "",
    mid: song.mid || "",
    media_mid: song.media_mid || "",
  };
}

/// 把当前播放队列快照同步给 Rust 引擎（主窗口销毁后上下曲/自动续播依赖它）
function syncEngineQueue() {
  const st = useMusicStore.getState();
  if (st.playQueue.length === 0) return;
  const entries = st.playQueue.map((s) => buildQueueEntry(s, currentEngineSrc));
  invoke("player_set_queue", { queue: entries }).catch(() => {});
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
      // 队列追加后同步引擎快照（最小化期间上下曲/自动续播能看到新曲目）
      syncEngineQueue();
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
  heartbeatQueue: [],
  heartbeatLoading: false,
  heartbeatPlayedIds: new Set(),

  localSongs: [],
  importingLocal: false,
  importProgress: { done: 0, total: 0 },

  loginInfo: null,
  loginInfos: { netease: null, kugou: null, qqmusic: null },
  playbackSource: "netease",

  searchResults: [],
  userPlaylists: [],
  userPlaylistsError: "",
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
  dailyRecommendPlaylists: [],

  artistSearchResults: [],
  artistSongs: [],
  selectedArtist: null,
  searchingArtists: false,
  loadingArtistSongs: false,
  artistDetail: null,
  artistAlbums: [],
  artistMvs: [],
  albumDetailSongs: [],
  albumDetailMeta: null,
  loadingArtistDetail: false,
  loadingArtistAlbums: false,
  loadingArtistMvs: false,
  loadingAlbumDetail: false,

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

  // 评论系统
  currentComments: null,
  loadingComments: false,
  sendingComment: false,
  commentError: "",

  desktopLyricsVisible: false,
  desktopLyricsFontSize: 36,
  desktopLyricsHighlightColor: "#FFD700",
  desktopLyricsBaseColor: "rgba(255,255,255,0.35)",
  desktopLyricsLineCount: 2,
  desktopLyricsLocked: false,
  desktopLyricsShowTranslation: true,
  desktopLyricsHideUnlockBtn: false,

  searching: false,
  loadingPlaylists: false,
  loadingLeftTracks: false,
  loadingRightTracks: false,
  loadingLyrics: false,

  audioRef: null,

  // ── 本地导入歌曲 Actions ──
  loadLocalSongs: async () => {
    try {
      const s = await getStore();
      const stored = await s.get<Song[]>("localSongs");
      const list: Song[] = Array.isArray(stored) ? stored : [];
      set({ localSongs: list });
      // 持久化时封面已置空（避免几十 MB JSON 写入崩溃），这里按需懒加载封面：
      // 分批（每批 50 首）调用后端读取内嵌封面，只填充缺失的，避免一次性全部解析
      const missing = list.filter((song) => song.provider === "local" && !song.cover && song._localPath);
      if (missing.length > 0) {
        (async () => {
          const filled = new Map<string, string>();
          for (let i = 0; i < missing.length; i += 50) {
            const batch = missing.slice(i, i + 50);
            await Promise.all(batch.map(async (song) => {
              try {
                const cover = await invoke<string>("get_local_song_cover", { path: song._localPath });
                if (cover) filled.set(song.id, cover);
              } catch { /* 单个失败忽略 */ }
            }));
            if (filled.size > 0) {
              // 每批更新一次 state（不可变更新，React 才能感知）
              set((state) => {
                const st = state as { localSongs: Song[] };
                return {
                  localSongs: st.localSongs.map((s) =>
                    filled.has(s.id) ? { ...s, cover: filled.get(s.id)! } : s
                  ),
                };
              });
            }
          }
        })();
      }
    } catch {
      set({ localSongs: [] });
    }
  },

  importLocalSongs: async (paths) => {
    set({ importingLocal: true });
    try {
      const infos = await invoke<LocalSongInfoPayload[]>("import_local_music", { paths });
      return await mergeLocalSongInfos(infos, set, get);
    } catch (e) {
      console.error("Import local songs failed:", e);
      return { count: 0, noCoverCount: 0 };
    } finally {
      set({ importingLocal: false });
    }
  },

  importLocalFolder: async (folder) => {
    set({ importingLocal: true, importProgress: { done: 0, total: 0 } });
    let unlistenBatch: (() => void) | null = null;
    let unlistenDone: (() => void) | null = null;
    try {
      // 后端分批解析，每批通过 local-import-batch 事件推送（避免几十 MB payload 走 IPC 崩溃）
      unlistenBatch = await listen<LocalSongInfoPayload[]>("local-import-batch", (e) => {
        const infos = e.payload || [];
        if (infos.length === 0) return;
        // 增量合并（不立刻持久化，全部完成后统一保存）
        const newSongs: Song[] = infos.map((info) => ({
          provider: "local",
          id: info.id,
          name: info.title || info.name,
          artist: info.artist || "本地音乐",
          artists: info.artist ? [{ name: info.artist }] : [],
          album: info.album,
          cover: info.cover || "",
          duration: info.duration_ms,
          fee: 0,
          playable: true,
          language: 0,
          hash: info.path,
          _localPath: info.path,
        }));
        set((state) => {
          const st = state as { localSongs: Song[]; importProgress: { done: number; total: number } };
          const merged = new Map<string, Song>();
          for (const song of st.localSongs) merged.set(song.id, song);
          for (const song of newSongs) merged.set(song.id, song);
          return {
            localSongs: Array.from(merged.values()),
            importProgress: { done: st.importProgress.done + infos.length, total: st.importProgress.total },
          };
        });
      });
      unlistenDone = await listen<number>("local-import-done", (e) => {
        const total = typeof e.payload === "number" ? e.payload : 0;
        set((state) => {
          const st = state as { importProgress: { done: number; total: number } };
          return { importProgress: { done: total, total } };
        });
      });

      // 启动导入（后端完成后返回总文件数；期间靠事件增量推送）
      const total = await invoke<number>("import_local_music_folder", { folder });
      set((state) => {
        const st = state as { importProgress: { done: number; total: number } };
        return { importProgress: { done: total, total } };
      });

      // 全部批次收齐后统一持久化（cover 已被 serializeLocalSong 置空，JSON 很小）
      const s = await getStore();
      const finalList = get().localSongs.map(serializeLocalSong);
      await s.set("localSongs", finalList);
      await s.save();

      return { count: get().localSongs.length, noCoverCount: 0 };
    } catch (e) {
      console.error("Import local music folder failed:", e);
      return { count: 0, noCoverCount: 0 };
    } finally {
      if (unlistenBatch) { unlistenBatch(); unlistenBatch = null; }
      if (unlistenDone) { unlistenDone(); unlistenDone = null; }
      set({ importingLocal: false });
    }
  },

  setImportingLocal: (importing) => set({ importingLocal: importing }),

  removeLocalSong: async (id) => {
    set((state) => ({ localSongs: state.localSongs.filter((s) => s.id !== id) }));
    try {
      const s = await getStore();
      const list = get().localSongs.map(serializeLocalSong);
      await s.set("localSongs", list);
      await s.save();
    } catch (e) {
      console.error("Remove local song persist failed:", e);
    }
  },

  clearLocalSongs: async () => {
    set({ localSongs: [] });
    try {
      const s = await getStore();
      await s.set("localSongs", []);
      await s.save();
    } catch (e) {
      console.error("Clear local songs persist failed:", e);
    }
  },

  init: async () => {
    // 加载本地导入的歌曲（不阻塞其余初始化）
    get().loadLocalSongs();

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
      const dlShowTranslation = await store.get<boolean>("desktopLyricsShowTranslation");
      const dlHideUnlockBtn = await store.get<boolean>("desktopLyricsHideUnlockBtn");
      if (vol != null) set({ volume: vol, prevVolume: vol > 0 ? vol : 0.7 });
      if (mode) set({ playMode: mode });
      if (quality) set({ playbackQuality: quality });
      if (fontSize != null) set({ lyricsFontSize: fontSize });
      const expStyle = await store.get<string>("expandedStyle");
      if (highlightColor) set({ lyricsHighlightColor: highlightColor });
      // 恢复播放器样式（glass/modern 都需还原，否则切到 glass 后重启会退回默认 modern）
      if (expStyle === "modern" || expStyle === "glass") set({ expandedStyle: expStyle });
      const dynamic = await store.get<boolean>("dynamicEnabled");
      if (dynamic) set({ dynamicEnabled: true });
      const filmEffect = await store.get<boolean>("coverFilmEffect");
      if (filmEffect) set({ coverFilmEffect: true });
      if (dlFontSize != null) set({ desktopLyricsFontSize: dlFontSize });
      if (dlHighlightColor) set({ desktopLyricsHighlightColor: dlHighlightColor });
      if (dlBaseColor) set({ desktopLyricsBaseColor: dlBaseColor });
      if (dlLineCount) set({ desktopLyricsLineCount: dlLineCount });
      if (dlShowTranslation != null) set({ desktopLyricsShowTranslation: dlShowTranslation });
      if (dlHideUnlockBtn != null) set({ desktopLyricsHideUnlockBtn: dlHideUnlockBtn });
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

    // 恢复上次播放状态（主窗口销毁重建后：音乐仍在 Rust 引擎播放，恢复 UI 展示）
    const restore = await restorePlaybackFromBackend();
    // 引擎完全空闲（无播放源）但保存状态为“正在播放”：
    // - 引擎记录过本会话歌曲（隐藏期间播完）→ 自动续播队列，保持后台音乐继续的预期
    // - 冷启动（引擎无记录）→ 恢复上次播放的歌曲
    if (!restore.hasSource) {
      const saved = loadPlaybackState();
      if (saved?.playing && saved.song) {
        if (restore.engineSong && saved.queue.length > 1) {
          get().nextTrack();
        } else {
          get().playSong(saved.song);
        }
      }
    }

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

    // 全局音乐控制热键事件监听（上一曲/下一曲/播放暂停）
    unlistenFns.push(
      await listen<{ action: string }>("music-hotkey", (event) => {
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
        }
      })
    );

    // 任务栏媒体控件（SMTC）按钮事件：play/pause/next/prev
    unlistenFns.push(
      await listen<{ action: string }>("smtc-control", (event) => {
        const { action } = event.payload;
        switch (action) {
          case "play":
            if (!get().isPlaying) get().togglePlay();
            break;
          case "pause":
            if (get().isPlaying) get().togglePlay();
            break;
          case "next":
            get().nextTrack();
            break;
          case "prev":
            get().prevTrack();
            break;
        }
      })
    );

    // 桌面歌词窗口就绪后请求数据
    // 解决窗口首次打开时 emit 早于 listen 注册的时序问题
    unlistenFns.push(
      await listen("desktop-lyrics:request-data", () => {
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

    // 监听解锁按钮显示/隐藏热键事件（Rust 端触发，切换 hideUnlockBtn 开关）
    unlistenFns.push(
      await listen("lyrics:toggle-hide-unlock-btn", () => {
        get().toggleDesktopLyricsHideUnlockBtn();
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

  loadArtistDetail: async (artistId) => {
    set({ loadingArtistDetail: true });
    try {
      const detail = await invoke<ArtistDetail>("music_artist_detail", { artistId });
      set({ artistDetail: detail });
    } catch (e) {
      console.error("Load artist detail failed:", e);
      set({ artistDetail: null });
    } finally {
      set({ loadingArtistDetail: false });
    }
  },

  loadArtistAlbums: async (artistId, offset = 0) => {
    set({ loadingArtistAlbums: true });
    try {
      const albums = await invoke<Album[]>("music_artist_albums", { artistId, limit: 50, offset });
      set((state) => ({ artistAlbums: offset === 0 ? albums : [...state.artistAlbums, ...albums] }));
    } catch (e) {
      console.error("Load artist albums failed:", e);
      set({ artistAlbums: [] });
    } finally {
      set({ loadingArtistAlbums: false });
    }
  },

  loadArtistMvs: async (artistId, offset = 0) => {
    set({ loadingArtistMvs: true });
    try {
      const mvs = await invoke<Mv[]>("music_artist_mvs", { artistId, limit: 50, offset });
      set((state) => ({ artistMvs: offset === 0 ? mvs : [...state.artistMvs, ...mvs] }));
    } catch (e) {
      console.error("Load artist mvs failed:", e);
      set({ artistMvs: [] });
    } finally {
      set({ loadingArtistMvs: false });
    }
  },

  loadAlbumDetail: async (albumId) => {
    set({ loadingAlbumDetail: true });
    try {
      const [meta, songs] = await invoke<[Album, Song[]]>("music_album_detail", { albumId });
      set({ albumDetailMeta: meta, albumDetailSongs: songs });
    } catch (e) {
      console.error("Load album detail failed:", e);
      set({ albumDetailMeta: null, albumDetailSongs: [] });
    } finally {
      set({ loadingAlbumDetail: false });
    }
  },

  clearArtistState: () => {
    set({
      artistSearchResults: [],
      artistSongs: [],
      selectedArtist: null,
      artistDetail: null,
      artistAlbums: [],
      artistMvs: [],
      albumDetailSongs: [],
      albumDetailMeta: null,
    });
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

    // 参考 Mineradio: 在 URL 获取之前就 dispatch 歌词加载（与 URL 获取并行）
    // 不立即清空 currentLyrics，保留旧歌词避免闪烁，新歌词加载完成后自动替换
    // 用户手动点击时重置跳过计数
    if (!isAutoSkipping) {
      unplayableSkipCount = 0;
    }
    set({ currentSong: song, currentTime: 0, duration: 0, isPlaying: false });
    savePlaybackState();
    // 引擎同步：当前歌曲完整信息（窗口重建后恢复 UI 用，保证与引擎实际播放一致）
    invoke("player_set_now_playing", { song }).catch(() => {});
    get().loadLyricsForSong(song);
    // SMTC：更新任务栏媒体控件元数据（标题/歌手/专辑/封面）
    invoke("smtc_update_metadata", {
      title: song.name || "未知歌曲",
      artist: song.artist || "未知歌手",
      album: song.album || "",
      cover: song.cover || "",
    }).catch(() => {});
    // 心动模式：仅对"我喜欢"歌单生效。
    // 用户手动播放（传入 queue）且队列非"我喜欢"歌单时自动降级为随机播放；
    // 心动模式自动续播（不传 queue）不触发降级，保证相似歌曲连续播放
    if (get().playMode === "heartbeat") {
      if (queue && queue.length > 0 && !queue.every((s) => get().likedSongIds.has(s.id))) {
        // 非"我喜欢"歌单：降级为随机播放（保留已播放记录，重新回到心动时不重复）
        set({ playMode: "shuffle", heartbeatQueue: [] });
        getStore().then((s) => s.set("playMode", "shuffle").then(() => s.save()));
        if (get().desktopLyricsVisible) {
          emit("desktop-lyrics:state", { isPlaying: get().isPlaying, playMode: "shuffle", volume: get().volume });
        }
      } else if (song.provider === "netease") {
        // 记录已播放，用于去重（带上限，防止长期挂机集合无限增长）
        set((st) => {
          const next = new Set(st.heartbeatPlayedIds);
          next.add(song.id);
          if (next.size > 2000) {
            return { heartbeatPlayedIds: new Set([song.id]) };
          }
          return { heartbeatPlayedIds: next };
        });
        // 以当前播放歌曲为基准预拉相似歌曲，保证连续播放
        get().loadHeartbeatSongs(song);
      }
    }

    try {
      // ── 本地歌曲：直接播放本地文件，无需获取网络 URL ──
      if (song.provider === "local") {
        const localPath = song._localPath || song.hash;
        if (!localPath) {
          set({ isPlaying: false });
          return;
        }
        if (mySeq !== playSongSeq) return;
        // RustAudio 适配器：直接传本地路径（不再 convertFileSrc，后端 rodio 直接读文件）
        currentEngineSrc = localPath;
        audio.src = localPath;
        audio.volume = state.volume;
        syncEngineQueue();
        try {
          await audio.play();
          if (mySeq !== playSongSeq) return;
          set({ isPlaying: true, currentQuality: "本地", currentBitrate: 0 });
          updateMediaSession(song, true);
        } catch (err) {
          if (mySeq !== playSongSeq) return;
          console.error("Play local song failed:", err);
          set({
            isPlaying: false,
            musicToast: { type: "warning", message: "无法播放该本地音频文件，请检查格式是否受支持" },
          });
        }
        return;
      }

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

        // 检查是否因版权/会员限制无法播放，自动跳过
        if (unplayableSkipCount < 10) {
          unplayableSkipCount++;
          isAutoSkipping = true;
          // 判断是否版权相关
          const msg = result.message || "";
          const isCopyright = msg.includes("版权") || msg.includes("会员") || msg.includes("copyright") || result.reason === "QQ_URL_UNAVAILABLE";
          set({
            musicToast: {
              type: "warning",
              message: isCopyright ? "无版权" : (result.message || "无法播放"),
            },
          });
          // 延迟跳转，让 toast 可见
          setTimeout(() => {
            if (get().playQueue.length > 1) {
              get().nextTrack();
            }
          }, 800);
        } else {
          // 连续跳过太多，停下
          isAutoSkipping = false;
          unplayableSkipCount = 0;
          set({
            musicToast: {
              type: "warning",
              message: "当前队列中多首歌曲无法播放，已停止自动切换",
            },
          });
        }
        return;
      }

      const audioUrl = await getProxyAudioUrl(result.url, state.proxyPort);

      if (mySeq !== playSongSeq) return;

      currentEngineSrc = result.url;
      audio.src = audioUrl;
      audio.volume = state.volume;
      syncEngineQueue();
      await audio.play();

      if (mySeq !== playSongSeq) return;

      set({ isPlaying: true, proxyPort: state.proxyPort || get().proxyPort, currentQuality: result.quality, currentBitrate: result.br });
      // SMTC：更新系统媒体控件（标题/封面/播放状态）
      updateMediaSession(song, true);
      // 推送桌面歌词状态
      // 歌词数据已由 loadLyricsForSong 并行加载完成后自动 emit，此处不再重复 emitDesktopLyricsData
      // 避免 loadLyricsForSong 未完成时推送旧歌词
      if (get().desktopLyricsVisible) {
        emit("desktop-lyrics:state", { isPlaying: true, playMode: get().playMode, volume: get().volume });
      }
      // 歌词已在 playSong 开始时并行加载，此处不再重复调用
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
      setMediaPlaybackState(false);
      if (get().desktopLyricsVisible) {
        emit("desktop-lyrics:state", { isPlaying: false, playMode: get().playMode, volume: get().volume });
      }
    } else {
      try {
        await audioRef.play();
        set({ isPlaying: true });
        const cur = get().currentSong;
        if (cur) updateMediaSession(cur, true);
        else setMediaPlaybackState(true);
        if (get().desktopLyricsVisible) {
          emit("desktop-lyrics:state", { isPlaying: true, playMode: get().playMode, volume: get().volume });
        }
      } catch {
        // 播放失败，URL 可能已过期，尝试重新获取
        const state = get();
        const song = state.currentSong;
        if (song) {
          const savedTime = audioRef.currentTime;
          // 本地歌曲：直接重设 src 重试
          if (song.provider === "local") {
            const localPath = song._localPath || song.hash;
            if (localPath) {
              audioRef.src = localPath;
              audioRef.currentTime = savedTime;
              try {
                await audioRef.play();
                set({ isPlaying: true });
                if (get().desktopLyricsVisible) {
                  emit("desktop-lyrics:state", { isPlaying: true, playMode: get().playMode, volume: get().volume });
                }
                return;
              } catch {}
            }
            set({ isPlaying: false });
            if (get().desktopLyricsVisible) {
              emit("desktop-lyrics:state", { isPlaying: false, playMode: get().playMode, volume: get().volume });
            }
            return;
          }
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
    const { playQueue, currentIndex, playMode, audioRef, heartbeatQueue } = get();
    if (playQueue.length === 0) return;

    // 单曲循环：重新播放当前歌曲
    if (playMode === "one") {
      if (audioRef) {
        audioRef.currentTime = 0;
        audioRef.play().catch(() => {});
      }
      return;
    }

    // 心动模式：从相似歌曲队列取下一首，队列空则回退随机（队列会由 playSong 自动补充）
    if (playMode === "heartbeat") {
      let heartbeatNext: Song | null = null;
      if (heartbeatQueue.length > 0) {
        const [first, ...rest] = heartbeatQueue;
        heartbeatNext = first;
        set({ heartbeatQueue: rest });
      } else {
        heartbeatNext = playQueue[Math.floor(Math.random() * playQueue.length)] || null;
      }
      if (heartbeatNext) {
        get().playSong(heartbeatNext);
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

  // 心动模式：根据基准歌曲拉取相似歌曲并追加到心动队列（自动去重已播放/已排队歌曲）
  loadHeartbeatSongs: async (baseSong) => {
    if (!baseSong || baseSong.provider !== "netease") return;
    const state = get();
    // 一次只允许一个拉取任务，避免快速切歌时并发请求造成队列错乱
    if (state.heartbeatLoading) return;
    set({ heartbeatLoading: true });
    try {
      const songs = await invoke<Song[]>("music_simi_song", { id: baseSong.id, limit: 20 });
      // 去重：排除当前播放队列、心动队列、已播放过的相似歌曲
      const playedIds = new Set<string>([...state.playQueue.map((s) => s.id), ...state.heartbeatPlayedIds]);
      const queueIds = new Set(state.heartbeatQueue.map((s) => s.id));
      const currentId = get().currentSong?.id;
      const fresh = songs.filter(
        (s) => s.id && !playedIds.has(s.id) && !queueIds.has(s.id) && s.id !== currentId
      );
      if (fresh.length > 0) {
        set((st) => ({ heartbeatQueue: [...st.heartbeatQueue, ...fresh] }));
      }
    } catch (e) {
      console.error("[Music] loadHeartbeatSongs failed:", e);
    } finally {
      set({ heartbeatLoading: false });
    }
  },

  togglePlayMode: () => {
    const modes: PlayMode[] = ["list", "heartbeat", "shuffle", "one"];
    const current = modes.indexOf(get().playMode);
    const next = modes[(current + 1) % modes.length];
    // 心动模式：仅"我喜欢"歌单（网易云）可用，否则自动切换为随机播放
    if (next === "heartbeat") {
      const st = get();
      const usable = st.playbackSource === "netease"
        && st.playQueue.length > 0
        && st.playQueue.every((s) => st.likedSongIds.has(s.id));
      if (!usable) {
        set({ playMode: "shuffle", heartbeatQueue: [] });
        getStore().then((s) => s.set("playMode", "shuffle").then(() => s.save()));
        if (get().desktopLyricsVisible) {
          emit("desktop-lyrics:state", { isPlaying: get().isPlaying, playMode: "shuffle", volume: get().volume });
        }
        return;
      }
    }
    // 离开心动模式时清空相似歌曲队列，防止残留旧数据
    set({ playMode: next, heartbeatQueue: next === "heartbeat" ? get().heartbeatQueue : [] });
    getStore().then((s) => s.set("playMode", next).then(() => s.save()));
    // 进入心动模式：以当前歌曲为基准预拉相似歌曲
    if (next === "heartbeat") {
      get().loadHeartbeatSongs(get().currentSong);
    }
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
            showTranslation: get().desktopLyricsShowTranslation,
            hideUnlockBtn: get().desktopLyricsHideUnlockBtn,
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

  setDesktopLyricsShowTranslation: async (show) => {
    set({ desktopLyricsShowTranslation: show });
    getStore().then((s) => s.set("desktopLyricsShowTranslation", show).then(() => s.save()));
    get().emitDesktopLyricsSettings();
  },

  setDesktopLyricsHideUnlockBtn: async (hide) => {
    set({ desktopLyricsHideUnlockBtn: hide });
    getStore().then((s) => s.set("desktopLyricsHideUnlockBtn", hide).then(() => s.save()));
    get().emitDesktopLyricsSettings();
  },

  toggleDesktopLyricsHideUnlockBtn: async () => {
    await get().setDesktopLyricsHideUnlockBtn(!get().desktopLyricsHideUnlockBtn);
  },

  emitDesktopLyricsSettings: () => {
    const s = get();
    emit("desktop-lyrics:settings", {
      fontSize: s.desktopLyricsFontSize,
      highlightColor: s.desktopLyricsHighlightColor,
      baseColor: s.desktopLyricsBaseColor,
      lineCount: s.desktopLyricsLineCount,
      isLocked: s.desktopLyricsLocked,
      showTranslation: s.desktopLyricsShowTranslation,
      hideUnlockBtn: s.desktopLyricsHideUnlockBtn,
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
    // 心动模式仅支持网易云：切换到其他平台时自动降级为随机播放
    if (provider !== "netease" && get().playMode === "heartbeat") {
      set({ playMode: "shuffle", heartbeatQueue: [] });
      getStore().then((s) => s.set("playMode", "shuffle").then(() => s.save()));
    }
    // 更新 loginInfo 为当前平台的登录状态
    const info = get().loginInfos[provider];
    // 切换平台时立即清空榜单/推荐，避免旧平台数据残留闪烁
    set({ loginInfo: info, userPlaylists: [], userPlaylistsError: "", officialCharts: [], recommendations: [], recommendSongs: [], dailyRecommendPlaylists: [] });
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
    set({ loadingPlaylists: true, userPlaylistsError: "" });
    try {
      const cmd = provider === "kugou" ? "kugou_user_playlists"
        : provider === "qqmusic" ? "qq_user_playlists"
        : "music_user_playlist";
      const playlists = await invoke<Playlist[]>(cmd);
      set({ userPlaylists: playlists, userPlaylistsError: "" });
    } catch (e) {
      const msg = typeof e === "string" && e ? e : "歌单获取失败，登录可能已过期";
      set({ userPlaylists: [], userPlaylistsError: msg });
      // 酷狗/QQ 登录态失效时刷新登录状态，让界面提示重新登录
      if (provider === "kugou" || provider === "qqmusic") {
        get().loginStatusFor(provider);
      }
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

  // QQ 榜单歌曲加载 (榜单不是普通歌单, 走 qq_rank_songs; meta 由前端提供)
  loadRightRankTracks: async (rankId) => {
    set({ loadingRightTracks: true, rightPlaylistTracks: [] });
    try {
      const songs = await invoke<Song[]>("qq_rank_songs", { rankId, limit: 99999 });
      set({ rightPlaylistTracks: songs });
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
    // 本地导入歌曲：读取同目录的同名 .lrc 歌词文件
    if (song.provider === "local") {
      set({ loadingLyrics: true });
      try {
        const localPath = song._localPath || song.hash || "";
        if (localPath) {
          const lrcText = await invoke<string>("get_local_lyric", { path: localPath });
          if (lrcText && lrcText.trim()) {
            set({ currentLyrics: { lyric: lrcText } });
            if (get().desktopLyricsVisible) {
              get().emitDesktopLyricsData();
            }
            return;
          }
        }
        set({ currentLyrics: null });
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
      return;
    }
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

  loadComments: async (songId, page = 1) => {
    set({ loadingComments: true, commentError: "" });
    try {
      const result = await invoke<CommentPage>("music_song_comments", { id: songId, page, pageSize: 20 });
      // 分页时追加到已有列表（跳过已加载的 id），第一页直接替换
      set((state) => {
        if (page === 1 || !state.currentComments) {
          return { currentComments: result, loadingComments: false };
        }
        const seen = new Set(state.currentComments.comments.map((c) => c.comment_id));
        const merged = [...state.currentComments.comments, ...result.comments.filter((c) => !seen.has(c.comment_id))];
        return {
          currentComments: { ...result, comments: merged, hot_comments: state.currentComments.hot_comments },
          loadingComments: false,
        };
      });
    } catch (e) {
      console.error("[Music] loadComments failed:", e);
      set({ loadingComments: false, commentError: String(e) || "加载评论失败" });
    }
  },

  sendComment: async (songId, content) => {
    const trimmed = content.trim();
    if (!trimmed) return false;
    set({ sendingComment: true });
    try {
      await invoke("music_send_comment", { id: songId, content: trimmed });
      // 发送成功后刷新第一页，让新评论出现在列表
      await get().loadComments(songId, 1);
      return true;
    } catch (e) {
      console.error("[Music] sendComment failed:", e);
      return false;
    } finally {
      set({ sendingComment: false });
    }
  },

  clearComments: () => {
    set({ currentComments: null });
  },

  loadRecommendations: async () => {
    try {
      const provider = get().playbackSource;
      if (provider === "kugou" || provider === "qqmusic") {
        // 酷狗/QQ 不显示推荐歌单 (QQ 推荐歌单接口受限, 与酷狗一致只显示榜单)
        set({ recommendations: [], recommendSongs: [], dailyRecommendPlaylists: [] });
      } else {
        const [songs, dailyPlaylists] = await Promise.all([
          invoke<Song[]>("music_recommend_songs").catch(() => []),
          invoke<Playlist[]>("music_recommend_resource").catch(() => []),
        ]);
        set({ recommendations: [], recommendSongs: songs, dailyRecommendPlaylists: dailyPlaylists });
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
    // QQ 音乐: 使用预设榜单 (榜单列表接口已失效, 后端返回实测可用的 topid)
    if (get().playbackSource === "qqmusic") {
      try {
        const charts = await invoke<Playlist[]>("qq_rank_list").catch(() => []);
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
        dailyRecommendPlaylists: state.dailyRecommendPlaylists.map((pl) =>
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

// ── SMTC：注册系统媒体控制按键（Windows 任务栏媒体控件） ──
// 在模块加载时注册一次，通过 useMusicStore 调用现有播放控制
registerMediaActions({
  // 系统「播放」按钮：仅在暂停时恢复
  onPlay: () => {
    const st = useMusicStore.getState();
    if (!st.isPlaying) st.togglePlay();
  },
  // 系统「暂停」按钮：仅在播放中暂停
  onPause: () => {
    const st = useMusicStore.getState();
    if (st.isPlaying) st.togglePlay();
  },
  onNext: () => {
    useMusicStore.getState().nextTrack();
  },
  onPrev: () => {
    useMusicStore.getState().prevTrack();
  },
  onSeek: (time) => {
    useMusicStore.getState().seekTo(time);
  },
});
