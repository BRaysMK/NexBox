import {
  Box,
  Heading,
  Text,
  VStack,
  HStack,
  useColorModeValue,
  Grid,
  Button,
  useToast,
} from "@chakra-ui/react";
import { useAppStartup } from "@/contexts/app-startup-context";
import { useBackground } from "@/contexts/background-context";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import {
  Cpu,
  Monitor,
  MemoryStick as Ram,
  Database,
  CircuitBoard,
  HardDrive,
  Volume2,
  Wifi,
  Download,
  Trash2,
} from "lucide-react";
import { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { useHardwareReportExport } from "@/lib/use-hardware-report-export";

interface DisplayInfo {
  name: string;
  value: string;
}

interface MemoryStatus {
  total: number;
  available: number;
  used: number;
  usage_percent: number;
}

interface DiskInfo {
  name: string;
  total_gb: number;
  available_gb: number;
  used_gb: number;
  usage_percent: number;
}

function Sparkline({ data, color }: { data: number[]; color: string }) {
  if (data.length < 2) return null;

  const width = 120;
  const height = 40;
  const maxVal = Math.max(...data, 100);
  const minVal = Math.min(...data, 0);
  const range = maxVal - minVal || 1;

  const points = data.map((val, i) => {
    const x = (i / (data.length - 1)) * width;
    const y = height - ((val - minVal) / range) * height;
    return `${x},${y}`;
  }).join(" ");

  return (
    <svg width={width} height={height} style={{ overflow: "visible" }}>
      <polyline
        points={points}
        fill="none"
        stroke={color}
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        opacity={0.8}
      />
      <defs>
        <linearGradient id={`grad-${color.replace("#", "")}`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity="0.3" />
          <stop offset="100%" stopColor={color} stopOpacity="0" />
        </linearGradient>
      </defs>
      <polygon
        points={`0,${height} ${points.split(" ").join(" ")} ${width},${height}`}
        fill={`url(#grad-${color.replace("#", "")})`}
      />
    </svg>
  );
}

function StatCard({
  title,
  value,
  subValue,
  color,
  sparklineData,
  icon: IconComponent,
  cardBg,
  textColor,
  subTextColor,
  liquidGlassEnabled,
}: {
  title: string;
  value: string;
  subValue?: string;
  color: string;
  sparklineData: number[];
  icon: React.ElementType;
  cardBg: string;
  textColor: string;
  subTextColor: string;
  liquidGlassEnabled: boolean;
}) {
  const cardContent = (
    <Box position="relative" overflow="hidden" p={5} height="140px">
      <VStack align="start" spacing={2} position="relative" zIndex={2}>
        <HStack spacing={2}>
          <IconComponent size={16} color={color} />
          <Text fontSize="sm" color={subTextColor} fontWeight="medium">
            {title}
          </Text>
        </HStack>
        <HStack spacing={2} align="baseline">
          <Text fontSize="3xl" fontWeight="bold" color={textColor}>
            {value}
          </Text>
          {subValue && (
            <Text fontSize="xs" color={subTextColor}>
              {subValue}
            </Text>
          )}
        </HStack>
      </VStack>
      <Box position="absolute" bottom={2} left={2} zIndex={1}>
        <Sparkline data={sparklineData} color={color} />
      </Box>
    </Box>
  );

  if (liquidGlassEnabled) {
    return (
      <LiquidGlassCard
        borderRadius="xl"
        overflow="hidden"
        position="relative"
        borderColor="white"
      >
        {cardContent}
      </LiquidGlassCard>
    );
  }

  return (
    <Box
      bg={cardBg}
      borderRadius="xl"
      border="1px solid"
      borderColor="white"
      overflow="hidden"
      position="relative"
    >
      {cardContent}
    </Box>
  );
}

const MARQUEE_STYLE_ID = "nexbox-marquee-keyframes";

function MarqueeText({ text, color }: { text: string; color: string }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const textRef = useRef<HTMLSpanElement>(null);
  const [needsScroll, setNeedsScroll] = useState(false);

  // Measure overflow after render
  useEffect(() => {
    const el = containerRef.current;
    const txt = textRef.current;
    if (!el || !txt) return;
    const timer = setTimeout(() => {
      setNeedsScroll(txt.scrollWidth > el.clientWidth);
    }, 50);
    return () => clearTimeout(timer);
  }, [text]);

  // Inject keyframes once
  useEffect(() => {
    if (needsScroll && !document.getElementById(MARQUEE_STYLE_ID)) {
      const style = document.createElement("style");
      style.id = MARQUEE_STYLE_ID;
      style.textContent = `
        @keyframes nexbox-marquee {
          0% { transform: translateX(0); }
          100% { transform: translateX(-50%); }
        }
      `;
      document.head.appendChild(style);
    }
  }, [needsScroll]);

  const animationStyle = needsScroll
    ? { animation: "nexbox-marquee 12s linear infinite" }
    : undefined;

  return (
    <Box ref={containerRef} flex={1} overflow="hidden" textAlign="right" whiteSpace="nowrap">
      <Box
        as="span"
        ref={textRef}
        display="inline-block"
        whiteSpace="nowrap"
        fontSize="sm"
        fontWeight="medium"
        color={color}
        sx={animationStyle}
      >
        {text}
        {needsScroll && (
          <Box as="span" ml={6}>
            {text}
          </Box>
        )}
      </Box>
    </Box>
  );
}

function DetailCard({
  title,
  icon: IconComponent,
  info,
  type,
  cardBg,
  textColor,
  subTextColor,
  liquidGlassEnabled,
}: {
  title: string;
  icon: React.ElementType;
  info: DisplayInfo[];
  type: string;
  cardBg: string;
  textColor: string;
  subTextColor: string;
  liquidGlassEnabled: boolean;
}) {
  const iconColor =
    type === "cpu"
      ? "#3b82f6"
      : type === "gpu"
        ? "#22c55e"
        : type === "memory"
          ? "#06b6d4"
          : type === "storage"
            ? "#a855f7"
            : type === "sound"
              ? "#f97316"
              : type === "network"
                ? "#14b8a6"
                : "#f59e0b";

  const cardContent = (
    <Box position="relative" overflow="hidden" p={5} minH="140px">
      <VStack align="start" spacing={3} position="relative" zIndex={2}>
        <HStack spacing={2}>
          <IconComponent size={18} color={iconColor} />
          <Text fontSize="md" fontWeight="bold" color={textColor}>
            {title}
          </Text>
        </HStack>

        <VStack align="start" spacing={1.5} width="full">
          {info.map((item, index) => (
            <HStack key={index} justify="space-between" width="full" spacing={3}>
              <Text fontSize="sm" color={subTextColor} noOfLines={1} flexShrink={0} maxW="35%">
                {item.name}
              </Text>
              <MarqueeText text={item.value} color={textColor} />
            </HStack>
          ))}
        </VStack>
      </VStack>
    </Box>
  );

  if (liquidGlassEnabled) {
    return (
      <LiquidGlassCard
        borderRadius="xl"
        overflow="hidden"
        position="relative"
        borderColor="white"
      >
        {cardContent}
      </LiquidGlassCard>
    );
  }

  return (
    <Box
      bg={cardBg}
      borderRadius="xl"
      border="1px solid"
      borderColor="white"
      overflow="hidden"
      position="relative"
    >
      {cardContent}
    </Box>
  );
}

export default function HardwarePage() {
  const { hardwareInfo } = useAppStartup();
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const { exportReport, isExporting } = useHardwareReportExport();
  
  const cardBg = useColorModeValue("white", "#111111");
  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const btnBorderColor = useColorModeValue("gray.300", "#333333");

  const [cpuLoad, setCpuLoad] = useState<number | null>(null);
  const [cpuTemp, setCpuTemp] = useState<number | null>(null);
  const [gpuTemps, setGpuTemps] = useState<number[]>([]);
  const [gpuUsages, setGpuUsages] = useState<number[]>([]);
  const [memoryStatus, setMemoryStatus] = useState<MemoryStatus | null>(null);
  const [diskStatus, setDiskStatus] = useState<DiskInfo | null>(null);

  const [cpuSparkline, setCpuSparkline] = useState<number[]>(Array(20).fill(0));
  const [gpuSparkline, setGpuSparkline] = useState<number[]>(Array(20).fill(0));
  const [memSparkline, setMemSparkline] = useState<number[]>(Array(20).fill(0));
  const [storageSparkline, setStorageSparkline] = useState<number[]>(Array(20).fill(0));

  const isMounted = useRef(true);
  const intervalRef = useRef<NodeJS.Timeout | null>(null);

  useEffect(() => {
    isMounted.current = true;

    const fetchSensorData = async () => {
      if (!isMounted.current) return;

      try {
        // 从悬窗缓存读取实时数据（后台线程每1秒更新，无需等待 PowerShell）
        const overlay = await invoke<{
          cpu_usage: number | null;
          cpu_temp: number | null;
          gpu_temp: number | null;
          gpu_usage: number | null;
          memory_usage: number | null;
        }>("get_overlay_hardware_data");
        if (!isMounted.current) return;

        const cpuLoadVal = overlay.cpu_usage ?? null;
        const cpuTempVal = overlay.cpu_temp ?? null;
        const gpuTempVal = overlay.gpu_temp ?? null;
        const gpuUsageVal = overlay.gpu_usage ?? null;
        const memPercent = overlay.memory_usage ?? null;

        if (cpuLoadVal !== null) {
          setCpuLoad(cpuLoadVal);
          setCpuSparkline((prev) => [...prev.slice(1), cpuLoadVal]);
        }
        if (cpuTempVal !== null) {
          setCpuTemp(Math.round(cpuTempVal));
        }
        if (gpuTempVal !== null) {
          setGpuTemps([gpuTempVal]);
        }
        if (gpuUsageVal !== null) {
          setGpuUsages([gpuUsageVal]);
          setGpuSparkline((prev) => [...prev.slice(1), gpuUsageVal]);
        }
        if (memPercent !== null) {
          setMemSparkline((prev) => [...prev.slice(1), Math.round(memPercent)]);
        }

        // 内存和磁盘的详情（非百分比信息）仍需单独查询
        const memResult = await invoke<MemoryStatus>("get_memory_status");
        if (!isMounted.current) return;
        if (memResult) {
          setMemoryStatus(memResult);
          if (memPercent === null) {
            setMemSparkline((prev) => [...prev.slice(1), Math.round(memResult.usage_percent)]);
          }
        } else if (memPercent !== null) {
          setMemoryStatus({ usage_percent: memPercent, used: 0, total: 0 } as MemoryStatus);
        }

        const diskResult = await invoke<DiskInfo>("get_disk_status");
        if (!isMounted.current) return;
        if (diskResult) {
          setDiskStatus(diskResult);
          setStorageSparkline((prev) => [...prev.slice(1), Math.round(diskResult.usage_percent)]);
        }
      } catch (error) {
        console.error("Failed to fetch sensor data:", error);
      }
    };

    fetchSensorData();
    intervalRef.current = setInterval(fetchSensorData, 2000);

    return () => {
      isMounted.current = false;
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };
  }, []); // 不依赖 hardwareInfo，立即开始轮询

  const gpuTemp = gpuTemps[0] ?? null;
  const gpuUsage = gpuUsages[0] ?? null;
  const memUsage = memoryStatus ? Math.round(memoryStatus.usage_percent) : null;
  const memUsed = memoryStatus ? (memoryStatus.used / 1024).toFixed(1) : "--";
  const memTotal = memoryStatus ? (memoryStatus.total / 1024).toFixed(1) : "--";
  const diskUsage = diskStatus ? Math.round(diskStatus.usage_percent) : null;
  const diskUsed = diskStatus ? diskStatus.used_gb.toFixed(1) : "--";
  const diskTotal = diskStatus ? diskStatus.total_gb.toFixed(1) : "--";

  const cpuDisplayInfo: DisplayInfo[] = hardwareInfo ? [
    { name: t("hardware.model"), value: hardwareInfo.cpu.name },
    {
      name: t("hardware.coresThreads"),
      value: `${hardwareInfo.cpu.cores} ${t("hardware.cores")} ${hardwareInfo.cpu.threads} ${t("hardware.threads")}`,
    },
    {
      name: t("hardware.baseClock"),
      value: `${(hardwareInfo.cpu.max_clock_speed / 1000).toFixed(1)} GHz`,
    },
    {
      name: t("hardware.l3Cache"),
      value: `${(hardwareInfo.cpu.l3_cache_size / 1024).toFixed(0)} MB`,
    },
  ] : [];

  const gpuDisplayInfos: DisplayInfo[][] = hardwareInfo ? hardwareInfo.gpu.map((gpu) => [
    { name: t("hardware.model"), value: gpu.name },
    { name: t("hardware.vendor"), value: gpu.vendor },
    { name: t("hardware.memory"), value: `${gpu.memory_gb.toFixed(1)} GB` },
    { name: t("hardware.driverVersion"), value: gpu.driver_version },
  ]) : [];

  const totalCapacity = hardwareInfo ? hardwareInfo.memory.reduce((sum, mem) => sum + mem.capacity_gb, 0) : 0;
  const memoryDisplayInfo: DisplayInfo[] = hardwareInfo ? [
    { name: t("hardware.totalCapacity"), value: `${totalCapacity.toFixed(0)} GB` },
    { name: t("hardware.speed"), value: hardwareInfo.memory.length > 0 ? `${hardwareInfo.memory[0].speed_mhz} MHz` : "--" },
    { name: t("hardware.count"), value: `${hardwareInfo.memory.length}` },
  ] : [];

  const storageDisplayInfo: DisplayInfo[] = hardwareInfo ? hardwareInfo.disk.map((disk, i) => ({
    name: `${t("hardware.storage")} ${i + 1}`,
    value: disk,
  })) : [];

  const motherboardDisplayInfo: DisplayInfo[] = hardwareInfo ? [
    { name: t("hardware.model"), value: hardwareInfo.motherboard },
  ] : [];

  const soundCardDisplayInfos: DisplayInfo[][] = hardwareInfo ? (hardwareInfo.sound_card || []).map((card) => [
    { name: t("hardware.model"), value: card.name },
    { name: t("hardware.manufacturer"), value: card.manufacturer },
  ]) : [];

  const networkCardDisplayInfos: DisplayInfo[][] = hardwareInfo ? (hardwareInfo.network_card || []).map((card) => [
    { name: t("hardware.model"), value: card.name },
    { name: t("hardware.manufacturer"), value: card.manufacturer },
    { name: t("hardware.adapterType"), value: card.adapter_type },
    { name: t("hardware.macAddress"), value: card.mac_address },
    { name: t("hardware.linkSpeed"), value: card.speed_mbps > 0 ? `${card.speed_mbps} Mbps` : "--" },
  ]) : [];

  return (
    <Box pt={8}>
      <HStack justify="space-between" mb={6}>
        <Heading size="lg" color={headingColor}>
          {t("hardware.title")}
        </Heading>
        <HStack gap={2}>
          <Button
            leftIcon={<Trash2 size={16} />}
            size="sm"
            variant="outline"
            colorScheme="red"
            color="#e74c3c"
            borderColor="rgba(231,76,60,0.3)"
            _hover={{ bg: "rgba(231,76,60,0.1)" }}
            onClick={async () => {
              try {
                await invoke("clear_hardware_data");
                toast({
                  title: "硬件数据已清除",
                  status: "success",
                  duration: 3000,
                  isClosable: true,
                });
              } catch (e) {
                toast({
                  title: "清除失败",
                  description: String(e),
                  status: "error",
                  duration: 3000,
                  isClosable: true,
                });
              }
            }}
          >
            清除数据
          </Button>
          <Button
            leftIcon={<Download size={16} />}
            size="sm"
            variant="outline"
            color={headingColor}
            borderColor={btnBorderColor}
            onClick={exportReport}
            isLoading={isExporting}
          >
            {t("hardwareReport.export") || "导出报告"}
          </Button>
        </HStack>
      </HStack>

      <VStack spacing={6} align="stretch">
        <Grid
          templateColumns={{
            base: "repeat(2, 1fr)",
            md: "repeat(4, 1fr)",
          }}
          gap={4}
        >
          <StatCard
            title="CPU"
            value={`${cpuLoad ?? "--"}%`}
            subValue={cpuTemp !== null ? `${t("hardware.temperature")} ${Math.round(cpuTemp)}${t("hardware.temperatureUnit")}` : undefined}
            color="#3b82f6"
            sparklineData={cpuSparkline}
            icon={Cpu}
            cardBg={cardBg}
            textColor={textColor}
            subTextColor={subTextColor}
            liquidGlassEnabled={liquidGlassEnabled}
          />
          <StatCard
            title="GPU"
            value={`${gpuUsage ?? "--"}%`}
            subValue={gpuTemp !== null ? `${t("hardware.temperature")} ${Math.round(gpuTemp)}${t("hardware.temperatureUnit")}` : undefined}
            color="#22c55e"
            sparklineData={gpuSparkline}
            icon={Monitor}
            cardBg={cardBg}
            textColor={textColor}
            subTextColor={subTextColor}
            liquidGlassEnabled={liquidGlassEnabled}
          />
          <StatCard
            title={t("hardware.ram")}
            value={`${memUsage ?? "--"}%`}
            subValue={`${memUsed} / ${memTotal} GB`}
            color="#06b6d4"
            sparklineData={memSparkline}
            icon={Ram}
            cardBg={cardBg}
            textColor={textColor}
            subTextColor={subTextColor}
            liquidGlassEnabled={liquidGlassEnabled}
          />
          <StatCard
            title={t("hardware.storage")}
            value={`${diskUsage ?? "--"}%`}
            subValue={`${diskUsed} / ${diskTotal} GB`}
            color="#a855f7"
            sparklineData={storageSparkline}
            icon={Database}
            cardBg={cardBg}
            textColor={textColor}
            subTextColor={subTextColor}
            liquidGlassEnabled={liquidGlassEnabled}
          />
        </Grid>

        <Grid
          templateColumns={{
            base: "1fr",
            md: "repeat(2, 1fr)",
            lg: "repeat(3, 1fr)",
          }}
          gap={4}
        >
          <DetailCard
            title={t("hardware.processor")}
            icon={Cpu}
            info={cpuDisplayInfo}
            type="cpu"
            cardBg={cardBg}
            textColor={textColor}
            subTextColor={subTextColor}
            liquidGlassEnabled={liquidGlassEnabled}
          />
          {gpuDisplayInfos.map((gpuInfo, i) => (
            <DetailCard
              key={i}
              title={t("hardware.gpu")}
              icon={Monitor}
              info={gpuInfo}
              type="gpu"
              cardBg={cardBg}
              textColor={textColor}
              subTextColor={subTextColor}
              liquidGlassEnabled={liquidGlassEnabled}
            />
          ))}
          <DetailCard
            title={t("hardware.ram")}
            icon={Ram}
            info={memoryDisplayInfo}
            type="memory"
            cardBg={cardBg}
            textColor={textColor}
            subTextColor={subTextColor}
            liquidGlassEnabled={liquidGlassEnabled}
          />
          <DetailCard
            title={t("hardware.motherboard")}
            icon={CircuitBoard}
            info={motherboardDisplayInfo}
            type="motherboard"
            cardBg={cardBg}
            textColor={textColor}
            subTextColor={subTextColor}
            liquidGlassEnabled={liquidGlassEnabled}
          />
          {storageDisplayInfo.length > 0 && (
            <DetailCard
              title={t("hardware.storage")}
              icon={HardDrive}
              info={storageDisplayInfo}
              type="storage"
              cardBg={cardBg}
              textColor={textColor}
              subTextColor={subTextColor}
              liquidGlassEnabled={liquidGlassEnabled}
            />
          )}
          {soundCardDisplayInfos.map((info, i) => (
            <DetailCard
              key={`sound-${i}`}
              title={t("hardware.soundCard")}
              icon={Volume2}
              info={info}
              type="sound"
              cardBg={cardBg}
              textColor={textColor}
              subTextColor={subTextColor}
              liquidGlassEnabled={liquidGlassEnabled}
            />
          ))}
          {networkCardDisplayInfos.map((info, i) => (
            <DetailCard
              key={`network-${i}`}
              title={t("hardware.networkCard")}
              icon={Wifi}
              info={info}
              type="network"
              cardBg={cardBg}
              textColor={textColor}
              subTextColor={subTextColor}
              liquidGlassEnabled={liquidGlassEnabled}
            />
          ))}
        </Grid>
      </VStack>
    </Box>
  );
}
