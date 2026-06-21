import {
  Box,
  Text,
  Heading,
  VStack,
  HStack,
  Button,
  SimpleGrid,
  useColorModeValue,
  useToast,
  Badge,
  IconButton,
} from "@chakra-ui/react";
import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import {
  ArrowLeft,
  Gauge,
  Cpu,
  Shield,
  Zap,
  RefreshCw,
} from "lucide-react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useNavigate } from "react-router-dom";
import { useThemeColor } from "@/contexts/theme-color-context";
import { hexToRgba } from "@/lib/color-utils";

interface OptionState {
  running: boolean;
  message: string;
}

function OptionRow({
  icon,
  title,
  description,
  isLoading,
  isApplied,
  gameRunning,
  onApply,
}: {
  icon: React.ReactNode;
  title: string;
  description: string;
  isLoading: boolean;
  isApplied: boolean;
  gameRunning: boolean | null;
  onApply: () => void;
}) {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const iconBg = useColorModeValue("white", "#222222");
  const rowBg = useColorModeValue("gray.50", "#1a1a1a");
  const { getActiveColor } = useThemeColor();

  const content = (
    <HStack align="flex-start" spacing={4}>
      <Box
        w={10} h={10}
        borderRadius="lg"
        bg={iconBg}
        border="1px solid"
        borderColor={borderColor}
        display="flex"
        alignItems="center"
        justifyContent="center"
        flexShrink={0}
        color={isApplied ? getActiveColor() : subTextColor}
      >
        {icon}
      </Box>
      <VStack align="flex-start" spacing={1} flex={1}>
        <Text fontSize="sm" fontWeight="bold" color={textColor}>
          {title}
        </Text>
        <Text fontSize="xs" color={subTextColor} lineHeight="short">
          {description}
        </Text>
      </VStack>
      <VStack align="flex-end" spacing={2} flexShrink={0}>
        <Button
          size="sm"
          bg={isApplied ? getActiveColor() : undefined}
          color={isApplied ? "white" : getActiveColor()}
          borderColor={!isApplied ? getActiveColor() : undefined}
          variant={!isApplied ? "outline" : undefined}
          _hover={isApplied ? { bg: getActiveColor(), opacity: 0.9 } : undefined}
          onClick={onApply}
          isLoading={isLoading}
          loadingText=""
          px={4}
          borderRadius="lg"
          minW="72px"
        >
          {isApplied ? t("optimization.aceOptimize.applied") : t("optimization.aceOptimize.apply")}
        </Button>
        {gameRunning !== null && (
          <Badge
            colorScheme={gameRunning ? "green" : "gray"}
            variant="subtle"
            fontSize="2xs"
            px={2}
            py={0.5}
            borderRadius="full"
          >
            {gameRunning ? t("optimization.aceOptimize.status.processRunning") : t("optimization.aceOptimize.status.processNotRunning")}
          </Badge>
        )}
      </VStack>
    </HStack>
  );

  if (liquidGlassEnabled) {
    return (
      <LiquidGlassCard p={4} _hover={{ borderColor: getActiveColor() }}>
        {content}
      </LiquidGlassCard>
    );
  }

  return (
    <Box
      p={4}
      borderRadius="xl"
      bg={rowBg}
      border="1px solid"
      borderColor={borderColor}
      transition="all 0.15s"
      _hover={{ borderColor: getActiveColor(), boxShadow: `0 0 0 1px ${hexToRgba(getActiveColor(), 0.3)}` }}
    >
      {content}
    </Box>
  );
}

function SettingCard({
  title,
  subTitle,
  icon,
  color,
  children,
}: {
  title: string;
  subTitle?: string;
  icon?: React.ReactNode;
  color?: string;
  children: React.ReactNode;
}) {
  const { liquidGlassEnabled } = useBackground();
  const cardBg = useColorModeValue("white", "#111111");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const headerColor = useColorModeValue("gray.900", "#ffffff");
  const { getActiveColor } = useThemeColor();
  const accentColor = color || getActiveColor();

  const content = (
    <VStack align="stretch" spacing={4}>
      <HStack spacing={3}>
        {icon && (
          <Box
            w={9} h={9}
            borderRadius="lg"
            bg={`${accentColor}15`}
            display="flex"
            alignItems="center"
            justifyContent="center"
            color={accentColor}
          >
            {icon}
          </Box>
        )}
        <VStack align="flex-start" spacing={0}>
          <Text fontWeight="bold" fontSize="md" color={headerColor}>
            {title}
          </Text>
          {subTitle && (
            <Text fontSize="xs" color="gray.500">
              {subTitle}
            </Text>
          )}
        </VStack>
      </HStack>
      {children}
    </VStack>
  );

  if (liquidGlassEnabled) {
    return <LiquidGlassCard p={5}>{content}</LiquidGlassCard>;
  }

  return (
    <Box bg={cardBg} borderRadius="xl" p={5} border="1px solid" borderColor={borderColor}>
      {content}
    </Box>
  );
}

export default function AceOptimizePage() {
  const { t } = useTranslation();
  const toast = useToast();
  const navigate = useNavigate();

  const { liquidGlassEnabled } = useBackground();
  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const borderColorVal = useColorModeValue("gray.200", "#333333");
  const { getActiveColor } = useThemeColor();

  const [deltaPriority, setDeltaPriority] = useState<OptionState>({ running: false, message: "" });
  const [deltaAffinity, setDeltaAffinity] = useState<OptionState>({ running: false, message: "" });
  const [acePriority, setAcePriority] = useState<OptionState>({ running: false, message: "" });
  const [aceAffinity, setAceAffinity] = useState<OptionState>({ running: false, message: "" });
  const [optimizeAllLoading, setOptimizeAllLoading] = useState(false);

  const applyDeltaPriority = useCallback(async () => {
    setDeltaPriority(prev => ({ ...prev, running: true }));
    try {
      const result = await invoke<{ success: boolean; message: string; was_running: boolean }>("boost_delta_force_priority");
      setDeltaPriority({ running: false, message: result.message });
      toast({
        title: result.was_running ? t("optimization.aceOptimize.deltaBoost.success") : t("optimization.aceOptimize.deltaBoost.notRunning"),
        status: result.was_running ? "success" : "info",
        duration: 2000,
      });
    } catch (e: any) {
      setDeltaPriority({ running: false, message: String(e) });
      toast({ title: String(e), status: "error", duration: 2000 });
    }
  }, [toast, t]);

  const applyDeltaAffinity = useCallback(async () => {
    setDeltaAffinity(prev => ({ ...prev, running: true }));
    try {
      const result = await invoke<{ success: boolean; message: string; was_running: boolean }>("boost_delta_force_affinity");
      setDeltaAffinity({ running: false, message: result.message });
      toast({
        title: result.was_running ? t("optimization.aceOptimize.deltaBoost.affinitySuccess") : t("optimization.aceOptimize.deltaBoost.notRunning"),
        status: result.was_running ? "success" : "info",
        duration: 2000,
      });
    } catch (e: any) {
      setDeltaAffinity({ running: false, message: String(e) });
      toast({ title: String(e), status: "error", duration: 2000 });
    }
  }, [toast, t]);

  const applyAcePriority = useCallback(async () => {
    setAcePriority(prev => ({ ...prev, running: true }));
    try {
      const result = await invoke<{ success: boolean; message: string; count: number }>("limit_ace_priority");
      setAcePriority({ running: false, message: result.message });
      toast({
        title: result.count > 0 ? t("optimization.aceOptimize.aceLimit.success", { count: result.count }) : t("optimization.aceOptimize.aceLimit.notRunning"),
        status: result.count > 0 ? "success" : "info",
        duration: 2000,
      });
    } catch (e: any) {
      setAcePriority({ running: false, message: String(e) });
      toast({ title: String(e), status: "error", duration: 2000 });
    }
  }, [toast, t]);

  const applyAceAffinity = useCallback(async () => {
    setAceAffinity(prev => ({ ...prev, running: true }));
    try {
      const result = await invoke<{ success: boolean; message: string; count: number }>("restrict_ace_affinity");
      setAceAffinity({ running: false, message: result.message });
      toast({
        title: result.count > 0 ? t("optimization.aceOptimize.aceLimit.affinitySuccess", { count: result.count }) : t("optimization.aceOptimize.aceLimit.notRunning"),
        status: result.count > 0 ? "success" : "info",
        duration: 2000,
      });
    } catch (e: any) {
      setAceAffinity({ running: false, message: String(e) });
      toast({ title: String(e), status: "error", duration: 2000 });
    }
  }, [toast, t]);

  const applyAll = useCallback(async () => {
    setOptimizeAllLoading(true);
    try {
      const result = await invoke<{ success: boolean; message: string; delta_boosted: boolean; ace_limited: boolean; ace_count: number }>("optimize_all_game_processes");
      if (result.delta_boosted) {
        setDeltaPriority({ running: false, message: t("optimization.aceOptimize.status.optimized") });
        setDeltaAffinity({ running: false, message: t("optimization.aceOptimize.status.optimized") });
      }
      if (result.ace_limited) {
        setAcePriority({ running: false, message: t("optimization.aceOptimize.status.optimized") });
        setAceAffinity({ running: false, message: t("optimization.aceOptimize.status.optimized") });
      }
      toast({
        title: result.message,
        status: result.success ? "success" : "info",
        duration: 3000,
      });
    } catch (e: any) {
      toast({ title: String(e), status: "error", duration: 2000 });
    }
    setOptimizeAllLoading(false);
  }, [toast, t]);

  return (
    <Box pt={8} pb={8}>
      <HStack justify="space-between" mb={6}>
        <HStack>
          <IconButton
            aria-label={t("builtinTools.back")}
            icon={<ArrowLeft size={20} />}
            variant="ghost"
            onClick={() => navigate("/optimize")}
            color={headingColor}
          />
          <Heading size="lg" color={headingColor}>
            {t("optimization.aceOptimize.title")}
          </Heading>
        </HStack>
      </HStack>

      <SimpleGrid columns={2} spacing={5} mb={5}>
        <SettingCard
          title={t("optimization.aceOptimize.deltaSection.title")}
          subTitle={t("optimization.aceOptimize.deltaSection.subtitle")}
          icon={<Gauge size={18} />}
          color={getActiveColor()}
        >
          <VStack align="stretch" spacing={3}>
            <OptionRow
              icon={<Cpu size={18} />}
              title={t("optimization.aceOptimize.deltaBoost.title")}
              description={t("optimization.aceOptimize.deltaBoost.description")}
              isLoading={deltaPriority.running}
              isApplied={!!deltaPriority.message && !deltaPriority.message.includes("未运行")}
              gameRunning={deltaPriority.message ? !deltaPriority.message.includes("未运行") : null}
              onApply={applyDeltaPriority}
            />
            <OptionRow
              icon={<Cpu size={18} />}
              title={t("optimization.aceOptimize.deltaBoost.affinityTitle")}
              description={t("optimization.aceOptimize.deltaBoost.affinityDescription")}
              isLoading={deltaAffinity.running}
              isApplied={!!deltaAffinity.message && !deltaAffinity.message.includes("未运行")}
              gameRunning={deltaAffinity.message ? !deltaAffinity.message.includes("未运行") : null}
              onApply={applyDeltaAffinity}
            />
          </VStack>
        </SettingCard>

        <SettingCard
          title={t("optimization.aceOptimize.aceSection.title")}
          subTitle={t("optimization.aceOptimize.aceSection.subtitle")}
          icon={<Shield size={18} />}
          color="#DD6B20"
        >
          <VStack align="stretch" spacing={3}>
            <OptionRow
              icon={<Gauge size={18} />}
              title={t("optimization.aceOptimize.aceLimit.title")}
              description={t("optimization.aceOptimize.aceLimit.description")}
              isLoading={acePriority.running}
              isApplied={!!acePriority.message && !acePriority.message.includes("未找到")}
              gameRunning={acePriority.message ? !acePriority.message.includes("未找到") : null}
              onApply={applyAcePriority}
            />
            <OptionRow
              icon={<Cpu size={18} />}
              title={t("optimization.aceOptimize.aceLimit.affinityTitle")}
              description={t("optimization.aceOptimize.aceLimit.affinityDescription")}
              isLoading={aceAffinity.running}
              isApplied={!!aceAffinity.message && !aceAffinity.message.includes("未找到")}
              gameRunning={aceAffinity.message ? !aceAffinity.message.includes("未找到") : null}
              onApply={applyAceAffinity}
            />
          </VStack>
        </SettingCard>
      </SimpleGrid>

      {(() => {
        const optimizeAllContent = (
          <HStack justify="space-between">
            <VStack align="flex-start" spacing={1}>
              <HStack>
                <RefreshCw size={16} color={getActiveColor()} />
                <Text fontWeight="bold" fontSize="sm" color={headingColor}>
                  {t("optimization.aceOptimize.optimizeAll.title")}
                </Text>
              </HStack>
              <Text fontSize="xs" color="gray.500">
                {t("optimization.aceOptimize.optimizeAll.description")}
              </Text>
            </VStack>
            <Button
              size="md"
              bg={getActiveColor()}
              color="white"
              _hover={{ bg: getActiveColor(), opacity: 0.9 }}
              onClick={applyAll}
              isLoading={optimizeAllLoading}
              loadingText={t("optimization.aceOptimize.optimizeAll.optimizing")}
              px={6}
              borderRadius="lg"
              leftIcon={<Zap size={16} />}
            >
              {t("optimization.aceOptimize.optimizeAll.button")}
            </Button>
          </HStack>
        );

        if (liquidGlassEnabled) {
          return <LiquidGlassCard p={5}>{optimizeAllContent}</LiquidGlassCard>;
        }

        return (
          <Box bg={useColorModeValue("white", "#111111")} borderRadius="xl" p={5} border="1px solid" borderColor={borderColorVal}>
            {optimizeAllContent}
          </Box>
        );
      })()}
    </Box>
  );
}
