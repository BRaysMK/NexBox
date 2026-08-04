//! 游戏启动时自动应用滤镜模块
//!
//! 后台每 2.5 秒轮询一次系统进程，当检测到内置/自定义名单中的游戏进程运行时：
//! - 自动开启当前选中的滤镜（复用 `display_filter::apply_filter_to_display`）
//! 当所有名单内游戏进程退出时：
//! - 自动恢复默认显示（仅关闭由自动任务开启的滤镜，不误关用户手动开启的滤镜）
//!
//! 参考 `optimization.rs` 的 ACE 自动检测模式（generation 代次控制线程生命周期 +
//! `app.store` 持久化配置）。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use sysinfo::System;
use tauri_plugin_store::StoreExt;

use crate::display_filter;

// ─── 内置热门游戏名单（与 docs/game-auto-filter.md 保持一致） ───
// 进程名不区分大小写、无需 .exe 后缀；命中任意一个进程即触发

pub const BUILTIN_GAMES: &[(&str, &[&str])] = &[
    // 射击 / FPS
    ("三角洲行动", &["DeltaForceClient-Win64-Shipping"]),
    ("无畏契约", &["VALORANT", "VALORANT-Win64-Shipping"]),
    ("CS2", &["cs2"]),
    ("CS:GO", &["csgo"]),
    ("APEX 英雄", &["r5apex"]),
    ("绝地求生 PUBG", &["TslGame"]),
    ("使命召唤：战区", &["cod", "ModernWarfare"]),
    ("守望先锋", &["Overwatch", "Overwatch2"]),
    ("堡垒之夜", &["FortniteClient-Win64-Shipping"]),
    ("彩虹六号：围攻", &["RainbowSix", "RainbowSix_BE"]),
    ("逃离塔科夫", &["EscapeFromTarkov"]),
    ("战地系列", &["bf1", "bfv", "bf2042"]),
    ("全境封锁", &["TheDivision", "TheDivision2"]),
    ("命运 2", &["destiny2"]),
    ("猎杀对决", &["HuntGame"]),
    ("星球大战：前线", &["starwarsbattlefrontii"]),
    ("光环：无限", &["HaloInfinite"]),
    ("泰坦陨落 2", &["Titanfall2"]),
    ("求生之路 2", &["left4dead2"]),
    ("地球防卫军 5", &["EDF5"]),
    // MOBA / 对战
    ("英雄联盟", &["LeagueClient", "League of Legends"]),
    ("DOTA 2", &["dota2"]),
    ("王者荣耀 PC 版", &["HonorOfKings"]),
    ("决战！平安京", &["OnmyojiArena"]),
    ("虚荣", &["Vainglory"]),
    // 开放世界 / RPG
    ("崩坏：星穹铁道", &["StarRail"]),
    ("原神", &["GenshinImpact", "YuanShen"]),
    ("绝区零", &["ZenlessZoneZero"]),
    ("鸣潮", &["WutheringWaves"]),
    ("黑神话：悟空", &["b1-Win64-Shipping"]),
    ("艾尔登法环", &["eldenring"]),
    ("赛博朋克 2077", &["Cyberpunk2077"]),
    ("GTA V", &["GTA5"]),
    ("荒野大镖客 2", &["RDR2"]),
    ("巫师 3", &["witcher3"]),
    ("博德之门 3", &["bg3", "bg3_dx11"]),
    ("上古卷轴 5", &["SkyrimSE", "TESV"]),
    ("辐射 4", &["Fallout4"]),
    ("星空", &["Starfield"]),
    ("刺客信条系列", &["ACOdyssey", "ACValhalla", "AC Syndicate"]),
    ("塞尔达（模拟器）", &["ryujinx", "yuzu"]),
    ("幻兽帕鲁", &["Palworld"]),
    ("流放之路", &["PathOfExile", "PathOfExile_x64"]),
    ("暗黑破坏神 4", &["Diablo IV"]),
    ("魔兽世界", &["Wow", "WowClassic"]),
    ("最终幻想 14", &["ffxiv_dx11"]),
    ("命运方舟", &["LostArk"]),
    ("星际战甲", &["Warframe"]),
    ("怪物猎人", &["MonsterHunterWorld", "MonsterHunterRise"]),
    ("只狼：影逝二度", &["sekiro"]),
    ("对马岛之魂", &["GhostOfTsushima"]),
    // 动作 / 竞速 / 其他
    ("永劫无间", &["NarakaBladepoint"]),
    ("地平线 5", &["ForzaHorizon5"]),
    ("地平线 4", &["ForzaHorizon4"]),
    ("尘埃拉力赛 2.0", &["dirt2"]),
    ("欧洲卡车模拟 2", &["eurotrucks2"]),
    ("双人成行", &["It Takes Two"]),
    ("胡闹厨房 2", &["Overcooked2"]),
    ("泰拉瑞亚", &["Terraria"]),
    ("我的世界", &["javaw", "java", "Minecraft.Windows"]),
    ("星露谷物语", &["StardewValley"]),
    ("缺氧", &["OxygenNotIncluded"]),
    ("环世界", &["RimWorld"]),
    ("城市：天际线", &["Cities"]),
    ("文明 6", &["CivilizationVI"]),
    ("全面战争：战锤 3", &["warhammer3"]),
    ("帝国时代 4", &["AgeOfEmpires4"]),
    ("三国：全面战争", &["ThreeKingdoms"]),
    ("糖豆人", &["FallGuys_client"]),
    ("模拟人生 4", &["TS4_x64"]),
    ("中国式家长", &["ChineseParents"]),
    ("太吾绘卷", &["Taiwu"]),
    ("鬼谷八荒", &["TaleOfImmortal"]),
    ("戴森球计划", &["DysonSphereProgram"]),
];

/// 轮询间隔（秒）
const POLL_INTERVAL_SECS: u64 = 2;

// ─── 全局状态 ───

/// 开关是否开启（内存态，供轮询线程与状态查询读取）
static ENABLED: AtomicBool = AtomicBool::new(false);
/// 代次：开关切换时 +1，通知旧轮询线程退出
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// 当前滤镜是否由自动任务开启（用于退出游戏时区分自动/手动）
static AUTO_FILTER_ON: AtomicBool = AtomicBool::new(false);
/// 自定义游戏名单内存缓存（None = 尚未从 store 加载）
static CUSTOM_GAMES: Mutex<Option<Vec<CustomGame>>> = Mutex::new(None);

// ─── 数据结构 ───

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CustomGame {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub process_names: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct GameFilterConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub custom_games: Vec<CustomGame>,
}

/// 返回给前端的单个游戏条目（内置 + 自定义合并）
#[derive(serde::Serialize, Clone)]
pub struct GameEntry {
    pub id: String,
    pub name: String,
    pub process_names: Vec<String>,
    /// 是否内置名单（内置不可删除）
    pub is_builtin: bool,
}

#[derive(serde::Serialize)]
pub struct GameFilterStatus {
    pub enabled: bool,
    pub games: Vec<GameEntry>,
}

// ─── 配置持久化 ───

async fn load_persisted_config(app: &tauri::AppHandle) -> GameFilterConfig {
    match app.store("game_filter.json") {
        Ok(store) => {
            if let Some(value) = store.get("config") {
                if let Ok(config) = serde_json::from_value::<GameFilterConfig>(value) {
                    return config;
                }
            }
        }
        Err(e) => {
            log::warn!("Failed to open game_filter store: {}", e);
        }
    }
    GameFilterConfig::default()
}

async fn save_persisted_config(app: &tauri::AppHandle, config: &GameFilterConfig) {
    match app.store("game_filter.json") {
        Ok(store) => {
            store.set("config", serde_json::to_value(config).unwrap());
            if let Err(e) = store.save() {
                log::error!("Failed to save game_filter config: {}", e);
            }
        }
        Err(e) => {
            log::error!("Failed to open game_filter store for saving: {}", e);
        }
    }
}

/// 合并内置 + 自定义名单，返回前端条目列表
fn merge_games() -> Vec<GameEntry> {
    let mut games: Vec<GameEntry> = BUILTIN_GAMES
        .iter()
        .map(|(name, procs)| GameEntry {
            id: format!("builtin_{}", name),
            name: (*name).to_string(),
            process_names: procs.iter().map(|s| (*s).to_string()).collect(),
            is_builtin: true,
        })
        .collect();

    if let Ok(lock) = CUSTOM_GAMES.lock() {
        if let Some(custom) = lock.as_ref() {
            games.extend(custom.iter().map(|g| GameEntry {
                id: g.id.clone(),
                name: g.name.clone(),
                process_names: g.process_names.clone(),
                is_builtin: false,
            }));
        }
    }
    games
}

// ─── 进程匹配 ───

/// 去除 .exe 后缀（大小写不敏感）
fn strip_exe_suffix(s: &str) -> &str {
    if s.len() >= 4 && s[s.len() - 4..].eq_ignore_ascii_case(".exe") {
        &s[..s.len() - 4]
    } else {
        s
    }
}

/// 进程名与名单条目匹配（大小写不敏感、兼容带/不带 .exe）
fn process_matches(process_name: &str, entry_names: &[String]) -> bool {
    let proc = strip_exe_suffix(process_name);
    entry_names
        .iter()
        .any(|n| strip_exe_suffix(n).eq_ignore_ascii_case(proc))
}

/// 检测是否有名单内游戏在运行（复用 System 实例，避免每次重建）
/// 供 game_win_key 模块复用同一份游戏名单
pub(crate) fn any_game_running(system: &System) -> bool {
    let games = merge_games();
    if games.is_empty() {
        return false;
    }
    for (_, process) in system.processes() {
        let name = process.name().to_string();
        if games.iter().any(|g| process_matches(&name, &g.process_names)) {
            return true;
        }
    }
    false
}

// ─── 自动应用 / 恢复滤镜 ───

/// 检测到游戏启动：自动开启当前选中的滤镜
fn auto_apply_filter() {
    let idx = display_filter::get_active_index();
    let was_active = display_filter::is_filter_active(idx);

    if was_active {
        // 用户已手动开启滤镜，自动任务不干预
        AUTO_FILTER_ON.store(false, Ordering::Relaxed);
        return;
    }

    display_filter::set_filter_active(idx, true);
    match display_filter::apply_filter_to_display(idx) {
        Ok(()) => {
            AUTO_FILTER_ON.store(true, Ordering::Relaxed);
            log::info!("游戏滤镜自动应用[{}]: 检测到游戏运行，已自动开启当前滤镜", idx);
        }
        Err(e) => {
            AUTO_FILTER_ON.store(false, Ordering::Relaxed);
            display_filter::set_filter_active(idx, false);
            log::error!("游戏滤镜自动应用[{}]: 应用滤镜失败: {}", idx, e);
        }
    }
}

/// 游戏退出：若滤镜由自动任务开启则恢复默认显示
fn auto_restore_filter() {
    if !AUTO_FILTER_ON.load(Ordering::Relaxed) {
        return;
    }
    let idx = display_filter::get_active_index();
    let still_on = display_filter::is_filter_active(idx);
    if !still_on {
        // 用户在游戏运行期间手动关闭了滤镜，重置标记不再干预
        AUTO_FILTER_ON.store(false, Ordering::Relaxed);
        return;
    }

    display_filter::set_filter_active(idx, false);
    if let Err(e) = display_filter::restore_display_default(idx) {
        log::error!("游戏滤镜自动恢复[{}]: 恢复默认显示失败: {}", idx, e);
    } else {
        log::info!("游戏滤镜自动恢复[{}]: 游戏已退出，已恢复默认显示", idx);
    }
    AUTO_FILTER_ON.store(false, Ordering::Relaxed);
}

// ─── 后台轮询线程 ───

fn game_filter_loop(generation: u64) {
    let mut system = System::new();
    let mut game_running = false;

    loop {
        if GENERATION.load(Ordering::Relaxed) != generation {
            break;
        }
        thread::sleep(Duration::from_secs(POLL_INTERVAL_SECS));
        if GENERATION.load(Ordering::Relaxed) != generation {
            break;
        }
        if !ENABLED.load(Ordering::Relaxed) {
            game_running = false;
            AUTO_FILTER_ON.store(false, Ordering::Relaxed);
            continue;
        }

        system.refresh_processes();
        let running = any_game_running(&system);

        if running && !game_running {
            // 游戏刚启动
            auto_apply_filter();
        } else if !running && game_running {
            // 游戏刚退出
            auto_restore_filter();
        }
        game_running = running;
    }
}

// ─── 初始化 / 启动 ───

/// 应用启动时调用：恢复持久化配置并启动轮询线程
pub async fn init(app: tauri::AppHandle) -> Result<(), String> {
    let config = load_persisted_config(&app).await;

    // 初始化内存缓存
    {
        let mut lock = CUSTOM_GAMES.lock().map_err(|e| e.to_string())?;
        *lock = Some(config.custom_games.clone());
    }

    ENABLED.store(config.enabled, Ordering::Relaxed);

    if config.enabled {
        let gen = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
        thread::spawn(move || {
            let _ = std::panic::catch_unwind(|| game_filter_loop(gen));
        });
        log::info!("游戏滤镜自动应用: 已根据持久化配置启动轮询");
    }
    Ok(())
}

// ─── Tauri 命令 ───

/// 获取开关状态 + 内置/自定义名单
#[tauri::command]
pub async fn get_game_filter_status(_app: tauri::AppHandle) -> Result<GameFilterStatus, String> {
    Ok(GameFilterStatus {
        enabled: ENABLED.load(Ordering::Relaxed),
        games: merge_games(),
    })
}

/// 开关切换：开启启动轮询线程，关闭停止（代次 +1 使旧线程退出）
#[tauri::command]
pub async fn set_game_filter_enabled(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let current = ENABLED.load(Ordering::Relaxed);
    if current == enabled {
        return Ok(());
    }

    let gen = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    ENABLED.store(enabled, Ordering::Relaxed);

    // 持久化
    let config = GameFilterConfig {
        enabled,
        custom_games: {
            let lock = CUSTOM_GAMES.lock().map_err(|e| e.to_string())?;
            lock.as_ref().cloned().unwrap_or_default()
        },
    };
    save_persisted_config(&app, &config).await;

    if enabled {
        thread::spawn(move || {
            let _ = std::panic::catch_unwind(|| game_filter_loop(gen));
        });
        log::info!("游戏滤镜自动应用: 已开启");
    } else {
        log::info!("游戏滤镜自动应用: 已关闭");
    }
    Ok(())
}

/// 添加自定义游戏
#[tauri::command]
pub async fn add_custom_game(
    app: tauri::AppHandle,
    name: String,
    process_names: Vec<String>,
) -> Result<GameFilterStatus, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("游戏名称不能为空".to_string());
    }
    let procs: Vec<String> = process_names
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if procs.is_empty() {
        return Err("至少填写一个进程名".to_string());
    }

    let mut custom = {
        let mut lock = CUSTOM_GAMES.lock().map_err(|e| e.to_string())?;
        let list = lock.get_or_insert_with(Vec::new);
        list.push(CustomGame {
            id: format!("custom_{}", chrono::Utc::now().timestamp_millis()),
            name: name.clone(),
            process_names: procs,
        });
        list.clone()
    };

    // 持久化
    let config = GameFilterConfig {
        enabled: ENABLED.load(Ordering::Relaxed),
        custom_games: std::mem::take(&mut custom),
    };
    save_persisted_config(&app, &config).await;
    log::info!("游戏滤镜自动应用: 已添加自定义游戏 {}", name);

    Ok(GameFilterStatus {
        enabled: ENABLED.load(Ordering::Relaxed),
        games: merge_games(),
    })
}

/// 删除自定义游戏
#[tauri::command]
pub async fn remove_custom_game(
    app: tauri::AppHandle,
    id: String,
) -> Result<GameFilterStatus, String> {
    let removed = {
        let mut lock = CUSTOM_GAMES.lock().map_err(|e| e.to_string())?;
        let list = lock.get_or_insert_with(Vec::new);
        let before = list.len();
        list.retain(|g| g.id != id);
        list.len() != before
    };
    if !removed {
        return Err("未找到要删除的自定义游戏".to_string());
    }

    let custom = {
        let lock = CUSTOM_GAMES.lock().map_err(|e| e.to_string())?;
        lock.as_ref().cloned().unwrap_or_default()
    };
    let config = GameFilterConfig {
        enabled: ENABLED.load(Ordering::Relaxed),
        custom_games: custom,
    };
    save_persisted_config(&app, &config).await;
    log::info!("游戏滤镜自动应用: 已删除自定义游戏 id={}", id);

    Ok(GameFilterStatus {
        enabled: ENABLED.load(Ordering::Relaxed),
        games: merge_games(),
    })
}
