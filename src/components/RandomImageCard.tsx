import React, { useEffect, useState } from "react";
import { Box, Text, HStack, VStack, useColorModeValue, useDisclosure } from "@chakra-ui/react";
import { useTranslation } from "react-i18next";
import { FaImage } from "react-icons/fa6";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { store } from "@/lib/store";
import { initVisibility, subscribeVisibility } from "@/lib/ui-visibility";
import RandomImageModal from "@/components/RandomImageModal";

/** 主页「随机图片」卡片显示开关（store 持久化，默认开启） */
export function useRandomImageEnabled() {
  const [enabled, setEnabled] = useState(() => initVisibility("nexbox_random_image_enabled"));

  useEffect(() =>
    subscribeVisibility("nexbox_random_image_enabled", "random-image-setting-changed", setEnabled, store),
  []);

  return enabled;
}

/** 主页「随机图片」卡片：与 Win 键卡片同尺寸，点击打开弹窗选择类别并生成图片 */
export default function RandomImageCard() {
  const { t } = useTranslation();
  const { isOpen, onOpen, onClose } = useDisclosure();

  const textColor = useColorModeValue("gray.800", "#ffffff");
  const subTextColor = useColorModeValue("gray.600", "#ffffff");

  return (
    <>
      <LiquidGlassCard px={3} py={2} boxShadow="sm" minW="260px" maxW="360px" cursor="pointer" onClick={onOpen}>
        <VStack spacing={1} align="start">
          <HStack spacing={3} align="center" justify="space-between" w="full">
            <HStack spacing={2} minW={0}>
              <Text fontSize="sm" color={textColor} fontWeight="semibold" noOfLines={1}>
                {t("home.randomImage.title") || "随机图片"}
              </Text>
              <FaImage size={16} color={textColor} style={{ flexShrink: 0 }} />
            </HStack>
          </HStack>
          <Text fontSize="xs" color={subTextColor} noOfLines={1}>
            {t("home.randomImage.subtitle") || "点击打开随机图片"}
          </Text>
        </VStack>
      </LiquidGlassCard>
      <RandomImageModal isOpen={isOpen} onClose={onClose} />
    </>
  );
}
