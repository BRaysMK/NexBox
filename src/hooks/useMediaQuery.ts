import { useEffect, useState } from "react";

/**
 * 媒体查询 hook（响应式布局用）
 *
 * NexBox 是 Tauri 桌面应用，断点按**窗口宽度**设定（非移动端）。
 * 与项目既有做法一致（`ResolutionConverterPage.tsx` 用 `useBreakpointValue`），
 * 但这里需要布尔判断（切换布局结构），故自建轻量 hook。
 */
export function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState(() => {
    if (typeof window === "undefined" || !window.matchMedia) return false;
    return window.matchMedia(query).matches;
  });

  useEffect(() => {
    if (typeof window === "undefined" || !window.matchMedia) return;
    const mql = window.matchMedia(query);
    const onChange = () => setMatches(mql.matches);
    onChange();
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, [query]);

  return matches;
}

/**
 * 改枪码页的布局断点
 *
 * - `isNarrow`（< 1100px）：侧边导航收为抽屉、工具栏换行
 * - `isCompact`（< 900px）：表格视图隐藏次要列、磁贴单列
 */
export function useNotebookLayout() {
  const isNarrow = useMediaQuery("(max-width: 1099px)");
  const isCompact = useMediaQuery("(max-width: 899px)");
  return { isNarrow, isCompact };
}
