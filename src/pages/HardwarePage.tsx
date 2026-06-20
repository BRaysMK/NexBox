import {
  Box,
  Heading,
  Text,
  VStack,
  HStack,
  useColorModeValue,
  Grid,
  Spinner,
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
} from "lucide-react";
import { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";

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
  borderColor,
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
  borderColor: string;
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
        transition="all 0.2s"
        _hover={{
          borderColor: color,
          boxShadow: `0 0 20px -5px ${color}40`,
        }}
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
      borderColor={borderColor}
      overflow="hidden"
      position="relative"
      transition="all 0.2s"
      _hover={{
        borderColor: color,
        boxShadow: `0 0 20px -5px ${color}40`,
      }}
    >
      {cardContent}
    </Box>
  );
}

function DetailCard({
  title,
  icon: IconComponent,
  info,
  type,
  cardBg,
  borderColor,
  textColor,
  subTextColor,
  liquidGlassEnabled,
}: {
  title: string;
  icon: React.ElementType;
  info: DisplayInfo[];
  type: string;
  cardBg: string;
  borderColor: string;
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
            <HStack key={index} justify="space-between" width="full" spacing={4}>
              <Text fontSize="sm" color={subTextColor} noOfLines={1}>
                {item.name}
              </Text>
              <Text fontSize="sm" fontWeight="medium" color={textColor} noOfLines={1} textAlign="right">
                {item.value}
              </Text>
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
        transition="all 0.2s"
        _hover={{
          borderColor: iconColor,
          boxShadow: `0 0 20px -5px ${iconColor}40`,
        }}
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
      borderColor={borderColor}
      overflow="hidden"
      position="relative"
      transition="all 0.2s"
      _hover={{
        borderColor: iconColor,
        boxShadow: `0 0 20px -5px ${iconColor}40`,
      }}
    >
      {cardContent}
    </Box>
  );
}

export default function HardwarePage() {
  const { hardwareInfo } = useAppStartup();
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  
  const cardBg = useColorModeValue("white", "#111111");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");

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
      if (!isMounted.current || !hardwareInfo) return;

      try {
        const cpuStatus = await invoke<[number | null, number | null]>("get_lhm_cpu_status");
        if (!isMounted.current) return;
        const [cpuLoadResult, cpuTempResult] = cpuStatus;
        if (cpuLoadResult !== null) {
          setCpuLoad(cpuLoadResult);
          setCpuSparkline((prev) => [...prev.slice(1), cpuLoadResult]);
        }
        if (cpuTempResult !== null) {
          setCpuTemp(Math.round(cpuTempResult));
        }

        const memResult = await invoke<MemoryStatus>("get_memory_status");
        if (!isMounted.current) return;
        if (memResult) {
          setMemoryStatus(memResult);
          setMemSparkline((prev) => [...prev.slice(1), Math.round(memResult.usage_percent)]);
        }

        const diskResult = await invoke<DiskInfo>("get_disk_status");
        if (!isMounted.current) return;
        if (diskResult) {
          setDiskStatus(diskResult);
          setStorageSparkline((prev) => [...prev.slice(1), Math.round(diskResult.usage_percent)]);
        }

        const gpuStatusList = await invoke<[number | null, number | null][]>("get_lhm_gpu_status");
        if (!isMounted.current) return;

        const gpuTempsResult = gpuStatusList.map(([temp]) => temp ?? 0);
        const gpuUsagesResult = gpuStatusList.map(([, usage]) => usage ?? 0);

        setGpuTemps(gpuTempsResult);
        setGpuUsages(gpuUsagesResult);

        if (gpuUsagesResult.length > 0 && gpuUsagesResult[0] !== null) {
          setGpuSparkline((prev) => [...prev.slice(1), gpuUsagesResult[0]]);
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
  }, [hardwareInfo]);

  if (!hardwareInfo) {
    return (
      <Box pt={8} display="flex" justifyContent="center" alignItems="center" minH="50vh">
        <Spinner size="xl" />
      </Box>
    );
  }

  const gpuTemp = gpuTemps[0] ?? null;
  const gpuUsage = gpuUsages[0] ?? null;
  const memUsage = memoryStatus ? Math.round(memoryStatus.usage_percent) : null;
  const memUsed = memoryStatus ? (memoryStatus.used / 1024).toFixed(1) : "--";
  const memTotal = memoryStatus ? (memoryStatus.total / 1024).toFixed(1) : "--";
  const diskUsage = diskStatus ? Math.round(diskStatus.usage_percent) : null;
  const diskUsed = diskStatus ? diskStatus.used_gb.toFixed(1) : "--";
  const diskTotal = diskStatus ? diskStatus.total_gb.toFixed(1) : "--";

  const cpuDisplayInfo: DisplayInfo[] = [
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
  ];

  const gpuDisplayInfos: DisplayInfo[][] = hardwareInfo.gpu.map((gpu) => [
    { name: t("hardware.model"), value: gpu.name },
    { name: t("hardware.vendor"), value: gpu.vendor },
    { name: t("hardware.memory"), value: `${gpu.memory_gb.toFixed(1)} GB` },
    { name: t("hardware.driverVersion"), value: gpu.driver_version },
  ]);

  const totalCapacity = hardwareInfo.memory.reduce((sum, mem) => sum + mem.capacity_gb, 0);
  const memoryDisplayInfo: DisplayInfo[] = [
    { name: t("hardware.totalCapacity"), value: `${totalCapacity.toFixed(0)} GB` },
    { name: t("hardware.speed"), value: hardwareInfo.memory.length > 0 ? `${hardwareInfo.memory[0].speed_mhz} MHz` : "--" },
    { name: t("hardware.count"), value: `${hardwareInfo.memory.length}` },
  ];

  const storageDisplayInfo: DisplayInfo[] = hardwareInfo.disk.map((disk, i) => ({
    name: `${t("hardware.storage")} ${i + 1}`,
    value: disk,
  }));

  const motherboardDisplayInfo: DisplayInfo[] = [
    { name: t("hardware.model"), value: hardwareInfo.motherboard },
  ];

  return (
    <Box pt={8}>
      <Heading size="lg" color={headingColor} mb={6}>
        {t("hardware.title")}
      </Heading>

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
            borderColor={borderColor}
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
            borderColor={borderColor}
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
            borderColor={borderColor}
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
            borderColor={borderColor}
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
            borderColor={borderColor}
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
              borderColor={borderColor}
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
            borderColor={borderColor}
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
            borderColor={borderColor}
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
              borderColor={borderColor}
              textColor={textColor}
              subTextColor={subTextColor}
              liquidGlassEnabled={liquidGlassEnabled}
            />
          )}
        </Grid>
      </VStack>
    </Box>
  );
}
