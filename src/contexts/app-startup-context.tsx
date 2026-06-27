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
const DEFAULT_ISLAND_HOTKEY = "Shift+F11";

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

interface CrosshairSettings {
  enabled: boolean;
  style: string;
  size: number;
  thickness: number;
  color: string;
  gap: number;
  dot_size: number;
  opacity: number;
  monitor_index: number;
}

interface OverlaySettings {
  display_items: DisplayItems;
  custom_items: CustomOverlayItem[];
  opacity: number;
  style: string;
  font: string;
  _version?: number;
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
  islandHotkey: string;
  saveIslandHotkey: (shortcut: string) => Promise<void>;
}

const DEFAULT_OVERLAY_SETTINGS: OverlaySettings = {
  display_items: [
    { id: "fps", label: "FPS", enabled: true },
    { id: "cpu_temp", label: "CPU温度", enabled: false },
    { id: "cpu_usage", label: "CPU占用", enabled: true },
    { id: "cpu_clock", label: "CPU频率", enabled: false },
    { id: "cpu_voltage", label: "CPU电压", enabled: false },
    { id: "cpu_power", label: "CPU功耗", enabled: false },
    { id: "gpu_temp", label: "GPU温度", enabled: true },
    { id: "gpu_usage", label: "GPU占用", enabled: true },
    { id: "gpu_fan_speed", label: "GPU风扇转速", enabled: false },
    { id: "gpu_power", label: "GPU功耗", enabled: false },
    { id: "gpu_clock", label: "GPU频率", enabled: false },
    { id: "gpu_voltage", label: "GPU电压", enabled: false },
    { id: "gpu_vram", label: "GPU显存占用", enabled: false },
    { id: "gpu_memory_clock", label: "GPU显存频率", enabled: false },
    { id: "memory_usage", label: "内存占用", enabled: true },
    { id: "ssd_temp", label: "硬盘温度", enabled: false },
    { id: "game_ping", label: "游戏延迟", enabled: true },
    { id: "delta_password", label: "三角洲密码", enabled: true },
    // { id: "netease_lyric", label: "网易云歌词", enabled: false },
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
  islandHotkey: DEFAULT_ISLAND_HOTKEY,
  saveIslandHotkey: async () => {},
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
  const [islandHotkey, setIslandHotkey] = useState(DEFAULT_ISLAND_HOTKEY);
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
          // 新格式数组：检查版本，过旧则重置顺序和标签，保留启用状态
          const currentVersion = 3;
          const savedVersion = savedSettings._version ?? 1;
          if (savedVersion < currentVersion) {
            // 版本过旧：用默认项重建，只保留启用状态
            const savedMap = new Map(savedSettings.display_items.map((i) => [i.id, i.enabled]));
            displayItems = DEFAULT_OVERLAY_SETTINGS.display_items.map((d) => ({
              ...d,
              enabled: savedMap.has(d.id) ? savedMap.get(d.id)! : d.enabled,
            }));
            needsMigration = true;
          } else {
            // 最新版本，补充可能缺失的项，移除已废弃的项
            const defaultItems = DEFAULT_OVERLAY_SETTINGS.display_items;
            const defaultIds = new Set(defaultItems.map((i) => i.id));
            displayItems = [
              ...savedSettings.display_items.filter((i) => defaultIds.has(i.id)),
              ...defaultItems.filter((i) => !savedSettings.display_items.some((s) => s.id === i.id)),
            ];
          }
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
            { id: "gpu_fan_speed", label: "GPU风扇转速", enabled: false },
            { id: "gpu_power", label: "GPU功耗", enabled: false },
            { id: "gpu_clock", label: "GPU频率", enabled: false },
            { id: "gpu_vram", label: "GPU显存占用", enabled: false },
            { id: "memory_usage", label: "内存占用", enabled: oldItems.memory_usage ?? true },
            { id: "game_ping", label: "游戏延迟", enabled: oldItems.game_ping ?? true },
            { id: "delta_password", label: "三角洲密码", enabled: oldItems.delta_password ?? true },
          ];
        }
        if (needsMigration) {
          settingsToUse = {
            ...DEFAULT_OVERLAY_SETTINGS,
            ...savedSettings,
            _version: 3,
            display_items: displayItems,
          };
          await store.set("overlay-settings", settingsToUse);
          await store.save();
        } else {
          settingsToUse = {
            ...DEFAULT_OVERLAY_SETTINGS,
            ...savedSettings,
            display_items: displayItems,
          };
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

  const loadCrosshairSettings = async () => {
    try {
      const saved = await store.get<CrosshairSettings>("crosshair-settings");
      if (saved) {
        saved.enabled = false;
        await invoke("update_crosshair_settings", { settings: saved });
      }
    } catch (error) {
      console.error("Failed to load crosshair settings:", error);
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

  const loadIslandHotkey = async () => {
    try {
      const saved = await store.get<string>("island-hotkey");
      if (saved) {
        setIslandHotkey(saved);
        await invoke("set_island_hotkey", { shortcut: saved });
      } else {
        await invoke("set_island_hotkey", { shortcut: DEFAULT_ISLAND_HOTKEY });
      }
    } catch (error) {
      console.error("Failed to load island hotkey:", error);
    }
  };

  const saveIslandHotkey = async (shortcut: string) => {
    setIslandHotkey(shortcut);
    try {
      await invoke("set_island_hotkey", { shortcut });
      await store.set("island-hotkey", shortcut);
      await store.save();
    } catch (error) {
      console.error("Failed to save island hotkey:", error);
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
        { name: "crosshair-settings", fn: loadCrosshairSettings, weight: 1 },
        { name: "filter-hotkey", fn: loadFilterHotkey, weight: 1 },
        { name: "island-hotkey", fn: loadIslandHotkey, weight: 1 },
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
        islandHotkey,
        saveIslandHotkey,
      }}
    >
      {children}
    </AppStartupContext.Provider>
  );
}
