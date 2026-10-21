//! 规则库远程热更新
//!
//! 设计要点（每一条都对应一个真实踩过的坑）：
//!
//! 1. **多源回退**：GitHub raw 为主（缓存 5 分钟，更新及时），jsDelivr 为备
//!    （缓存 7 天，但国内可达性更好）。两者内容相同，任一可用即可。
//! 2. **本地永远优先**：线上库只用来**补充本地未收录的指纹**，
//!    绝不覆盖本地已有的条目。原因：本地数据是用户自己确认过的；
//!    而线上库（哪怕是本项目发布的）也可能有解析错误。
//!    实测教训：v5 库有 56.8% 的枪名归属错误，若让线上覆盖本地，用户数据会被污染。
//! 3. **不写入任何备注**：线上库只提供「指纹 → 枪名」，config 恒为空。
//! 4. **落盘到 appDataDir，不碰 resource_dir**：
//!    resource_dir 属于安装目录，可能只读（Program Files 下 MSI 安装）。
//!    下载的线上库单独存 `rules.online.json`，与本地库**分开存储**。
//! 5. **只手动触发**：不自动联网。用户点「检查词库更新」才发请求。

use crate::rule_overrides::{clear_online_rules, save_online_rules, OnlineRules};

/// 远程规则库的候选地址（按顺序尝试，第一个成功即用）
///
/// 顺序理由：GitHub raw 的 Cache-Control 是 `max-age=300`（5 分钟），
/// 更新后很快就能拿到新版本；jsDelivr 对 `@main` 分支的缓存是 7 天，
/// 作为兜底更合适 —— 它稳定但滞后，不该放第一位。
const REMOTE_SOURCES: &[(&str, &str)] = &[
    (
        "github-raw",
        "https://raw.githubusercontent.com/BRaysMK/GunCodesAssistant/main/src-tauri/resources/gun-codes/rules.json",
    ),
    (
        "jsdelivr",
        "https://cdn.jsdelivr.net/gh/BRaysMK/GunCodesAssistant@main/src-tauri/resources/gun-codes/rules.json",
    ),
];


/// 探测系统代理（Windows 注册表）。
///
/// **为什么需要**：NexBox 用 reqwest 0.11，没有 `system-proxy` feature，
/// 而本机实测直连 GitHub 失败、必须走 SOCKS5 代理。
///
/// 返回形如 `socks5h://127.0.0.1:1080`。用 `socks5h` 让代理解析 DNS，
/// 避免本机 DNS 污染（实测 `socks5` 本地解析仍会失败）。
#[cfg(target_os = "windows")]
fn detect_system_proxy() -> Option<String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings")
        .ok()?;

    let enabled: u32 = key.get_value("ProxyEnable").unwrap_or(0);
    if enabled == 0 {
        return None;
    }
    let server: String = key.get_value("ProxyServer").ok()?;
    let s = server.trim();
    if s.is_empty() {
        return None;
    }

    // ProxyServer 可能是 "host:port" 或 "http=host:port;https=host:port"
    let addr = if let Some(rest) = s.split("https=").nth(1) {
        rest.split(';').next().unwrap_or(rest)
    } else if let Some(rest) = s.split("http=").nth(1) {
        rest.split(';').next().unwrap_or(rest)
    } else {
        s
    };
    let addr = addr.trim().trim_start_matches("http://");
    if addr.is_empty() {
        return None;
    }
    Some(format!("socks5h://{addr}"))
}

#[cfg(not(target_os = "windows"))]
fn detect_system_proxy() -> Option<String> {
    None
}

/// 构造 HTTP 客户端（自动尝试系统代理）
fn build_client() -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("NexBox/1.0 (+https://github.com/BRaysMK/GunCodesAssistant)");

    if let Some(proxy) = detect_system_proxy() {
        match reqwest::Proxy::all(&proxy) {
            Ok(p) => {
                log::info!("[Rules] 使用系统代理: {proxy}");
                builder = builder.proxy(p);
            }
            Err(e) => log::warn!("[Rules] 代理无效（{proxy}）: {e}，将尝试直连"),
        }
    } else {
        log::info!("[Rules] 未检测到系统代理，直连");
    }

    builder.build().unwrap_or_else(|e| {
        log::warn!("[Rules] 客户端构造失败: {e}，回退默认");
        reqwest::Client::new()
    })
}

/// 从单个地址拉取并校验
async fn fetch_one(url: &str) -> Result<OnlineRules, String> {
    let client = build_client();

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let text = resp.text().await.map_err(|e| format!("读取响应失败: {e}"))?;

    let rules: OnlineRules =
        serde_json::from_str(&text).map_err(|e| format!("JSON 解析失败: {e}"))?;

    // 基本校验：宁可拒绝，也不要把坏数据落盘
    if rules.fingerprints.is_empty() {
        return Err("线上库指纹为空".into());
    }
    if rules.guns.is_empty() {
        return Err("线上库枪械表为空".into());
    }
    if !rules.mode.is_empty() && rules.mode != "烽火地带" {
        return Err(format!("线上库模式不符: {}", rules.mode));
    }

    Ok(rules)
}

/// 联网检查并下载线上规则库。
///
/// 返回 `(来源名, 规则库)`。所有源都失败时返回 Err（附带每个源的失败原因）。
pub async fn fetch_remote() -> Result<(&'static str, OnlineRules), String> {
    let mut errors: Vec<String> = Vec::new();
    for (name, url) in REMOTE_SOURCES {
        match fetch_one(url).await {
            Ok(r) => {
                log::info!(
                    "[Rules] 线上库拉取成功: {} (v{}, {} 指纹)",
                    name,
                    r.version,
                    r.fingerprints.len()
                );
                return Ok((name, r));
            }
            Err(e) => {
                log::warn!("[Rules] 源 {} 失败: {e}", name);
                errors.push(format!("{name}: {e}"));
            }
        }
    }
    Err(format!("所有源均失败 —— {}", errors.join("；")))
}

/// 当前时间戳（Unix 秒，避免引入 chrono 依赖）
fn now_secs() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

// ═══════════════════════ Tauri 命令 ═══════════════════════

/// 检查线上词库是否有更新（**只比对，不落盘**）
#[tauri::command]
pub async fn check_gun_rules_update(app: tauri::AppHandle) -> serde_json::Value {
    let local = crate::local_gun_codes::load_local_rules(&app);
    let local_ver = local.version;

    match fetch_remote().await {
        Ok((src, remote)) => {
            // 计算「若叠加，能新增多少」：本地没有、有枪名、烽火地带、枪名在表里
            let would_add = remote
                .fingerprints
                .iter()
                .filter(|(fp, f)| {
                    !local.fingerprints.contains_key(*fp)
                        && !f.gun.trim().is_empty()
                        && (f.mode.is_empty() || f.mode == "烽火地带")
                        && (local.guns.contains_key(&f.gun) || remote.guns.contains_key(&f.gun))
                })
                .count();

            serde_json::json!({
                "ok": true,
                "source": src,
                "localVersion": local_ver,
                "remoteVersion": remote.version,
                "remoteGenerated": remote.generated,
                "remoteFingerprints": remote.fingerprints.len(),
                "wouldAdd": would_add,
                "hasUpdate": would_add > 0,
            })
        }
        Err(e) => serde_json::json!({
            "ok": false,
            "error": e,
            "localVersion": local_ver,
        }),
    }
}

/// 下载线上词库并落盘（**不合并、不覆盖本地库**）
#[tauri::command]
pub async fn update_gun_rules(app: tauri::AppHandle) -> serde_json::Value {
    let (src, mut remote) = match fetch_remote().await {
        Ok(v) => v,
        Err(e) => {
            return serde_json::json!({ "ok": false, "error": e });
        }
    };

    // 记录下载元信息
    remote.fetched_at = now_secs();
    remote.fetched_from = src.to_string();

    let saved = match save_online_rules(&app, &remote) {
        Ok(p) => p.display().to_string(),
        Err(e) => {
            return serde_json::json!({ "ok": false, "error": format!("保存线上库失败: {e}") });
        }
    };

    // 刷新缓存：重新走三层叠加
    let reloaded = crate::local_gun_codes::reload_rules(&app);

    serde_json::json!({
        "ok": true,
        "source": src,
        "savedOnline": saved,
        "remoteVersion": remote.version,
        "remoteGenerated": remote.generated,
        "remoteFingerprints": remote.fingerprints.len(),
        "after": reloaded.fingerprints.len(),
        "version": reloaded.version,
        "gunCount": reloaded.guns.len(),
        "guns": crate::local_gun_codes::guns_table_public(&reloaded),
    })
}

/// 清空线上库，回到纯本地库
#[tauri::command]
pub fn reset_gun_rules(app: tauri::AppHandle) -> serde_json::Value {
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
