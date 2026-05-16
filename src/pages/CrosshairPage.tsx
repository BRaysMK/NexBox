import {
  Box,
  Text,
  Heading,
  VStack,
  HStack,
  Switch,
  Grid,
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
} from "@chakra-ui/react";
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { Eye, EyeOff, ArrowLeft } from "lucide-react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useNavigate } from "react-router-dom";

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
  { id: "Dot", labelKey: "crosshair.styles.dot", icon: "●" },
  { id: "Circle", labelKey: "crosshair.styles.circle", icon: "○" },
  { id: "CrossDot", labelKey: "crosshair.styles.crossDot", icon: "✚" },
  { id: "CircleCross", labelKey: "crosshair.styles.circleCross", icon: "⊕" },
];

const COLOR_PRESETS = [
  { name: "Red", value: "#ff0000" },
  { name: "Green", value: "#00ff00" },
  { name: "Blue", value: "#0000ff" },
  { name: "Cyan", value: "#00ffff" },
  { name: "Magenta", value: "#ff00ff" },
  { name: "Yellow", value: "#ffff00" },
  { name: "White", value: "#ffffff" },
  { name: "Orange", value: "#ff8800" },
  { name: "Pink", value: "#ff0088" },
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
        <Text fontWeight="medium" color="white">{title}</Text>
        {children}
      </VStack>
    </Box>
  );
}



export default function CrosshairPage() {
  const { t } = useTranslation();
  const toast = useToast();
  const navigate = useNavigate();

  const [settings, setSettings] = useState<CrosshairSettings>(DEFAULT_SETTINGS);
  const [isLoading, setIsLoading] = useState(false);

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const textColor = useColorModeValue("gray.800", "#e0e0e0");

  useEffect(() => {
    loadSettings();
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

      <VStack align="stretch" spacing={5}>
        <SettingCard title={t("crosshair.enableCrosshair")}>
          <HStack justify="space-between">
            <HStack>
              <Icon as={settings.enabled ? Eye : EyeOff} boxSize={5} color={settings.enabled ? "green.400" : "gray.400"} />
              <Badge colorScheme={settings.enabled ? "green" : "gray"}>
                {settings.enabled ? t("crosshair.statusEnabled") : t("crosshair.statusDisabled")}
              </Badge>
            </HStack>
            <Switch
              isChecked={settings.enabled}
              onChange={toggleCrosshair}
              colorScheme="teal"
              isDisabled={isLoading}
              size="lg"
            />
          </HStack>
        </SettingCard>

        <SettingCard title={t("crosshair.style")}>
          <Grid templateColumns="repeat(5, 1fr)" gap={3}>
            {STYLE_OPTIONS.map((option) => (
              <Box
                key={option.id}
                bg={settings.style === option.id ? "teal.500" : useColorModeValue("gray.100", "#222222")}
                color={settings.style === option.id ? "white" : textColor}
                borderRadius="lg"
                py={4}
                textAlign="center"
                cursor="pointer"
                onClick={() => updateSetting("style", option.id)}
                _hover={{ bg: settings.style === option.id ? "teal.600" : useColorModeValue("gray.200", "#333333") }}
                transition="all 0.2s"
              >
                <Text fontSize="2xl" mb={1}>{option.icon}</Text>
                <Text fontSize="xs" fontWeight="medium">
                  {t(option.labelKey)}
                </Text>
              </Box>
            ))}
          </Grid>
        </SettingCard>

        <SettingCard title={t("crosshair.color")}>
          <VStack align="stretch" spacing={4}>
            <HStack wrap="wrap" gap={4}>
              {COLOR_PRESETS.map((color) => (
                <Box
                  key={color.value}
                  w={12}
                  h={12}
                  bg={color.value}
                  borderRadius="lg"
                  cursor="pointer"
                  border={settings.color === color.value ? "3px solid" : "3px solid transparent"}
                  borderColor={settings.color === color.value ? "teal.500" : "transparent"}
                  onClick={() => updateSetting("color", color.value)}
                  _hover={{ transform: "scale(1.1)" }}
                  transition="all 0.2s"
                  boxShadow="md"
                />
              ))}
            </HStack>
            <HStack spacing={3}>
              <Text color={textColor} fontSize="sm" whiteSpace="nowrap">
                {t("crosshair.customColor") || "自定义颜色"}:
              </Text>
              <Box
                as="input"
                type="color"
                value={settings.color}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) => updateSetting("color", e.target.value)}
                w={12}
                h={12}
                borderRadius="lg"
                cursor="pointer"
                border="2px solid"
                borderColor={useColorModeValue("gray.300", "#444444")}
                _hover={{ borderColor: "teal.400" }}
                transition="all 0.2s"
                sx={{
                  "&::-webkit-color-swatch-wrapper": {
                    padding: "4px",
                  },
                  "&::-webkit-color-swatch": {
                    borderRadius: "6px",
                    border: "none",
                  },
                }}
              />
              <Box
                px={3}
                py={1}
                bg={useColorModeValue("gray.100", "#222222")}
                borderRadius="md"
                border="1px solid"
                borderColor={useColorModeValue("gray.200", "#333333")}
              >
                <Text
                  color={settings.color}
                  fontSize="sm"
                  fontWeight="bold"
                  letterSpacing="0.05em"
                  fontFamily="'Microsoft YaHei', '微软雅黑', sans-serif"
                >
                  {settings.color.toUpperCase()}
                </Text>
              </Box>
            </HStack>
          </VStack>
        </SettingCard>

        <SettingCard title={t("crosshair.parameters")}>
          <VStack align="stretch" spacing={5}>
            <Box>
              <Text color={textColor} mb={2}>{t("crosshair.size")}: {settings.size}</Text>
              <Slider
                value={settings.size}
                min={10}
                max={100}
                step={1}
                onChange={(val) => updateSetting("size", val)}
              >
                <SliderTrack bg={useColorModeValue("gray.200", "gray.600")}>
                  <SliderFilledTrack bg="teal.400" />
                </SliderTrack>
                <SliderThumb />
              </Slider>
            </Box>

            <Box>
              <Text color={textColor} mb={2}>{t("crosshair.thickness")}: {settings.thickness}</Text>
              <Slider
                value={settings.thickness}
                min={1}
                max={10}
                step={1}
                onChange={(val) => updateSetting("thickness", val)}
              >
                <SliderTrack bg={useColorModeValue("gray.200", "gray.600")}>
                  <SliderFilledTrack bg="teal.400" />
                </SliderTrack>
                <SliderThumb />
              </Slider>
            </Box>

            <Box>
              <Text color={textColor} mb={2}>{t("crosshair.gap")}: {settings.gap}</Text>
              <Slider
                value={settings.gap}
                min={0}
                max={50}
                step={1}
                onChange={(val) => updateSetting("gap", val)}
              >
                <SliderTrack bg={useColorModeValue("gray.200", "gray.600")}>
                  <SliderFilledTrack bg="teal.400" />
                </SliderTrack>
                <SliderThumb />
              </Slider>
            </Box>

            <Box>
              <Text color={textColor} mb={2}>{t("crosshair.dotSize")}: {settings.dot_size}</Text>
              <Slider
                value={settings.dot_size}
                min={1}
                max={8}
                step={1}
                onChange={(val) => updateSetting("dot_size", val)}
              >
                <SliderTrack bg={useColorModeValue("gray.200", "gray.600")}>
                  <SliderFilledTrack bg="teal.400" />
                </SliderTrack>
                <SliderThumb />
              </Slider>
            </Box>

            <Box>
              <Text color={textColor} mb={2}>{t("crosshair.opacity")}: {settings.opacity}</Text>
              <Slider
                value={settings.opacity}
                min={50}
                max={255}
                step={5}
                onChange={(val) => updateSetting("opacity", val)}
              >
                <SliderTrack bg={useColorModeValue("gray.200", "gray.600")}>
                  <SliderFilledTrack bg="teal.400" />
                </SliderTrack>
                <SliderThumb />
              </Slider>
            </Box>

            <Button
              mt={2}
              colorScheme="gray"
              size="sm"
              alignSelf="flex-end"
              onClick={resetToDefault}
            >
              {t("crosshair.resetDefault") || "恢复默认"}
            </Button>
          </VStack>
        </SettingCard>
      </VStack>
    </Box>
  );
}
