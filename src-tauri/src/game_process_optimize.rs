//! 游戏进程优化模块
//!
//! 以「游戏」为单位管理进程优化（优先级提升）与核心优化（CPU 核心分配），
//! 每个游戏可独立开启「进程自动优化」和「核心自动优化」（两个独立开关）：
//! - 后台每 5 秒轮询一次系统进程，检测到该游戏进程启动即自动应用该游戏已开启的
//!   进程优化 / 核心优化；进程全部退出后重置状态。
//! - 支持三种添加游戏方式：选择 .exe 文件 / 选择正在运行的进程 / 从滤镜游戏名单选择。
//! - 配置持久化到 `app.store("game_process_optimize.json")`。
//!
//! 参考 `optimization.rs` 的 ACE 自动检测模式（generation 代次控制线程生命周期 +
//! `app.store` 持久化配置）与 `game_filter.rs` 的名单匹配约定。

use std::collections::HashMap;
use std::panic;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sysinfo::{ProcessRefreshKind, System};
use tauri_plugin_store::StoreExt;
use winreg::enums::*;
use winreg::RegKey;

use crate::optimization;

const STORE_FILE: &str = "game_process_optimize.json";
/// 自动优化轮询间隔（秒）
const AUTO_POLL_INTERVAL_SECS: u64 = 5;

/// 内置默认游戏 ID（三角洲行动）
const DEFAULT_GAME_ID: &str = "delta-force";

// ─── 数据结构 ───

/// 进程优先级等级
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum PriorityLevel {
    #[default]
    Realtime,
    High,
    AboveNormal,
    Normal,
    BelowNormal,
    Idle,
}

/// 单个游戏的优化配置
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GameOptimizeConfig {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub process_names: Vec<String>,
    #[serde(default)]
    pub priority: PriorityLevel,
    /// 核心优化掩码；`None` 或 `Some(0)` = 运行时按「除 CPU0 外全部核心」计算
    #[serde(default)]
    pub affinity_mask: Option<u64>,
    /// 进程自动优化（启动游戏自动提升进程优先级，每游戏独立）
    #[serde(default)]
    pub auto_optimize_priority: bool,
    /// 核心自动优化（启动游戏自动分配 CPU 核心，每游戏独立）
    #[serde(default)]
    pub auto_optimize_affinity: bool,
    /// IFEO 优先级优化（通过注册表 IFEO PerfOptions 在进程启动时由系统强制应用优先级，持久且对受保护进程生效）
    #[serde(default)]
    pub auto_optimize_ifeo: bool,
    /// 兼容旧版单一开关 `auto_optimize`（仅反序列化迁移用，不参与序列化）
    #[serde(default, rename = "auto_optimize", skip_serializing)]
    pub legacy_auto_optimize: Option<bool>,
}

/// 手动应用优化后的结果（进程优化 / 核心优化）
#[derive(Serialize)]
pub struct GameOptimizeResult {
    pub success: bool,
    pub message: String,
    pub found_count: usize,
    pub modified_count: usize,
}

/// 单个游戏的自动优化运行状态
#[derive(Serialize, Clone, Default)]
pub struct GameAutoStatus {
    pub game_id: String,
    pub running: bool,
    pub optimized: bool,
    /// 进程优先级是否已自动应用
    pub priority_applied: bool,
    /// 核心分配是否已自动应用
    pub affinity_applied: bool,
    /// IFEO 优先级是否已通过注册表应用（持久）
    pub ifeo_applied: bool,
    pub last_apply: Option<String>,
}

/// 选择 .exe 文件的结果
#[derive(Serialize)]
pub struct GameExecutableInfo {
    pub path: String,
    pub process_name: String,
    pub game_name: String,
}

/// 运行中进程信息（去重后的进程名列表）
#[derive(Serialize)]
pub struct RunningProcessInfo {
    pub name: String,
    pub pid: u32,
    pub memory_mb: f64,
}

// ─── 全局状态 ───

/// 游戏配置内存缓存（None = 尚未加载）
static CONFIGS: Mutex<Option<Vec<GameOptimizeConfig>>> = Mutex::new(None);
/// 代次：开关/配置变化时 +1，通知旧自动优化线程退出
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// 自动优化线程是否存活
static AUTO_RUNNING: AtomicBool = AtomicBool::new(false);
/// 每游戏运行状态（自动优化线程写入，状态查询读取）
static GAME_STATUS: Mutex<Option<HashMap<String, GameAutoStatus>>> = Mutex::new(None);

// ─── 配置存取 ───

fn get_configs() -> Vec<GameOptimizeConfig> {
    CONFIGS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_default()
}

async fn load_persisted_configs(app: &tauri::AppHandle) -> Vec<GameOptimizeConfig> {
    let mut migrated = false;
    let configs: Vec<GameOptimizeConfig> = match app.store(STORE_FILE) {
        Ok(store) => {
            if let Some(value) = store.get("config") {
                match serde_json::from_value::<Vec<GameOptimizeConfig>>(value) {
                    Ok(mut cfg) => {
                        // 兼容旧版单一 auto_optimize 开关：迁移为两个独立开关
                        for c in cfg.iter_mut() {
                            if let Some(v) = c.legacy_auto_optimize.take() {
                                c.auto_optimize_priority = v;
                                c.auto_optimize_affinity = v;
                                migrated = true;
                            }
                        }
                        cfg
                    }
                    Err(e) => {
                        log::warn!("Failed to parse game_process_optimize config: {}", e);
                        Vec::new()
                    }
                }
            } else {
                Vec::new()
            }
        }
        Err(e) => {
            log::warn!("Failed to open game_process_optimize store: {}", e);
            Vec::new()
        }
    };

    // 迁移后立即回写，避免旧字段长期残留
    if migrated {
        save_persisted_configs(app, &configs).await;
    }
    configs
}

async fn save_persisted_configs(app: &tauri::AppHandle, configs: &[GameOptimizeConfig]) {
    match app.store(STORE_FILE) {
        Ok(store) => {
            store.set("config", serde_json::to_value(configs).unwrap());
            if let Err(e) = store.save() {
                log::error!("Failed to save game_process_optimize config: {}", e);
            }
        }
        Err(e) => {
            log::error!("Failed to open game_process_optimize store for saving: {}", e);
        }
    }
}

/// 首次运行预置的默认游戏（三角洲行动，与旧版一致：实时优先级 + 除 CPU0 外全部核心）
fn default_configs() -> Vec<GameOptimizeConfig> {
    vec![GameOptimizeConfig {
        id: DEFAULT_GAME_ID.to_string(),
        name: "三角洲行动".to_string(),
        process_names: vec!["DeltaForceClient-Win64-Shipping".to_string()],
        priority: PriorityLevel::Realtime,
        affinity_mask: None,
        auto_optimize_priority: false,
        auto_optimize_affinity: false,
        auto_optimize_ifeo: false,
        legacy_auto_optimize: None,
    }]
}

// ─── 进程匹配辅助 ───

/// 去除 .exe 后缀（大小写不敏感）
fn strip_exe_suffix(s: &str) -> &str {
    if s.len() >= 4 && s[s.len() - 4..].eq_ignore_ascii_case(".exe") {
        &s[..s.len() - 4]
    } else {
        s
    }
}

/// 进程名与名单条目匹配（大小写不敏感、兼容带/不带 .exe）
fn process_matches(process_name: &str, entry: &str) -> bool {
    strip_exe_suffix(process_name).eq_ignore_ascii_case(strip_exe_suffix(entry))
}

/// 找出所有匹配指定进程名的 PID（进程名去重：同一进程名只取第一个 PID）
fn find_pids_by_name(system: &System, names: &[String]) -> Vec<u32> {
    let mut pids = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (_, process) in system.processes() {
        let name = process.name().to_string();
        if names.iter().any(|n| process_matches(&name, n)) {
            let pid = process.pid().as_u32();
            if seen.insert(pid) {
                pids.push(pid);
            }
        }
    }
    pids
}

// ─── 优先级 / 亲和性应用 ───

const IDLE_PRIORITY_CLASS: u32 = 0x00000040;
const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x00004000;
const NORMAL_PRIORITY_CLASS: u32 = 0x00000020;
const ABOVE_NORMAL_PRIORITY_CLASS: u32 = 0x00008000;
const HIGH_PRIORITY_CLASS: u32 = 0x00000080;
const REALTIME_PRIORITY_CLASS: u32 = 0x00000100;

/// 设置进程优先级（复用 optimization 的句柄打开/权限提升逻辑）
fn set_process_priority(pid: u32, level: PriorityLevel) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::SetPriorityClass;

    let class = match level {
        PriorityLevel::Realtime => REALTIME_PRIORITY_CLASS,
        PriorityLevel::High => HIGH_PRIORITY_CLASS,
        PriorityLevel::AboveNormal => ABOVE_NORMAL_PRIORITY_CLASS,
        PriorityLevel::Normal => NORMAL_PRIORITY_CLASS,
        PriorityLevel::BelowNormal => BELOW_NORMAL_PRIORITY_CLASS,
        PriorityLevel::Idle => IDLE_PRIORITY_CLASS,
    };
    unsafe {
        let handle = optimization::open_process_any(pid);
        if handle.is_null() {
            return false;
        }
        let ok = SetPriorityClass(handle, class) != 0;
        CloseHandle(handle);
        ok
    }
}

/// 解析亲和性掩码：`Some(m)` 且 `m>0` 用指定掩码，否则用「除 CPU0 外全部核心」
fn resolve_affinity_mask(mask: Option<u64>) -> u64 {
    match mask {
        Some(m) if m > 0 => m,
        _ => {
            let num_cores = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4);
            let all_cores_mask: u64 = if num_cores >= 64 {
                u64::MAX
            } else {
                (1u64 << num_cores) - 1
            };
            all_cores_mask ^ 1
        }
    }
}

// ─── IFEO 注册表强制优先级 ───
// 原理：Windows 的 Image File Execution Options 的 PerfOptions 会在进程启动瞬间
// 由系统直接应用 CPU 优先级 / IO 优先级，无需进程运行时注入，且对受保护的
// 进程也生效（进程无法自行改回）。注册表级持久设置，会一直生效直到被清除。

/// 优先级等级 → PerfOptions CpuPriorityClass 值
fn priority_to_cpu_class(level: PriorityLevel) -> u32 {
    match level {
        PriorityLevel::Idle => 1,
        PriorityLevel::BelowNormal => 2,
        PriorityLevel::Normal => 3,
        PriorityLevel::AboveNormal => 4,
        PriorityLevel::High => 5,
        PriorityLevel::Realtime => 6,
    }
}

/// 优先级等级 → PerfOptions IoPriority 值（1=高，2=正常，3=低）
fn priority_to_io_priority(level: PriorityLevel) -> u32 {
    match level {
        PriorityLevel::Idle | PriorityLevel::BelowNormal => 3,
        PriorityLevel::Normal | PriorityLevel::AboveNormal => 2,
        PriorityLevel::High | PriorityLevel::Realtime => 1,
    }
}

/// IFEO PerfOptions 注册表路径（进程名自动补 .exe）
fn ifeo_perf_options_path(process_name: &str) -> String {
    let exe = if process_name.to_ascii_lowercase().ends_with(".exe") {
        process_name.to_string()
    } else {
        format!("{}.exe", process_name)
    };
    format!(
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options\{}\PerfOptions",
        exe
    )
}

/// 写入单个进程的 IFEO PerfOptions（CPU + IO 优先级），需要管理员权限
fn apply_game_ifeo_registry(process_name: &str, level: PriorityLevel) -> Result<(), String> {
    let path = ifeo_perf_options_path(process_name);
    let (key, _) = RegKey::predef(HKEY_LOCAL_MACHINE)
        .create_subkey(&path)
        .map_err(|e| format!("创建 IFEO 注册表键失败 ({}): {}", process_name, e))?;
    key.set_value("CpuPriorityClass", &priority_to_cpu_class(level))
        .map_err(|e| format!("写入 CpuPriorityClass 失败 ({}): {}", process_name, e))?;
    key.set_value("IoPriority", &priority_to_io_priority(level))
        .map_err(|e| format!("写入 IoPriority 失败 ({}): {}", process_name, e))?;
    Ok(())
}

/// 删除单个进程的 IFEO PerfOptions
fn remove_game_ifeo_registry(process_name: &str) -> Result<(), String> {
    let path = ifeo_perf_options_path(process_name);
    match RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(&path) {
        Ok(key) => {
            let _ = key.delete_value("CpuPriorityClass");
            let _ = key.delete_value("IoPriority");
            Ok(())
        }
        Err(_) => Ok(()), // 键不存在视为已清除
    }
}

/// 查询某个进程的 IFEO PerfOptions 是否已应用（只读，无需管理员）
fn ifeo_is_applied(process_name: &str) -> bool {
    let path = ifeo_perf_options_path(process_name);
    if let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(&path) {
        if let Ok(v) = key.get_value::<u32, _>("CpuPriorityClass") {
            return v > 0;
        }
    }
    false
}

/// 同步 IFEO 注册表与配置：为「已开启 IFEO 且优先级有变化/新增」的进程写入，
/// 为「已关闭或已删除」的进程清除。需要管理员权限；无权限时仅记录日志不报错。
fn sync_ifeo_registry(
    new_configs: &[GameOptimizeConfig],
    old_configs: &[GameOptimizeConfig],
) {
    if !optimization::is_admin() {
        log::info!("[游戏进程优化] 非管理员，跳过 IFEO 注册表同步");
        return;
    }
    let enabled = |cfgs: &[GameOptimizeConfig]| -> HashMap<String, PriorityLevel> {
        let mut map = HashMap::new();
        for c in cfgs {
            if c.auto_optimize_ifeo {
                for n in &c.process_names {
                    map.entry(n.to_lowercase()).or_insert(c.priority);
                }
            }
        }
        map
    };
    let old = enabled(old_configs);
    let new = enabled(new_configs);

    // 关闭/删除的 → 清除
    for name in old.keys() {
        if !new.contains_key(name) {
            let _ = remove_game_ifeo_registry(name);
            log::info!("[游戏进程优化] 已清除 IFEO: {}", name);
        }
    }
    // 新增或优先级变化的 → 写入
    for (name, level) in &new {
        if old.get(name) != Some(level) {
            match apply_game_ifeo_registry(name, *level) {
                Ok(()) => log::info!("[游戏进程优化] 已应用 IFEO: {}", name),
                Err(e) => log::warn!("[游戏进程优化] 应用 IFEO 失败 ({}): {}", name, e),
            }
        }
    }
}

// ─── 自动优化线程 ───

fn update_game_status(
    game_id: &str,
    running: bool,
    optimized: bool,
    priority_applied: bool,
    affinity_applied: bool,
) {
    let mut guard = GAME_STATUS.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    let entry = map.entry(game_id.to_string()).or_default();
    entry.game_id = game_id.to_string();
    entry.running = running;
    entry.optimized = optimized;
    entry.priority_applied = priority_applied;
    entry.affinity_applied = affinity_applied;
    if running && optimized {
        entry.last_apply = Some(chrono::Local::now().to_rfc3339());
    }
}

fn auto_optimize_loop(generation: u64) {
    let mut system = System::new();
    let mut running_map: HashMap<String, bool> = HashMap::new();

    loop {
        if GENERATION.load(Ordering::Relaxed) != generation {
            break;
        }
        thread::sleep(Duration::from_secs(AUTO_POLL_INTERVAL_SECS));
        if GENERATION.load(Ordering::Relaxed) != generation {
            break;
        }

        let configs = get_configs();
        let auto_games: Vec<GameOptimizeConfig> = configs
            .iter()
            .filter(|c| c.auto_optimize_priority || c.auto_optimize_affinity)
            .cloned()
            .collect();
        if auto_games.is_empty() {
            continue;
        }

        system.refresh_processes();

        for game in &auto_games {
            let pids = find_pids_by_name(&system, &game.process_names);
            let is_running = !pids.is_empty();
            let was_running = running_map.get(&game.id).copied().unwrap_or(false);

            if is_running && !was_running {
                // 游戏刚启动 → 按该游戏独立开启的开关分别应用
                let mut priority_applied = false;
                let mut affinity_applied = false;
                for pid in &pids {
                    if game.auto_optimize_priority
                        && set_process_priority(*pid, game.priority)
                    {
                        priority_applied = true;
                    }
                    if game.auto_optimize_affinity
                        && optimization::set_process_affinity(*pid, resolve_affinity_mask(game.affinity_mask))
                    {
                        affinity_applied = true;
                    }
                }
                let optimized = priority_applied || affinity_applied;
                update_game_status(&game.id, true, optimized, priority_applied, affinity_applied);
                log::info!(
                    "[游戏进程优化] {} 已自动应用优化（进程: {}, 核心: {}）",
                    game.name,
                    if priority_applied { "✓" } else { "-" },
                    if affinity_applied { "✓" } else { "-" },
                );
            } else if !is_running && was_running {
                // 游戏已退出 → 重置状态
                update_game_status(&game.id, false, false, false, false);
                log::info!("[游戏进程优化] {} 已退出，重置自动优化状态", game.name);
            }

            running_map.insert(game.id.clone(), is_running);
        }
    }
}

/// 确保自动优化线程与当前配置一致：有开启任一类自动优化的游戏则启动，否则停止
fn ensure_auto_thread() {
    let has_auto = get_configs()
        .iter()
        .any(|c| c.auto_optimize_priority || c.auto_optimize_affinity);
    let running = AUTO_RUNNING.load(Ordering::Relaxed);
    if has_auto == running {
        return; // 状态一致，无需操作
    }

    let gen = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    if has_auto {
        AUTO_RUNNING.store(true, Ordering::Relaxed);
        thread::spawn(move || {
            let _ = panic::catch_unwind(|| auto_optimize_loop(gen));
            AUTO_RUNNING.store(false, Ordering::Relaxed);
        });
        log::info!("[游戏进程优化] 自动优化线程已启动");
    } else {
        AUTO_RUNNING.store(false, Ordering::Relaxed);
        log::info!("[游戏进程优化] 自动优化线程已停止");
    }
}

// ─── 初始化 ───

/// 应用启动时调用：恢复持久化配置（首次预置三角洲）并启动自动优化线程
pub async fn init(app: tauri::AppHandle) -> Result<(), String> {
    let configs = load_persisted_configs(&app).await;

    {
        let mut lock = CONFIGS.lock().map_err(|e| e.to_string())?;
        *lock = Some(configs.clone());
    }

    if configs.is_empty() {
        let defaults = default_configs();
        save_persisted_configs(&app, &defaults).await;
        let mut lock = CONFIGS.lock().map_err(|e| e.to_string())?;
        *lock = Some(defaults);
        log::info!("[游戏进程优化] 首次运行，已预置默认游戏：三角洲行动");
    }

    // 启动时同步一次 IFEO 注册表，确保与持久化配置一致
    let current = get_configs();
    sync_ifeo_registry(&current, &[]);

    ensure_auto_thread();
    Ok(())
}

// ─── Tauri 命令 ───

/// 获取全部游戏优化配置（首次未初始化时返回预置三角洲）
#[tauri::command]
pub async fn get_game_optimize_configs(_app: tauri::AppHandle) -> Result<Vec<GameOptimizeConfig>, String> {
    Ok(get_configs())
}

/// 保存全部游戏优化配置（增删改/排序统一入口）
#[tauri::command]
pub async fn save_game_optimize_configs(
    app: tauri::AppHandle,
    configs: Vec<GameOptimizeConfig>,
) -> Result<(), String> {
    let old_configs = get_configs();
    let cleaned: Vec<GameOptimizeConfig> = configs
        .into_iter()
        .filter(|c| !c.name.trim().is_empty() && !c.process_names.is_empty())
        .map(|mut c| {
            c.name = c.name.trim().to_string();
            c.process_names = c
                .process_names
                .iter()
                .map(|s| s.trim().trim_end_matches(".exe").trim_end_matches(".EXE").to_string())
                .filter(|s| !s.is_empty())
                .collect();
            c
        })
        .collect();

    {
        let mut lock = CONFIGS.lock().map_err(|e| e.to_string())?;
        *lock = Some(cleaned.clone());
    }

    // 清理已删除游戏的运行状态
    {
        let mut guard = GAME_STATUS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(map) = guard.as_mut() {
            map.retain(|id, _| cleaned.iter().any(|c| &c.id == id));
        }
    }

    save_persisted_configs(&app, &cleaned).await;
    // 同步 IFEO 注册表（新增/删除/改名/关闭都会据此写入或清除）
    sync_ifeo_registry(&cleaned, &old_configs);
    ensure_auto_thread();
    Ok(())
}

/// 手动应用指定游戏的进程优先级优化
#[tauri::command]
pub async fn optimize_game_priority(
    _app: tauri::AppHandle,
    game_id: String,
) -> Result<GameOptimizeResult, String> {
    let configs = get_configs();
    let game = configs
        .iter()
        .find(|c| c.id == game_id)
        .ok_or_else(|| "未找到该游戏配置".to_string())?
        .clone();

    let mut system = System::new();
    system.refresh_processes();
    let pids = find_pids_by_name(&system, &game.process_names);

    let mut modified = 0usize;
    for pid in &pids {
        if set_process_priority(*pid, game.priority) {
            modified += 1;
        }
    }

    Ok(GameOptimizeResult {
        success: modified > 0,
        message: if modified > 0 {
            format!("已提升 {} 个「{}」进程优先级", modified, game.name)
        } else if !pids.is_empty() {
            format!(
                "「{}」进程已运行，但优先级修改失败（进程受保护或权限不足）",
                game.name
            )
        } else {
            format!("「{}」未运行，请先启动游戏", game.name)
        },
        found_count: pids.len(),
        modified_count: modified,
    })
}

/// 手动应用指定游戏的 IFEO 强制优先级（注册表级，进程启动即生效，需管理员权限）
#[tauri::command]
pub async fn apply_game_ifeo(
    _app: tauri::AppHandle,
    game_id: String,
) -> Result<GameOptimizeResult, String> {
    if !optimization::is_admin() {
        return Err("写入 IFEO 注册表需要管理员权限。当前 NexBox 未以管理员身份运行，请右键「以管理员身份运行」后重试".to_string());
    }
    let configs = get_configs();
    let game = configs
        .iter()
        .find(|c| c.id == game_id)
        .ok_or_else(|| "未找到该游戏配置".to_string())?
        .clone();

    let mut modified = 0usize;
    let mut failed = 0usize;
    for name in &game.process_names {
        match apply_game_ifeo_registry(name, game.priority) {
            Ok(()) => modified += 1,
            Err(e) => {
                failed += 1;
                log::warn!("[游戏进程优化] 手动应用 IFEO 失败 ({}): {}", name, e);
            }
        }
    }

    Ok(GameOptimizeResult {
        success: modified > 0,
        message: if modified > 0 {
            format!("已为「{}」写入 {} 个进程的 IFEO 强制优先级（进程重启后生效）", game.name, modified)
        } else {
            format!("「{}」IFEO 写入失败（{} 个进程）", game.name, failed)
        },
        found_count: failed,
        modified_count: modified,
    })
}

/// 恢复指定游戏的 IFEO 强制优先级：删除注册表 PerfOptions，解除强制（需管理员权限）
#[tauri::command]
pub async fn restore_game_ifeo(
    _app: tauri::AppHandle,
    game_id: String,
) -> Result<GameOptimizeResult, String> {
    if !optimization::is_admin() {
        return Err("写入 IFEO 注册表需要管理员权限。当前 NexBox 未以管理员身份运行，请右键「以管理员身份运行」后重试".to_string());
    }
    let configs = get_configs();
    let game = configs
        .iter()
        .find(|c| c.id == game_id)
        .ok_or_else(|| "未找到该游戏配置".to_string())?
        .clone();

    let mut modified = 0usize;
    let mut failed = 0usize;
    for name in &game.process_names {
        match remove_game_ifeo_registry(name) {
            Ok(()) => modified += 1,
            Err(e) => {
                failed += 1;
                log::warn!("[游戏进程优化] 恢复 IFEO 失败 ({}): {}", name, e);
            }
        }
    }

    Ok(GameOptimizeResult {
        success: modified > 0,
        message: if modified > 0 {
            format!("已为「{}」清除 {} 个进程的 IFEO 强制优先级", game.name, modified)
        } else {
            format!("「{}」IFEO 恢复失败（{} 个进程）", game.name, failed)
        },
        found_count: failed,
        modified_count: modified,
    })
}

/// 手动应用指定游戏的 CPU 核心分配（mask 为空时使用该游戏已保存掩码/默认）
#[tauri::command]
pub async fn optimize_game_affinity(
    _app: tauri::AppHandle,
    game_id: String,
    mask: Option<u64>,
) -> Result<GameOptimizeResult, String> {
    let configs = get_configs();
    let game = configs
        .iter()
        .find(|c| c.id == game_id)
        .ok_or_else(|| "未找到该游戏配置".to_string())?
        .clone();

    let resolved = resolve_affinity_mask(mask.or(game.affinity_mask));

    let mut system = System::new();
    system.refresh_processes();
    let pids = find_pids_by_name(&system, &game.process_names);

    let mut modified = 0usize;
    for pid in &pids {
        if optimization::set_process_affinity(*pid, resolved) {
            modified += 1;
        }
    }

    Ok(GameOptimizeResult {
        success: modified > 0,
        message: if modified > 0 {
            format!("已为 {} 个「{}」进程分配指定核心", modified, game.name)
        } else if !pids.is_empty() {
            format!(
                "「{}」进程已运行，但核心分配失败（进程受保护或权限不足）",
                game.name
            )
        } else {
            format!("「{}」未运行，请先启动游戏", game.name)
        },
        found_count: pids.len(),
        modified_count: modified,
    })
}

/// 开关指定游戏的「进程/核心/IFEO」自动优化之一（kind: "priority" | "affinity" | "ifeo"，每游戏独立）
#[tauri::command]
pub async fn set_game_auto_optimize(
    app: tauri::AppHandle,
    game_id: String,
    kind: String,
    enabled: bool,
) -> Result<(), String> {
    // IFEO 开启需要管理员权限写注册表；未授权时先拒绝，避免配置被持久化为开启但注册表未写入
    if kind == "ifeo" && enabled && !optimization::is_admin() {
        return Err("开启 IFEO 优先级优化需要管理员权限。当前 NexBox 未以管理员身份运行，请右键「以管理员身份运行」后重试".to_string());
    }

    let old_configs = get_configs();
    let mut configs = get_configs();
    let game = configs
        .iter_mut()
        .find(|c| c.id == game_id)
        .ok_or_else(|| format!("未找到游戏配置: {}", game_id))?;

    match kind.as_str() {
        "priority" => game.auto_optimize_priority = enabled,
        "affinity" => game.auto_optimize_affinity = enabled,
        "ifeo" => game.auto_optimize_ifeo = enabled,
        _ => return Err(format!("未知的自动优化类型: {}", kind)),
    }

    let updated = configs.clone();
    {
        let mut lock = CONFIGS.lock().map_err(|e| e.to_string())?;
        *lock = Some(updated.clone());
    }
    save_persisted_configs(&app, &updated).await;
    if kind == "ifeo" {
        sync_ifeo_registry(&updated, &old_configs);
    }
    ensure_auto_thread();
    log::info!(
        "[游戏进程优化] 游戏 {} 的「{}」自动优化已{}",
        game_id,
        match kind.as_str() {
            "priority" => "进程",
            "affinity" => "核心",
            _ => "IFEO",
        },
        if enabled { "开启" } else { "关闭" }
    );
    Ok(())
}

/// 获取全部游戏的自动优化运行状态（前端轮询展示）
#[tauri::command]
pub async fn get_game_auto_optimize_status(
    _app: tauri::AppHandle,
) -> Result<Vec<GameAutoStatus>, String> {
    let configs = get_configs();
    let guard = GAME_STATUS.lock().map_err(|e| e.to_string())?;
    let result = configs
        .iter()
        .map(|c| {
            let mut status = guard
                .as_ref()
                .and_then(|map| map.get(&c.id))
                .cloned()
                .unwrap_or_default();
            // IFEO 是注册表持久状态，与进程是否运行无关，直接按注册表实际值报告
            if c.auto_optimize_ifeo {
                status.ifeo_applied = c
                    .process_names
                    .iter()
                    .any(|n| ifeo_is_applied(n));
            } else {
                status.ifeo_applied = false;
            }
            status
        })
        .collect();
    Ok(result)
}

/// 从 .lnk 快捷方式中读取目标路径（MS-SHLLINK Shell Link 二进制格式）。
///
/// 直接解析 Shell Link 文件的 LinkInfo 结构，读取 LocalBasePath（ANSI）
/// 或 LocalBasePathUnicode（UTF-16）字段，毫秒级完成，无需启动 PowerShell。
/// 解析失败（如损坏/特殊快捷方式）时返回 None。
fn resolve_shortcut_target(lnk_path: &str) -> Option<String> {
    use std::path::Path;

    let data = std::fs::read(lnk_path).ok()?;
    // 最短的合法 Shell Link 至少包含 76 字节 ShellLinkHeader
    if data.len() < 0x4C {
        return None;
    }

    let rd_u16 = |off: usize| -> Option<u16> {
        if off + 2 > data.len() {
            None
        } else {
            Some(u16::from_le_bytes([data[off], data[off + 1]]))
        }
    };
    let rd_u32 = |off: usize| -> Option<u32> {
        if off + 4 > data.len() {
            None
        } else {
            Some(u32::from_le_bytes([
                data[off],
                data[off + 1],
                data[off + 2],
                data[off + 3],
            ]))
        }
    };

    // ShellLinkHeader.LinkFlags 位于偏移 0x14
    let link_flags = rd_u32(0x14)?;
    const HAS_LINK_TARGET_IDLIST: u32 = 0x0000_0001;
    const HAS_LINK_INFO: u32 = 0x0000_0002;

    if link_flags & HAS_LINK_INFO == 0 {
        return None; // 无 LinkInfo，无法解析本地路径
    }

    // 偏移 0x4C 开始是 LinkTargetIDList（若有）+ LinkInfo
    let mut offset: usize = 0x4C;
    if link_flags & HAS_LINK_TARGET_IDLIST != 0 {
        // LinkTargetIDList: 2 字节大小 + IDList
        let idlist_size = rd_u16(offset)? as usize;
        offset += 2 + idlist_size;
    }

    // ── LinkInfo 结构（MS-SHLLINK 2.3）──
    // 布局：LinkInfoSize(4) | LinkInfoHeaderSize(4) | LinkInfoFlags(4) |
    //        VolumeIDOffset(4) | LocalBasePathOffset(4) |
    //        CommonNetworkRelativeLinkOffset(4) | CommonPathSuffixOffset(4) |
    //        LocalBasePathOffsetUnicode(4，仅当 LinkInfoHeaderSize >= 0x24)
    let link_info_size = rd_u32(offset)? as usize;
    if link_info_size < 0x1C || offset + link_info_size > data.len() {
        return None;
    }
    let link_info_header_size = rd_u32(offset + 0x04)? as usize;
    let link_info_flags = rd_u32(offset + 0x08)?;
    let local_base_path_offset = rd_u32(offset + 0x10)? as usize;
    const VOLUME_AND_LOCAL_BASE_PATH: u32 = 0x0000_0001;

    // 优先解析 ANSI LocalBasePath
    if local_base_path_offset > 0 {
        let abs = offset + local_base_path_offset;
        if abs < data.len() {
            let end = data[abs..].iter().position(|&b| b == 0).map(|p| abs + p);
            if let Some(end) = end {
                let raw = &data[abs..end];
                if !raw.is_empty() {
                    if let Ok(s) = std::str::from_utf8(raw) {
                        let s = s.trim();
                        if !s.is_empty() && Path::new(s).is_absolute() {
                            return Some(s.to_string());
                        }
                    }
                }
            }
        }
    }

    // 回退：解析 UTF-16 LocalBasePathUnicode（+0x1C，仅当 LinkInfoHeaderSize >= 0x24）
    if link_info_flags & VOLUME_AND_LOCAL_BASE_PATH != 0 && link_info_header_size >= 0x24 {
        let unicode_offset = rd_u32(offset + 0x1C)? as usize;
        if unicode_offset > 0 {
            let abs = offset + unicode_offset;
            if abs + 2 <= data.len() {
                let units: Vec<u16> = (0..)
                    .map_while(|i| {
                        let p = abs + i * 2;
                        if p + 2 > data.len() {
                            None
                        } else {
                            Some(u16::from_le_bytes([data[p], data[p + 1]]))
                        }
                    })
                    .take_while(|&u| u != 0)
                    .collect();
                let s = String::from_utf16_lossy(&units).trim().to_string();
                if !s.is_empty() && Path::new(&s).is_absolute() {
                    return Some(s);
                }
            }
        }
    }

    None
}

/// 选择游戏可执行文件（.exe/.lnk）。
/// 若选择的是 .lnk 快捷方式，则解析其指向的目标 exe，使用目标程序名作为进程名。
#[tauri::command]
pub async fn select_game_executable() -> Option<GameExecutableInfo> {
    let file = rfd::FileDialog::new()
        .set_title("选择游戏可执行文件")
        .add_filter("可执行文件和快捷方式", &["exe", "lnk"])
        .add_filter("所有文件", &["*"])
        .pick_file()?;

    let path = file.to_string_lossy().to_string();
    let file_name = file
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    // 快捷方式：解析到目标 exe，用目标名作为进程名（游戏名保留快捷方式名更友好）
    let is_lnk = file_name.len() >= 4 && file_name[file_name.len() - 4..].eq_ignore_ascii_case(".lnk");
    if is_lnk {
        if let Some(target) = resolve_shortcut_target(&path) {
            let target_name = std::path::Path::new(&target)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let process_name = strip_exe_suffix(&target_name).to_string();
            let game_name = if process_name.is_empty() {
                strip_exe_suffix(&file_name).to_string()
            } else {
                process_name.clone()
            };
            return Some(GameExecutableInfo {
                path: target,
                process_name,
                game_name,
            });
        }
        // 解析失败时回退：去掉 .lnk 后缀作为进程名
        let game_name = strip_exe_suffix(&file_name).to_string();
        return Some(GameExecutableInfo {
            path: path.clone(),
            process_name: game_name.clone(),
            game_name,
        });
    }

    let process_name = strip_exe_suffix(&file_name).to_string();
    let game_name = if process_name.is_empty() {
        file_name
    } else {
        process_name.clone()
    };

    Some(GameExecutableInfo {
        path,
        process_name,
        game_name,
    })
}

/// 枚举当前运行中的进程（去重后按内存占用降序），供「选择正在运行的进程」
#[tauri::command]
pub async fn list_running_processes() -> Result<Vec<RunningProcessInfo>, String> {
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessRefreshKind::everything().with_memory());

    let mut seen = std::collections::HashSet::new();
    let mut list: Vec<RunningProcessInfo> = Vec::new();
    for (_, proc_) in sys.processes() {
        let name = proc_.name().to_string();
        if name.is_empty() || seen.contains(&name.to_lowercase()) {
            continue;
        }
        seen.insert(name.to_lowercase());
        list.push(RunningProcessInfo {
            name,
            pid: proc_.pid().as_u32(),
            memory_mb: proc_.memory() as f64 / 1024.0 / 1024.0,
        });
    }

    list.sort_by(|a, b| {
        b.memory_mb
            .partial_cmp(&a.memory_mb)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(list)
}

/// 当前进程是否以管理员身份运行（前端用于权限提示）
#[tauri::command]
pub async fn check_game_optimize_admin() -> Result<bool, String> {
    Ok(optimization::is_admin())
}
