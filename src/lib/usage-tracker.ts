/**
 * 功能使用频率记录器
 *
 * 目的：记录用户实际使用哪些功能、次数多少，为后续「砍功能 / 简化界面」提供数据依据。
 *
 * 记录来源：
 *  1. 页面访问：路由变化时 trackUsage("page:/music")（App.tsx 统一监听）
 *  2. 按钮点击：任意带 data-track="xxx" 属性的元素被点击时自动记录（全局捕获监听）
 *  3. 业务代码主动调用：trackUsage("music.play") 等
 *
 * 存储：localStorage（WebView2 profile 持久化，跨重启保留；主窗口销毁重建后仍在）
 */
const STORAGE_KEY = "nexbox.usageStats.v1";

export type UsageStat = { key: string; count: number };

let cache: Record<string, number> | null = null;
let writeTimer: ReturnType<typeof setTimeout> | null = null;

function load(): Record<string, number> {
  if (cache) return cache;
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    cache = raw ? (JSON.parse(raw) as Record<string, number>) : {};
  } catch {
    cache = {};
  }
  return cache;
}

function scheduleSave() {
  if (writeTimer) clearTimeout(writeTimer);
  writeTimer = setTimeout(() => {
    writeTimer = null;
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(cache ?? {}));
    } catch {
      // ignore quota errors
    }
  }, 300);
}

/** 记录一次功能使用。key 建议形如 "page:/music"、"click:xxx"、"music.play" */
export function trackUsage(key: string): void {
  if (!key) return;
  const stats = load();
  stats[key] = (stats[key] ?? 0) + 1;
  scheduleSave();
}

/** 获取全部统计（按次数降序） */
export function getUsageStats(): UsageStat[] {
  const stats = load();
  return Object.entries(stats)
    .map(([key, count]) => ({ key, count }))
    .sort((a, b) => b.count - a.count);
}

/** 获取总记录次数（所有功能使用次数之和） */
export function getUsageTotal(): number {
  const stats = load();
  return Object.values(stats).reduce((sum, n) => sum + n, 0);
}

/** 清空统计 */
export function resetUsageStats(): void {
  cache = {};
  if (writeTimer) clearTimeout(writeTimer);
  try {
    localStorage.removeItem(STORAGE_KEY);
  } catch {
    // ignore
  }
}

/** 导出为 JSON 字符串（用于保存分析） */
export function exportUsageStats(): string {
  return JSON.stringify(load(), null, 2);
}

/** 从 storage 变化事件刷新缓存（多窗口同步用，本应用单主窗口可忽略） */
export function refreshUsageCache(): void {
  cache = null;
}
