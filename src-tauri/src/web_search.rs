//! 联网搜索模块（移植自 tubatools-master WebSearchService 的 Rust 实现）。
//!
//! 策略（按顺序尝试，任一成功即返回）：
//! 1. uapis.cn 聚合搜索（POST https://uapis.cn/api/v1/search/aggregate）—— 无需 key 也可调用，
//!    若配置了 `UAPI_API_KEY` 环境变量则携带 Authorization 头获得更高配额
//! 2. Bing（https://cn.bing.com/search + 浏览器 UA，国内可达，结果质量好）
//! 3. 自定义 SearXNG 端点（`SEARXNG_ENDPOINT` 环境变量，兼容 /search?q=&format=json）
//! 4. DuckDuckGo HTML（https://html.duckduckgo.com/html/ + 浏览器 UA + 正则解析，无需 key）
//! 5. DuckDuckGo Instant Answer API（https://api.duckduckgo.com/，摘要类查询）
//! 6. 维基百科 opensearch（兜底，知识类查询可用）
//!
//! 失败时返回空列表，调用方决定如何处理（通常降级为不联网回答）。

use serde::Deserialize;
use std::time::Duration;

const CONNECT_TIMEOUT_SECS: u64 = 8;
const REQUEST_TIMEOUT_SECS: u64 = 20;
const UAPI_KEY_ENV: &str = "UAPI_API_KEY";
const SEARXNG_ENV: &str = "SEARXNG_ENDPOINT";
const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

#[derive(Debug, Clone, serde::Serialize)]
pub struct WebSearchItem {
    pub title: String,
    pub snippet: String,
    pub url: String,
    pub source: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WebSearchResult {
    pub query: String,
    pub items: Vec<WebSearchItem>,
}

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("WEB_CLIENT_ERROR|创建搜索 HTTP 客户端失败: {e}"))
}

fn resolve_env_key() -> Option<String> {
    if let Ok(env_key) = std::env::var(UAPI_KEY_ENV) {
        let trimmed = env_key.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    None
}

fn resolve_searxng_endpoint() -> Option<String> {
    if let Ok(env_key) = std::env::var(SEARXNG_ENV) {
        let trimmed = env_key.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    None
}

/// HTML 实体解码（简易版）。
fn html_decode(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&#x27;", "'")
        .replace("&apos;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

/// 去除 HTML 标签。
fn strip_html(input: &str) -> String {
    let re = regex::Regex::new(r"<[^>]+>").unwrap();
    let text = re.replace_all(input, "");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// uapis 聚合搜索（无需 key 也可调用）
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct UapiSearchResults {
    results: Vec<UapiSearchItem>,
}

#[derive(Debug, Deserialize)]
struct UapiSearchItem {
    title: Option<String>,
    snippet: Option<String>,
    url: Option<String>,
    domain: Option<String>,
    source: Option<String>,
}

async fn search_uapis(client: &reqwest::Client, query: &str) -> Result<Vec<WebSearchItem>, String> {
    let key = resolve_env_key();

    let body = serde_json::json!({ "query": query });
    let mut req = client
        .post("https://uapis.cn/api/v1/search/aggregate")
        .json(&body);

    if let Some(k) = key {
        req = req.header("Authorization", format!("Bearer {k}"));
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("WEB_NETWORK|uapis 网络请求失败: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("WEB_HTTP|uapis 返回 {status}"));
    }

    let data: UapiSearchResults = resp
        .json()
        .await
        .map_err(|e| format!("WEB_PARSE|uapis 响应解析失败: {e}"))?;

    let items = data
        .results
        .into_iter()
        .filter_map(|it| {
            let title = it.title.unwrap_or_default();
            if title.trim().is_empty() {
                None
            } else {
                Some(WebSearchItem {
                    title,
                    snippet: it.snippet.unwrap_or_default(),
                    url: it.url.unwrap_or_default(),
                    source: it.domain.unwrap_or(it.source.unwrap_or_else(|| "uapis".to_string())),
                })
            }
        })
        .take(8)
        .collect();

    Ok(items)
}

// ---------------------------------------------------------------------------
// 自定义 SearXNG 端点
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct SearxngResponse {
    results: Vec<SearxngItem>,
}

#[derive(Debug, Deserialize)]
struct SearxngItem {
    title: Option<String>,
    content: Option<String>,
    url: Option<String>,
    engine: Option<String>,
}

async fn search_searxng(client: &reqwest::Client, endpoint: &str, query: &str) -> Result<Vec<WebSearchItem>, String> {
    let url = format!(
        "{}/search?q={}&format=json&categories=general&language=auto",
        endpoint.trim_end_matches('/'),
        urlencoding::encode(query)
    );
    let resp = client
        .get(&url)
        .header("User-Agent", BROWSER_UA)
        .send()
        .await
        .map_err(|e| format!("WEB_NETWORK|searxng 网络请求失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("WEB_HTTP|searxng 返回 {}", resp.status()));
    }
    let data: SearxngResponse = resp
        .json()
        .await
        .map_err(|e| format!("WEB_PARSE|searxng 响应解析失败: {e}"))?;

    let items = data
        .results
        .into_iter()
        .filter_map(|it| {
            let title = it.title.unwrap_or_default();
            if title.trim().is_empty() {
                None
            } else {
                Some(WebSearchItem {
                    title,
                    snippet: it.content.unwrap_or_default(),
                    url: it.url.unwrap_or_default(),
                    source: it.engine.unwrap_or_else(|| "SearXNG".to_string()),
                })
            }
        })
        .take(8)
        .collect();

    Ok(items)
}

// ---------------------------------------------------------------------------
// Bing 搜索（cn.bing.com，国内可达，结果质量好）
// ---------------------------------------------------------------------------

async fn search_bing(client: &reqwest::Client, query: &str) -> Result<Vec<WebSearchItem>, String> {
    let url = format!(
        "https://cn.bing.com/search?q={}&count=10",
        urlencoding::encode(query)
    );
    let resp = client
        .get(&url)
        .header("User-Agent", BROWSER_UA)
        .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
        .send()
        .await
        .map_err(|e| format!("WEB_NETWORK|bing 网络请求失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("WEB_HTTP|bing 返回 {}", resp.status()));
    }
    let html = resp
        .text()
        .await
        .map_err(|e| format!("WEB_PARSE|bing 读取失败: {e}"))?;

    // 解析 b_algo 结果块：<li class="b_algo">...</li>
    let algo_re = regex::Regex::new(r#"<li class="b_algo"[^>]*>([\s\S]*?)(?=<li class="b_algo"|</ol>|</div>\s*</li>)"#).unwrap();
    let link_re = regex::Regex::new(r#"<h2[^>]*><a[^>]*href="([^"]*)"[^>]*>([\s\S]*?)</a></h2>"#).unwrap();
    let snippet_re = regex::Regex::new(r#"<p[^>]*>([\s\S]*?)</p>"#).unwrap();

    let mut items = Vec::new();
    for block in algo_re.captures_iter(&html) {
        let block_html = &block[1];
        if let Some(link_cap) = link_re.captures(block_html) {
            let url = html_decode(&link_cap[1]);
            let title = strip_html(&html_decode(&link_cap[2]));
            if title.trim().is_empty() {
                continue;
            }
            let snippet = snippet_re
                .captures(block_html)
                .map(|m| strip_html(&html_decode(&m[1])))
                .unwrap_or_default();

            items.push(WebSearchItem {
                title,
                snippet,
                url,
                source: "Bing".to_string(),
            });
            if items.len() >= 8 {
                break;
            }
        }
    }

    Ok(items)
}

// ---------------------------------------------------------------------------
// DuckDuckGo HTML 搜索（无需 key）
// ---------------------------------------------------------------------------

async fn search_ddg_html(client: &reqwest::Client, query: &str) -> Result<Vec<WebSearchItem>, String> {
    let url = format!(
        "https://html.duckduckgo.com/html/?q={}",
        urlencoding::encode(query)
    );
    let resp = client
        .get(&url)
        .header("User-Agent", BROWSER_UA)
        .send()
        .await
        .map_err(|e| format!("WEB_NETWORK|ddg-html 网络请求失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("WEB_HTTP|ddg-html 返回 {}", resp.status()));
    }
    let html = resp
        .text()
        .await
        .map_err(|e| format!("WEB_PARSE|ddg-html 读取失败: {e}"))?;

    let link_re = regex::Regex::new(r#"class="result__a"[^>]*href="([^"]*)"[^>]*>([\s\S]*?)</a>"#).unwrap();
    let snippet_re = regex::Regex::new(r#"class="result__snippet"[^>]*>([\s\S]*?)</a>"#).unwrap();

    let link_matches: Vec<_> = link_re.captures_iter(&html).collect();
    let snippet_matches: Vec<_> = snippet_re.captures_iter(&html).collect();

    let mut items = Vec::new();
    for (i, cap) in link_matches.iter().take(8).enumerate() {
        let raw_url = html_decode(&cap[1]);
        let title = strip_html(&html_decode(&cap[2]));
        if title.trim().is_empty() {
            continue;
        }
        let actual_url = extract_ddg_actual_url(&raw_url);
        let snippet = snippet_matches
            .get(i)
            .map(|m| strip_html(&html_decode(&m[1])))
            .unwrap_or_default();

        items.push(WebSearchItem {
            title,
            snippet,
            url: actual_url,
            source: "DuckDuckGo".to_string(),
        });
    }

    Ok(items)
}

/// 解析 DuckDuckGo HTML 的重定向链接。
fn extract_ddg_actual_url(ddg_url: &str) -> String {
    if ddg_url.is_empty() {
        return String::new();
    }
    if ddg_url.starts_with("//duckduckgo.com/l/") {
        if let Some(idx) = ddg_url.find("uddg=") {
            let encoded = &ddg_url[idx + 5..];
            let end = encoded.find('&').unwrap_or(encoded.len());
            return urlencoding::decode(&encoded[..end])
                .map(|s| s.into_owned())
                .unwrap_or_default();
        }
    }
    if ddg_url.starts_with("//") {
        return format!("https:{ddg_url}");
    }
    ddg_url.to_string()
}

// ---------------------------------------------------------------------------
// DuckDuckGo Instant Answer API
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct DdgResponse {
    #[serde(rename = "AbstractText")]
    abstract_text: Option<String>,
    #[serde(rename = "Heading")]
    heading: Option<String>,
    #[serde(rename = "AbstractURL")]
    abstract_url: Option<String>,
    #[serde(rename = "AbstractSource")]
    abstract_source: Option<String>,
    #[serde(rename = "RelatedTopics")]
    related_topics: Vec<serde_json::Value>,
}

fn extract_related_text(v: &serde_json::Value) -> Option<(String, String, String)> {
    if let Some(text) = v.get("Text").and_then(|x| x.as_str()) {
        let url = v.get("FirstURL").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let (title, snippet) = if let Some(idx) = text.find(" - ") {
            (text[..idx].to_string(), text[idx + 3..].to_string())
        } else {
            (text.to_string(), String::new())
        };
        return Some((title, snippet, url));
    }
    if let Some(topics) = v.get("Topics").and_then(|x| x.as_array()) {
        for t in topics {
            if let Some(r) = extract_related_text(t) {
                return Some(r);
            }
        }
    }
    None
}

async fn search_ddg_api(client: &reqwest::Client, query: &str) -> Result<Vec<WebSearchItem>, String> {
    let url = format!(
        "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1&kl=cn-zh",
        urlencoding::encode(query)
    );
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("WEB_NETWORK|ddg 网络请求失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("WEB_HTTP|ddg 返回 {}", resp.status()));
    }
    let data: DdgResponse = resp
        .json()
        .await
        .map_err(|e| format!("WEB_PARSE|ddg 响应解析失败: {e}"))?;

    let mut items = Vec::new();
    if let Some(text) = data.abstract_text {
        if !text.trim().is_empty() {
            items.push(WebSearchItem {
                title: data.heading.unwrap_or_else(|| query.to_string()),
                snippet: text,
                url: data.abstract_url.unwrap_or_default(),
                source: data
                    .abstract_source
                    .unwrap_or_else(|| "DuckDuckGo".to_string()),
            });
        }
    }
    for v in data.related_topics.iter().take(8) {
        if let Some((title, snippet, url)) = extract_related_text(v) {
            if !title.trim().is_empty() {
                items.push(WebSearchItem {
                    title,
                    snippet,
                    url,
                    source: "DuckDuckGo".to_string(),
                });
                if items.len() >= 8 {
                    break;
                }
            }
        }
    }

    Ok(items)
}

// ---------------------------------------------------------------------------
// 维基百科兜底
// ---------------------------------------------------------------------------

async fn search_wiki(client: &reqwest::Client, query: &str) -> Result<Vec<WebSearchItem>, String> {
    let url = format!(
        "https://zh.wikipedia.org/w/api.php?action=opensearch&search={}&limit=3&format=json",
        urlencoding::encode(query)
    );
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("WEB_NETWORK|wiki 网络请求失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("WEB_HTTP|wiki 返回 {}", resp.status()));
    }
    let arr: Vec<serde_json::Value> = resp
        .json()
        .await
        .map_err(|e| format!("WEB_PARSE|wiki 响应解析失败: {e}"))?;
    if arr.len() < 4 {
        return Ok(Vec::new());
    }
    let titles = arr.get(1).and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let urls = arr.get(3).and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut items = Vec::new();
    for (i, t) in titles.iter().enumerate() {
        let title = t.as_str().unwrap_or("").to_string();
        let url = urls.get(i).and_then(|v| v.as_str()).unwrap_or("").to_string();
        if title.is_empty() {
            continue;
        }
        items.push(WebSearchItem {
            title: title.clone(),
            snippet: format!("维基百科条目：{title}"),
            url,
            source: "维基百科".to_string(),
        });
    }
    Ok(items)
}

// ---------------------------------------------------------------------------
// 顶层入口
// ---------------------------------------------------------------------------

/// 执行联网搜索，自动多级 fallback；最终返回非空 Vec（无结果时返回占位提示）。
pub async fn web_search(query: &str) -> Vec<WebSearchItem> {
    let q = query.trim();
    if q.is_empty() {
        return Vec::new();
    }
    let client = match build_client() {
        Ok(c) => c,
        Err(e) => {
            log::error!("[WebSearch] {e}");
            return Vec::new();
        }
    };

    // 1) uapis（无需 key，有 key 则带授权）
    match search_uapis(&client, q).await {
        Ok(items) if !items.is_empty() => {
            log::info!("[WebSearch] 命中来源: uapis.cn，{n} 条结果，查询: {q}", n = items.len());
            return items;
        }
        Ok(_) => {}
        Err(e) => log::info!("[WebSearch] uapis 未命中: {e}"),
    }

    // 2) Bing（cn.bing.com，国内可达，结果质量好）
    match search_bing(&client, q).await {
        Ok(items) if !items.is_empty() => {
            log::info!("[WebSearch] 命中来源: Bing，{n} 条结果，查询: {q}", n = items.len());
            return items;
        }
        Ok(_) => {}
        Err(e) => log::info!("[WebSearch] bing 未命中: {e}"),
    }

    // 3) 自定义 SearXNG 端点（如配置）
    if let Some(ep) = resolve_searxng_endpoint() {
        match search_searxng(&client, &ep, q).await {
            Ok(items) if !items.is_empty() => {
                log::info!("[WebSearch] 命中来源: SearXNG({ep})，{n} 条结果", n = items.len());
                return items;
            }
            Ok(_) => {}
            Err(e) => log::info!("[WebSearch] searxng 未命中: {e}"),
        }
    }

    // 5) DuckDuckGo HTML
    match search_ddg_html(&client, q).await {
        Ok(items) if !items.is_empty() => {
            log::info!("[WebSearch] 命中来源: DuckDuckGo HTML，{n} 条结果", n = items.len());
            return items;
        }
        Ok(_) => {}
        Err(e) => log::info!("[WebSearch] ddg-html 未命中: {e}"),
    }

    // 6) DuckDuckGo Instant Answer API
    match search_ddg_api(&client, q).await {
        Ok(items) if !items.is_empty() => {
            log::info!("[WebSearch] 命中来源: DuckDuckGo API，{n} 条结果", n = items.len());
            return items;
        }
        Ok(_) => {}
        Err(e) => log::info!("[WebSearch] ddg 未命中: {e}"),
    }

    // 7) 维基百科兜底
    match search_wiki(&client, q).await {
        Ok(items) if !items.is_empty() => {
            log::info!("[WebSearch] 命中来源: 维基百科，{n} 条结果", n = items.len());
            return items;
        }
        Ok(_) => {}
        Err(e) => log::info!("[WebSearch] wiki 未命中: {e}"),
    }

    log::info!("[WebSearch] 所有搜索源均未命中: {q}");
    Vec::new()
}

/// 把搜索结果格式化为注入到 system prompt 的文本。
pub fn format_for_prompt(query: &str, items: &[WebSearchItem]) -> String {
    if items.is_empty() {
        return format!("（联网搜索 \"{query}\" 未返回结果，请基于已有知识回答。）");
    }
    let mut s = format!("以下是联网搜索 \"{query}\" 的最新结果，请基于这些信息回答：\n");
    for (i, it) in items.iter().take(6).enumerate() {
        s.push_str(&format!(
            "\n[{}] {}\n来源：{}\n{}\n{}",
            i + 1,
            it.title,
            it.source,
            it.snippet,
            if it.url.is_empty() {
                String::new()
            } else {
                format!("链接：{}", it.url)
            }
        ));
    }
    s
}