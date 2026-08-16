// 卡片/界面显隐设置的同步初始化工具。
//
// 问题：原来各卡片用 useState(true) 默认显示 + 异步 store.get 再隐藏，
// 导致切换页面时卡片先渲染（显示）→ 异步读取完成才隐藏 → 过渡期间闪烁。
//
// 方案：挂载时**同步**读 localStorage 确定初始值（localStorage 是同步 API），
// 首次渲染即正确显隐，杜绝闪烁；store 持久化与跨页面事件监听保持不变。

// 同步读取显隐开关初始值：true（默认显示）除非明确存了 false
export function initVisibility(key: string): boolean {
  try {
    const ls = localStorage.getItem(key);
    if (ls !== null) return ls === "true";
  } catch { /* ignore */ }
  return true;
}

// 订阅某个显隐开关的变更事件
export function subscribeVisibility(
  key: string,
  eventName: string,
  onChanged: (value: boolean) => void,
  // 可选：异步从 store 兜底读取一次（兼容老版本只写了 store 没写 localStorage 的情况）
  store?: { get: (k: string) => Promise<unknown> },
): () => void {
  const handler = (e: Event) => onChanged((e as CustomEvent<boolean>).detail);
  window.addEventListener(eventName, handler as EventListener);

  if (store) {
    (async () => {
      try {
        const saved = await store.get(key);
        if (saved !== null && saved !== undefined && typeof saved === "boolean") {
          // store 有值且与 localStorage 不同 → 同步一次（同时写回 localStorage 保持一致）
          const cur = localStorage.getItem(key);
          if (cur === null || (cur === "true") !== saved) {
            localStorage.setItem(key, String(saved));
            onChanged(saved);
          }
        }
      } catch { /* ignore */ }
    })();
  }

  return () => window.removeEventListener(eventName, handler as EventListener);
}
