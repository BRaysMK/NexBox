/**
 * 桌面歌词设置弹窗
 *
 * 可调节：
 * - 字体大小 (24~60px)
 * - 高亮颜色 (已唱)
 * - 底色 (未唱)
 * - 显示行数 (单行/双行)
 * - 实时预览
 */

import { useState, useEffect, memo } from "react";
import {
  Modal,
  ModalOverlay,
  ModalContent,
  ModalHeader,
  ModalCloseButton,
  ModalBody,
  ModalFooter,
  Button,
  Text,
  HStack,
  VStack,
  Box,
  Tooltip,
  useColorModeValue,
  ButtonGroup,
} from "@chakra-ui/react";
import { CustomColorPicker } from "@/components/special/custom-color-picker";
import { useMusicStore } from "@/stores/music-store";

interface DesktopLyricsSettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

// 复用桌面歌词的 range slider 样式
const rangeSliderSx = {
  "&": {
    appearance: "none",
    WebkitAppearance: "none",
    height: "6px",
    borderRadius: "3px",
    outline: "none",
    cursor: "pointer",
  },
  "&::-webkit-slider-runnable-track": {
    height: "6px",
    borderRadius: "3px",
  },
  "&::-webkit-slider-thumb": {
    WebkitAppearance: "none",
    width: "14px",
    height: "14px",
    borderRadius: "50%",
    marginTop: "-4px",
    border: "none",
  },
  "&::-moz-range-track": {
    height: "6px",
    borderRadius: "3px",
  },
  "&::-moz-range-thumb": {
    width: "14px",
    height: "14px",
    borderRadius: "50%",
    border: "none",
  },
};

function DesktopLyricsSettingsModalInner({
  isOpen,
  onClose,
}: DesktopLyricsSettingsModalProps) {
  const desktopLyricsFontSize = useMusicStore((s) => s.desktopLyricsFontSize);
  const desktopLyricsHighlightColor = useMusicStore((s) => s.desktopLyricsHighlightColor);
  const desktopLyricsBaseColor = useMusicStore((s) => s.desktopLyricsBaseColor);
  const desktopLyricsLineCount = useMusicStore((s) => s.desktopLyricsLineCount);

  const labelColor = useColorModeValue("gray.700", "#ffffff");
  const subLabelColor = useColorModeValue("gray.500", "#ffffff");
  const modalBg = useColorModeValue("white", "#111111");
  const modalBorderColor = useColorModeValue("gray.200", "#333333");
  const sliderTrackBg = useColorModeValue("rgba(0,0,0,0.1)", "rgba(255,255,255,0.9)");
  const previewBg = useColorModeValue("linear-gradient(135deg, #1a1a2e, #16213e)", "linear-gradient(135deg, #1a1a2e, #16213e)");

  // 预览用的模拟歌词
  const previewLines = [
    { text: "你是我最美的意外", progress: 0.5 },
    { text: "让我牵着你的手", progress: 0 },
  ];

  const accentColor = desktopLyricsHighlightColor;

  return (
    <Modal isOpen={isOpen} onClose={onClose} isCentered size="md">
      <ModalOverlay />
      <ModalContent bg={modalBg} borderColor={modalBorderColor} borderRadius="xl">
        <ModalHeader color={labelColor} fontSize="lg" fontWeight="bold">
          桌面歌词设置
        </ModalHeader>
        <ModalCloseButton />

        <ModalBody>
          <VStack spacing={5} align="stretch">
            {/* 字体大小 */}
            <Box>
              <HStack justify="space-between" mb={2}>
                <Text color={labelColor} fontSize="sm" fontWeight="medium">
                  字体大小
                </Text>
                <Text color={subLabelColor} fontSize="xs">
                  {desktopLyricsFontSize}px
                </Text>
              </HStack>
              <HStack spacing={2} align="center">
                <Text fontSize="xs" color={subLabelColor}>A</Text>
                <Box
                  as="input"
                  type="range"
                  min={24}
                  max={60}
                  step={2}
                  value={desktopLyricsFontSize}
                  onChange={(e) =>
                    useMusicStore.getState().setDesktopLyricsFontSize(
                      parseInt((e.target as HTMLInputElement).value)
                    )
                  }
                  w="100%"
                  sx={{
                    ...rangeSliderSx,
                    background: `linear-gradient(to right, ${accentColor} 0%, ${accentColor} ${((desktopLyricsFontSize - 24) / 36) * 100}%, ${sliderTrackBg} ${((desktopLyricsFontSize - 24) / 36) * 100}%, ${sliderTrackBg} 100%)`,
                    "&::-webkit-slider-thumb": {
                      ...rangeSliderSx["&::-webkit-slider-thumb"],
                      background: accentColor,
                    },
                    "&::-moz-range-thumb": {
                      ...rangeSliderSx["&::-moz-range-thumb"],
                      background: accentColor,
                    },
                  }}
                />
                <Text fontSize="md" color={subLabelColor} fontWeight="bold">A</Text>
              </HStack>
            </Box>

            {/* 颜色选择 */}
            <HStack spacing={6} align="start">
              <Box flex={1}>
                <Text color={labelColor} fontSize="sm" fontWeight="medium" mb={2}>
                  高亮颜色（已唱）
                </Text>
                <CustomColorPicker
                  color={desktopLyricsHighlightColor}
                  onChange={(c) => useMusicStore.getState().setDesktopLyricsHighlightColor(c)}
                />
              </Box>
              <Box flex={1}>
                <Text color={labelColor} fontSize="sm" fontWeight="medium" mb={2}>
                  底色（未唱）
                </Text>
                <CustomColorPicker
                  color={desktopLyricsBaseColor}
                  onChange={(c) => useMusicStore.getState().setDesktopLyricsBaseColor(c)}
                />
              </Box>
            </HStack>

            {/* 显示行数 */}
            <Box>
              <Text color={labelColor} fontSize="sm" fontWeight="medium" mb={2}>
                显示行数
              </Text>
              <ButtonGroup size="sm" variant="outline" isAttached>
                <Button
                  onClick={() => useMusicStore.getState().setDesktopLyricsLineCount(1)}
                  bg={desktopLyricsLineCount === 1 ? `${accentColor}22` : "transparent"}
                  borderColor={desktopLyricsLineCount === 1 ? accentColor : modalBorderColor}
                  color={desktopLyricsLineCount === 1 ? accentColor : subLabelColor}
                  fontWeight={desktopLyricsLineCount === 1 ? "bold" : "normal"}
                >
                  单行（仅当前句）
                </Button>
                <Button
                  onClick={() => useMusicStore.getState().setDesktopLyricsLineCount(2)}
                  bg={desktopLyricsLineCount === 2 ? `${accentColor}22` : "transparent"}
                  borderColor={desktopLyricsLineCount === 2 ? accentColor : modalBorderColor}
                  color={desktopLyricsLineCount === 2 ? accentColor : subLabelColor}
                  fontWeight={desktopLyricsLineCount === 2 ? "bold" : "normal"}
                >
                  双行（当前+下一句）
                </Button>
              </ButtonGroup>
            </Box>

            {/* 预览 */}
            <Box>
              <Text color={subLabelColor} fontSize="xs" mb={2}>
                预览效果：
              </Text>
              <Box
                borderRadius="lg"
                p={4}
                bg={previewBg}
                overflow="hidden"
              >
                <VStack spacing={2} align="center">
                  {/* 当前行预览 */}
                  <Box position="relative" overflow="hidden" textAlign="center">
                    <span
                      style={{
                        fontSize: `${desktopLyricsFontSize * 0.6}px`,
                        fontWeight: "bold",
                        color: desktopLyricsBaseColor,
                        textShadow: "-1px -1px 0 rgba(0,0,0,0.8), 1px -1px 0 rgba(0,0,0,0.8), -1px 1px 0 rgba(0,0,0,0.8), 1px 1px 0 rgba(0,0,0,0.8)",
                        whiteSpace: "nowrap",
                      }}
                    >
                      {previewLines[0].text}
                    </span>
                    <span
                      style={{
                        position: "absolute",
                        top: 0,
                        left: 0,
                        fontSize: `${desktopLyricsFontSize * 0.6}px`,
                        fontWeight: "bold",
                        color: desktopLyricsHighlightColor,
                        textShadow: "-1px -1px 0 rgba(0,0,0,0.8), 1px -1px 0 rgba(0,0,0,0.8), -1px 1px 0 rgba(0,0,0,0.8), 1px 1px 0 rgba(0,0,0,0.8)",
                        whiteSpace: "nowrap",
                        overflow: "hidden",
                        width: `${previewLines[0].progress * 100}%`,
                      }}
                    >
                      {previewLines[0].text}
                    </span>
                  </Box>
                  {/* 下一行预览 */}
                  {desktopLyricsLineCount === 2 && (
                    <span
                      style={{
                        fontSize: `${desktopLyricsFontSize * 0.6 * 0.7}px`,
                        fontWeight: "bold",
                        color: desktopLyricsBaseColor,
                        opacity: 0.5,
                        textShadow: "-1px -1px 0 rgba(0,0,0,0.6), 1px -1px 0 rgba(0,0,0,0.6), -1px 1px 0 rgba(0,0,0,0.6), 1px 1px 0 rgba(0,0,0,0.6)",
                      }}
                    >
                      {previewLines[1].text}
                    </span>
                  )}
                </VStack>
              </Box>
            </Box>
          </VStack>
        </ModalBody>

        <ModalFooter>
          <Button variant="ghost" onClick={onClose} color={labelColor}>
            完成
          </Button>
        </ModalFooter>
      </ModalContent>
    </Modal>
  );
}

export const DesktopLyricsSettingsModal = memo(DesktopLyricsSettingsModalInner);
