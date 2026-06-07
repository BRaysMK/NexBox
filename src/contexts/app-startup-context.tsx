"use client";

import { createContext, useContext, useState, ReactNode, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LazyStore } from "@tauri-apps/plugin-store";
import { type HardwareInfo, getHardwareInfo } from "@/lib/hardware";

const SETTINGS_FILE = "settings.json";
const store = new LazyStore(SETTINGS_FILE);

const DEFAULT_OVERLAY_HOTKEY = "Shift+F10";
const DEFAULT_CROSSHAIR_HOTKEY = "Shift+F9";
const DEFAULT_FILTER_HOTKEY = "Shift+F8";

interface DisplayItem {
  id: string;
  label: string;
  enabled: boolean;
}

type DisplayItems = DisplayItem[];

interface CustomOverlayItem {
  id: string;
  text: string;
  color: string;
  enabled: boolean;
}

interface OverlaySettings {
  display_items: DisplayItems;
  custom_items: CustomOverlayItem[];
  opacity: number;
  style: string;
  font: string;
  position_x?: number | null;
  position_y?: number | null;
}

interface ThirdPartyTool {
  id: string;
  name: string;
  description: string;
  category: string;
  tool_type: string;
  download_url: string;
  file_name: string;
  website_url: string | null;
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
  tools: ThirdPartyTool[];
  initTools: () => Promise<void>;
  overlaySettings: OverlaySettings | null;
  saveOverlaySettings: (settings: OverlaySettings) => Promise<void>;
  overlayHotkey: string;
  saveOverlayHotkey: (shortcut: string) => Promise<void>;
  crosshairHotkey: string;
  saveCrosshairHotkey: (shortcut: string) => Promise<void>;
  filterHotkey: string;
  saveFilterHotkey: (shortcut: string) => Promise<void>;
}

const DEFAULT_OVERLAY_SETTINGS: OverlaySettings = {
  display_items: [
    { id: "fps", label: "FPS", enabled: true },
    { id: "cpu_usage", label: "CPU占用", enabled: true },
    { id: "gpu_temp", label: "GPU温度", enabled: true },
    { id: "gpu_usage", label: "GPU占用", enabled: true },
    { id: "memory_usage", label: "内存占用", enabled: true },
    { id: "game_ping", label: "游戏延迟", enabled: true },
    { id: "delta_password", label: "三角洲密码", enabled: true },
  ],
  custom_items: [],
  opacity: 255,
  style: "default",
  font: "MiSans Medium",
  position_x: null,
  position_y: null,
};

const AppStartupContext = createContext<AppStartupContextType>({
  isStartupComplete: false,
  startupProgress: 0,
  startupMessage: "正在启动...",
  hardwareInfo: null,
  tools: [],
  initTools: async () => {},
  overlaySettings: null,
  saveOverlaySettings: async () => {},
  overlayHotkey: DEFAULT_OVERLAY_HOTKEY,
  saveOverlayHotkey: async () => {},
  crosshairHotkey: DEFAULT_CROSSHAIR_HOTKEY,
  saveCrosshairHotkey: async () => {},
  filterHotkey: DEFAULT_FILTER_HOTKEY,
  saveFilterHotkey: async () => {},
});

export function useAppStartup() {
  return useContext(AppStartupContext);
}

export function AppStartupProvider({ children }: { children: ReactNode }) {
  const [isStartupComplete, setIsStartupComplete] = useState(false);
  const [startupProgress, setStartupProgress] = useState(0);
  const [startupMessage, setStartupMessage] = useState("正在启动...");
  const [hardwareInfo, setHardwareInfo] = useState<HardwareInfo | null>(null);
  const [tools, setTools] = useState<ThirdPartyTool[]>([]);
  const [overlaySettings, setOverlaySettings] = useState<OverlaySettings | null>(null);
  const [overlayHotkey, setOverlayHotkey] = useState(DEFAULT_OVERLAY_HOTKEY);
  const [crosshairHotkey, setCrosshairHotkey] = useState(DEFAULT_CROSSHAIR_HOTKEY);
  const [filterHotkey, setFilterHotkey] = useState(DEFAULT_FILTER_HOTKEY);
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

  const initTools = async () => {
    try {
      const toolsData = await invoke<ThirdPartyTool[]>("get_thirdparty_tools");
      setTools(toolsData);
    } catch (error) {
      console.error("Failed to load tools:", error);
    }
  };

  const loadOverlaySettings = async () => {
    try {
      const savedSettings = await store.get<OverlaySettings>("overlay-settings");
      let settingsToUse: OverlaySettings;
      let needsMigration = false;
      if (savedSettings) {
        // 处理旧格式（对象）到新格式（数组）的迁移
        let displayItems: DisplayItems;
        if (Array.isArray(savedSettings.display_items)) {
          // 新格式：数组
          displayItems = savedSettings.display_items;
        } else {
          // 旧格式：对象，需要迁移
          needsMigration = true;
          const oldItems = savedSettings.display_items as unknown as {
            fps: boolean;
            cpu_usage: boolean;
            gpu_temp: boolean;
            gpu_usage: boolean;
            memory_usage: boolean;
            delta_password: boolean;
            game_ping: boolean;
          };
          displayItems = [
            { id: "fps", label: "FPS", enabled: oldItems.fps ?? true },
            { id: "cpu_usage", label: "CPU占用", enabled: oldItems.cpu_usage ?? true },
            { id: "gpu_temp", label: "GPU温度", enabled: oldItems.gpu_temp ?? true },
            { id: "gpu_usage", label: "GPU占用", enabled: oldItems.gpu_usage ?? true },
            { id: "memory_usage", label: "内存占用", enabled: oldItems.memory_usage ?? true },
            { id: "game_ping", label: "游戏延迟", enabled: oldItems.game_ping ?? true },
            { id: "delta_password", label: "三角洲密码", enabled: oldItems.delta_password ?? true },
          ];
        }
        settingsToUse = {
          ...DEFAULT_OVERLAY_SETTINGS,
          ...savedSettings,
          display_items: displayItems,
        };
        // 如果是旧格式，迁移后保存新格式到存储
        if (needsMigration) {
          await store.set("overlay-settings", settingsToUse);
          await store.save();
        }
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

  const loadOverlayHotkey = async () => {
    try {
      const saved = await store.get<string>("overlay-hotkey");
      if (saved) {
        setOverlayHotkey(saved);
        await invoke("set_overlay_hotkey", { shortcut: saved });
      } else {
        await invoke("set_overlay_hotkey", { shortcut: DEFAULT_OVERLAY_HOTKEY });
      }
    } catch (error) {
      console.error("Failed to load overlay hotkey:", error);
    }
  };

  const saveOverlayHotkey = async (shortcut: string) => {
    setOverlayHotkey(shortcut);
    try {
      await invoke("set_overlay_hotkey", { shortcut });
      await store.set("overlay-hotkey", shortcut);
      await store.save();
    } catch (error) {
      console.error("Failed to save overlay hotkey:", error);
    }
  };

  const loadCrosshairHotkey = async () => {
    try {
      const saved = await store.get<string>("crosshair-hotkey");
      if (saved) {
        setCrosshairHotkey(saved);
        await invoke("set_crosshair_hotkey", { shortcut: saved });
      } else {
        await invoke("set_crosshair_hotkey", { shortcut: DEFAULT_CROSSHAIR_HOTKEY });
      }
    } catch (error) {
      console.error("Failed to load crosshair hotkey:", error);
    }
  };

  const saveCrosshairHotkey = async (shortcut: string) => {
    setCrosshairHotkey(shortcut);
    try {
      await invoke("set_crosshair_hotkey", { shortcut });
      await store.set("crosshair-hotkey", shortcut);
      await store.save();
    } catch (error) {
      console.error("Failed to save crosshair hotkey:", error);
    }
  };

  const loadFilterHotkey = async () => {
    try {
      const saved = await store.get<string>("filter-hotkey");
      if (saved) {
        setFilterHotkey(saved);
        await invoke("set_filter_hotkey", { shortcut: saved });
      } else {
        await invoke("set_filter_hotkey", { shortcut: DEFAULT_FILTER_HOTKEY });
      }
    } catch (error) {
      console.error("Failed to load filter hotkey:", error);
    }
  };

  const saveFilterHotkey = async (shortcut: string) => {
    setFilterHotkey(shortcut);
    try {
      await invoke("set_filter_hotkey", { shortcut });
      await store.set("filter-hotkey", shortcut);
      await store.save();
    } catch (error) {
      console.error("Failed to save filter hotkey:", error);
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
      const tasks = [
        { name: "overlay-settings", fn: loadOverlaySettings, weight: 1 },
        { name: "hardware-info", fn: loadHardwareInfo, weight: 4 },
        { name: "overlay-hotkey", fn: loadOverlayHotkey, weight: 1 },
        { name: "crosshair-hotkey", fn: loadCrosshairHotkey, weight: 1 },
        { name: "filter-hotkey", fn: loadFilterHotkey, weight: 1 },
      ];

      const totalWeight = tasks.reduce((sum, t) => sum + t.weight, 0);
      let completedWeight = 0;

      setStartupProgress(5);

      const updateProgress = () => {
        const baseProgress = 5;
        const maxProgress = 95;
        const progress = baseProgress + (completedWeight / totalWeight) * (maxProgress - baseProgress);
        setStartupProgress(Math.min(progress, 95));
      };

      await Promise.all(
        tasks.map(async (task) => {
          try {
            await task.fn();
          } catch (error) {
            console.error(`Failed to load ${task.name}:`, error);
          } finally {
            completedWeight += task.weight;
            updateProgress();
          }
        })
      );

      setStartupProgress(100);
      setTimeout(() => {
        setIsStartupComplete(true);
      }, 100);
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
        initTools,
        overlaySettings,
        saveOverlaySettings,
        overlayHotkey,
        saveOverlayHotkey,
        crosshairHotkey,
        saveCrosshairHotkey,
        filterHotkey,
        saveFilterHotkey,
      }}
    >
      {children}
    </AppStartupContext.Provider>
  );
}