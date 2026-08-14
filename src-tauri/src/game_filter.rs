//! 游戏启动时自动应用滤镜模块
//!
//! 后台每 2.5 秒轮询一次系统进程，当检测到内置/自定义名单中的游戏进程运行时：
//! - 自动开启当前选中的滤镜（复用 `display_filter::apply_filter_to_display`）
//! 当所有名单内游戏进程退出时：
//! - 自动恢复默认显示（仅关闭由自动任务开启的滤镜，不误关用户手动开启的滤镜）
//!
//! 参考 `optimization.rs` 的 ACE 自动检测模式（generation 代次控制线程生命周期 +
//! `app.store` 持久化配置）。

use std::collections::HashSet;
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
    ("暗区突围无限", &["ABInfinite", "ABInfinite-Win64-Shipping"]),
    ("漫威争锋", &["Marvel-Win64-Shipping"]),
    ("潜行者 2", &["Stalker2-Win64-Shipping"]),
    ("绝地潜兵 2", &["helldivers2"]),
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
    ("无主之地 3", &["Borderlands3"]),
    ("无主之地 4", &["Borderlands4"]),
    ("毁灭战士：永恒", &["DOOMEternal"]),
    ("毁灭战士（2016）", &["DOOM"]),
    ("孤岛惊魂 6", &["farcry6"]),
    ("狙击精英 5", &["SniperElite5"]),
    ("深岩银河", &["DeepRockGalactic"]),
    ("行星边际 2", &["PlanetSide2"]),
    ("地铁：离去", &["MetroExodus"]),
    ("死亡循环", &["Deathloop"]),
    ("光环：士官长合集", &["MCC-Win64-Shipping"]),
    ("孤岛危机 3", &["Crysis3"]),
    ("生化危机 4 重制", &["re4"]),
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
    ("怪物猎人：荒野", &["MonsterHunterWilds"]),
    ("只狼：影逝二度", &["sekiro"]),
    ("对马岛之魂", &["GhostOfTsushima"]),
    ("匹诺曹的谎言", &["LiesOfP"]),
    ("堕落之主", &["LordsOfTheFallen"]),
    ("卧龙：苍天陨落", &["WoLong"]),
    ("龙之信条 2", &["Dragon's Dogma 2"]),
    ("最终幻想 7 重制版", &["FF7R"]),
    ("最终幻想 16", &["ff16"]),
    ("霍格沃茨之遗", &["HogwartsLegacy"]),
    ("原子之心", &["AtomicHeart"]),
    ("遗迹 2", &["Remnant2"]),
    ("仁王 2", &["nioh2"]),
    ("神界：原罪 2", &["DivinityOriginalSin2"]),
    ("极乐迪斯科", &["DiscoElysium"]),
    ("天国拯救 2", &["KingdomCome"]),
    ("如龙 8", &["Yakuza8"]),
    ("真三国无双：起源", &["DynastyWarriorsOrigins"]),
    ("艾尔登法环：夜王", &["EldenRingNightreign"]),
    // 动作 / 竞速 / 其他
    ("永劫无间", &["NarakaBladepoint"]),
    ("地平线 6", &["ForzaHorizon6"]),
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
    // 动作 / 冒险
    ("战神", &["GodOfWar"]),
    ("战神：诸神黄昏", &["GoWR"]),
    ("漫威蜘蛛侠", &["MarvelsSpiderMan"]),
    ("漫威蜘蛛侠 2", &["Spider-Man2"]),
    ("地平线：零之曙光", &["HorizonZeroDawn"]),
    ("地平线：西之绝境", &["HorizonForbiddenWest"]),
    ("最后生还者 1", &["TheLastOfUs"]),
    ("神秘海域：盗贼遗产", &["Uncharted4"]),
    ("古墓丽影：暗影", &["ShadowOfTheTombRaider"]),
    ("星球大战绝地：幸存者", &["StarWarsJediSurvivor"]),
    ("星球大战绝地：陨落的武士团", &["StarWarsJediFallenOrder"]),
    ("蝙蝠侠：阿卡姆骑士", &["BatmanArkhamKnight"]),
    ("消逝的光芒 2", &["DyingLight2"]),
    ("往日不再", &["Days Gone"]),
    ("死亡空间", &["DeadSpace"]),
    ("心灵杀手 2", &["AlanWake2"]),
    ("控制", &["Control"]),
    ("羞辱 2", &["Dishonored2"]),
    ("看门狗 2", &["WatchDogs2"]),
    ("生化危机：村庄", &["re8"]),
    ("双影奇境", &["SplitFiction"]),
    ("死亡搁浅", &["DeathStranding"]),
    ("刺客信条：影", &["ACShadows"]),
    // 策略 / 模拟
    ("文明 7", &["CivilizationVII"]),
    ("群星", &["Stellaris"]),
    ("十字军之王 3", &["CK3"]),
    ("维多利亚 3", &["Victoria3"]),
    ("钢铁雄心 4", &["HOI4"]),
    ("城市：天际线 2", &["Cities2"]),
    ("冰汽时代 2", &["Frostpunk2"]),
    ("战锤 40K：星际战士 2", &["Warhammer40KSpaceMarine2"]),
    ("微软飞行模拟 2024", &["FlightSimulator"]),
    ("极限竞速：Motorsport", &["ForzaMotorsport"]),
    ("F1 24", &["F1_24"]),
    ("模拟农场 25", &["FarmingSimulator25"]),
    ("双点医院", &["TwoPointHospital"]),
    ("帝国时代 2：决定版", &["AoE2"]),
    ("全面战争：战锤 2", &["warhammer2"]),
    // 生存 / 合作
    ("英灵神殿", &["valheim"]),
    ("森林之子", &["SonsOfTheForest"]),
    ("夜族崛起", &["VRising"]),
    ("盗贼之海", &["SeaOfThieves"]),
    ("火箭联盟", &["RocketLeague"]),
    ("黎明杀机", &["DeadByDaylight"]),
    ("第五人格", &["IdentityV"]),
    ("动物派对", &["PartyAnimals"]),
    ("在我们中间", &["AmongUs"]),
    ("方舟：生存进化", &["ShooterGame"]),
    ("无人深空", &["NMS"]),
    ("人类一败涂地", &["HumanFallFlat"]),
    // 独立 / Roguelike
    ("哈迪斯 2", &["Hades2"]),
    ("吸血鬼幸存者", &["VampireSurvivors"]),
    ("咩咩启示录", &["CultOfTheLamb"]),
    ("潜水员戴夫", &["DaveTheDiver"]),
    ("巴拉特罗", &["Balatro"]),
    ("动物井", &["AnimalWell"]),
    ("死亡细胞", &["DeadCells"]),
    ("空洞骑士", &["HollowKnight"]),
    ("灵魂面甲", &["Soulmask"]),
    // 国产 / 其他
    ("燕云十六声", &["yysls"]),
    ("无限暖暖", &["InfinityNikki"]),
    ("尘白禁区", &["Snowbreak"]),
    ("暖雪", &["WarmSnow"]),
    ("剑网 3", &["JX3"]),
    ("逆水寒", &["nsh"]),
    ("仙剑奇侠传 7", &["Pal7", "Pal7-Win64-Shipping"]),
    ("幻塔", &["TowerOfFantasy"]),
    ("荒野乱斗", &["BrawlStars"]),
    ("古剑奇谭 3", &["Gujian3"]),
    ("卡拉彼丘", &["Strinova"]),
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
    !running_game_pids(system).is_empty()
}

/// 返回当前正在运行的滤镜名单游戏进程 PID 集合（内置 + 自定义名单）
/// 供 game_mode 模块复用同一份名单来豁免游戏进程
pub(crate) fn running_game_pids(system: &System) -> HashSet<u32> {
    let games = merge_games();
    let mut pids = HashSet::new();
    if games.is_empty() {
        return pids;
    }
    for (_, process) in system.processes() {
        let name = process.name().to_string();
        if games.iter().any(|g| process_matches(&name, &g.process_names)) {
            pids.insert(process.pid().as_u32());
        }
    }
    pids
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
