import {
  Box, Flex, Heading, Text, VStack, HStack, Switch,
  Slider, SliderTrack, SliderFilledTrack, SliderThumb,
  useColorModeValue, useToast, IconButton,
  SimpleGrid, Select, Badge,
} from "@chakra-ui/react";
import {
  ArrowLeft, Volume2, Disc3, Music, Clapperboard,
  Mic, Gamepad2, Circle, RefreshCw,
} from "lucide-react";
import { useState, useEffect, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useNavigate } from "react-router-dom";
import { LazyStore } from "@tauri-apps/plugin-store";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";

// ─── Types ───

interface AudioDeviceInfo {
  id: string;
  name: string;
  is_default: boolean;
}

interface EqSettings {
  enabled: boolean;
  bands: number[];
  master_gain: number;
  output_device_id: string;
  preset_id: string;
}

interface Preset {
  id: string;
  nameKey: string;
  icon: React.ElementType;
  bands: number[];
  master_gain: number;
  color: string;
}

// ─── 10段 EQ 频率标签 ───

const BAND_FREQ_LABELS = ["31", "62", "125", "250", "500", "1K", "2K", "4K", "8K", "16K"];
const DEFAULT_BANDS = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

const store = new LazyStore("settings.json");
const EQ_STORE_KEY = "eq-settings";

const DEFAULT_SETTINGS: EqSettings = {
  enabled: false,
  bands: DEFAULT_BANDS,
  master_gain: 0,
  output_device_id: "default",
  preset_id: "flat",
};

// ─── Presets ───

const PRESETS: Preset[] = [
  { id: "flat", nameKey: "eqTuning.presets.flat", icon: Circle, bands: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0], master_gain: 0, color: "#8B949E" },
  { id: "gaming", nameKey: "eqTuning.presets.gaming", icon: Gamepad2, bands: [4, 3, 1, 0, -1, 0, 2, 5, 6, 4], master_gain: 2, color: "#00D9FF" },
  { id: "music", nameKey: "eqTuning.presets.music", icon: Music, bands: [2, 3, 4, 1, 0, 0, 1, 3, 4, 2], master_gain: 0, color: "#7B68EE" },
  { id: "movie", nameKey: "eqTuning.presets.movie", icon: Clapperboard, bands: [6, 5, 2, 0, 0, 0, 1, 3, 2, 1], master_gain: 3, color: "#FFA502" },
  { id: "voice", nameKey: "eqTuning.presets.voice", icon: Mic, bands: [-4, -2, 2, 3, 3, 3, 1, 0, -2, -4], master_gain: 1, color: "#00FF88" },
  { id: "custom", nameKey: "eqTuning.presets.custom", icon: Disc3, bands: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0], master_gain: 0, color: "#FF6B9D" },
];

// ─── Component ───

export default function EqTuningPage() {
  const { t } = useTranslation();
  const toast = useToast();
  const navigate = useNavigate();
  const { liquidGlassEnabled } = useBackground();
  const { getActiveColor } = useThemeColor();

  const [settings, setSettings] = useState<EqSettings>(DEFAULT_SETTINGS);
  const [devices, setDevices] = useState<AudioDeviceInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [toastMsg, setToastMsg] = useState<{ title: string; status: "info" | "success" | "error" } | null>(null);
  const debounceTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const initRef = useRef(false);

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const textColor = useColorModeValue("gray.700", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const cardBg = useColorModeValue("white", "#161B22");
  const cardBorder = useColorModeValue("gray.200", "#2a2a3a");
  const sliderTrackBg = useColorModeValue("gray.100", "#1C2333");

  // ─── Toast 副作用（避免跨组件 setState-in-render）───

  useEffect(() => {
    if (toastMsg) {
      toast({ title: toastMsg.title, status: toastMsg.status, duration: 1500, isClosable: true });
      setToastMsg(null);
    }
  }, [toastMsg, toast]);

  // ─── 持久化 + 后端同步（settings 变更时自动处理）───

  useEffect(() => {
    if (!initRef.current) return; // 初始化期间不触发

    store.set(EQ_STORE_KEY, settings);

    if (!settings.enabled) return;

    // 防抖同步到后端
    if (debounceTimer.current) clearTimeout(debounceTimer.current);
    debounceTimer.current = setTimeout(() => {
      invoke("update_eq_settings", { settings }).catch((e) => {
        console.error("EQ 后端更新失败:", e);
      });
    }, 150);

    return () => {
      if (debounceTimer.current) clearTimeout(debounceTimer.current);
    };
  }, [settings]);

  // ─── 加载保存的设置 + 恢复 EQ 运行状态 ───

  useEffect(() => {
    (async () => {
      try {
        const saved = await store.get<EqSettings>(EQ_STORE_KEY);
        if (saved) {
          const restored = {
            ...DEFAULT_SETTINGS,
            ...saved,
            bands: saved.bands || DEFAULT_BANDS,
          };
          setSettings(restored);

          // 如果保存的状态是运行中，尝试自动重启管线
          if (restored.enabled) {
            try {
              const result = await invoke<EqSettings>("start_eq", { settings: restored });
              const updated = { ...result, enabled: true };
              setSettings(updated);
              store.set(EQ_STORE_KEY, updated);
            } catch (e: any) {
              // 重启失败（可能是反馈回路），重置为关闭状态
              const off = { ...restored, enabled: false };
              setSettings(off);
              store.set(EQ_STORE_KEY, off);
              console.warn("EQ 自动重启失败:", String(e));
            }
          }
        }
      } catch { /* ignore */ }
      setLoading(false);
      initRef.current = true;
    })();
  }, []);

  // ─── 枚举音频设备 ───

  useEffect(() => {
    (async () => {
      try {
        const deviceList = await invoke<AudioDeviceInfo[]>("get_audio_devices");
        setDevices(deviceList);
      } catch {
        setDevices([{ id: "default", name: "默认音频输出设备", is_default: true }]);
      }
    })();
  }, []);

  // ─── 监听后端状态变更 ───

  useEffect(() => {
    const unlisten = listen<EqSettings>("eq-status-changed", (event) => {
      setSettings(event.payload);
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  // ─── 更新单个频段 ───

  const updateBand = useCallback((index: number, value: number) => {
    setSettings(prev => {
      const bands = [...prev.bands];
      bands[index] = Math.round(value * 2) / 2;
      return { ...prev, bands, preset_id: "custom" };
    });
  }, []);

  // ─── 应用预设 ───

  const applyPreset = useCallback((preset: Preset) => {
    setSettings(prev => ({
      ...prev,
      bands: [...preset.bands],
      master_gain: preset.master_gain,
      preset_id: preset.id,
    }));
    setToastMsg({ title: t(`eqTuning.presets.${preset.id}` as any), status: "success" });
  }, [t]);

  // ─── 主开关 ───

  const toggleEq = useCallback(async () => {
    if (settings.enabled) {
      try {
        await invoke("stop_eq");
        const next = { ...settings, enabled: false };
        setSettings(next);
        setToastMsg({ title: t("eqTuning.stopped"), status: "info" });
      } catch (e: any) {
        setToastMsg({ title: String(e), status: "error" });
      }
    } else {
      try {
        const result = await invoke<EqSettings>("start_eq", { settings });
        setToastMsg({ title: t("eqTuning.started"), status: "success" });
      } catch (e: any) {
        setToastMsg({ title: String(e), status: "error" });
      }
    }
  }, [settings, t]);

  // ─── 设备切换 ───

  const changeDevice = useCallback((deviceId: string) => {
    setSettings(prev => ({ ...prev, output_device_id: deviceId }));
  }, []);

  // ─── 渲染 ───

  if (loading) return <Box pt={8} />;

  return (
    <Box pt={8} pb={16}>
      <VStack align="stretch" spacing={6}>
        {/* 顶栏 */}
        <Flex justify="space-between" align="center">
          <HStack spacing={3}>
            <IconButton
              aria-label={t("builtinTools.back")}
              icon={<ArrowLeft size={20} />}
              variant="ghost"
              color={headingColor}
              onClick={() => navigate("/builtin-tools")}
              _hover={{ bg: "rgba(255,255,255,0.1)" }}
            />
            <Heading size="lg" color={headingColor}>{t("eqTuning.title")}</Heading>
          </HStack>
          <HStack spacing={3}>
            <Text fontSize="sm" color={subTextColor}>
              {settings.enabled ? t("eqTuning.running") : t("eqTuning.stopped")}
            </Text>
            <Switch
              size="lg"
              isChecked={settings.enabled}
              onChange={toggleEq}
              colorScheme="cyan"
              sx={{
                "& .chakra-switch__track": {
                  bg: settings.enabled ? "cyan.500" : "gray.500",
                  boxShadow: settings.enabled ? "0 0 12px rgba(0,217,255,0.5)" : "none",
                },
              }}
            />
          </HStack>
        </Flex>

        {/* 预设卡片 */}
        <LiquidGlassCard>
          <VStack align="stretch" spacing={4} p={1}>
            <Text fontWeight="semibold" color={headingColor} fontSize="sm">
              {t("eqTuning.presets.title")}
            </Text>
            <SimpleGrid columns={{ base: 2, md: 3, lg: 6 }} spacing={3}>
              {PRESETS.map(preset => {
                const isActive = settings.preset_id === preset.id;
                const Icon = preset.icon;
                return (
                  <Box
                    key={preset.id}
                    onClick={() => applyPreset(preset)}
                    p={3}
                    borderRadius="xl"
                    bg={isActive ? `${preset.color}18` : cardBg}
                    border="1px solid"
                    borderColor={isActive ? preset.color : cardBorder}
                    cursor="pointer"
                    transition="all 0.2s"
                    _hover={{
                      borderColor: preset.color,
                      transform: "translateY(-2px)",
                      shadow: "md",
                    }}
                  >
                    <VStack spacing={1}>
                      <Icon size={20} color={isActive ? preset.color : subTextColor} />
                      <Text fontSize="xs" fontWeight="bold" color={isActive ? preset.color : textColor}>
                        {t(preset.nameKey)}
                      </Text>
                      {/* 迷你频段预览 */}
                      <HStack spacing="1px" h="24px" w="full" justify="center">
                        {preset.bands.map((db, i) => {
                          const h = Math.abs(db) / 12 * 100;
                          return (
                            <Box key={i} w="6px" position="relative" h="full">
                              <Box
                                position="absolute"
                                bottom="50%"
                                w="full"
                                h={`${h / 2}%`}
                                bg={db >= 0 ? preset.color : "red.400"}
                                borderRadius="1px"
                                opacity={0.8}
                              />
                              <Box
                                position="absolute"
                                top="50%"
                                w="full"
                                h={`${h / 2}%`}
                                bg={db <= 0 ? preset.color : "red.400"}
                                borderRadius="1px"
                                opacity={0.6}
                              />
                            </Box>
                          );
                        })}
                      </HStack>
                    </VStack>
                  </Box>
                );
              })}
            </SimpleGrid>
          </VStack>
        </LiquidGlassCard>

        {/* 10段均衡器 */}
        <LiquidGlassCard>
          <VStack align="stretch" spacing={5} p={1}>
            <Flex justify="space-between" align="center">
              <Text fontWeight="semibold" color={headingColor} fontSize="sm">
                {t("eqTuning.equalizer")}
              </Text>
              <IconButton
                aria-label={t("eqTuning.reset")}
                icon={<RefreshCw size={16} />}
                size="xs"
                variant="ghost"
                color={subTextColor}
                onClick={() => applyPreset(PRESETS[0])}
              />
            </Flex>
            {BAND_FREQ_LABELS.map((label, i) => {
              const isMid = i === 5; // 1kHz
              return (
                <HStack key={i} spacing={3}>
                  <Text
                    fontSize="xs"
                    fontWeight={isMid ? "bold" : "normal"}
                    color={isMid ? "#00D9FF" : subTextColor}
                    w="36px"
                    textAlign="right"
                  >
                    {label}
                  </Text>
                  <Slider
                    flex={1}
                    min={-12}
                    max={12}
                    step={0.5}
                    value={settings.bands[i]}
                    onChange={(v) => updateBand(i, v)}
                    focusThumbOnChange={false}
                  >
                    <SliderTrack
                      bg={sliderTrackBg}
                      h="6px"
                      borderRadius="full"
                    >
                      <SliderFilledTrack
                        bg={`linear-gradient(90deg, #FF4757 0%, ${isMid ? "#00D9FF" : "#7B68EE"} 50%, #00D9FF 100%)`}
                      />
                    </SliderTrack>
                    <SliderThumb
                      boxSize={5}
                      bg={isMid ? "#00D9FF" : getActiveColor()}
                      boxShadow="0 0 8px rgba(0,217,255,0.4)"
                      _active={{ boxShadow: "0 0 16px rgba(0,217,255,0.6)" }}
                    />
                  </Slider>
                  <Text
                    fontSize="xs"
                    fontWeight="medium"
                    color={settings.bands[i] !== 0 ? headingColor : subTextColor}
                    w="40px"
                  >
                    {settings.bands[i] > 0 ? "+" : ""}{settings.bands[i].toFixed(1)}dB
                  </Text>
                </HStack>
              );
            })}

            {/* 主增益 */}
            <Box pt={3}>
              <HStack spacing={3}>
                <Text fontSize="xs" fontWeight="bold" color={subTextColor} w="36px" textAlign="right">
                  {t("eqTuning.masterGain")}
                </Text>
                <Slider
                  flex={1}
                  min={-20}
                  max={6}
                  step={0.5}
                  value={settings.master_gain}
                  onChange={(v) => setSettings(prev => ({ ...prev, master_gain: v }))}
                  focusThumbOnChange={false}
                >
                  <SliderTrack bg={sliderTrackBg} h="6px" borderRadius="full">
                    <SliderFilledTrack bg="linear-gradient(90deg, #FF4757, #FFA502, #00FF88)" />
                  </SliderTrack>
                  <SliderThumb boxSize={5} bg="#FFA502" boxShadow="0 0 8px rgba(255,165,2,0.4)" />
                </Slider>
                <Text fontSize="xs" fontWeight="medium" color={headingColor} w="40px">
                  {settings.master_gain > 0 ? "+" : ""}{settings.master_gain.toFixed(1)}dB
                </Text>
              </HStack>
            </Box>
          </VStack>
        </LiquidGlassCard>

        {/* 设备选择 */}
        <LiquidGlassCard>
          <VStack align="stretch" spacing={3} p={1}>
            <Text fontWeight="semibold" color={headingColor} fontSize="sm">
              {t("eqTuning.outputDevice")}
            </Text>
            <Select
              value={settings.output_device_id}
              onChange={(e) => changeDevice(e.target.value)}
              size="sm"
              bg={cardBg}
              borderColor={
                devices.some(d => d.is_default && (settings.output_device_id === "default" || settings.output_device_id === d.id))
                  ? "#FF4757"
                  : cardBorder
              }
              color={textColor}
              _hover={{ borderColor: "#00D9FF" }}
              icon={<Volume2 size={16} />}
            >
              {devices.map(dev => (
                <option key={dev.id} value={dev.id}>
                  {dev.is_default ? `🔊 ${dev.name}（捕获端）` : `🎵 ${dev.name}（输出端）`}
                </option>
              ))}
            </Select>
            {devices.length <= 1 && devices.some(d => d.is_default) && (
              <Text fontSize="xs" color="#FF4757">
                ⚠️ 仅检测到一个音频设备。EQ 无法将音频输出到与捕获相同的设备（会形成反馈啸叫）。
                请插入另一个音频输出设备（USB 耳机、HDMI 音响等）后重试。
              </Text>
            )}
            {devices.length > 1 && devices.some(d => d.is_default && (settings.output_device_id === "default" || settings.output_device_id === d.id)) && (
              <Text fontSize="xs" color="#FFA502">
                ⚠️ 当前输出设备与捕获设备相同，启动 EQ 时会自动切换到其他可用设备。
              </Text>
            )}
          </VStack>
        </LiquidGlassCard>
      </VStack>
    </Box>
  );
}
