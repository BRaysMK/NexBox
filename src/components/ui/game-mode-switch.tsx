import { Box, Flex, Text, useColorModeValue } from "@chakra-ui/react";
import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useThemeColor } from "@/contexts/theme-color-context";
import { useBackground } from "@/contexts/background-context";
import { useDynamicIsland, type IslandStatus } from "@/components/ui/dynamic-island";

type Preset = "default" | "regular" | "competitive";

const OPTIONS: { value: Preset; label: string }[] = [
  { value: "default", label: "默认" },
  { value: "regular", label: "常规" },
  { value: "competitive", label: "竞技" },
];

// 顶部切换模式时的灵动岛提示文案与状态色
// 常规：蓝色；竞技：主题色（info）；默认：主题色（info）
const MODE_TOAST: Record<Preset, { title: string; status: IslandStatus; description?: string }> = {
  default: { title: "已切换为默认", status: "info" },
  regular: { title: "已切换为常规", status: "blue", description: "压制少部分进程" },
  competitive: { title: "已切换为竞技", status: "info", description: "压制大部分进程" },
};

interface Status {
  preset: Preset;
  effective_preset: Preset;
  manual_enabled: boolean;
  auto_enabled: boolean;
}

// 一次性注入胶囊效果所需的关键帧样式
const FX_STYLE = `
.gm-slider {
  position: absolute;
  top: 3px;
  bottom: 3px;
  border-radius: 999px;
  transition: left .22s cubic-bezier(.4, 0, .2, 1), width .22s cubic-bezier(.4, 0, .2, 1);
  z-index: 0;
}
.gm-regular-track {
  border-color: #3b82f6 !important;
  box-shadow: 0 0 8px rgba(64,156,255,.55), 0 0 20px rgba(64,156,255,.28);
}
.gm-regular-slider {
  overflow: hidden;
  background: linear-gradient(135deg, #3b82f6, #22d3ee);
  box-shadow: 0 0 8px rgba(64,156,255,.6);
}
.gm-regular-slider::after {
  content: "";
  position: absolute;
  top: 0;
  left: -150%;
  width: 55%;
  height: 100%;
  border-radius: 999px;
  background: linear-gradient(90deg, transparent, rgba(255,255,255,.65), transparent);
  animation: gm-sweep 1.5s linear infinite;
}
@keyframes gm-sweep { 0% { left: -150%; } 100% { left: 150%; } }

.gm-comp-track {
  border-color: #ff5f6d !important;
  box-shadow: 0 0 8px rgba(255,95,109,.55), 0 0 18px rgba(255,95,109,.28);
}
.gm-comp-slider {
  overflow: hidden;
  background: linear-gradient(135deg, #ff5f6d, #ff8f6d);
  box-shadow: 0 0 8px rgba(255,95,109,.6);
}
.gm-comp-slider::after {
  content: "";
  position: absolute;
  top: 0;
  left: -150%;
  width: 55%;
  height: 100%;
  border-radius: 999px;
  background: linear-gradient(90deg, transparent, rgba(255,255,255,.6), transparent);
  animation: gm-sweep 1.5s linear infinite;
}`;

/** 顶栏右上角游戏模式切换条（横着的圆角矩形，默认/常规/竞技） */
export function GameModeSwitch() {
  const { getActiveColor } = useThemeColor();
  const { liquidGlassEnabled, liquidGlassBlur } = useBackground();
  const toast = useDynamicIsland("gamepad");
  const textColor = useColorModeValue("gray.600", "gray.300");
  const activeTextColor = useColorModeValue("white", "white");
  // 玻璃开启时使用半透明玻璃底色 + 柔和边框；关闭时用普通底色
  const glassTrackBg = useColorModeValue("rgba(255,255,255,0.25)", "rgba(0,0,0,0.25)");
  const plainTrackBg = useColorModeValue("whiteAlpha.700", "blackAlpha.500");
  const glassBorderColor = useColorModeValue("rgba(255,255,255,0.2)", "rgba(255,255,255,0.1)");
  const plainBorderColor = useColorModeValue("gray.200", "#333333");
  const trackBg = liquidGlassEnabled ? glassTrackBg : plainTrackBg;
  const borderColor = liquidGlassEnabled ? glassBorderColor : plainBorderColor;
  const effectiveBlur = liquidGlassEnabled ? liquidGlassBlur : 0;
  const backdropFilter = liquidGlassEnabled
    ? `blur(${effectiveBlur}px) saturate(1.3)`
    : "blur(10px)";

  const [preset, setPreset] = useState<Preset>("default");
  const [slider, setSlider] = useState({ left: 0, width: 0 });
  const btnRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const trackRef = useRef<HTMLDivElement | null>(null);
  const presetRef = useRef<Preset>("default");
  presetRef.current = preset;

  // 精确测量滑块位置：相对容器 padding box 定位，消除 border/offsetParent 歧义
  const measureSlider = useCallback(() => {
    const track = trackRef.current;
    const idx = OPTIONS.findIndex((o) => o.value === presetRef.current);
    const el = btnRefs.current[idx];
    if (!track || !el) return;
    const trackRect = track.getBoundingClientRect();
    const elRect = el.getBoundingClientRect();
    // 滑块 absolute 定位相对容器的 padding box：减去 border 宽度
    const borderLeft = track.clientLeft;
    setSlider({
      left: elRect.left - trackRect.left - borderLeft,
      width: elRect.width,
    });
  }, []);

  // 选中项变化 → 等布局稳定后测量并滑动（双重 rAF 确保 DOM 已更新）
  useEffect(() => {
    let raf2 = 0;
    const raf1 = requestAnimationFrame(() => {
      raf2 = requestAnimationFrame(measureSlider);
    });
    return () => {
      cancelAnimationFrame(raf1);
      cancelAnimationFrame(raf2);
    };
  }, [preset, measureSlider]);

  // 容器尺寸变化（resize / 顶栏布局）时重测，避免滑块偏移
  useEffect(() => {
    const track = trackRef.current;
    if (track && typeof ResizeObserver !== "undefined") {
      const ro = new ResizeObserver(() => measureSlider());
      ro.observe(track);
      return () => ro.disconnect();
    }
  }, [measureSlider]);

  // 注入一次动画样式
  useEffect(() => {
    if (document.getElementById("gm-fx-style")) return;
    const style = document.createElement("style");
    style.id = "gm-fx-style";
    style.textContent = FX_STYLE;
    document.head.appendChild(style);
  }, []);

  useEffect(() => {
    // 初始加载生效档位（游戏运行时可能已被后端强制为竞技）
    invoke<Status>("game_mode_get_status")
      .then((s) => setPreset(s.effective_preset || "default"))
      .catch((e) => console.error("加载游戏模式状态失败:", e));
    // 后端生效档位变化（游戏启动/退出）→ 实时同步顶栏
    let unlisten: UnlistenFn | undefined;
    listen<Preset>("game-mode-effective-changed", (event) => {
      setPreset(event.payload || "default");
    }).then((fn) => {
      unlisten = fn;
    });
    // 页面内切换时同步
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail;
      if (detail && detail.preset) setPreset(detail.preset as Preset);
    };
    window.addEventListener("game-mode-preset-changed", handler as EventListener);
    return () => {
      if (unlisten) unlisten();
      window.removeEventListener("game-mode-preset-changed", handler as EventListener);
    };
  }, []);

  const handleSelect = useCallback(
    async (value: Preset) => {
      if (value === preset) return;
      setPreset(value);
      try {
        await invoke("game_mode_set_preset", { preset: value });
        window.dispatchEvent(
          new CustomEvent("game-mode-preset-changed", { detail: { preset: value } })
        );
        // 顶栏手动切换 → 灵动岛提示
        toast({
          title: MODE_TOAST[value].title,
          status: MODE_TOAST[value].status,
          description: MODE_TOAST[value].description,
          iconKey: "gamepad",
        });
      } catch (e) {
        console.error("切换游戏模式失败:", e);
        setPreset((p) => p);
      }
    },
    [preset, toast]
  );

  const isRegular = preset === "regular";
  const isCompetitive = preset === "competitive";

  // 滑块背景：默认主题色 / 常规蓝青渐变 / 竞技红渐变
  const sliderClass = isCompetitive
    ? "gm-slider gm-comp-slider"
    : isRegular
      ? "gm-slider gm-regular-slider"
      : "gm-slider";

  return (
    <Flex
      ref={trackRef}
      position="relative"
      align="center"
      p="3px"
      gap="2px"
      borderRadius="full"
      bg={trackBg}
      border="1px solid"
      borderColor={borderColor}
      backdropFilter={backdropFilter}
      onMouseDown={(e) => e.stopPropagation()}
      className={
        isRegular ? "gm-regular-track" : isCompetitive ? "gm-comp-track" : undefined
      }
    >
      {slider.width > 0 && (
        <Box
          className={sliderClass}
          style={{ left: slider.left, width: slider.width }}
          bg={!isRegular && !isCompetitive ? getActiveColor() : undefined}
          aria-hidden
        />
      )}
      {OPTIONS.map((opt, i) => {
        const active = preset === opt.value;
        return (
          <Box
            key={opt.value}
            as="button"
            ref={(el) => {
              btnRefs.current[i] = el;
            }}
            px={3}
            py="4px"
            borderRadius="full"
            bg="transparent"
            color={active ? activeTextColor : textColor}
            fontSize="xs"
            fontWeight="medium"
            cursor="pointer"
            transition="color 0.15s"
            zIndex={1}
            _hover={!active ? { bg: useColorModeValue("gray.100", "gray.700") } : undefined}
            onClick={(e) => {
              e.stopPropagation();
              handleSelect(opt.value);
            }}
          >
            <Text fontSize="xs" lineHeight="1.2" fontWeight="medium">
              {opt.label}
            </Text>
          </Box>
        );
      })}
    </Flex>
  );
}