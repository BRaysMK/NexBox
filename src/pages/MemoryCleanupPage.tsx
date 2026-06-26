import {
  Box, Heading, VStack, Text, HStack, useColorModeValue,
  Button, Progress, useToast, SimpleGrid, Switch,
  Slider, SliderTrack, SliderFilledTrack, SliderThumb,
} from "@chakra-ui/react";
import { ArrowLeft, MemoryStick, Cpu, HardDrive } from "lucide-react";
import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { LazyStore } from "@tauri-apps/plugin-store";
import { CustomSelect } from "@/components/special/custom-select";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";

interface MemoryData {
  physical_total: number; physical_used: number; physical_available: number;
  virtual_total: number; virtual_used: number; virtual_available: number;
  working_set_total: number; working_set_used: number; working_set_available: number;
}
interface CleanupResult { success: boolean; message: string; freed_mb: number; }
interface AutoCleanConfig {
  enabled: boolean; interval_seconds: number; threshold_mb: number; clean_type: string;
}

function formatMemory(mb: number): string {
  if (mb >= 1024) {
    return `${(mb / 1024).toFixed(1)} GB`;
  }
  return `${mb} MB`;
}

const store = new LazyStore("auto-clean.json");

export default function MemoryCleanupPage() {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const { config: themeConfig, getContrastTextColor } = useThemeColor();
  const navigate = useNavigate();
  const toast = useToast();

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const cardBg = useColorModeValue("white", "#111111");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const textColor = useColorModeValue("gray.700", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const labelColor = useColorModeValue("gray.600", "#b0b0b0");
  const trackBg = useColorModeValue("gray.200", "#333333");

  const [memoryData, setMemoryData] = useState<MemoryData | null>(null);
  const [loading, setLoading] = useState(true);
  const [cleaningAll, setCleaningAll] = useState(false);
  const [cleaningStandby, setCleaningStandby] = useState(false);
  const [trimmingWs, setTrimmingWs] = useState(false);

  const [autoClean, setAutoClean] = useState(false);
  const [autoInterval, setAutoInterval] = useState("300");
  const [autoThreshold, setAutoThreshold] = useState(12288);
  const [autoCleanType, setAutoCleanType] = useState("all");

  const intervalOptions = [
    { value: "300", label: t("optimization.memoryCleanup.interval5min", "5分钟") },
    { value: "600", label: t("optimization.memoryCleanup.interval10min", "10分钟") },
    { value: "1800", label: t("optimization.memoryCleanup.interval30min", "30分钟") },
    { value: "3600", label: t("optimization.memoryCleanup.interval1hour", "1小时") },
  ];

  const cleanTypeOptions = [
    { value: "all", label: t("optimization.memoryCleanup.cleanAll") },
    { value: "standby", label: t("optimization.memoryCleanup.cleanStandby") },
    { value: "working_set", label: t("optimization.memoryCleanup.trimWorkingSet") },
  ];

  const fetchMemoryData = useCallback(async () => {
    try {
      const data = await invoke<MemoryData>("get_detailed_memory_status");
      setMemoryData(data);
    } catch (error) {
      console.error("Failed to fetch memory data:", error);
    } finally {
      setLoading(false);
    }
  }, []);

  // Load auto-clean config from store
  useEffect(() => {
    (async () => {
      try {
        const enabled = await store.get<boolean>("auto-clean-enabled");
        const interval = await store.get<string>("auto-clean-interval");
        const threshold = await store.get<number>("auto-clean-threshold");
        const cleanType = await store.get<string>("auto-clean-type");

        if (enabled !== null && enabled !== undefined) setAutoClean(enabled);
        if (interval) setAutoInterval(interval);
        if (threshold !== null && threshold !== undefined) setAutoThreshold(threshold);
        if (cleanType) setAutoCleanType(cleanType);
      } catch (error) {
        console.error("Failed to load auto-clean config:", error);
      }
    })();
  }, []);

  useEffect(() => {
    fetchMemoryData();
    const interval = setInterval(fetchMemoryData, 2000);
    return () => clearInterval(interval);
  }, [fetchMemoryData]);

  // Start/stop auto-clean when switch changes
  useEffect(() => {
    (async () => {
      try {
        if (autoClean) {
          await invoke("start_auto_clean", {
            config: {
              enabled: autoClean,
              interval_seconds: parseInt(autoInterval),
              threshold_mb: autoThreshold,
              clean_type: autoCleanType,
            },
          });
        } else {
          await invoke("stop_auto_clean");
        }
      } catch (error) {
        console.error("Auto-clean toggle error:", error);
      }
    })();
  }, [autoClean]);

  const restartAutoClean = useCallback(async () => {
    try {
      await invoke("stop_auto_clean");
      if (autoClean) {
        await invoke("start_auto_clean", {
          config: {
            enabled: autoClean,
            interval_seconds: parseInt(autoInterval),
            threshold_mb: autoThreshold,
            clean_type: autoCleanType,
          },
        });
      }
    } catch (error) {
      console.error("Failed to restart auto-clean:", error);
    }
  }, [autoClean, autoInterval, autoThreshold, autoCleanType]);

  const handleAutoCleanChange = async (enabled: boolean) => {
    setAutoClean(enabled);
    await store.set("auto-clean-enabled", enabled);
    await store.save();
  };

  const handleIntervalChange = async (value: string) => {
    setAutoInterval(value);
    await store.set("auto-clean-interval", value);
    await store.save();
    await restartAutoClean();
  };

  const handleThresholdChange = async (value: number) => {
    setAutoThreshold(value);
    await store.set("auto-clean-threshold", value);
    await store.save();
  };

  const handleThresholdChangeEnd = async (value: number) => {
    await restartAutoClean();
  };

  const handleCleanTypeChange = async (value: string) => {
    setAutoCleanType(value);
    await store.set("auto-clean-type", value);
    await store.save();
    await restartAutoClean();
  };

  // Stop auto-clean on unmount
  useEffect(() => {
    return () => {
      invoke("stop_auto_clean").catch(() => {});
    };
  }, []);

  const handleCleanAll = async () => {
    setCleaningAll(true);
    try {
      const result1 = await invoke<CleanupResult>("clean_standby_memory");
      const result2 = await invoke<CleanupResult>("trim_system_working_set");
      const totalFreed = result1.freed_mb + result2.freed_mb;
      await fetchMemoryData();
      toast({
        title: t("optimization.memoryCleanup.cleanAll"),
        description:
          totalFreed > 0
            ? t("optimization.memoryCleanup.freedMemory", { size: totalFreed })
            : t("optimization.memoryCleanup.noMemoryFreed"),
        status: totalFreed > 0 ? "success" : "info",
        duration: 4000,
        isClosable: true,
      });
    } catch (error) {
      toast({
        title: t("optimization.memoryCleanup.error"),
        description: String(error),
        status: "error",
        duration: 4000,
        isClosable: true,
      });
    } finally {
      setCleaningAll(false);
    }
  };

  const handleCleanStandby = async () => {
    setCleaningStandby(true);
    try {
      const result = await invoke<CleanupResult>("clean_standby_memory");
      await fetchMemoryData();
      toast({
        title: t("optimization.memoryCleanup.cleanStandby"),
        description:
          result.freed_mb > 0
            ? t("optimization.memoryCleanup.freedMemory", { size: result.freed_mb })
            : t("optimization.memoryCleanup.noMemoryFreed"),
        status: result.freed_mb > 0 ? "success" : "info",
        duration: 4000,
        isClosable: true,
      });
    } catch (error) {
      toast({
        title: t("optimization.memoryCleanup.error"),
        description: String(error),
        status: "error",
        duration: 4000,
        isClosable: true,
      });
    } finally {
      setCleaningStandby(false);
    }
  };

  const handleTrimWorkingSet = async () => {
    setTrimmingWs(true);
    try {
      const result = await invoke<CleanupResult>("trim_system_working_set");
      await fetchMemoryData();
      toast({
        title: t("optimization.memoryCleanup.trimWorkingSet"),
        description:
          result.freed_mb > 0
            ? t("optimization.memoryCleanup.freedMemory", { size: result.freed_mb })
            : t("optimization.memoryCleanup.noMemoryFreed"),
        status: result.freed_mb > 0 ? "success" : "info",
        duration: 4000,
        isClosable: true,
      });
    } catch (error) {
      toast({
        title: t("optimization.memoryCleanup.error"),
        description: String(error),
        status: "error",
        duration: 4000,
        isClosable: true,
      });
    } finally {
      setTrimmingWs(false);
    }
  };

  const getUsagePercent = (used: number, total: number): number => {
    if (total <= 0) return 0;
    return Math.round((used / total) * 100);
  };

  const getProgressColor = (percent: number): string => {
    if (percent < 60) return "green";
    if (percent < 85) return "yellow";
    return "red";
  };

  const renderMemoryCard = (
    icon: React.ReactNode,
    title: string,
    used: number,
    available: number,
    total: number
  ) => {
    const percent = getUsagePercent(used, total);
    const progressColor = getProgressColor(percent);

    return (
      <LiquidGlassCard w="full" p={5}>
        <HStack mb={4} spacing={3}>
          <Box color={themeConfig.primaryColor}>{icon}</Box>
          <Text fontWeight="bold" color={headingColor} fontSize="md">
            {title}
          </Text>
        </HStack>
        <Progress
          value={percent}
          size="sm"
          colorScheme={progressColor}
          borderRadius="full"
          mb={3}
          bg={trackBg}
        />
        <SimpleGrid columns={3} spacing={2}>
          <VStack align="center" spacing={0}>
            <Text fontSize="xs" color={subTextColor}>
              {t("optimization.memoryCleanup.used")}
            </Text>
            <Text fontSize="sm" fontWeight="bold" color={textColor}>
              {formatMemory(used)}
            </Text>
          </VStack>
          <VStack align="center" spacing={0}>
            <Text fontSize="xs" color={subTextColor}>
              {t("optimization.memoryCleanup.available")}
            </Text>
            <Text fontSize="sm" fontWeight="bold" color="green.400">
              {formatMemory(available)}
            </Text>
          </VStack>
          <VStack align="center" spacing={0}>
            <Text fontSize="xs" color={subTextColor}>
              {t("optimization.memoryCleanup.total")}
            </Text>
            <Text fontSize="sm" fontWeight="bold" color={labelColor}>
              {formatMemory(total)}
            </Text>
          </VStack>
        </SimpleGrid>
      </LiquidGlassCard>
    );
  };

  return (
    <Box pt={8}>
      <LiquidGlassCard w="full" boxShadow="2xl" overflow="hidden" position="relative" p={6}>
        <VStack align="start" spacing={6}>
          {/* Top bar: back button + cleanup buttons */}
          <HStack justifyContent="space-between" alignItems="center" w="full">
            <Button
              variant="ghost"
              leftIcon={<ArrowLeft size={18} />}
              onClick={() => navigate("/optimize")}
              color={headingColor}
            >
              {t("tests.back") || "返回"}
            </Button>
            <HStack spacing={3}>
              <Button
                bg={themeConfig.primaryColor}
                color={getContrastTextColor()}
                size="sm"
                onClick={handleCleanAll}
                isLoading={cleaningAll}
                loadingText={t("optimization.memoryCleanup.cleaning")}
                borderRadius="xl"
                fontWeight="600"
                _hover={{ bg: themeConfig.primaryColor, filter: "brightness(0.9)" }}
                _active={{ bg: themeConfig.primaryColor, filter: "brightness(0.8)" }}
              >
                {t("optimization.memoryCleanup.cleanAll")}
              </Button>
              <Button
                colorScheme="blue"
                variant="outline"
                size="sm"
                onClick={handleCleanStandby}
                isLoading={cleaningStandby}
                loadingText={t("optimization.memoryCleanup.cleaning")}
                borderRadius="xl"
              >
                {t("optimization.memoryCleanup.cleanStandby")}
              </Button>
              <Button
                colorScheme="purple"
                variant="outline"
                size="sm"
                onClick={handleTrimWorkingSet}
                isLoading={trimmingWs}
                loadingText={t("optimization.memoryCleanup.cleaning")}
                borderRadius="xl"
              >
                {t("optimization.memoryCleanup.trimWorkingSet")}
              </Button>
            </HStack>
          </HStack>

          {loading ? (
            <Text color={subTextColor} textAlign="center" w="full" py={8}>
              {t("optimization.memoryCleanup.loading")}
            </Text>
          ) : (
            memoryData && (
              <>
                {/* Memory usage cards */}
                <SimpleGrid columns={{ base: 1, md: 3 }} spacing={4} w="full">
                  {renderMemoryCard(
                    <MemoryStick size={22} />,
                    t("optimization.memoryCleanup.physicalMemory"),
                    memoryData.physical_used,
                    memoryData.physical_available,
                    memoryData.physical_total
                  )}
                  {renderMemoryCard(
                    <HardDrive size={22} />,
                    t("optimization.memoryCleanup.virtualMemory"),
                    memoryData.virtual_used,
                    memoryData.virtual_available,
                    memoryData.virtual_total
                  )}
                  {renderMemoryCard(
                    <Cpu size={22} />,
                    t("optimization.memoryCleanup.workingSet"),
                    memoryData.working_set_used,
                    memoryData.working_set_available,
                    memoryData.working_set_total
                  )}
                </SimpleGrid>

                {/* Scheduled cleanup card */}
                <LiquidGlassCard w="full" p={5}>
                  <VStack align="start" spacing={4} w="full">
                    <HStack justify="space-between" w="full">
                      <Text fontWeight="bold" color={headingColor} fontSize="md">
                        {t("optimization.memoryCleanup.scheduledCleanup", "定时清理")}
                      </Text>
                      <Switch
                        isChecked={autoClean}
                        onChange={(e) => handleAutoCleanChange(e.target.checked)}
                        sx={{
                          "span.chakra-switch__track": {
                            bg: autoClean ? themeConfig.primaryColor : undefined,
                          },
                        }}
                      />
                    </HStack>

                    <SimpleGrid columns={{ base: 1, md: 3 }} spacing={4} w="full">
                      {/* Interval selector */}
                      <VStack align="start" spacing={1}>
                        <Text fontSize="sm" color={labelColor}>
                          {t("optimization.memoryCleanup.cleanInterval", "清理间隔")}
                        </Text>
                        <CustomSelect
                          value={autoInterval}
                          onChange={handleIntervalChange}
                          options={intervalOptions}
                          width="full"
                        />
                      </VStack>

                      {/* Memory threshold slider */}
                      <VStack align="start" spacing={1}>
                        <Text fontSize="sm" color={labelColor}>
                          {t("optimization.memoryCleanup.memoryThreshold", "内存阈值")}
                        </Text>
                        <HStack w="full" spacing={3}>
                          <Slider
                            flex={1}
                            value={autoThreshold}
                            min={4096}
                            max={32768}
                            step={1024}
                            onChange={handleThresholdChange}
                            onChangeEnd={handleThresholdChangeEnd}
                          >
                            <SliderTrack bg={trackBg}>
                              <SliderFilledTrack bg={themeConfig.primaryColor} />
                            </SliderTrack>
                            <SliderThumb boxSize={4} />
                          </Slider>
                          <Text fontSize="sm" fontWeight="bold" color={textColor} minW="60px" textAlign="right">
                            {Math.round(autoThreshold / 1024)} GB
                          </Text>
                        </HStack>
                      </VStack>

                      {/* Clean type selector */}
                      <VStack align="start" spacing={1}>
                        <Text fontSize="sm" color={labelColor}>
                          {t("optimization.memoryCleanup.cleanType", "清理类型")}
                        </Text>
                        <CustomSelect
                          value={autoCleanType}
                          onChange={handleCleanTypeChange}
                          options={cleanTypeOptions}
                          width="full"
                          direction="up"
                        />
                      </VStack>
                    </SimpleGrid>
                  </VStack>
                </LiquidGlassCard>
              </>
            )
          )}
        </VStack>
      </LiquidGlassCard>
    </Box>
  );
}
