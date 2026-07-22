import {
  Box,
  Text,
  Heading,
  VStack,
  HStack,
  useColorModeValue,
  useToast,
  Button,
  Badge,
  Spinner,
  IconButton,
  Tabs,
  TabList,
  TabPanels,
  TabPanel,
  Tab,
  Input,
  Modal,
  ModalOverlay,
  ModalContent,
  ModalHeader,
  ModalFooter,
  ModalBody,
  ModalCloseButton,
  useDisclosure,
} from "@chakra-ui/react";
import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import {
  ArrowLeft,
  Volume2,
  Trash2,
  Download,
  Activity,
  Music,
  Gamepad2,
  Mic,
  Headphones,
  Play,
  Square,
  RefreshCw,
  CheckCircle,
  XCircle,
  AlertTriangle,
  Upload,
  Save,
} from "lucide-react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useThemeColor } from "@/contexts/theme-color-context";
import { store } from "@/lib/store";

// ===== 类型定义 =====
interface DriverStatus {
  installed: boolean;
  service_exists: boolean;
  service_running: boolean;
  device_name: string;
  needs_reboot: boolean;
}

interface EqBand {
  freq: number;
  gain: number;
}

interface EqPreset {
  id: string;
  name: string;
  bands: EqBand[];
  enabled: boolean;
}

interface EngineStatus {
  running: boolean;
  pid: number | null;
}

// ===== EQ 频段常量 =====
const DEFAULT_EQ_FREQS = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
const EQ_STORE_SELECTED = "eq-selected-preset-id";
const EQ_STORE_BANDS = "eq-current-bands";
const EQ_STORE_IMPORTED = "eq-imported-preset-ids";
const EQ_STORE_SAVED = "eq-saved-preset-ids";
const EQ_STORE_CUSTOM_BANDS = "eq-custom-bands";
const EQ_STORE_PREAMP = "eq-preamp";
const EQ_STORE_FX = "eq-effects";

// ===== 辅助函数 =====
function formatFreq(freq: number): string {
  if (freq >= 1000) {
    return `${(freq / 1000).toFixed(freq >= 10000 ? 0 : 1)}k`;
  }
  return `${Math.round(freq)}`;
}

function getPresetIcon(name: string) {
  const lower = name.toLowerCase();
  if (lower.includes("人声") || lower.includes("voice") || lower.includes("vocal")) return Mic;
  if (lower.includes("低音") || lower.includes("bass") || lower.includes("哈曼")) return Headphones;
  if (lower.includes("音乐") || lower.includes("music")) return Music;
  if (lower.includes("竞技") || lower.includes("脚步") || lower.includes("枪声") || lower.includes("gam")) return Gamepad2;
  return Activity;
}

// ===== 频率响应曲线 Canvas 组件 =====
function FrequencyResponseCurve({ bands }: { bands: EqBand[] }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const lineColor = useColorModeValue("#3182ce", "#63b3ed");
  const fillColor = useColorModeValue("rgba(49, 130, 206, 0.15)", "rgba(99, 179, 237, 0.15)");
  const gridColor = useColorModeValue("rgba(0, 0, 0, 0.06)", "rgba(255, 255, 255, 0.06)");
  const textColor = useColorModeValue("#888", "#888");
  const zeroLineColor = useColorModeValue("rgba(0,0,0,0.15)", "rgba(255,255,255,0.15)");
  const dotBgColor = useColorModeValue("white", "#1a1a1a");
  const { getActiveColor } = useThemeColor();
  const activeColor = getActiveColor();

  // 监听 canvas 尺寸变化，触发重绘
  const [resizeTick, setResizeTick] = useState(0);
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const observer = new ResizeObserver(() => {
      setResizeTick((t) => t + 1);
    });
    observer.observe(canvas);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.scale(dpr, dpr);

    const W = rect.width;
    const H = rect.height;
    const padding = { left: 46, right: 16, top: 16, bottom: 26 };
    const plotW = W - padding.left - padding.right;
    const plotH = H - padding.top - padding.bottom;

    // 清空
    ctx.clearRect(0, 0, W, H);

    // 绘制网格
    ctx.strokeStyle = gridColor;
    ctx.lineWidth = 1;
    ctx.font = "10px sans-serif";
    ctx.fillStyle = textColor;

    // 水平网格线 (增益: -12 to +12, 每 3dB)
    ctx.textAlign = "right";
    ctx.textBaseline = "middle";
    for (let db = -12; db <= 12; db += 3) {
      const y = padding.top + plotH / 2 - (db / 12) * (plotH / 2);
      ctx.beginPath();
      ctx.moveTo(padding.left, y);
      ctx.lineTo(W - padding.right, y);
      ctx.stroke();
      const label = db === 0 ? "0" : `${db > 0 ? "+" : ""}${db}`;
      ctx.fillText(label, padding.left - 6, y);
    }

    // 垂直网格线 (频率: 对数刻度)
    const freqMarks = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
    const logMin = Math.log10(20);
    const logMax = Math.log10(20000);
    ctx.textAlign = "center";
    ctx.textBaseline = "top";
    freqMarks.forEach((freq) => {
      const x = padding.left + ((Math.log10(freq) - logMin) / (logMax - logMin)) * plotW;
      ctx.beginPath();
      ctx.moveTo(x, padding.top);
      ctx.lineTo(x, H - padding.bottom);
      ctx.stroke();
      ctx.fillText(formatFreq(freq), x, H - padding.bottom + 4);
    });

    // 0dB 基准线
    const zeroY = padding.top + plotH / 2;
    ctx.strokeStyle = zeroLineColor;
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.moveTo(padding.left, zeroY);
    ctx.lineTo(W - padding.right, zeroY);
    ctx.stroke();

    if (bands.length === 0) return;

    // 计算 EQ 频率响应曲线
    // 使用简化的峰值滤波器响应近似
    const sampleRate = 48000;
    const numPoints = 300;
    const points: { x: number; y: number }[] = [];

    for (let i = 0; i < numPoints; i++) {
      const t = i / (numPoints - 1);
      const freq = Math.pow(10, logMin + t * (logMax - logMin));

      // 计算每个频段对该频率的贡献
      let totalGain = 0;
      for (const band of bands) {
        // 使用钟形曲线近似（类似 peaking filter 响应）
        const ratio = freq / band.freq;
        const logRatio = Math.log2(ratio);
        // Q值近似为 1.4，带宽约一个倍频程
        const bandwidth = 1.0;
        const contribution = band.gain * Math.exp(-(logRatio * logRatio) / (2 * bandwidth * bandwidth));
        totalGain += contribution;
      }

      const x = padding.left + t * plotW;
      const clampedGain = Math.max(-12, Math.min(12, totalGain));
      const y = padding.top + plotH / 2 - (clampedGain / 12) * (plotH / 2);
      points.push({ x, y });
    }

    // 绘制填充区域
    ctx.beginPath();
    ctx.moveTo(points[0].x, zeroY);
    points.forEach((p) => ctx.lineTo(p.x, p.y));
    ctx.lineTo(points[points.length - 1].x, zeroY);
    ctx.closePath();
    ctx.fillStyle = fillColor;
    ctx.fill();

    // 绘制曲线
    ctx.strokeStyle = activeColor;
    ctx.lineWidth = 2.5;
    ctx.beginPath();
    points.forEach((p, i) => {
      if (i === 0) ctx.moveTo(p.x, p.y);
      else ctx.lineTo(p.x, p.y);
    });
    ctx.stroke();

    // 绘制频段标记点
    bands.forEach((band) => {
      const x = padding.left + ((Math.log10(band.freq) - logMin) / (logMax - logMin)) * plotW;
      const clampedGain = Math.max(-12, Math.min(12, band.gain));
      const y = padding.top + plotH / 2 - (clampedGain / 12) * (plotH / 2);

      // 外圈
      ctx.beginPath();
      ctx.arc(x, y, 5, 0, Math.PI * 2);
      ctx.fillStyle = dotBgColor;
      ctx.fill();
      // 内圈
      ctx.beginPath();
      ctx.arc(x, y, 3, 0, Math.PI * 2);
      ctx.fillStyle = activeColor;
      ctx.fill();
    });

    // Y 轴标签
    ctx.save();
    ctx.translate(15, H / 2);
    ctx.rotate(-Math.PI / 2);
    ctx.fillStyle = textColor;
    ctx.font = "11px sans-serif";
    ctx.textAlign = "center";
    ctx.fillText("dB", 0, 0);
    ctx.restore();

    // X 轴标签
    ctx.textAlign = "center";
    ctx.fillText("Hz", W / 2, H - 2);
  }, [bands, lineColor, fillColor, gridColor, textColor, activeColor, zeroLineColor, dotBgColor, resizeTick]);

  return (
    <canvas
      ref={canvasRef}
      style={{ width: "100%", height: "220px", display: "block" }}
    />
  );
}

// ===== EQ 频段滑块组件 =====
function EqBandSlider({
  band,
  onChange,
}: {
  band: EqBand;
  onChange: (gain: number) => void;
}) {
  const labelColor = useColorModeValue("gray.600", "#aaaaaa");
  const valueColor = useColorModeValue("gray.800", "#e0e0e0");
  const { getActiveColor } = useThemeColor();
  const activeColor = getActiveColor();

  const gain = band.gain;
  const gainPercent = ((gain + 12) / 24) * 100;
  const sTrackBg = useColorModeValue("gray.200", "#333333");
  const sZeroBg = useColorModeValue("gray.400", "#555555");
  const sHandleBg = useColorModeValue("white", "#222222");

  return (
    <VStack spacing={1} align="center" w="full">
      {/* 增益值 */}
      <Text
        fontSize="xs"
        fontWeight="bold"
        color={gain > 0 ? "green.500" : gain < 0 ? "orange.500" : valueColor}
        minH="16px"
      >
        {gain > 0 ? "+" : ""}{gain.toFixed(0)}
      </Text>

      {/* 垂直滑块 */}
      <Box position="relative" h="120px" w="32px" display="flex" justifyContent="center">
        {/* 滑轨 */}
        <Box
          position="absolute"
          left="50%"
          transform="translateX(-50%)"
          w="6px"
          h="full"
          borderRadius="full"
          bg={sTrackBg}
        />
        {/* 0dB 基准线 */}
        <Box
          position="absolute"
          left="50%"
          transform="translateX(-50%)"
          top="50%"
          w="16px"
          h="2px"
          bg={sZeroBg}
          borderRadius="full"
        />
        {/* 填充 */}
        <Box
          position="absolute"
          left="50%"
          transform="translateX(-50%)"
          w="6px"
          borderRadius="full"
          bg={activeColor}
          top={gain >= 0 ? `${100 - gainPercent}%` : "50%"}
          height={`${Math.abs(gainPercent - 50)}%`}
          opacity={0.6}
        />
        {/* 滑块手柄 */}
        <Box
          position="absolute"
          left="50%"
          transform="translate(-50%, -50%)"
          top={`${100 - gainPercent}%`}
          w="20px"
          h="20px"
          borderRadius="full"
          bg={sHandleBg}
          border="3px solid"
          borderColor={activeColor}
          cursor="pointer"
          shadow="md"
          _active={{ transform: "translate(-50%, -50%) scale(0.9)" }}
        />
        {/* 隐藏的 input range */}
        <input
          type="range"
          min={-12}
          max={12}
          step={1}
          value={gain}
          onChange={(e) => onChange(parseFloat(e.target.value))}
          style={{
            position: "absolute",
            width: "120px",
            height: "32px",
            top: "50%",
            left: "50%",
            transform: "translate(-50%, -50%) rotate(-90deg)",
            opacity: 0,
            cursor: "pointer",
          }}
        />
      </Box>

      {/* 频率标签 */}
      <Text fontSize="xs" color={labelColor} fontWeight="medium">
        {formatFreq(band.freq)}
      </Text>
    </VStack>
  );
}

// ===== 主页面组件 =====
export default function AudioEqPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const toast = useToast();

  const headingColor = useColorModeValue("#1A202C", "#ffffff");
  const labelColor = useColorModeValue("gray.700", "#e0e0e0");
  const descColor = useColorModeValue("gray.500", "#888888");
  const presetIconColor = useColorModeValue("#4A5568", "#aaaaaa");
  const iconBg = useColorModeValue("gray.100", "#222222");
  const warningBg = useColorModeValue("orange.50", "rgba(237, 137, 54, 0.1)");
  const warningColor = useColorModeValue("orange.600", "orange.300");
  const curveBg = useColorModeValue("gray.50", "#0a0a0a");
  const { getActiveColor } = useThemeColor();
  const activeColor = getActiveColor();

  const [driverStatus, setDriverStatus] = useState<DriverStatus | null>(null);
  const [engineStatus, setEngineStatus] = useState<EngineStatus>({ running: false, pid: null });
  const [presets, setPresets] = useState<EqPreset[]>([]);
  const [selectedPresetId, setSelectedPresetId] = useState<string>("");
  const [bands, setBands] = useState<EqBand[]>(
    DEFAULT_EQ_FREQS.map((f) => ({ freq: f, gain: 0 }))
  );
  const [loading, setLoading] = useState(true);
  const [installing, setInstalling] = useState(false);
  const [uninstalling, setUninstalling] = useState(false);
  const [startingEngine, setStartingEngine] = useState(false);
  const [tabIndex, setTabIndex] = useState(0);
  const [importing, setImporting] = useState(false);
  const [importedIds, setImportedIds] = useState<string[]>([]);
  const [savedIds, setSavedIds] = useState<string[]>([]);
  const [presetName, setPresetName] = useState("");
  const [preamp, setPreamp] = useState(0);
  const defaultFx = { clarity: 0, ambience: 0, width: 0, dynamics: 0, bass: 0 };
  const [fx, setFx] = useState(defaultFx);
  const { isOpen: saveModalOpen, onOpen: onSaveModalOpen, onClose: onSaveModalClose } = useDisclosure();
  const CUSTOM_PRESET_ID = "__custom__";
  const selectedPresetIdRef = useRef<string>("");
  const fileInputRef = useRef<HTMLInputElement>(null);

  const builtInPresets = presets.filter((p) => !importedIds.includes(p.id) && !savedIds.includes(p.id));
  const savedPresets = presets.filter((p) => savedIds.includes(p.id));
  const importedPresets = presets.filter((p) => importedIds.includes(p.id));

  // 加载数据
  const loadData = useCallback(async () => {
    try {
      setLoading(true);
      const [status, engine, presetList, savedId, savedBands, savedImported, savedUser, savedPreamp, savedFx] = await Promise.all([
        invoke<DriverStatus>("check_virtual_audio_driver"),
        invoke<EngineStatus>("get_eq_engine_status"),
        invoke<EqPreset[]>("get_eq_presets"),
        store.get<string>(EQ_STORE_SELECTED),
        store.get<EqBand[]>(EQ_STORE_BANDS),
        store.get<string[]>(EQ_STORE_IMPORTED),
        store.get<string[]>(EQ_STORE_SAVED),
        store.get<number>(EQ_STORE_PREAMP),
        store.get<typeof defaultFx>(EQ_STORE_FX),
      ]);
      setDriverStatus(status);
      setEngineStatus(engine);
      setPresets(presetList);
      setImportedIds(savedImported ?? []);
      setSavedIds(savedUser ?? []);
      setPreamp(savedPreamp ?? 0);
      setFx(savedFx ?? defaultFx);

      if (presetList.length === 0) return;

      // 恢复上次选中的预设和频段
      if (savedId) {
        // 自定义：恢复之前保存的频段
        if (savedId === CUSTOM_PRESET_ID) {
          setSelectedPresetId(CUSTOM_PRESET_ID);
          selectedPresetIdRef.current = CUSTOM_PRESET_ID;
          if (savedBands) setBands(savedBands);
          return;
        }
        // 已保存的预设
        const saved = presetList.find((p) => p.id === savedId);
        if (saved) {
          setSelectedPresetId(saved.id);
          selectedPresetIdRef.current = saved.id;
          setBands(savedBands ?? saved.bands);
          return;
        }
      }

      // 兜底：选第一个
      const first = presetList[0];
      setSelectedPresetId(first.id);
      selectedPresetIdRef.current = first.id;
      setBands(first.bands);
    } catch (error) {
      console.error("Failed to load EQ data:", error);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadData();
  }, [loadData]);

  // 定时刷新引擎状态
  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const status = await invoke<EngineStatus>("get_eq_engine_status");
        setEngineStatus(status);
      } catch (e) {
        // ignore
      }
    }, 3000);
    return () => clearInterval(interval);
  }, []);

  // 处理安装驱动
  const handleInstall = async () => {
    setInstalling(true);
    try {
      const result = await invoke<string>("install_virtual_audio_driver");
      toast({
        title: t("audioEq.installSuccess"),
        description: result,
        status: "success",
        duration: 3000,
        isClosable: true,
      });
      await loadData();
    } catch (error) {
      toast({
        title: t("audioEq.installFailed"),
        description: String(error),
        status: "error",
        duration: 5000,
        isClosable: true,
      });
    } finally {
      setInstalling(false);
    }
  };

  // 处理卸载驱动
  const handleUninstall = async () => {
    setUninstalling(true);
    try {
      const result = await invoke<string>("uninstall_virtual_audio_driver");
      toast({
        title: t("audioEq.uninstallSuccess"),
        description: result,
        status: "success",
        duration: 3000,
        isClosable: true,
      });
      await loadData();
      await loadData();
    } catch (error) {
      toast({
        title: t("audioEq.uninstallFailed"),
        description: String(error),
        status: "error",
        duration: 8000,
        isClosable: true,
      });
    } finally {
      setUninstalling(false);
    }
  };

  // 启动 EQ 引擎
  const handleStartEngine = async () => {
    setStartingEngine(true);
    try {
      await invoke("start_eq_engine");
      // 引擎启动后，同步当前设置
      const bandTuples = bands.map((b) => [b.freq, b.gain] as [number, number]);
      await invoke("update_eq_bands", { bands: bandTuples }).catch(() => {});
      await invoke("update_eq_preamp", { gain: preamp }).catch(() => {});
      await invoke("update_eq_effects", { ...fx }).catch(() => {});
      toast({
        title: t("audioEq.engineStarted"),
        status: "success",
        duration: 2000,
        isClosable: true,
      });
      const status = await invoke<EngineStatus>("get_eq_engine_status");
      setEngineStatus(status);
    } catch (error) {
      toast({
        title: t("audioEq.engineStartFailed"),
        description: String(error),
        status: "error",
        duration: 5000,
        isClosable: true,
      });
    } finally {
      setStartingEngine(false);
    }
  };

  // 停止 EQ 引擎
  const handleStopEngine = async () => {
    try {
      await invoke("stop_eq_engine");
      toast({
        title: t("audioEq.engineStopped"),
        status: "info",
        duration: 2000,
        isClosable: true,
      });
      setEngineStatus({ running: false, pid: null });
    } catch (error) {
      toast({
        title: t("audioEq.engineStopFailed"),
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    }
  };

  // 总增益调节
  const handlePreampChange = (gain: number) => {
    setPreamp(gain);
    store.set(EQ_STORE_PREAMP, gain).then(() => store.save()).catch(() => {});
    invoke("update_eq_preamp", { gain }).catch(() => {});
  };

  // 选择预设
  const handleSelectPreset = async (preset: EqPreset) => {
    setSelectedPresetId(preset.id);
    selectedPresetIdRef.current = preset.id;
    setBands(preset.bands);
    await store.set(EQ_STORE_SELECTED, preset.id);
    await store.set(EQ_STORE_BANDS, preset.bands);
    await store.save();
    try {
      await invoke("apply_eq_preset", { presetId: preset.id });
      toast({
        title: t("audioEq.presetApplied"),
        description: preset.name,
        status: "success",
        duration: 2000,
        isClosable: true,
      });
    } catch (error) {
      console.error("Failed to apply preset:", error);
    }
  };

  // 更新频段增益（实时同步到音频引擎）
  const handleBandChange = (index: number, gain: number) => {
    setBands((prev) => {
      const newBands = [...prev];
      newBands[index] = { ...newBands[index], gain };
      // 实时发送到音频引擎（无需重启）
      if (engineStatus.running) {
        const bandTuples = newBands.map((b) => [b.freq, b.gain] as [number, number]);
        invoke("update_eq_bands", { bands: bandTuples }).catch(() => {});
      }
      // 保存当前频段状态 + 当前选中的预设 ID
      store.set(EQ_STORE_BANDS, newBands).then(() => store.save()).catch(() => {});
      store.set(EQ_STORE_SELECTED, selectedPresetIdRef.current).catch(() => {});
      // 自定义模式：额外保存自定义频段供下次切换恢复
      if (selectedPresetIdRef.current === CUSTOM_PRESET_ID) {
        store.set(EQ_STORE_CUSTOM_BANDS, newBands).catch(() => {});
      }
      return newBands;
    });
  };

  // 重置频段（实时同步）
  const handleResetBands = () => {
    const resetBands = bands.map((b) => ({ ...b, gain: 0 }));
    setBands(resetBands);
    store.set(EQ_STORE_BANDS, resetBands).then(() => store.save()).catch(() => {});
    if (selectedPresetIdRef.current === CUSTOM_PRESET_ID) {
      store.set(EQ_STORE_CUSTOM_BANDS, resetBands).catch(() => {});
    }
    if (engineStatus.running) {
      const bandTuples = resetBands.map((b) => [b.freq, b.gain] as [number, number]);
      invoke("update_eq_bands", { bands: bandTuples }).catch(() => {});
    }
  };

  // 导入 FAC 预设文件
  const handleImportFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    setImporting(true);
    try {
      const content = await file.text();
      const imported = await invoke<EqPreset>("import_eq_preset", { content });
      setPresets((prev) => [...prev, imported]);
      const newImportedIds = [...importedIds, imported.id];
      setImportedIds(newImportedIds);
      await store.set(EQ_STORE_IMPORTED, newImportedIds);
      await store.save();
      toast({
        title: t("audioEq.importSuccess") ?? "导入成功",
        description: imported.name,
        status: "success",
        duration: 2000,
        isClosable: true,
      });
      // 重置 input 以便重复导入同一文件
      if (fileInputRef.current) fileInputRef.current.value = "";
    } catch (error) {
      toast({
        title: t("audioEq.importFailed") ?? "导入失败",
        description: String(error),
        status: "error",
        duration: 4000,
        isClosable: true,
      });
    } finally {
      setImporting(false);
    }
  };

  // 构建 FAC 文件内容
  const buildFacContent = (name: string): string => {
    const bandsStr = bands
      .map(
        (b, i) =>
          `Band ${i + 1}\n   ${b.freq}: CF\n   ${Math.round(b.gain)}: Boost/Cut`
      )
      .join("\n");
    return `CLASS1 : Effect Type
9: Version
${name}
0: Double Params Flag
1: Total number of elements
50: Main 0
20: Main 1
0: Main 2
0: Main 3
60: Main 4
60: Main 5
0: Element Number
   0: Param 0
   0: Param 1
   0: Param 2
   0: Param 3
   0: Param 4
   0: Param 5
   0: Param 6
7: Number of Application Dependent Integers
0: Number of Application Dependent Reals
0: Number of Application Dependent Strings
1: Integer[0]
1: Integer[1]
0: Integer[2]
1: Integer[3]
1: Integer[4]
0: Integer[5]
2: Integer[6]
10: Number of EQ Bands
1: On/Off Flag
${bandsStr}`;
  };

  // 保存当前频段为预设
  const handleSavePreset = async () => {
    const name = presetName.trim();
    if (!name) {
      toast({
        title: t("audioEq.saveFailed") ?? "保存失败",
        description: "请输入预设名称",
        status: "error",
        duration: 2000,
        isClosable: true,
      });
      return;
    }
    try {
      const content = buildFacContent(name);
      const saved = await invoke<EqPreset>("import_eq_preset", { content });
      setPresets((prev) => [...prev, saved]);
      const newSavedIds = [...savedIds, saved.id];
      setSavedIds(newSavedIds);
      await store.set(EQ_STORE_SAVED, newSavedIds);
      await store.save();
      setPresetName("");
      onSaveModalClose();
      // 保持在预设 Tab，选中新预设
      setSelectedPresetId(saved.id);
      selectedPresetIdRef.current = saved.id;
      toast({
        title: t("audioEq.saveSuccess") ?? "保存成功",
        description: name,
        status: "success",
        duration: 2000,
        isClosable: true,
      });
    } catch (error) {
      toast({
        title: t("audioEq.saveFailed") ?? "保存失败",
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    }
  };

  // 切换到自定义模式（恢复上次自定义值，首次则为全零）
  const handleCustomPreset = async () => {
    setSelectedPresetId(CUSTOM_PRESET_ID);
    selectedPresetIdRef.current = CUSTOM_PRESET_ID;
    // 读取上次保存的自定义频段
    const savedCustom = await store.get<EqBand[]>(EQ_STORE_CUSTOM_BANDS);
    const customBands = savedCustom
      ? bands.map((b, i) => ({ ...b, gain: savedCustom[i]?.gain ?? 0 }))
      : bands.map((b) => ({ ...b, gain: 0 }));
    setBands(customBands);
    store.set(EQ_STORE_SELECTED, CUSTOM_PRESET_ID).then(() => store.save()).catch(() => {});
    const bandTuples = customBands.map((b) => [b.freq, b.gain] as [number, number]);
    invoke("update_eq_bands", { bands: bandTuples }).catch(() => {});
  };

  // 删除保存的预设（从 presets tab 中删除）
  const handleDeleteSavedPreset = async (presetId: string, name: string) => {
    try {
      await invoke("delete_eq_preset", { presetId });
      setPresets((prev) => prev.filter((p) => p.id !== presetId));
      const newSavedIds = savedIds.filter((id) => id !== presetId);
      setSavedIds(newSavedIds);
      await store.set(EQ_STORE_SAVED, newSavedIds);
      await store.save();
      if (selectedPresetId === presetId) {
        setSelectedPresetId("");
        selectedPresetIdRef.current = "";
      }
      toast({
        title: t("audioEq.deleteSuccess") ?? "已删除",
        description: name,
        status: "info",
        duration: 2000,
        isClosable: true,
      });
    } catch (error) {
      toast({
        title: t("audioEq.deleteFailed") ?? "删除失败",
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    }
  };

  // 删除导入的预设
  const handleDeletePreset = async (presetId: string, name: string) => {
    try {
      await invoke("delete_eq_preset", { presetId });
      setPresets((prev) => prev.filter((p) => p.id !== presetId));
      const newImportedIds = importedIds.filter((id) => id !== presetId);
      setImportedIds(newImportedIds);
      await store.set(EQ_STORE_IMPORTED, newImportedIds);
      await store.save();
      // 如果删除的是当前选中的，清除选中
      if (selectedPresetId === presetId) {
        setSelectedPresetId("");
        selectedPresetIdRef.current = "";
      }
      toast({
        title: t("audioEq.deleteSuccess") ?? "已删除",
        description: name,
        status: "info",
        duration: 2000,
        isClosable: true,
      });
    } catch (error) {
      toast({
        title: t("audioEq.deleteFailed") ?? "删除失败",
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    }
  };

  if (loading) {
    return (
      <Box pt={8} display="flex" justifyContent="center" alignItems="center" minH="400px">
        <VStack spacing={4}>
          <Spinner size="xl" color={activeColor} />
          <Text color={descColor}>{t("audioEq.loading")}</Text>
        </VStack>
      </Box>
    );
  }

  return (
    <Box pt={8} pb={8}>
      {/* 顶部导航 */}
      <HStack mb={6} spacing={3}>
        <IconButton
          aria-label={t("common.back")}
          icon={<ArrowLeft size={20} />}
          variant="ghost"
          onClick={() => navigate("/builtin-tools")}
          color={headingColor}
        />
        <Volume2 size={24} color={headingColor} />
        <Heading size="lg" color={headingColor}>
          {t("audioEq.title")}
        </Heading>
      </HStack>

      <VStack spacing={4} align="stretch">
        {/* 虚拟声卡 & EQ引擎 合并顶部卡片 */}
        <LiquidGlassCard p={4}>
          <VStack align="stretch" spacing={3}>
            {/* 标题行 + 状态 */}
            <HStack justify="space-between" align="start">
              <HStack spacing={3}>
                <Box
                  w={10}
                  h={10}
                  borderRadius="lg"
                  bg={iconBg}
                  display="flex"
                  alignItems="center"
                  justifyContent="center"
                >
                  <Volume2 size={20} color={activeColor} />
                </Box>
                <VStack align="start" spacing={0}>
                  <Text fontWeight="bold" fontSize="md" color={headingColor}>
                    {t("audioEq.virtualSoundCard")} & {t("audioEq.eqEngine")}
                  </Text>
                  <Text fontSize="xs" color={descColor}>
                    {driverStatus?.device_name ?? t("audioEq.virtualSoundCardDesc")}
                  </Text>
                </VStack>
              </HStack>

              <HStack spacing={2}>
                {engineStatus.running && (
                  <Badge colorScheme="green" px={2} py={1} borderRadius="full" variant="subtle">
                    <HStack spacing={1}>
                      <Box w={2} h={2} borderRadius="full" bg="green.400" />
                      <Text fontSize="xs">{t("audioEq.running")}</Text>
                    </HStack>
                  </Badge>
                )}
                {driverStatus?.needs_reboot ? (
                  <Badge colorScheme="orange" px={2} py={1} borderRadius="full" variant="subtle">
                    <HStack spacing={1}>
                      <AlertTriangle size={12} />
                      <Text fontSize="xs">{t("audioEq.needsReboot")}</Text>
                    </HStack>
                  </Badge>
                ) : driverStatus?.installed ? (
                  <Badge colorScheme="green" px={2} py={1} borderRadius="full" variant="subtle">
                    <HStack spacing={1}>
                      <CheckCircle size={12} />
                      <Text fontSize="xs">{t("audioEq.installed")}</Text>
                    </HStack>
                  </Badge>
                ) : (
                  <Badge colorScheme="gray" px={2} py={1} borderRadius="full" variant="subtle">
                    <HStack spacing={1}>
                      <XCircle size={12} />
                      <Text fontSize="xs">{t("audioEq.notInstalled")}</Text>
                    </HStack>
                  </Badge>
                )}
              </HStack>
            </HStack>

            {/* 操作按钮行 */}
            <HStack spacing={2} flexWrap="wrap" justify="space-between">
              {/* 左侧：卸载 */}
              <HStack spacing={2}>
                {driverStatus?.installed && !driverStatus?.needs_reboot && (
                  <Button
                    leftIcon={<Trash2 size={16} />}
                    colorScheme="red"
                    variant="ghost"
                    size="sm"
                    onClick={handleUninstall}
                    isLoading={uninstalling}
                  >
                    {t("audioEq.uninstallDriver")}
                  </Button>
                )}
              </HStack>

              {/* 右侧：安装/启动/停止/刷新 */}
              <HStack spacing={2}>
                {!driverStatus?.installed && !driverStatus?.needs_reboot && (
                  <Button
                    leftIcon={<Download size={16} />}
                    colorScheme="blue"
                    size="sm"
                    onClick={handleInstall}
                    isLoading={installing}
                    loadingText={t("audioEq.installing")}
                  >
                    {t("audioEq.installDriver")}
                  </Button>
                )}
                {!engineStatus.running && (
                  <Button
                    leftIcon={<Play size={16} />}
                    colorScheme="green"
                    size="sm"
                    onClick={handleStartEngine}
                    isLoading={startingEngine}
                    loadingText={t("audioEq.starting")}
                    isDisabled={!driverStatus?.installed}
                  >
                    {t("audioEq.startEngine")}
                  </Button>
                )}
                {engineStatus.running && (
                  <Button
                    leftIcon={<Square size={16} />}
                    colorScheme="red"
                    variant="outline"
                    size="sm"
                    onClick={handleStopEngine}
                  >
                    {t("audioEq.stopEngine")}
                  </Button>
                )}
                <IconButton
                  aria-label={t("common.refresh")}
                  icon={<RefreshCw size={16} />}
                  size="sm"
                  variant="ghost"
                  onClick={loadData}
                />
              </HStack>
            </HStack>

            {/* 警告提示 */}
            {driverStatus?.needs_reboot && (
              <HStack spacing={2} p={2} borderRadius="md" bg={warningBg}>
                <AlertTriangle size={14} color="orange" />
                <Text fontSize="xs" color={warningColor}>
                  {t("audioEq.rebootHint")}
                </Text>
              </HStack>
            )}
            {!driverStatus?.installed && !driverStatus?.needs_reboot && (
              <HStack spacing={2} p={2} borderRadius="md" bg={warningBg}>
                <AlertTriangle size={14} color="orange" />
                <Text fontSize="xs" color={warningColor}>
                  {t("audioEq.installHint")}
                </Text>
              </HStack>
            )}
            {!driverStatus?.installed && (
              <Text fontSize="xs" color="orange.500">
                {t("audioEq.installDriverFirst")}
              </Text>
            )}
          </VStack>
        </LiquidGlassCard>

        {/* 总增益 */}
        <LiquidGlassCard p={4}>
          <HStack spacing={4} align="center">
            <Text fontSize="sm" fontWeight="bold" color={headingColor} whiteSpace="nowrap">
              总增益
            </Text>
            <Box flex={1}>
              <input
                type="range"
                min={-12}
                max={12}
                step={0.5}
                value={preamp}
                onChange={(e) => handlePreampChange(parseFloat(e.target.value))}
                style={{
                  width: "100%",
                  height: "6px",
                  appearance: "none",
                  background: `linear-gradient(to right, #8884 0%, ${activeColor} ${((preamp + 12) / 24) * 100}%, #8884 ${((preamp + 12) / 24) * 100}%, #8884 100%)`,
                  borderRadius: "3px",
                  outline: "none",
                  cursor: "pointer",
                }}
              />
            </Box>
            <Text fontSize="sm" fontWeight="bold" color={preamp === 0 ? descColor : (preamp > 0 ? "#38A169" : "#E53E3E")} minW="40px" textAlign="right">
              {preamp > 0 ? "+" : ""}{preamp} dB
            </Text>
          </HStack>
        </LiquidGlassCard>

        {/* 均衡器 + 预设 双栏布局 */}
        <HStack spacing={4} align="start">
          {/* 左侧：均衡器 */}
          <Box flex="1" minW={0}>
            <LiquidGlassCard p={4}>
              <VStack align="stretch" spacing={4}>
                <HStack justify="space-between">
                  <Text fontWeight="bold" fontSize="md" color={headingColor}>
                    {t("audioEq.equalizer")}
                  </Text>
                  <HStack spacing={1}>
                    {selectedPresetId === CUSTOM_PRESET_ID && (
                      <Button
                        leftIcon={<Save size={14} />}
                        size="xs"
                        variant="ghost"
                        onClick={() => {
                          setPresetName("");
                          onSaveModalOpen();
                        }}
                      >
                        保存为预设
                      </Button>
                    )}
                    <Button
                      leftIcon={<RefreshCw size={14} />}
                      size="xs"
                      variant="ghost"
                      onClick={handleResetBands}
                    >
                      {t("audioEq.reset")}
                    </Button>
                  </HStack>
                </HStack>

                {/* 频率响应曲线 */}
                <Box borderRadius="lg" bg={curveBg} p={2} overflow="hidden">
                  <FrequencyResponseCurve bands={bands} />
                </Box>

                {/* 频段滑块 */}
                <HStack justify="space-between" align="stretch" spacing={1} px={1}>
                  {bands.map((band, index) => (
                    <EqBandSlider
                      key={index}
                      band={band}
                      onChange={(gain) => handleBandChange(index, gain)}
                    />
                  ))}
                </HStack>

                {/* 频段标签 */}
                <HStack spacing={1} justify="space-between" px={1}>
                  {bands.map((band, index) => (
                    <VStack key={index} spacing={0} w="full" align="center">
                      <Text fontSize="2xs" color={descColor}>
                        {formatFreq(band.freq)}Hz
                      </Text>
                    </VStack>
                  ))}
                </HStack>
              </VStack>
            </LiquidGlassCard>
          </Box>

          {/* 右侧：预设 / 导入 */}
          <Box w="230px" flexShrink={0}>
            <LiquidGlassCard p={4}>
              <Tabs index={tabIndex} onChange={setTabIndex} variant="soft-rounded" size="sm" isFitted>
                <TabList mb={3}>
                  <Tab fontSize="xs" py={1} _selected={{ color: activeColor }}>{t("audioEq.presets")}</Tab>
                  <Tab fontSize="xs" py={1} _selected={{ color: activeColor }}>{t("audioEq.import") ?? "导入"}</Tab>
                </TabList>

                <TabPanels>
                  {/* 预设列表 */}
                  <TabPanel p={0}>
                    <VStack spacing={1} align="stretch">
                      {builtInPresets.map((preset) => {
                        const Icon = getPresetIcon(preset.name);
                        const isSelected = selectedPresetId === preset.id;
                        return (
                          <Box
                            key={preset.id}
                            onClick={() => handleSelectPreset(preset)}
                            cursor="pointer"
                            px={3}
                            py={2.5}
                            borderRadius="lg"
                            border="1px solid"
                            borderColor={isSelected ? activeColor : `${activeColor}18`}
                            bg={isSelected ? `${activeColor}12` : `${activeColor}04`}
                            _hover={{
                              bg: `${activeColor}10`,
                              borderColor: isSelected ? activeColor : `${activeColor}40`,
                            }}
                            transition="all 0.2s"
                          >
                            <HStack spacing={2}>
                              <Icon size={16} color={isSelected ? activeColor : presetIconColor} />
                              <Text
                                fontSize="sm"
                                fontWeight={isSelected ? "bold" : "medium"}
                                color={isSelected ? activeColor : labelColor}
                                isTruncated
                              >
                                {preset.name}
                              </Text>
                            </HStack>
                          </Box>
                        );
                      })}
                      {/* 用户保存的预设 */}
                      {savedPresets.map((preset) => {
                        const Icon = getPresetIcon(preset.name);
                        const isSelected = selectedPresetId === preset.id;
                        return (
                          <HStack key={preset.id} spacing={0}>
                            <Box
                              flex={1}
                              onClick={() => handleSelectPreset(preset)}
                              cursor="pointer"
                              px={3}
                              py={2}
                              borderRadius="lg"
                              border="1px solid"
                              borderColor={isSelected ? activeColor : `${activeColor}18`}
                              bg={isSelected ? `${activeColor}12` : `${activeColor}04`}
                              _hover={{
                                bg: `${activeColor}10`,
                                borderColor: isSelected ? activeColor : `${activeColor}40`,
                              }}
                              transition="all 0.2s"
                            >
                              <HStack spacing={2}>
                                <Icon size={14} color={isSelected ? activeColor : presetIconColor} />
                                <Text fontSize="xs" fontWeight={isSelected ? "bold" : "medium"} color={isSelected ? activeColor : labelColor} isTruncated>
                                  {preset.name}
                                </Text>
                              </HStack>
                            </Box>
                            <IconButton
                              aria-label={t("audioEq.deletePreset") ?? "删除"}
                              icon={<Trash2 size={12} />}
                              size="xs"
                              variant="ghost"
                              color={descColor}
                              _hover={{ color: "red.400", bg: "transparent" }}
                              onClick={(e) => {
                                e.stopPropagation();
                                handleDeleteSavedPreset(preset.id, preset.name);
                              }}
                              ml={1}
                            />
                          </HStack>
                        );
                      })}
                      {/* 自定义 */}
                      <Box
                        onClick={handleCustomPreset}
                        cursor="pointer"
                        px={3}
                        py={2.5}
                        borderRadius="lg"
                        border="1px solid"
                        borderColor={selectedPresetId === CUSTOM_PRESET_ID ? activeColor : `${activeColor}18`}
                        bg={selectedPresetId === CUSTOM_PRESET_ID ? `${activeColor}12` : `${activeColor}04`}
                        _hover={{
                          bg: `${activeColor}10`,
                          borderColor: selectedPresetId === CUSTOM_PRESET_ID ? activeColor : `${activeColor}40`,
                        }}
                        transition="all 0.2s"
                      >
                        <HStack spacing={2}>
                          <Music size={16} color={selectedPresetId === CUSTOM_PRESET_ID ? activeColor : presetIconColor} />
                          <Text fontSize="sm" fontWeight={selectedPresetId === CUSTOM_PRESET_ID ? "bold" : "medium"} color={selectedPresetId === CUSTOM_PRESET_ID ? activeColor : labelColor}>
                            自定义
                          </Text>
                        </HStack>
                      </Box>
                    </VStack>
                  </TabPanel>

                  {/* 导入 */}
                  <TabPanel p={0}>
                    <VStack spacing={3} align="stretch" pt={2}>
                      {/* 导入按钮 */}
                      <Input
                        ref={fileInputRef}
                        type="file"
                        accept=".fac"
                        onChange={handleImportFile}
                        display="none"
                        id="fac-file-input"
                      />
                      <Button
                        as="label"
                        htmlFor="fac-file-input"
                        leftIcon={<Upload size={14} />}
                        size="sm"
                        w="full"
                        variant="outline"
                        isLoading={importing}
                        cursor="pointer"
                        borderColor={`${activeColor}40`}
                        color={activeColor}
                        _hover={{ bg: `${activeColor}10`, borderColor: activeColor }}
                      >
                        {t("audioEq.chooseFacFile") ?? "选择 .fac 文件"}
                      </Button>
                      <Text fontSize="xs" color={descColor}>
                        {t("audioEq.importDesc") ?? "导入 .fac 预设文件。"}
                      </Text>

                      {/* 导入的预设列表 */}
                      {importedPresets.length === 0 ? (
                        <Text fontSize="xs" color={descColor} textAlign="center" py={2}>
                          {t("audioEq.noImported") ?? "暂无导入的预设"}
                        </Text>
                      ) : (
                        <VStack spacing={1} align="stretch">
                          {importedPresets.map((preset) => {
                            const Icon = getPresetIcon(preset.name);
                            const isSelected = selectedPresetId === preset.id;
                            return (
                              <HStack key={preset.id} spacing={0}>
                                <Box
                                  flex={1}
                                  onClick={() => handleSelectPreset(preset)}
                                  cursor="pointer"
                                  px={3}
                                  py={2}
                                  borderRadius="lg"
                                  border="1px solid"
                                  borderColor={isSelected ? activeColor : `${activeColor}18`}
                                  bg={isSelected ? `${activeColor}12` : `${activeColor}04`}
                                  _hover={{
                                    bg: `${activeColor}10`,
                                    borderColor: isSelected ? activeColor : `${activeColor}40`,
                                  }}
                                  transition="all 0.2s"
                                >
                                  <HStack spacing={2}>
                                    <Icon size={14} color={isSelected ? activeColor : presetIconColor} />
                                    <Text
                                      fontSize="xs"
                                      fontWeight={isSelected ? "bold" : "medium"}
                                      color={isSelected ? activeColor : labelColor}
                                      isTruncated
                                    >
                                      {preset.name}
                                    </Text>
                                  </HStack>
                                </Box>
                                <IconButton
                                  aria-label={t("audioEq.deletePreset") ?? "删除"}
                                  icon={<Trash2 size={12} />}
                                  size="xs"
                                  variant="ghost"
                                  color={descColor}
                                  _hover={{ color: "red.400", bg: "transparent" }}
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    handleDeletePreset(preset.id, preset.name);
                                  }}
                                  ml={1}
                                />
                              </HStack>
                            );
                          })}
                        </VStack>
                      )}
                    </VStack>
                  </TabPanel>
                </TabPanels>
              </Tabs>
            </LiquidGlassCard>
          </Box>
        </HStack>

        {/* 音效处理 */}
        <LiquidGlassCard p={4}>
          <Text fontWeight="bold" fontSize="md" color={headingColor} mb={3}>
            音效处理
          </Text>
          <VStack spacing={3} align="stretch">
            {([
              { key: "clarity", label: "保真", desc: "还原高频细节" },
              { key: "width", label: "环绕", desc: "拓宽声场" },
              { key: "dynamics", label: "动态", desc: "提升响度" },
            ] as const).map(({ key, label, desc }) => (
              <Box key={key}>
                <HStack justify="space-between" mb={1}>
                  <HStack spacing={2}>
                    <Text fontSize="sm" fontWeight="medium" color={labelColor}>{label}</Text>
                    <Text fontSize="xs" color={descColor}>{desc}</Text>
                  </HStack>
                  <Text fontSize="sm" fontWeight="bold" color={fx[key] > 0 ? activeColor : descColor} minW="36px" textAlign="right">
                    {Math.round(fx[key] * 100)}%
                  </Text>
                </HStack>
                <input
                  type="range"
                  min={0}
                  max={100}
                  step={1}
                  value={Math.round(fx[key] * 100)}
                  onChange={(e) => {
                    const v = parseInt(e.target.value) / 100;
                    const newFx = { ...fx, [key]: v };
                    setFx(newFx);
                    invoke("update_eq_effects", { ...newFx }).catch(() => {});
                    store.set(EQ_STORE_FX, newFx).then(() => store.save()).catch(() => {});
                  }}
                  style={{
                    width: "100%",
                    height: "6px",
                    appearance: "none",
                    background: `linear-gradient(to right, ${activeColor} ${fx[key] * 100}%, #8884 ${fx[key] * 100}%, #8884 100%)`,
                    borderRadius: "3px",
                    outline: "none",
                    cursor: "pointer",
                    accentColor: activeColor,
                  }}
                />
              </Box>
            ))}
          </VStack>
        </LiquidGlassCard>

        {/* 说明区域 */}
        <LiquidGlassCard p={4}>
          <VStack align="start" spacing={3}>
            <Text fontWeight="bold" fontSize="sm" color={headingColor}>
              {t("audioEq.usageGuide")}
            </Text>
            <VStack align="start" spacing={2}>
              <HStack spacing={2} align="start">
                <Text fontSize="xs" color={activeColor} fontWeight="bold">1.</Text>
                <Text fontSize="xs" color={descColor} lineHeight="1.6">
                  {t("audioEq.step1")}
                </Text>
              </HStack>
              <HStack spacing={2} align="start">
                <Text fontSize="xs" color={activeColor} fontWeight="bold">2.</Text>
                <Text fontSize="xs" color={descColor} lineHeight="1.6">
                  {t("audioEq.step2")}
                </Text>
              </HStack>
              <HStack spacing={2} align="start">
                <Text fontSize="xs" color={activeColor} fontWeight="bold">3.</Text>
                <Text fontSize="xs" color={descColor} lineHeight="1.6">
                  {t("audioEq.step3")}
                </Text>
              </HStack>
              <HStack spacing={2} align="start">
                <Text fontSize="xs" color={activeColor} fontWeight="bold">4.</Text>
                <Text fontSize="xs" color={descColor} lineHeight="1.6">
                  {t("audioEq.step4")}
                </Text>
              </HStack>
            </VStack>
          </VStack>
        </LiquidGlassCard>
      </VStack>

      {/* 保存为预设 模态框 */}
      <Modal isOpen={saveModalOpen} onClose={onSaveModalClose} isCentered size="sm">
        <ModalOverlay bg="blackAlpha.600" />
        <ModalContent bg={useColorModeValue("white", "#1a1a1a")} border="1px solid" borderColor={useColorModeValue("gray.100", "#333")}>
          <ModalHeader fontSize="md" color={headingColor}>保存为预设</ModalHeader>
          <ModalCloseButton color={descColor} />
          <ModalBody>
            <VStack spacing={3}>
              <Text fontSize="sm" color={descColor}>
                将当前的频段设置保存为新预设
              </Text>
              <Input
                placeholder="输入预设名称"
                value={presetName}
                onChange={(e) => setPresetName(e.target.value)}
                onKeyDown={(e) => { if (e.key === "Enter") handleSavePreset(); }}
                autoFocus
                focusBorderColor={activeColor}
              />
            </VStack>
          </ModalBody>
          <ModalFooter>
            <Button size="sm" variant="ghost" color={descColor} onClick={onSaveModalClose} mr={2}>
              取消
            </Button>
            <Button size="sm" bg={activeColor} color="white" _hover={{ opacity: 0.9 }} onClick={handleSavePreset} leftIcon={<Save size={14} />}>
              保存
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>
    </Box>
  );
}
