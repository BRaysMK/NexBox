import {
  Box,
  Menu,
  MenuButton,
  MenuItem,
  MenuList,
  Portal,
} from "@chakra-ui/react";
import { Copy, MousePointerSquareDashed } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { copyText } from "@/lib/clipboard";

/** 右键时记录的上下文 */
interface CtxState {
  x: number;
  y: number;
  /** 该位置下的可复制文本（改枪码等） */
  codeText: string;
  /** 当前是否有选中文本 */
  hasSelection: boolean;
}

/**
 * 应用内右键菜单（NexBox / Chakra 版）
 *
 * **为什么需要**：Tauri 的 WebView2 默认弹出浏览器那套菜单
 * （刷新 / 另存为 / 打印 / 检查），在桌面应用里非常违和。
 *
 * 处理策略（不是简单禁用）：
 *   - 可编辑区域（input / textarea / contenteditable）→ **不拦截**，
 *     保留系统原生菜单，输入法、拼写检查、粘贴历史才不会坏。
 *   - 其它区域 → 拦截，弹出应用自己的菜单：
 *       · 有选中文本 → 「复制」
 *       · 命中改枪码 → 「复制改枪码」
 *       · 「全选」
 */
export function AppContextMenu() {
  const { t } = useTranslation();
  const [ctx, setCtx] = useState<CtxState | null>(null);
  const anchorRef = useRef<HTMLButtonElement>(null);

  const handleContextMenu = useCallback((e: MouseEvent) => {
    const target = e.target as HTMLElement | null;
    if (!target) return;

    // ① 可编辑区域：放行原生菜单
    const tag = target.tagName;
    const isEditable =
      tag === "INPUT" ||
      tag === "TEXTAREA" ||
      target.isContentEditable ||
      !!target.closest('[contenteditable="true"]');
    if (isEditable) {
      setCtx(null);
      return;
    }

    // ② 其它区域：拦截，避免出现浏览器的「刷新/另存为/打印」
    e.preventDefault();

    const sel = window.getSelection();
    const hasSelection = !!sel && !sel.isCollapsed && (sel.toString() ?? "").trim().length > 0;
    const codeEl = target.closest("code");
    const codeText = codeEl?.textContent?.trim() ?? "";

    setCtx({ x: e.clientX, y: e.clientY, codeText, hasSelection });
  }, []);

  useEffect(() => {
    window.addEventListener("contextmenu", handleContextMenu, true);
    return () => window.removeEventListener("contextmenu", handleContextMenu, true);
  }, [handleContextMenu]);

  // 点击/滚动/按键时收起菜单
  useEffect(() => {
    if (!ctx) return;
    const close = () => setCtx(null);
    window.addEventListener("click", close);
    window.addEventListener("scroll", close, true);
    window.addEventListener("keydown", close);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("scroll", close, true);
      window.removeEventListener("keydown", close);
    };
  }, [ctx]);

  if (!ctx) return null;

  const doCopySelection = () => {
    const text = window.getSelection()?.toString() ?? "";
    if (text) void copyText(text);
    setCtx(null);
  };

  const doCopyCode = () => {
    if (ctx.codeText) void copyText(ctx.codeText);
    setCtx(null);
  };

  const doSelectAll = () => {
    const sel = window.getSelection();
    if (sel) {
      const range = document.createRange();
      range.selectNodeContents(document.getElementById("root") ?? document.body);
      sel.removeAllRanges();
      sel.addRange(range);
    }
    setCtx(null);
  };

  return (
    <Portal>
      <Box
        ref={anchorRef as never}
        position="fixed"
        left={`${ctx.x}px`}
        top={`${ctx.y}px`}
        w="1px"
        h="1px"
        pointerEvents="none"
        zIndex={9999}
      />
      <Menu isOpen placement="bottom-start" onClose={() => setCtx(null)}>
        <MenuButton
          position="fixed"
          left={`${ctx.x}px`}
          top={`${ctx.y}px`}
          w="1px"
          h="1px"
          opacity={0}
          pointerEvents="none"
          aria-hidden
          tabIndex={-1}
        />
        <MenuList minW="160px" zIndex={10000}>
          {ctx.hasSelection && (
            <MenuItem icon={<Copy size={13} />} onClick={doCopySelection}>
              {t("gunCodes.ctxCopy")}
            </MenuItem>
          )}
          {ctx.codeText && (
            <MenuItem icon={<Copy size={13} />} onClick={doCopyCode}>
              {t("gunCodes.ctxCopyCode")}
            </MenuItem>
          )}
          <MenuItem icon={<MousePointerSquareDashed size={13} />} onClick={doSelectAll}>
            {t("gunCodes.ctxSelectAll")}
          </MenuItem>
        </MenuList>
      </Menu>
    </Portal>
  );
}
