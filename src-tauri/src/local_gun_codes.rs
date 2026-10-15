//! 本地改枪码存储模块（三角洲行动）—— 重写版（2026-10）
//!
//! ## 设计要点
//!
//! - **规则库外置**：`resources/gun-codes/rules.json`（随包分发，可热更，编译期内嵌兜底）
//! - **三段式解析**：① 行分类 → ② 同行匹配 → ③ 跨行继承
//!   （真实文档是表格布局，枪名与码**不在同一单元格**；旧版按纯文本行匹配，必然失败）
//! - **指纹取前 10 位**：实测 606 真实码中前 6 位有 5 组冲突、前 10 位唯一
//! - **可解释结果**：返回 `confidence` / `reason` / `source`，替代静默的「未知枪械」
//!
//! ## 兼容性
//!
//! - `LocalGunCode` 结构与 8 个命令签名**保持不变**，前端调用点无需改动
//! - 旧版 6 位学习表继续可用（查表时 10 位优先、6 位降级）

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_store::StoreExt;

// ═══════════════════════ 数据结构 ═══════════════════════

/// 一条本地改枪码（**结构勿改**，与旧版持久化格式兼容）
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LocalGunCode {
    pub id: String,
    pub weapon_name: String,
    pub code: String,
    pub note: String,
    pub created_at: u64,
}

/// 单条识别结果（供前端粘贴时实时提示）
///
/// `confidence` / `reason` / `source` 为新增字段，`#[serde(default)]` 保证
/// 旧前端不传也能反序列化。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ImportResult {
    pub weapon_name: String,
    pub note: String,
    pub recognized: bool,
    /// 置信度 0.0~1.0
    #[serde(default)]
    pub confidence: f32,
    /// 人类可读的识别依据（为什么认成这把枪 / 为什么没认出来）
    #[serde(default)]
    pub reason: String,
    /// 码的来源段（后 11 位，同一渠道/批次共用）
    #[serde(default)]
    pub source: String,
}

/// 批量导入结果
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct ImportBatchResult {
    pub imported: usize,
    pub updated: usize,
    pub skipped: usize,
    /// 未能识别枪名的条数（仍会导入，枪名标为「未知枪械」）
    #[serde(default)]
    pub unrecognized: usize,
}

/// 规则库（`rules.json`）
///
/// ⚠️ 模式说明：`烽火地带` / `全面战场` / `黑潮爆破` 是三个模式，
/// 改枪码兼容性未知。本工具**仅收录烽火地带**，
/// `mode` 字段用于标注，规则库构建时已剔除另两个模式的码。
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default)]
pub struct GunRules {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub generated: String,
    /// 本规则库适用的游戏模式（当前恒为「烽火地带」）
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub source: String,
    /// 规范枪名 → 元数据
    #[serde(default)]
    pub guns: HashMap<String, GunMeta>,
    /// 前 10 位指纹 → (枪名, 配置名)
    #[serde(default)]
    pub fingerprints: HashMap<String, Fingerprint>,
    /// 表头/说明噪音关键词
    #[serde(default)]
    pub noise: Vec<String>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default)]
pub struct GunMeta {
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default)]
pub struct Fingerprint {
    #[serde(default)]
    pub gun: String,
    #[serde(default)]
    pub config: String,
    /// 该码所属游戏模式（当前恒为「烽火地带」）
    #[serde(default)]
    pub mode: String,
}

/// 解析出的一个候选码（批量导入用）
#[derive(Clone, Debug)]
pub struct ParsedCode {
    pub code: String,
    pub gun: Option<String>,
    pub config: String,
    pub confidence: f32,
    pub reason: String,
    pub source: String,
}

// ═══════════════════════ 存储层（兼容旧版） ═══════════════════════

/// 内存锁，防止并发读写
static STORE_LOCK: Mutex<()> = Mutex::new(());

fn store_path() -> &'static str {
    "local_gun_codes.json"
}

/// 读取全部本地改枪码
pub fn load_all(app: &tauri::AppHandle) -> Vec<LocalGunCode> {
    let _guard = STORE_LOCK.lock().unwrap();
    match app.store(store_path()) {
        Ok(store) => {
            if let Some(value) = store.get("items") {
                if let Ok(items) = serde_json::from_value::<Vec<LocalGunCode>>(value) {
                    return items;
                }
            }
        }
        Err(e) => log::warn!("[LocalGunCode] 打开存储失败: {e}"),
    }
    Vec::new()
}

/// 保存全部改枪码
fn save_all(app: &tauri::AppHandle, items: &[LocalGunCode]) {
    let _guard = STORE_LOCK.lock().unwrap();
    match app.store(store_path()) {
        Ok(store) => {
            store.set("items", serde_json::to_value(items).unwrap());
            if let Err(e) = store.save() {
                log::error!("[LocalGunCode] 保存失败: {e}");
            }
        }
        Err(e) => log::error!("[LocalGunCode] 打开存储失败: {e}"),
    }
}

/// 学习表存储 key（`{指纹: (枪名, 配置名)}`）
fn prefix_map_key() -> &'static str {
    "prefix_map"
}

/// 读取学习表（兼容旧版仅存枪名的格式，以及旧版 6 位指纹）
fn load_prefix_map(app: &tauri::AppHandle) -> HashMap<String, (String, String)> {
    let _guard = STORE_LOCK.lock().unwrap();
    let mut out = HashMap::new();
    if let Ok(store) = app.store(store_path()) {
        if let Some(value) = store.get(prefix_map_key()) {
            // 新格式：{ "前缀": ["枪名", "配置名"] }
            if let Ok(map) = serde_json::from_value::<HashMap<String, Vec<String>>>(value.clone()) {
                for (k, v) in map {
                    let gun = v.first().cloned().unwrap_or_default();
                    let cfg = v.get(1).cloned().unwrap_or_default();
                    if !gun.is_empty() {
                        out.insert(k.to_uppercase(), (gun, cfg));
                    }
                }
                return out;
            }
            // 旧格式：{ "前缀": "枪名" }
            if let Ok(map) = serde_json::from_value::<HashMap<String, String>>(value) {
                for (k, gun) in map {
                    if !gun.is_empty() {
                        out.insert(k.to_uppercase(), (gun, String::new()));
                    }
                }
            }
        }
    }
    out
}

/// 保存学习表
fn save_prefix_map(app: &tauri::AppHandle, map: &HashMap<String, (String, String)>) {
    let _guard = STORE_LOCK.lock().unwrap();
    if let Ok(store) = app.store(store_path()) {
        let serializable: HashMap<String, Vec<String>> = map
            .iter()
            .map(|(k, (g, c))| (k.clone(), vec![g.clone(), c.clone()]))
            .collect();
        store.set(prefix_map_key(), serde_json::to_value(serializable).unwrap());
        if let Err(e) = store.save() {
            log::error!("[LocalGunCode] 保存学习表失败: {e}");
        }
    }
}

// ═══════════════════════ 规则库 ═══════════════════════

/// 编译期内嵌兜底：resource_dir 找不到时仍可用
const RULES_EMBEDDED: &str = include_str!("../resources/gun-codes/rules.json");

/// 规则库缓存（进程内只读一次；`reload_rules` 可刷新）
static RULES_CACHE: Mutex<Option<GunRules>> = Mutex::new(None);

/// 读取规则库：优先 `resource_dir`（可热更），失败则用编译期内嵌
pub fn load_rules(app: &tauri::AppHandle) -> GunRules {
    {
        let cache = RULES_CACHE.lock().unwrap();
        if let Some(r) = cache.as_ref() {
            return r.clone();
        }
    }

    let mut loaded: Option<GunRules> = None;

    // 1) 运行时资源目录（随包分发 → 可热更）
    if let Ok(resource_dir) = app.path().resource_dir() {
        for p in [
            resource_dir.join("gun-codes").join("rules.json"),
            resource_dir.join("resources").join("gun-codes").join("rules.json"),
        ] {
            if p.exists() {
                match std::fs::read_to_string(&p) {
                    Ok(text) => match serde_json::from_str::<GunRules>(&text) {
                        Ok(r) => {
                            log::info!(
                                "[LocalGunCode] 规则库已加载: {} (v{}, {} 枪, {} 指纹)",
                                p.display(),
                                r.version,
                                r.guns.len(),
                                r.fingerprints.len()
                            );
                            loaded = Some(r);
                            break;
                        }
                        Err(e) => log::warn!("[LocalGunCode] 规则库解析失败 {}: {e}", p.display()),
                    },
                    Err(e) => log::warn!("[LocalGunCode] 规则库读取失败 {}: {e}", p.display()),
                }
            }
        }
    }

    // 2) 编译期内嵌兜底
    let rules = loaded.unwrap_or_else(|| {
        log::info!("[LocalGunCode] 使用内嵌规则库");
        serde_json::from_str::<GunRules>(RULES_EMBEDDED).unwrap_or_default()
    });

    *RULES_CACHE.lock().unwrap() = Some(rules.clone());
    rules
}

/// 强制刷新规则库缓存（热更后调用）
pub fn reload_rules(app: &tauri::AppHandle) -> GunRules {
    *RULES_CACHE.lock().unwrap() = None;
    load_rules(app)
}

/// 序列化规则库的枪械表（枪名 → 分类 + 别名）
///
/// **为什么需要**：前端曾把「枪名 → 分类」写成手写常量，与规则库脱节
/// （实测漏了 14 把枪，导致 17% 的码被归到「其他」分类）。
/// 改为由后端下发，前端不再重复维护，规则库加枪即刻生效。
///
/// 体积很小（约 3.7 KB），且 `rules.json` 本就随包分发，无额外开销。
fn guns_table(r: &GunRules) -> Vec<serde_json::Value> {
    let mut v: Vec<serde_json::Value> = r
        .guns
        .iter()
        .map(|(name, meta)| {
            serde_json::json!({
                "name": name,
                "category": meta.category,
                "aliases": meta.aliases,
            })
        })
        .collect();
    // 稳定排序，便于测试与前端 diff
    v.sort_by(|a, b| {
        a["name"]
            .as_str()
            .unwrap_or("")
            .cmp(b["name"].as_str().unwrap_or(""))
    });
    v
}

/// 规则库元信息（供前端展示「词库版本」+ 获取枪械分类表）
#[tauri::command]
pub fn get_gun_rules_meta(app: tauri::AppHandle) -> serde_json::Value {
    let r = load_rules(&app);
    let learned = load_prefix_map(&app);
    // 分类统计
    let mut cats: HashMap<String, usize> = HashMap::new();
    for g in r.guns.values() {
        *cats.entry(g.category.clone()).or_insert(0) += 1;
    }
    serde_json::json!({
        "version": r.version,
        "generated": r.generated,
        "source": r.source,
        "gunCount": r.guns.len(),
        "fingerprintCount": r.fingerprints.len(),
        "learnedCount": learned.len(),
        "categories": cats,
        // 枪械分类表（前端据此分类，替代手写常量）
        "guns": guns_table(&r),
    })
}

/// 刷新规则库命令（前端「更新词库」按钮）
#[tauri::command]
pub fn refresh_gun_rules(app: tauri::AppHandle) -> serde_json::Value {
    let r = reload_rules(&app);
    serde_json::json!({
        "ok": true,
        "gunCount": r.guns.len(),
        "fingerprintCount": r.fingerprints.len(),
        "version": r.version,
        // 一并返回枪械表，保证「刷新词库」后前端分类同步更新
        "guns": guns_table(&r),
    })
}

// ═══════════════════════ 归一化 ═══════════════════════

/// 归一化：去空格 / 连字符 / 下划线，转大写
///
/// 用于解决「同一把枪分裂成多个筛选项」：`AK-12` 与 `AK12` 归一后相同。
pub fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace() && *c != '-' && *c != '_')
        .collect::<String>()
        .to_uppercase()
}

/// 构建「归一化别名 → 规范枪名」索引（规则库 + 学习表）
fn build_alias_index(app: &tauri::AppHandle, rules: &GunRules) -> HashMap<String, String> {
    let mut idx: HashMap<String, String> = HashMap::new();
    for (canon, meta) in &rules.guns {
        idx.entry(normalize(canon)).or_insert_with(|| canon.clone());
        for a in &meta.aliases {
            idx.entry(normalize(a)).or_insert_with(|| canon.clone());
        }
    }
    // 指纹里出现的枪名也纳入（防止规则库 guns 段漏收）
    for fp in rules.fingerprints.values() {
        if !fp.gun.is_empty() {
            idx.entry(normalize(&fp.gun)).or_insert_with(|| fp.gun.clone());
        }
    }
    // 学习表里的枪名同样纳入
    for (_, (gun, _)) in load_prefix_map(app) {
        if !gun.is_empty() {
            idx.entry(normalize(&gun)).or_insert(gun);
        }
    }
    idx
}

/// 武器型号后缀 / 修饰词（迭代剥离用）
///
/// 属**结构性知识**（游戏内武器分类词的构词法），非枪名数据，
/// 因此硬编码在内核；枪名本身一律来自规则库。
///
/// 用于处理「枪名 + 型号后缀紧贴」的文档格式，例如：
/// - `AR57突击步枪`          → 剥「突击步枪」→ `AR57`
/// - `SR-3M紧凑突击步枪`     → 剥「紧凑」+「突击步枪」→ `SR-3M`
/// - `M250通用机枪`          → 剥「通用机枪」→ `M250`
const WEAPON_SUFFIXES: &[&str] = &[
    "突击步枪",
    "战斗步枪",
    "狙击步枪",
    "精确射手步枪",
    "射手步枪",
    "冲锋枪",
    "轻机枪",
    "通用机枪",
    "机枪",
    "霰弹枪",
    "手枪",
    "步枪",
    // 修饰词（可与型号后缀叠加）
    "紧凑",
    "精确",
    "战术",
];

/// ③ 前缀 / ④ 子串匹配的最小长度门槛
///
/// 低于此长度的枪名（如「弓」只有 1 字）**只走整体等于与别名匹配**，
/// 不参与前缀/子串匹配 —— 否则 `弓弩`（分类词）会因以「弓」开头而被误判。
/// 短枪名的变体（如「复合弓」）一律通过**规则库别名**补全，而非放宽匹配。
const SUBSTR_MIN_LEN: usize = 3;

/// 把任意写法归一到规范枪名（**四段式匹配**）
///
/// 按优先级依次尝试，命中即返回：
///   ① 整体等于           `AR57`          → `AR57`
///   ② 迭代剥离型号后缀    `AR57突击步枪`  → `AR57`
///   ③ 前缀匹配（最长优先）`QJB201轻机枪`  → `QJB`
///   ④ 子串匹配（最长优先）`Marlin杠杆步枪`→ `杠杆步枪`
///
/// ⚠️ ③④ 一律**按归一化长度降序**遍历，保证最长匹配优先，
/// 否则 `M700` 会命中 `M7`、`M1014` 会命中 `M10`（张冠李戴）。
fn canonical_gun(alias_idx: &HashMap<String, String>, raw: &str) -> Option<String> {
    let n = normalize(raw);
    if n.is_empty() {
        return None;
    }

    // ① 整体等于
    if let Some(g) = alias_idx.get(&n) {
        return Some(g.clone());
    }

    // ② 迭代剥离型号后缀 / 修饰词（最多 4 轮，覆盖「紧凑突击步枪」这类叠加）
    //
    // ⚠️ 必须用**字符数**而非字节数：中文一字 3 字节，若用 `str::len()`（字节）
    //    判断，`"弓弩"(6B)` 会被误判为以 `"弓"(3B)` 结尾，错误剥离出 `"弓"`，
    //    导致 `弓弩`（分类词）被误配成枪名 `弓`。
    let mut stripped = n.clone();
    for _ in 0..4 {
        let before = stripped.clone();
        // 按字符长度降序尝试，保证「精确射手步枪」优先于「步枪」
        for suf in sorted_suffixes() {
            let sl = suf.chars().count();
            let cl = stripped.chars().count();
            if cl > sl && stripped.ends_with(suf.as_str()) {
                // 按字符数换算成字节边界再截断（`truncate` 是字节语义）
                let keep_bytes = stripped
                    .char_indices()
                    .nth(cl - sl)
                    .map(|(i, _)| i)
                    .unwrap_or_else(|| stripped.len());
                stripped.truncate(keep_bytes);
                break;
            }
        }
        if stripped == before {
            break; // 无可剥离，退出
        }
        if let Some(g) = alias_idx.get(&stripped) {
            return Some(g.clone());
        }
    }

    // ③ 前缀匹配（token 以枪名开头），最长优先
    // ④ 子串匹配（枪名出现在 token 中），最长优先
    // 两者共用一次「长度降序」遍历，且**都受最小长度门槛约束**：
    // 短名（如「弓」）只能靠整体等于或别名命中，避免「弓弩」被误判。
    let mut best_substr: Option<&String> = None;
    let mut best_substr_len = 0usize;
    for (cand, canon) in sorted_alias_entries(alias_idx) {
        let cand_len = cand.chars().count(); // 字符数，非字节
        if cand_len < SUBSTR_MIN_LEN {
            continue;
        }
        if n.starts_with(cand.as_str()) {
            return Some(canon.clone());
        }
        if cand_len > best_substr_len && n.contains(cand.as_str()) {
            best_substr = Some(canon);
            best_substr_len = cand_len;
        }
    }
    best_substr.cloned()
}

/// 后缀词表：按归一化后**字符数**降序（避免「步枪」抢先匹配「突击步枪」）
fn sorted_suffixes() -> Vec<String> {
    let mut v: Vec<String> = WEAPON_SUFFIXES.iter().map(|s| normalize(s)).collect();
    v.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()));
    v
}

// ═══════════════ 测试用公开包装（供 tests/ 集成测试调用）═══════════════
//
// 集成测试（`tests/*.rs`）属于独立 crate，无法访问 `pub(crate)` 项。
// 这些包装仅转发到内部实现，不含额外逻辑。

/// 测试用：四段式枪名匹配（见 [`canonical_gun`]）
pub fn canonical_gun_for_test(
    alias_idx: &HashMap<String, String>,
    raw: &str,
) -> Option<String> {
    canonical_gun(alias_idx, raw)
}

/// 测试用：从流式文本片段提取枪名（见 [`match_gun_in_text`]）
pub fn match_gun_in_text_for_test(
    alias_idx: &HashMap<String, String>,
    text: &str,
) -> Option<String> {
    match_gun_in_text(alias_idx, text)
}

/// 测试用：按 `烽火地带-<码>` 锚点分段（见 [`split_by_code_anchor`]）
pub fn split_by_code_anchor_for_test(line: &str) -> Vec<(String, String)> {
    split_by_code_anchor(line)
}

/// 别名索引条目：按 key **字符数**降序（保证最长匹配优先，防张冠李戴）
///
/// 用字符数而非字节数：中文字符 3 字节，按字节排序会让中文名排在英文名之前，
/// 破坏「最长优先」的语义（`杠杆步枪` 应优先于 `弓`）。
fn sorted_alias_entries(alias_idx: &HashMap<String, String>) -> Vec<(&String, &String)> {
    let mut v: Vec<(&String, &String)> = alias_idx.iter().collect();
    v.sort_by(|a, b| b.0.chars().count().cmp(&a.0.chars().count()));
    v
}

// ═══════════════════════ 码与指纹 ═══════════════════════

/// 是否为疑似改枪码：20~32 位、纯字母数字、且**同时含字母和数字**
///
/// 另加一条判别：排除**纯十六进制**串（仅 `0-9A-F`）。
/// 实测：WPS 表格的 `DISPIMG("ID_<32位hex>")` 图片 ID 会混入文档，
/// 它们是全 24 份真实文档里**唯二的纯十六进制候选**；真实改枪码总含 G-Z 字母。
pub fn is_code_token(t: &str) -> bool {
    let n = t.len();
    if !(20..=32).contains(&n) {
        return false;
    }
    if !t.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    let has_alpha = t.chars().any(|c| c.is_ascii_alphabetic());
    let has_digit = t.chars().any(|c| c.is_ascii_digit());
    if !(has_alpha && has_digit) {
        return false;
    }
    // 排除纯十六进制（图片 ID / 哈希），真实码含 G-Z
    let only_hex = t
        .chars()
        .all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c.to_ascii_uppercase()));
    !only_hex
}

/// 提取指纹：前 10 位字母数字（转大写）。
///
/// 实测 606 真实码：前 6 位有 5 组冲突、前 10 位唯一。
pub fn extract_fingerprint(code: &str) -> String {
    let clean: String = code.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let take = clean.len().min(10);
    clean.chars().take(take).collect::<String>().to_uppercase()
}

/// 提取来源段：指纹之后的剩余部分（同渠道/批次的码共用同一来源段）
pub fn extract_source(code: &str) -> String {
    let clean: String = code.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if clean.len() <= 10 {
        return String::new();
    }
    clean.chars().skip(10).collect::<String>().to_uppercase()
}

/// 在一个单元格内找出嵌入的码（形如 `枪名-烽火地带-6HN54R8...`）
fn find_code_in_cell(cell: &str) -> Option<String> {
    cell.split(|c: char| !c.is_ascii_alphanumeric())
        .find(|t| is_code_token(t))
        .map(|s| s.to_string())
}

// ═══════════════════════ 三段式解析器 ═══════════════════════

/// 行类型
#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) enum RowKind {
    /// 表头 / 分类标题（含「区」「哈夫币」等）→ 重置继承
    Header,
    /// 说明 / 广告行 → 跳过
    Noise,
    /// 含改枪码的数据行
    Data,
}

/// 切分一行单元格：制表符优先（Excel 粘贴即 TSV），否则整行视为一格
pub(crate) fn split_cells(line: &str) -> Vec<String> {
    if line.contains('\t') {
        line.split('\t').map(|s| s.trim().to_string()).collect()
    } else {
        vec![line.trim().to_string()]
    }
}

/// 判定行类型
pub(crate) fn classify_row(cells: &[String], rules: &GunRules, has_code: bool) -> RowKind {
    if has_code {
        return RowKind::Data;
    }
    let joined = cells.join(" ");
    let has_noise = rules
        .noise
        .iter()
        .any(|k| !k.is_empty() && joined.contains(k.as_str()));
    // 含噪音关键词的短行 → 表头（用于重置继承）
    if has_noise && joined.chars().count() <= 40 {
        return RowKind::Header;
    }
    // 长文本 → 说明/广告
    if joined.chars().count() > 40 {
        return RowKind::Noise;
    }
    RowKind::Noise
}

/// 该行是否为「整套导入码」（一键导入一整套改枪）——这类码**不属于任何单把枪**，
/// 不应参与跨行继承，否则会错误继承上一行的枪名。
///
/// 识别特征：行内含「一键」「整套」「套件」「均衡版」「高手版」等词，
/// 或码以 `BLAST` 开头（实测上游套件码前缀）。
pub(crate) fn is_bundle_row(cells: &[String], rules: &GunRules) -> bool {
    const BUNDLE_KW: [&str; 5] = ["一键", "整套", "套件", "均衡版", "高手版"];
    let joined = cells.join(" ");
    if BUNDLE_KW.iter().any(|k| joined.contains(k)) {
        return true;
    }
    // 规则库噪音词若含「一键导入」也可命中
    if rules.noise.iter().any(|k| k.contains("一键") && joined.contains(k.as_str())) {
        return true;
    }
    cells
        .iter()
        .filter(|c| is_code_token(c))
        .any(|c| c.to_uppercase().starts_with("BLAST"))
}

/// 是否为其他模式（全面战场 / 黑潮爆破）的码。
///
/// ⚠️ 三个模式的改枪码**兼容性未知**，本工具仅收录烽火地带。
/// 判定依据：
///   1. 规则库显式标注 `mode` 且非烽火地带
///   2. 行内出现「黑潮」「全面战场」「战场码」等模式词
///
/// 返回 `Some(模式名)` 表示应跳过该码。
pub(crate) fn foreign_mode_of_row(cells: &[String], rules: &GunRules) -> Option<String> {
    const OTHER_MODES: [(&str, &[&str]); 2] = [
        ("黑潮爆破", &["黑潮"]),
        ("全面战场", &["全面战场", "战场码", "战场改枪"]),
    ];
    let joined = cells.join(" ");
    for (name, kws) in OTHER_MODES {
        if kws.iter().any(|k| joined.contains(k)) {
            return Some(name.to_string());
        }
    }
    // 规则库整体标注为其他模式时，一律视为外来模式
    if !rules.mode.is_empty() && rules.mode != "烽火地带" {
        return Some(rules.mode.clone());
    }
    None
}

/// ② 同行匹配：在单元格里找枪名
///
/// 用 [`canonical_gun`] 做四段式匹配，因此同时支持两种文档形态：
/// - **xlsx/TSV**：枪名是独立单元格（`"AR57"`）
/// - **PDF/流式**：枪名带型号后缀紧贴码（`"AR57突击步枪-烽火地带-xxx"`）
///
/// 取**归一化长度最长**的命中，避免短名抢先（`M7` 抢 `M700`）。
fn match_gun_in_row(alias_idx: &HashMap<String, String>, cells: &[String]) -> Option<String> {
    let mut best: Option<String> = None;
    let mut best_len = 0usize;
    for c in cells {
        if c.is_empty() {
            continue;
        }
        if is_code_token(c) {
            continue;
        }
        // 长度上限放宽到 32：覆盖 `SR-3M紧凑突击步枪`（16 字）这类带后缀的写法；
        // 更长的串（说明文本）由后续的子串匹配自然筛掉。
        if c.chars().count() > 32 {
            continue;
        }
        if let Some(g) = canonical_gun(alias_idx, c) {
            let l = normalize(&g).chars().count();
            if l > best_len {
                best_len = l;
                best = Some(g);
            }
        }
    }
    best
}

/// 从一段文本（无制表符的流式行）中提取枪名
///
/// 按空格分片后，除单片外**额外尝试相邻两片拼接**——
/// 解决 `AS Val`、`SR-3M` 这类含空格/连字符的枪名被空格拆散的问题。
/// 从右往左优先（离码越近的片越可能是本段枪名）。
pub(crate) fn match_gun_in_text(alias_idx: &HashMap<String, String>, text: &str) -> Option<String> {
    let parts: Vec<&str> = text.split_whitespace().filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return None;
    }

    // 候选：单片 + 相邻两片拼接
    let mut cands: Vec<String> = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        cands.push((*p).to_string());
        if i + 1 < parts.len() {
            cands.push(format!("{}{}", p, parts[i + 1]));
        }
    }

    let mut best: Option<String> = None;
    let mut best_len = 0usize;
    // 从右往左（离码更近者优先）
    for c in cands.iter().rev() {
        let trimmed = c.trim_end_matches('-');
        if trimmed.is_empty() || is_code_token(trimmed) {
            continue;
        }
        if let Some(g) = canonical_gun(alias_idx, trimmed) {
            let l = normalize(&g).chars().count();
            if l > best_len {
                best_len = l;
                best = Some(g);
            }
        }
    }
    best
}

/// 按 `烽火地带-<码>` 为锚，把流式行切成 `[(码, 该段左侧文本)]`
///
/// 用途：PDF / 贴吧文本这类**无制表符**的格式里，一行可能含多个码
/// （如双栏排版被拉平）。若整行一起匹配枪名，第二段会错误继承第一段的枪名。
/// 按码锚点分段后，每段只用自己的左侧文本匹配，杜绝跨段串枪。
pub(crate) fn split_by_code_anchor(line: &str) -> Vec<(String, String)> {
    const ANCHOR: &str = "烽火地带-";
    let mut out: Vec<(String, String)> = Vec::new();
    let mut last_end = 0usize;

    while let Some(rel) = line[last_end..].find(ANCHOR) {
        let anchor_at = last_end + rel;
        let after = anchor_at + ANCHOR.len();
        // 锚点之后取连续的字母数字作为码
        let code: String = line[after..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        if is_code_token(&code) {
            // 段左侧文本 = 上一码结束 ~ 本锚点之前
            let left = line[last_end..anchor_at].trim().to_string();
            out.push((code.clone(), left));
            last_end = after + code.len();
        } else {
            last_end = after;
        }
    }
    out
}

/// 提取配置名：优先取「紧邻首个码左侧」的短单元格，其次取右侧
fn extract_config(
    alias_idx: &HashMap<String, String>,
    cells: &[String],
    first_code_idx: usize,
) -> String {
    // 左邻：通常是「36w」「红点38w」这类描述
    for j in (0..first_code_idx).rev() {
        let t = &cells[j];
        if t.is_empty() || is_code_token(t) {
            continue;
        }
        if canonical_gun(alias_idx, t).is_some() {
            continue;
        }
        if t.chars().count() <= 24 {
            return t.clone();
        }
    }
    // 右邻
    for t in cells.iter().skip(first_code_idx + 1) {
        if t.is_empty() || is_code_token(t) {
            continue;
        }
        if canonical_gun(alias_idx, t).is_some() {
            continue;
        }
        if t.chars().count() <= 24 {
            return t.clone();
        }
    }
    String::new()
}

/// 解析一段文档文本 → 候选码列表
///
/// 三段式：① 行分类 → ② 同行匹配 → ③ 跨行继承
pub fn parse_document(app: &tauri::AppHandle, text: &str) -> Vec<ParsedCode> {
    let rules = load_rules(app);
    let alias_idx = build_alias_index(app, &rules);
    let learned = load_prefix_map(app);

    let mut out: Vec<ParsedCode> = Vec::new();
    let mut carry: Option<String> = None;
    // 粘性分区模式：表头声明后，后续数据行沿用（`None` = 烽火地带）
    let mut current_mode: Option<String> = None;
    let mut seen: HashSet<String> = HashSet::new();

    for raw_line in text.lines() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        let mut cells = split_cells(line);

        // 单元格内嵌码（如 `M14-烽火地带-6JCB1FO...`）拆出来：
        // 先把码替换出来，前缀文本作为额外单元格参与枪名匹配
        let mut extra: Vec<String> = Vec::new();
        for c in cells.iter_mut() {
            if is_code_token(c) {
                continue;
            }
            if let Some(code) = find_code_in_cell(c) {
                let prefix = c.replace(&code, " ");
                let prefix = prefix.trim().to_string();
                *c = code;
                if !prefix.is_empty() {
                    extra.push(prefix);
                }
            }
        }
        cells.extend(extra);

        let code_positions: Vec<usize> = cells
            .iter()
            .enumerate()
            .filter(|(_, c)| is_code_token(c))
            .map(|(i, _)| i)
            .collect();

        // ① 行分类
        let kind = classify_row(&cells, &rules, !code_positions.is_empty());
        match kind {
            RowKind::Header => {
                // 表头/新分区 → 重置继承，避免跨区串枪
                carry = None;
                // 但表头可能是「模式分区标记」（如「黑潮爆破改枪码大全」）：
                // 记住当前分区模式，后续数据行按它过滤。
                // 实测：模式标题只在分区开头出现一次，其后几十行数据都不含模式词，
                // 故必须用**粘性分区状态**，不能逐行判断。
                if let Some(m) = foreign_mode_of_row(&cells, &rules) {
                    current_mode = Some(m);
                } else if cells.iter().any(|c| {
                    c.contains("烽火")
                }) {
                    current_mode = None;
                }
                continue;
            }
            RowKind::Noise => continue,
            RowKind::Data => {}
        }

        // ⚠️ 模式过滤（粘性）：全面战场 / 黑潮爆破 与烽火地带兼容性未知。
        // 当前分区被标记为其他模式时，整段数据行跳过。
        let row_mode = foreign_mode_of_row(&cells, &rules).or_else(|| current_mode.clone());
        if let Some(m) = row_mode {
            log::debug!("[LocalGunCode] 跳过非烽火地带内容（{m}）：{}", cells.join(" | "));
            carry = None;
            current_mode = Some(m);
            continue;
        }

        // ② 同行匹配
        let is_bundle = is_bundle_row(&cells, &rules);
        let row_gun = match_gun_in_row(&alias_idx, &cells);
        if row_gun.is_some() {
            carry = row_gun.clone();
        }
        // ③ 跨行继承（整套套件码不继承，它不属于任何单把枪）
        let use_gun = if is_bundle {
            None
        } else {
            row_gun.clone().or_else(|| carry.clone())
        };

        // 锚点分段：把无制表符的流式行按 `烽火地带-<码>` 切成多段，
        // 每段独立解析枪名 —— 防止「双码同行」时第二段错继承第一段的枪名。
        // 有制表符（xlsx 粘贴）时 cells 已是列，无需分段。
        let segments = if line.contains('\t') {
            Vec::new()
        } else {
            split_by_code_anchor(line)
        };

        let first_idx = *code_positions.first().unwrap();
        let row_config = extract_config(&alias_idx, &cells, first_idx);

        for &ci in &code_positions {
            let code = cells[ci].clone();
            let fp = extract_fingerprint(&code);

            // 段内枪名（仅流式行）：比整行匹配更精确
            let seg_gun = segments
                .iter()
                .find(|(c, _)| c == &code)
                .and_then(|(_, left)| match_gun_in_text(&alias_idx, left));

            if !seen.insert(fp.clone()) {
                // 同一指纹在同一份文档里重复出现 → 后者跳过（去重）
                continue;
            }

            // 识别优先级：学习表(10位) > 规则库(10位) > 学习表(6位,旧) > 枪名
            let mut gun = seg_gun.clone().or_else(|| use_gun.clone());
            let mut config = row_config.clone();
            let mut confidence = if is_bundle {
                0.0
            } else if row_gun.is_some() {
                0.95
            } else {
                0.8
            };
            let mut reason = if is_bundle {
                "整套套件码（不属于单把枪）".to_string()
            } else if seg_gun.is_some() {
                "段内匹配到枪名".to_string()
            } else if row_gun.is_some() {
                "同行单元格匹配到枪名".to_string()
            } else if carry.is_some() {
                "继承上一行的枪名".to_string()
            } else {
                String::new()
            };

            if let Some((g, c)) = learned.get(&fp) {
                gun = Some(g.clone());
                if !c.is_empty() {
                    config = c.clone();
                }
                confidence = 1.0;
                reason = "学习表命中（前10位指纹）".to_string();
            } else if let Some(f) = rules.fingerprints.get(&fp) {
                gun = Some(f.gun.clone());
                if !f.config.is_empty() {
                    config = f.config.clone();
                }
                confidence = 1.0;
                reason = "规则库命中（前10位指纹）".to_string();
            } else if let Some((g, c)) = learned.get(&fp.chars().take(6).collect::<String>()) {
                // 旧版 6 位学习数据降级匹配
                gun = Some(g.clone());
                if !c.is_empty() {
                    config = c.clone();
                }
                confidence = 0.7;
                reason = "旧版 6 位学习表命中（建议重新学习）".to_string();
            } else if gun.is_none() {
                reason = "未能识别枪名（可手动补充后自动学习）".to_string();
                confidence = 0.0;
            }

            out.push(ParsedCode {
                source: extract_source(&code),
                code,
                gun,
                config,
                confidence,
                reason,
            });
        }
    }

    out
}

/// 从代码本身识别枪名 + 配置名（含表格解析兜底）
///
/// 返回 `(枪名, 配置名, 置信度, 依据, 来源段)`
pub fn recognize_weapon(app: &tauri::AppHandle, code: &str) -> Option<(String, String, f32, String, String)> {
    let rules = load_rules(app);
    let fp = extract_fingerprint(code);
    let src = extract_source(code);

    // 1. 学习表（10 位）——用户手动确认过的，最可信
    let learned = load_prefix_map(app);
    if let Some((g, c)) = learned.get(&fp) {
        return Some((g.clone(), c.clone(), 1.0, "学习表命中（前10位指纹）".into(), src));
    }
    // 2. 规则库（10 位）
    if let Some(f) = rules.fingerprints.get(&fp) {
        return Some((f.gun.clone(), f.config.clone(), 1.0, "规则库命中（前10位指纹）".into(), src));
    }
    // 3. 旧版 6 位学习表降级
    let fp6: String = fp.chars().take(6).collect();
    if let Some((g, c)) = learned.get(&fp6) {
        return Some((g.clone(), c.clone(), 0.7, "旧版6位学习表命中".into(), src));
    }
    // 4. 码内嵌格式兜底：`DELTA-M4A1-001`
    let upper = code.to_uppercase();
    if let Some(start) = upper.find("DELTA-") {
        let rest = &upper[start + 6..];
        if let Some(end) = rest.find('-') {
            let cand = &rest[..end];
            if !cand.is_empty() && cand.chars().count() <= 20 {
                return Some((cand.to_string(), String::new(), 0.5, "从 DELTA- 格式解析".into(), src));
            }
        }
    }
    None
}

/// 学习：记住指纹对应的 (枪名, 配置名)，写入 10 位指纹
fn learn_fingerprint(app: &tauri::AppHandle, code: &str, weapon_name: &str, note: &str) {
    if weapon_name.trim().is_empty() || weapon_name.trim() == "未知枪械" {
        return;
    }
    let fp = extract_fingerprint(code);
    if fp.len() < 10 {
        return;
    }
    let mut map = load_prefix_map(app);
    map.insert(fp.clone(), (weapon_name.trim().to_string(), note.trim().to_string()));
    save_prefix_map(app, &map);
    log::info!("[LocalGunCode] 已学习指纹 {fp} → {weapon_name} / {note}");
}

// ═══════════════════════ 命令（8 个，签名不变） ═══════════════════════

/// 获取全部本地改枪码
#[tauri::command]
pub fn get_local_gun_codes(app: tauri::AppHandle) -> Vec<LocalGunCode> {
    let mut items = load_all(&app);
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    items
}

/// 导入（新增）一条改枪码
#[tauri::command]
pub fn add_local_gun_code(
    app: tauri::AppHandle,
    weapon_name: String,
    code: String,
    note: String,
) -> Result<LocalGunCode, String> {
    if code.trim().is_empty() {
        return Err("改枪码不能为空".to_string());
    }

    let mut weapon = weapon_name.trim().to_string();
    let mut note = note.trim().to_string();

    // 枪名或备注为空时用识别结果补齐
    if weapon.is_empty() || note.is_empty() {
        if let Some((w, n, _, _, _)) = recognize_weapon(&app, &code) {
            if weapon.is_empty() {
                weapon = w;
            }
            if note.is_empty() {
                note = n;
            }
        }
    }
    if weapon.is_empty() {
        weapon = "未知枪械".to_string();
    }

    learn_fingerprint(&app, &code, &weapon, &note);

    let item = LocalGunCode {
        id: format!("{:x}", uuid::Uuid::new_v4().simple()),
        weapon_name: weapon,
        code: code.trim().to_string(),
        note,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    };

    let mut items = load_all(&app);
    items.push(item.clone());
    save_all(&app, &items);
    Ok(item)
}

/// 删除一条改枪码
#[tauri::command]
pub fn delete_local_gun_code(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let mut items = load_all(&app);
    let before = items.len();
    items.retain(|i| i.id != id);
    if items.len() == before {
        return Err("未找到该改枪码".to_string());
    }
    save_all(&app, &items);
    Ok(())
}

/// 更新一条改枪码
#[tauri::command]
pub fn update_local_gun_code(
    app: tauri::AppHandle,
    id: String,
    weapon_name: String,
    code: String,
    note: String,
) -> Result<(), String> {
    let mut items = load_all(&app);
    if let Some(item) = items.iter_mut().find(|i| i.id == id) {
        if !code.trim().is_empty() {
            item.code = code.trim().to_string();
        }
        if !weapon_name.trim().is_empty() {
            item.weapon_name = weapon_name.trim().to_string();
        }
        item.note = note.trim().to_string();
        let (c, w, n) = (item.code.clone(), item.weapon_name.clone(), item.note.clone());
        save_all(&app, &items);
        // 用户修正过 → 学习
        learn_fingerprint(&app, &c, &w, &n);
        Ok(())
    } else {
        Err("未找到该改枪码".to_string())
    }
}

/// 识别改枪码中的枪名 + 配置名（供前端粘贴时实时提示）
#[tauri::command]
pub fn recognize_gun_name(app: tauri::AppHandle, code: String) -> ImportResult {
    match recognize_weapon(&app, &code) {
        Some((w, n, conf, reason, src)) => ImportResult {
            weapon_name: w,
            note: n,
            recognized: true,
            confidence: conf,
            reason,
            source: src,
        },
        None => ImportResult {
            weapon_name: "未知枪械".to_string(),
            note: String::new(),
            recognized: false,
            confidence: 0.0,
            reason: "未能识别枪名（可手动补充后自动学习）".to_string(),
            source: extract_source(&code),
        },
    }
}

/// 批量导入：从文档文本中解析全部改枪码（**三段式解析**）
#[tauri::command]
pub fn import_gun_codes_batch(
    app: tauri::AppHandle,
    text: String,
) -> Result<ImportBatchResult, String> {
    let parsed = parse_document(&app, &text);

    let mut result = ImportBatchResult::default();
    let mut items = load_all(&app);
    let mut existing: HashMap<String, usize> = HashMap::new();
    for (i, it) in items.iter().enumerate() {
        existing.insert(it.code.trim().to_uppercase(), i);
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut changed = false;

    for p in parsed {
        let key = p.code.trim().to_uppercase();
        let gun = p.gun.clone().unwrap_or_else(|| "未知枪械".to_string());
        if p.gun.is_none() {
            result.unrecognized += 1;
        }
        if let Some(&idx) = existing.get(&key) {
            // 已存在：补空备注 / 补未知枪名
            let it = &mut items[idx];
            let mut upd = false;
            if it.note.is_empty() && !p.config.is_empty() {
                it.note = p.config.clone();
                upd = true;
            }
            if it.weapon_name == "未知枪械" && gun != "未知枪械" {
                it.weapon_name = gun.clone();
                upd = true;
            }
            if upd {
                result.updated += 1;
                changed = true;
            } else {
                result.skipped += 1;
            }
        } else {
            // 用户已确认过的枪名 → 学习其指纹
            if p.gun.is_some() {
                learn_fingerprint(&app, &p.code, &gun, &p.config);
            }
            items.push(LocalGunCode {
                id: format!("{:x}", uuid::Uuid::new_v4().simple()),
                weapon_name: gun,
                code: p.code.clone(),
                note: p.config.clone(),
                created_at: now,
            });
            existing.insert(key, items.len() - 1);
            result.imported += 1;
            changed = true;
        }
    }

    if changed {
        save_all(&app, &items);
    }
    Ok(result)
}

/// 预览：解析文档但**不写库**（供前端「导入预览流」展示）
#[tauri::command]
pub fn preview_import_gun_codes(app: tauri::AppHandle, text: String) -> serde_json::Value {
    let parsed = parse_document(&app, &text);
    let existing: HashSet<String> = load_all(&app)
        .iter()
        .map(|i| i.code.trim().to_uppercase())
        .collect();

    let rows: Vec<serde_json::Value> = parsed
        .iter()
        .map(|p| {
            let key = p.code.trim().to_uppercase();
            serde_json::json!({
                "code": p.code,
                "gun": p.gun.clone().unwrap_or_default(),
                "config": p.config,
                "recognized": p.gun.is_some(),
                "confidence": p.confidence,
                "reason": p.reason,
                "source": p.source,
                "duplicate": existing.contains(&key),
            })
        })
        .collect();

    let recognized = parsed.iter().filter(|p| p.gun.is_some()).count();
    serde_json::json!({
        "total": parsed.len(),
        "recognized": recognized,
        "unrecognized": parsed.len() - recognized,
        "rows": rows,
    })
}

// ═══════════════════════ 导出 / 导入备份 ═══════════════════════

/// 导出全部改枪码为 JSON 文件
#[tauri::command]
pub fn export_gun_codes(app: tauri::AppHandle, path: String) -> Result<usize, String> {
    let items = load_all(&app);
    let json = serde_json::to_string_pretty(&items).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("写入失败: {e}"))?;
    Ok(items.len())
}

/// 从 JSON 文件导入改枪码（按代码去重，合并到现有数据）
#[tauri::command]
pub fn import_gun_codes(app: tauri::AppHandle, path: String) -> Result<usize, String> {
    let content = std::fs::read_to_string(&path).map_err(|e| format!("读取失败: {e}"))?;
    let incoming: Vec<LocalGunCode> =
        serde_json::from_str(&content).map_err(|e| format!("JSON 解析失败: {e}"))?;

    let mut items = load_all(&app);
    let mut existing_codes: HashSet<String> = items
        .iter()
        .map(|i| i.code.trim().to_uppercase())
        .collect();

    let mut added = 0usize;
    let mut changed = false;
    for item in incoming {
        let key = item.code.trim().to_uppercase();
        if key.is_empty() {
            continue;
        }
        if !existing_codes.contains(&key) {
            learn_fingerprint(&app, &item.code, &item.weapon_name, &item.note);
            items.push(item);
            existing_codes.insert(key);
            added += 1;
            changed = true;
        } else if let Some(e) = items
            .iter_mut()
            .find(|i| i.code.trim().to_uppercase() == key)
        {
            let mut upd = false;
            if e.note.is_empty() && !item.note.is_empty() {
                e.note = item.note.clone();
                upd = true;
            }
            if e.weapon_name == "未知枪械" && item.weapon_name != "未知枪械" {
                e.weapon_name = item.weapon_name.clone();
                upd = true;
            }
            if upd {
                changed = true;
            }
        }
    }
    if changed {
        save_all(&app, &items);
    }
    Ok(added)
}

// ═══════════════════════ 单元测试 ═══════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个轻量别名索引（不依赖 AppHandle），模拟规则库加载结果
    fn idx_from(pairs: &[(&str, &[&str])]) -> HashMap<String, String> {
        let mut m = HashMap::new();
        for (canon, aliases) in pairs {
            m.insert(normalize(canon), canon.to_string());
            for a in *aliases {
                m.insert(normalize(a), canon.to_string());
            }
        }
        m
    }

    /// 典型测试索引（覆盖长短名冲突、变体、中文名、别名）
    fn test_idx() -> HashMap<String, String> {
        idx_from(&[
            ("AR57", &[]),
            ("MK47", &[]),
            ("M7", &[]),
            ("M700", &[]),
            ("M1014", &[]),
            ("M1911", &[]),
            ("M249", &[]),
            ("M250", &[]),
            ("M16A4", &[]),
            ("M4A1", &[]),
            ("SR-3M", &["SR3M"]),
            ("AS Val", &["ASVAL"]),
            ("QJB", &["QJB201"]),
            ("QBZ-95", &["QBZ95-1"]),
            ("杠杆步枪", &[]),
            ("弓", &["复合弓"]),
            ("野牛", &[]),
            ("P90", &[]),
            ("PKM", &[]),
            ("KC17", &[]),
        ])
    }

    // ═══════ 新增：四段式枪名匹配（PDF / 流式格式支持）═══════

    #[test]
    fn test_four_stage_match_pdf_style() {
        // 覆盖 PDF 文档里「枪名 + 型号后缀紧贴」的全部形态
        let idx = test_idx();
        let cases: &[(&str, &str)] = &[
            ("AR57突击步枪", "AR57"),           // ② 剥「突击步枪」
            ("MK47突击步枪", "MK47"),
            ("SR-3M紧凑突击步枪", "SR-3M"),      // ② 剥「紧凑」+「突击步枪」两段
            ("AS Val突击步枪", "AS Val"),        // ② 含空格枪名
            ("QJB201轻机枪", "QJB"),             // ③ 前缀匹配
            ("QBZ95-1突击步枪", "QBZ-95"),       // ③ 前缀（归一化后）
            ("M250通用机枪", "M250"),
            ("Marlin杠杆步枪", "杠杆步枪"),       // ④ 子串匹配
            ("野牛冲锋枪", "野牛"),
            ("P90冲锋枪", "P90"),
            ("PKM通用机枪", "PKM"),
            ("KC17突击步枪", "KC17"),
            ("复合弓", "弓"),                    // 别名（数据补全，非放宽门槛）
            ("AR57", "AR57"),                    // ① 整体等于
        ];
        for (input, expect) in cases {
            let got = canonical_gun(&idx, input);
            assert_eq!(
                got.as_deref(),
                Some(*expect),
                "canonical_gun({input:?}) 期望 {expect:?}，实得 {got:?}"
            );
        }
    }

    #[test]
    fn test_match_longest_first_no_confusion() {
        // 关键防张冠李戴：短名不得抢先命中长名
        let idx = test_idx();
        let cases: &[(&str, &str)] = &[
            ("M700", "M700"),
            ("M1014", "M1014"),
            ("M1911", "M1911"),
            ("M249", "M249"),
            ("M250", "M250"),
            ("M16A4", "M16A4"),
            ("M4A1", "M4A1"),
            ("M7", "M7"),
            // 带后缀时同样要选最长的
            ("M700狙击步枪", "M700"),
            ("M1014霰弹枪", "M1014"),
            ("M4A1突击步枪", "M4A1"),
        ];
        for (input, expect) in cases {
            let got = canonical_gun(&idx, input);
            assert_eq!(
                got.as_deref(),
                Some(*expect),
                "canonical_gun({input:?}) 期望 {expect:?}，实得 {got:?}（疑似短名抢先）"
            );
        }
    }

    #[test]
    fn test_no_false_positive_on_noise() {
        // 噪音词必须一律返回 None，否则会污染识别结果
        let idx = test_idx();
        let noise = &[
            "T1", "65W", "满改", "丐版", "腰射", "烽火地带", "修脚流", "倍镜",
            "性价比", "半改", "红点", "消音", "10w", "满腰射", "狙击", "连狙",
        ];
        for n in noise {
            let got = canonical_gun(&idx, n);
            assert_eq!(got, None, "噪音词 {n:?} 被误配为 {got:?}");
        }
    }

    #[test]
    fn test_short_name_not_matched_by_substr() {
        // 「弓」只有 1 字，低于子串门槛 → `弓弩` / `弓兵` 不得误配为「弓」
        let idx = test_idx();
        assert_eq!(canonical_gun(&idx, "弓弩"), None, "「弓弩」是分类词，不该匹配为「弓」");
        assert_eq!(canonical_gun(&idx, "复合弓"), Some("弓".into()), "复合弓 有别名，应命中");
        assert_eq!(canonical_gun(&idx, "弓"), Some("弓".into()), "整体等于应命中");
    }

    #[test]
    fn test_match_gun_in_text_joins_adjacent_parts() {
        // 含空格/连字符的枪名被空格拆散时，靠「相邻拼接」救回
        let idx = test_idx();
        assert_eq!(
            match_gun_in_text(&idx, "T0 50W 半改 AS Val突击步枪-"),
            Some("AS Val".into()),
            "AS + Val 需拼接匹配"
        );
        assert_eq!(
            match_gun_in_text(&idx, "T1 19W 丐版 SR-3M紧凑突击步枪-"),
            Some("SR-3M".into()),
            "SR + 3M... 需拼接匹配"
        );
        assert_eq!(
            match_gun_in_text(&idx, "T1 65W 满改 AR57突击步枪-"),
            Some("AR57".into()),
            "单片即可匹配"
        );
        assert_eq!(match_gun_in_text(&idx, ""), None, "空串应返回 None");
    }

    #[test]
    fn test_split_by_code_anchor() {
        // 双码同行 → 切成两段，各自带自己的左侧文本
        let line = "T1 50W 满腰射 AR57突击步枪-烽火地带-6JNVKVK03EINQ63AGU05N \
                    T0 50W 三连发满改 MK4冲锋枪-烽火地带-6IUK40806N3JUGK4SSBSR";
        let segs = split_by_code_anchor(line);
        assert_eq!(segs.len(), 2, "应切成 2 段，实得 {segs:?}");
        assert_eq!(segs[0].0, "6JNVKVK03EINQ63AGU05N");
        assert_eq!(segs[1].0, "6IUK40806N3JUGK4SSBSR");
        // 第 1 段左侧不得包含第 2 段的枪名（防跨段串枪）
        assert!(!segs[0].1.contains("MK4"), "第 1 段混入了第 2 段枪名: {:?}", segs[0].1);
        assert!(segs[0].1.contains("AR57"));
        assert!(segs[1].1.contains("MK4"));
        assert!(!segs[1].1.contains("AR57"), "第 2 段混入第 1 段枪名: {:?}", segs[1].1);
    }

    #[test]
    fn test_anchor_segmentation_prevents_cross_segment() {
        // 端到端语义：双码同行时，每段应解析出自己的枪名（而非都取第一段的）
        let idx = test_idx();
        let line = "T1 50W 满腰射 AR57突击步枪-烽火地带-6JNVKVK03EINQ63AGU05N \
                    T0 50W 三连发满改 MK47突击步枪-烽火地带-6IUK40806N3JUGK4SSBSR";
        let segs = split_by_code_anchor(line);
        let guns: Vec<Option<String>> = segs
            .iter()
            .map(|(_, left)| match_gun_in_text(&idx, left))
            .collect();
        assert_eq!(guns[0], Some("AR57".into()), "第 1 段应为 AR57，实得 {:?}", guns[0]);
        assert_eq!(guns[1], Some("MK47".into()), "第 2 段应为 MK47，实得 {:?}", guns[1]);
    }

    #[test]
    fn test_split_by_code_anchor_edge_cases() {
        // 无锚点
        assert!(split_by_code_anchor("ABC 普通文本").is_empty());
        // 锚点后码不合法（纯数字）
        assert!(split_by_code_anchor("烽火地带-123456789012345678901").is_empty());
        // 单码
        let one = split_by_code_anchor("M4A1突击步枪-烽火地带-6L6FVH000B5F1EC0CC9I8");
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].0, "6L6FVH000B5F1EC0CC9I8");
    }

    #[test]
    fn test_xlsx_path_still_works() {
        // 回归：有制表符时仍按列切分，枪名是独立单元格
        let cells = split_cells("汤姆逊\t36w\t6L6INHO0B1H3TI624U64G\t丐版腰射");
        assert_eq!(cells.len(), 4);
        assert!(is_code_token(&cells[2]));
        // 独立单元格枪名（xlsx 形态）仍能被四段式的「整体等于」命中
        let idx = test_idx();
        assert_eq!(canonical_gun(&idx, "野牛"), Some("野牛".into()));
        assert_eq!(canonical_gun(&idx, "P90"), Some("P90".into()));
    }

    #[test]
    fn test_rules_have_pdf_support_data() {
        // 规则库必须含支撑 PDF 格式的数据（否则上面那些匹配会退化为 None）
        let r: GunRules = serde_json::from_str(RULES_EMBEDDED).expect("内嵌规则库解析失败");
        assert_eq!(r.version, 4, "规则库版本应为 v4（补别名/噪音/SR9）");

        // 复合弓 别名
        let bow = r.guns.get("弓").expect("规则库缺「弓」");
        assert!(
            bow.aliases.iter().any(|a| a == "复合弓"),
            "「弓」缺别名「复合弓」，PDF 里会识别失败"
        );
        // SR9 新增
        assert!(r.guns.contains_key("SR9"), "规则库缺 SR9（PDF 里出现过）");
        // PDF 表头噪音词
        for w in ["弓弩", "连狙", "栓狙"] {
            assert!(r.noise.iter().any(|n| n == w), "噪音词缺 {w:?}（PDF 表头词）");
        }
    }

    #[test]
    fn test_guns_table_covers_all_rules() {
        // 枪械表必须覆盖规则库全部枪名，且分类非空
        let r: GunRules = serde_json::from_str(RULES_EMBEDDED).expect("内嵌规则库解析失败");
        let table = guns_table(&r);
        assert_eq!(
            table.len(),
            r.guns.len(),
            "枪械表条数({}) 应等于规则库枪数({})",
            table.len(),
            r.guns.len()
        );
        for row in &table {
            let name = row["name"].as_str().unwrap_or("");
            let cat = row["category"].as_str().unwrap_or("");
            assert!(!name.is_empty(), "枪械表存在空枪名");
            assert!(!cat.is_empty(), "枪 {name} 分类为空");
            assert!(r.guns.contains_key(name), "枪械表含规则库没有的枪: {name}");
        }
        // 必须含此前前端手写表漏掉的 14 把（防回归）
        let names: Vec<&str> = table.iter().filter_map(|r| r["name"].as_str()).collect();
        for g in [
            "SR-3M", "SCAR-H", "ASH-12", "AK-12", "AS Val", "AKS-74U", "FS-12", "QBZ-95",
            "93R", "CAR-15", "PTR-32", "PSG-1", "SR9", "SV-98",
        ] {
            assert!(names.contains(&g), "枪械表缺 {g}（前端分类会归到「其他」）");
        }
    }

    #[test]
    fn test_is_code_token() {
        assert!(is_code_token("6L6INHO0B1H3TI624U64G"));
        assert!(is_code_token("BLAST6LHIML007OULUBJA9PRPI"));
        // 纯数字不是码
        assert!(!is_code_token("123456789012345678901"));
        // 纯字母不是码
        assert!(!is_code_token("ABCDEFGHIJKLMNOPQRSTUV"));
        // 太短
        assert!(!is_code_token("6L6INH"));
    }

    #[test]
    fn test_rejects_pure_hex_image_ids() {
        // WPS 表格 DISPIMG 图片 ID：纯 0-9A-F，必须被排除
        assert!(!is_code_token("DDF43452B6EB4469AB09D4596FFAFC66"));
        assert!(!is_code_token("11176EE2C1EB4292845AD3FF12B09B4B"));
        // 真实码含 G-Z，仍应通过
        assert!(is_code_token("6L6INHO0B1H3TI624U64G"));   // 含 I/N/H/O/T/U
        assert!(is_code_token("BLAST6LHIML007OULUBJA9PRPI")); // 含 L/M/U/P/R
    }

    #[test]
    fn test_fingerprint_is_10_chars() {
        let fp = extract_fingerprint("6L6INHO0B1H3TI624U64G");
        assert_eq!(fp, "6L6INHO0B1");
        assert_eq!(fp.chars().count(), 10);
    }

    #[test]
    fn test_source_is_tail() {
        assert_eq!(extract_source("6L6INHO0B1H3TI624U64G"), "H3TI624U64G");
    }

    #[test]
    fn test_normalize_merges_variants() {
        // 这是修复「同一把枪分裂成多个筛选项」的关键
        assert_eq!(normalize("AK-12"), normalize("AK12"));
        assert_eq!(normalize("SR-3M"), normalize("SR3M"));
        assert_eq!(normalize("AS Val"), normalize("ASVAL"));
        assert_eq!(normalize("as val"), normalize("AS VAL"));
        // M7 与 M700 不应混淆
        assert_ne!(normalize("M7"), normalize("M700"));
    }

    #[test]
    fn test_split_cells_tsv() {
        let c = split_cells("汤姆逊\t36w\t6L6INHO0B1H3TI624U64G\t丐版腰射");
        assert_eq!(c.len(), 4);
        assert_eq!(c[0], "汤姆逊");
        assert!(is_code_token(&c[2]));
    }

    #[test]
    fn test_find_embedded_code() {
        // `M14-烽火地带-6JCB1FO...` 这类内嵌格式
        let cell = "MK47突击步枪-烽火地带-6HN54R8B1H3TI624U64G";
        let c = find_code_in_cell(cell).unwrap();
        assert!(is_code_token(&c));
    }

    #[test]
    fn test_row_classification() {
        let rules = GunRules {
            noise: vec!["哈夫币".into(), "改枪码".into(), "步枪".into()],
            ..Default::default()
        };
        // 表头
        let header = vec!["步枪区".to_string(), "哈夫币".to_string(), "改枪码".to_string()];
        assert_eq!(classify_row(&header, &rules, false), RowKind::Header);
        // 长说明
        let noise = vec!["文档里出现的广告均为本人审批担保，请勿相信任何非官方渠道".to_string()];
        assert_eq!(classify_row(&noise, &rules, false), RowKind::Noise);
        // 数据行
        let data = vec!["6L6INHO0B1H3TI624U64G".to_string()];
        assert_eq!(classify_row(&data, &rules, true), RowKind::Data);
    }

    #[test]
    fn test_extract_config_prefers_left() {
        let idx = HashMap::new();
        let cells: Vec<String> = vec![
            "汤姆逊".into(),
            "36w".into(),
            "6L6INHO0B1H3TI624U64G".into(),
            "丐版腰射".into(),
        ];
        // 左邻 `36w` 优先
        assert_eq!(extract_config(&idx, &cells, 2), "36w");
    }

    #[test]
    fn test_fingerprint_uniqueness_on_real_samples() {
        // 实测：前 6 位有冲突的真实样本，前 10 位必须区分开
        let a = "6JFFGC404U3TN9SQ0V71K";
        let b = "6JFFGCS04U3TN9SQ0V71K";
        assert_eq!(extract_fingerprint(a).chars().take(6).collect::<String>(),
                   extract_fingerprint(b).chars().take(6).collect::<String>());
        assert_ne!(extract_fingerprint(a), extract_fingerprint(b));
    }

    #[test]
    fn test_bundle_row_not_inherited() {
        // 「一键导入一整套改枪」行的码不属于任何单把枪，不应继承枪名
        let rules = GunRules {
            noise: vec!["一键导入".into()],
            ..Default::default()
        };
        let bundle = vec![
            "一键导入一整套改枪".to_string(),
            "均衡版→".to_string(),
            "BLAST6LHIML007OULUBJA9PRPI".to_string(),
        ];
        assert!(is_bundle_row(&bundle, &rules));

        // 普通数据行不应被判为套件码
        let normal = vec![
            "汤姆逊".to_string(),
            "36w".to_string(),
            "6L6INHO0B1H3TI624U64G".to_string(),
        ];
        assert!(!is_bundle_row(&normal, &rules));
    }

    #[test]
    fn test_foreign_mode_rejected() {
        // ⚠️ 三模式（烽火地带/全面战场/黑潮爆破）改枪码兼容性未知，仅收录烽火地带
        let rules = GunRules { mode: "烽火地带".into(), ..Default::default() };

        // 黑潮爆破标题行
        let hb = vec!["黑潮爆破模式改枪码大全".to_string(), "10.14".to_string()];
        assert_eq!(foreign_mode_of_row(&hb, &rules).as_deref(), Some("黑潮爆破"));

        // 全面战场标题行
        let qm = vec!["地板的改枪码大全(全面战场)".to_string()];
        assert_eq!(foreign_mode_of_row(&qm, &rules).as_deref(), Some("全面战场"));

        // 战场码
        let zc = vec!["战场码次要".to_string()];
        assert_eq!(foreign_mode_of_row(&zc, &rules).as_deref(), Some("全面战场"));

        // 普通烽火行不应被拦
        let ok = vec!["汤姆逊".to_string(), "36w".to_string(), "6L6INHO0B1H3TI624U64G".to_string()];
        assert_eq!(foreign_mode_of_row(&ok, &rules), None);

        // 规则库整体标注为其他模式 → 一律视为外来
        let other = GunRules { mode: "黑潮爆破".into(), ..Default::default() };
        assert_eq!(foreign_mode_of_row(&ok, &other).as_deref(), Some("黑潮爆破"));
    }

    #[test]
    fn test_rules_are_fenghuo_only() {
        // 内嵌规则库必须是纯烽火地带（防止误把战场/黑潮码打进包）
        let r: GunRules = serde_json::from_str(RULES_EMBEDDED).expect("内嵌规则库解析失败");
        assert_eq!(r.mode, "烽火地带", "规则库 mode 应为烽火地带，实际: {}", r.mode);
        let bad: Vec<&String> = r
            .fingerprints
            .iter()
            .filter(|(_, v)| !v.mode.is_empty() && v.mode != "烽火地带")
            .map(|(k, _)| k)
            .collect();
        assert!(bad.is_empty(), "规则库混入非烽火地带指纹: {:?}", &bad[..bad.len().min(5)]);
    }

    #[test]
    fn test_embedded_rules_parse() {
        // 内嵌规则库必须可解析且非空（否则识别全线失效）
        let r: GunRules = serde_json::from_str(RULES_EMBEDDED).expect("内嵌规则库解析失败");
        assert!(r.guns.len() > 50, "枪械数过少: {}", r.guns.len());
        // 阈值按「仅烽火地带」口径设定（剔除战场/黑潮后约 399 条；
        // 旧版混入三模式时约 589 条，故此处门槛取 350 以防回归到污染状态）
        assert!(r.fingerprints.len() > 350, "指纹数过少: {}", r.fingerprints.len());
        assert!(!r.noise.is_empty(), "噪音词表为空");
        // 每条指纹的枪名都应能在 guns 里找到（或至少非空）
        // 注：套件码等少数条目无单枪归属，允许为空
        let with_gun = r.fingerprints.values().filter(|f| !f.gun.is_empty()).count();
        assert!(
            with_gun * 100 / r.fingerprints.len().max(1) >= 90,
            "有枪名的指纹占比过低: {with_gun}/{}",
            r.fingerprints.len()
        );
    }

    #[test]
    fn test_alias_resolution_from_rules() {
        let r: GunRules = serde_json::from_str(RULES_EMBEDDED).unwrap();
        // AK-12 的别名 AK12 应能归一到 AK-12
        let mut idx: HashMap<String, String> = HashMap::new();
        for (canon, meta) in &r.guns {
            idx.entry(normalize(canon)).or_insert_with(|| canon.clone());
            for a in &meta.aliases {
                idx.entry(normalize(a)).or_insert_with(|| canon.clone());
            }
        }
        assert_eq!(idx.get(&normalize("AK12")), Some(&"AK-12".to_string()));
        assert_eq!(idx.get(&normalize("SR3M")), Some(&"SR-3M".to_string()));
        assert_eq!(idx.get(&normalize("QJB201")), Some(&"QJB".to_string()));
    }
}
