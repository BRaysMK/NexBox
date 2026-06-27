import {
  Box,
  Text,
  Heading,
  VStack,
  HStack,
  SimpleGrid,
  Slider,
  SliderTrack,
  SliderFilledTrack,
  SliderThumb,
  useColorModeValue,
  useColorMode,
  useToast,
  Badge,
  IconButton,
  Button,
  Menu,
  MenuButton,
  MenuList,
  MenuItem,
  Portal,
  Switch,
} from "@chakra-ui/react";
import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LazyStore } from "@tauri-apps/plugin-store";
import { useTranslation } from "react-i18next";
import { ArrowLeft, Monitor, Video, Square, Play, Pause, ChevronDown, Check, FolderOpen } from "lucide-react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import { useNavigate } from "react-router-dom";

// ─── Types ───

interface DisplayInfo {
  index: number;
  name: string;
  device_name: string;
  is_primary: boolean;
  width: number;
  height: number;
}

interface WindowInfo {
  hwnd: number;
  title: string;
  class_name: string;
  exe_name: string;
  visible: boolean;
  width: number;
  height: number;
}

interface RecordingSettings {
  mode: "fullscreen" | "window";
  displayIndex: number;
  windowHwnd: number;
  outputWidth: number;
  outputHeight: number;
  fps: number;
  format: string;
  quality: number;
  outputPath: string;
  captureCursor: boolean;
}

interface RecordingState {
  is_recording: boolean;
  is_paused: boolean;
  duration_secs: number;
  file_path: string | null;
  file_size: number;
  current_fps: number;
  frames_captured: number;
  frames_dropped: number;
}

// ─── Constants ───

const STORE_KEY = "screen-record-settings";
const store = new LazyStore("settings.json");

const DEFAULT_SETTINGS: RecordingSettings = {
  mode: "fullscreen",
  displayIndex: 0,
  windowHwnd: 0,
  outputWidth: 1920,
  outputHeight: 1080,
  fps: 30,
  format: "mp4",
  quality: 75,
  outputPath: "",
  captureCursor: true,
};

const FPS_OPTIONS = [
  { value: 15, label: "15 FPS" },
  { value: 24, label: "24 FPS" },
  { value: 30, label: "30 FPS" },
  { value: 60, label: "60 FPS" },
  { value: 120, label: "120 FPS" },
];

const RESOLUTION_OPTIONS = [
  { value: "original", width: 0, height: 0, labelKey: "screenRecord.resolution.original" },
  { value: "4k", width: 3840, height: 2160, label: "4K (3840×2160)" },
  { value: "1440p", width: 2560, height: 1440, label: "2K (2560×1440)" },
  { value: "1080p", width: 1920, height: 1080, label: "1080p (1920×1080)" },
  { value: "720p", width: 1280, height: 720, label: "720p (1280×720)" },
];

// ─── Components ───

function SettingCard({ title, children }: { title: string; children: React.ReactNode }) {
  const { liquidGlassEnabled } = useBackground();
  const cardBg = useColorModeValue("white", "#111111");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const { colorMode } = useColorMode();
  const headerColor = colorMode === "light" ? "#000000" : "#ffffff";

  if (liquidGlassEnabled) {
    return (
      <LiquidGlassCard p={5}>
        <VStack align="stretch" spacing={4}>
          <Text fontWeight="medium" color="white">
            {title}
          </Text>
          {children}
        </VStack>
      </LiquidGlassCard>
    );
  }

  return (
    <Box bg={cardBg} borderRadius="xl" p={5} border="1px solid" borderColor={borderColor}>
      <VStack align="stretch" spacing={4}>
        <Text fontWeight="medium" color={headerColor}>
          {title}
        </Text>
        {children}
      </VStack>
    </Box>
  );
}

// ─── Main Page ───

export default function ScreenRecordPage() {
  const { t } = useTranslation();
  const toast = useToast();
  const navigate = useNavigate();
  const { getActiveColor, getHoverColor, getContrastTextColor } = useThemeColor();
  const { liquidGlassEnabled } = useBackground();

  const [settings, setSettings] = useState<RecordingSettings>(DEFAULT_SETTINGS);
  const [displays, setDisplays] = useState<DisplayInfo[]>([]);
  const [windows, setWindows] = useState<WindowInfo[]>([]);
  const [recState, setRecState] = useState<RecordingState>({
    is_recording: false,
    is_paused: false,
    duration_secs: 0,
    file_path: null,
    file_size: 0,
    current_fps: 0,
    frames_captured: 0,
    frames_dropped: 0,
  });
  const [isLoading, setIsLoading] = useState(false);
  const statusTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const headingColor = useColorModeValue("black", "#ffffff");
  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const sliderBg = useColorModeValue("gray.200", "gray.600");
  const hoverBg = useColorModeValue("gray.100", "#252525");
  const menuListBg = useColorModeValue("white", "#1a1a1a");

  // ─── Load initial data ───

  useEffect(() => {
    loadSettings();
    loadDisplays();
    loadWindows();
  }, []);

  // ─── Listen for recording events ───

  useEffect(() => {
    const unlisteners: (() => void)[] = [];

    listen<string>("screen-record-complete", (event) => {
      const path = event.payload;
      toast({
        title: `${t("screenRecord.saveSuccess")} ${path}`,
        status: "success",
        duration: 5000,
        isClosable: true,
      });
      setRecState((prev) => ({
        ...prev,
        is_recording: false,
        is_paused: false,
        file_path: path,
      }));
    }).then((fn) => unlisteners.push(fn));

    listen<string>("screen-record-error", (event) => {
      toast({
        title: t("screenRecord.saveFailed"),
        description: event.payload,
        status: "error",
        duration: 5000,
        isClosable: true,
      });
      setRecState((prev) => ({
        ...prev,
        is_recording: false,
        is_paused: false,
      }));
    }).then((fn) => unlisteners.push(fn));

    return () => {
      unlisteners.forEach((fn) => fn());
    };
  }, [toast, t]);

  // ─── Status polling during recording ───

  useEffect(() => {
    if (recState.is_recording) {
      statusTimerRef.current = setInterval(async () => {
        try {
          const status = await invoke<RecordingState>("get_screen_recording_status");
          setRecState(status);
        } catch {
          // ignore polling errors
        }
      }, 500);
    } else {
      if (statusTimerRef.current) {
        clearInterval(statusTimerRef.current);
        statusTimerRef.current = null;
      }
    }

    return () => {
      if (statusTimerRef.current) {
        clearInterval(statusTimerRef.current);
      }
    };
  }, [recState.is_recording]);

  // ─── Data loading ───

  const loadSettings = async () => {
    try {
      const saved = await store.get<RecordingSettings>(STORE_KEY);
      if (saved) {
        setSettings((prev) => ({ ...prev, ...saved }));
        return;
      }
    } catch {
      // ignore
    }

    // Set default output path
    try {
      const homePath = await invoke<string>("get_recordings_folder");
      setSettings((prev) => ({
        ...prev,
        outputPath: `${homePath}\\recording_${Date.now()}.avi`,
      }));
    } catch {
      // Fallback: Desktop
      setSettings((prev) => ({
        ...prev,
        outputPath: `C:\\Users\\Public\\Videos\\NexBox\\recording_${Date.now()}.avi`,
      }));
    }
  };

  const loadDisplays = async () => {
    try {
      const list = await invoke<DisplayInfo[]>("enumerate_screen_record_displays");
      setDisplays(list);

      // Auto-select primary display as target resolution
      const primary = list.find((d) => d.is_primary);
      if (primary) {
        setSettings((prev) => ({
          ...prev,
          outputWidth: prev.outputWidth || primary.width,
          outputHeight: prev.outputHeight || primary.height,
        }));
      }
    } catch (error) {
      console.error("Failed to load displays:", error);
    }
  };

  const loadWindows = async () => {
    try {
      const list = await invoke<WindowInfo[]>("enumerate_screen_record_windows");
      setWindows(list);
    } catch (error) {
      console.error("Failed to load windows:", error);
    }
  };

  // ─── Actions ───

  const updateSettings = async (newSettings: RecordingSettings) => {
    setSettings(newSettings);
    try {
      await store.set(STORE_KEY, newSettings);
      await store.save();
    } catch (error) {
      console.error("Failed to save settings:", error);
    }
  };

  const updateSetting = <K extends keyof RecordingSettings>(key: K, value: RecordingSettings[K]) => {
    updateSettings({ ...settings, [key]: value });
  };

  const selectOutputPath = async () => {
    try {
      const selected = await invoke<string>("pick_recording_save_path", {
        defaultName: `recording_${Date.now()}.avi`,
      });
      if (selected) {
        updateSetting("outputPath", selected);
      }
    } catch (error) {
      console.error("Failed to pick save path:", error);
    }
  };

  const startRecording = async () => {
    if (!settings.outputPath) {
      toast({
        title: t("screenRecord.selectWindow") || "Please select an output path",
        status: "warning",
        duration: 3000,
        isClosable: true,
      });
      return;
    }

    if (settings.mode === "window" && settings.windowHwnd === 0) {
      toast({
        title: t("screenRecord.selectWindow"),
        status: "warning",
        duration: 3000,
        isClosable: true,
      });
      return;
    }

    setIsLoading(true);
    try {
      // Generate output path with timestamp if using default
      let finalPath = settings.outputPath;
      if (finalPath.includes("recording_")) {
        const dir = finalPath.replace(/recording_\d+\.avi$/, "");
        finalPath = `${dir}recording_${Date.now()}.avi`;
      }

      await invoke("start_screen_recording", {
        config: {
          mode: settings.mode,
          display_index: settings.displayIndex,
          window_hwnd: settings.windowHwnd,
          output_width: settings.outputWidth,
          output_height: settings.outputHeight,
          fps: settings.fps,
          format: settings.format,
          quality: settings.quality,
          output_path: finalPath,
          capture_cursor: settings.captureCursor,
        },
      });

      setRecState({
        is_recording: true,
        is_paused: false,
        duration_secs: 0,
        file_path: finalPath,
        file_size: 0,
        current_fps: 0,
        frames_captured: 0,
        frames_dropped: 0,
      });

      toast({
        title: t("screenRecord.recording"),
        status: "info",
        duration: 2000,
        isClosable: true,
      });
    } catch (error) {
      console.error("Failed to start recording:", error);
      toast({
        title: t("screenRecord.saveFailed"),
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    }
    setIsLoading(false);
  };

  const pauseRecording = async () => {
    try {
      if (recState.is_paused) {
        await invoke("resume_screen_recording");
        setRecState((prev) => ({ ...prev, is_paused: false }));
      } else {
        await invoke("pause_screen_recording");
        setRecState((prev) => ({ ...prev, is_paused: true }));
      }
    } catch (error) {
      console.error("Failed to pause/resume:", error);
    }
  };

  const stopRecording = async () => {
    try {
      await invoke("stop_screen_recording");
      setRecState((prev) => ({ ...prev, is_recording: false, is_paused: false }));
    } catch (error) {
      console.error("Failed to stop recording:", error);
    }
  };

  const setResolution = (resolution: (typeof RESOLUTION_OPTIONS)[0]) => {
    if (resolution.width === 0) {
      // Original: use selected display resolution
      const display = displays.find(
        (d) => (settings.mode === "fullscreen" && d.index === settings.displayIndex) || d.is_primary
      );
      if (display) {
        updateSettings({
          ...settings,
          outputWidth: display.width,
          outputHeight: display.height,
        });
      }
    } else {
      updateSettings({
        ...settings,
        outputWidth: resolution.width,
        outputHeight: resolution.height,
      });
    }
  };

  const getSelectedResolutionLabel = () => {
    const found = RESOLUTION_OPTIONS.find(
      (r) => r.width === settings.outputWidth && r.height === settings.outputHeight
    );
    if (found) return found.label || t(found.labelKey);
    return `${settings.outputWidth}×${settings.outputHeight}`;
  };

  // ─── Format helpers ───

  const formatDuration = (secs: number) => {
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = Math.floor(secs % 60);
    if (h > 0) return `${h.toString().padStart(2, "0")}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
    return `${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
  };

  const formatFileSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  };

  // ─── Derived state ───

  const isRecording = recState.is_recording;
  const isPaused = recState.is_paused;

  // ─── Render ───

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
            isDisabled={isRecording}
          />
          <Heading size="lg" color={headingColor}>
            <HStack spacing={2}>
              <Video size={24} />
              <Text>{t("screenRecord.title")}</Text>
            </HStack>
          </Heading>
        </HStack>
      </HStack>

      <SimpleGrid columns={2} spacing={5}>
        {/* Left Column: Settings */}
        <VStack align="stretch" spacing={5}>
          {/* Recording Mode */}
          <SettingCard title={t("screenRecord.mode")}>
            <HStack spacing={3}>
              <LiquidGlassCard
                py={2.5}
                px={4}
                textAlign="center"
                cursor="pointer"
                flex={1}
                onClick={() => !isRecording && updateSetting("mode", "fullscreen")}
                opacity={settings.mode === "fullscreen" || isRecording ? (isRecording ? 0.7 : 1) : 0.5}
                border={settings.mode === "fullscreen" ? `1px solid ${getActiveColor()}` : "1px solid transparent"}
              >
                <Text
                  fontSize="sm"
                  fontWeight="medium"
                  color={settings.mode === "fullscreen" ? getActiveColor() : textColor}
                >
                  {t("screenRecord.mode.fullscreen")}
                </Text>
              </LiquidGlassCard>
              <LiquidGlassCard
                py={2.5}
                px={4}
                textAlign="center"
                cursor="pointer"
                flex={1}
                onClick={() => !isRecording && updateSetting("mode", "window")}
                opacity={settings.mode === "window" || isRecording ? (isRecording ? 0.7 : 1) : 0.5}
                border={settings.mode === "window" ? `1px solid ${getActiveColor()}` : "1px solid transparent"}
              >
                <Text
                  fontSize="sm"
                  fontWeight="medium"
                  color={settings.mode === "window" ? getActiveColor() : textColor}
                >
                  {t("screenRecord.mode.window")}
                </Text>
              </LiquidGlassCard>
            </HStack>
          </SettingCard>

          {/* Target Selection (Display or Window) */}
          <SettingCard
            title={settings.mode === "fullscreen" ? t("screenRecord.targetDisplay") : t("screenRecord.targetWindow")}
          >
            {settings.mode === "fullscreen" ? (
              <Menu matchWidth>
                <MenuButton as={Box} bg="transparent" p={0} border="none" w="full" cursor="pointer" disabled={isRecording}>
                  <LiquidGlassCard px={3} py={1.5}>
                    <HStack justify="space-between">
                      <HStack spacing={2}>
                        <Monitor size={14} />
                        <Text fontSize="sm" color={textColor}>
                          {displays.length === 0
                            ? t("screenRecord.noDisplays")
                            : displays.find((d) => d.index === settings.displayIndex)?.name ||
                              t("screenRecord.primaryMonitor")}
                        </Text>
                      </HStack>
                      <ChevronDown size={16} />
                    </HStack>
                  </LiquidGlassCard>
                </MenuButton>
                <Portal>
                  <MenuList bg={menuListBg} borderColor={cardBorder} maxH="300px" overflowY="auto" zIndex={9999}>
                    {displays.map((d) => (
                      <MenuItem
                        key={d.index}
                        onClick={() => {
                          updateSettings({
                            ...settings,
                            displayIndex: d.index,
                            outputWidth: d.width,
                            outputHeight: d.height,
                          });
                        }}
                        bg={settings.displayIndex === d.index ? hoverBg : "transparent"}
                        _hover={{ bg: hoverBg }}
                      >
                        <HStack spacing={2} w="full" justify="space-between">
                          <Text fontSize="sm">{d.name}</Text>
                          {settings.displayIndex === d.index && <Check size={14} color={getActiveColor()} />}
                        </HStack>
                      </MenuItem>
                    ))}
                    {displays.length === 0 && (
                      <MenuItem isDisabled>
                        <Text fontSize="sm" color={subTextColor}>
                          {t("screenRecord.noDisplays")}
                        </Text>
                      </MenuItem>
                    )}
                  </MenuList>
                </Portal>
              </Menu>
            ) : (
              <Menu matchWidth>
                <MenuButton as={Box} bg="transparent" p={0} border="none" w="full" cursor="pointer" disabled={isRecording}>
                  <LiquidGlassCard px={3} py={1.5}>
                    <HStack justify="space-between">
                      <HStack spacing={2}>
                        <Monitor size={14} />
                        <Text fontSize="sm" color={textColor} noOfLines={1}>
                          {settings.windowHwnd === 0
                            ? t("screenRecord.selectWindow")
                            : windows.find((w) => w.hwnd === settings.windowHwnd)?.title ||
                              t("screenRecord.noWindow")}
                        </Text>
                      </HStack>
                      <ChevronDown size={16} />
                    </HStack>
                  </LiquidGlassCard>
                </MenuButton>
                <Portal>
                  <MenuList bg={menuListBg} borderColor={cardBorder} maxH="300px" overflowY="auto" zIndex={9999} minW="400px">
                    <MenuItem
                      onClick={() => updateSetting("windowHwnd", 0)}
                      bg="transparent"
                      _hover={{ bg: hoverBg }}
                    >
                      <Text fontSize="sm" color={subTextColor}>
                        {t("screenRecord.selectWindow")}
                      </Text>
                    </MenuItem>
                    {windows.map((w) => (
                      <MenuItem
                        key={w.hwnd}
                        onClick={() => {
                          updateSettings({
                            ...settings,
                            windowHwnd: w.hwnd,
                            outputWidth: w.width,
                            outputHeight: w.height,
                          });
                        }}
                        bg={settings.windowHwnd === w.hwnd ? hoverBg : "transparent"}
                        _hover={{ bg: hoverBg }}
                      >
                        <VStack align="stretch" spacing={0} w="full">
                          <HStack spacing={2} w="full" justify="space-between">
                            <Text fontSize="sm" fontWeight="medium" noOfLines={1} maxW="300px">
                              {w.title}
                            </Text>
                            {settings.windowHwnd === w.hwnd && <Check size={14} color={getActiveColor()} />}
                          </HStack>
                          <Text fontSize="xs" color={subTextColor}>
                            {w.exe_name ? w.exe_name.split("\\").pop() : ""} - {w.width}×{w.height}
                          </Text>
                        </VStack>
                      </MenuItem>
                    ))}
                    {windows.length === 0 && (
                      <MenuItem isDisabled>
                        <Text fontSize="sm" color={subTextColor}>
                          {t("screenRecord.selectWindow")}
                        </Text>
                      </MenuItem>
                    )}
                  </MenuList>
                </Portal>
              </Menu>
            )}
          </SettingCard>

          {/* Resolution */}
          <SettingCard title={t("screenRecord.resolution")}>
            <Menu matchWidth>
              <MenuButton as={Box} bg="transparent" p={0} border="none" w="full" cursor="pointer" disabled={isRecording}>
                <LiquidGlassCard px={3} py={1.5}>
                  <HStack justify="space-between">
                    <Text fontSize="sm" color={textColor}>
                      {getSelectedResolutionLabel()}
                    </Text>
                    <ChevronDown size={16} />
                  </HStack>
                </LiquidGlassCard>
              </MenuButton>
              <Portal>
                <MenuList bg={menuListBg} borderColor={cardBorder} zIndex={9999}>
                  {RESOLUTION_OPTIONS.map((res) => (
                    <MenuItem
                      key={res.value}
                      onClick={() => setResolution(res)}
                      bg={
                        (res.width === settings.outputWidth && res.height === settings.outputHeight) ||
                        (res.width === 0 && settings.outputWidth === (displays.find((d) => d.is_primary)?.width || 0))
                          ? hoverBg
                          : "transparent"
                      }
                      _hover={{ bg: hoverBg }}
                    >
                      <HStack spacing={2} w="full" justify="space-between">
                        <Text fontSize="sm">{res.label || t(res.labelKey)}</Text>
                        {res.width === settings.outputWidth && res.height === settings.outputHeight && (
                          <Check size={14} color={getActiveColor()} />
                        )}
                      </HStack>
                    </MenuItem>
                  ))}
                </MenuList>
              </Portal>
            </Menu>
          </SettingCard>

          {/* FPS */}
          <SettingCard title={t("screenRecord.fps")}>
            <VStack align="stretch" spacing={2}>
              <HStack justify="space-between">
                <Text fontSize="sm" color={subTextColor}>
                  {t("screenRecord.fps")}
                </Text>
                <Text fontSize="sm" fontWeight="bold" color={getActiveColor()}>
                  {settings.fps} FPS
                </Text>
              </HStack>
              <HStack spacing={2}>
                {FPS_OPTIONS.map((opt) => (
                  <LiquidGlassCard
                    key={opt.value}
                    py={1.5}
                    px={3}
                    textAlign="center"
                    cursor={isRecording ? "not-allowed" : "pointer"}
                    flex={1}
                    onClick={() => !isRecording && updateSetting("fps", opt.value)}
                    opacity={settings.fps === opt.value ? 1 : 0.5}
                    border={settings.fps === opt.value ? `1px solid ${getActiveColor()}` : "1px solid transparent"}
                  >
                    <Text
                      fontSize="xs"
                      fontWeight="bold"
                      color={settings.fps === opt.value ? getActiveColor() : textColor}
                    >
                      {opt.label}
                    </Text>
                  </LiquidGlassCard>
                ))}
              </HStack>
            </VStack>
          </SettingCard>

          {/* Quality */}
          <SettingCard title={t("screenRecord.quality")}>
            <Box>
              <HStack justify="space-between" mb={1}>
                <Text color={textColor} fontSize="sm">
                  {t("screenRecord.quality")}
                </Text>
                <Text color={getActiveColor()} fontSize="sm" fontWeight="bold">
                  {settings.quality}%
                </Text>
              </HStack>
              <Slider
                value={settings.quality}
                min={10}
                max={100}
                step={5}
                onChange={(val) => updateSetting("quality", val)}
                isDisabled={isRecording}
              >
                <SliderTrack bg={sliderBg}>
                  <SliderFilledTrack bg={getActiveColor()} />
                </SliderTrack>
                <SliderThumb />
              </Slider>
            </Box>
          </SettingCard>

          {/* Capture Cursor */}
          <SettingCard title={t("screenRecord.captureCursor")}>
            <HStack justify="space-between">
              <Text fontSize="sm" color={textColor}>
                {t("screenRecord.captureCursor")}
              </Text>
              <Switch
                isChecked={settings.captureCursor}
                onChange={(e) => updateSetting("captureCursor", e.target.checked)}
                isDisabled={isRecording}
                size="lg"
                sx={{
                  "& .chakra-switch__track[data-checked]": {
                    bg: getActiveColor(),
                  },
                }}
              />
            </HStack>
          </SettingCard>

          {/* Save Path */}
          <SettingCard title={t("screenRecord.savePath")}>
            <VStack align="stretch" spacing={2}>
              <Button
                leftIcon={<FolderOpen size={16} />}
                variant="outline"
                colorScheme="gray"
                size="sm"
                onClick={selectOutputPath}
                isDisabled={isRecording}
                justifyContent="flex-start"
                h="auto"
                py={2.5}
                whiteSpace="normal"
                textAlign="left"
              >
                <VStack align="stretch" spacing={0.5}>
                  <Text fontSize="sm" color={textColor}>
                    {settings.outputPath || t("screenRecord.browse")}
                  </Text>
                  {settings.outputPath && (
                    <Text fontSize="2xs" color={subTextColor} noOfLines={1}>
                      {settings.outputPath}
                    </Text>
                  )}
                </VStack>
              </Button>
            </VStack>
          </SettingCard>
        </VStack>

        {/* Right Column: Status & Controls */}
        <VStack align="stretch" spacing={5}>
          {/* Recording Status Card */}
          <SettingCard title={t("screenRecord.title")}>
            <VStack align="stretch" spacing={4}>
              {/* Status Badge */}
              <HStack justify="center">
                {isRecording ? (
                  <Badge
                    colorScheme={isPaused ? "yellow" : "red"}
                    fontSize="md"
                    px={4}
                    py={1}
                    borderRadius="full"
                    display="flex"
                    alignItems="center"
                    gap={2}
                  >
                    <Box
                      w={2}
                      h={2}
                      borderRadius="full"
                      bg={isPaused ? "yellow.400" : "red.400"}
                      animation={isPaused ? "none" : "pulse 1s infinite"}
                    />
                    {isPaused ? t("screenRecord.paused") : t("screenRecord.recording")}
                  </Badge>
                ) : (
                  <Badge colorScheme="gray" fontSize="md" px={4} py={1} borderRadius="full">
                    {t("crosshair.statusDisabled")}
                  </Badge>
                )}
              </HStack>

              {/* Duration */}
              <Box textAlign="center">
                <Text fontSize="4xl" fontWeight="bold" color={isRecording ? getActiveColor() : subTextColor} fontFamily="monospace">
                  {formatDuration(recState.duration_secs)}
                </Text>
                <Text fontSize="xs" color={subTextColor}>
                  {t("screenRecord.duration")}
                </Text>
              </Box>

              {/* Stats Grid */}
              <SimpleGrid columns={2} spacing={3}>
                <Box textAlign="center" p={2} borderRadius="md" bg={hoverBg}>
                  <Text fontSize="lg" fontWeight="bold" color={textColor}>
                    {recState.current_fps.toFixed(0)}
                  </Text>
                  <Text fontSize="xs" color={subTextColor}>
                    {t("screenRecord.currentFps")}
                  </Text>
                </Box>
                <Box textAlign="center" p={2} borderRadius="md" bg={hoverBg}>
                  <Text fontSize="lg" fontWeight="bold" color={textColor}>
                    {formatFileSize(recState.file_size)}
                  </Text>
                  <Text fontSize="xs" color={subTextColor}>
                    {t("screenRecord.fileSize")}
                  </Text>
                </Box>
                <Box textAlign="center" p={2} borderRadius="md" bg={hoverBg}>
                  <Text fontSize="lg" fontWeight="bold" color={textColor}>
                    {recState.frames_captured}
                  </Text>
                  <Text fontSize="xs" color={subTextColor}>
                    Frames
                  </Text>
                </Box>
                <Box textAlign="center" p={2} borderRadius="md" bg={hoverBg}>
                  <Text fontSize="lg" fontWeight="bold" color={recState.frames_dropped > 0 ? "red.400" : textColor}>
                    {recState.frames_dropped}
                  </Text>
                  <Text fontSize="xs" color={subTextColor}>
                    Dropped
                  </Text>
                </Box>
              </SimpleGrid>
            </VStack>
          </SettingCard>

          {/* Control Buttons */}
          <SettingCard title="">
            <VStack align="stretch" spacing={3}>
              {!isRecording ? (
                <Button
                  leftIcon={<Play size={18} />}
                  colorScheme="red"
                  size="lg"
                  w="full"
                  onClick={startRecording}
                  isLoading={isLoading}
                  _hover={{ transform: "scale(1.02)" }}
                  transition="all 0.2s"
                >
                  {t("screenRecord.startRecording")}
                </Button>
              ) : (
                <>
                  <Button
                    leftIcon={isPaused ? <Play size={18} /> : <Pause size={18} />}
                    colorScheme={isPaused ? "green" : "yellow"}
                    size="lg"
                    w="full"
                    onClick={pauseRecording}
                  >
                    {isPaused ? t("screenRecord.resumeRecording") : t("screenRecord.pauseRecording")}
                  </Button>
                  <Button
                    leftIcon={<Square size={18} />}
                    colorScheme="red"
                    size="lg"
                    w="full"
                    onClick={stopRecording}
                    variant="solid"
                  >
                    {t("screenRecord.stopRecording")}
                  </Button>
                </>
              )}
            </VStack>
          </SettingCard>
        </VStack>
      </SimpleGrid>
    </Box>
  );
}
