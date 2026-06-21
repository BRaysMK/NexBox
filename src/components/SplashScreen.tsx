import { useAppStartup } from "@/contexts/app-startup-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import { Box } from "@chakra-ui/react";
import { useEffect, useState } from "react";

const DEFAULT_LOGO = "/logo/Chinesew.png";

export function SplashScreen() {
  const { startupProgress } = useAppStartup();
  const { getActiveColor } = useThemeColor();
  const primaryColor = getActiveColor();
  const [logoSrc, setLogoSrc] = useState(DEFAULT_LOGO);

  useEffect(() => {
    const customLogo = localStorage.getItem("nexbox_splash_logo");
    if (customLogo) {
      setLogoSrc(customLogo);
    }
  }, []);

  return (
    <Box
      w="100vw"
      h="100vh"
      bg="#000"
      position="fixed"
      top="0"
      left="0"
      zIndex="9999"
      data-tauri-drag-region
      opacity={startupProgress >= 100 ? 0 : 1}
      transition="opacity 0.4s ease-out"
      willChange="opacity"
    >
      {/* Logo - 居中 */}
      <Box
        position="absolute"
        top="50%"
        left="50%"
        transform="translate(-50%, -50%)"
        data-tauri-drag-region
      >
        <Box
          as="img"
          src={logoSrc}
          alt="NexBox Logo"
          maxH="150px"
          maxW="300px"
          objectFit="contain"
          draggable={false}
        />
      </Box>
      
      {/* 进度条 - 中下方 */}
      <Box
        position="absolute"
        bottom="25%"
        left="50%"
        transform="translateX(-50%)"
        w="40%"
        maxW="160px"
        pointerEvents="none"
      >
        <Box 
          bg="rgba(255,255,255,0.15)" 
          h="2px" 
          borderRadius="full" 
          overflow="hidden"
        >
          <Box
            h="100%"
            bg={primaryColor}
            w={`${startupProgress}%`}
            transition="width 200ms ease-out"
            willChange="width"
          />
        </Box>
      </Box>
    </Box>
  );
}