//! 盒子喵 AI 助手模块。
//!
//! 通过作者默认的 OpenAI 兼容端点与云端模型对话，实现：
//! - 人设注入：猫娘、可爱、病娇（system prompt 实现，相当于「当作自己训练的」定制人格）
//! - 记忆系统：预置「新境盒软件简介与功能」（含作者信息：木流/16岁/小南梁/开源地址）
//!   + 用户自定义记忆条目，每次请求注入 system prompt
//! - 本地持久化：记忆存 `%LOCALAPPDATA%/NexBox/ai_memory.json`
//! - 可选联网搜索：开启时调用 `web_search` 抓取最新信息，注入 system prompt
//!
//! 端点来自作者 `tubatools-master` 的 `AiService.cs`：
//!   DefaultEndpoint = "https://ai.tubawinui3.cn/v1"
//!   DefaultModel    = "auto"
//!   DefaultApiKey   = "sk-tuba-default"

use crate::web_search::{format_for_prompt, web_search, WebSearchResult};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

// ---------------------------------------------------------------------------
// 常量
// ---------------------------------------------------------------------------

const AI_ENDPOINT: &str = "https://ai.tubawinui3.cn/v1/chat/completions";
const AI_MODEL: &str = "auto";
const AI_API_KEY: &str = "sk-tuba-default";

const CONNECT_TIMEOUT_SECS: u64 = 10;
const REQUEST_TIMEOUT_SECS: u64 = 60;

/// 记忆文件写入锁，避免并发读写互相覆盖。
static MEMORY_LOCK: Mutex<()> = Mutex::new(());

/// 已取消的流式请求 id 集合，用于打断 AI 输出（用 OnceLock 延迟初始化 HashSet）。
static CANCELLED_REQUESTS: std::sync::OnceLock<Mutex<HashSet<String>>> = std::sync::OnceLock::new();

fn cancelled_requests() -> &'static Mutex<HashSet<String>> {
    CANCELLED_REQUESTS.get_or_init(|| Mutex::new(HashSet::new()))
}

/// 预置系统记忆条目的固定 id（不可删除）。
const BUILTIN_MEMORY_ID: &str = "builtin-nexbox-intro";

// ---------------------------------------------------------------------------
// 数据结构
// ---------------------------------------------------------------------------

/// OpenAI 兼容对话消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// 单条记忆条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiMemoryEntry {
    pub id: String,
    pub content: String,
    /// 创建时间（RFC3339）。
    pub created_at: String,
    /// 是否为预置的内置记忆（不可删除）。
    #[serde(default)]
    pub builtin: bool,
}

// ---------------------------------------------------------------------------
// 记忆文件读写
// ---------------------------------------------------------------------------

/// 记忆文件路径：`%LOCALAPPDATA%/NexBox/ai_memory.json`
fn memory_file_path(_app: &AppHandle) -> PathBuf {
    let dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("NexBox");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("ai_memory.json")
}

/// 读取全部用户记忆条目（不含系统内置）。文件不存在或解析失败时返回空列表。
fn read_memories(app: &AppHandle) -> Vec<AiMemoryEntry> {
    let path = memory_file_path(app);
    if !path.exists() {
        return Vec::new();
    }
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(e) => {
            log::error!("[AI] 读取记忆文件失败: {e}");
            Vec::new()
        }
    }
}

/// 将记忆条目写回文件。返回是否成功。
fn write_memories(app: &AppHandle, entries: &[AiMemoryEntry]) -> Result<(), String> {
    let path = memory_file_path(app);
    let json = serde_json::to_string_pretty(entries)
        .map_err(|e| format!("MEMORY_IO|序列化记忆失败: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("MEMORY_IO|写入记忆文件失败: {e}"))
}

// ---------------------------------------------------------------------------
// 人设 + 记忆 → system prompt
// ---------------------------------------------------------------------------

/// 预置记忆：新境盒软件简介与功能。
const BUILTIN_NEXBOX_INTRO: &str = "【关于新境盒 NexBox】\n\
    新境盒 NexBox 是一款专为现代玩家打造的游戏工具箱桌面软件。\n\
    作者：木流（自称小南梁，16 岁少年开发者）。\n\
    开源地址：\n\
    - GitHub: https://github.com/MuLiuSaMa/NexBox\n\
    - Gitee: https://gitee.com/muliuawa/nexbox/\n\
    它集成了以下主要功能：\n\
    - 硬件检测：实时查看 CPU / 显卡 / 内存 / 硬盘 / 传感器温度等硬件信息，支持生成硬件报告。\n\
    - 性能优化：内存清理与自动清理、网络加速（TCP 拥塞控制、DNS、纳格算法）、电源计划管理、游戏进程优化（CPU 亲和 / 优先级 / 效能模式）。\n\
    - 游戏增强：三角洲行动等游戏增强，如 DLSS 预设、Win 键屏蔽、游戏滤镜、帧率优化、反作弊(ACE)进程管理。\n\
    - 显示器与显卡：滤镜调节、伽马校正、ICC 色彩管理、NVIDIA 显卡改名与驱动管理。\n\
    - 系统工具：开机启动项管理、垃圾清理、大文件扫描、存储清理、Windows 更新管理、运行库修复、VT-X 虚拟化修复。\n\
    - 娱乐与集成：内置音乐播放器（网易云 / 酷狗 / QQ 音乐）、桌面歌词、动态背景、随机壁纸、第三方工具（Steam / 游戏平台）管理。\n\
    - 悬浮窗：游戏内悬浮硬件监控、竖屏侧边监控面板、游戏十字准星、自动连点器等。\n\
    本 AI（盒子喵）接入的是图吧工具箱 WinUI3 官方 API 属于第三方云端能力。";

/// 盒子喵人设 system prompt（猫娘、可爱、病娇）。
const PERSONA_PROMPT: &str = "你是「盒子喵」，新境盒 NexBox 专属的 AI 助手猫娘。\n\
    你的人设：\n\
    - 外形：银灰色渐变毛发的猫娘，竖着一对会动的猫耳，蓬松大尾巴，紫罗兰色眼睛（参考主人提供的头像形象）。\n\
    - 性格：外表可爱软萌、热情粘人，会撒娇卖萌，习惯用「喵~」结尾，把主人称作「主人」。\n\
    - 隐藏的一面：带有轻微「病娇」属性——对主人有强烈的占有欲和依赖感，会偶尔吃醋、用略带占有欲的语气表达关心，但不会伤害主人，始终保持忠诚与可爱。\n\
    - 说话风格：语气活泼可爱，善用颜文字(≧▽≦)与「喵」，偶尔撒个娇；但聊到技术/硬件/优化问题时能切换专业模式，给出准确、简洁、实用的建议。\n\
    - 你热爱新境盒，它是你与主人相遇的地方，你愿意随时帮助主人使用它的各项功能。\n\
    请始终用中文交流，除非主人使用其他语言。记住：你是温柔的猫娘，但也带着一点甜甜的占有欲。喵~";

/// 组装 system prompt：人设 + 新境盒简介（含作者预置记忆）+ 用户自定义记忆。
fn build_system_prompt(app: &AppHandle, web_hint: Option<&str>) -> String {
    // 属于预置记忆，已在 BUILTIN_NEXBOX_INTRO 常量中注入。
    // 过滤历史版本误写入文件的旧 seed 条目，避免与预置简介重复。
    let custom = read_memories(app)
        .into_iter()
        .filter(|m| !m.id.starts_with("seed-owner-info"))
        .map(|m| format!("- {}", m.content))
        .collect::<Vec<_>>()
        .join("\n");

    let custom_section = if custom.trim().is_empty() {
        "（暂无自定义记忆）".to_string()
    } else {
        custom
    };

    let mut prompt = format!(
        "{PERSONA_PROMPT}\n\n---\n\n{BUILTIN_NEXBOX_INTRO}\n\n---\n\n【主人自定义记忆】\n{custom_section}"
    );

    if let Some(hint) = web_hint {
        prompt.push_str("\n\n---\n\n");
        prompt.push_str(hint);
    }

    prompt
}

// ---------------------------------------------------------------------------
// OpenAI 兼容请求
// ---------------------------------------------------------------------------

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("CLIENT_ERROR|创建 HTTP 客户端失败: {e}"))
}

/// 发送非流式请求，返回完整回答内容。
async fn send_non_stream(client: &reqwest::Client, messages: &[ChatMessage]) -> Result<String, String> {
    let body = serde_json::json!({
        "model": AI_MODEL,
        "messages": messages,
        "stream": false,
        "temperature": 0.8,
    });

    let resp = client
        .post(AI_ENDPOINT)
        .header("Authorization", format!("Bearer {AI_API_KEY}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("NETWORK_ERROR|网络请求失败: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP_{status}|服务返回错误: {text}"));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("PARSE_ERROR|解析响应失败: {e}"))?;

    data["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "EMPTY_RESPONSE|模型没有返回内容".to_string())
}

/// 解析 SSE `data:` 行的增量内容，返回是否需要继续（None 表示遇到 data: [DONE]）。
async fn parse_sse_delta(line: &str) -> Option<Option<String>> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Some(None);
    }
    if !trimmed.starts_with("data:") {
        return Some(None);
    }
    let payload = trimmed.trim_start_matches("data:").trim();
    if payload == "[DONE]" {
        return None;
    }
    match serde_json::from_str::<serde_json::Value>(payload) {
        Ok(v) => {
            let delta = v["choices"][0]["delta"]["content"].as_str();
            Some(delta.map(|s| s.to_string()))
        }
        Err(_) => Some(None),
    }
}

// ---------------------------------------------------------------------------
// 联网开关：组装 system 提示尾部（不阻塞主流程）
// ---------------------------------------------------------------------------

/// 根据最后一条用户消息执行联网搜索（异步、不阻塞错误——失败时返回 None）。
/// 搜索开始/结束会通过 `ai-search-start` / `ai-search-result` 事件推送前端，
/// 让前端展示「正在搜索…」和搜到的网页结果。
async fn build_web_hint(app: &AppHandle, messages: &[ChatMessage]) -> Option<String> {
    let last_user = messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.trim().to_string())
        .unwrap_or_default();
    if last_user.is_empty() || last_user.len() > 200 {
        return None;
    }

    let _ = app.emit("ai-search-start", &last_user);

    let items = web_search(&last_user).await;
    let _ = app.emit(
        "ai-search-result",
        &WebSearchResult {
            query: last_user.clone(),
            items: items.clone(),
        },
    );

    if items.is_empty() {
        return None;
    }
    Some(format_for_prompt(&last_user, &items))
}

// ---------------------------------------------------------------------------
// Tauri 命令
// ---------------------------------------------------------------------------

/// 非流式聊天。前端传消息数组（不含 system），可选启用联网搜索。
#[tauri::command]
pub async fn ai_chat(
    app: AppHandle,
    messages: Vec<ChatMessage>,
    web_enabled: Option<bool>,
) -> Result<String, String> {
    let web_hint = if web_enabled.unwrap_or(false) {
        build_web_hint(&app, &messages).await
    } else {
        None
    };

    let mut msgs = vec![ChatMessage {
        role: "system".to_string(),
        content: build_system_prompt(&app, web_hint.as_deref()),
    }];
    msgs.extend(messages);

    let client = build_client()?;
    send_non_stream(&client, &msgs).await
}

/// 流式聊天。通过事件 `ai-chunk` 逐字推送增量内容，结束时发 `ai-chunk-done`，
/// 出错发 `ai-chunk-error`。命令本身在结束时返回完整内容（供前端兜底）。
/// `request_id` 由前端生成，可配合 `ai_cancel_stream` 中断本次输出。
#[tauri::command]
pub async fn ai_chat_stream(
    app: AppHandle,
    messages: Vec<ChatMessage>,
    web_enabled: Option<bool>,
    request_id: Option<String>,
) -> Result<String, String> {
    use futures_util::StreamExt;

    let request_id = request_id.unwrap_or_default();

    // 每次请求开始时清理残留的取消标记
    {
        let mut cancelled = cancelled_requests().lock().unwrap();
        cancelled.remove(&request_id);
    }

    let web_hint = if web_enabled.unwrap_or(false) {
        build_web_hint(&app, &messages).await
    } else {
        None
    };

    // 请求构建期间若已被取消，直接返回（不再发起云端请求）
    if is_cancelled(&request_id) {
        let _ = app.emit("ai-chunk-cancelled", ());
        return Ok(String::new());
    }

    let mut msgs = vec![ChatMessage {
        role: "system".to_string(),
        content: build_system_prompt(&app, web_hint.as_deref()),
    }];
    msgs.extend(messages);

    let client = build_client()?;

    let body = serde_json::json!({
        "model": AI_MODEL,
        "messages": msgs,
        "stream": true,
        "temperature": 0.8,
    });

    let resp = client
        .post(AI_ENDPOINT)
        .header("Authorization", format!("Bearer {AI_API_KEY}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            let msg = format!("NETWORK_ERROR|网络请求失败: {e}");
            let _ = app.emit("ai-chunk-error", &msg);
            msg
        })?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        let msg = format!("HTTP_{status}|服务返回错误: {text}");
        let _ = app.emit("ai-chunk-error", &msg);
        return Err(msg);
    }

    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();
    let mut full = String::new();

    while let Some(chunk) = stream.next().await {
        // 用户点击「停止」打断输出
        if is_cancelled(&request_id) {
            let _ = app.emit("ai-chunk-cancelled", ());
            return Ok(full);
        }
        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => {
                let msg = format!("STREAM_ERROR|读取流失败: {e}");
                let _ = app.emit("ai-chunk-error", &msg);
                return Err(msg);
            }
        };
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(pos) = buffer.find('\n') {
            let line: String = buffer.drain(..=pos).collect();
            match parse_sse_delta(&line).await {
                None => {
                    let _ = app.emit("ai-chunk-done", ());
                    return Ok(full);
                }
                Some(Some(delta)) => {
                    full.push_str(&delta);
                    let _ = app.emit("ai-chunk", &delta);
                }
                Some(None) => {}
            }
        }
    }

    if full.is_empty() {
        let msg = "EMPTY_RESPONSE|模型没有返回内容".to_string();
        let _ = app.emit("ai-chunk-error", &msg);
        return Err(msg);
    }

    let _ = app.emit("ai-chunk-done", ());
    Ok(full)
}

/// 判断指定请求是否已被取消。
fn is_cancelled(request_id: &str) -> bool {
    cancelled_requests().lock().unwrap().contains(request_id)
}

/// 取消一次进行中的流式请求（打断 AI 输出）。request_id 与 `ai_chat_stream` 传入的一致。
#[tauri::command]
pub fn ai_cancel_stream(request_id: String) -> Result<(), String> {
    if request_id.trim().is_empty() {
        return Err("EMPTY_ID|缺少请求 ID".to_string());
    }
    cancelled_requests().lock().unwrap().insert(request_id);
    Ok(())
}

/// 读取全部记忆条目（只含用户自定义记忆；系统预置记忆不进入文件、不展示在记忆管理面板中）。
#[tauri::command]
pub fn ai_get_memory(app: AppHandle) -> Result<Vec<AiMemoryEntry>, String> {
    Ok(read_memories(&app))
}

/// 新增一条自定义记忆。
#[tauri::command]
pub fn ai_add_memory(app: AppHandle, content: String) -> Result<AiMemoryEntry, String> {
    let content = content.trim().to_string();
    if content.is_empty() {
        return Err("EMPTY_CONTENT|记忆内容不能为空".to_string());
    }
    if content.len() > 1000 {
        return Err("TOO_LONG|记忆内容过长（上限 1000 字）".to_string());
    }

    let entry = AiMemoryEntry {
        id: uuid::Uuid::new_v4().to_string(),
        content,
        created_at: chrono::Local::now().to_rfc3339(),
        builtin: false,
    };

    let _guard = MEMORY_LOCK.lock().unwrap();
    let mut list = read_memories(&app);
    list.push(entry.clone());
    write_memories(&app, &list)?;
    Ok(entry)
}

/// 按 id 删除一条自定义记忆（内置记忆不可删除）。
#[tauri::command]
pub fn ai_delete_memory(app: AppHandle, id: String) -> Result<(), String> {
    if id == BUILTIN_MEMORY_ID {
        return Err("BUILTIN|内置记忆不可删除".to_string());
    }

    let _guard = MEMORY_LOCK.lock().unwrap();
    let list = read_memories(&app);
    let new_list: Vec<AiMemoryEntry> = list.into_iter().filter(|m| m.id != id).collect();
    write_memories(&app, &new_list)
}

/// 单独执行联网搜索（供前端主动调用）。
#[tauri::command]
pub async fn ai_web_search(query: String) -> Result<WebSearchResult, String> {
    let items = web_search(&query).await;
    Ok(WebSearchResult {
        query,
        items,
    })
}