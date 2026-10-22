//! 规则库分层存储：线上库 + 用户改动
//!
//! v1.6.0 起，规则库拆成三层，**互不干扰**：
//!
//! | 层 | 文件 | 位置 | 可写 |
//! |---|---|---|---|
//! | 本地库 | `rules.json` | 安装目录 `resources/gun-codes/` | 只读（随包） |
//! | 线上库 | `rules.online.json` | `appDataDir` | 可写（热更新下载） |
//! | 用户改动 | `rules.overrides.json` | `appDataDir` | 可写（删/改单条） |
//!
//! 旧版把本地与线上合并成一个 `rules.merged.json` 再加载，导致：
//!   ① 版本号变成 `max(本地, 线上)`，看不出两边各自的进度；
//!   ② 想「只清空线上部分」做不到，只能整体重置。
//!
//! 现在清空线上库 = 删一个文件，清空用户改动 = 删另一个文件。

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use tauri::Manager;

use crate::local_gun_codes::{Fingerprint, GunMeta, GunRules};

/// 线上库文件名
const ONLINE_FILE: &str = "rules.online.json";
/// 用户改动文件名
const OVERRIDES_FILE: &str = "rules.overrides.json";

// ═══════════════════════ 线上库 ═══════════════════════

/// 线上规则库（与 rules.json 同构，另带来源标记）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OnlineRules {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub generated: String,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub guns: HashMap<String, GunMeta>,
    #[serde(default)]
    pub fingerprints: HashMap<String, Fingerprint>,
    /// 本地记录的下载信息（非远端字段）
    #[serde(default)]
    pub fetched_at: String,
    /// 本地记录的下载来源（github-raw / jsdelivr）
    #[serde(default)]
    pub fetched_from: String,
}

pub fn online_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join(ONLINE_FILE))
}

/// 读取线上库（不存在则 None）
pub fn load_online_rules(app: &tauri::AppHandle) -> Option<OnlineRules> {
    let p = online_path(app)?;
    if !p.exists() {
        return None;
    }
    match std::fs::read_to_string(&p) {
        Ok(text) => match serde_json::from_str::<OnlineRules>(&text) {
            Ok(r) => Some(r),
            Err(e) => {
                log::warn!("[Rules] 线上库解析失败 {}: {e}", p.display());
                None
            }
        },
        Err(e) => {
            log::warn!("[Rules] 线上库读取失败 {}: {e}", p.display());
            None
        }
    }
}

/// 保存线上库
pub fn save_online_rules(app: &tauri::AppHandle, r: &OnlineRules) -> Result<PathBuf, String> {
    let p = online_path(app).ok_or("无法获取应用数据目录")?;
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建目录失败: {e}"))?;
    }
    let text = serde_json::to_string_pretty(r).map_err(|e| format!("序列化失败: {e}"))?;
    std::fs::write(&p, text).map_err(|e| format!("写入失败: {e}"))?;
    Ok(p)
}

/// 删除线上库，回到纯本地库
pub fn clear_online_rules(app: &tauri::AppHandle) -> Result<bool, String> {
    let Some(p) = online_path(app) else {
        return Err("无法获取应用数据目录".into());
    };
    if !p.exists() {
        return Ok(false);
    }
    std::fs::remove_file(&p).map_err(|e| format!("删除失败: {e}"))?;
    log::info!("[Rules] 已清空线上库");
    Ok(true)
}

/// 把线上库叠加到随包库上：**线上优先，可覆盖随包库的同名指纹**。
///
/// 返回 `(added, updated)`：
///   - `added`   随包库没有、线上补入的条数
///   - `updated` 两边都有但枪名不同、被线上覆盖的条数
///
/// # 为什么线上可以覆盖随包库（v1.9.0 起的语义变更）
///
/// 旧实现是「只补空缺，绝不覆盖」，理由是「本地永远优先」。
/// 但那条原则本意是保护**用户手动确认的数据**（学习表 / overrides），
/// 却被错误地套用到了**随包分发的 rules.json** 上 ——
/// 随包库只是内置默认值，它会随版本更新，不该压过线上修正。
///
/// 实测后果：随包库某条错了、线上库修正了，用户点了「更新词库」也修不掉，
/// 表现为「虽然同步了线上的，但错的还是错」。
///
/// # 真正的优先级
///
/// ```text
/// 学习表（用户手动填的）      ← 最高，在 recognize_weapon 第一层生效
/// 用户改动 overrides         ← 最高，在本函数之后应用
/// 线上库                     ← 中，可覆盖随包库
/// 随包库 rules.json          ← 最低，兜底默认值
/// ```
///
/// # 保留的安全校验
///
/// 线上条目的枪名必须能在枪表里找到，否则跳过 ——
/// 防止旧版线上库（v5 曾有 56.8% 枪名错认）的脏数据混进来。
pub fn overlay_online(base: &mut GunRules, online: &OnlineRules) -> (usize, usize) {
    let mut added = 0usize;
    let mut updated = 0usize;

    // 先把线上枪表补进 base（仅当 base 没有该枪）
    for (name, meta) in &online.guns {
        base.guns.entry(name.clone()).or_insert_with(|| meta.clone());
    }

    for (fp, f) in &online.fingerprints {
        // ── 安全校验：不干净的条目一律跳过（新增和覆盖都要过这一关）──
        if f.gun.trim().is_empty() {
            continue; // 空枪名会污染界面
        }
        if !f.mode.is_empty() && f.mode != "烽火地带" {
            continue;
        }
        // 枪名必须在枪表里（防止孤立条目）
        if !base.guns.contains_key(&f.gun) {
            continue;
        }

        // config 强制清空：线上库不带备注进来
        let clean = Fingerprint {
            gun: f.gun.clone(),
            config: String::new(),
            mode: "烽火地带".to_string(),
        };

        match base.fingerprints.get(fp) {
            None => {
                // 随包库没有 → 补入
                base.fingerprints.insert(fp.clone(), clean);
                added += 1;
            }
            Some(existing) => {
                // 两边都有 → 枪名不同则线上覆盖（这就是本版的核心修正）
                if existing.gun != clean.gun {
                    base.fingerprints.insert(fp.clone(), clean);
                    updated += 1;
                }
            }
        }
    }

    // 版本号取较大者：让界面能反映「已包含到哪一版」
    base.version = base.version.max(online.version);

    (added, updated)
}

// ═══════════════════════ 用户改动 ═══════════════════════

/// 用户对规则库的手动改动
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuleOverrides {
    /// 被删除的指纹（黑名单，加载时跳过）
    #[serde(default)]
    pub removed: Vec<String>,
    /// 被修改的枪名（指纹 → 新枪名）
    #[serde(default)]
    pub patched: HashMap<String, PatchedFingerprint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchedFingerprint {
    pub gun: String,
    #[serde(default)]
    pub config: String,
}

pub fn overrides_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join(OVERRIDES_FILE))
}

/// 读取用户改动（不存在则 None）
pub fn load_overrides(app: &tauri::AppHandle) -> Option<RuleOverrides> {
    let p = overrides_path(app)?;
    if !p.exists() {
        return None;
    }
    match std::fs::read_to_string(&p) {
        Ok(text) => match serde_json::from_str::<RuleOverrides>(&text) {
            Ok(r) => Some(r),
            Err(e) => {
                log::warn!("[Rules] 用户改动解析失败 {}: {e}", p.display());
                None
            }
        },
        Err(e) => {
            log::warn!("[Rules] 用户改动读取失败 {}: {e}", p.display());
            None
        }
    }
}

/// 保存用户改动
pub fn save_overrides(app: &tauri::AppHandle, o: &RuleOverrides) -> Result<(), String> {
    let p = overrides_path(app).ok_or("无法获取应用数据目录")?;
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建目录失败: {e}"))?;
    }
    let text = serde_json::to_string_pretty(o).map_err(|e| format!("序列化失败: {e}"))?;
    std::fs::write(&p, text).map_err(|e| format!("写入失败: {e}"))?;
    Ok(())
}

/// 删除用户改动文件（恢复规则库原样）
pub fn clear_overrides(app: &tauri::AppHandle) -> Result<bool, String> {
    let Some(p) = overrides_path(app) else {
        return Err("无法获取应用数据目录".into());
    };
    if !p.exists() {
        return Ok(false);
    }
    std::fs::remove_file(&p).map_err(|e| format!("删除失败: {e}"))?;
    log::info!("[Rules] 已清除用户改动");
    Ok(true)
}

/// 把用户改动应用到规则库上，返回 `(删除数, 修改数)`。
///
/// 顺序很重要：**先删后改**。若同一指纹既被删又被改，以删为准（用户意图更明确）。
pub fn apply_overrides(base: &mut GunRules, ov: &RuleOverrides) -> (usize, usize) {
    let mut removed = 0usize;
    let mut patched = 0usize;

    let removed_set: HashSet<&String> = ov.removed.iter().collect();
    for fp in &removed_set {
        if base.fingerprints.remove(*fp).is_some() {
            removed += 1;
        }
    }

    for (fp, p) in &ov.patched {
        if removed_set.contains(fp) {
            continue; // 已删除的忽略
        }
        if p.gun.trim().is_empty() {
            continue;
        }
        if let Some(entry) = base.fingerprints.get_mut(fp) {
            entry.gun = p.gun.trim().to_string();
            entry.config = p.config.trim().to_string();
            patched += 1;
        }
    }

    (removed, patched)
}

// ═══════════════════════ Tauri 命令 ═══════════════════════

/// 列出规则库条目（支持搜索与分页）
///
/// 2327 条不能一次全发，必须分页。搜索同时匹配指纹与枪名。
#[tauri::command]
pub fn list_rule_fingerprints(
    app: tauri::AppHandle,
    query: Option<String>,
    page: Option<usize>,
    page_size: Option<usize>,
) -> serde_json::Value {
    let rules = crate::local_gun_codes::load_rules(&app);
    let ov = load_overrides(&app).unwrap_or_default();
    let removed_set: HashSet<&String> = ov.removed.iter().collect();

    let q = query.unwrap_or_default().trim().to_uppercase();
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(50).clamp(1, 200);

    // 先收集、排序，保证分页稳定
    let mut all: Vec<(String, String, String)> = rules
        .fingerprints
        .iter()
        .map(|(fp, f)| (fp.clone(), f.gun.clone(), f.config.clone()))
        .collect();
    all.sort_by(|a, b| a.0.cmp(&b.0));

    let filtered: Vec<_> = all
        .into_iter()
        .filter(|(fp, gun, _)| {
            if q.is_empty() {
                return true;
            }
            fp.to_uppercase().contains(&q) || gun.to_uppercase().contains(&q)
        })
        .collect();

    let total = filtered.len();
    let start = (page - 1) * page_size;
    let items: Vec<serde_json::Value> = filtered
        .into_iter()
        .skip(start)
        .take(page_size)
        .map(|(fp, gun, cfg)| {
            let is_patched = ov.patched.contains_key(&fp);
            serde_json::json!({
                "fingerprint": fp,
                "gun": gun,
                "config": cfg,
                "patched": is_patched,
                "removed": removed_set.contains(&fp),
            })
        })
        .collect();

    serde_json::json!({
        "ok": true,
        "total": total,
        "page": page,
        "pageSize": page_size,
        "items": items,
        "removedCount": ov.removed.len(),
        "patchedCount": ov.patched.len(),
    })
}

/// 删除单条规则（写入黑名单）
#[tauri::command]
pub fn delete_rule_fingerprint(app: tauri::AppHandle, fingerprint: String) -> serde_json::Value {
    let fp = fingerprint.trim().to_uppercase();
    if fp.is_empty() {
        return serde_json::json!({ "ok": false, "error": "指纹不能为空" });
    }
    let rules = crate::local_gun_codes::load_rules(&app);
    if !rules.fingerprints.contains_key(&fp) {
        return serde_json::json!({ "ok": false, "error": format!("未找到指纹 {fp}") });
    }

    let mut ov = load_overrides(&app).unwrap_or_default();
    if !ov.removed.iter().any(|x| x == &fp) {
        ov.removed.push(fp.clone());
    }
    ov.patched.remove(&fp); // 删除优先，清掉可能存在的修改

    if let Err(e) = save_overrides(&app, &ov) {
        return serde_json::json!({ "ok": false, "error": e });
    }
    crate::local_gun_codes::reload_rules(&app);
    log::info!("[Rules] 已删除规则指纹 {fp}");
    serde_json::json!({ "ok": true, "fingerprint": fp, "removedCount": ov.removed.len() })
}

/// 修改单条规则的枪名
#[tauri::command]
pub fn update_rule_fingerprint(
    app: tauri::AppHandle,
    fingerprint: String,
    gun: String,
) -> serde_json::Value {
    let fp = fingerprint.trim().to_uppercase();
    let g = gun.trim().to_string();
    if fp.is_empty() {
        return serde_json::json!({ "ok": false, "error": "指纹不能为空" });
    }
    if g.is_empty() {
        return serde_json::json!({ "ok": false, "error": "枪名不能为空" });
    }

    let rules = crate::local_gun_codes::load_rules(&app);
    if !rules.fingerprints.contains_key(&fp) {
        return serde_json::json!({ "ok": false, "error": format!("未找到指纹 {fp}") });
    }

    let mut ov = load_overrides(&app).unwrap_or_default();
    // 若在删除黑名单里，先移出（用户改成「要这条」）
    ov.removed.retain(|x| x != &fp);
    ov.patched.insert(
        fp.clone(),
        PatchedFingerprint {
            gun: g.clone(),
            config: String::new(),
        },
    );

    if let Err(e) = save_overrides(&app, &ov) {
        return serde_json::json!({ "ok": false, "error": e });
    }
    crate::local_gun_codes::reload_rules(&app);
    log::info!("[Rules] 已修改规则指纹 {fp} → {g}");
    serde_json::json!({ "ok": true, "fingerprint": fp, "gun": g, "patchedCount": ov.patched.len() })
}

/// 清空线上库（回到纯本地库）
#[tauri::command]
pub fn clear_online_rules_cmd(app: tauri::AppHandle) -> serde_json::Value {
    match clear_online_rules(&app) {
        Ok(removed) => {
            let r = crate::local_gun_codes::reload_rules(&app);
            serde_json::json!({
                "ok": true,
                "removed": removed,
                "version": r.version,
                "fingerprintCount": r.fingerprints.len(),
                "gunCount": r.guns.len(),
                "guns": crate::local_gun_codes::guns_table_public(&r),
            })
        }
        Err(e) => serde_json::json!({ "ok": false, "error": e }),
    }
}

/// 清空全部用户改动（删除黑名单 + 修改记录）
#[tauri::command]
pub fn clear_rule_overrides(app: tauri::AppHandle) -> serde_json::Value {
    match clear_overrides(&app) {
        Ok(removed) => {
            let r = crate::local_gun_codes::reload_rules(&app);
            serde_json::json!({
                "ok": true,
                "removed": removed,
                "version": r.version,
                "fingerprintCount": r.fingerprints.len(),
                "guns": crate::local_gun_codes::guns_table_public(&r),
            })
        }
        Err(e) => serde_json::json!({ "ok": false, "error": e }),
    }
}
