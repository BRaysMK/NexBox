import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import {
  Box,
  Heading,
  VStack,
  HStack,
  Slider,
  SliderTrack,
  SliderFilledTrack,
  SliderThumb,
  useColorModeValue,
  Text,
  IconButton,
} from "@chakra-ui/react";
import { ArrowLeft } from "lucide-react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { ThemeSwitch } from "@/components/special/theme-switch";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { HotkeyRecorder } from "@/components/hotkey-recorder";
import { useAppStartup } from "@/contexts/app-startup-context";
import { useToast } from "@chakra-ui/react";

export default function DynamicIslandPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { islandHotkey, saveIslandHotkey } = useAppStartup();
  const toast = useToast();

  // State from localStorage (shared with widget window)
  // Default: music controller & message notification ON for new users
  const [isVisible, setIsVisible] = useState(false);
  const [islandTheme, setIslandTheme] = useState(() => localStorage.getItem("nsd_island_theme") || "black");
  const [musicCtrl, setMusicCtrl] = useState(() => localStorage.getItem("nsd_music_ctrl") !== "false");
  const [msgNotify, setMsgNotify] = useState(() => localStorage.getItem("nsd_msg_notify") !== "false");
  const [hardwareMon, setHardwareMon] = useState(() => localStorage.getItem("nsd_hardware_mon") === "true");
  const [pinTaskbar, setPinTaskbar] = useState(() => localStorage.getItem("nsd_pin_taskbar") === "true");
  const [opacity, setOpacity] = useState(() => Number(localStorage.getItem("nsd_island_opacity") || "100"));

  // Theme-aware colors
  const headingColor = useColorModeValue("gray.800", "white");
  const labelColor = useColorModeValue("gray.700", "gray.200");
  const descColor = useColorModeValue("gray.500", "gray.400");
  const switchColorScheme = undefined; // ThemeSwitch handles styling natively
  const sliderTrackBg = useColorModeValue("gray.100", "whiteAlpha.200");
  const sliderFilledBg = useColorModeValue("teal.400", "teal.300");
  const sliderThumbBg = useColorModeValue("teal.500", "teal.300");
  const sliderThumbBorder = useColorModeValue("white", "gray.800");

  // On first launch, sync defaults to localStorage
  useEffect(() => {
    if (localStorage.getItem("nsd_music_ctrl") === null) {
      localStorage.setItem("nsd_music_ctrl", "true");
    }
    if (localStorage.getItem("nsd_msg_notify") === null) {
      localStorage.setItem("nsd_msg_notify", "true");
    }
  }, []);

  // Check initial widget status
  useEffect(() => {
    let attempts = 0;
    const interval = setInterval(async () => {
      try {
        const visible = await invoke<boolean>("is_widget_visible");
        setIsVisible(visible);
        clearInterval(interval);
      } catch {
        attempts++;
        if (attempts >= 6) clearInterval(interval);
      }
    }, 200);
    return () => clearInterval(interval);
  }, []);

  const toggleVisibility = async () => {
    const next = !isVisible;
    localStorage.setItem("nsd_island_visible", String(next));
    await emit("control-island-visibility", { show: next });
    setIsVisible(next);
  };

  const handleThemeChange = async (theme: string) => {
    setIslandTheme(theme);
    localStorage.setItem("nsd_island_theme", theme);
    await emit("control-island-theme", { theme });
  };

  const handleMusicCtrlChange = async (enabled: boolean) => {
    setMusicCtrl(enabled);
    localStorage.setItem("nsd_music_ctrl", String(enabled));
    await emit("control-music-ctl", { enabled });
    if (enabled && hardwareMon) {
      setHardwareMon(false);
      localStorage.setItem("nsd_hardware_mon", "false");
      await emit("control-hardware-mon", { enabled: false });
    }
  };

  const handleMsgNotifyChange = (enabled: boolean) => {
    setMsgNotify(enabled);
    localStorage.setItem("nsd_msg_notify", String(enabled));
  };

  const handleHardwareMonChange = async (enabled: boolean) => {
    setHardwareMon(enabled);
    localStorage.setItem("nsd_hardware_mon", String(enabled));
    await emit("control-hardware-mon", { enabled });
    if (enabled && musicCtrl) {
      setMusicCtrl(false);
      localStorage.setItem("nsd_music_ctrl", "false");
      await emit("control-music-ctl", { enabled: false });
    }
  };

  const handlePinTaskbarChange = async (enabled: boolean) => {
    setPinTaskbar(enabled);
    localStorage.setItem("nsd_pin_taskbar", String(enabled));
    await emit("control-pin-taskbar", { enabled });
  };

  const handleOpacityChange = async (value: number) => {
    setOpacity(value);
    localStorage.setItem("nsd_island_opacity", String(value));
    await emit("control-island-opacity", { opacity: value });
  };

  return (
    <Box w="full" h="full" px={{ base: 4, md: 8 }} py={6}>
      <VStack align="stretch" spacing={6} w="full">

        {/* Header Row */}
        <HStack spacing={3} align="center">
          <IconButton
            aria-label="返回内置工具"
            icon={<ArrowLeft size={20} />}
            variant="ghost"
            onClick={() => navigate("/builtin-tools")}
            color={headingColor}
            size="sm"
          />
          <Heading size="lg" color={headingColor}>
            {t("sidebar.dynamicIsland") || "灵动岛"}
          </Heading>
        </HStack>

        {/* Main Switch Card */}
        <LiquidGlassCard w="full" p={5}>
          <HStack justify="space-between" w="full">
            <Box>
              <Text fontWeight="600" color={labelColor} fontSize="md">
                灵动岛开关
              </Text>
              <Text fontSize="sm" color={descColor} mt={1}>
                {isVisible ? "灵动岛已开启，显示在屏幕顶部" : "点击开启灵动岛"}
              </Text>
            </Box>
            <HStack spacing={3}>
              <HotkeyRecorder
                value={islandHotkey}
                onChange={(val) => {
                  saveIslandHotkey(val);
                  toast({
                    title: "快捷键已保存",
                    status: "success",
                    duration: 2000,
                    isClosable: true,
                  });
                }}
              />
              <ThemeSwitch
                size="lg"
                isChecked={isVisible}
                onChange={toggleVisibility}
              />
            </HStack>
          </HStack>
        </LiquidGlassCard>

        {/* Settings Card */}
        <LiquidGlassCard w="full" p={5}>
          <VStack spacing={5} align="stretch" divider={<Box borderBottom="1px solid" borderColor={descColor} opacity={0.15} />}>

            {/* Island Theme */}
            <SettingRow label="灵动岛颜色" desc="切换暗色/亮色背景" labelColor={labelColor} descColor={descColor}>
              <HStack spacing={1}>
                <CapsuleButton
                  active={islandTheme === "black"}
                  label="暗色"
                  onClick={() => handleThemeChange("black")}
                />
                <CapsuleButton
                  active={islandTheme === "white"}
                  label="亮色"
                  onClick={() => handleThemeChange("white")}
                />
              </HStack>
            </SettingRow>

            {/* Music Controller */}
            <SettingRow
              label="音乐控制器"
              desc="支持网易云音乐控制及歌曲信息显示"
              labelColor={labelColor}
              descColor={descColor}
            >
              <ThemeSwitch
                isChecked={musicCtrl}
                onChange={(e) => { void handleMusicCtrlChange(e.target.checked); }}
              />
            </SettingRow>

            {/* Message Notification */}
            <SettingRow
              label="消息通知接收"
              desc="启用系统控制中心消息弹窗提醒"
              labelColor={labelColor}
              descColor={descColor}
            >
              <ThemeSwitch
                isChecked={msgNotify}
                onChange={(e) => handleMsgNotifyChange(e.target.checked)}
              />
            </SettingRow>

            {/* Hardware Monitor */}
            <SettingRow
              label="系统硬件监控"
              desc="显示 CPU / GPU / 内存实时占用率"
              labelColor={labelColor}
              descColor={descColor}
            >
              <ThemeSwitch
                isChecked={hardwareMon}
                onChange={(e) => { void handleHardwareMonChange(e.target.checked); }}
              />
            </SettingRow>

            {/* Pin to Taskbar */}
            <SettingRow
              label="置于任务栏"
              desc="将灵动岛锁定至任务栏左下角"
              labelColor={labelColor}
              descColor={descColor}
            >
              <ThemeSwitch
                isChecked={pinTaskbar}
                onChange={(e) => { void handlePinTaskbarChange(e.target.checked); }}
              />
            </SettingRow>

            {/* Opacity */}
            <Box>
              <HStack justify="space-between" mb={3}>
                <Box>
                  <Text fontWeight="600" color={labelColor} fontSize="sm">
                    悬浮窗不透明度
                  </Text>
                  <Text fontSize="xs" color={descColor}>
                    调节灵动岛的外观透明度 ({opacity}%)
                  </Text>
                </Box>
              </HStack>
              <Slider
                value={opacity}
                onChange={handleOpacityChange}
                min={0}
                max={100}
                step={1}
              >
                <SliderTrack bg={sliderTrackBg}>
                  <SliderFilledTrack bg={sliderFilledBg} />
                </SliderTrack>
                <SliderThumb boxSize={5} bg={sliderThumbBg} border="2px solid" borderColor={sliderThumbBorder} />
              </Slider>
            </Box>
          </VStack>
        </LiquidGlassCard>
      </VStack>
    </Box>
  );
}

// ─── Sub-components ──────────────────────────────────────────────

function CapsuleButton({ active, label, onClick }: { active: boolean; label: string; onClick: () => void }) {
  const bg = useColorModeValue("white", "#1a1a1a");
  const textColor = useColorModeValue("gray.900", "white");
  return (
    <Box
      as="button"
      px={4}
      py={1.5}
      fontSize="sm"
      fontWeight="600"
      borderRadius="md"
      bg={active ? bg : "transparent"}
      color={textColor}
      opacity={active ? 1 : 0.5}
      boxShadow={active ? "0 1px 4px rgba(0,0,0,0.08)" : "none"}
      onClick={onClick}
      transition="all 0.2s ease"
      _hover={{ opacity: 1 }}
    >
      {label}
    </Box>
  );
}

function SettingRow({
  label,
  desc,
  children,
  labelColor,
  descColor,
}: {
  label: string;
  desc: string;
  children: React.ReactNode;
  labelColor: string;
  descColor: string;
}) {
  return (
    <HStack justify="space-between" w="full" py={1.5}>
      <Box>
        <Text fontWeight="600" color={labelColor} fontSize="sm">
          {label}
        </Text>
        <Text fontSize="xs" color={descColor} mt={0.5}>
          {desc}
        </Text>
      </Box>
      {children}
    </HStack>
  );
}
