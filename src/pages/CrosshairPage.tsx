import {
  Box,
  Text,
  Heading,
  VStack,
  HStack,
  Switch,
  SimpleGrid,
  Slider,
  SliderTrack,
  SliderFilledTrack,
  SliderThumb,
  useColorModeValue,
  useToast,
  Badge,
  Icon,
  IconButton,
  Button,
  Tooltip,
  Input,
} from "@chakra-ui/react";
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { Eye, EyeOff, ArrowLeft, RotateCcw } from "lucide-react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useAppStartup } from "@/contexts/app-startup-context";
import { useNavigate } from "react-router-dom";
import { HotkeyRecorder } from "@/components/hotkey-recorder";
import { useThemeColor } from "@/contexts/theme-color-context";

interface CrosshairSettings {
  enabled: boolean;
  style: string;
  size: number;
  thickness: number;
  color: string;
  gap: number;
  dot_size: number;
  opacity: number;
}

const DEFAULT_SETTINGS: CrosshairSettings = {
  enabled: false,
  style: "Cross",
  size: 20,
  thickness: 2,
  color: "#ff0000",
  gap: 0,
  dot_size: 2,
  opacity: 255,
};

const STYLE_OPTIONS = [
  { id: "Cross", labelKey: "crosshair.styles.cross", icon: "+" },
  { id: "Dot", labelKey: "crosshair.styles.dot", icon: "\u25CF" },
  { id: "Circle", labelKey: "crosshair.styles.circle", icon: "\u25CB" },
  { id: "CrossDot", labelKey: "crosshair.styles.crossDot", icon: "\u271A" },
  { id: "CircleCross", labelKey: "crosshair.styles.circleCross", icon: "\u2295" },
];

const COLOR_PRESETS = [
  { value: "#ff0000" },
  { value: "#00ff00" },
  { value: "#0000ff" },
  { value: "#00ffff" },
  { value: "#ff00ff" },
  { value: "#ffff00" },
  { value: "#ffffff" },
  { value: "#ff8800" },
  { value: "#ff0088" },
];

function SettingCard({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  const { liquidGlassEnabled } = useBackground();
  const cardBg = useColorModeValue("white", "#111111");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const headerColor = useColorModeValue("gray.900", "#ffffff");

  if (liquidGlassEnabled) {
    return (
      <LiquidGlassCard p={5}>
        <VStack align="stretch" spacing={4}>
          <Text fontWeight="medium" color="white">{title}</Text>
          {children}
        </VStack>
      </LiquidGlassCard>
    );
  }

  return (
    <Box bg={cardBg} borderRadius="xl" p={5} border="1px solid" borderColor={borderColor}>
      <VStack align="stretch" spacing={4}>
        <Text fontWeight="medium" color={headerColor}>{title}</Text>
        {children}
      </VStack>
    </Box>
  );
}

export default function CrosshairPage() {
  const { t } = useTranslation();
  const toast = useToast();
  const navigate = useNavigate();
  const { crosshairHotkey, saveCrosshairHotkey } = useAppStartup();
  const { getActiveColor, getHoverColor, getBorderColor, getContrastTextColor } = useThemeColor();

  const [settings, setSettings] = useState<CrosshairSettings>(DEFAULT_SETTINGS);
  const [isLoading, setIsLoading] = useState(false);

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const sliderBg = useColorModeValue("gray.200", "gray.600");

  useEffect(() => {
    loadSettings();
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    listen<void>("crosshair-status-changed", () => {
      loadSettings();
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const loadSettings = async () => {
    try {
      const status = await invoke<CrosshairSettings>("get_crosshair_status");
      setSettings(status);
    } catch (error) {
      console.error("Failed to load crosshair settings:", error);
    }
  };

  const resetToDefault = () => {
    const defaults: CrosshairSettings = {
      ...DEFAULT_SETTINGS,
      enabled: settings.enabled,
    };
    updateSettings(defaults);
  };

  const updateSettings = async (newSettings: CrosshairSettings) => {
    setSettings(newSettings);
    setIsLoading(true);
    try {
      await invoke("update_crosshair_settings", { settings: newSettings });
    } catch (error) {
      console.error("Failed to update settings:", error);
      toast({
        title: t("crosshair.updateFailed"),
        status: "error",
        duration: 2000,
        isClosable: true,
      });
    }
    setIsLoading(false);
  };

  const toggleCrosshair = async () => {
    setIsLoading(true);
    try {
      const result = await invoke<{ success: boolean; message: string }>("toggle_crosshair");
      if (result.success) {
        setSettings(prev => ({ ...prev, enabled: !prev.enabled }));
        toast({
          title: result.message,
          status: "success",
          duration: 2000,
          isClosable: true,
        });
      }
    } catch (error) {
      console.error("Failed to toggle crosshair:", error);
      toast({
        title: t("crosshair.toggleFailed"),
        status: "error",
        duration: 2000,
        isClosable: true,
      });
    }
    setIsLoading(false);
  };

  const updateSetting = <K extends keyof CrosshairSettings>(
    key: K,
    value: CrosshairSettings[K]
  ) => {
    const newSettings = { ...settings, [key]: value };
    updateSettings(newSettings);
  };

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
            {t("crosshair.title")}
          </Heading>
        </HStack>
      </HStack>

      <SimpleGrid columns={2} spacing={5}>
        <VStack align="stretch" spacing={5}>
          <SettingCard title={t("crosshair.enableCrosshair")}>
            <HStack justify="space-between" wrap="wrap" spacing={4}>
              <HStack>
                <Icon as={settings.enabled ? Eye : EyeOff} boxSize={5} color={settings.enabled ? "green.400" : "gray.400"} />
                <Badge colorScheme={settings.enabled ? "green" : "gray"}>
                  {settings.enabled ? t("crosshair.statusEnabled") : t("crosshair.statusDisabled")}
                </Badge>
              </HStack>
              <HStack spacing={4}>
                <HotkeyRecorder
                  value={crosshairHotkey}
                  onChange={(val) => {
                    saveCrosshairHotkey(val);
                    toast({
                      title: t("crosshair.hotkeySaved") || "快捷键已保存",
                      status: "success",
                      duration: 2000,
                      isClosable: true,
                    });
                  }}
                />
                <Switch
                  isChecked={settings.enabled}
                  onChange={toggleCrosshair}
                  isDisabled={isLoading}
                  size="lg"
                  sx={{
                    '& .chakra-switch__track[data-checked]': {
                      bg: getActiveColor(),
                    },
                  }}
                />
              </HStack>
            </HStack>
          </SettingCard>

          <SettingCard title={t("crosshair.style")}>
            <SimpleGrid columns={5} spacing={2}>
              {STYLE_OPTIONS.map((option) => {
                const isActive = settings.style === option.id;
                return (
                  <Box
                    key={option.id}
                    bg={isActive ? getActiveColor() : useColorModeValue("gray.100", "#222222")}
                    color={isActive ? getContrastTextColor() : textColor}
                    borderRadius="lg"
                    py={3}
                    textAlign="center"
                    cursor="pointer"
                    onClick={() => updateSetting("style", option.id)}
                    _hover={{ bg: isActive ? getActiveColor() : useColorModeValue("gray.200", "#333333") }}
                    transition="all 0.15s"
                  >
                    <Text fontSize="xl" mb={0.5}>{option.icon}</Text>
                    <Text fontSize="xs" fontWeight="medium">
                      {t(option.labelKey)}
                    </Text>
                  </Box>
                );
              })}
            </SimpleGrid>
          </SettingCard>

          <SettingCard title={t("crosshair.color")}>
            <VStack align="stretch" spacing={3}>
              <HStack flexWrap="wrap" gap={2}>
                {COLOR_PRESETS.map((color) => (
                  <Box
                    key={color.value}
                    w={8}
                    h={8}
                    bg={color.value}
                    borderRadius="md"
                    cursor="pointer"
                    border="2px solid"
                    borderColor={settings.color === color.value ? getActiveColor() : "transparent"}
                    onClick={() => updateSetting("color", color.value)}
                    _hover={{ transform: "scale(1.15)" }}
                    transition="all 0.15s"
                    boxShadow={settings.color === color.value ? `0 0 8px ${color.value}` : "none"}
                  />
                ))}
                <Tooltip label={t("crosshair.customColor") || "自定义颜色"}>
                  <Box
                    position="relative"
                    w={8}
                    h={8}
                    borderRadius="md"
                    overflow="hidden"
                    cursor="pointer"
                    border="2px dashed"
                    borderColor={cardBorder}
                    flexShrink={0}
                  >
                    <Box w="100%" h="100%" bg={settings.color} />
                    <Input
                      type="color"
                      value={settings.color}
                      onChange={(e: React.ChangeEvent<HTMLInputElement>) => updateSetting("color", e.target.value)}
                      position="absolute"
                      top={0}
                      left={0}
                      w="100%"
                      h="100%"
                      opacity={0}
                      cursor="pointer"
                    />
                  </Box>
                </Tooltip>
              </HStack>
              <Text fontSize="xs" color={subTextColor} fontFamily="mono">
                {settings.color.toUpperCase()}
              </Text>
            </VStack>
          </SettingCard>
        </VStack>

        <VStack align="stretch" spacing={5}>
          <SettingCard title={t("crosshair.parameters")}>
            <VStack align="stretch" spacing={4}>
              <Box>
                <HStack justify="space-between" mb={1}>
                  <Text color={textColor} fontSize="sm">{t("crosshair.size")}</Text>
                  <Text color={getActiveColor()} fontSize="sm" fontWeight="bold">{settings.size}</Text>
                </HStack>
                <Slider value={settings.size} min={10} max={100} step={1} onChange={(val) => updateSetting("size", val)}>
                  <SliderTrack bg={sliderBg}><SliderFilledTrack bg={getActiveColor()} /></SliderTrack>
                  <SliderThumb />
                </Slider>
              </Box>

              <Box>
                <HStack justify="space-between" mb={1}>
                  <Text color={textColor} fontSize="sm">{t("crosshair.thickness")}</Text>
                  <Text color={getActiveColor()} fontSize="sm" fontWeight="bold">{settings.thickness}</Text>
                </HStack>
                <Slider value={settings.thickness} min={1} max={10} step={1} onChange={(val) => updateSetting("thickness", val)}>
                  <SliderTrack bg={sliderBg}><SliderFilledTrack bg={getActiveColor()} /></SliderTrack>
                  <SliderThumb />
                </Slider>
              </Box>

              <Box>
                <HStack justify="space-between" mb={1}>
                  <Text color={textColor} fontSize="sm">{t("crosshair.gap")}</Text>
                  <Text color={getActiveColor()} fontSize="sm" fontWeight="bold">{settings.gap}</Text>
                </HStack>
                <Slider value={settings.gap} min={0} max={50} step={1} onChange={(val) => updateSetting("gap", val)}>
                  <SliderTrack bg={sliderBg}><SliderFilledTrack bg={getActiveColor()} /></SliderTrack>
                  <SliderThumb />
                </Slider>
              </Box>

              <Box>
                <HStack justify="space-between" mb={1}>
                  <Text color={textColor} fontSize="sm">{t("crosshair.dotSize")}</Text>
                  <Text color={getActiveColor()} fontSize="sm" fontWeight="bold">{settings.dot_size}</Text>
                </HStack>
                <Slider value={settings.dot_size} min={1} max={8} step={1} onChange={(val) => updateSetting("dot_size", val)}>
                  <SliderTrack bg={sliderBg}><SliderFilledTrack bg={getActiveColor()} /></SliderTrack>
                  <SliderThumb />
                </Slider>
              </Box>

              <Box>
                <HStack justify="space-between" mb={1}>
                  <Text color={textColor} fontSize="sm">{t("crosshair.opacity")}</Text>
                  <Text color={getActiveColor()} fontSize="sm" fontWeight="bold">
                    {Math.round(settings.opacity / 255 * 100)}%
                  </Text>
                </HStack>
                <Slider value={settings.opacity} min={50} max={255} step={5} onChange={(val) => updateSetting("opacity", val)}>
                  <SliderTrack bg={sliderBg}><SliderFilledTrack bg={getActiveColor()} /></SliderTrack>
                  <SliderThumb />
                </Slider>
              </Box>

              <HStack justify="space-between" pt={1}>
                <HStack spacing={2}>
                  <Box
                    w={10} h={10}
                    borderRadius="md"
                    bg="black"
                    display="flex"
                    alignItems="center"
                    justifyContent="center"
                    opacity={settings.opacity / 255}
                  >
                    <Text fontSize="lg" color={settings.color} fontWeight="bold" lineHeight={1}>
                      {STYLE_OPTIONS.find(s => s.id === settings.style)?.icon || "+"}
                    </Text>
                  </Box>
                  <VStack align="flex-start" spacing={0}>
                    <Text fontSize="xs" color={subTextColor} fontWeight="medium">{t("crosshair.preview")}</Text>
                    <Text fontSize="2xs" color={subTextColor}>{t(`crosshair.styles.${settings.style.toLowerCase()}`)}</Text>
                  </VStack>
                </HStack>
                <Button
                  leftIcon={<RotateCcw size={13} />}
                  colorScheme="gray"
                  variant="outline"
                  size="sm"
                  onClick={resetToDefault}
                >
                  {t("crosshair.resetDefault") || "恢复默认"}
                </Button>
              </HStack>
            </VStack>
          </SettingCard>
        </VStack>
      </SimpleGrid>
    </Box>
  );
}
