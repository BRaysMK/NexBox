"use client";

import { Box, Flex, HStack, IconButton, Image, useColorModeValue } from "@chakra-ui/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { LuMinus, LuX } from "react-icons/lu";
import { useCallback, useState, useEffect } from "react";
import { GlobalSearch } from "./global-search";
import { CloseConfirmDialog } from "../CloseConfirmDialog";

export function TitleBar() {
  const iconColor = useColorModeValue("gray.600", "gray.400");
  const hoverColor = useColorModeValue("gray.800", "gray.200");
  const closeHoverColor = useColorModeValue("red.600", "red.400");
  const minimizeHoverBg = useColorModeValue("gray.100", "gray.700");
  const closeHoverBg = useColorModeValue("red.50", "red.900");
  const bgColor = useColorModeValue("whiteAlpha.800", "blackAlpha.800");
  const logoSrc = useColorModeValue("/logo/NexBoxW.png", "/logo/NexBoxB.png");

  const [showCloseDialog, setShowCloseDialog] = useState(false);

  const getCloseBehavior = useCallback(() => {
    return localStorage.getItem("nexbox_close_behavior") || "ask";
  }, []);

  const handleMouseDown = useCallback(async (e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    if (target.closest("button") || target.closest("input") || target.closest('[role="search"]')) {
      return;
    }
    try {
      const appWindow = getCurrentWindow();
      await appWindow.startDragging();
    } catch (error) {
      console.error("Failed to start dragging:", error);
    }
  }, []);

  const handleMinimize = async () => {
    try {
      const appWindow = getCurrentWindow();
      await appWindow.minimize();
    } catch (error) {
      console.error("Failed to minimize window:", error);
    }
  };

  const handleClose = async () => {
    const behavior = getCloseBehavior();
    switch (behavior) {
      case "close":
        await performClose();
        break;
      case "minimize":
        await performMinimizeToTray();
        break;
      case "ask":
      default:
        setShowCloseDialog(true);
        break;
    }
  };

  const performMinimizeToTray = async (savePreference: boolean = false) => {
    if (savePreference) {
      localStorage.setItem("nexbox_close_behavior", "minimize");
    }
    try {
      await invoke("minimize_to_tray");
    } catch (error) {
      console.error("Failed to minimize to tray:", error);
    }
    setShowCloseDialog(false);
  };

  const performClose = async (savePreference: boolean = false) => {
    if (savePreference) {
      localStorage.setItem("nexbox_close_behavior", "close");
    }
    try {
      const appWindow = getCurrentWindow();
      await appWindow.close();
    } catch (error) {
      console.error("Failed to close window:", error);
    }
    setShowCloseDialog(false);
  };

  return (
    <>
      <Box
        position="fixed"
        top={0}
        left={0}
        right={0}
        h="48px"
        zIndex={999}
        onMouseDown={handleMouseDown}
      >
        <Flex justify="space-between" align="center" h="full" pl={4} pr={4}>
          <Box ml="112px" onMouseDown={(e) => e.stopPropagation()}>
            <GlobalSearch />
          </Box>
          <HStack spacing={1} h="40px" align="center">
            <IconButton
              icon={<LuMinus size={18} />}
              aria-label="最小化"
              variant="solid"
              borderRadius="full"
              bg={bgColor}
              backdropFilter="blur(10px)"
              color={iconColor}
              h="36px"
              minW="36px"
              w="36px"
              _hover={{
                color: hoverColor,
                bg: minimizeHoverBg,
              }}
              onClick={handleMinimize}
            />
            <IconButton
              icon={<LuX size={18} />}
              aria-label="关闭"
              variant="solid"
              borderRadius="full"
              bg={bgColor}
              backdropFilter="blur(10px)"
              color={iconColor}
              h="36px"
              minW="36px"
              w="36px"
              _hover={{
                color: closeHoverColor,
                bg: closeHoverBg,
              }}
              onClick={handleClose}
            />
          </HStack>
        </Flex>
      </Box>

      <CloseConfirmDialog
        isOpen={showCloseDialog}
        onClose={() => setShowCloseDialog(false)}
        onCloseApp={(savePreference) => performClose(savePreference)}
        onMinimizeToTray={(savePreference) => performMinimizeToTray(savePreference)}
      />
    </>
  );
}
