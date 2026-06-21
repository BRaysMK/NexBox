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
  useColorMode,
  useToast,
  Badge,
  Icon,
  IconButton,
  Button,
  Menu,
  MenuButton,
  MenuList,
  MenuItem,
  Portal,
} from "@chakra-ui/react";
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LazyStore } from "@tauri-apps/plugin-store";
import { useTranslation } from "react-i18next";
import { Eye, EyeOff, ArrowLeft, RotateCcw, Monitor, ChevronDown, Check, Image } from "lucide-react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useAppStartup } from "@/contexts/app-startup-context";
import { useNavigate } from "react-router-dom";
import { HotkeyRecorder } from "@/components/hotkey-recorder";
import { CustomColorPicker } from "@/components/special/custom-color-picker";
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
  monitor_index: number;
  use_custom_image: boolean;
  custom_image_path: string | null;
}

interface DisplayInfo {
  index: number;
  name: string;
  device_name: string;
  is_primary: boolean;
  width: number;
  height: number;
}

const CROSSHAIR_STORE_KEY = "crosshair-settings";
const store = new LazyStore("settings.json");

const DEFAULT_SETTINGS: CrosshairSettings = {
  enabled: false,
  style: "Cross",
  size: 20,
  thickness: 2,
  color: "#ff0000",
  gap: 0,
  dot_size: 2,
  opacity: 255,
  monitor_index: -1,
  use_custom_image: false,
  custom_image_path: null,
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
  const { colorMode } = useColorMode();
  const headerColor = colorMode === 'light' ? '#000000' : '#ffffff';

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
  const [displays, setDisplays] = useState<DisplayInfo[]>([]);

  const headingColor = useColorModeValue("black", "#ffffff");
  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const sliderBg = useColorModeValue("gray.200", "gray.600");
  const hoverBg = useColorModeValue("gray.100", "#252525");
  const menuListBg = useColorModeValue("white", "#1a1a1a");
  const inputBg = useColorModeValue("white", "#1a1a1a");

  useEffect(() => {
    loadSettings();
    loadDisplays();
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
      const isDefault = JSON.stringify(status) === JSON.stringify({ ...DEFAULT_SETTINGS, enabled: status.enabled });
      if (isDefault) {
        const saved = await store.get<CrosshairSettings>(CROSSHAIR_STORE_KEY);
        if (saved) {
          saved.enabled = status.enabled;
          setSettings(saved);
          await invoke("update_crosshair_settings", { settings: saved });
          return;
        }
      }
      setSettings(status);
    } catch (error) {
      console.error("Failed to load crosshair settings:", error);
    }
  };

  const loadDisplays = async () => {
    try {
      const list = await invoke<DisplayInfo[]>("get_crosshair_displays");
      setDisplays(list);
    } catch (error) {
      console.error("Failed to load displays:", error);
    }
  };

  const selectImage = async () => {
    try {
      const path = await invoke<string | null>("pick_crosshair_image");
      if (path) {
        updateSetting("custom_image_path", path);
      }
    } catch (error) {
      console.error("Failed to pick image:", error);
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
      await store.set(CROSSHAIR_STORE_KEY, newSettings);
      await store.save();
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

          <SettingCard title={t("crosshair.renderMode")}>
            <HStack spacing={3}>
              <LiquidGlassCard
                py={2.5}
                px={4}
                textAlign="center"
                cursor="pointer"
                flex={1}
                onClick={() => updateSetting("use_custom_image", false)}
                opacity={settings.use_custom_image ? 0.5 : 1}
                border={settings.use_custom_image ? "1px solid transparent" : `1px solid ${getActiveColor()}`}
              >
                <Text fontSize="sm" fontWeight="medium" color={settings.use_custom_image ? textColor : getActiveColor()}>
                  {t("crosshair.procedural")}
                </Text>
              </LiquidGlassCard>
              <LiquidGlassCard
                py={2.5}
                px={4}
                textAlign="center"
                cursor="pointer"
                flex={1}
                onClick={() => updateSetting("use_custom_image", true)}
                opacity={settings.use_custom_image ? 1 : 0.5}
                border={settings.use_custom_image ? `1px solid ${getActiveColor()}` : "1px solid transparent"}
              >
                <Text fontSize="sm" fontWeight="medium" color={settings.use_custom_image ? getActiveColor() : textColor}>
                  {t("crosshair.customImage")}
                </Text>
              </LiquidGlassCard>
            </HStack>
          </SettingCard>

          <SettingCard title={t("crosshair.monitor")}>
            <Menu matchWidth>
              <MenuButton
                as={Box}
                bg="transparent"
                p={0}
                border="none"
                w="full"
                cursor="pointer"
              >
                <LiquidGlassCard px={3} py={1.5}>
                  <HStack justify="space-between">
                    <HStack spacing={2}>
                      <Monitor size={14} />
                      <Text fontSize="sm" color={textColor}>
                        {settings.monitor_index === -1
                          ? t("crosshair.primaryMonitor")
                          : displays.find(d => d.index === settings.monitor_index)?.name || t("crosshair.primaryMonitor")}
                      </Text>
                    </HStack>
                    <ChevronDown size={16} />
                  </HStack>
                </LiquidGlassCard>
              </MenuButton>
              <Portal>
                <MenuList bg={menuListBg} borderColor={cardBorder} maxH="300px" overflowY="auto" zIndex={9999}>
                  <MenuItem
                    onClick={() => updateSetting("monitor_index", -1)}
                    bg={settings.monitor_index === -1 ? hoverBg : "transparent"}
                    _hover={{ bg: hoverBg }}
                  >
                    <HStack spacing={2} w="full" justify="space-between">
                      <Text fontSize="sm">{t("crosshair.primaryMonitor")}</Text>
                      {settings.monitor_index === -1 && <Check size={14} color={getActiveColor()} />}
                    </HStack>
                  </MenuItem>
                  {displays.map((d) => (
                    <MenuItem
                      key={d.index}
                      onClick={() => updateSetting("monitor_index", d.index)}
                      bg={settings.monitor_index === d.index ? hoverBg : "transparent"}
                      _hover={{ bg: hoverBg }}
                    >
                      <HStack spacing={2} w="full" justify="space-between">
                        <Text fontSize="sm">{d.name}</Text>
                        {settings.monitor_index === d.index && <Check size={14} color={getActiveColor()} />}
                      </HStack>
                    </MenuItem>
                  ))}
                </MenuList>
              </Portal>
            </Menu>
          </SettingCard>

          {!settings.use_custom_image && (
            <SettingCard title={t("crosshair.style")}>
              <SimpleGrid columns={5} spacing={2}>
                {STYLE_OPTIONS.map((option) => {
                  const isActive = settings.style === option.id;
                  return (
                    <LiquidGlassCard
                      key={option.id}
                      py={3}
                      textAlign="center"
                      cursor="pointer"
                      onClick={() => updateSetting("style", option.id)}
                    >
                      <Text fontSize="xl" mb={0.5} color={isActive ? getActiveColor() : textColor}>{option.icon}</Text>
                      <Text fontSize="xs" fontWeight="medium" color={isActive ? getActiveColor() : textColor}>
                        {t(option.labelKey)}
                      </Text>
                    </LiquidGlassCard>
                  );
                })}
              </SimpleGrid>
            </SettingCard>
          )}

          {!settings.use_custom_image && (
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
                <CustomColorPicker color={settings.color} onChange={(c) => updateSetting("color", c)} />
              </HStack>
            </VStack>
          </SettingCard>
          )}
        </VStack>

        <VStack align="stretch" spacing={5}>
          <SettingCard title={t("crosshair.parameters")}>
            <VStack align="stretch" spacing={4}>
              {settings.use_custom_image ? (
                <>
                  <Box>
                    <Button
                      leftIcon={<Image size={16} />}
                      w="full"
                      variant="outline"
                      colorScheme="gray"
                      size="sm"
                      onClick={selectImage}
                      justifyContent="flex-start"
                      h="auto"
                      py={2.5}
                      whiteSpace="normal"
                      textAlign="left"
                    >
                      <VStack align="stretch" spacing={0.5}>
                        <Text fontSize="sm" color={textColor}>
                          {settings.custom_image_path
                            ? settings.custom_image_path.split(/[\\/]/).pop()
                            : t("crosshair.selectImage")}
                        </Text>
                        {settings.custom_image_path && (
                          <Text fontSize="2xs" color={subTextColor} noOfLines={1}>
                            {settings.custom_image_path}
                          </Text>
                        )}
                      </VStack>
                    </Button>
                  </Box>

                  <Box>
                    <HStack justify="space-between" mb={1}>
                      <Text color={textColor} fontSize="sm">{t("crosshair.size")}</Text>
                      <Text color={getActiveColor()} fontSize="sm" fontWeight="bold">{settings.size}</Text>
                    </HStack>
                    <Slider value={settings.size} min={1} max={200} step={1} onChange={(val) => updateSetting("size", val)}>
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
                    <Slider value={settings.opacity} min={0} max={255} step={5} onChange={(val) => updateSetting("opacity", val)}>
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
                        <Image size={20} color="white" />
                      </Box>
                      <VStack align="flex-start" spacing={0}>
                        <Text fontSize="xs" color={subTextColor} fontWeight="medium">{t("crosshair.preview")}</Text>
                        <Text fontSize="2xs" color={subTextColor}>{t("crosshair.customImage")}</Text>
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
                </>
              ) : (
                <>
                  <Box>
                    <HStack justify="space-between" mb={1}>
                      <Text color={textColor} fontSize="sm">{t("crosshair.size")}</Text>
                      <Text color={getActiveColor()} fontSize="sm" fontWeight="bold">{settings.size}</Text>
                    </HStack>
                    <Slider value={settings.size} min={1} max={100} step={1} onChange={(val) => updateSetting("size", val)}>
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
                        <Text fontSize="2xs" color={subTextColor}>{t(STYLE_OPTIONS.find(s => s.id === settings.style)?.labelKey ?? "crosshair.styles.cross")}</Text>
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
                </>
              )}
            </VStack>
          </SettingCard>
        </VStack>
      </SimpleGrid>
    </Box>
  );
}
