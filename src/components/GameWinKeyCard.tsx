import React, { useEffect, useState } from "react";
import { Box, Text, HStack, VStack, useColorModeValue, Spinner } from "@chakra-ui/react";
import { useTranslation } from "react-i18next";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { ThemeSwitch } from "@/components/special/theme-switch";
import { FaWindows } from "react-icons/fa6";
import { invoke } from "@tauri-apps/api/core";

export default function GameWinKeyCard() {
  const { t } = useTranslation();
  const [enabled, setEnabled] = useState(false);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);

  const textColor = useColorModeValue("gray.800", "#e6e6e6");
  const subTextColor = useColorModeValue("gray.600", "#a0a0a0");

  useEffect(() => {
    let mounted = true;
    (async () => {
      try {
        const v = await invoke<boolean>("get_game_win_key_status");
        if (mounted) setEnabled(v);
      } catch {
        // ignore
      } finally {
        if (mounted) setLoading(false);
      }
    })();

    // 监听设置页开关变化，保持状态同步
    const handler = (e: CustomEvent) => {
      if (mounted) setEnabled(e.detail);
    };
    window.addEventListener("game-win-key-setting-changed", handler as EventListener);
    return () => {
      mounted = false;
      window.removeEventListener("game-win-key-setting-changed", handler as EventListener);
    };
  }, []);

  const handleToggle = async (val: boolean) => {
    setBusy(true);
    try {
      await invoke("set_game_win_key_enabled", { enabled: val });
      setEnabled(val);
      // 广播事件同步设置页开关
      window.dispatchEvent(new CustomEvent("game-win-key-setting-changed", { detail: val }));
    } catch {
      // ignore
    } finally {
      setBusy(false);
    }
  };

  return (
    <LiquidGlassCard px={3} py={2} boxShadow="sm" minW="260px" maxW="360px">
      {loading ? (
        <HStack spacing={3} align="center">
          <Spinner size="sm" />
          <Text fontSize="sm">{t("home.gameWinKey.loading") || t("home.loading")}</Text>
        </HStack>
      ) : (
        <VStack spacing={1} align="start">
          <HStack spacing={3} align="center" justify="space-between" w="full">
            <HStack spacing={2} minW={0}>
              <Text fontSize="sm" color={textColor} fontWeight="semibold" noOfLines={1}>
                {t("home.gameWinKey.title") || "游戏时禁用 Win 键"}
              </Text>
              <FaWindows size={16} color={textColor} style={{ flexShrink: 0 }} />
            </HStack>
            <ThemeSwitch
              isChecked={enabled}
              onChange={(e) => handleToggle(e.target.checked)}
              isDisabled={busy}
            />
          </HStack>
          <Text fontSize="xs" color={subTextColor} noOfLines={1}>
            {t("home.gameWinKey.subtitle") || "检测到名单内游戏运行时自动屏蔽 Win 键"}
          </Text>
        </VStack>
      )}
    </LiquidGlassCard>
  );
}
