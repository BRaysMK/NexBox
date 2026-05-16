"use client";

import { createContext, useContext, useState, ReactNode, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LazyStore } from "@tauri-apps/plugin-store";
import { type HardwareInfo, getHardwareInfo } from "@/lib/hardware";

const SETTINGS_FILE = "settings.json";
const store = new LazyStore(SETTINGS_FILE);

interface DisplayItems {
  fps: boolean;
  cpu_usage: boolean;
  gpu_temp: boolean;
  gpu_usage: boolean;
  memory_usage: boolean;
  delta_password: boolean;
}

interface OverlaySettings {
  display_items: DisplayItems;
  opacity: number;
}

interface ThirdPartyTool {
  id: string;
  name: string;
  description: string;
  category: string;
  tool_type: string;
  download_url: string;
  file_name: string;
  check_executable: string | null;
}

interface ToolWithStatus {
  tool: ThirdPartyTool;
  installed: boolean;
}

interface AppStartupContextType {
  isStartupComplete: boolean;
  startupProgress: number;
  startupMessage: string;
  hardwareInfo: HardwareInfo | null;
  tools: ToolWithStatus[];
  refreshTools: () => Promise<void>;
  overlaySettings: OverlaySettings | null;
  saveOverlaySettings: (settings: OverlaySettings) => Promise<void>;
}

const DEFAULT_OVERLAY_SETTINGS: OverlaySettings = {
  display_items: {
    fps: false,
    cpu_usage: true,
    gpu_temp: true,
    gpu_usage: true,
    memory_usage: true,
    delta_password: false,
  },
  opacity: 200,
};

const AppStartupContext = createContext<AppStartupContextType>({
  isStartupComplete: false,
  startupProgress: 0,
  startupMessage: "正在启动...",
  hardwareInfo: null,
  tools: [],
  refreshTools: async () => {},
  overlaySettings: null,
  saveOverlaySettings: async () => {},
});

export function useAppStartup() {
  return useContext(AppStartupContext);
}

export function AppStartupProvider({ children }: { children: ReactNode }) {
  const [isStartupComplete, setIsStartupComplete] = useState(false);
  const [startupProgress, setStartupProgress] = useState(0);
  const [startupMessage, setStartupMessage] = useState("正在启动...");
  const [hardwareInfo, setHardwareInfo] = useState<HardwareInfo | null>(null);
  const [tools, setTools] = useState<ToolWithStatus[]>([]);
  const [overlaySettings, setOverlaySettings] = useState<OverlaySettings | null>(null);
  const hasStarted = useRef(false);

  const updateProgress = (progress: number, message: string) => {
    setStartupProgress(progress);
    setStartupMessage(message);
  };

  const loadHardwareInfo = async () => {
    try {
      const info = await getHardwareInfo();
      setHardwareInfo(info);
      return true;
    } catch (error) {
      console.error("Failed to load hardware info:", error);
      return false;
    }
  };

  const refreshTools = async () => {
    try {
      const toolsData = await invoke<ToolWithStatus[]>("get_thirdparty_tools_with_status");
      setTools(toolsData);
    } catch (error) {
      console.error("Failed to load tools:", error);
    }
  };

  const loadTools = async () => {
    await refreshTools();
    return true;
  };

  const loadOverlaySettings = async () => {
    try {
      const savedSettings = await store.get<OverlaySettings>("overlay-settings");
      let settingsToUse: OverlaySettings;
      if (savedSettings) {
        settingsToUse = {
          ...DEFAULT_OVERLAY_SETTINGS,
          ...savedSettings,
          display_items: {
            ...DEFAULT_OVERLAY_SETTINGS.display_items,
            ...savedSettings.display_items,
          },
        };
      } else {
        settingsToUse = DEFAULT_OVERLAY_SETTINGS;
      }
      setOverlaySettings(settingsToUse);
      
      await invoke("update_overlay_settings", { settings: settingsToUse });
    } catch (error) {
      console.error("Failed to load overlay settings:", error);
      setOverlaySettings(DEFAULT_OVERLAY_SETTINGS);
      try {
        await invoke("update_overlay_settings", { settings: DEFAULT_OVERLAY_SETTINGS });
      } catch (e) {
        console.error("Failed to initialize backend settings:", e);
      }
    }
  };

  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingSettingsRef = useRef<OverlaySettings | null>(null);

  const saveOverlaySettings = async (settings: OverlaySettings) => {
    setOverlaySettings(settings);
    pendingSettingsRef.current = settings;
    if (saveTimerRef.current) {
      clearTimeout(saveTimerRef.current);
    }
    saveTimerRef.current = setTimeout(async () => {
      saveTimerRef.current = null;
      const s = pendingSettingsRef.current;
      if (s) {
        invoke("update_overlay_settings", { settings: s });
        try {
          await store.set("overlay-settings", s);
          await store.save();
        } catch (error) {
          console.error("Failed to save overlay settings:", error);
        }
      }
    }, 100);
  };

  useEffect(() => {
    if (hasStarted.current) return;
    hasStarted.current = true;

    const runStartup = async () => {
      updateProgress(10, "正在初始化...");

      // 并行加载所有数据，加快启动速度
      const [, ,] = await Promise.all([
        loadOverlaySettings(),
        (async () => {
          updateProgress(30, "正在加载硬件信息...");
          return loadHardwareInfo();
        })(),
        (async () => {
          updateProgress(50, "正在搜索工具...");
          return loadTools();
        })(),
      ]);

      updateProgress(100, "启动完成！");
      setIsStartupComplete(true);
    };

    runStartup();

    const unlisten = listen("tauri://close-requested", () => {});

    return () => {
      unlisten.then((fn) => fn());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <AppStartupContext.Provider
      value={{
        isStartupComplete,
        startupProgress,
        startupMessage,
        hardwareInfo,
        tools,
        refreshTools,
        overlaySettings,
        saveOverlaySettings,
      }}
    >
      {children}
    </AppStartupContext.Provider>
  );
}