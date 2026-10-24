//! WebView2 启动参数单一真源
//!
//! 全应用**所有** WebView2 窗口必须使用逐字相同的 `additionalBrowserArgs`：
//! 参数不一致会让 WebView2 环境冲突，直接导致新窗口创建失败（本工程踩过）。
//! 因此这里只保留一份常量，窗口创建处一律引用，禁止再写字面量。
//!
//! `tauri.conf.json` 里 main / desktop-lyrics 两个窗口的 `additionalBrowserArgs`
//! 无法引用 Rust 常量，只能复制 —— 由本文件的测试 `browser_args_match_tauri_conf`
//! 守住三处一致，改了一处忘另一处会直接测试失败。
//!
//! ## 关于媒体会话（重要）
//!
//! 早先这里禁用 `MediaSessionService` + `HardwareMediaKeyHandling`，用来压制
//! Chromium 自动注册的媒体会话（避免与本应用自建的 WinRT SMTC 会话重复）。
//! 现在方案反过来：**由 Chromium 的 MediaSession 直接对接系统 SMTC**
//! （前端 `src/lib/media-session.ts` 设置 `navigator.mediaSession`），
//! 因此这两项特性**必须保持启用**，不能再出现在 disable 列表里。
//!
//! 其余参数保持原样，各有用途（详见下方逐项注释）。

/// 全窗口统一的 WebView2 参数。
///
/// 必须与 `tauri.conf.json` 中两个窗口的 `additionalBrowserArgs` 完全一致。
pub const BROWSER_ARGS: &str = concat!(
    // ── 特性开关 ──
    // 注意：这里**不再**包含 MediaSessionService / HardwareMediaKeyHandling——
    // 系统媒体会话现在由 Chromium MediaSession 提供，禁用即等于关掉整个 SMTC 集成。
    "--disable-features=",
    // Edge 专属 UI 与省电策略：本应用自带 UI，这些只增加开销
    "msWebOOUI,msPdfOOUI,msSmartScreenProtection,msEdgeAutofill,msEdgeShopping,msEdgeWallet,",
    // Chromium 原生窗口遮挡计算对 always-on-top 透明小窗有误判 bug：
    // 被误判「被遮挡」→ 渲染挂起 → 恢复 → 反复横跳，表现为透明窗口内容闪烁
    "CalculateNativeWinOcclusion ",
    // ── 自动播放 ──
    // 音乐播放器需要在无用户手势时起播（切歌、自动续播）
    "--autoplay-policy=no-user-gesture-required ",
    // ── 后台行为 ──
    // 关闭后台网络/组件更新/默认应用/扩展/同步：桌面应用不需要，纯降开销
    "--disable-background-networking --disable-client-side-phishing-detection ",
    "--disable-component-update --disable-default-apps --disable-extensions --disable-sync ",
    // 关键三项：窗口失焦/被遮挡/最小化到托盘时不降频、不冻结定时器。
    // 音乐播放、灵动岛动画、桌面歌词插值都依赖后台持续运行
    "--disable-backgrounding-occluded-windows --disable-renderer-backgrounding ",
    "--disable-background-timer-throttling",
);

#[cfg(test)]
mod tests {
    use super::BROWSER_ARGS;

    /// 守住「Rust 常量 == tauri.conf.json 两个窗口的参数」。
    /// JSON 侧无法引用常量，只能复制，因此必须由测试兜住。
    #[test]
    fn browser_args_match_tauri_conf() {
        // 不引入 serde_json 依赖负担：conf 里该字段是单个字符串值，直接按行提取即可
        let conf = include_str!("../../tauri.conf.json");
        let key = "\"additionalBrowserArgs\": \"";
        let mut found = 0usize;
        for line in conf.lines() {
            let line = line.trim();
            let Some(rest) = line.strip_prefix(key) else {
                continue;
            };
            // 该字段可能是对象/数组的最后一项（无尾逗号），也可能不是（有尾逗号）
            let Some(end) = rest.rfind('"') else {
                panic!("tauri.conf.json 中 additionalBrowserArgs 行格式异常: {line}");
            };
            let value = &rest[..end];
            assert!(
                rest[end + 1..].trim().is_empty() || rest[end + 1..].trim() == ",",
                "tauri.conf.json 中 additionalBrowserArgs 行尾有多余内容: {line}",
            );
            found += 1;
            assert_eq!(
                value, BROWSER_ARGS,
                "tauri.conf.json 第 {found} 处 additionalBrowserArgs 与 utils::webview::BROWSER_ARGS 不一致",
            );
        }
        assert_eq!(
            found, 2,
            "tauri.conf.json 应有 2 处 additionalBrowserArgs（main / desktop-lyrics），实际 {found} 处",
        );
    }

    /// 系统媒体会话由 Chromium MediaSession 提供，这两项特性一旦被禁用，
    /// 音量浮层/锁屏将完全不出现本应用 —— 属于「静默失效」，必须由测试拦截。
    #[test]
    fn media_session_features_stay_enabled() {
        assert!(
            !BROWSER_ARGS.contains("MediaSessionService"),
            "BROWSER_ARGS 不得禁用 MediaSessionService：系统媒体会话依赖它",
        );
        assert!(
            !BROWSER_ARGS.contains("HardwareMediaKeyHandling"),
            "BROWSER_ARGS 不得禁用 HardwareMediaKeyHandling：系统媒体键依赖它",
        );
    }
}
