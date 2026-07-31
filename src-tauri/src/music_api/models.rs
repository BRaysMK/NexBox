use serde::{Deserialize, Serialize};

/// 统一歌曲结构
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Song {
    pub provider: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_mid: Option<String>,
    pub name: String,
    pub artist: String,
    pub artists: Vec<Artist>,
    pub album: String,
    pub cover: String,
    pub duration: u64,
    pub fee: i32,
    pub playable: bool,
    #[serde(default)]
    pub language: i32,
    // === 酷狗扩展字段 ===
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub album_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub album_audio_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub hq_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sq_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub res_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Artist {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mid: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pic_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub music_size: Option<i64>,
}

/// 歌单
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Playlist {
    pub provider: String,
    pub id: String,
    pub name: String,
    pub cover: String,
    pub track_count: u32,
    pub creator: String,
    #[serde(default)]
    pub subscribed: bool,
}

/// 播放地址结果
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SongUrlResult {
    pub url: Option<String>,
    pub playable: bool,
    pub trial: bool,
    pub level: String,
    pub quality: String,
    pub br: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee: Option<i32>,
}

/// 登录信息
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LoginInfo {
    pub provider: String,
    pub logged_in: bool,
    pub user_id: String,
    pub nickname: String,
    pub avatar: String,
    pub vip_type: i32,
    pub vip_level: String,
    pub is_vip: bool,
    pub is_svip: bool,
}

/// 歌词
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Lyrics {
    pub lyric: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roma: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yrc: Option<String>,
}

/// 二维码检查结果
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QrCheckResult {
    /// 801=等待扫码, 802=待确认, 803=成功, 800=过期
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
}

/// 音质候选
#[derive(Debug, Clone)]
pub struct QualityCandidate {
    pub level: String,
    pub label: String,
}

pub fn netease_quality_candidates() -> Vec<QualityCandidate> {
    vec![
        QualityCandidate { level: "jymaster".into(), label: "超清母带".into() },
        QualityCandidate { level: "hires".into(),    label: "高清臻音".into() },
        QualityCandidate { level: "lossless".into(), label: "无损".into()    },
        QualityCandidate { level: "exhigh".into(),   label: "极高".into()    },
        QualityCandidate { level: "standard".into(), label: "标准".into()    },
    ]
}

pub fn normalize_quality(value: &str) -> String {
    let raw = value.to_lowercase();
    match raw.as_str() {
        "jymaster" | "master" | "studio" | "svip" => "jymaster".into(),
        "hires" | "hi-res" | "highres" | "zhenyin" | "spatial" => "hires".into(),
        "lossless" | "flac" | "sq" => "lossless".into(),
        "exhigh" | "high" | "320" | "320k" | "hq" => "exhigh".into(),
        "standard" | "normal" | "128" | "128k" | "std" => "standard".into(),
        _ => "hires".into(),
    }
}

pub fn quality_candidates_from(target: &str) -> Vec<QualityCandidate> {
    let normalized = normalize_quality(target);
    let all = netease_quality_candidates();
    let start = all.iter().position(|c| c.level == normalized).unwrap_or(0);
    all[start..].to_vec()
}

/// 酷狗音质候选
#[allow(dead_code)]
pub fn kugou_quality_candidates() -> Vec<QualityCandidate> {
    vec![
        QualityCandidate { level: "jymaster".into(), label: "超清母带".into() },
        QualityCandidate { level: "hires".into(),    label: "Hi-Res".into()    },
        QualityCandidate { level: "lossless".into(), label: "无损".into()    },
        QualityCandidate { level: "exhigh".into(),   label: "极高".into()    },
        QualityCandidate { level: "standard".into(), label: "标准".into()    },
    ]
}
