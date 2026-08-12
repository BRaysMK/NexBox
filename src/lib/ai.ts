import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/**
 * 盒子喵 AI 助手前端封装。
 *
 * 调用链：前端 → Tauri 命令（Rust 后端发起 HTTP 请求到作者云端 OpenAI 兼容端点）
 * → 流式通过 `ai-chunk` 事件推送增量文本。
 */

/** 对话角色 */
export type ChatRole = "user" | "assistant" | "system" | "search";

/** 联网搜索结果（内嵌在对话消息中展示在 AI 输出上方） */
export interface SearchResultPayload {
  query: string;
  items: WebSearchItem[];
  /** 搜索是否已完成（即使无结果也算完成，避免一直显示「搜索中」） */
  done?: boolean;
}

/** 对话消息 */
export interface ChatMessage {
  role: ChatRole;
  content: string;
  /** role === "search" 时携带搜索结果 */
  search?: SearchResultPayload;
}

/** 记忆条目 */
export interface AiMemoryEntry {
  id: string;
  content: string;
  created_at: string;
  /** 是否为预置内置记忆（不可删除） */
  builtin: boolean;
}

/** 联网搜索结果条目 */
export interface WebSearchItem {
  title: string;
  snippet: string;
  url: string;
  source: string;
}

/** 联网搜索结果 */
export interface WebSearchResult {
  query: string;
  items: WebSearchItem[];
}

/** 新境盒软件简介记忆（预置，供前端展示/兜底） */
export const NEXBOX_INTRO_MEMORY =
  "【关于新境盒 NexBox】\n" +
  "新境盒 NexBox 是一款专为现代玩家打造的游戏工具箱桌面软件。\n" +
  "它集成了以下主要功能：\n" +
  "- 硬件检测：实时查看 CPU / 显卡 / 内存 / 硬盘 / 传感器温度等硬件信息，支持生成硬件报告。\n" +
  "- 性能优化：内存清理与自动清理、网络加速（TCP 拥塞控制、DNS、纳格算法）、电源计划管理、游戏进程优化（CPU 亲和 / 优先级 / 效能模式）。\n" +
  "- 游戏增强：三角洲行动等游戏增强，如 DLSS 预设、Win 键屏蔽、游戏滤镜、帧率优化、反作弊(ACE)进程管理。\n" +
  "- 显示器与显卡：滤镜调节、伽马校正、ICC 色彩管理、NVIDIA 显卡改名与驱动管理。\n" +
  "- 系统工具：开机启动项管理、垃圾清理、大文件扫描、存储清理、Windows 更新管理、运行库修复、VT-X 修复。\n" +
  "- 娱乐与集成：内置音乐播放器（网易云 / 酷狗 / QQ 音乐）、桌面歌词、动态背景、随机壁纸、第三方工具（Steam / 游戏平台）管理。\n" +
  "- 悬浮窗：硬件监控、竖屏侧边面板、十字准星、自动连点器。";

/**
 * 解析后端返回的错误字符串（格式 `CODE|message`），映射为可直接展示的提示。
 */
export function parseAiError(raw: string): string {
  const code = raw.split("|")[0] ?? "";
  const message = raw.includes("|") ? raw.split("|").slice(1).join("|") : raw;

  switch (code) {
    case "NETWORK_ERROR":
      return "网络请求失败，请检查网络连接";
    case "HTTP_401":
    case "401":
      return "API 密钥无效或已过期";
    case "HTTP_429":
    case "429":
      return "请求过于频繁，请稍后再试";
    case "EMPTY_CONTENT":
      return "记忆内容不能为空";
    case "TOO_LONG":
      return message;
    case "BUILTIN":
      return "内置记忆不可删除";
    case "MEMORY_IO":
      return message;
    case "EMPTY_RESPONSE":
      return "模型没有返回内容，请重试";
    case "STREAM_ERROR":
      return "读取回复中断，请重试";
    case "CLIENT_ERROR":
      return message;
    case "PARSE_ERROR":
      return "解析模型回复失败，请重试";
    default:
      if (code.startsWith("HTTP_")) {
        return `服务返回异常（${code.replace("HTTP_", "")}），请稍后再试`;
      }
      return message || `请求失败（${code}）`;
  }
}

/** 将任意错误转为可展示的字符串（兼容字符串 / Error / 未知类型） */
function toErrorString(e: unknown): string {
  if (typeof e === "string") return parseAiError(e);
  if (e instanceof Error) return e.message;
  return String(e);
}

/**
 * 非流式发送对话。
 * @param messages 不含 system 的用户/助手历史消息
 * @param webEnabled 是否启用联网搜索（开启时后端会自动调用搜索并把结果注入 system）
 */
export async function sendChat(
  messages: ChatMessage[],
  webEnabled = false
): Promise<string> {
  try {
    return await invoke<string>("ai_chat", { messages, webEnabled });
  } catch (e) {
    throw new Error(toErrorString(e));
  }
}

/**
 * 流式发送对话。通过回调逐段接收增量文本。
 *
 * @param messages 不含 system 的用户/助手历史消息
 * @param onChunk 每个增量文本的回调
 * @param webEnabled 是否启用联网搜索
 * @param onCancelled 用户点击「停止」打断输出时的回调（可选）
 * @returns 完整回答（被打断时返回已生成的部分内容）
 */
/** 当前正在进行的流式请求 id（模块级跟踪，供 cancelStream 打断） */
let activeRequestId = "";

/**
 * 联网搜索状态回调：
 * - onSearchStart: 搜索开始时触发，参数为搜索关键词
 * - onSearchResult: 搜索完成时触发，参数为搜索到的网页结果
 */
export interface SearchCallbacks {
  onSearchStart?: (query: string) => void;
  onSearchResult?: (result: WebSearchResult) => void;
}

export async function sendChatStream(
  messages: ChatMessage[],
  onChunk: (delta: string) => void,
  webEnabled = false,
  onCancelled?: () => void,
  searchCallbacks?: SearchCallbacks
): Promise<string> {
  const requestId = `ai-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
  activeRequestId = requestId;
  const unlisten = await listen<string>("ai-chunk", (e) => onChunk(e.payload));
  const unlistenDone = await listen<null>("ai-chunk-done", () => {});
  const unlistenErr = await listen<string>("ai-chunk-error", () => {});
  const unlistenCancel = await listen<null>("ai-chunk-cancelled", () => {
    onCancelled?.();
  });
  const unlistenSearchStart = await listen<string>("ai-search-start", (e) => {
    searchCallbacks?.onSearchStart?.(e.payload);
  });
  const unlistenSearchResult = await listen<WebSearchResult>("ai-search-result", (e) => {
    searchCallbacks?.onSearchResult?.(e.payload);
  });

  try {
    return await invoke<string>("ai_chat_stream", { messages, webEnabled, requestId });
  } catch (e) {
    throw new Error(toErrorString(e));
  } finally {
    unlisten();
    unlistenDone();
    unlistenErr();
    unlistenCancel();
    unlistenSearchStart();
    unlistenSearchResult();
    if (activeRequestId === requestId) activeRequestId = "";
  }
}

/**
 * 打断当前进行中的流式输出（对应后端 `ai_cancel_stream`）。
 * 会通过 `ai-chunk-cancelled` 事件通知前端停止累积，并保留已生成的部分内容。
 */
export async function cancelStream(): Promise<void> {
  if (!activeRequestId) return;
  const id = activeRequestId;
  try {
    await invoke("ai_cancel_stream", { requestId: id });
  } catch {
    // 忽略
  }
}

/** 读取全部记忆条目（含预置内置记忆） */
export async function getMemory(): Promise<AiMemoryEntry[]> {
  try {
    return await invoke<AiMemoryEntry[]>("ai_get_memory");
  } catch (e) {
    throw new Error(toErrorString(e));
  }
}

/** 新增一条自定义记忆 */
export async function addMemory(content: string): Promise<AiMemoryEntry> {
  try {
    return await invoke<AiMemoryEntry>("ai_add_memory", { content });
  } catch (e) {
    throw new Error(toErrorString(e));
  }
}

/** 按 id 删除一条记忆 */
export async function deleteMemory(id: string): Promise<void> {
  try {
    await invoke("ai_delete_memory", { id });
  } catch (e) {
    throw new Error(toErrorString(e));
  }
}

/** 单独调用联网搜索 */
export async function webSearch(query: string): Promise<WebSearchResult> {
  try {
    return await invoke<WebSearchResult>("ai_web_search", { query });
  } catch (e) {
    throw new Error(toErrorString(e));
  }
}