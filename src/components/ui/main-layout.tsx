"use client";

import { Box, useColorModeValue } from "@chakra-ui/react";
import { ReactNode, useEffect, useRef, useState } from "react";
import { Sidebar } from "./sidebar";
import { TitleBar } from "./title-bar";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";

interface MainLayoutProps {
  children: ReactNode;
}

export function MainLayout({ children }: MainLayoutProps) {
  const bgColor = useColorModeValue("#fafafa", "#0a0a0a");
  const { backgroundMode, customBgImages, activeBgIndex, dynamicBgVideo, activePresetIndex, presetBackgrounds } = useBackground();
  const { config } = useThemeColor();
  const videoRef = useRef<HTMLVideoElement>(null);
  const [videoReady, setVideoReady] = useState(false);

  const activeImage = customBgImages[activeBgIndex];
  const activePreset = presetBackgrounds[activePresetIndex];
  const showImageBg = backgroundMode === "image" && activeImage;
  const showDynamicBg = backgroundMode === "dynamic" && dynamicBgVideo;
  const showPresetBg = backgroundMode === "preset" && activePreset;

  // 预加载视频：当 dynamicBgVideo 变化时，提前创建 link 预加载
  useEffect(() => {
    if (!dynamicBgVideo) {
      setVideoReady(false);
      return;
    }

    setVideoReady(false);

    // 添加 <link rel="preload"> 提示浏览器提前下载视频
    const link = document.createElement("link");
    link.rel = "preload";
    link.as = "video";
    link.href = dynamicBgVideo;
    document.head.appendChild(link);

    return () => {
      document.head.removeChild(link);
    };
  }, [dynamicBgVideo]);

  useEffect(() => {
    let bgImage = "";
    let bgColorToUse = bgColor;

    if (showImageBg) {
      bgImage = `url(${activeImage})`;
      bgColorToUse = "transparent";
    } else if (showPresetBg) {
      bgImage = `url(${activePreset.path})`;
      bgColorToUse = "transparent";
    } else if (showDynamicBg) {
      bgColorToUse = "transparent";
    }

    if (bgImage) {
      document.body.style.backgroundImage = bgImage;
      document.body.style.backgroundSize = "cover";
      document.body.style.backgroundPosition = "center";
      document.body.style.backgroundRepeat = "no-repeat";
      document.body.style.backgroundAttachment = "fixed";
      document.body.style.backgroundColor = bgColorToUse;
    } else {
      document.body.style.backgroundImage = "none";
      document.body.style.backgroundColor = bgColorToUse;
    }
    return () => {
      document.body.style.backgroundImage = "";
      document.body.style.backgroundSize = "";
      document.body.style.backgroundPosition = "";
      document.body.style.backgroundRepeat = "";
      document.body.style.backgroundAttachment = "";
      document.body.style.backgroundColor = "";
    };
  }, [showImageBg, showDynamicBg, showPresetBg, activeImage, activePreset, bgColor]);

  useEffect(() => {
    if (videoRef.current && dynamicBgVideo) {
      videoRef.current.play().catch(() => {});
    }
  }, [dynamicBgVideo, showDynamicBg]);

  return (
    <Box
      position="relative"
      minHeight="100vh"
      bg="transparent"
    >
      {showDynamicBg && (
        <Box
          position="fixed"
          top={0}
          left={0}
          right={0}
          bottom={0}
          zIndex={-1}
          overflow="hidden"
          opacity={videoReady ? 1 : 0}
          transition="opacity 0.6s ease-in"
        >
          <video
            ref={videoRef}
            src={dynamicBgVideo!}
            autoPlay
            muted
            loop
            playsInline
            preload="auto"
            onLoadedData={() => setVideoReady(true)}
            style={{
              width: "100%",
              height: "100%",
              objectFit: "cover",
            }}
          />
        </Box>
      )}
      <TitleBar />
      <Sidebar />
      <Box 
        ml="96px" 
        pt="56px"
        pb={8}
        px={8} 
        pr="40px" 
        overflowY="auto" 
        h="calc(100vh)"
        sx={{
          "&::-webkit-scrollbar": {
            width: "6px",
            height: "6px",
          },
          "&::-webkit-scrollbar-track": {
            background: "transparent",
            margin: "10px 0",
          },
          "&::-webkit-scrollbar-thumb": {
            background: config.primaryColor,
            borderRadius: "3px",
            minHeight: "40px",
          },
          "&::-webkit-scrollbar-thumb:hover": {
            background: config.primaryColor,
            opacity: 0.8,
            filter: "brightness(0.9)",
          },
        }}
      >
        <Box position="relative" minHeight="100%">
          {children}
        </Box>
      </Box>
    </Box>
  );
}
