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
  Switch,
  Modal,
  ModalOverlay,
  ModalContent,
  ModalHeader,
  ModalCloseButton,
  ModalBody,
  ModalFooter,
  Checkbox,
  useDisclosure,
  Tooltip,
} from "@chakra-ui/react";
import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { LazyStore } from "@tauri-apps/plugin-store";
import {
  ArrowLeft,
  Gauge,
  Cpu,
  Shield,
  Zap,
  RefreshCw,
  Settings2,
} from "lucide-react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useNavigate } from "react-router-dom";
import { useThemeColor } from "@/contexts/theme-color-context";

interface OptionState {
  running: boolean;
  message: string;
  foundCount: number; // 后端发现的进程数（与是否成功修改无关）
  modifiedCount: number; // 成功修改的进程数
}

interface AceAutoDetectStatus {
  enabled: boolean;
  is_running: boolean;
  last_check: string | null;
  total_optimized: number;
  currently_optimized: string[];
}

function formatRelativeTime(isoString: string): string {
  const date = new Date(isoString);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSecs = Math.floor(diffMs / 1000);
  
  if (diffSecs < 60) {
    return `${diffSecs}秒前`;
  }
  const diffMins = Math.floor(diffSecs / 60);
  if (diffMins < 60) {
    return `${diffMins}分钟前`;
  }
  const diffHours = Math.floor(diffMins / 60);
  if (diffHours < 24) {
    return `${diffHours}小时前`;
  }
  const diffDays = Math.floor(diffHours / 24);
  return `${diffDays}天前`;
}

const STORE = new LazyStore("ace-affinity-config.json");

function OptionRow({
  icon,
  title,
  description,
  isLoading,
  isApplied,
  gameRunning,
  needsAdmin,
  onApply,
  onSettings,
  titleBadge,
}: {
  icon: React.ReactNode;
  title: string;
  description: string;
  isLoading: boolean;
  isApplied: boolean;
  gameRunning: boolean | null;
  needsAdmin?: boolean;
  onApply: () => void;
  onSettings?: () => void;
  titleBadge?: string;
}) {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const textColor = useColorModeValue("gray.800", "#ffffff");
  const subTextColor = useColorModeValue("gray.500", "#ffffff");
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
        color={getActiveColor()}
      >
        {icon}
      </Box>
      <VStack align="flex-start" spacing={1} flex={1}>
        <HStack spacing={2} align="center" wrap="wrap">
          <Text fontSize="sm" fontWeight="bold" color={textColor}>
            {title}
          </Text>
          {titleBadge && (
            <Badge colorScheme="purple" variant="subtle" fontSize="2xs" px={2} py={0.5} borderRadius="full" whiteSpace="nowrap">
              {titleBadge}
            </Badge>
          )}
        </HStack>
        <Text fontSize="xs" color={subTextColor} lineHeight="short">
          {description}
        </Text>
      </VStack>
      <VStack align="flex-end" spacing={2} flexShrink={0}>
        <HStack spacing={2}>
          {onSettings && (
            <Tooltip label={t("optimization.aceOptimize.affinitySettings.title")}>
              <IconButton
                aria-label={t("optimization.aceOptimize.affinitySettings.title")}
                icon={<Settings2 size={16} />}
                size="sm"
                variant="outline"
                color={getActiveColor()}
                borderColor={getActiveColor()}
                _hover={{ bg: getActiveColor(), color: "white", opacity: 0.9 }}
                onClick={(e) => { e.stopPropagation(); onSettings?.(); }}
              />
            </Tooltip>
          )}
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
        </HStack>
        {gameRunning !== null && (
          <Badge
            colorScheme={needsAdmin ? "orange" : (gameRunning ? "green" : "gray")}
            variant="subtle"
            fontSize="2xs"
            px={2}
            py={0.5}
            borderRadius="full"
          >
            {needsAdmin
              ? t("optimization.aceOptimize.status.needsAdmin")
              : (gameRunning
                ? t("optimization.aceOptimize.status.processRunning")
                : t("optimization.aceOptimize.status.processNotRunning"))}
          </Badge>
        )}
      </VStack>
    </HStack>
  );

  if (liquidGlassEnabled) {
    return (
      <LiquidGlassCard p={4}>
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

function CoreSelectionModal({
  isOpen,
  onClose,
  coreCount,
  target,
  currentSavedMask,
  onSave,
}: {
  isOpen: boolean;
  onClose: () => void;
  coreCount: number;
  target: "delta" | "ace";
  currentSavedMask: number;
  onSave: (mask: number) => void;
}) {
  const { t } = useTranslation();
  const { getActiveColor } = useThemeColor();
  const modalBg = useColorModeValue("white", "#1a1a1a");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const headingColor = useColorModeValue("gray.900", "#ffffff");

  const [selectedCores, setSelectedCores] = useState<number[]>([]);

  useEffect(() => {
    const cores: number[] = [];
    for (let i = 0; i < coreCount; i++) {
      if (currentSavedMask & (1 << i)) {
        cores.push(i);
      }
    }
    setSelectedCores(cores);
  }, [currentSavedMask, coreCount, isOpen]);

  const handleToggle = (coreIndex: number) => {
    setSelectedCores(prev =>
      prev.includes(coreIndex)
        ? prev.filter(c => c !== coreIndex)
        : [...prev, coreIndex]
    );
  };

  const handleSelectAll = () => {
    setSelectedCores(Array.from({ length: coreCount }, (_, i) => i));
  };

  const handleClearAll = () => {
    setSelectedCores([]);
  };

  const handleSave = () => {
    const mask = selectedCores.reduce((acc, core) => acc | (1 << core), 0);
    onSave(mask);
    onClose();
  };

  const isDefaultForDelta = coreCount > 0 && currentSavedMask === ((1 << coreCount) - 2);
  const isDefaultForAce = currentSavedMask === 1;

  return (
    <Modal isOpen={isOpen} onClose={onClose} isCentered size="md">
      <ModalOverlay backdropFilter="blur(4px)" />
      <ModalContent bg={modalBg} borderColor={borderColor} borderRadius="xl">
        <ModalHeader color={headingColor}>
          {t("optimization.aceOptimize.affinitySettings.title")}
          <Text fontSize="xs" color="gray.500" fontWeight="normal" mt={1}>
            {t("optimization.aceOptimize.affinitySettings.description", { count: coreCount })}
          </Text>
        </ModalHeader>
        <ModalCloseButton />
        <ModalBody>
          <VStack align="stretch" spacing={1}>
            <HStack justify="space-between" mb={2}>
              <Text fontSize="xs" color="gray.500">
                CPU0 - CPU{coreCount - 1}
              </Text>
              <HStack spacing={2}>
                <Button size="xs" variant="ghost" onClick={handleSelectAll}>
                  {t("optimization.aceOptimize.affinitySettings.selectAll")}
                </Button>
                <Button size="xs" variant="ghost" onClick={handleClearAll}>
                  {t("optimization.aceOptimize.affinitySettings.clearAll")}
                </Button>
              </HStack>
            </HStack>
            <Box
              maxH="300px"
              overflowY="auto"
              sx={{
                "&::-webkit-scrollbar": { width: "4px" },
                "&::-webkit-scrollbar-thumb": { bg: "gray.600", borderRadius: "full" },
              }}
            >
              <SimpleGrid columns={4} spacing={2}>
                {Array.from({ length: coreCount }, (_, i) => (
                  <Checkbox
                    key={i}
                    isChecked={selectedCores.includes(i)}
                    onChange={() => handleToggle(i)}
                    size="sm"
                    sx={{
                      "& .chakra-checkbox__control": {
                        borderColor: getActiveColor(),
                      },
                      "& .chakra-checkbox__control[data-checked]": {
                        bg: getActiveColor(),
                        borderColor: getActiveColor(),
                        color: "white",
                      },
                    }}
                  >
                    <Text fontSize="xs" color={headingColor}>
                      CPU{i}
                    </Text>
                  </Checkbox>
                ))}
              </SimpleGrid>
            </Box>
            <Text fontSize="2xs" color="gray.500" mt={2}>
              {target === "delta"
                ? t("optimization.aceOptimize.affinitySettings.deltaDefault")
                : t("optimization.aceOptimize.affinitySettings.aceDefault")}
              {isDefaultForDelta || isDefaultForAce ? " ✓" : ""}
            </Text>
          </VStack>
        </ModalBody>
        <ModalFooter gap={3}>
          <Button variant="ghost" onClick={onClose}>
            {t("optimization.aceOptimize.affinitySettings.cancel")}
          </Button>
          <Button
            bg={getActiveColor()}
            color="white"
            _hover={{ bg: getActiveColor(), opacity: 0.9 }}
            onClick={handleSave}
          >
            {t("optimization.aceOptimize.affinitySettings.save")}
          </Button>
        </ModalFooter>
      </ModalContent>
    </Modal>
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

  const [deltaPriority, setDeltaPriority] = useState<OptionState>({ running: false, message: "", foundCount: 0, modifiedCount: 0 });
  const [deltaAffinity, setDeltaAffinity] = useState<OptionState>({ running: false, message: "", foundCount: 0, modifiedCount: 0 });
  const [acePriority, setAcePriority] = useState<OptionState>({ running: false, message: "", foundCount: 0, modifiedCount: 0 });
  const [aceAffinity, setAceAffinity] = useState<OptionState>({ running: false, message: "", foundCount: 0, modifiedCount: 0 });
  const [aceEfficiency, setAceEfficiency] = useState<OptionState>({ running: false, message: "", foundCount: 0, modifiedCount: 0 });
  const [optimizeAllLoading, setOptimizeAllLoading] = useState(false);

  // 核心选择配置
  const [deltaAffinityMask, setDeltaAffinityMask] = useState<number | null>(null);
  const [aceAffinityMask, setAceAffinityMask] = useState<number | null>(null);
  const [coreCount, setCoreCount] = useState(0);
  const [settingsTarget, setSettingsTarget] = useState<"delta" | "ace">("delta");
  const { isOpen: isSettingsOpen, onOpen: onSettingsOpen, onClose: onSettingsClose } = useDisclosure();

  const getDefaultDeltaMask = useCallback((cores: number) => cores > 0 ? (1 << cores) - 2 : 0, []);
  const getDefaultAceMask = useCallback(() => 1, []);

  // 从 Store 加载已保存的配置
  useEffect(() => {
    const cores = navigator.hardwareConcurrency || 4;
    setCoreCount(cores);

    const loadConfig = async () => {
      const savedDelta = await STORE.get<number>("delta_affinity_mask");
      const savedAce = await STORE.get<number>("ace_affinity_mask");
      setDeltaAffinityMask(savedDelta ?? getDefaultDeltaMask(cores));
      setAceAffinityMask(savedAce ?? getDefaultAceMask());
    };
    loadConfig();
  }, [getDefaultDeltaMask, getDefaultAceMask]);

  // ACE 自动检测状态
  const [autoDetectEnabled, setAutoDetectEnabled] = useState<boolean | null>(null); // null = 加载中
  const [autoDetectStatus, setAutoDetectStatus] = useState<AceAutoDetectStatus | null>(null);
  const [autoDetectLoading, setAutoDetectLoading] = useState(false);
  const pollIntervalRef = useRef<number | null>(null);
  const lastBackendEnabledRef = useRef<boolean | null>(null);
  const isMountedRef = useRef(true);

  // 加载自动检测状态并开始轮询
  useEffect(() => {
    isMountedRef.current = true;
    const loadStatus = async () => {
      try {
        const status = await invoke<AceAutoDetectStatus>("get_ace_auto_detect_status");
        if (!isMountedRef.current) return;
        setAutoDetectStatus(status);
        setAutoDetectEnabled(status.enabled);
        lastBackendEnabledRef.current = status.enabled;
      } catch (e) {
        console.error("Failed to load auto detect status:", e);
        if (!isMountedRef.current) return;
        setAutoDetectEnabled(false);
      }
    };
    loadStatus();

    // 每 3 秒轮询一次状态
    pollIntervalRef.current = window.setInterval(loadStatus, 3000);
    return () => {
      isMountedRef.current = false;
      if (pollIntervalRef.current) clearInterval(pollIntervalRef.current);
    };
  }, []);

  const handleToggleAutoDetect = useCallback(async (checked: boolean | React.ChangeEvent<HTMLInputElement>) => {
    // Chakra UI Switch 在某些版本中可能传递 event 而非 boolean
    const isChecked = typeof checked === 'boolean' ? checked : checked.target.checked;
    
    // 防抖：如果后端状态已经是目标状态，不重复发送命令
    if (lastBackendEnabledRef.current === isChecked) {
      return;
    }
    
    setAutoDetectLoading(true);
    try {
      await invoke("set_ace_auto_detect", { enabled: isChecked });
      if (!isMountedRef.current) return;
      setAutoDetectEnabled(isChecked);
      lastBackendEnabledRef.current = isChecked;
      toast({
        title: isChecked
          ? t("optimization.aceOptimize.autoDetect.enabled")
          : t("optimization.aceOptimize.autoDetect.disabled"),
        status: "success",
        duration: 2000,
      });
    } catch (e: any) {
      if (!isMountedRef.current) return;
      setAutoDetectEnabled(lastBackendEnabledRef.current ?? false);
      toast({ title: String(e), status: "error", duration: 2000 });
    } finally {
      if (isMountedRef.current) setAutoDetectLoading(false);
    }
  }, [toast, t]);

  const applyDeltaPriority = useCallback(async () => {
    setDeltaPriority(prev => ({ ...prev, running: true }));
    try {
      const result = await invoke<{ success: boolean; message: string; was_running: boolean }>("boost_delta_force_priority");
      setDeltaPriority({ running: false, message: result.message, foundCount: result.was_running ? 1 : 0, modifiedCount: result.was_running ? 1 : 0 });
      toast({
        title: result.was_running ? t("optimization.aceOptimize.deltaBoost.success") : t("optimization.aceOptimize.deltaBoost.notRunning"),
        status: result.was_running ? "success" : "info",
        duration: 2000,
      });
    } catch (e: any) {
      setDeltaPriority({ running: false, message: String(e), foundCount: 0, modifiedCount: 0 });
      toast({ title: String(e), status: "error", duration: 2000 });
    }
  }, [toast, t]);

  const applyDeltaAffinity = useCallback(async () => {
    setDeltaAffinity(prev => ({ ...prev, running: true }));
    try {
      const mask = deltaAffinityMask ?? getDefaultDeltaMask(coreCount);
      const result = await invoke<{ success: boolean; message: string; was_running: boolean }>("boost_delta_force_affinity_with_mask", { mask });
      setDeltaAffinity({ running: false, message: result.message, foundCount: result.was_running ? 1 : 0, modifiedCount: result.was_running ? 1 : 0 });
      toast({
        title: result.was_running ? t("optimization.aceOptimize.deltaBoost.affinitySuccess") : t("optimization.aceOptimize.deltaBoost.notRunning"),
        status: result.was_running ? "success" : "info",
        duration: 2000,
      });
    } catch (e: any) {
      setDeltaAffinity({ running: false, message: String(e), foundCount: 0, modifiedCount: 0 });
      toast({ title: String(e), status: "error", duration: 2000 });
    }
  }, [toast, t, deltaAffinityMask, getDefaultDeltaMask, coreCount]);

  const applyAcePriority = useCallback(async () => {
    setAcePriority(prev => ({ ...prev, running: true }));
    try {
      const result = await invoke<{ success: boolean; message: string; count: number; found_count: number }>("limit_ace_priority");
      setAcePriority({ running: false, message: result.message, foundCount: result.found_count ?? 0, modifiedCount: result.count ?? 0 });
      toast({
        title: result.count > 0
          ? t("optimization.aceOptimize.aceLimit.success", { count: result.count })
          : (result.found_count ?? 0) > 0
            ? result.message
            : t("optimization.aceOptimize.aceLimit.notRunning"),
        status: result.count > 0 ? "success" : "info",
        duration: 2000,
      });
    } catch (e: any) {
      setAcePriority({ running: false, message: String(e), foundCount: 0, modifiedCount: 0 });
      toast({ title: String(e), status: "error", duration: 2000 });
    }
  }, [toast, t]);

  const applyAceAffinity = useCallback(async () => {
    setAceAffinity(prev => ({ ...prev, running: true }));
    try {
      const mask = aceAffinityMask ?? getDefaultAceMask();
      const result = await invoke<{ success: boolean; message: string; count: number; found_count: number }>("restrict_ace_affinity_with_mask", { mask });
      setAceAffinity({ running: false, message: result.message, foundCount: result.found_count ?? 0, modifiedCount: result.count ?? 0 });
      toast({
        title: result.count > 0
          ? t("optimization.aceOptimize.aceLimit.affinitySuccess", { count: result.count })
          : (result.found_count ?? 0) > 0
            ? result.message
            : t("optimization.aceOptimize.aceLimit.notRunning"),
        status: result.count > 0 ? "success" : "info",
        duration: 2000,
      });
    } catch (e: any) {
      setAceAffinity({ running: false, message: String(e), foundCount: 0, modifiedCount: 0 });
      toast({ title: String(e), status: "error", duration: 2000 });
    }
  }, [toast, t, aceAffinityMask, getDefaultAceMask]);

  const applyAceEfficiency = useCallback(async () => {
    setAceEfficiency(prev => ({ ...prev, running: true }));
    try {
      const result = await invoke<{ success: boolean; message: string; count: number; found_count: number }>("set_ace_efficiency_mode");
      setAceEfficiency({ running: false, message: result.message, foundCount: result.found_count ?? 0, modifiedCount: result.count ?? 0 });
      toast({
        title: result.count > 0
          ? t("optimization.aceOptimize.aceEfficiency.success", { count: result.count })
          : (result.found_count ?? 0) > 0
            ? result.message
            : t("optimization.aceOptimize.aceEfficiency.notRunning"),
        status: result.count > 0 ? "success" : "info",
        duration: 2000,
      });
    } catch (e: any) {
      setAceEfficiency({ running: false, message: String(e), foundCount: 0, modifiedCount: 0 });
      toast({ title: String(e), status: "error", duration: 2000 });
    }
  }, [toast, t]);

  const openDeltaSettings = useCallback(() => {
    setSettingsTarget("delta");
    onSettingsOpen();
  }, [onSettingsOpen]);

  const openAceSettings = useCallback(() => {
    setSettingsTarget("ace");
    onSettingsOpen();
  }, [onSettingsOpen]);

  const handleSaveAffinityConfig = useCallback(async (mask: number) => {
    if (settingsTarget === "delta") {
      setDeltaAffinityMask(mask);
      await STORE.set("delta_affinity_mask", mask);
    } else {
      setAceAffinityMask(mask);
      await STORE.set("ace_affinity_mask", mask);
    }
    toast({
      title: t("optimization.aceOptimize.affinitySettings.configSaved"),
      status: "success",
      duration: 2000,
    });
  }, [settingsTarget, t, toast]);

  const applyAll = useCallback(async () => {
    setOptimizeAllLoading(true);
    try {
      const result = await invoke<{ success: boolean; message: string; delta_boosted: boolean; ace_limited: boolean; ace_count: number }>("optimize_all_game_processes");
      if (result.delta_boosted) {
        setDeltaPriority({ running: false, message: t("optimization.aceOptimize.status.optimized"), foundCount: 1, modifiedCount: 1 });
        setDeltaAffinity({ running: false, message: t("optimization.aceOptimize.status.optimized"), foundCount: 1, modifiedCount: 1 });
      }
      if (result.ace_limited) {
        setAcePriority({ running: false, message: t("optimization.aceOptimize.status.optimized"), foundCount: result.ace_count, modifiedCount: result.ace_count });
        setAceAffinity({ running: false, message: t("optimization.aceOptimize.status.optimized"), foundCount: result.ace_count, modifiedCount: result.ace_count });
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
              isApplied={deltaPriority.modifiedCount > 0}
              gameRunning={deltaPriority.foundCount > 0 ? true : (deltaPriority.message ? false : null)}
              onApply={applyDeltaPriority}
            />
            <OptionRow
              icon={<Cpu size={18} />}
              title={t("optimization.aceOptimize.deltaBoost.affinityTitle")}
              description={t("optimization.aceOptimize.deltaBoost.affinityDescription")}
              isLoading={deltaAffinity.running}
              isApplied={deltaAffinity.modifiedCount > 0}
              gameRunning={deltaAffinity.foundCount > 0 ? true : (deltaAffinity.message ? false : null)}
              onApply={applyDeltaAffinity}
              onSettings={openDeltaSettings}
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
              isApplied={acePriority.modifiedCount > 0}
              gameRunning={acePriority.foundCount > 0 ? true : (acePriority.message ? false : null)}
              needsAdmin={acePriority.foundCount > 0 && acePriority.modifiedCount === 0}
              onApply={applyAcePriority}
            />
            <OptionRow
              icon={<Cpu size={18} />}
              title={t("optimization.aceOptimize.aceLimit.affinityTitle")}
              description={t("optimization.aceOptimize.aceLimit.affinityDescription")}
              isLoading={aceAffinity.running}
              isApplied={aceAffinity.modifiedCount > 0}
              gameRunning={aceAffinity.foundCount > 0 ? true : (aceAffinity.message ? false : null)}
              needsAdmin={aceAffinity.foundCount > 0 && aceAffinity.modifiedCount === 0}
              onApply={applyAceAffinity}
              onSettings={openAceSettings}
            />
            <OptionRow
              icon={<Zap size={18} />}
              title={t("optimization.aceOptimize.aceEfficiency.title")}
              description={t("optimization.aceOptimize.aceEfficiency.description")}
              isLoading={aceEfficiency.running}
              isApplied={aceEfficiency.modifiedCount > 0}
              gameRunning={aceEfficiency.foundCount > 0 ? true : (aceEfficiency.message ? false : null)}
              needsAdmin={aceEfficiency.foundCount > 0 && aceEfficiency.modifiedCount === 0}
              onApply={applyAceEfficiency}
            />

            {/* 自动检测并优化 */}
            <Box
              mt={2}
              pt={3}
              borderTop="1px solid"
              borderColor={useColorModeValue("gray.200", "#333333")}
            >
              <HStack justify="space-between" align="center">
                <VStack align="flex-start" spacing={2}>
                  <Text fontWeight="bold" fontSize="sm" color={headingColor}>
                    {t("optimization.aceOptimize.autoDetect.title")}
                  </Text>
                  {autoDetectStatus && (
                    <HStack spacing={2} align="center">
                      <Text fontSize="xs" color="gray.500">
                        {t("optimization.aceOptimize.autoDetect.lastCheck")}:{" "}
                        {autoDetectStatus.last_check
                          ? formatRelativeTime(autoDetectStatus.last_check)
                          : t("optimization.aceOptimize.autoDetect.never")}
                      </Text>
                      {autoDetectStatus.currently_optimized.length > 0 && (
                        <Badge colorScheme="green" variant="subtle" fontSize="2xs">
                          {t("optimization.aceOptimize.autoDetect.optimizing", { count: autoDetectStatus.currently_optimized.length })}
                        </Badge>
                      )}
                    </HStack>
                  )}
                </VStack>
                <Switch
                  isChecked={autoDetectEnabled ?? false}
                  onChange={handleToggleAutoDetect}
                  isDisabled={autoDetectLoading || autoDetectEnabled === null}
                  size="md"
                  sx={{
                    "& > span": {
                      bg: autoDetectEnabled ? getActiveColor() : useColorModeValue("gray.200", "gray.600"),
                    },
                    "& > span > span": {
                      bg: "white",
                    },
                    "&:hover > span": {
                      bg: autoDetectEnabled ? getActiveColor() : useColorModeValue("gray.200", "gray.600"),
                    },
                    "&[data-disabled] > span": {
                      bg: useColorModeValue("gray.200", "gray.600"),
                    },
                  }}
                  aria-label={t("optimization.aceOptimize.autoDetect.title")}
                />
              </HStack>
            </Box>
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

      <CoreSelectionModal
        isOpen={isSettingsOpen}
        onClose={onSettingsClose}
        coreCount={coreCount}
        target={settingsTarget}
        currentSavedMask={settingsTarget === "delta" ? (deltaAffinityMask ?? getDefaultDeltaMask(coreCount)) : (aceAffinityMask ?? getDefaultAceMask())}
        onSave={handleSaveAffinityConfig}
      />
    </Box>
  );
}
