import {
  Box,
  Text,
  Heading,
  VStack,
  HStack,
  SimpleGrid,
  useColorModeValue,
  IconButton,
  Button,
  Badge,
  Tooltip,
  Divider,
} from "@chakra-ui/react";
import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import {
  ArrowLeft,
  Gauge,
  Download,
  Upload,
  Activity,
  Timer,
  RefreshCw,
  Network,
} from "lucide-react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import { useNavigate } from "react-router-dom";

interface SpeedTestProgress {
  stage: string; // ping / download / upload / done
  pingMs: number;
  jitterMs: number;
  packetLossPct: number;
  downloadMbps: number;
  uploadMbps: number;
  progressPct: number;
  server: string;
  message: string;
}

type Stage = "idle" | "ping" | "download" | "upload" | "done";

// 最多保留的采样点数量
const MAX_POINTS = 120;

// 固定参数：默认最大线程（16）
const DEFAULT_THREADS = 16;
const DEFAULT_DURATION = 6;

function formatSpeed(mbps: number): string {
  if (mbps <= 0) return "0";
  if (mbps < 1) return mbps.toFixed(2);
  if (mbps < 100) return mbps.toFixed(1);
  return mbps.toFixed(0);
}

function StageBadge({ stage }: { stage: Stage }) {
  const { t } = useTranslation();
  const { getActiveColor } = useThemeColor();
  const map: Record<Stage, { label: string; cs: string }> = {
    idle: { label: t("speedtest.idle"), cs: "gray" },
    ping: { label: t("speedtest.ping"), cs: "blue" },
    download: { label: t("speedtest.download"), cs: "green" },
    upload: { label: t("speedtest.upload"), cs: "purple" },
    done: { label: t("speedtest.done"), cs: "teal" },
  };
  const info = map[stage];
  if (stage === "idle") {
    return (
      <Badge px={2} py={0.5} borderRadius="full" fontSize="xs" color={getActiveColor()} bg={`${getActiveColor()}22`}>
        {info.label}
      </Badge>
    );
  }
  return (
    <Badge colorScheme={info.cs} px={2} py={0.5} borderRadius="full" fontSize="xs">
      {info.label}
    </Badge>
  );
}

// 大数字仪表（下载/上传）
function SpeedMeter({
  icon,
  title,
  value,
  accent,
  active,
}: {
  icon: React.ReactNode;
  title: string;
  value: number;
  accent: string;
  active: boolean;
}) {
  const { getActiveColor } = useThemeColor();
  const subColor = useColorModeValue("gray.500", "#888888");
  const valueColor = useColorModeValue("#000000", "#ffffff");
  const activeColor = getActiveColor();
  return (
    <Box textAlign="center" opacity={active ? 1 : 0.5} transition="opacity 0.3s">
      <HStack justify="center" spacing={2} color={active ? accent : subColor}>
        {icon}
        <Text fontSize="sm" fontWeight="medium" color={subColor}>{title}</Text>
      </HStack>
      <HStack justify="center" align="baseline" spacing={1} mt={1}>
        <Text
          fontSize="5xl"
          fontWeight="bold"
          color={active ? activeColor : valueColor}
          lineHeight="1"
          style={{ fontVariantNumeric: "tabular-nums" }}
          transition="color 0.3s"
        >
          {formatSpeed(value)}
        </Text>
        <Text fontSize="md" color={subColor}>Mbps</Text>
      </HStack>
    </Box>
  );
}

// ===== SVG 折线图组件（实时速度曲线，大尺寸） =====
function SpeedChart({
  downloadPoints,
  uploadPoints,
  stage,
  activeColor,
}: {
  downloadPoints: number[];
  uploadPoints: number[];
  stage: Stage;
  activeColor: string;
}) {
  const chartBg = useColorModeValue("rgba(0,0,0,0.02)", "rgba(255,255,255,0.02)");
  const gridColor = useColorModeValue("rgba(0,0,0,0.08)", "rgba(255,255,255,0.08)");
  const labelColor = useColorModeValue("#888888", "#666666");
  const W = 720;
  const H = 460;
  const PAD_X = 12;
  const PAD_TOP = 28;
  const PAD_BOTTOM = 40;

  const data = stage === "download" ? downloadPoints : stage === "upload" ? uploadPoints : [];
  const showDownload = stage === "done" ? true : stage === "download" || stage === "upload";
  const showUpload = stage === "done" ? true : stage === "upload";

  // 计算 Y 轴最大刻度（取两条曲线的最大值，向上取整到 10 的倍数）
  const allValues = [...downloadPoints, ...uploadPoints];
  const rawMax = allValues.length > 0 ? Math.max(...allValues) : 1;
  const maxY = Math.max(Math.ceil((rawMax * 1.15) / 10) * 10, 10);

  const toX = (i: number, len: number) => {
    if (len <= 1) return PAD_X;
    return PAD_X + (i / (len - 1)) * (W - PAD_X * 2);
  };
  const toY = (v: number) => {
    const ratio = v / maxY;
    return H - PAD_BOTTOM - ratio * (H - PAD_TOP - PAD_BOTTOM);
  };

  const buildPath = (points: number[]): string => {
    if (points.length === 0) return "";
    const parts = points.map((v, i) => `${i === 0 ? "M" : "L"}${toX(i, points.length).toFixed(1)},${toY(v).toFixed(1)}`);
    return parts.join(" ");
  };
  const buildAreaPath = (points: number[]): string => {
    if (points.length === 0) return "";
    const line = buildPath(points);
    const lastX = toX(points.length - 1, points.length).toFixed(1);
    const firstX = toX(0, points.length).toFixed(1);
    const baseY = (H - PAD_BOTTOM).toFixed(1);
    return `${line} L${lastX},${baseY} L${firstX},${baseY} Z`;
  };

  const downloadPath = showDownload ? buildPath(downloadPoints) : "";
  const uploadPath = showUpload ? buildPath(uploadPoints) : "";
  const downloadArea = showDownload ? buildAreaPath(downloadPoints) : "";
  const uploadArea = showUpload ? buildAreaPath(uploadPoints) : "";

  // 网格刻度（更多细分，测速网站风格）
  const gridLines = [0, 0.2, 0.4, 0.6, 0.8, 1].map((r) => {
    const y = H - PAD_BOTTOM - r * (H - PAD_TOP - PAD_BOTTOM);
    const val = maxY * r;
    return { y, val };
  });

  return (
    <Box
      borderRadius="lg"
      bg={chartBg}
      p={2}
      border="1px solid"
      borderColor={gridColor}
      overflow="hidden"
      w="full"
    >
      <svg viewBox={`0 0 ${W} ${H}`} width="100%" style={{ display: "block" }}>
        <defs>
          <linearGradient id="dlArea" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={activeColor} stopOpacity="0.35" />
            <stop offset="100%" stopColor={activeColor} stopOpacity="0.02" />
          </linearGradient>
          <linearGradient id="ulArea" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="#A78BFA" stopOpacity="0.35" />
            <stop offset="100%" stopColor="#A78BFA" stopOpacity="0.02" />
          </linearGradient>
        </defs>

        {/* 网格线 */}
        {gridLines.map((g, i) => (
          <g key={i}>
            <line x1={PAD_X} x2={W - PAD_X} y1={g.y} y2={g.y} stroke={gridColor} strokeWidth="1" />
            <text x={W - PAD_X} y={g.y - 6} fontSize="12" fill={labelColor} textAnchor="end">
              {g.val}
            </text>
          </g>
        ))}

        {/* 面积填充 */}
        {downloadArea && (
          <path d={downloadArea} fill="url(#dlArea)" style={{ transition: "d 0.18s linear" }} />
        )}
        {uploadArea && (
          <path d={uploadArea} fill="url(#ulArea)" style={{ transition: "d 0.18s linear" }} />
        )}

        {/* 折线 */}
        {downloadPath && (
          <path
            d={downloadPath}
            fill="none"
            stroke={activeColor}
            strokeWidth="2.5"
            strokeLinecap="round"
            strokeLinejoin="round"
            style={{ transition: "d 0.18s linear", shapeRendering: "auto" }}
          />
        )}
        {uploadPath && (
          <path
            d={uploadPath}
            fill="none"
            stroke="#A78BFA"
            strokeWidth="2.5"
            strokeLinecap="round"
            strokeLinejoin="round"
            style={{ transition: "d 0.18s linear", shapeRendering: "auto" }}
          />
        )}

        {/* 最新点高亮 */}
        {data.length > 0 && (
          <circle
            cx={toX(data.length - 1, data.length)}
            cy={toY(data[data.length - 1])}
            r="5"
            fill={stage === "upload" ? "#A78BFA" : activeColor}
            stroke="#ffffff"
            strokeWidth="2"
            style={{ transition: "cx 0.18s linear, cy 0.18s linear" }}
          />
        )}
      </svg>
    </Box>
  );
}

function MetricCard({
  label,
  value,
  unit,
  icon,
  accent,
}: {
  label: string;
  value: string;
  unit: string;
  icon: React.ReactNode;
  accent: string;
}) {
  const subColor = useColorModeValue("gray.600", "#999999");
  return (
    <LiquidGlassCard p={4} textAlign="center">
      <VStack spacing={1}>
        <Box color={accent}>{icon}</Box>
        <HStack spacing={1} align="baseline">
          <Text
            fontSize="2xl"
            fontWeight="bold"
            color={useColorModeValue("#000000", "#ffffff")}
            style={{ fontVariantNumeric: "tabular-nums" }}
          >
            {value}
          </Text>
          <Text fontSize="xs" color={subColor}>{unit}</Text>
        </HStack>
        <Text fontSize="xs" color={subColor}>{label}</Text>
      </VStack>
    </LiquidGlassCard>
  );
}

export default function SpeedTestPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { getActiveColor, getContrastTextColor, getHoverColor } = useThemeColor();

  const [stage, setStage] = useState<Stage>("idle");
  const [ping, setPing] = useState(0);
  const [jitter, setJitter] = useState(0);
  const [loss, setLoss] = useState(0);
  const [download, setDownload] = useState(0);
  const [upload, setUpload] = useState(0);
  const [message, setMessage] = useState("");

  // 折线图数据点
  const [dlPoints, setDlPoints] = useState<number[]>([]);
  const [ulPoints, setUlPoints] = useState<number[]>([]);
  const dlRef = useRef<number[]>([]);
  const ulRef = useRef<number[]>([]);
  const unlistenRef = useRef<UnlistenFn | null>(null);
  const prevStageRef = useRef<Stage>("idle");

  const activeColor = getActiveColor();
  const contrastText = getContrastTextColor();
  const hoverBg = getHoverColor(false);

  // 订阅测速进度
  useEffect(() => {
    let cancelled = false;
    const setup = async () => {
      const unlisten = await listen<SpeedTestProgress>("speedtest-progress", (event) => {
        if (cancelled) return;
        const d = event.payload;
        setPing(d.pingMs);
        setJitter(d.jitterMs);
        setLoss(d.packetLossPct);
        setDownload(d.downloadMbps);
        setUpload(d.uploadMbps);
        setMessage(d.message);
        const s = d.stage as Stage;
        setStage(s);

        // 阶段切换时重置对应曲线
        if (s === "download" && prevStageRef.current !== "download") {
          dlRef.current = [];
        }
        if (s === "upload" && prevStageRef.current !== "upload") {
          ulRef.current = [];
        }

        // 收集数据点：下载阶段收集下载曲线，上传阶段收集上传曲线
        if (s === "download" && d.downloadMbps > 0) {
          dlRef.current.push(d.downloadMbps);
          if (dlRef.current.length > MAX_POINTS) dlRef.current.shift();
          setDlPoints([...dlRef.current]);
        }
        if (s === "upload" && d.uploadMbps > 0) {
          ulRef.current.push(d.uploadMbps);
          if (ulRef.current.length > MAX_POINTS) ulRef.current.shift();
          setUlPoints([...ulRef.current]);
        }
        prevStageRef.current = s;
      });
      if (!cancelled) {
        unlistenRef.current = unlisten;
      } else {
        unlisten();
      }
    };
    setup();
    return () => {
      cancelled = true;
      unlistenRef.current?.();
      unlistenRef.current = null;
    };
  }, []);

  const handleStart = useCallback(async () => {
    try {
      setStage("ping");
      setPing(0);
      setJitter(0);
      setLoss(0);
      setDownload(0);
      setUpload(0);
      setDlPoints([]);
      setUlPoints([]);
      dlRef.current = [];
      ulRef.current = [];
      prevStageRef.current = "ping";
      const config = { threads: DEFAULT_THREADS, durationSecs: DEFAULT_DURATION };
      await invoke("start_speedtest", { config });
    } catch (err) {
      console.error("start speedtest failed:", err);
      setStage("idle");
      setMessage(String(err));
    }
  }, []);

  const handleStop = useCallback(async () => {
    try {
      await invoke("stop_speedtest");
    } catch (err) {
      console.error("stop speedtest failed:", err);
    }
  }, []);

  const headingColor = useColorModeValue("#000000", "#ffffff");
  const subColor = useColorModeValue("gray.500", "#888888");
  const dividerColor = useColorModeValue("gray.200", "#333333");

  const running = stage === "ping" || stage === "download" || stage === "upload";

  return (
    <Box pt={8}>
      <VStack align="stretch" spacing={6}>
        <HStack justify="space-between" align="center">
          <HStack spacing={3}>
            <IconButton
              aria-label="back"
              icon={<ArrowLeft size={20} />}
              variant="ghost"
              onClick={() => navigate(-1)}
            />
            <HStack spacing={2}>
              <Gauge color={activeColor} />
              <Heading size="lg" color={headingColor}>
                {t("speedtest.title")}
              </Heading>
            </HStack>
          </HStack>
          <StageBadge stage={stage} />
        </HStack>

        {/* 主测速卡片：液态玻璃效果（跟随全局开关），左侧下载/上传大数字 + 右侧超大折线图 */}
        <LiquidGlassCard p={6}>
          <SimpleGrid columns={{ base: 1, lg: 4 }} spacing={6} alignItems="center">
            {/* 左侧：下载/上传 */}
            <VStack spacing={6} align="stretch">
              <SpeedMeter
                icon={<Download size={18} />}
                title={t("speedtest.download")}
                value={download}
                accent="#38A169"
                active={stage === "download" || stage === "done"}
              />
              <Divider borderColor={dividerColor} />
              <SpeedMeter
                icon={<Upload size={18} />}
                title={t("speedtest.upload")}
                value={upload}
                accent="#A78BFA"
                active={stage === "upload" || stage === "done"}
              />
            </VStack>

            {/* 右侧：超大折线图 */}
            <Box gridColumn={{ lg: "span 3" }}>
              <SpeedChart
                downloadPoints={dlPoints}
                uploadPoints={ulPoints}
                stage={stage}
                activeColor={activeColor}
              />
            </Box>
          </SimpleGrid>
        </LiquidGlassCard>

        {/* 延迟/抖动/丢包：独立卡片区 */}
        <SimpleGrid columns={{ base: 1, sm: 3 }} spacing={4}>
          <MetricCard
            label={t("speedtest.ping")}
            value={ping > 0 ? ping.toFixed(1) : "--"}
            unit="ms"
            icon={<Timer size={18} />}
            accent={activeColor}
          />
          <MetricCard
            label={t("speedtest.jitter")}
            value={jitter > 0 ? jitter.toFixed(1) : "--"}
            unit="ms"
            icon={<Activity size={18} />}
            accent="#DD6B20"
          />
          <MetricCard
            label={t("speedtest.packetLoss")}
            value={loss > 0 ? loss.toFixed(1) : "0"}
            unit="%"
            icon={<Network size={18} />}
            accent="#E53E3E"
          />
        </SimpleGrid>

        {/* 消息提示 */}
        {message && (
          <Text fontSize="sm" color={subColor} textAlign="center">
            {message}
          </Text>
        )}

        {/* 开始/停止 */}
        <VStack spacing={2}>
          <Tooltip label={t("speedtest.startHint")} isDisabled={!running}>
            <Button
              size="lg"
              bg={running ? "#E53E3E" : activeColor}
              color={running ? "#ffffff" : contrastText}
              _hover={{ bg: running ? "#C53030" : hoverBg }}
              isLoading={running}
              loadingText={t("speedtest.testing")}
              leftIcon={running ? <RefreshCw size={18} /> : <Gauge size={18} />}
              onClick={running ? handleStop : handleStart}
              w={{ base: "full", md: "320px" }}
              h="52px"
              borderRadius="xl"
              boxShadow={running ? "none" : `0 4px 20px ${activeColor}55`}
            >
              {running ? t("speedtest.stop") : t("speedtest.start")}
            </Button>
          </Tooltip>
          <Text fontSize="xs" color={subColor}>
            {t("speedtest.serverNote")}
          </Text>
        </VStack>
      </VStack>
    </Box>
  );
}
