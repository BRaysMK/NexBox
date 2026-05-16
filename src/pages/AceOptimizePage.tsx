import {
  Box,
  Heading,
  VStack,
  Text,
  HStack,
  useColorModeValue,
  Button,
  useToast,
  SimpleGrid,
  Badge,
} from "@chakra-ui/react";
import { ArrowLeft, Gauge, ShieldOff, Zap } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";

interface ProcessOptimizeResult {
  success: boolean;
  message: string;
  process_name: string;
  was_running: boolean;
}

interface AceOptimizeResult {
  success: boolean;
  message: string;
  ace_tray: boolean;
  sguard64: boolean;
  sguardsvc64: boolean;
}

interface AllGameOptimizeResult {
  success: boolean;
  message: string;
  delta_boosted: boolean;
  ace_limited: boolean;
  ace_count: number;
}

export default function AceOptimizePage() {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const navigate = useNavigate();
  const toast = useToast();

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const cardBg = useColorModeValue("white", "#111111");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const textColor = useColorModeValue("gray.700", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");

  const [boostingDelta, setBoostingDelta] = useState(false);
  const [optimizingAce, setOptimizingAce] = useState(false);
  const [optimizingAll, setOptimizingAll] = useState(false);
  const [deltaStatus, setDeltaStatus] = useState<"idle" | "optimized" | "not_running">("idle");
  const [aceStatus, setAceStatus] = useState<"idle" | "optimized" | "not_running">("idle");
  const [aceCount, setAceCount] = useState(0);

  const getStatusBadge = (status: "idle" | "optimized" | "not_running") => {
    if (status === "optimized") {
      return (
        <Badge colorScheme="green" variant="subtle" fontSize="xs">
          {t("optimization.aceOptimize.status.optimized")}
        </Badge>
      );
    }
    if (status === "not_running") {
      return (
        <Badge colorScheme="gray" variant="subtle" fontSize="xs">
          {t("optimization.aceOptimize.status.notRunning")}
        </Badge>
      );
    }
    return (
      <Badge colorScheme="yellow" variant="subtle" fontSize="xs">
        {t("optimization.aceOptimize.status.notOptimized")}
      </Badge>
    );
  };

  const handleBoostDelta = async () => {
    setBoostingDelta(true);
    try {
      const result = await invoke<ProcessOptimizeResult>("boost_delta_force_priority");
      if (result.was_running) {
        setDeltaStatus("optimized");
        toast({
          title: t("optimization.aceOptimize.deltaBoost.title"),
          description: t("optimization.aceOptimize.deltaBoost.success"),
          status: "success",
          duration: 4000,
          isClosable: true,
        });
      } else {
        setDeltaStatus("not_running");
        toast({
          title: t("optimization.aceOptimize.deltaBoost.title"),
          description: t("optimization.aceOptimize.deltaBoost.notRunning"),
          status: "warning",
          duration: 4000,
          isClosable: true,
        });
      }
    } catch (error) {
      toast({
        title: t("optimization.aceOptimize.error"),
        description: String(error),
        status: "error",
        duration: 4000,
        isClosable: true,
      });
    } finally {
      setBoostingDelta(false);
    }
  };

  const handleOptimizeAce = async () => {
    setOptimizingAce(true);
    try {
      const result = await invoke<AceOptimizeResult>("optimize_ace_processes");
      const optimizedCount = [result.ace_tray, result.sguard64, result.sguardsvc64].filter(Boolean).length;
      if (optimizedCount > 0) {
        setAceStatus("optimized");
        setAceCount(optimizedCount);
        toast({
          title: t("optimization.aceOptimize.aceLimit.title"),
          description: t("optimization.aceOptimize.aceLimit.success", { count: optimizedCount }),
          status: "success",
          duration: 4000,
          isClosable: true,
        });
      } else {
        setAceStatus("not_running");
        toast({
          title: t("optimization.aceOptimize.aceLimit.title"),
          description: t("optimization.aceOptimize.aceLimit.notRunning"),
          status: "warning",
          duration: 4000,
          isClosable: true,
        });
      }
    } catch (error) {
      toast({
        title: t("optimization.aceOptimize.error"),
        description: String(error),
        status: "error",
        duration: 4000,
        isClosable: true,
      });
    } finally {
      setOptimizingAce(false);
    }
  };

  const handleOptimizeAll = async () => {
    setOptimizingAll(true);
    try {
      const result = await invoke<AllGameOptimizeResult>("optimize_all_game_processes");

      if (result.delta_boosted) {
        setDeltaStatus("optimized");
      } else {
        setDeltaStatus("not_running");
      }
      if (result.ace_limited) {
        setAceStatus("optimized");
        setAceCount(result.ace_count);
      } else {
        setAceStatus("not_running");
      }

      if (result.delta_boosted && result.ace_limited) {
        toast({
          title: t("optimization.aceOptimize.optimizeAll.success"),
          description: result.message,
          status: "success",
          duration: 4000,
          isClosable: true,
        });
      } else if (result.delta_boosted || result.ace_limited) {
        toast({
          title: t("optimization.aceOptimize.optimizeAll.partial"),
          description: result.message,
          status: "warning",
          duration: 4000,
          isClosable: true,
        });
      } else {
        toast({
          title: t("optimization.aceOptimize.optimizeAll.partial"),
          description: result.message,
          status: "info",
          duration: 4000,
          isClosable: true,
        });
      }
    } catch (error) {
      toast({
        title: t("optimization.aceOptimize.error"),
        description: String(error),
        status: "error",
        duration: 4000,
        isClosable: true,
      });
    } finally {
      setOptimizingAll(false);
    }
  };

  const renderOptimizeCard = (
    icon: React.ReactNode,
    title: string,
    description: string,
    status: "idle" | "optimized" | "not_running",
    buttonLabel: string,
    onOptimize: () => void,
    isLoading: boolean,
    loadingLabel: string,
    accentColor: string,
    accentBg: string
  ) => (
    <Box
      bg={cardBg}
      borderRadius="xl"
      borderWidth="1px"
      borderColor={cardBorder}
      p={5}
      boxShadow="md"
    >
      <VStack align="start" spacing={3} h="full">
        <HStack spacing={3} w="full" justify="space-between">
          <HStack spacing={3}>
            <Box color={accentColor}>{icon}</Box>
            <Text fontWeight="bold" color={headingColor} fontSize="md">
              {title}
            </Text>
          </HStack>
          {getStatusBadge(status)}
        </HStack>
        <Text fontSize="sm" color={subTextColor} flex={1}>
          {description}
        </Text>
        <Button
          colorScheme={accentColor === "green.400" ? "green" : accentColor === "orange.400" ? "orange" : "teal"}
          size="md"
          w="full"
          onClick={onOptimize}
          isLoading={isLoading}
          loadingText={loadingLabel}
          borderRadius="xl"
          fontWeight="600"
        >
          {buttonLabel}
        </Button>
      </VStack>
    </Box>
  );

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
          {t("optimization.aceOptimize.title")}
        </Heading>
        <Box w="100px" />
      </HStack>

      <SimpleGrid columns={{ base: 1, md: 2 }} spacing={4} w="full">
        {renderOptimizeCard(
          <Zap size={22} />,
          t("optimization.aceOptimize.deltaBoost.title"),
          t("optimization.aceOptimize.deltaBoost.description"),
          deltaStatus,
          t("optimization.aceOptimize.deltaBoost.button"),
          handleBoostDelta,
          boostingDelta,
          t("optimization.aceOptimize.deltaBoost.optimizing"),
          "green.400",
          "rgba(56, 161, 105, 0.1)"
        )}
        {renderOptimizeCard(
          <ShieldOff size={22} />,
          t("optimization.aceOptimize.aceLimit.title"),
          t("optimization.aceOptimize.aceLimit.description"),
          aceStatus,
          t("optimization.aceOptimize.aceLimit.button"),
          handleOptimizeAce,
          optimizingAce,
          t("optimization.aceOptimize.aceLimit.limiting"),
          "orange.400",
          "rgba(221, 107, 32, 0.1)"
        )}
      </SimpleGrid>

      <HStack w="full" justify="center">
        <Button
          colorScheme="teal"
          size="lg"
          w="full"
          maxW="400px"
          onClick={handleOptimizeAll}
          isLoading={optimizingAll}
          loadingText={t("optimization.aceOptimize.optimizeAll.optimizing")}
          borderRadius="xl"
          fontWeight="600"
          leftIcon={<Gauge size={18} />}
          h="50px"
        >
          {t("optimization.aceOptimize.optimizeAll.button")}
        </Button>
      </HStack>
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