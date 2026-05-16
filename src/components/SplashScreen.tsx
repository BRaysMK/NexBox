import { useAppStartup } from "@/contexts/app-startup-context";
import { Box, Text, VStack } from "@chakra-ui/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef, useState } from "react";

const DEFAULT_LOGO = "/logo/Chinesew.png";

export function SplashScreen() {
  const { startupProgress, startupMessage } = useAppStartup();
  const [logoSrc, setLogoSrc] = useState(DEFAULT_LOGO);
  const textColor = "#fff";
  const subTextColor = "rgba(255,255,255,0.7)";
  const isDragging = useRef(false);

  useEffect(() => {
    const customLogo = localStorage.getItem("nexbox_splash_logo");
    if (customLogo) {
      setLogoSrc(customLogo);
    }
  }, []);

  const handleMouseDown = useCallback(async (e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    if (target.closest("button") || target.closest("a")) {
      return;
    }
    if (isDragging.current) {
      return;
    }
    isDragging.current = true;
    try {
      const appWindow = getCurrentWindow();
      await appWindow.startDragging();
    } catch (error) {
      console.error("Failed to start dragging:", error);
    } finally {
      // 重置拖动结束后重置标记
      setTimeout(() => {
        isDragging.current = false;
      }, 100);
    }
  }, []);

  return (
    <Box
      w="100vw"
      h="100vh"
      bg="#000"
      display="flex"
      alignItems="center"
      justifyContent="center"
      position="fixed"
      top="0"
      left="0"
      zIndex="9999"
      onMouseDown={handleMouseDown}
      pointerEvents="auto"
      opacity={startupProgress >= 100 ? 0 : 1}
      transition="opacity 0.4s ease-out"
      willChange="opacity"
    >
      <VStack spacing="8" maxW="400px" w="100%" px="8">
        <Box
          as="img"
          src={logoSrc}
          alt="NexBox Logo"
          maxH="150px"
          maxW="300px"
          objectFit="contain"
          draggable={false}
        />
        <VStack w="100%" spacing="4">
          <Box w="100%" bg="rgba(255,255,255,0.08)" h="3px" borderRadius="full" overflow="hidden">
            <Box
              h="100%"
              bg="white"
              w={`${startupProgress}%`}
              transition="width 200ms linear"
              willChange="width"
            />
          </Box>
          <Text color={subTextColor} fontSize="sm" textAlign="center">
            {startupMessage}
          </Text>
        </VStack>
      </VStack>
    </Box>
  );
}