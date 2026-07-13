/// 统一歌曲结构
export interface Song {
  provider: string;
  id: string;
  mid?: string;
  media_mid?: string;
  name: string;
  artist: string;
  artists: Artist[];
  album: string;
  cover: string;
  duration: number;
  fee: number;
  playable: boolean;
  language: number;
}

export interface Artist {
  id?: string;
  mid?: string;
  name: string;
  pic_url?: string;
  music_size?: number;
}

export interface Playlist {
  provider: string;
  id: string;
  name: string;
  cover: string;
  track_count: number;
  creator: string;
}

export interface SongUrlResult {
  url: string | null;
  playable: boolean;
  trial: boolean;
  level: string;
  quality: string;
  br: number;
  reason?: string;
  message?: string;
  fee?: number;
}

export interface LoginInfo {
  provider: string;
  logged_in: boolean;
  user_id: string;
  nickname: string;
  avatar: string;
  vip_type: number;
  vip_level: string;
  is_vip: boolean;
  is_svip: boolean;
}

export interface Lyrics {
  lyric: string;
  translation?: string;
  roma?: string;
  yrc?: string; // YRC 逐字歌词
}

/** 逐词数据 */
export interface LyricWord {
  text: string;
  t: number;     // 开始时间（秒）
  d: number;     // 持续时间（秒）
  c0: number;    // 在整行文本中的起始字符索引
  c1: number;    // 在整行文本中的结束字符索引
}

/** 卡拉OK歌词行 */
export interface KaraokeLine {
  time: number;          // 行开始时间（秒）
  duration: number;      // 行持续时间（秒）
  text: string;          // 整行文本
  translation?: string;  // 翻译
  words?: LyricWord[];   // 逐词数据（有 YRC 时存在）
  charCount: number;     // 字符数
  hasKaraoke: boolean;   // 是否有逐字数据
}

export interface QrCheckResult {
  code: number; // 801=等待扫码, 802=待确认, 803=成功, 800=过期
  message: string;
  cookie?: string;
  nickname?: string;
  avatar?: string;
}

export type PlayMode = "list" | "shuffle" | "one";

export type PlaybackQuality = "jymaster" | "hires" | "lossless" | "exhigh" | "standard";
