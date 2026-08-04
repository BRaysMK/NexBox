"use client";

import { createContext, useContext, useState, ReactNode, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { store } from "@/lib/store";
import { type HardwareInfo, getHardwareInfo } from "@/lib/hardware";

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
  offset_x: number;
  offset_y: number;
}

interface OverlaySettings {
  display_items: DisplayItems;
  custom_items: CustomOverlayItem[];
  opacity: number;
  style: string;
  font: string;
  font_size: number;
  item_width: number;
  font_color: string;
  _version?: number;
  position_x?: number | null;
  position_y?: number | null;
  delta_password_maps?: string[];
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
  hotkeysEnabled: boolean;
  saveHotkeysEnabled: (enabled: boolean) => Promise<void>;
}

const DEFAULT_OVERLAY_SETTINGS: OverlaySettings = {
  display_items: [
    { id: "fps", label: "FPS", enabled: true },
    { id: "fps_1low", label: "1% Low", enabled: false },
    { id: "fps_01low", label: "0.1% Low", enabled: false },
    { id: "cpu_temp", label: "CPU温度", enabled: false },
    { id: "cpu_usage", label: "CPU占用", enabled: true },
    { id: "cpu_fan_speed", label: "CPU风扇转速", enabled: false },
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
    { id: "delta_password", label: "三角洲密码", enabled: false },
  ],
  custom_items: [],
  opacity: 255,
  style: "default",
  font: "Microsoft YaHei",
  font_size: 13,
  item_width: 130,
  font_color: "#ffffff",
  position_x: null,
  position_y: null,
  delta_password_maps: [],
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
  hotkeysEnabled: true,
  saveHotkeysEnabled: async () => {},
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
  const [hotkeysEnabled, setHotkeysEnabled] = useState(true);
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
          const currentVersion = 4;
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
            { id: "fps_1low", label: "1% Low", enabled: false },
            { id: "fps_01low", label: "0.1% Low", enabled: false },
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
            _version: 4,
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
      
      // 仅在存在已保存设置时同步到后端，避免在 LazyStore 未就绪时用默认值覆盖后端已正确加载的设置
      if (savedSettings) {
        await invoke("update_overlay_settings", { settings: settingsToUse });
      }
    } catch (error) {
      console.error("Failed to load overlay settings:", error);
      // 加载失败时仅设置前端 UI 默认值，不覆盖后端已有的设置
      setOverlaySettings(DEFAULT_OVERLAY_SETTINGS);
    }
  };

  const loadOverlayHotkey = async () => {
    try {
      const saved = await store.get<string>("overlay-hotkey");
      if (saved) {
        setOverlayHotkey(saved);
        await invoke("set_overlay_hotkey", { shortcut: saved });
      }
      // 没有保存值则无需调用，Rust 端已用默认值初始化
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
      }
      // 没有保存值则无需调用，Rust 端已用默认值初始化
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
        let autoApply = await store.get<boolean>("nexbox_auto_crosshair");
        if (autoApply === null || autoApply === undefined) {
          autoApply = localStorage.getItem("nexbox_auto_crosshair") === "true";
        }
        saved.enabled = false;
        await invoke("update_crosshair_settings", { settings: saved });
        if (autoApply) {
          await invoke("toggle_crosshair");
        }
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
      }
      // 没有保存值则无需调用，Rust 端已用默认值初始化
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

  const loadHotkeysEnabled = async () => {
    try {
      const saved = await store.get<boolean>("hotkeys-enabled");
      if (saved !== undefined && saved !== null) {
        setHotkeysEnabled(saved);
        await invoke("set_hotkeys_enabled_cmd", { enabled: saved });
      }
      // 没有保存值则保持默认开启，Rust 端默认也是开启
    } catch (error) {
      console.error("Failed to load hotkeys enabled:", error);
    }
  };

  const saveHotkeysEnabled = async (enabled: boolean) => {
    setHotkeysEnabled(enabled);
    try {
      await invoke("set_hotkeys_enabled_cmd", { enabled });
      await store.set("hotkeys-enabled", enabled);
      await store.save();
    } catch (error) {
      console.error("Failed to save hotkeys enabled:", error);
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
        try {
          await invoke("update_overlay_settings", { settings: s });
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
        { name: "hotkeys-enabled", fn: loadHotkeysEnabled, weight: 1 },
        {
          name: "filter-restore",
          fn: async () => {
            try {
              let autoApply = await store.get<boolean>("nexbox_auto_apply");
              if (autoApply === null || autoApply === undefined) {
                autoApply = localStorage.getItem("nexbox_auto_apply") === "true";
              }
              await invoke("restore_filter_state", { displayIndex: null, autoApply });
            } catch (e) {
              console.error("Failed to restore filter state:", e);
            }
          },
          weight: 1,
        },
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

    return () => {};
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
        hotkeysEnabled,
        saveHotkeysEnabled,
      }}
    >
      {children}
    </AppStartupContext.Provider>
  );
}
