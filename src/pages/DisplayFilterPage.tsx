import {
  Box,
  Flex,
  Heading,
  Text,
  VStack,
  HStack,
  useColorModeValue,
  Card,
  CardBody,
  useToast,
  IconButton,
  Tooltip,
  SimpleGrid,
  Button,
  Input,
  AlertDialog,
  AlertDialogBody,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogContent,
  AlertDialogOverlay,
} from "@chakra-ui/react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { ThemeSwitch } from "@/components/special/theme-switch";
import { CustomSelect } from "@/components/special/custom-select";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import { hexToRgba } from "@/lib/color-utils";
import { 
  Sun, BookOpen, Monitor, Sparkles, RotateCcw, 
  Film, Heart, Palette, Gamepad2, Save, Settings2, ArrowLeft,
  Upload, Trash2, FileImage
} from "lucide-react";
import { useState, useEffect, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useNavigate } from "react-router-dom";
import { useAppStartup } from "@/contexts/app-startup-context";
import { HotkeyRecorder } from "@/components/hotkey-recorder";

interface FilterSettings {
  temperature: number;
  brightness: number;
  contrast: number;
  saturation: number;
  mode: number;
  is_active: boolean;
}

interface FilterPreset {
  id: string;
  name: string;
  mode: number;
  temperature: number;
  brightness: number;
  contrast: number;
  saturation: number;
  description: string;
}

interface IccPresetInfo {
  id: string;
  name: string;
  description: string;
}

interface DisplayInfo {
  index: number;
  name: string;
  device_name: string;
  is_primary: boolean;
  width: number;
  height: number;
}

const presetIcons: Record<string, React.ElementType> = {
  "normal": Monitor,
  "vivid": Sparkles,
  "movie": Film,
  "highlight": Sun,
  "soft": Heart,
  "gaming": Gamepad2,
  "reading": BookOpen,
  "custom": Settings2,
};

const presetColors: Record<string, string> = {
  "normal": "#98DDD0",
  "vivid": "#FF6B9D",
  "movie": "#9B59B6",
  "highlight": "#F1C40F",
  "soft": "#E8B4B8",
  "gaming": "#00D9FF",
  "reading": "#DEB887",
  "custom": "#6B7280",
};

const modeParams: Record<number, { gamma: number; sCurve: number; rBoost: number; gBoost: number; bBoost: number }> = {
  0: { gamma: 1.0, sCurve: 0.0, rBoost: 1.0, gBoost: 1.0, bBoost: 1.0 },
  1: { gamma: 0.95, sCurve: 0.08, rBoost: 1.02, gBoost: 1.0, bBoost: 1.03 },
  2: { gamma: 1.05, sCurve: -0.05, rBoost: 1.0, gBoost: 0.98, bBoost: 0.96 },
  3: { gamma: 0.92, sCurve: 0.05, rBoost: 1.0, gBoost: 1.0, bBoost: 1.0 },
  4: { gamma: 1.08, sCurve: -0.08, rBoost: 0.98, gBoost: 1.0, bBoost: 1.02 },
  5: { gamma: 0.96, sCurve: 0.1, rBoost: 1.0, gBoost: 1.0, bBoost: 1.02 },
  6: { gamma: 1.0, sCurve: 0.0, rBoost: 1.0, gBoost: 0.99, bBoost: 0.97 },
};

export default function DisplayFilterPage() {
  const navigate = useNavigate();
  const { filterHotkey, saveFilterHotkey } = useAppStartup();
  const [settings, setSettings] = useState<FilterSettings>({
    temperature: 6500,
    brightness: 100,
    contrast: 100,
    saturation: 100,
    mode: 0,
    is_active: false,
  });
  const [presets, setPresets] = useState<FilterPreset[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [activePresetId, setActivePresetId] = useState<string>("normal");
  const [savedCustom, setSavedCustom] = useState<{
    temperature: number;
    brightness: number;
    contrast: number;
    saturation: number;
  } | null>(null);
  const [hasChanges, setHasChanges] = useState(false);
  const [inputVersion, setInputVersion] = useState(0);
  const [manualPresetChange, setManualPresetChange] = useState(false);
  const [iccPresets, setIccPresets] = useState<IccPresetInfo[]>([]);
  const [activeIccId, setActiveIccId] = useState<string | null>(null);
  const [deleteIccId, setDeleteIccId] = useState<string | null>(null);
  const cancelDeleteRef = useRef<HTMLButtonElement>(null);
  const [displays, setDisplays] = useState<DisplayInfo[]>([]);
  const [activeDisplayIndex, setActiveDisplayIndex] = useState<number>(0);
  const activeDisplayIndexRef = useRef(0);
  
  const editValuesRef = useRef({
    temperature: 6500,
    brightness: 100,
    contrast: 100,
    saturation: 100,
  });
  
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const toast = useToast();

  const { getActiveColor, getHoverColor, getContrastTextColor } = useThemeColor();
  const primaryColor = getActiveColor();
  const contrastText = getContrastTextColor();

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const cardBg = useColorModeValue("white", "#111111");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const textColor = useColorModeValue("gray.700", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const sliderBg = useColorModeValue("gray.100", "#222222");
  const infoBg = useColorModeValue("gray.50", "#1a1a1a");
  const inputBg = useColorModeValue("white", "#1a1a1a");

  const loadSettings = useCallback(async () => {
    try {
      const result: FilterSettings = await invoke("get_filter_settings", { displayIndex: activeDisplayIndexRef.current });
      setSettings(result);
    } catch (error) {
      console.error("Failed to load filter settings:", error);
    }
  }, []);

  const loadPresets = useCallback(async () => {
    try {
      const result: FilterPreset[] = await invoke("get_filter_presets");
      setPresets(result);
    } catch (error) {
      console.error("Failed to load presets:", error);
    }
  }, []);

  const loadCustomSettings = useCallback(async () => {
    try {
      const result = await invoke<{ temperature: number; brightness: number; contrast: number; saturation: number }>("get_custom_filter_settings", { displayIndex: activeDisplayIndexRef.current });
      editValuesRef.current = {
        temperature: result.temperature,
        brightness: result.brightness,
        contrast: result.contrast,
        saturation: result.saturation,
      };
      setSavedCustom(result);
    } catch (error) {
      console.error("Failed to load custom settings:", error);
      const defaults = {
        temperature: 6500,
        brightness: 100,
        contrast: 100,
        saturation: 100,
      };
      editValuesRef.current = defaults;
      setSavedCustom(defaults);
    }
  }, []);

  const loadIccPresets = useCallback(async () => {
    try {
      const result: IccPresetInfo[] = await invoke("get_icc_presets");
      setIccPresets(result);
    } catch (error) {
      console.error("Failed to load ICC presets:", error);
    }
  }, []);

  const loadDisplays = useCallback(async () => {
    try {
      const result: DisplayInfo[] = await invoke("get_displays");
      if (result.length > 0) {
        setDisplays(result);
        const idx = result[0].index;
        setActiveDisplayIndex(idx);
        activeDisplayIndexRef.current = idx;
        await invoke("set_active_display", { displayIndex: idx });
      } else {
        setDisplays([{ index: 0, name: "DISPLAY1", device_name: "DISPLAY1", is_primary: true, width: 0, height: 0 }]);
      }
    } catch (error) {
      console.error("Failed to load displays:", error);
      setDisplays([{ index: 0, name: "DISPLAY1", device_name: "DISPLAY1", is_primary: true, width: 0, height: 0 }]);
    }
  }, []);

  useEffect(() => {
    activeDisplayIndexRef.current = activeDisplayIndex;
  }, [activeDisplayIndex]);

  useEffect(() => {
    loadDisplays();
    loadSettings();
    loadPresets();
    loadCustomSettings();
    loadIccPresets();
  }, [loadDisplays, loadSettings, loadPresets, loadCustomSettings, loadIccPresets]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    listen<void>("filter-status-changed", () => {
      loadSettings();
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      if (unlisten) unlisten();
    };
  }, [loadSettings]);

  useEffect(() => {
    if (presets.length === 0 || savedCustom === null) return;
    if (manualPresetChange) return;
    if (activePresetId === "custom") return;
    if (activePresetId === "") return; // ICC 预设选中时跳过同步

    const exactPreset = presets.find(
      (p) =>
        p.mode === settings.mode &&
        p.temperature === settings.temperature &&
        p.brightness === settings.brightness &&
        p.contrast === settings.contrast &&
        p.saturation === settings.saturation
    );

    const matchesSavedCustom =
      settings.mode === 0 &&
      settings.temperature === savedCustom.temperature &&
      settings.brightness === savedCustom.brightness &&
      settings.contrast === savedCustom.contrast &&
      settings.saturation === savedCustom.saturation;

    let nextId: string;
    if (matchesSavedCustom) {
      nextId = exactPreset?.id === "normal" ? "normal" : "custom";
    } else if (exactPreset) {
      nextId = exactPreset.id;
    } else {
      const modePreset = presets.find((p) => p.mode === settings.mode);
      nextId = modePreset?.id ?? "normal";
    }

    setActivePresetId((prev) => (prev === nextId ? prev : nextId));
  }, [presets, settings, savedCustom, activePresetId, manualPresetChange]);

  const toggleFilter = async () => {
    setIsLoading(true);
    try {
      const result: any = await invoke("toggle_filter", { displayIndex: activeDisplayIndex });
      if (result.success) {
        setSettings(prev => ({
          ...prev,
          is_active: result.settings.is_active,
        }));
        toast({
          title: result.settings.is_active 
            ? t("displayFilter.filterEnabled") 
            : t("displayFilter.filterDisabled"),
          status: "success",
          duration: 2000,
          isClosable: true,
        });
      }
    } catch (error) {
      toast({
        title: t("displayFilter.error"),
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    } finally {
      setIsLoading(false);
    }
  };

  const applyPreset = async (preset: FilterPreset) => {
    setIsLoading(true);
    setManualPresetChange(true);
    setActivePresetId(preset.id);
    setActiveIccId(null);
    setHasChanges(false);
    setInputVersion(v => v + 1);
    
    try {
      const result: any = await invoke("apply_preset", {
        displayIndex: activeDisplayIndex,
        presetId: preset.id,
      });
      if (result.success) {
        setSettings({
          temperature: preset.temperature,
          brightness: preset.brightness,
          contrast: preset.contrast,
          saturation: preset.saturation,
          mode: preset.mode,
          is_active: true,
        });
        toast({
          title: `${t("displayFilter.presetAppliedPrefix")}${preset.name}${t("displayFilter.presetAppliedSuffix")}`,
          status: "success",
          duration: 2000,
          isClosable: true,
        });
      }
    } catch (error) {
      toast({
        title: t("displayFilter.error"),
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    } finally {
      setIsLoading(false);
      // 延迟重置标志，让 useEffect 跳过一次自动同步
      setTimeout(() => setManualPresetChange(false), 100);
    }
  };

  const openCustom = async () => {
    setManualPresetChange(true);
    setActivePresetId("custom");
    setIsLoading(true);
    // 不要用当前已应用的 settings 覆盖：切换其它预设后 settings 会变成该预设参数，
    // 会冲掉未保存的编辑或磁盘上的自定义档案。editValuesRef 由 loadCustomSettings / 保存 / 用户输入维护。
    setInputVersion((v) => v + 1);
    if (savedCustom) {
      const r = editValuesRef.current;
      setHasChanges(
        r.temperature !== savedCustom.temperature ||
          r.brightness !== savedCustom.brightness ||
          r.contrast !== savedCustom.contrast ||
          r.saturation !== savedCustom.saturation
      );
      
      // 应用已保存的自定义滤镜设置
      try {
        const result: any = await invoke("set_filter_settings", {
          displayIndex: activeDisplayIndex,
          temperature: savedCustom.temperature,
          brightness: savedCustom.brightness,
          contrast: savedCustom.contrast,
          saturation: savedCustom.saturation,
          mode: 0,
        });
        if (result.success) {
          setSettings({
            temperature: savedCustom.temperature,
            brightness: savedCustom.brightness,
            contrast: savedCustom.contrast,
            saturation: savedCustom.saturation,
            mode: 0,
            is_active: true,
          });
        }
      } catch (error) {
        console.error("Failed to apply custom settings:", error);
      }
    } else {
      setHasChanges(false);
    }
    setIsLoading(false);
    setTimeout(() => setManualPresetChange(false), 100);
  };

  const saveAndApply = async () => {
    setIsLoading(true);
    setManualPresetChange(true);
    setActivePresetId("custom");
    
    const temp = Math.max(1000, Math.min(10000, editValuesRef.current.temperature));
    const brightness = Math.max(50, Math.min(150, editValuesRef.current.brightness));
    const contrast = Math.max(50, Math.min(150, editValuesRef.current.contrast));
    const saturation = Math.max(50, Math.min(150, editValuesRef.current.saturation));
    
    try {
      const result: any = await invoke("set_filter_settings", {
        displayIndex: activeDisplayIndex,
        temperature: temp,
        brightness: brightness,
        contrast: contrast,
        saturation: saturation,
        mode: 0,
      });
      if (result.success) {
        setSettings({
          temperature: temp,
          brightness: brightness,
          contrast: contrast,
          saturation: saturation,
          mode: 0,
          is_active: true,
        });
        
        await invoke("save_custom_filter_settings", {
          displayIndex: activeDisplayIndex,
          temperature: temp,
          brightness: brightness,
          contrast: contrast,
          saturation: saturation,
        });

        setSavedCustom({
          temperature: temp,
          brightness: brightness,
          contrast: contrast,
          saturation: saturation,
        });
        
        setHasChanges(false);
        setInputVersion(v => v + 1);
        
        toast({
          title: t("displayFilter.saveSuccess"),
          status: "success",
          duration: 2000,
          isClosable: true,
        });
      }
    } catch (error) {
      toast({
        title: t("displayFilter.error"),
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    } finally {
      setIsLoading(false);
      setTimeout(() => setManualPresetChange(false), 100);
    }
  };

  const resetToDefault = async () => {
    setManualPresetChange(true);
    setActivePresetId("normal");
    setActiveIccId(null);
    try {
      const result: any = await invoke("apply_preset", { displayIndex: activeDisplayIndex, presetId: "normal" });
      if (result.success) {
        setSettings({
          temperature: 6500,
          brightness: 100,
          contrast: 100,
          saturation: 100,
          mode: 0,
          is_active: true,
        });
        const normal = {
          temperature: 6500,
          brightness: 100,
          contrast: 100,
          saturation: 100,
        };
        editValuesRef.current = normal;
        if (savedCustom) {
          setHasChanges(
            normal.temperature !== savedCustom.temperature ||
              normal.brightness !== savedCustom.brightness ||
              normal.contrast !== savedCustom.contrast ||
              normal.saturation !== savedCustom.saturation
          );
        } else {
          setHasChanges(false);
        }
        setInputVersion((v) => v + 1);
        toast({
          title: t("displayFilter.resetSuccess"),
          status: "success",
          duration: 2000,
          isClosable: true,
        });
      }
    } catch (error) {
      toast({
        title: t("displayFilter.error"),
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    } finally {
      setTimeout(() => setManualPresetChange(false), 100);
    }
  };

  const handleImportIcc = async () => {
    setIsLoading(true);
    try {
      const filePath: string | null = await invoke("select_icc_file");
      if (!filePath) {
        setIsLoading(false);
        return;
      }

      const result: any = await invoke("import_icc_profile", { path: filePath });
      if (result.success && result.preset) {
        setIccPresets(prev => [...prev, result.preset]);
        toast({
          title: t("displayFilter.importIccSuccess"),
          status: "success",
          duration: 2000,
          isClosable: true,
        });
      }
    } catch (error) {
      toast({
        title: t("displayFilter.importIccFailed"),
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    } finally {
      setIsLoading(false);
    }
  };

  const handleApplyIcc = async (id: string) => {
    setIsLoading(true);
    setActiveIccId(id);
    setActivePresetId(""); // 取消内置预设选中状态
    setManualPresetChange(true);
    try {
      const result: any = await invoke("apply_icc_preset", { displayIndex: activeDisplayIndex, id });
      if (result.success) {
        setSettings((prev: FilterSettings) => ({
          ...prev,
          is_active: true,
        }));
        toast({
          title: t("displayFilter.iccApplied"),
          status: "success",
          duration: 2000,
          isClosable: true,
        });
      }
    } catch (error) {
      setActiveIccId(null);
      toast({
        title: t("displayFilter.error"),
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    } finally {
      setIsLoading(false);
      setTimeout(() => setManualPresetChange(false), 100);
    }
  };

  const handleDeleteIcc = async () => {
    if (!deleteIccId) return;
    setIsLoading(true);
    try {
      const result: any = await invoke("delete_icc_preset", { id: deleteIccId });
      if (result.success) {
        setIccPresets(prev => prev.filter(p => p.id !== deleteIccId));
        if (activeIccId === deleteIccId) {
          setActiveIccId(null);
        }
        toast({
          title: t("displayFilter.deleteIccSuccess"),
          status: "success",
          duration: 2000,
          isClosable: true,
        });
      }
    } catch (error) {
      toast({
        title: t("displayFilter.error"),
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    } finally {
      setDeleteIccId(null);
      setIsLoading(false);
    }
  };

  const getTemperatureColor = (temp: number): string => {
    if (temp >= 7000) return "#e0f0ff";
    if (temp >= 6000) return "#ffffff";
    if (temp >= 5000) return "#fff4e0";
    if (temp >= 4000) return "#ffe8c0";
    if (temp >= 3000) return "#ffd080";
    return "#ffb040";
  };

  const currentModeParams = modeParams[settings.mode] || modeParams[0];

  const handleInputChange = (key: keyof typeof editValuesRef.current, value: string) => {
    const numValue = parseInt(value) || 0;
    editValuesRef.current[key] = numValue;
    setHasChanges(true);
  };

  const handleDisplayChange = async (value: string) => {
    const idx = parseInt(value);
    setActiveDisplayIndex(idx);
    activeDisplayIndexRef.current = idx;
    await invoke("set_active_display", { displayIndex: idx });
    loadSettings();
    loadCustomSettings();
  };

  const EditableItem = ({ 
    label, 
    value, 
    onChange, 
    unit, 
    colorValue 
  }: { 
    label: string; 
    value: number; 
    onChange: (val: string) => void;
    unit: string;
    colorValue?: number;
  }) => (
    <HStack justify="space-between" py={2} px={3} borderRadius="lg" bg={infoBg}>
      <HStack spacing={2}>
        <Text color={subTextColor} fontSize="sm">
          {label}
        </Text>
        {colorValue !== undefined && (
          <Box 
            w={3} 
            h={3} 
            borderRadius="full" 
            bg={getTemperatureColor(colorValue)}
            border="1px solid"
            borderColor={cardBorder}
          />
        )}
      </HStack>
      <HStack spacing={2}>
        <Input
          key={`${label}-${inputVersion}`}
          defaultValue={value}
          onChange={(e) => onChange(e.target.value)}
          size="xs"
          w="70px"
          h="24px"
          textAlign="right"
          fontWeight="600"
          color={textColor}
          bg={inputBg}
          borderColor={cardBorder}
          borderRadius="md"
          px={2}
          _focus={{ borderColor: primaryColor, boxShadow: "none" }}
        />
        <Text color={subTextColor} fontSize="sm" minW="20px">
          {unit}
        </Text>
      </HStack>
    </HStack>
  );

  const ReadOnlyItem = ({ label, value, unit = "", colorValue }: { 
    label: string; 
    value: string | number; 
    unit?: string;
    colorValue?: number;
  }) => (
    <HStack justify="space-between" py={2} px={3} borderRadius="lg" bg={infoBg}>
      <Text color={subTextColor} fontSize="sm">
        {label}
      </Text>
      <HStack>
        {colorValue !== undefined && (
          <Box 
            w={3} 
            h={3} 
            borderRadius="full" 
            bg={getTemperatureColor(colorValue)}
            border="1px solid"
            borderColor={cardBorder}
          />
        )}
        <Text color={textColor} fontSize="sm" fontWeight="600">
          {value}{unit}
        </Text>
      </HStack>
    </HStack>
  );

  const content = (
    <VStack align="start" spacing={6}>
      <HStack justify="space-between" w="full">
        <HStack>
          <IconButton
            aria-label={t("builtinTools.back")}
            icon={<ArrowLeft size={20} />}
            variant="ghost"
            onClick={() => navigate("/builtin-tools")}
            color={headingColor}
          />
          <Heading size="lg" color={headingColor} fontWeight="700">
            {t("displayFilter.title")}
          </Heading>
        </HStack>
        <HStack>
          <Tooltip label={t("displayFilter.resetDefault")}>
            <IconButton
              aria-label="Reset"
              icon={<RotateCcw size={18} />}
              variant="ghost"
              onClick={resetToDefault}
              isDisabled={isLoading}
            />
          </Tooltip>
          <HStack spacing={4}>
            <HotkeyRecorder
              value={filterHotkey}
              onChange={(val) => {
                saveFilterHotkey(val);
                toast({
                  title: t("displayFilter.hotkeySaved") || "快捷键已保存",
                  status: "success",
                  duration: 2000,
                  isClosable: true,
                });
              }}
            />
            <HStack
              bg={settings.is_active ? hexToRgba(primaryColor, 0.2) : sliderBg}
              px={4}
              py={2}
              borderRadius="xl"
              border="1px solid"
              borderColor={settings.is_active ? primaryColor : "transparent"}
            >
              <Text color={textColor} fontSize="sm" fontWeight="500">
                {t("displayFilter.enable")}
              </Text>
              <ThemeSwitch
                isChecked={settings.is_active}
                onChange={toggleFilter}
                isDisabled={isLoading}
              />
            </HStack>
          </HStack>
        </HStack>
      </HStack>

      {displays.length > 0 && (
        <HStack w="full" spacing={3}>
          <Monitor size={18} color={textColor} />
          <CustomSelect
            value={activeDisplayIndex.toString()}
            onChange={handleDisplayChange}
            options={displays.map((d) => ({
              value: d.index.toString(),
              label: `${d.name}${d.is_primary ? ` (${t("displayFilter.primary")})` : ""}`,
            }))}
            width="360px"
          />
        </HStack>
      )}

      <VStack align="start" spacing={4} w="full">
        <Text color={textColor} fontSize="md" fontWeight="600">
          {t("displayFilter.presets")}
        </Text>
        <SimpleGrid
          columns={{
            base: 2,
            sm: 3,
            md: 4,
            lg: 5,
          }}
          spacing={3}
          w="full"
        >
          {presets.map((preset) => {
            const Icon = presetIcons[preset.id] || Monitor;
            const isActive = activePresetId === preset.id;
            const accentColor = preset.id === "normal" ? primaryColor : (presetColors[preset.id] || primaryColor);
            return (
              <Tooltip key={preset.id} label={preset.description} placement="top">
                <Box
                  bg={isActive ? `${accentColor}20` : sliderBg}
                  borderRadius="xl"
                  p={4}
                  cursor="pointer"
                  onClick={() => applyPreset(preset)}
                  border="2px solid"
                  borderColor={isActive ? accentColor : "transparent"}
                  transition="all 0.2s"
                  _hover={{
                    borderColor: accentColor,
                    transform: "translateY(-2px)",
                  }}
                  position="relative"
                  overflow="hidden"
                >
                  {isActive && (
                    <Box
                      position="absolute"
                      top={0}
                      left={0}
                      right={0}
                      h="3px"
                      bg={accentColor}
                    />
                  )}
                  <VStack spacing={2}>
                    <Icon size={24} color={accentColor} />
                    <Text color={textColor} fontSize="sm" fontWeight="600">
                      {preset.name}
                    </Text>
                  </VStack>
                </Box>
              </Tooltip>
            );
          })}
          
          <Tooltip label={t("displayFilter.customDescription")} placement="top">
            <Box
              bg={activePresetId === "custom" ? `${presetColors["custom"]}20` : sliderBg}
              borderRadius="xl"
              p={4}
              cursor="pointer"
              onClick={openCustom}
              border="2px solid"
              borderColor={activePresetId === "custom" ? presetColors["custom"] : "transparent"}
              transition="all 0.2s"
              _hover={{
                borderColor: presetColors["custom"],
                transform: "translateY(-2px)",
              }}
              position="relative"
              overflow="hidden"
            >
              {activePresetId === "custom" && (
                <Box
                  position="absolute"
                  top={0}
                  left={0}
                  right={0}
                  h="3px"
                  bg={presetColors["custom"]}
                />
              )}
              <Box position="absolute" top={2} right={2}>
                <Text fontSize="10px" fontWeight="700" color="#FF6B9D" bg="rgba(255,107,157,0.1)" px={1.5} py={0.5} borderRadius="full">
                  BETA
                </Text>
              </Box>
              <VStack spacing={2}>
                <Settings2 size={24} color={presetColors["custom"]} />
                <Text color={textColor} fontSize="sm" fontWeight="600">
                  {t("displayFilter.custom")}
                </Text>
              </VStack>
            </Box>
          </Tooltip>
        </SimpleGrid>
      </VStack>

      {/* ICC Color Profiles Section */}
      <VStack align="start" spacing={4} w="full">
        <HStack justify="space-between" w="full">
          <HStack>
            <FileImage size={20} color={textColor} />
            <Text color={textColor} fontSize="md" fontWeight="600">
              {t("displayFilter.iccProfiles")}
            </Text>
          </HStack>
          <Button
            size="sm"
            leftIcon={<Upload size={16} />}
            variant="outline"
            colorScheme="blue"
            onClick={handleImportIcc}
            isLoading={isLoading}
          >
            {t("displayFilter.importIcc")}
          </Button>
        </HStack>
        {iccPresets.length === 0 ? (
          <Text color={subTextColor} fontSize="sm" py={2}>
            {t("displayFilter.noIccProfiles")}
          </Text>
        ) : (
          <SimpleGrid
            columns={{
              base: 2,
              sm: 3,
              md: 4,
              lg: 5,
            }}
            spacing={3}
            w="full"
          >
            {iccPresets.map((icc) => {
              const isActive = activeIccId === icc.id;
              const accentColor = "#38B2AC";
              return (
                <Box
                  key={icc.id}
                  bg={isActive ? `${accentColor}20` : sliderBg}
                  borderRadius="xl"
                  p={4}
                  cursor="pointer"
                  onClick={() => handleApplyIcc(icc.id)}
                  border="2px solid"
                  borderColor={isActive ? accentColor : "transparent"}
                  transition="all 0.2s"
                  _hover={{
                    borderColor: accentColor,
                    transform: "translateY(-2px)",
                  }}
                  position="relative"
                  overflow="hidden"
                >
                  {isActive && (
                    <Box
                      position="absolute"
                      top={0}
                      left={0}
                      right={0}
                      h="3px"
                      bg={accentColor}
                    />
                  )}
                  <Tooltip label={icc.description} placement="top">
                    <IconButton
                      aria-label={t("displayFilter.deleteIcc")}
                      icon={<Trash2 size={14} />}
                      size="xs"
                      variant="ghost"
                      position="absolute"
                      top={1}
                      right={1}
                      color="red.400"
                      _hover={{ bg: "red.50" }}
                      onClick={(e) => {
                        e.stopPropagation();
                        setDeleteIccId(icc.id);
                      }}
                    />
                  </Tooltip>
                  <VStack spacing={2}>
                    <FileImage size={24} color={accentColor} />
                    <Text color={textColor} fontSize="sm" fontWeight="600" noOfLines={1}>
                      {icc.name}
                    </Text>
                  </VStack>
                </Box>
              );
            })}
          </SimpleGrid>
        )}
      </VStack>

      <VStack align="start" spacing={4} w="full">
        {(activePresetId === "custom" || activeIccId) && (
        <HStack justify="space-between" w="full">
          <HStack>
            <Text color={textColor} fontSize="md" fontWeight="600">
              {t("displayFilter.currentSettings")}
            </Text>
            {activePresetId === "custom" && (
              <Text fontSize="10px" fontWeight="700" color="#FF6B9D" bg="rgba(255,107,157,0.1)" px={1.5} py={0.5} borderRadius="full">
                BETA
              </Text>
            )}
            {activeIccId && (
              <Text fontSize="10px" fontWeight="700" color={primaryColor} bg={`${primaryColor}20`} px={1.5} py={0.5} borderRadius="full">
                ICC
              </Text>
            )}
          </HStack>
          {activePresetId === "custom" && (
            <Button
              size="sm"
              leftIcon={<Save size={16} />}
              bg={primaryColor}
              color={contrastText}
              onClick={saveAndApply}
              isLoading={isLoading}
              isDisabled={!hasChanges}
              _hover={{
                bg: getHoverColor(),
              }}
            >
              {t("displayFilter.saveAndApply")}
            </Button>
          )}
        </HStack>
        )}

        {activeIccId && (
          <Box w="full" p={4} borderRadius="md" bg={sliderBg}>
            <Text color={textColor} fontSize="sm">
              {iccPresets.find(p => p.id === activeIccId)?.description || t("displayFilter.iccApplied")}
            </Text>
          </Box>
        )}

        {!activeIccId && (
        <SimpleGrid columns={{ base: 1, md: 2 }} spacing={3} w="full">
          <VStack spacing={2} align="stretch">
            {activePresetId === "custom" ? (
              <>
                <EditableItem 
                  label={t("displayFilter.colorTemperature")} 
                  value={editValuesRef.current.temperature}
                  onChange={(val) => handleInputChange("temperature", val)}
                  unit="K"
                  colorValue={editValuesRef.current.temperature}
                />
                <EditableItem 
                  label={t("displayFilter.brightness")} 
                  value={editValuesRef.current.brightness}
                  onChange={(val) => handleInputChange("brightness", val)}
                  unit="%"
                />
                <EditableItem 
                  label={t("displayFilter.contrast")} 
                  value={editValuesRef.current.contrast}
                  onChange={(val) => handleInputChange("contrast", val)}
                  unit="%"
                />
                <EditableItem 
                  label={t("displayFilter.saturation")} 
                  value={editValuesRef.current.saturation}
                  onChange={(val) => handleInputChange("saturation", val)}
                  unit="%"
                />
              </>
            ) : (
              <>
                <ReadOnlyItem 
                  label={t("displayFilter.colorTemperature")} 
                  value={settings.temperature} 
                  unit="K"
                  colorValue={settings.temperature}
                />
                <ReadOnlyItem 
                  label={t("displayFilter.brightness")} 
                  value={settings.brightness} 
                  unit="%"
                />
                <ReadOnlyItem 
                  label={t("displayFilter.contrast")} 
                  value={settings.contrast} 
                  unit="%"
                />
                <ReadOnlyItem 
                  label={t("displayFilter.saturation")} 
                  value={settings.saturation} 
                  unit="%"
                />
              </>
            )}
          </VStack>
          
          <VStack spacing={2} align="stretch">
            <ReadOnlyItem 
              label="Gamma" 
              value={currentModeParams.gamma.toFixed(2)} 
            />
            <ReadOnlyItem 
              label="S-Curve" 
              value={currentModeParams.sCurve.toFixed(2)} 
            />
            <ReadOnlyItem 
              label="R Boost" 
              value={(currentModeParams.rBoost * 100).toFixed(0)} 
              unit="%"
            />
            <ReadOnlyItem 
              label="G Boost" 
              value={(currentModeParams.gBoost * 100).toFixed(0)} 
              unit="%"
            />
            <ReadOnlyItem 
              label="B Boost" 
              value={(currentModeParams.bBoost * 100).toFixed(0)} 
              unit="%"
            />
          </VStack>
        </SimpleGrid>
        )}
      </VStack>

      <Box 
        w="full" 
        p={4} 
        borderRadius="xl" 
        bg={useColorModeValue(hexToRgba(primaryColor, 0.1), hexToRgba(primaryColor, 0.1))}
        border="1px solid"
        borderColor={useColorModeValue(hexToRgba(primaryColor, 0.3), hexToRgba(primaryColor, 0.2))}
      >
        <Text color={subTextColor} fontSize="xs">
          {t("displayFilter.tip")}
        </Text>
      </Box>
    </VStack>
  );

  return (
    <Box pt={8}>
      {/* Delete ICC Confirmation Dialog */}
      <AlertDialog
        isOpen={deleteIccId !== null}
        leastDestructiveRef={cancelDeleteRef}
        onClose={() => setDeleteIccId(null)}
      >
        <AlertDialogOverlay>
          <AlertDialogContent>
            <AlertDialogHeader fontSize="lg" fontWeight="bold">
              {t("displayFilter.deleteIcc")}
            </AlertDialogHeader>
            <AlertDialogBody>
              {t("displayFilter.deleteIccConfirm")}
            </AlertDialogBody>
            <AlertDialogFooter>
              <Button ref={cancelDeleteRef} onClick={() => setDeleteIccId(null)}>
                {t("displayFilter.cancel")}
              </Button>
              <Button colorScheme="red" onClick={handleDeleteIcc} ml={3} isLoading={isLoading}>
                {t("displayFilter.delete")}
              </Button>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialogOverlay>
      </AlertDialog>
      {liquidGlassEnabled ? (
        <LiquidGlassCard
          w="full"
          boxShadow="2xl"
          overflow="hidden"
          position="relative"
          p={6}
        >
          {content}
        </LiquidGlassCard>
      ) : (
        <Card
          bg={cardBg}
          borderColor={cardBorder}
          borderWidth="1px"
          w="full"
          boxShadow="2xl"
          overflow="hidden"
          position="relative"
        >
          <CardBody p={6}>
            {content}
          </CardBody>
        </Card>
      )}
    </Box>
  );
}
