import {
  Box,
  Text,
  Heading,
  VStack,
  HStack,
  Flex,
  useColorModeValue,
  useToast,
  Button,
  Badge,
  Spinner,
  IconButton,
} from "@chakra-ui/react";
import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import {
  ArrowLeft,
  Volume2,
  Power,
  Trash2,
  Download,
  Activity,
  Music,
  Gamepad2,
  Film,
  Mic,
  Headphones,
  Radio,
  Tv,
  Play,
  Square,
  RefreshCw,
  CheckCircle,
  XCircle,
  AlertTriangle,
} from "lucide-react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useThemeColor } from "@/contexts/theme-color-context";

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

// ===== 辅助函数 =====
function formatFreq(freq: number): string {
  if (freq >= 1000) {
    return `${(freq / 1000).toFixed(freq >= 10000 ? 0 : 1)}k`;
  }
  return `${Math.round(freq)}`;
}

function getPresetIcon(name: string) {
  const lower = name.toLowerCase();
  if (lower.includes("music")) return Music;
  if (lower.includes("gam")) return Gamepad2;
  if (lower.includes("mov") || lower.includes("film")) return Film;
  if (lower.includes("voice") || lower.includes("vocal") || lower.includes("transcript")) return Mic;
  if (lower.includes("bass")) return Headphones;
  if (lower.includes("tv")) return Tv;
  if (lower.includes("stream")) return Radio;
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
    const padding = { left: 40, right: 20, top: 20, bottom: 30 };
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
    for (let db = -12; db <= 12; db += 3) {
      const y = padding.top + plotH / 2 - (db / 12) * (plotH / 2);
      ctx.beginPath();
      ctx.moveTo(padding.left, y);
      ctx.lineTo(W - padding.right, y);
      ctx.stroke();
      if (db !== 0) {
        ctx.fillText(`${db > 0 ? "+" : ""}${db}`, 5, y + 3);
      } else {
        ctx.fillText("0", 5, y + 3);
      }
    }

    // 垂直网格线 (频率: 对数刻度)
    const freqMarks = [32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
    const logMin = Math.log10(20);
    const logMax = Math.log10(20000);
    freqMarks.forEach((freq) => {
      const x = padding.left + ((Math.log10(freq) - logMin) / (logMax - logMin)) * plotW;
      ctx.beginPath();
      ctx.moveTo(x, padding.top);
      ctx.lineTo(x, H - padding.bottom);
      ctx.stroke();
      ctx.fillText(formatFreq(freq), x - 10, H - padding.bottom + 15);
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
  }, [bands, lineColor, fillColor, gridColor, textColor, activeColor, zeroLineColor, dotBgColor]);

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

  const cardBg = useColorModeValue("white", "#111111");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const labelColor = useColorModeValue("gray.700", "#e0e0e0");
  const descColor = useColorModeValue("gray.500", "#888888");
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
  const [bands, setBands] = useState<EqBand[]>([]);
  const [loading, setLoading] = useState(true);
  const [installing, setInstalling] = useState(false);
  const [uninstalling, setUninstalling] = useState(false);
  const [startingEngine, setStartingEngine] = useState(false);
  const selectedPresetIdRef = useRef<string>("");

  // 加载数据
  const loadData = useCallback(async () => {
    try {
      setLoading(true);
      const [status, engine, presetList] = await Promise.all([
        invoke<DriverStatus>("check_virtual_audio_driver"),
        invoke<EngineStatus>("get_eq_engine_status"),
        invoke<EqPreset[]>("get_eq_presets"),
      ]);
      setDriverStatus(status);
      setEngineStatus(engine);
      setPresets(presetList);

      // 默认选中第一个预设
      if (presetList.length > 0 && !selectedPresetIdRef.current) {
        const first = presetList[0];
        setSelectedPresetId(first.id);
        selectedPresetIdRef.current = first.id;
        setBands(first.bands);
      }
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
      const needsReboot = result.includes("重启");
      toast({
        title: needsReboot ? t("audioEq.uninstallSuccess") : t("audioEq.uninstallSuccess"),
        description: result,
        status: needsReboot ? "warning" : "success",
        duration: needsReboot ? 8000 : 3000,
        isClosable: true,
      });
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

  // 选择预设
  const handleSelectPreset = async (preset: EqPreset) => {
    setSelectedPresetId(preset.id);
    selectedPresetIdRef.current = preset.id;
    setBands(preset.bands);
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
      return newBands;
    });
  };

  // 重置频段（实时同步）
  const handleResetBands = () => {
    const resetBands = bands.map((b) => ({ ...b, gain: 0 }));
    setBands(resetBands);
    if (engineStatus.running) {
      const bandTuples = resetBands.map((b) => [b.freq, b.gain] as [number, number]);
      invoke("update_eq_bands", { bands: bandTuples }).catch(() => {});
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
        {/* 虚拟声卡管理区域 */}
        <LiquidGlassCard p={4}>
          <VStack align="stretch" spacing={4}>
            <HStack justify="space-between">
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
                  <Power size={20} color={activeColor} />
                </Box>
                <VStack align="start" spacing={0}>
                  <Text fontWeight="bold" fontSize="md" color={headingColor}>
                    {t("audioEq.virtualSoundCard")}
                  </Text>
                  <Text fontSize="xs" color={descColor}>
                    {t("audioEq.virtualSoundCardDesc")}
                  </Text>
                </VStack>
              </HStack>

              {/* 状态指示器 */}
              {driverStatus?.needs_reboot ? (
                <Badge colorScheme="orange" px={3} py={1} borderRadius="full" variant="subtle">
                  <HStack spacing={1}>
                    <AlertTriangle size={12} />
                    <Text fontSize="xs">{t("audioEq.needsReboot")}</Text>
                  </HStack>
                </Badge>
              ) : driverStatus?.installed ? (
                <Badge colorScheme="green" px={3} py={1} borderRadius="full" variant="subtle">
                  <HStack spacing={1}>
                    <CheckCircle size={12} />
                    <Text fontSize="xs">{t("audioEq.installed")}</Text>
                  </HStack>
                </Badge>
              ) : (
                <Badge colorScheme="gray" px={3} py={1} borderRadius="full" variant="subtle">
                  <HStack spacing={1}>
                    <XCircle size={12} />
                    <Text fontSize="xs">{t("audioEq.notInstalled")}</Text>
                  </HStack>
                </Badge>
              )}
            </HStack>

            {(driverStatus?.installed || driverStatus?.needs_reboot) && (
              <HStack spacing={2} fontSize="xs" color={descColor}>
                <Activity size={14} />
                <Text>{driverStatus.device_name}</Text>
                {driverStatus.service_running && (
                  <Badge colorScheme="green" fontSize="2xs" variant="subtle">
                    {t("audioEq.serviceRunning")}
                  </Badge>
                )}
              </HStack>
            )}

            <HStack spacing={3}>
              {!driverStatus?.installed && !driverStatus?.needs_reboot ? (
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
              ) : (
                <Button
                  leftIcon={<Trash2 size={16} />}
                  colorScheme="red"
                  variant="outline"
                  size="sm"
                  onClick={handleUninstall}
                  isLoading={uninstalling}
                  loadingText={t("audioEq.uninstalling")}
                >
                  {t("audioEq.uninstallDriver")}
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

            {driverStatus?.needs_reboot && (
              <HStack spacing={2} p={3} borderRadius="md" bg={warningBg}>
                <AlertTriangle size={16} color="orange" />
                <Text fontSize="xs" color={warningColor}>
                  {t("audioEq.rebootHint")}
                </Text>
              </HStack>
            )}

            {!driverStatus?.installed && !driverStatus?.needs_reboot && (
              <HStack spacing={2} p={3} borderRadius="md" bg={warningBg}>
                <AlertTriangle size={16} color="orange" />
                <Text fontSize="xs" color={warningColor}>
                  {t("audioEq.installHint")}
                </Text>
              </HStack>
            )}
          </VStack>
        </LiquidGlassCard>

        {/* EQ 引擎控制区域 */}
        <LiquidGlassCard p={4}>
          <VStack align="stretch" spacing={4}>
            <HStack justify="space-between">
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
                    {t("audioEq.eqEngine")}
                  </Text>
                  <Text fontSize="xs" color={descColor}>
                    {t("audioEq.eqEngineDesc")}
                  </Text>
                </VStack>
              </HStack>

              {engineStatus.running ? (
                <Badge colorScheme="green" px={3} py={1} borderRadius="full" variant="subtle">
                  <HStack spacing={1}>
                    <Box w={2} h={2} borderRadius="full" bg="green.400" />
                    <Text fontSize="xs">{t("audioEq.running")}</Text>
                  </HStack>
                </Badge>
              ) : (
                <Badge colorScheme="gray" px={3} py={1} borderRadius="full" variant="subtle">
                  <Text fontSize="xs">{t("audioEq.stopped")}</Text>
                </Badge>
              )}
            </HStack>

            <HStack spacing={3}>
              {!engineStatus.running ? (
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
              ) : (
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
            </HStack>

            {!driverStatus?.installed && (
              <Text fontSize="xs" color="orange.500">
                {t("audioEq.installDriverFirst")}
              </Text>
            )}
          </VStack>
        </LiquidGlassCard>

        {/* 预设选择区域 */}
        {presets.length > 0 && (
          <LiquidGlassCard p={4}>
            <VStack align="stretch" spacing={4}>
              <HStack justify="space-between">
                <Text fontWeight="bold" fontSize="md" color={headingColor}>
                  {t("audioEq.presets")}
                </Text>
                <Badge fontSize="xs" colorScheme="gray">
                  {presets.length}
                </Badge>
              </HStack>

              <Flex wrap="wrap" gap={2}>
                {presets.map((preset) => {
                  const Icon = getPresetIcon(preset.name);
                  const isSelected = selectedPresetId === preset.id;
                  return (
                    <Box
                      key={preset.id}
                      onClick={() => handleSelectPreset(preset)}
                      cursor="pointer"
                      px={3}
                      py={2}
                      borderRadius="lg"
                      border="2px solid"
                      borderColor={isSelected ? activeColor : borderColor}
                      bg={isSelected ? `${activeColor}15` : "transparent"}
                      _hover={{
                        borderColor: activeColor,
                        bg: `${activeColor}10`,
                      }}
                      transition="all 0.2s"
                    >
                      <HStack spacing={2}>
                        <Icon size={16} color={isSelected ? activeColor : descColor} />
                        <Text
                          fontSize="xs"
                          fontWeight={isSelected ? "bold" : "medium"}
                          color={isSelected ? activeColor : labelColor}
                        >
                          {preset.name}
                        </Text>
                      </HStack>
                    </Box>
                  );
                })}
              </Flex>
            </VStack>
          </LiquidGlassCard>
        )}

        {/* EQ 频段调音区域 */}
        {bands.length > 0 && (
          <LiquidGlassCard p={4}>
            <VStack align="stretch" spacing={4}>
              <HStack justify="space-between">
                <Text fontWeight="bold" fontSize="md" color={headingColor}>
                  {t("audioEq.equalizer")}
                </Text>
                <Button
                  leftIcon={<RefreshCw size={14} />}
                  size="xs"
                  variant="ghost"
                  onClick={handleResetBands}
                >
                  {t("audioEq.reset")}
                </Button>
              </HStack>

              {/* 频率响应曲线 */}
              <Box
                borderRadius="lg"
                bg={curveBg}
                p={2}
                overflow="hidden"
              >
                <FrequencyResponseCurve bands={bands} />
              </Box>

              {/* 频段滑块 */}
              <HStack
                justify="space-between"
                align="stretch"
                spacing={1}
                px={2}
              >
                {bands.map((band, index) => (
                  <EqBandSlider
                    key={index}
                    band={band}
                    onChange={(gain) => handleBandChange(index, gain)}
                  />
                ))}
              </HStack>

              {/* 频段信息表 */}
              <HStack spacing={1} justify="space-between" px={2}>
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
        )}

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
    </Box>
  );
}
