import {
  Box,
  Heading,
  VStack,
  Text,
  HStack,
  useColorModeValue,
  Button,
  Progress,
  useToast,
  SimpleGrid,
} from "@chakra-ui/react";
import { ArrowLeft, MemoryStick, Cpu, HardDrive } from "lucide-react";
import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";

interface MemoryData {
  physical_total: number;
  physical_used: number;
  physical_available: number;
  virtual_total: number;
  virtual_used: number;
  virtual_available: number;
  working_set_total: number;
  working_set_used: number;
  working_set_available: number;
}

interface CleanupResult {
  success: boolean;
  message: string;
  freed_mb: number;
}

function formatMemory(mb: number): string {
  if (mb >= 1024) {
    return `${(mb / 1024).toFixed(1)} GB`;
  }
  return `${mb} MB`;
}

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

  const [memoryData, setMemoryData] = useState<MemoryData | null>(null);
  const [loading, setLoading] = useState(true);
  const [cleaningAll, setCleaningAll] = useState(false);
  const [cleaningStandby, setCleaningStandby] = useState(false);
  const [trimmingWs, setTrimmingWs] = useState(false);

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

  useEffect(() => {
    fetchMemoryData();
    const interval = setInterval(fetchMemoryData, 3000);
    return () => clearInterval(interval);
  }, [fetchMemoryData]);

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
      <Box
        bg={cardBg}
        borderRadius="xl"
        borderWidth="1px"
        borderColor={cardBorder}
        p={5}
        boxShadow="md"
      >
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
          bg={useColorModeValue("gray.100", "#222222")}
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
      </Box>
    );
  };

  const content = (
    <VStack align="start" spacing={6}>
      <HStack justifyContent="space-between" alignItems="center" w="full">
        <Button
          variant="ghost"
          leftIcon={<ArrowLeft size={18} />}
          onClick={() => navigate("/optimize")}
          color={headingColor}
        >
          {t("tests.back") || "返回"}
        </Button>
        <Heading size="lg" color={headingColor} fontWeight="700">
          {t("optimization.memoryCleanup.title")}
        </Heading>
        <Box w="100px" />
      </HStack>

      {loading ? (
        <Text color={subTextColor} textAlign="center" w="full" py={8}>
          {t("optimization.memoryCleanup.loading")}
        </Text>
      ) : (
        memoryData && (
          <>
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

            <VStack spacing={3} w="full" mt={2}>
              <Button
                bg={themeConfig.primaryColor}
                color={getContrastTextColor()}
                size="lg"
                w="full"
                maxW="320px"
                onClick={handleCleanAll}
                isLoading={cleaningAll}
                loadingText={t("optimization.memoryCleanup.cleaning")}
                borderRadius="xl"
                fontWeight="600"
                _hover={{
                  bg: themeConfig.primaryColor,
                  filter: "brightness(0.9)",
                }}
                _active={{
                  bg: themeConfig.primaryColor,
                  filter: "brightness(0.8)",
                }}
              >
                {t("optimization.memoryCleanup.cleanAll")}
              </Button>
              <Button
                colorScheme="blue"
                variant="outline"
                size="md"
                w="full"
                maxW="320px"
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
                size="md"
                w="full"
                maxW="320px"
                onClick={handleTrimWorkingSet}
                isLoading={trimmingWs}
                loadingText={t("optimization.memoryCleanup.cleaning")}
                borderRadius="xl"
              >
                {t("optimization.memoryCleanup.trimWorkingSet")}
              </Button>
            </VStack>
          </>
        )
      )}
    </VStack>
  );

  if (liquidGlassEnabled) {
    return (
      <Box pt={8}>
        <LiquidGlassCard w="full" boxShadow="2xl" overflow="hidden" position="relative" p={6}>
          {content}
        </LiquidGlassCard>
      </Box>
    );
  }

  return (
    <Box pt={8}>
      <Box
        bg={cardBg}
        borderRadius="xl"
        borderWidth="1px"
        borderColor={cardBorder}
        w="full"
        boxShadow="2xl"
        overflow="hidden"
        position="relative"
        p={6}
      >
        {content}
      </Box>
    </Box>
  );
}