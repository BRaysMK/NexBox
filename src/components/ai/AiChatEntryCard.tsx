import React, { useEffect, useState } from "react";
import { Box, Text, HStack, VStack, useColorModeValue, useDisclosure } from "@chakra-ui/react";
import { useTranslation } from "react-i18next";
import { motion } from "framer-motion";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useThemeColor } from "@/contexts/theme-color-context";
import { hexToRgba } from "@/lib/color-utils";
import { store } from "@/lib/store";
import { initVisibility, subscribeVisibility } from "@/lib/ui-visibility";
import AiChatModal from "@/components/ai/AiChatModal";
import boxcatImg from "@/assets/boxcat.png";

/** 主页「盒子喵」卡片显示开关（store 持久化，默认开启） */
export function useAiEntryEnabled() {
  const [enabled, setEnabled] = useState(() => initVisibility("nexbox_ai_enabled"));

  useEffect(() =>
    subscribeVisibility("nexbox_ai_enabled", "ai-setting-changed", setEnabled, store),
  []);

  return enabled;
}

/** 主页「盒子喵」AI 助手入口卡片：猫娘头像 + 名字，点击打开聊天弹窗 */
export default function AiChatEntryCard() {
  const { t } = useTranslation();
  const { isOpen, onOpen, onClose } = useDisclosure();
  const { getActiveColor, getHoverColor } = useThemeColor();
  const primary = getActiveColor();

  const textColor = useColorModeValue("gray.800", "#ffffff");
  const subTextColor = useColorModeValue("gray.600", "#A0AEC0");

  return (
    <>
      <LiquidGlassCard
        px={3}
        py={2}
        boxShadow="sm"
        minW="260px"
        maxW="360px"
        cursor="pointer"
        onClick={onOpen}
        position="relative"
        overflow="visible"
      >
        <VStack spacing={1} align="start">
          <HStack spacing={3} align="center" w="full">
            {/* 猫娘头像：boxcat.png + 主题色光晕 + 悬浮动效 */}
            <motion.div
              animate={{ y: [0, -3, 0] }}
              transition={{ duration: 3, repeat: Infinity, ease: "easeInOut" }}
            >
              <motion.div
                animate={{
                  boxShadow: [
                    `0 0 10px ${hexToRgba(primary, 0.45)}`,
                    `0 0 22px ${hexToRgba(primary, 0.95)}`,
                    `0 0 10px ${hexToRgba(primary, 0.45)}`,
                  ],
                }}
                transition={{ duration: 2.5, repeat: Infinity, ease: "easeInOut" }}
                style={{ borderRadius: "50%" }}
              >
                <Box
                  w="34px"
                  h="34px"
                  borderRadius="full"
                  overflow="hidden"
                  display="flex"
                  alignItems="center"
                  justifyContent="center"
                  bg={`linear-gradient(135deg, ${getHoverColor()}, ${primary})`}
                  border="2px solid"
                  borderColor={hexToRgba(primary, 0.7)}
                >
                  <img
                    src={boxcatImg}
                    alt="BoxCat"
                    style={{ width: "100%", height: "100%", objectFit: "cover", display: "block" }}
                  />
                </Box>
              </motion.div>
            </motion.div>
            <Box minW={0}>
              <HStack spacing={1} align="center">
                <Text fontSize="sm" color={textColor} fontWeight="semibold" noOfLines={1}>
                  {t("ai.name", "盒子喵")}
                </Text>
                <Text fontSize="xs" color={primary} fontWeight="bold">
                  {t("ai.liveTag", "AI")}
                </Text>
              </HStack>
              <Text fontSize="xs" color={subTextColor} noOfLines={1}>
                {t("ai.entrySubtitle", "点击和盒子喵聊天喵~")}
              </Text>
            </Box>
          </HStack>
        </VStack>
      </LiquidGlassCard>
      <AiChatModal isOpen={isOpen} onClose={onClose} />
    </>
  );
}