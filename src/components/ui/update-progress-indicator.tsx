"use client";

import { Box, Flex, Text, useColorModeValue } from "@chakra-ui/react";
import { useUpdate } from "@/contexts/update-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import { LuRotateCw } from "react-icons/lu";
import { useTranslation } from "react-i18next";

/** 标题栏搜索框右侧的更新进度指示器：下载中显示主题色进度条，完成后显示"点击重启安装" */
export function UpdateProgressIndicator() {
  const { t } = useTranslation();
  const {
    isDownloading,
    downloadProgress,
    isDownloadComplete,
    toggleModal,
    handleInstall,
  } = useUpdate();
  const { getActiveColor, getContrastTextColor } = useThemeColor();
  const activeColor = getActiveColor();
  const contrastColor = getContrastTextColor();

  const barBg = useColorModeValue("gray.200", "whiteAlpha.200");
  const pillBg = useColorModeValue("whiteAlpha.800", "blackAlpha.700");
  const textColor = useColorModeValue("gray.700", "#ffffff");

  // 空闲状态不渲染
  if (!isDownloading && !isDownloadComplete) return null;

  // 下载完成：主题色胶囊按钮，点击重启安装
  if (isDownloadComplete) {
    return (
      <Flex
        as="button"
        align="center"
        gap={1.5}
        px={3}
        py={1}
        borderRadius="full"
        bg={activeColor}
        color={contrastColor}
        fontSize="xs"
        fontWeight="medium"
        whiteSpace="nowrap"
        cursor="pointer"
        className="update-pulse"
        transition="all 0.2s"
        _hover={{ transform: "scale(1.05)", boxShadow: `0 0 14px ${activeColor}66` }}
        onClick={handleInstall}
        title={t("settings.aboutSettings.updateModal.restartInstall")}
      >
        <LuRotateCw size={12} />
        <Text as="span">{t("settings.aboutSettings.updateModal.restartInstall")}</Text>
      </Flex>
    );
  }

  // 下载中：紧凑主题色进度胶囊，点击打开/收起详情弹窗
  return (
    <Flex
      as="button"
      align="center"
      gap={2}
      px={2}
      py={1}
      borderRadius="full"
      bg={pillBg}
      border="1px solid"
      borderColor={`${activeColor}55`}
      cursor="pointer"
      transition="all 0.2s"
      _hover={{ transform: "scale(1.04)", borderColor: activeColor }}
      onClick={toggleModal}
      title={t("settings.aboutSettings.updateModal.title")}
    >
      <Text fontSize="xs" color={textColor} fontWeight="medium" whiteSpace="nowrap">
        {t("settings.aboutSettings.newVersionDownloading")}
      </Text>
      <Box w="64px" h="6px" borderRadius="full" bg={barBg} overflow="hidden">
        <Box
          h="full"
          borderRadius="full"
          bg={activeColor}
          width={`${Math.max(0, Math.min(100, downloadProgress))}%`}
          transition="width 0.2s"
        />
      </Box>
      <Text
        fontSize="xs"
        color={textColor}
        fontWeight="medium"
        minW="44px"
        textAlign="right"
      >
        {Math.round(downloadProgress)}%
      </Text>
    </Flex>
  );
}
