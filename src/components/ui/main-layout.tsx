"use client";

import { Box, useColorModeValue } from "@chakra-ui/react";
import { ReactNode, useEffect, useRef, useState, useMemo } from "react";
import { Sidebar } from "./sidebar";
import { TitleBar } from "./title-bar";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import { convertFileSrc } from "@tauri-apps/api/core";

interface MainLayoutProps {
  children: ReactNode;
}

export function MainLayout({ children }: MainLayoutProps) {
  const bgColor = useColorModeValue("#fafafa", "#0a0a0a");
  const { backgroundMode, customBgImages, activeBgIndex, dynamicBgVideo, activePresetIndex, presetBackgrounds } = useBackground();
  const { config } = useThemeColor();
  const videoRef = useRef<HTMLVideoElement>(null);
  const [videoReady, setVideoReady] = useState(false);
  const idCounter = useRef(0);

  const activeImage = customBgImages[activeBgIndex];
  const activePreset = presetBackgrounds[activePresetIndex];
  const showImageBg = backgroundMode === "image" && activeImage;
  const showDynamicBg = backgroundMode === "dynamic" && dynamicBgVideo;
  const showPresetBg = backgroundMode === "preset" && activePreset;
  const videoSrc = useMemo(() => {
    if (!dynamicBgVideo) return null;
    return convertFileSrc(dynamicBgVideo);
  }, [dynamicBgVideo]);

  // 背景交叉淡化
  interface BgLayer {
    url: string;
    id: number;
    fading: boolean;
  }
  const [bgLayers, setBgLayers] = useState<BgLayer[]>([]);

  useEffect(() => {
    const src = showPresetBg
      ? activePreset?.path ?? null
      : showImageBg
        ? activeImage
        : null;

    if (!src) {
      setBgLayers([]);
      return;
    }

    const newId = ++idCounter.current;

    setBgLayers((prev) => [
      ...prev.map((l) => ({ ...l, fading: true })),
      { url: src, id: newId, fading: false },
    ]);

    // 动画完成后移除旧的图层
    const timer = setTimeout(() => {
      setBgLayers((prev) => prev.filter((l) => l.id === newId));
    }, 600);

    return () => clearTimeout(timer);
  }, [showPresetBg, activePresetIndex, showImageBg, activeBgIndex, activeImage, activePreset]);

  useEffect(() => {
    if (!dynamicBgVideo) {
      setVideoReady(false);
    } else {
      setVideoReady(false);
    };
  }, [dynamicBgVideo]);

  useEffect(() => {
    let bgColorToUse = bgColor;

    if (showImageBg || showPresetBg || showDynamicBg) {
      bgColorToUse = "transparent";
    }

    document.body.style.backgroundColor = bgColorToUse;

    return () => {
      document.body.style.backgroundColor = "";
    };
  }, [showImageBg, showDynamicBg, showPresetBg, bgColor]);

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
      {/* 背景交叉淡化层：用 <img> 而非 CSS background-image，
          原因：CSS value 有约 2MB 上限，大图 data URL 会被截断导致黑色 */}
      <Box sx={{
        "@keyframes bgFadeIn": {
          from: { opacity: 0 },
          to: { opacity: 1 },
        },
      }}>
        {bgLayers.map((layer) => (
          <Box
            key={layer.id}
            position="fixed"
            top={0}
            left={0}
            right={0}
            bottom={0}
            zIndex={-1}
            opacity={layer.fading ? 0 : 1}
            transition={layer.fading ? "opacity 0.5s ease-in-out" : undefined}
            animation={!layer.fading ? "bgFadeIn 0.5s ease-in-out" : undefined}
          >
            <img
              src={layer.url}
              alt=""
              style={{
                width: "100%",
                height: "100%",
                objectFit: "cover",
                display: "block",
                // GPU 合成层：避免背景图层重绘时影响悬浮框等覆盖层性能
                willChange: "transform" as any,
                transform: "translateZ(0)",
              }}
            />
          </Box>
        ))}
      </Box>
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
            src={videoSrc!}
            autoPlay
            muted
            loop
            playsInline
            preload="auto"
            onLoadedData={() => setVideoReady(true)}
            onError={(e) => console.error("视频加载失败:", e)}
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
