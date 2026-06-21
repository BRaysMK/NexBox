"use client";

import { createContext, useContext, useState, ReactNode, useEffect, useRef } from "react";
import { LazyStore } from "@tauri-apps/plugin-store";

type BackgroundMode = "none" | "preset" | "image" | "dynamic";

interface PresetBackground {
  id: number;
  name: string;
  path: string;
}

const PRESET_BACKGROUNDS: PresetBackground[] = [
  { id: 1, name: "预设1", path: "/backgrounds/preset1.jpg" },
  { id: 2, name: "预设2", path: "/backgrounds/preset2.jpg" },
  { id: 3, name: "预设3", path: "/backgrounds/preset3.png" },
  { id: 4, name: "预设4", path: "/backgrounds/preset4.png" },
];

interface BackgroundContextType {
  backgroundMode: BackgroundMode;
  customBgImages: string[];
  activeBgIndex: number;
  dynamicBgVideo: string | null;
  liquidGlassEnabled: boolean;
  activePresetIndex: number;
  presetBackgrounds: PresetBackground[];
  carouselEnabled: boolean;
  setBackgroundMode: (mode: BackgroundMode) => void;
  setCustomBgImages: (images: string[]) => void;
  addCustomBgImage: (image: string) => boolean;
  removeCustomBgImage: (index: number) => void;
  setActiveBgIndex: (index: number) => void;
  setDynamicBgVideo: (video: string | null) => void;
  setLiquidGlassEnabled: (enabled: boolean) => void;
  setActivePresetIndex: (index: number) => void;
  setCarouselEnabled: (enabled: boolean) => void;
}

const BackgroundContext = createContext<BackgroundContextType>({
  backgroundMode: "none",
  customBgImages: [],
  activeBgIndex: 0,
  dynamicBgVideo: null,
  liquidGlassEnabled: false,
  activePresetIndex: 0,
  presetBackgrounds: PRESET_BACKGROUNDS,
  carouselEnabled: false,
  setBackgroundMode: () => {},
  setCustomBgImages: () => {},
  addCustomBgImage: () => false,
  removeCustomBgImage: () => {},
  setActiveBgIndex: () => {},
  setDynamicBgVideo: () => {},
  setLiquidGlassEnabled: () => {},
  setActivePresetIndex: () => {},
  setCarouselEnabled: () => {},
});

export function useBackground() {
  return useContext(BackgroundContext);
}

const SETTINGS_FILE = "settings.json";
const store = new LazyStore(SETTINGS_FILE);
const MAX_BG_IMAGES = 3;

export function BackgroundProvider({ children }: { children: ReactNode }) {
  const [backgroundMode, setBackgroundMode] = useState<BackgroundMode>("none");
  const [customBgImages, setCustomBgImages] = useState<string[]>([]);
  const [activeBgIndex, setActiveBgIndex] = useState(0);
  const [dynamicBgVideo, setDynamicBgVideo] = useState<string | null>(null);
  const [liquidGlassEnabled, setLiquidGlassEnabled] = useState(false);
  const [activePresetIndex, setActivePresetIndex] = useState(0);
  const [carouselEnabled, setCarouselEnabled] = useState(false);
  const [isLoaded, setIsLoaded] = useState(false);
  const [isFirstLaunch, setIsFirstLaunch] = useState(false);

  useEffect(() => {
    async function loadSettings() {
      try {
        // 批量读取所有设置，减少 store IO 调用次数
        const [savedMode, savedImages, savedActiveIndex, savedDynamicVideo, savedLiquidGlass, savedActivePreset, savedCarousel, hasLaunched] =
          await Promise.all([
            store.get<string>("background-mode"),
            store.get<string[]>("custom-bg-images"),
            store.get<number>("active-bg-index"),
            store.get<string>("dynamic-bg-video"),
            store.get<boolean>("liquid-glass-enabled"),
            store.get<number>("active-preset-index"),
            store.get<boolean>("carousel-enabled"),
            store.get<boolean>("has-launched"),
          ]);

        // 检测是否首次启动
        if (!hasLaunched) {
          setIsFirstLaunch(true);
          // 首次启动默认启用预设背景
          setBackgroundMode("preset");
          setActivePresetIndex(0);
          await store.set("has-launched", true);
          await store.save();
        } else {
          // 非首次启动，加载保存的设置
          if (savedMode && ["none", "preset", "image", "dynamic"].includes(savedMode)) {
            setBackgroundMode(savedMode as BackgroundMode);
          }
          if (savedActivePreset !== null && savedActivePreset !== undefined) {
            setActivePresetIndex(savedActivePreset);
          }
          if (savedCarousel !== null && savedCarousel !== undefined) {
            setCarouselEnabled(savedCarousel);
          }
        }

        if (savedImages && Array.isArray(savedImages)) {
          setCustomBgImages(savedImages);
        }
        if (savedActiveIndex !== null && savedActiveIndex !== undefined) {
          setActiveBgIndex(savedActiveIndex);
        }
        if (savedDynamicVideo) {
          setDynamicBgVideo(savedDynamicVideo);
        }
        if (savedLiquidGlass !== null && savedLiquidGlass !== undefined) {
          setLiquidGlassEnabled(savedLiquidGlass);
        }

        setIsLoaded(true);
      } catch (error) {
        console.error("Failed to load background settings:", error);
        setIsLoaded(true);
      }
    }

    loadSettings();
  }, []);

  useEffect(() => {
    if (!isLoaded) return;

    async function saveSettings() {
      try {
        await store.set("background-mode", backgroundMode);

        if (customBgImages.length > 0) {
          await store.set("custom-bg-images", customBgImages);
        } else {
          await store.delete("custom-bg-images");
        }

        await store.set("active-bg-index", activeBgIndex);
        await store.set("active-preset-index", activePresetIndex);

        if (dynamicBgVideo) {
          await store.set("dynamic-bg-video", dynamicBgVideo);
        } else {
          await store.delete("dynamic-bg-video");
        }

        await store.set("liquid-glass-enabled", liquidGlassEnabled);
        await store.set("carousel-enabled", carouselEnabled);

        await store.save();
      } catch (error) {
        console.error("Failed to save background settings:", error);
      }
    }

    saveSettings();
  }, [backgroundMode, customBgImages, activeBgIndex, dynamicBgVideo, liquidGlassEnabled, activePresetIndex, carouselEnabled, isLoaded]);

  // 预设壁纸轮播定时器
  const carouselTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    if (backgroundMode === "preset" && carouselEnabled) {
      carouselTimerRef.current = setInterval(() => {
        setActivePresetIndex((prev) => (prev + 1) % PRESET_BACKGROUNDS.length);
      }, 10000);
    } else {
      if (carouselTimerRef.current) {
        clearInterval(carouselTimerRef.current);
        carouselTimerRef.current = null;
      }
    }

    return () => {
      if (carouselTimerRef.current) {
        clearInterval(carouselTimerRef.current);
        carouselTimerRef.current = null;
      }
    };
  }, [backgroundMode, carouselEnabled, setActivePresetIndex]);

  const addCustomBgImage = (image: string): boolean => {
    if (customBgImages.length >= MAX_BG_IMAGES) {
      return false;
    }
    setCustomBgImages([...customBgImages, image]);
    return true;
  };

  const removeCustomBgImage = (index: number) => {
    const newImages = customBgImages.filter((_, i) => i !== index);
    setCustomBgImages(newImages);
    if (activeBgIndex >= newImages.length) {
      setActiveBgIndex(Math.max(0, newImages.length - 1));
    }
  };

  return (
    <BackgroundContext.Provider
      value={{
        backgroundMode,
        customBgImages,
        activeBgIndex,
        dynamicBgVideo,
        liquidGlassEnabled,
        activePresetIndex,
        presetBackgrounds: PRESET_BACKGROUNDS,
        carouselEnabled,
        setBackgroundMode,
        setCustomBgImages,
        addCustomBgImage,
        removeCustomBgImage,
        setActiveBgIndex,
        setDynamicBgVideo,
        setLiquidGlassEnabled,
        setActivePresetIndex,
        setCarouselEnabled,
      }}
    >
      {children}
    </BackgroundContext.Provider>
  );
}
