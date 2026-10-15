import { Box, Text, HStack, VStack, Button, useDisclosure, useColorModeValue } from "@chakra-ui/react";
import { FaBug, FaBook } from "react-icons/fa6";
import { Smartphone } from "lucide-react";
import type { MouseEvent, ReactNode } from "react";
import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { QqGroupModal } from "@/components/ui/qq-group-modal";
import { QqGroupIcon } from "@/components/ui/qq-group-icon";
// 已阉割：远程接入弹窗
// import { RemoteAccessDialog } from "@/components/remote-access-dialog";
import { openExternal, useQQGroups } from "@/hooks/use-qq-groups";
import { useThemeColor } from "@/contexts/theme-color-context";
import { store } from "@/lib/store";

const FEEDBACK_URL = "https://nexbox.top/feedback";
const DOCS_URL = "https://docs.nexbox.top";
/** 本地兜底的 QQ 群图标 */
const LOCAL_QQ_ICON = "/icons/qq.webp";

/**
 * 主页「手机远程」（手机控制）卡片展示开关：
 * false = 主页不渲染该卡片，设置页同步隐藏对应显示开关；需要恢复时改回 true。
 * 已阉割：远程访问模块（remote_access / remote_monitor）已从本分支移除，故置 false。
 */
export const SHOW_REMOTE_ACCESS_CARD = false;

/** 读取持久化开关（兼容旧 localStorage），并订阅设置页变更事件 */
function useCardEnabled(key: string, event: string) {
  const [state, setState] = useState({ enabled: false, ready: false });

  useEffect(() => {
    (async () => {
      let enabled = true;
      const saved = await store.get<boolean>(key);
      if (saved !== null && saved !== undefined) {
        enabled = saved;
      } else {
        const ls = localStorage.getItem(key);
        if (ls !== null) enabled = ls === "true";
      }
      setState((s) => ({ ...s, enabled, ready: true }));
    })();
  }, [key]);

  useEffect(() => {
    const handler = (e: CustomEvent) => setState((s) => ({ ...s, enabled: e.detail }));
    window.addEventListener(event, handler as EventListener);
    return () => window.removeEventListener(event, handler as EventListener);
  }, [event]);

  return state;
}

/** 首页「问题反馈」卡片显示开关（持久化，默认开启） */
export function useFeedbackEnabled() {
  return useCardEnabled("nexbox_feedback_enabled", "feedback-setting-changed");
}

/** 首页「官方QQ群」卡片显示开关（持久化，默认开启） */
export function useQqGroupCardEnabled() {
  return useCardEnabled("nexbox_qq_group_card_enabled", "qq-group-card-setting-changed");
}

/** 首页「使用文档」卡片显示开关（持久化，默认开启） */
export function useDocsCardEnabled() {
  return useCardEnabled("nexbox_docs_card_enabled", "docs-card-setting-changed");
}

// 已阉割：useRemoteAccessCardEnabled（远程访问模块已移除）

/** 右上角方形图标块（80px）：图标 + 标题，副标题改为悬停 tooltip；纯浅色/深色适配（不用主题主色），不参与路由切换弹跳动画 */
function FeedbackLinkCard({
  icon,
  title,
  subtitle,
  onClick,
}: {
  icon: ReactNode;
  title: string;
  subtitle: string;
  onClick: () => void;
}) {
  const { getActiveColor } = useThemeColor();
  const titleColor = useColorModeValue("gray.800", "#ffffff");
  const iconBg = useColorModeValue("#f1f2f4", "#262626");

  return (
    <LiquidGlassCard
      className="no-bounce"
      w="80px"
      h="80px"
      p={0}
      gap={1.5}
      display="flex"
      flexDirection="column"
      alignItems="center"
      justifyContent="center"
      cursor="pointer"
      onClick={onClick}
      title={subtitle}
      transition="border-color 0.2s"
      _hover={{ borderColor: getActiveColor() }}
    >
      <Box
        w="34px"
        h="34px"
        borderRadius="lg"
        bg={iconBg}
        display="flex"
        alignItems="center"
        justifyContent="center"
        flexShrink={0}
        overflow="hidden"
      >
        {icon}
      </Box>
      <Text fontSize="xs" fontWeight="medium" color={titleColor} noOfLines={1} maxW="72px" textAlign="center">
        {title}
      </Text>
    </LiquidGlassCard>
  );
}

/** 首页「问题反馈」卡片：点击跳转官网反馈 */
export function FeedbackCard() {
  const { t } = useTranslation();
  // 注意：react-icons 的 <svg color> 不经过 Chakra 主题解析，必须传真实 hex，
  // 不能用 Chakra token（如 gray.800），否则浅色模式下图标会退化为白色
  const iconColor = useColorModeValue("#1a202c", "#ffffff");
  return (
    <FeedbackLinkCard
      icon={<FaBug size={18} color={iconColor} />}
      title={t("home.feedbackCard.title")}
      subtitle={t("home.feedbackCard.subtitle")}
      onClick={() => openExternal(FEEDBACK_URL)}
    />
  );
}

/** 首页「使用文档」卡片：点击跳转在线文档 */
export function DocsCard() {
  const { t } = useTranslation();
  // 注意：react-icons 的 <svg color> 不经过 Chakra 主题解析，必须传真实 hex，
  // 不能用 Chakra token（如 gray.800），否则浅色模式下图标会退化为白色
  const iconColor = useColorModeValue("#1a202c", "#ffffff");
  return (
    <FeedbackLinkCard
      icon={<FaBook size={18} color={iconColor} />}
      title={t("home.docsCard.title")}
      subtitle={t("home.docsCard.subtitle")}
      onClick={() => openExternal(DOCS_URL)}
    />
  );
}

/** 首页「官方QQ群」卡片：图标取①群的 gitee 图标（后端下载显示），打开弹窗 */
export function QqGroupCard() {
  const { t } = useTranslation();
  const { isOpen, onOpen, onClose } = useDisclosure();
  const { groups } = useQQGroups();

  return (
    <>
      <FeedbackLinkCard
        icon={<QqGroupIcon url={groups[0]?.icon} size={34} />}
        title={t("home.qqGroup.cardTitle")}
        subtitle={t("home.qqGroup.cardSubtitle")}
        onClick={onOpen}
      />
      <QqGroupModal isOpen={isOpen} onClose={onClose} />
    </>
  );
}
