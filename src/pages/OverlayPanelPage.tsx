import {
  Box,
  Text,
  Heading,
  VStack,
  HStack,
  Switch,
  Slider,
  SliderTrack,
  SliderFilledTrack,
  SliderThumb,
  Button,
  useColorModeValue,
  useToast,
  Badge,
  Icon,
  Divider,
  IconButton,
} from "@chakra-ui/react";
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { Eye, EyeOff, Cpu, Thermometer, Activity, HardDrive, Key, Gauge, ArrowLeft } from "lucide-react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useAppStartup } from "@/contexts/app-startup-context";
import { useNavigate } from "react-router-dom";

interface DisplayItems {
  fps: boolean;
  cpu_usage: boolean;
  gpu_temp: boolean;
  gpu_usage: boolean;
  memory_usage: boolean;
  delta_password: boolean;
}

interface OverlaySettings {
  display_items: DisplayItems;
  opacity: number;
}

interface HardwareData {
  fps: number | null;
  cpu_usage: number | null;
  gpu_temp: number | null;
  gpu_usage: number | null;
  memory_usage: number | null;
  delta_password: string | null;
}

const DEFAULT_DISPLAY_ITEMS: DisplayItems = {
  fps: false,
  cpu_usage: true,
  gpu_temp: true,
  gpu_usage: true,
  memory_usage: true,
  delta_password: false,
};

const DEFAULT_SETTINGS: OverlaySettings = {
  display_items: DEFAULT_DISPLAY_ITEMS,
  opacity: 200,
};

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

function SliderControl({
  label,
  value,
  min,
  max,
  onChange,
  suffix = "",
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  onChange: (val: number) => void;
  suffix?: string;
}) {
  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const sliderBg = useColorModeValue("gray.200", "gray.700");

  return (
    <Box>
      <HStack justify="space-between" mb={2}>
        <Text color={textColor} fontSize="sm">{label}</Text>
        <Text color="teal.400" fontSize="sm" fontWeight="bold">{value}{suffix}</Text>
      </HStack>
      <Slider value={value} min={min} max={max} onChange={onChange} colorScheme="teal">
        <SliderTrack bg={sliderBg}>
          <SliderFilledTrack />
        </SliderTrack>
        <SliderThumb />
      </Slider>
    </Box>
  );
}

function DisplayItemCheckbox({
  label,
  isChecked,
  onChange,
  icon,
  value,
}: {
  label: string;
  isChecked: boolean;
  onChange: (checked: boolean) => void;
  icon: React.ReactNode;
  value: string | null;
}) {
  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const valueColor = useColorModeValue("teal.500", "teal.300");

  return (
    <HStack justify="space-between" py={2}>
      <HStack spacing={3}>
        <Icon as={() => icon} boxSize={5} color={isChecked ? "teal.400" : "gray.400"} />
        <Text color={textColor} fontSize="sm">{label}</Text>
      </HStack>
      <HStack spacing={3}>
        {value !== null && (
          <Text color={valueColor} fontSize="sm" fontWeight="bold" minW="60px" textAlign="right">
            {value}
          </Text>
        )}
        <Switch
          isChecked={isChecked}
          onChange={(e) => onChange(e.target.checked)}
          colorScheme="teal"
          size="sm"
        />
      </HStack>
    </HStack>
  );
}

export default function OverlayPanelPage() {
  const { t } = useTranslation();
  const toast = useToast();
  const { overlaySettings, saveOverlaySettings } = useAppStartup();
  const navigate = useNavigate();

  const [hardwareData, setHardwareData] = useState<HardwareData>({
    fps: null,
    cpu_usage: null,
    gpu_temp: null,
    gpu_usage: null,
    memory_usage: null,
    delta_password: null,
  });
  const [isEnabled, setIsEnabled] = useState(false);
  const [isLoading, setIsLoading] = useState(false);

  const headingColor = useColorModeValue("gray.900", "#ffffff");

  const settings = overlaySettings || DEFAULT_SETTINGS;

  useEffect(() => {
    loadStatus();
  }, []);

  useEffect(() => {
    const interval = setInterval(() => {
      loadHardwareData();
    }, 1000);
    return () => clearInterval(interval);
  }, []);

  const loadStatus = async () => {
    try {
      const status = await invoke<boolean>("get_overlay_panel_status");
      setIsEnabled(status);
    } catch (error) {
      console.error("Failed to load overlay panel status:", error);
    }
  };

  const loadHardwareData = async () => {
    try {
      const data = await invoke<HardwareData>("get_overlay_hardware_data");
      setHardwareData(data);
    } catch (error) {
      console.error("Failed to load hardware data:", error);
    }
  };

  const startOverlay = async () => {
    setIsLoading(true);
    try {
      const result = await invoke<{ success: boolean; message: string }>("start_overlay_panel", {
        settings: settings,
      });
      if (result.success) {
        setIsEnabled(true);
        toast({
          title: result.message,
          status: "success",
          duration: 2000,
          isClosable: true,
        });
      }
    } catch (error) {
      console.error("Failed to start overlay panel:", error);
      toast({
        title: t("overlayPanel.startFailed") || "启动失败",
        status: "error",
        duration: 2000,
        isClosable: true,
      });
    }
    setIsLoading(false);
  };

  const stopOverlay = async () => {
    setIsLoading(true);
    try {
      const result = await invoke<{ success: boolean; message: string }>("stop_overlay_panel");
      if (result.success) {
        setIsEnabled(false);
        toast({
          title: result.message,
          status: "success",
          duration: 2000,
          isClosable: true,
        });
      }
    } catch (error) {
      console.error("Failed to stop overlay panel:", error);
      toast({
        title: t("overlayPanel.stopFailed") || "停止失败",
        status: "error",
        duration: 2000,
        isClosable: true,
      });
    }
    setIsLoading(false);
  };

  const toggleOverlay = async () => {
    if (isEnabled) {
      await stopOverlay();
    } else {
      await startOverlay();
    }
  };

  const updateSettings = (newSettings: OverlaySettings) => {
    saveOverlaySettings(newSettings);
  };

  const updateDisplayItem = (key: keyof DisplayItems, value: boolean) => {
    const newSettings = {
      ...settings,
      display_items: {
        ...settings.display_items,
        [key]: value,
      },
    };
    saveOverlaySettings(newSettings);
  };

  const updateSetting = <K extends keyof OverlaySettings>(
    key: K,
    value: OverlaySettings[K]
  ) => {
    const newSettings = { ...settings, [key]: value };
    saveOverlaySettings(newSettings);
  };

  const formatValue = (value: number | null, suffix: string): string => {
    if (value === null) return "--";
    return `${value}${suffix}`;
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
            {t("overlayPanel.title") || "悬浮框"}
          </Heading>
        </HStack>
      </HStack>

      <VStack align="stretch" spacing={5}>
        <SettingCard title={t("overlayPanel.enableOverlay") || "启用悬浮框"}>
          <HStack justify="space-between">
            <HStack>
              <Icon as={isEnabled ? Eye : EyeOff} boxSize={5} color={isEnabled ? "green.400" : "gray.400"} />
              <Badge colorScheme={isEnabled ? "green" : "gray"}>
                {isEnabled ? (t("overlayPanel.statusEnabled") || "已启用") : (t("overlayPanel.statusDisabled") || "已禁用")}
              </Badge>
            </HStack>
            <Switch
              isChecked={isEnabled}
              onChange={toggleOverlay}
              colorScheme="teal"
              isDisabled={isLoading}
              size="lg"
            />
          </HStack>
        </SettingCard>

        <SettingCard title={t("overlayPanel.displayItems") || "显示项"}>
          <VStack align="stretch" spacing={1}>
            <DisplayItemCheckbox
              label={t("overlayPanel.cpuUsage") || "CPU占用"}
              isChecked={settings.display_items.cpu_usage}
              onChange={(checked) => updateDisplayItem("cpu_usage", checked)}
              icon={<Cpu size={20} />}
              value={formatValue(hardwareData.cpu_usage, "%")}
            />
            <Divider />
            <DisplayItemCheckbox
              label={t("overlayPanel.gpuTemp") || "GPU温度"}
              isChecked={settings.display_items.gpu_temp}
              onChange={(checked) => updateDisplayItem("gpu_temp", checked)}
              icon={<Thermometer size={20} />}
              value={formatValue(hardwareData.gpu_temp, "C")}
            />
            <Divider />
            <DisplayItemCheckbox
              label={t("overlayPanel.gpuUsage") || "GPU占用"}
              isChecked={settings.display_items.gpu_usage}
              onChange={(checked) => updateDisplayItem("gpu_usage", checked)}
              icon={<Activity size={20} />}
              value={formatValue(hardwareData.gpu_usage, "%")}
            />
            <Divider />
            <DisplayItemCheckbox
              label={t("overlayPanel.memoryUsage") || "内存占用"}
              isChecked={settings.display_items.memory_usage}
              onChange={(checked) => updateDisplayItem("memory_usage", checked)}
              icon={<HardDrive size={20} />}
              value={hardwareData.memory_usage !== null 
                ? `${Math.round(hardwareData.memory_usage)}%` 
                : "--"}
            />
            <Divider />
            <DisplayItemCheckbox
              label={t("overlayPanel.deltaPassword") || "三角洲密码"}
              isChecked={settings.display_items.delta_password}
              onChange={(checked) => updateDisplayItem("delta_password", checked)}
              icon={<Key size={20} />}
              value={hardwareData.delta_password ? t("hardware.deltaPasswordFetched") : t("hardware.deltaPasswordNotFetched")}
            />
          </VStack>
        </SettingCard>

        <SettingCard title={t("overlayPanel.appearance") || "外观设置"}>
          <SliderControl
            label={t("overlayPanel.opacity") || "透明度"}
            value={Math.round(settings.opacity / 255 * 100)}
            min={20}
            max={100}
            onChange={(val) => updateSetting("opacity", Math.round(val / 100 * 255))}
            suffix="%"
          />
        </SettingCard>
      </VStack>
    </Box>
  );
}
