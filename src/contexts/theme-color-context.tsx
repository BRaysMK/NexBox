"use client";

import { createContext, useContext, useState, ReactNode, useEffect } from "react";
import { LazyStore } from "@tauri-apps/plugin-store";
import { hexToRgba, getContrastColor, isValidHexColor, normalizeHexColor } from "@/lib/color-utils";

export interface ThemeColorConfig {
  primaryColor: string;
  hoverOpacity: number;
  activeOpacity: number;
  borderOpacity: number;
}

export const DEFAULT_THEME_COLOR_CONFIG: ThemeColorConfig = {
  primaryColor: "#98DDD0",
  hoverOpacity: 0.2,
  activeOpacity: 0.3,
  borderOpacity: 0.4,
};

interface ThemeColorContextType {
  config: ThemeColorConfig;
  setPrimaryColor: (color: string) => void;
  setHoverOpacity: (opacity: number) => void;
  setActiveOpacity: (opacity: number) => void;
  setBorderOpacity: (opacity: number) => void;
  resetToDefault: () => void;
  getHoverColor: (isDark?: boolean) => string;
  getActiveColor: () => string;
  getBorderColor: () => string;
  getContrastTextColor: () => string;
}

const ThemeColorContext = createContext<ThemeColorContextType>({
  config: DEFAULT_THEME_COLOR_CONFIG,
  setPrimaryColor: () => {},
  setHoverOpacity: () => {},
  setActiveOpacity: () => {},
  setBorderOpacity: () => {},
  resetToDefault: () => {},
  getHoverColor: () => "rgba(152,221,208,0.2)",
  getActiveColor: () => "#98DDD0",
  getBorderColor: () => "rgba(152,221,208,0.4)",
  getContrastTextColor: () => "#1a1a1a",
});

export function useThemeColor() {
  return useContext(ThemeColorContext);
}

const SETTINGS_FILE = "settings.json";
const store = new LazyStore(SETTINGS_FILE);

export function ThemeColorProvider({ children }: { children: ReactNode }) {
  const [config, setConfig] = useState<ThemeColorConfig>(DEFAULT_THEME_COLOR_CONFIG);
  const [isLoaded, setIsLoaded] = useState(false);

  useEffect(() => {
    async function loadSettings() {
      try {
        const savedPrimaryColor = await store.get<string>("theme-primary-color");
        const savedHoverOpacity = await store.get<number>("theme-hover-opacity");
        const savedActiveOpacity = await store.get<number>("theme-active-opacity");
        const savedBorderOpacity = await store.get<number>("theme-border-opacity");

        if (savedPrimaryColor && isValidHexColor(savedPrimaryColor)) {
          setConfig(prev => ({ ...prev, primaryColor: normalizeHexColor(savedPrimaryColor) }));
        }
        if (savedHoverOpacity !== null && savedHoverOpacity !== undefined) {
          setConfig(prev => ({ ...prev, hoverOpacity: savedHoverOpacity }));
        }
        if (savedActiveOpacity !== null && savedActiveOpacity !== undefined) {
          setConfig(prev => ({ ...prev, activeOpacity: savedActiveOpacity }));
        }
        if (savedBorderOpacity !== null && savedBorderOpacity !== undefined) {
          setConfig(prev => ({ ...prev, borderOpacity: savedBorderOpacity }));
        }

        setIsLoaded(true);
      } catch (error) {
        console.error("Failed to load theme color settings:", error);
        setIsLoaded(true);
      }
    }

    loadSettings();
  }, []);

  useEffect(() => {
    if (!isLoaded) return;

    async function saveSettings() {
      try {
        await store.set("theme-primary-color", config.primaryColor);
        await store.set("theme-hover-opacity", config.hoverOpacity);
        await store.set("theme-active-opacity", config.activeOpacity);
        await store.set("theme-border-opacity", config.borderOpacity);
        await store.save();
      } catch (error) {
        console.error("Failed to save theme color settings:", error);
      }
    }

    saveSettings();
  }, [config, isLoaded]);

  const setPrimaryColor = (color: string) => {
    if (isValidHexColor(color)) {
      setConfig(prev => ({ ...prev, primaryColor: normalizeHexColor(color) }));
    }
  };

  const setHoverOpacity = (opacity: number) => {
    setConfig(prev => ({ ...prev, hoverOpacity: Math.max(0, Math.min(1, opacity)) }));
  };

  const setActiveOpacity = (opacity: number) => {
    setConfig(prev => ({ ...prev, activeOpacity: Math.max(0, Math.min(1, opacity)) }));
  };

  const setBorderOpacity = (opacity: number) => {
    setConfig(prev => ({ ...prev, borderOpacity: Math.max(0, Math.min(1, opacity)) }));
  };

  const resetToDefault = () => {
    setConfig(DEFAULT_THEME_COLOR_CONFIG);
  };

  const getHoverColor = (isDark: boolean = true) => {
    const opacity = isDark ? config.hoverOpacity + 0.1 : config.hoverOpacity;
    return hexToRgba(config.primaryColor, opacity);
  };

  const getActiveColor = () => {
    return config.primaryColor;
  };

  const getBorderColor = () => {
    return hexToRgba(config.primaryColor, config.borderOpacity);
  };

  const getContrastTextColor = () => {
    return getContrastColor(config.primaryColor);
  };

  return (
    <ThemeColorContext.Provider
      value={{
        config,
        setPrimaryColor,
        setHoverOpacity,
        setActiveOpacity,
        setBorderOpacity,
        resetToDefault,
        getHoverColor,
        getActiveColor,
        getBorderColor,
        getContrastTextColor,
      }}
    >
      {children}
    </ThemeColorContext.Provider>
  );
}
