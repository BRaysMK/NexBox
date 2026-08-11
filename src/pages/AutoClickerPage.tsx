import {
  Box,
  Text,
  Heading,
  VStack,
  HStack,
  SimpleGrid,
  Badge,
  IconButton,
  useColorModeValue,
  useColorMode,
  useToast,
  NumberInput,
  NumberInputField,
  NumberInputStepper,
  NumberIncrementStepper,
  NumberDecrementStepper,
} from "@chakra-ui/react";
import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { store } from "@/lib/store";
import { useTranslation } from "react-i18next";
import { ArrowLeft, MousePointerClick, Zap } from "lucide-react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import { useNavigate } from "react-router-dom";
import { MouseHotkeyRecorder } from "@/components/mouse-hotkey-recorder";

interface AutoClickerStatus {
  running: boolean;
  button: string;
  interval_ms: number;
}

const AUTOCLICKER_STORE_KEY = "autoclicker-settings";
const HOTKEY_STORE_KEY = "autoclicker-hotkey";
const DEFAULT_HOTKEY = "F8";

// 预设：每秒点击次数 → 间隔毫秒
const PRESETS: { cps: number; ms: number }[] = [
  { cps: 10, ms: 100 },
  { cps: 20, ms: 50 },
  { cps: 50, ms: 20 },
  { cps: 100, ms: 10 },
];

// 快捷键预设：F8 / 侧键1 / 侧键2 / 空格
const HOTKEY_PRESETS: { labelKey: string; value: string }[] = [
  { labelKey: "autoclicker.presetF8", value: "F8" },
  { labelKey: "autoclicker.presetSide1", value: "MouseX1" },
  { labelKey: "autoclicker.presetSide2", value: "MouseX2" },
  { labelKey: "autoclicker.presetSpace", value: "Space" },
];

function SettingCard({ title, children }: { title: string; children: React.ReactNode }) {
  const { liquidGlassEnabled } = useBackground();
  const cardBg = useColorModeValue("white", "#111111");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const { colorMode } = useColorMode();
  const headerColor = colorMode === "light" ? "#000000" : "#ffffff";

  if (liquidGlassEnabled) {
    return (
      <LiquidGlassCard p={5} h="full">
        <VStack align="stretch" spacing={4} h="full">
          <Text fontWeight="medium" color={headerColor}>{title}</Text>
          {children}
        </VStack>
      </LiquidGlassCard>
    );
  }

  return (
    <Box bg={cardBg} borderRadius="xl" p={5} border="1px solid" borderColor={borderColor} h="full">
      <VStack align="stretch" spacing={4} h="full">
        <Text fontWeight="medium" color={headerColor}>{title}</Text>
        {children}
      </VStack>
    </Box>
  );
}

export default function AutoClickerPage() {
  const { t } = useTranslation();
  const toast = useToast();
  const navigate = useNavigate();
  const { getActiveColor, getHoverColor, getBorderColor } = useThemeColor();

  const [running, setRunning] = useState(false);
  const [button, setButton] = useState<"left" | "right">("left");
  const [intervalMs, setIntervalMs] = useState(100);
  const [hotkey, setHotkey] = useState(DEFAULT_HOTKEY);

  const isDark = useColorModeValue(false, true);
  const headingColor = useColorModeValue("black", "#ffffff");
  const textColor = useColorModeValue("gray.800", "#ffffff");
  const subTextColor = useColorModeValue("gray.500", "#ffffff");
  const themeColor = getActiveColor();
  const themeContrastText = useColorModeValue("black", "#ffffff");
  const selectedBg = getHoverColor(isDark);
  const selectedBorder = getBorderColor();

  // 加载运行状态、已保存设置与热键
  useEffect(() => {
    (async () => {
      try {
        const status = await invoke<AutoClickerStatus>("autoclicker_get_status");
        setRunning(status.running);
        setButton(status.button === "right" ? "right" : "left");
        setIntervalMs(status.interval_ms);
      } catch { /* 忽略，使用默认值 */ }

      try {
        const saved = await store.get<{ button: string; interval_ms: number }>(AUTOCLICKER_STORE_KEY);
        if (saved) {
          setButton(saved.button === "right" ? "right" : "left");
          setIntervalMs(saved.interval_ms);
        }
      } catch { /* ignore */ }

      try {
        // 热键已由 Rust 端在启动时读取持久化配置注册，这里只同步 UI 显示值
        const savedHotkey = await invoke<string>("get_autoclicker_hotkey");
        if (savedHotkey) {
          setHotkey(savedHotkey);
        }
      } catch (e) {
        console.error("Failed to load autoclicker hotkey:", e);
      }
    })();
  }, []);

  // 监听后端状态变化（热键触发开关时实时刷新）
  useEffect(() => {
    const unlisten = listen<AutoClickerStatus>("autoclicker-status-changed", (event) => {
      setRunning(event.payload.running);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const saveSettings = useCallback(async (btn: string, ms: number) => {
    try {
      await invoke("autoclicker_update", { button: btn, intervalMs: ms });
      await store.set(AUTOCLICKER_STORE_KEY, { button: btn, interval_ms: ms });
      await store.save();
    } catch (e) {
      console.error("Failed to save autoclicker settings:", e);
    }
  }, []);

  const selectButton = (btn: "left" | "right") => {
    setButton(btn);
    saveSettings(btn, intervalMs);
  };

  const selectInterval = (ms: number) => {
    const v = Math.max(1, Math.min(10000, Math.round(ms)));
    setIntervalMs(v);
    saveSettings(button, v);
  };

  const saveHotkey = async (val: string) => {
    try {
      // Rust 端负责注册并写入 settings.json；这里仅同步前端 store 内存
      await invoke("set_autoclicker_hotkey", { shortcut: val });
      await store.set(HOTKEY_STORE_KEY, val);
      setHotkey(val);
      toast({
        title: t("autoclicker.hotkeySaved") || "快捷键已保存",
        status: "success",
        duration: 2000,
        isClosable: true,
      });
    } catch (e) {
      console.error("Failed to save autoclicker hotkey:", e);
      const msg = typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
      toast({
        title:
          msg && msg.trim() && msg !== "[object Object]"
            ? msg
            : t("autoclicker.hotkeySavedFailed") || "快捷键保存失败",
        status: "error",
        duration: 2000,
        isClosable: true,
      });
    }
  };

  const cps = Math.round(1000 / Math.max(1, intervalMs));

  return (
    <Box pt={8} pb={8}>
      <HStack justify="space-between" mb={6}>
        <HStack>
          <IconButton
            aria-label={t("builtinTools.back")}
            icon={<ArrowLeft size={20} />}
            variant="ghost"
            onClick={() => navigate("/builtin-tools")}
            color={headingColor}
          />
          <Heading size="lg" color={headingColor}>
            <HStack spacing={2}>
              <MousePointerClick size={24} color={themeColor} />
              <Text>{t("autoclicker.title")}</Text>
            </HStack>
          </Heading>
        </HStack>
        <Badge
          bg={running ? themeColor : undefined}
          color={running ? themeContrastText : subTextColor}
          px={3}
          py={1}
          borderRadius="full"
          fontSize="sm"
        >
          {running ? t("autoclicker.running") : t("autoclicker.stopped")}
        </Badge>
      </HStack>

      {/* 等宽三列卡片 */}
      <SimpleGrid columns={{ base: 1, md: 3 }} spacing={5} alignItems="stretch">
        {/* 快捷键 */}
        <SettingCard title={t("autoclicker.hotkey")}>
          <VStack align="stretch" spacing={3} justify="space-between" h="full">
            <Text fontSize="xs" color={subTextColor}>
              {t("autoclicker.hotkeyPresets")}
            </Text>
            <SimpleGrid columns={4} spacing={2}>
              {HOTKEY_PRESETS.map((p) => {
                const selected = hotkey === p.value;
                return (
                  <Box
                    key={p.value}
                    as="button"
                    py={2}
                    textAlign="center"
                    borderRadius="md"
                    border="2px solid"
                    borderColor={selected ? selectedBorder : "transparent"}
                    bg={selected ? selectedBg : "transparent"}
                    color={selected ? themeColor : textColor}
                    fontSize="sm"
                    transition="all 0.2s"
                    _hover={{ borderColor: selectedBorder }}
                    onClick={() => saveHotkey(p.value)}
                  >
                    {t(p.labelKey)}
                  </Box>
                );
              })}
            </SimpleGrid>
            <MouseHotkeyRecorder value={hotkey} onChange={saveHotkey} />
            <HStack spacing={2}>
              <Box w={2} h={2} borderRadius="full" bg={running ? themeColor : "gray.400"} />
              <Text fontSize="sm" color={running ? themeColor : textColor}>
                {running ? t("autoclicker.running") : t("autoclicker.stopped")}
              </Text>
            </HStack>
            <Text fontSize="xs" color={subTextColor}>
              {t("autoclicker.hotkeyHint")}
            </Text>
          </VStack>
        </SettingCard>

        {/* 点击键位 */}
        <SettingCard title={t("autoclicker.button")}>
          <VStack align="stretch" spacing={3} justify="space-between" h="full">
            <SimpleGrid columns={2} spacing={3}>
              {(["left", "right"] as const).map((b) => {
                const selected = button === b;
                return (
                  <Box
                    key={b}
                    as="button"
                    py={3}
                    textAlign="center"
                    borderRadius="lg"
                    border="2px solid"
                    borderColor={selected ? selectedBorder : "transparent"}
                    bg={selected ? selectedBg : "transparent"}
                    color={selected ? themeColor : textColor}
                    transition="all 0.2s"
                    _hover={{ borderColor: selectedBorder }}
                    onClick={() => selectButton(b)}
                  >
                    {b === "left" ? t("autoclicker.left") : t("autoclicker.right")}
                  </Box>
                );
              })}
            </SimpleGrid>
            <Text fontSize="xs" color={subTextColor}>
              {t("autoclicker.buttonHint")}
            </Text>
          </VStack>
        </SettingCard>

        {/* 点击频率 */}
        <SettingCard title={t("autoclicker.interval")}>
          <VStack align="stretch" spacing={3} justify="space-between" h="full">
            <SimpleGrid columns={2} spacing={3}>
              {PRESETS.map((p) => {
                const selected = intervalMs === p.ms;
                return (
                  <Box
                    key={p.cps}
                    as="button"
                    py={2}
                    textAlign="center"
                    borderRadius="lg"
                    border="2px solid"
                    borderColor={selected ? selectedBorder : "transparent"}
                    bg={selected ? selectedBg : "transparent"}
                    color={selected ? themeColor : textColor}
                    transition="all 0.2s"
                    _hover={{ borderColor: selectedBorder }}
                    onClick={() => selectInterval(p.ms)}
                  >
                    <Text fontWeight="bold">{p.cps}</Text>
                    <Text fontSize="xs" color={subTextColor}>{t("autoclicker.cps")}</Text>
                  </Box>
                );
              })}
            </SimpleGrid>
            <HStack spacing={2}>
              <Text fontSize="sm" color={subTextColor} whiteSpace="nowrap">
                {t("autoclicker.customInterval")}
              </Text>
              <NumberInput
                value={intervalMs}
                min={1}
                max={10000}
                step={10}
                onChange={(_, v) => {
                  if (!isNaN(v)) selectInterval(v);
                }}
                size="sm"
                flex="1"
              >
                <NumberInputField />
                <NumberInputStepper>
                  <NumberIncrementStepper />
                  <NumberDecrementStepper />
                </NumberInputStepper>
              </NumberInput>
            </HStack>
            <HStack>
              <Zap size={16} color={themeColor} />
              <Text fontSize="sm" color={textColor}>
                {t("autoclicker.currentSpeed")}:{" "}
                <Text as="span" fontWeight="bold" color={themeColor}>
                  {cps}
                </Text>{" "}
                {t("autoclicker.cps")}
              </Text>
            </HStack>
          </VStack>
        </SettingCard>
      </SimpleGrid>
    </Box>
  );
}
