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
  onRestore,
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
  onRestore?: () => void;
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
    <HStack align="flex-start" spacing={4} flexWrap="wrap" gap={3}>
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
      <VStack align="flex-start" spacing={1} flex={1} minW="0">
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
          {onRestore && (
            <Button
              size="sm"
              variant="outline"
              colorScheme="red"
              onClick={onRestore}
              isLoading={isLoading}
              loadingText=""
              px={4}
              borderRadius="lg"
              minW="72px"
            >
              {t("optimization.aceOptimize.restore")}
            </Button>
          )}
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
  currentSavedMask,
  onSave,
}: {
  isOpen: boolean;
  onClose: () => void;
  coreCount: number;
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
      if (((currentSavedMask / Math.pow(2, i)) % 2) >= 1) {
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
    const mask = selectedCores.reduce((acc, core) => acc + Math.pow(2, core), 0);
    onSave(mask);
    onClose();
  };

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
            <HStack justify="space-between" mb={2} flexWrap="wrap" gap={2}>
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
              <SimpleGrid columns={{ base: 3, sm: 4, md: 6 }} spacing={2}>
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
              {t("optimization.aceOptimize.affinitySettings.aceDefault")}
              {isDefaultForAce ? " ✓" : ""}
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
  const { getActiveColor } = useThemeColor();

  const [acePriority, setAcePriority] = useState<OptionState>({ running: false, message: "", foundCount: 0, modifiedCount: 0 });
  const [aceAffinity, setAceAffinity] = useState<OptionState>({ running: false, message: "", foundCount: 0, modifiedCount: 0 });
  const [aceEfficiency, setAceEfficiency] = useState<OptionState>({ running: false, message: "", foundCount: 0, modifiedCount: 0 });
  const [aceRegistry, setAceRegistry] = useState<OptionState>({ running: false, message: "", foundCount: 0, modifiedCount: 0 });

  // 核心选择配置
  const [aceAffinityMask, setAceAffinityMask] = useState<number | null>(null);
  const [coreCount, setCoreCount] = useState(0);
  const { isOpen: isSettingsOpen, onOpen: onSettingsOpen, onClose: onSettingsClose } = useDisclosure();

  const getDefaultAceMask = useCallback(() => 1, []);

  // 从 Store 加载已保存的配置
  useEffect(() => {
    const cores = navigator.hardwareConcurrency || 4;
    setCoreCount(cores);

    const loadConfig = async () => {
      const savedAce = await STORE.get<number>("ace_affinity_mask");
      // 过滤无效的旧值（JS 32 位位移曾产生负数），避免再次传给后端
      setAceAffinityMask(savedAce !== undefined && savedAce > 0 ? savedAce : getDefaultAceMask());
    };
    loadConfig();
  }, [getDefaultAceMask]);

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

  const applyAceRegistry = useCallback(async () => {
    setAceRegistry(prev => ({ ...prev, running: true }));
    try {
      const result = await invoke<{ success: boolean; message: string }>("apply_ace_registry_limits");
      setAceRegistry({ running: false, message: result.message, foundCount: 0, modifiedCount: result.success ? 1 : 0 });
      toast({
        title: result.success
          ? t("optimization.aceOptimize.aceRegistry.success")
          : result.message,
        status: result.success ? "success" : "error",
        duration: 3000,
      });
    } catch (e: any) {
      setAceRegistry({ running: false, message: String(e), foundCount: 0, modifiedCount: 0 });
      toast({ title: String(e), status: "error", duration: 3000 });
    }
  }, [toast, t]);

  const restoreAceRegistry = useCallback(async () => {
    setAceRegistry(prev => ({ ...prev, running: true }));
    try {
      const result = await invoke<{ success: boolean; message: string }>("restore_ace_registry_limits");
      setAceRegistry({ running: false, message: result.message, foundCount: 0, modifiedCount: 0 });
      toast({
        title: result.success
          ? t("optimization.aceOptimize.aceRegistry.restored")
          : result.message,
        status: result.success ? "success" : "error",
        duration: 3000,
      });
    } catch (e: any) {
      setAceRegistry({ running: false, message: String(e), foundCount: 0, modifiedCount: 0 });
      toast({ title: String(e), status: "error", duration: 3000 });
    }
  }, [toast, t]);

  const handleOpenAceSettings = useCallback(() => {
    onSettingsOpen();
  }, [onSettingsOpen]);

  const handleSaveAffinityConfig = useCallback(async (mask: number) => {
    setAceAffinityMask(mask);
    await STORE.set("ace_affinity_mask", mask);
    toast({
      title: t("optimization.aceOptimize.affinitySettings.configSaved"),
      status: "success",
      duration: 2000,
    });
  }, [t, toast]);

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

      <Box maxW="900px" mx="auto" mb={5}>
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
              onSettings={handleOpenAceSettings}
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
            <OptionRow
              icon={<Settings2 size={18} />}
              title={t("optimization.aceOptimize.aceRegistry.title")}
              description={t("optimization.aceOptimize.aceRegistry.description")}
              isLoading={aceRegistry.running}
              isApplied={aceRegistry.modifiedCount > 0}
              gameRunning={null}
              onApply={applyAceRegistry}
              onRestore={restoreAceRegistry}
              titleBadge={t("optimization.aceOptimize.aceRegistry.badge")}
            />

            {/* 自动检测并优化 */}
            <Box
              mt={2}
              pt={3}
              borderTop="1px solid"
              borderColor={useColorModeValue("gray.200", "#333333")}
            >
              <HStack justify="space-between" align="center" flexWrap="wrap" gap={3}>
                <VStack align="flex-start" spacing={2}>
                  <Text fontWeight="bold" fontSize="sm" color={headingColor}>
                    {t("optimization.aceOptimize.autoDetect.title")}
                  </Text>
                  {autoDetectStatus && (
                    <HStack spacing={2} align="center" flexWrap="wrap">
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
      </Box>

      <CoreSelectionModal
        isOpen={isSettingsOpen}
        onClose={onSettingsClose}
        coreCount={coreCount}
        currentSavedMask={aceAffinityMask ?? getDefaultAceMask()}
        onSave={handleSaveAffinityConfig}
      />
    </Box>
  );
}
