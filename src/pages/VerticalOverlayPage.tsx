/**
 * 竖排悬浮框独立窗口页面
 *
 * 特性：
 * - 所有硬件项竖向排列，完整显示项名
 * - 每项带 Lucide 图标
 * - 拖动模式 / 鼠标穿透切换
 * - 位置记忆
 * - 不透明度联动
 * - 数值颜色渐变（绿→黄→红）
 */

import { useEffect, useRef, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Gauge,
  Thermometer,
  Cpu,
  Zap,
  HardDrive,
  MemoryStick,
  Wifi,
  Key,
  Music,
  Fan,
  Activity,
  Battery,
  Monitor,
  type LucideIcon,
} from "lucide-react";

// ===== 类型定义 =====

interface DisplayItemConfig {
  id: string;
  label: string;
  enabled: boolean;
}

interface CustomOverlayItem {
  id: string;
  text: string;
  color: string;
  enabled: boolean;
}

interface OverlaySettings {
  display_items: DisplayItemConfig[];
  custom_items: CustomOverlayItem[];
  opacity: number;
  style: string;
  font: string;
  position_x?: number | null;
  position_y?: number | null;
}

interface HardwareData {
  fps: number | null;
  cpu_usage: number | null;
  cpu_temp: number | null;
  cpu_clock: number | null;
  cpu_voltage: number | null;
  cpu_power: number | null;
  cpu_fan_speed: number | null;
  gpu_temp: number | null;
  gpu_usage: number | null;
  gpu_fan_speed: number | null;
  gpu_power: number | null;
  gpu_clock: number | null;
  gpu_voltage: number | null;
  gpu_memory_clock: number | null;
  memory_usage: number | null;
  ssd_temp: number | null;
  delta_password: string | null;
  game_ping: number | null;
  gpu_vram_used: number | null;
  gpu_vram_total: number | null;
  netease_current_lyric?: string | null;
  netease_song_title?: string | null;
  netease_song_artist?: string | null;
}

// ===== 图标映射 =====

const ITEM_ICONS: Record<string, LucideIcon> = {
  fps: Gauge,
  cpu_temp: Thermometer,
  cpu_usage: Cpu,
  cpu_clock: Cpu,
  cpu_voltage: Zap,
  cpu_power: Activity,
  cpu_fan_speed: Fan,
  gpu_temp: Thermometer,
  gpu_usage: Monitor,
  gpu_fan_speed: Fan,
  gpu_power: Zap,
  gpu_clock: Monitor,
  gpu_voltage: Battery,
  gpu_vram: MemoryStick,
  gpu_memory_clock: Monitor,
  memory_usage: MemoryStick,
  ssd_temp: HardDrive,
  game_ping: Wifi,
  delta_password: Key,
  netease_lyric: Music,
};

// ===== 默认值 =====

const DEFAULT_DISPLAY_ITEMS: DisplayItemConfig[] = [
  { id: "fps", label: "FPS", enabled: false },
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
  { id: "game_ping", label: "游戏延迟", enabled: false },
  { id: "delta_password", label: "三角洲密码", enabled: false },
];

const DEFAULT_SETTINGS: OverlaySettings = {
  display_items: DEFAULT_DISPLAY_ITEMS,
  custom_items: [],
  opacity: 200,
  style: "vertical_panel",
  font: "Microsoft YaHei",
};

// ===== 工具函数 =====

/** 根据 item.id 和 hardwareData 获取显示值 */
function getItemValue(item: DisplayItemConfig, data: HardwareData | null): string {
  if (!data) return "--";
  switch (item.id) {
    case "fps":
      return data.fps != null ? `${data.fps}` : "--";
    case "cpu_temp":
      return data.cpu_temp != null ? `${data.cpu_temp.toFixed(0)}°C` : "--";
    case "cpu_usage":
      return data.cpu_usage != null ? `${data.cpu_usage}%` : "--";
    case "cpu_clock":
      return data.cpu_clock != null ? `${data.cpu_clock}MHz` : "--";
    case "cpu_voltage":
      return data.cpu_voltage != null ? `${data.cpu_voltage.toFixed(2)}V` : "--";
    case "cpu_power":
      return data.cpu_power != null ? `${data.cpu_power.toFixed(1)}W` : "--";
    case "cpu_fan_speed":
      return data.cpu_fan_speed != null ? `${data.cpu_fan_speed}RPM` : "--";
    case "gpu_temp":
      return data.gpu_temp != null ? `${data.gpu_temp.toFixed(0)}°C` : "--";
    case "gpu_usage":
      return data.gpu_usage != null ? `${data.gpu_usage}%` : "--";
    case "gpu_fan_speed":
      return data.gpu_fan_speed != null ? `${data.gpu_fan_speed}RPM` : "--";
    case "gpu_power":
      return data.gpu_power != null ? `${data.gpu_power}W` : "--";
    case "gpu_clock":
      return data.gpu_clock != null ? `${data.gpu_clock}MHz` : "--";
    case "gpu_voltage":
      return data.gpu_voltage != null ? `${data.gpu_voltage.toFixed(2)}V` : "--";
    case "gpu_vram":
      if (data.gpu_vram_used != null && data.gpu_vram_total != null) {
        return `${data.gpu_vram_used}/${data.gpu_vram_total}MB`;
      }
      return "--";
    case "gpu_memory_clock":
      return data.gpu_memory_clock != null ? `${data.gpu_memory_clock}MHz` : "--";
    case "memory_usage":
      return data.memory_usage != null ? `${data.memory_usage.toFixed(1)}%` : "--";
    case "ssd_temp":
      return data.ssd_temp != null ? `${data.ssd_temp.toFixed(0)}°C` : "--";
    case "game_ping":
      return data.game_ping != null ? `${data.game_ping}ms` : "--";
    case "delta_password":
      return data.delta_password ?? "--";
    case "netease_lyric":
      return data.netease_current_lyric ?? "--";
    default:
      return "--";
  }
}

/** 获取数值颜色（绿→黄→红） */
function getValueColor(value: string): string {
  if (value === "--" || value === "") return "rgba(255,255,255,0.5)";
  const num = parseFloat(value);
  if (isNaN(num)) return "#ffffff";

  // 温度类
  if (value.includes("°C")) {
    if (num < 60) return "#00ff88";
    if (num < 80) return "#ffcc00";
    return "#ff4444";
  }
  // 占用百分比类
  if (value.includes("%")) {
    if (num < 50) return "#00ff88";
    if (num < 80) return "#ffcc00";
    return "#ff4444";
  }
  // 延迟
  if (value.includes("ms")) {
    if (num < 30) return "#00ff88";
    if (num < 80) return "#ffcc00";
    return "#ff4444";
  }
  // FPS
  if (value.match(/^\d+$/) && !value.includes("MHz") && !value.includes("V") && !value.includes("W") && !value.includes("RPM") && !value.includes("MB")) {
    if (num >= 120) return "#00ff88";
    if (num >= 60) return "#ffcc00";
    return "#ff4444";
  }
  return "#ffffff";
}

// ===== 主组件 =====

export default function VerticalOverlayPage() {
  const [hardwareData, setHardwareData] = useState<HardwareData | null>(null);
  const [settings, setSettings] = useState<OverlaySettings>(DEFAULT_SETTINGS);
  const [isDragMode, setIsDragMode] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const isDragModeRef = useRef(isDragMode);
  isDragModeRef.current = isDragMode;

  const win = getCurrentWindow();

  // 强制背景透明（与桌面歌词窗口相同处理）
  useEffect(() => {
    const html = document.documentElement;
    const body = document.body;
    const root = document.getElementById("root");
    const prevHtmlBg = html.style.background;
    const prevBodyBg = body.style.background;
    const prevRootBg = root?.style.background;

    html.style.background = "transparent";
    body.style.background = "transparent";
    if (root) root.style.background = "transparent";

    return () => {
      html.style.background = prevHtmlBg;
      body.style.background = prevBodyBg;
      if (root) root.style.background = prevRootBg || "";
    };
  }, []);

  // 获取设置
  useEffect(() => {
    (async () => {
      try {
        const stored = await invoke<OverlaySettings>("get_overlay_current_settings");
        setSettings(stored);
      } catch {
        // 使用默认设置
      }
    })();
  }, []);

  // 监听硬件数据
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    (async () => {
      unlisten = await listen<HardwareData>("vertical-overlay-data", (event) => {
        setHardwareData(event.payload);
      });
    })();
    return () => { unlisten?.(); };
  }, []);

  // 监听设置更新
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    (async () => {
      unlisten = await listen<OverlaySettings>("vertical-overlay-settings", (event) => {
        setSettings(event.payload);
      });
    })();
    return () => { unlisten?.(); };
  }, []);

  // 默认开启鼠标穿透
  useEffect(() => {
    invoke("set_vertical_overlay_click_through", { enabled: true }).catch(() => {});
  }, []);

  // 窗口大小自适应
  useEffect(() => {
    const updateSize = () => {
      if (containerRef.current) {
        const height = containerRef.current.scrollHeight;
        invoke("resize_vertical_overlay", { height }).catch(() => {});
      }
    };
    // 延迟以等待 DOM 渲染完成
    const timer = setTimeout(updateSize, 100);
    return () => clearTimeout(timer);
  }, [settings.display_items, settings.custom_items, hardwareData]);

  // 拖动模式切换
  const toggleDragMode = useCallback(async () => {
    const newDragMode = !isDragMode;
    setIsDragMode(newDragMode);

    // 拖动模式：关闭穿透允许交互；退出拖动：恢复穿透
    await invoke("set_vertical_overlay_click_through", { enabled: !newDragMode });

    if (!newDragMode) {
      // 退出拖动模式，保存位置
      try {
        const pos = await win.outerPosition();
        await invoke("save_vertical_overlay_position", { x: pos.x, y: pos.y });
      } catch {
        // ignore
      }
    }
  }, [isDragMode, win]);

  // 拖动窗口
  const handleDragStart = useCallback(async (e: React.MouseEvent) => {
    if (!isDragModeRef.current) return;
    e.preventDefault();
    try {
      await win.startDragging();
    } catch {
      // ignore
    }
  }, [win]);

  // 计算启用的项
  const enabledItems = settings.display_items.filter((i) => i.enabled);
  const enabledCustomItems = settings.custom_items.filter((i) => i.enabled);

  // 计算背景不透明度
  const bgOpacity = settings.opacity / 255;

  return (
    <div
      ref={containerRef}
      style={{
        width: "220px",
        background: `rgba(17, 17, 17, ${bgOpacity})`,
        borderRadius: "12px",
        padding: "10px 12px",
        display: "flex",
        flexDirection: "column",
        gap: "2px",
        fontFamily: settings.font || "Microsoft YaHei",
        fontSize: "13px",
        color: "#ffffff",
        userSelect: "none",
        WebkitUserSelect: "none",
        backdropFilter: "blur(10px)",
        WebkitBackdropFilter: "blur(10px)",
        border: "1px solid rgba(255, 255, 255, 0.08)",
        cursor: isDragMode ? "move" : "default",
      }}
      onMouseDown={handleDragStart}
    >
      {/* 拖动按钮 */}
      <div
        style={{
          position: "absolute",
          top: "4px",
          right: "6px",
          width: "18px",
          height: "18px",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          cursor: "pointer",
          opacity: isDragMode ? 0.9 : 0.35,
          transition: "opacity 0.2s",
          zIndex: 10,
        }}
        onClick={(e) => {
          e.stopPropagation();
          toggleDragMode();
        }}
        title={isDragMode ? "点击固定位置" : "点击拖动"}
      >
        {isDragMode ? (
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#ff9800" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M12 17v5" />
            <path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z" />
          </svg>
        ) : (
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="rgba(255,255,255,0.6)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M5 9l-3 3 3 3" />
            <path d="M9 5l3-3 3 3" />
            <path d="M15 19l-3 3-3-3" />
            <path d="M19 9l3 3-3 3" />
            <path d="M2 12h20" />
            <path d="M12 2v20" />
          </svg>
        )}
      </div>

      {/* 内置项列表 */}
      {enabledItems.map((item) => {
        const value = getItemValue(item, hardwareData);
        const IconComp = ITEM_ICONS[item.id] ?? Gauge;
        const valueColor = getValueColor(value);
        return (
          <div
            key={item.id}
            style={{
              display: "flex",
              alignItems: "center",
              gap: "8px",
              padding: "3px 0",
              borderBottom: "1px solid rgba(255, 255, 255, 0.05)",
            }}
          >
            <IconComp size={14} style={{ flexShrink: 0, color: "rgba(255,255,255,0.5)" }} />
            <span style={{ flex: 1, whiteSpace: "nowrap", color: "rgba(255,255,255,0.7)" }}>
              {item.label}
            </span>
            <span style={{ textAlign: "right", fontWeight: 600, whiteSpace: "nowrap", color: valueColor }}>
              {value}
            </span>
          </div>
        );
      })}

      {/* 自定义项列表 */}
      {enabledCustomItems.map((item) => (
        <div
          key={item.id}
          style={{
            display: "flex",
            alignItems: "center",
            gap: "8px",
            padding: "3px 0",
            borderBottom: "1px solid rgba(255, 255, 255, 0.05)",
          }}
        >
          <span style={{ flexShrink: 0, width: 14, textAlign: "center", color: "rgba(255,255,255,0.5)" }}>•</span>
          <span style={{ flex: 1, whiteSpace: "nowrap", color: "rgba(255,255,255,0.7)" }}>
            {item.text}
          </span>
          <span style={{ textAlign: "right", fontWeight: 600, whiteSpace: "nowrap", color: item.color }}>
            ●
          </span>
        </div>
      ))}

      {/* 空状态 */}
      {enabledItems.length === 0 && enabledCustomItems.length === 0 && (
        <div style={{ textAlign: "center", padding: "12px 0", color: "rgba(255,255,255,0.4)" }}>
          请在设置中启用显示项
        </div>
      )}
    </div>
  );
}
