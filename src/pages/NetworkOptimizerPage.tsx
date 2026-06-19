import { useState, useEffect, useCallback } from "react";
import {
  Box,
  Heading,
  VStack,
  HStack,
  Text,
  SimpleGrid,
  Flex,
  Switch,
  Button,
  IconButton,
  Input,
  useColorModeValue,
  useToast,
  Spinner,
} from "@chakra-ui/react";
import { ArrowLeft, Globe } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { LazyStore } from "@tauri-apps/plugin-store";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import {
  dnsPresets,
  networkOptimizerItems,
  type DnsPreset,
  type NetworkOptimizerItem,
} from "@/config/network-optimizer";

const STORE_KEY = "network_optimizer_states";
const store = new LazyStore("settings.json");

export default function NetworkOptimizerPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { liquidGlassEnabled } = useBackground();
  const toast = useToast();
  const { getActiveColor, getHoverColor, getContrastTextColor } = useThemeColor();

  const [scannedStates, setScannedStates] = useState<Record<string, boolean>>({});
  const [savedStates, setSavedStates] = useState<Record<string, boolean>>({});
  const [isInitialScanning, setIsInitialScanning] = useState(true);
  const [isBatchOptimizing, setIsBatchOptimizing] = useState(false);
  const [togglingItems, setTogglingItems] = useState<Set<string>>(new Set());

  const [currentDns, setCurrentDns] = useState<{ primary: string; secondary: string }>({
    primary: "",
    secondary: "",
  });
  const [applyingDnsId, setApplyingDnsId] = useState<string | null>(null);
  const [isRestoringDns, setIsRestoringDns] = useState(false);
  const [customPrimary, setCustomPrimary] = useState("");
  const [customSecondary, setCustomSecondary] = useState("");
  const [isApplyingCustomDns, setIsApplyingCustomDns] = useState(false);

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const cardBg = useColorModeValue("white", "#111111");
  const cardBorder = useColorModeValue("gray.200", "#333333");

  const activeColor = getActiveColor();
  const contrastText = getContrastTextColor();
  const hoverBg = getHoverColor(false);

  // 初始化：加载网络状态 + DNS 配置（后端 check_network_tweak_states 返回 NetworTweakState）
  useEffect(() => {
    let cancelled = false;
    async function init() {
      const startTime = Date.now();
      const [savedResult, scannedResult] = await Promise.allSettled([
        store.get<Record<string, boolean>>(STORE_KEY),
        invoke("check_network_tweak_states").catch(() => null),
      ]);
      if (cancelled) return;
      const saved =
        savedResult.status === "fulfilled" && savedResult.value
          ? savedResult.value
          : {};
      const scanned = scannedResult.status === "fulfilled" ? scannedResult.value : null;

      // 解析 NetworTweakState -> Record<string, boolean> + DNS
      const scannedMap: Record<string, boolean> = {};
      let dns = { primary: "", secondary: "" };
      if (scanned && typeof scanned === "object") {
        const s = scanned as Record<string, unknown>;
        scannedMap["tcp_congestion_optimized"] = !!s.tcp_congestion_optimized;
        scannedMap["chimney_offload"] = !!s.chimney_offload;
        scannedMap["nagle_optimized"] = !!s.nagle_optimized;
        scannedMap["adapter_power_saving_off"] = !!s.adapter_power_saving_off;
        dns = {
          primary: String(s.dns_primary ?? ""),
          secondary: String(s.dns_secondary ?? ""),
        };
      }

      const remaining = Math.max(0, 600 - (Date.now() - startTime));
      if (remaining > 0) {
        await new Promise((r) => setTimeout(r, remaining));
      }
      if (cancelled) return;
      setSavedStates(saved);
      setScannedStates(scannedMap);
      setCurrentDns(dns);
      setIsInitialScanning(false);
    }
    init();
    return () => {
      cancelled = true;
    };
  }, []);

  const persistStates = useCallback(async (states: Record<string, boolean>) => {
    try {
      await store.set(STORE_KEY, states);
      await store.save();
    } catch {}
  }, []);

  const getItemState = useCallback(
    (item: NetworkOptimizerItem): boolean => {
      if (savedStates[item.id] !== undefined) return savedStates[item.id];
      const scanned = scannedStates[item.stateKey];
      if (scanned !== undefined) return scanned;
      return false;
    },
    [scannedStates, savedStates],
  );

  // 切换单个网络优化项
  const toggleItem = useCallback(
    async (item: NetworkOptimizerItem, enable: boolean) => {
      const cmd = enable ? item.enableCmd : item.disableCmd;
      setTogglingItems((prev) => new Set(prev).add(item.id));
      try {
        await invoke(cmd);
        const newSaved = { ...savedStates, [item.id]: enable };
        setSavedStates(newSaved);
        setScannedStates((prev) => ({ ...prev, [item.stateKey]: enable }));
        persistStates(newSaved);
        toast({
          title: enable
            ? t("networkOptimize.optimized")
            : t("networkOptimize.reverted"),
          description: t(item.titleKey),
          status: "success",
          duration: 2000,
          isClosable: true,
        });
      } catch (err) {
        toast({
          title: t("networkOptimize.operationError"),
          description: String(err),
          status: "error",
          duration: 3000,
          isClosable: true,
        });
      } finally {
        setTogglingItems((prev) => {
          const next = new Set(prev);
          next.delete(item.id);
          return next;
        });
      }
    },
    [savedStates, persistStates, toast, t],
  );

  // 批量优化
  const handleBatchEnable = useCallback(async () => {
    setIsBatchOptimizing(true);
    try {
      await invoke("batch_network_enable");
      const newSaved: Record<string, boolean> = {};
      const newScanned: Record<string, boolean> = {};
      for (const item of networkOptimizerItems) {
        newSaved[item.id] = true;
        newScanned[item.stateKey] = true;
      }
      setSavedStates(newSaved);
      setScannedStates(newScanned);
      persistStates(newSaved);
      toast({
        title: t("networkOptimize.batchOptimized"),
        status: "success",
        duration: 3000,
        isClosable: true,
      });
    } catch (err) {
      toast({
        title: t("networkOptimize.batchError"),
        description: String(err),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    } finally {
      setIsBatchOptimizing(false);
    }
  }, [persistStates, toast, t]);

  // 批量恢复
  const handleBatchDisable = useCallback(async () => {
    setIsBatchOptimizing(true);
    try {
      await invoke("batch_network_disable");
      const newScanned: Record<string, boolean> = {};
      for (const item of networkOptimizerItems) {
        newScanned[item.stateKey] = false;
      }
      setSavedStates({});
      setScannedStates(newScanned);
      persistStates({});
      toast({
        title: t("networkOptimize.batchReverted"),
        status: "success",
        duration: 3000,
        isClosable: true,
      });
    } catch (err) {
      toast({
        title: t("networkOptimize.batchError"),
        description: String(err),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    } finally {
      setIsBatchOptimizing(false);
    }
  }, [persistStates, toast, t]);

  // 应用 DNS
  const applyDns = useCallback(
    async (primary: string, secondary: string) => {
      try {
        await invoke("set_dns_servers", { dnsPrimary: primary, dnsSecondary: secondary });
        setCurrentDns({ primary, secondary });
        toast({
          title: t("networkOptimize.dnsApplied"),
          description: `${primary}${secondary ? " / " + secondary : ""}`,
          status: "success",
          duration: 3000,
          isClosable: true,
        });
      } catch (err) {
        toast({
          title: t("networkOptimize.applyError"),
          description: String(err),
          status: "error",
          duration: 3000,
          isClosable: true,
        });
      }
    },
    [toast, t],
  );

  // 应用预设 DNS
  const handleApplyPreset = useCallback(
    async (preset: DnsPreset) => {
      setApplyingDnsId(preset.id);
      try {
        await applyDns(preset.primary, preset.secondary);
      } finally {
        setApplyingDnsId(null);
      }
    },
    [applyDns],
  );

  // 恢复自动获取 DNS
  const handleRestoreDns = useCallback(async () => {
    setIsRestoringDns(true);
    try {
      await invoke("restore_dns_servers");
      setCurrentDns({ primary: "", secondary: "" });
      toast({
        title: t("networkOptimize.dnsRestored"),
        status: "success",
        duration: 3000,
        isClosable: true,
      });
    } catch (err) {
      toast({
        title: t("networkOptimize.applyError"),
        description: String(err),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    } finally {
      setIsRestoringDns(false);
    }
  }, [toast, t]);

  // 应用自定义 DNS
  const handleApplyCustomDns = useCallback(async () => {
    if (!customPrimary.trim()) {
      toast({
        title: t("networkOptimize.dnsRequired"),
        status: "warning",
        duration: 2000,
        isClosable: true,
      });
      return;
    }
    setIsApplyingCustomDns(true);
    try {
      await applyDns(customPrimary.trim(), customSecondary.trim());
    } finally {
      setIsApplyingCustomDns(false);
    }
  }, [customPrimary, customSecondary, applyDns, toast, t]);

  // DNS 预设卡片
  function DnsCard({ preset }: { preset: DnsPreset }) {
    const isApplied =
      currentDns.primary === preset.primary &&
      currentDns.secondary === preset.secondary;
    const isLoading = applyingDnsId === preset.id;

    const cardContent = (
      <VStack align="start" spacing={2} w="full">
        <HStack spacing={3} align="center" w="full">
          <Box
            w={10}
            h={10}
            borderRadius="lg"
            bg={`${preset.iconColor}20`}
            display="flex"
            alignItems="center"
            justifyContent="center"
            color={preset.iconColor}
            flexShrink={0}
          >
            <Globe size={20} />
          </Box>
          <Box flex={1} minW={0}>
            <HStack spacing={2}>
              <Text color={headingColor} fontSize="sm" fontWeight="bold" noOfLines={1}>
                {preset.name}
              </Text>
              {isApplied && (
                <Text color={activeColor} fontSize="xs" fontWeight="bold" flexShrink={0}>
                  {t("networkOptimize.dns.applied")}
                </Text>
              )}
            </HStack>
          </Box>
        </HStack>
        <VStack align="start" spacing={0} w="full" px={1}>
          <Text color={subTextColor} fontSize="xs">
            {preset.primary}
          </Text>
          {preset.secondary ? (
            <Text color={subTextColor} fontSize="xs">
              {preset.secondary}
            </Text>
          ) : null}
        </VStack>
        <Button
          size="sm"
          w="full"
          onClick={() => handleApplyPreset(preset)}
          isLoading={isLoading}
          loadingText={t("networkOptimize.dns.apply")}
          {...(isApplied
            ? {
                variant: "outline",
                sx: {
                  borderColor: activeColor,
                  color: activeColor,
                  _hover: { bg: hoverBg },
                },
              }
            : {
                bg: activeColor,
                color: contrastText,
                _hover: { opacity: 0.9 },
                _active: { transform: "scale(0.97)" },
              })}
        >
          {t("networkOptimize.dns.apply")}
        </Button>
      </VStack>
    );

    if (liquidGlassEnabled) {
      return (
        <LiquidGlassCard w="full" p={4}>
          {cardContent}
        </LiquidGlassCard>
      );
    }

    return (
      <Box
        w="full"
        bg={cardBg}
        borderRadius="xl"
        border="1px solid"
        borderColor={isApplied ? activeColor : cardBorder}
        p={4}
        transition="all 0.2s"
        _hover={{
          borderColor: preset.iconColor,
          boxShadow: `0 0 12px ${preset.iconColor}20`,
        }}
      >
        {cardContent}
      </Box>
    );
  }

  // 网络优化项卡片
  function OptimizeCard({ item }: { item: NetworkOptimizerItem }) {
    const isOptimized = getItemState(item);
    const isToggling = togglingItems.has(item.id);
    const IconComponent = item.icon;

    const cardContent = (
      <Flex justify="space-between" align="center" gap={3}>
        <HStack spacing={3} align="center" flex={1} minW={0}>
          <Box
            w={10}
            h={10}
            borderRadius="lg"
            bg={`${item.color}20`}
            display="flex"
            alignItems="center"
            justifyContent="center"
            color={item.color}
            flexShrink={0}
          >
            <IconComponent size={20} />
          </Box>
          <Box minW={0} flex={1}>
            <Text
              color={headingColor}
              fontSize="sm"
              fontWeight="bold"
              noOfLines={1}
            >
              {t(item.titleKey)}
            </Text>
            <Text color={subTextColor} fontSize="xs" noOfLines={2} mt={0.5}>
              {t(item.descKey)}
            </Text>
          </Box>
        </HStack>
        <Switch
          isChecked={isOptimized}
          isDisabled={isToggling}
          onChange={() => toggleItem(item, !isOptimized)}
          sx={{
            "& .chakra-switch__track[data-checked]": {
              bg: activeColor,
            },
          }}
          size="md"
        />
      </Flex>
    );

    if (liquidGlassEnabled) {
      return (
        <LiquidGlassCard w="full" p={4}>
          {cardContent}
        </LiquidGlassCard>
      );
    }

    return (
      <Box
        w="full"
        bg={cardBg}
        borderRadius="xl"
        border="1px solid"
        borderColor={cardBorder}
        p={4}
        transition="all 0.2s"
        _hover={{
          borderColor: item.color,
          boxShadow: `0 0 12px ${item.color}20`,
        }}
      >
        {cardContent}
      </Box>
    );
  }

  // Scanning state
  if (isInitialScanning) {
    return (
      <Box pt={8}>
        {liquidGlassEnabled ? (
          <LiquidGlassCard w="full" boxShadow="2xl" overflow="hidden" position="relative" p={6}>
            <Flex w="full" minH="360px" align="center" justify="center" direction="column" gap={4}>
              <Spinner size="xl" color={activeColor} thickness="3px" />
              <Text color={subTextColor} fontSize="sm">
                {t("networkOptimize.scanning")}
              </Text>
            </Flex>
          </LiquidGlassCard>
        ) : (
          <Box bg={cardBg} borderRadius="xl" borderWidth="1px" borderColor={cardBorder} w="full" boxShadow="2xl" overflow="hidden" position="relative" p={6}>
            <Flex w="full" minH="360px" align="center" justify="center" direction="column" gap={4}>
              <Spinner size="xl" color={activeColor} thickness="3px" />
              <Text color={subTextColor} fontSize="sm">
                {t("networkOptimize.scanning")}
              </Text>
            </Flex>
          </Box>
        )}
      </Box>
    );
  }

  const content = (
    <VStack align="start" spacing={6}>
      {/* 标题 */}
      <Flex
        w="full"
        justify="space-between"
        align={{ base: "start", md: "center" }}
        direction={{ base: "column", md: "row" }}
        gap={3}
      >
        <HStack spacing={3}>
          <IconButton
            aria-label={t("builtinTools.back")}
            icon={<ArrowLeft size={20} />}
            variant="ghost"
            onClick={() => navigate("/optimize")}
            color={headingColor}
          />
          <Heading size="lg" color={headingColor}>
            {t("networkOptimize.pageTitle")}
          </Heading>
        </HStack>
        <HStack spacing={2}>
          <Button
            size="sm"
            onClick={handleBatchEnable}
            isLoading={isBatchOptimizing}
            loadingText={t("networkOptimize.optimizing")}
            bg={activeColor}
            color={contrastText}
            _hover={{ opacity: 0.9 }}
            _active={{ transform: "scale(0.97)" }}
          >
            {t("networkOptimize.batch.enable")}
          </Button>
          <Button
            size="sm"
            onClick={handleBatchDisable}
            isLoading={isBatchOptimizing}
            loadingText={t("networkOptimize.optimizing")}
            variant="outline"
            sx={{
              borderColor: activeColor,
              color: activeColor,
              _hover: { bg: hoverBg },
            }}
          >
            {t("networkOptimize.batch.disable")}
          </Button>
        </HStack>
      </Flex>

      {/* Section 1: DNS 设置 */}
      <Box w="full">
        <Heading
          as="h3"
          fontSize="md"
          fontWeight="bold"
          color={headingColor}
          mb={3}
          position="relative"
          pl={3}
          sx={{
            "&::before": {
              content: '""',
              position: "absolute",
              left: 0,
              top: "50%",
              transform: "translateY(-50%)",
              width: "3px",
              height: "16px",
              borderRadius: "full",
              bg: activeColor,
            },
          }}
        >
          {t("networkOptimize.dns.title")}
        </Heading>
        {/* 当前 DNS 状态 */}
        {(() => {
          const dnsContent = (
            <HStack justify="space-between" align="center">
              <VStack align="start" spacing={1}>
                <Text fontSize="xs" color={subTextColor}>
                  {t("networkOptimize.currentDns")}
                </Text>
                {currentDns.primary ? (
                  <Text fontSize="sm" fontWeight="bold" color={headingColor}>
                    {currentDns.primary}
                    {currentDns.secondary ? ` / ${currentDns.secondary}` : ""}
                  </Text>
                ) : (
                  <Text fontSize="sm" color={subTextColor}>
                    {t("networkOptimize.noDnsConfig")}
                  </Text>
                )}
              </VStack>
              <Button
                size="sm"
                onClick={handleRestoreDns}
                isLoading={isRestoringDns}
                variant="outline"
                sx={{
                  borderColor: activeColor,
                  color: activeColor,
                  _hover: { bg: hoverBg },
                }}
              >
                {t("networkOptimize.dns.restore")}
              </Button>
            </HStack>
          );
          if (liquidGlassEnabled) {
            return <LiquidGlassCard w="full" p={4} mb={3}>{dnsContent}</LiquidGlassCard>;
          }
          return (
            <Box w="full" bg={cardBg} borderRadius="xl" border="1px solid" borderColor={cardBorder} p={4} mb={3}>
              {dnsContent}
            </Box>
          );
        })()}

        {/* DNS 预设列表 */}
        <SimpleGrid columns={{ base: 1, md: 2, lg: 3 }} spacing={3} mb={3}>
          {dnsPresets.map((preset) => (
            <DnsCard key={preset.id} preset={preset} />
          ))}
        </SimpleGrid>

        {/* 自定义 DNS 输入 */}
        {(() => {
          const customDnsContent = (
            <>
              <Text fontSize="xs" fontWeight="bold" color={subTextColor} mb={2}>
                {t("networkOptimize.dns.customLabel")}
              </Text>
              <HStack spacing={2} flexWrap="wrap">
                <Input
                  placeholder={t("networkOptimize.dns.primary")}
                  value={customPrimary}
                  onChange={(e) => setCustomPrimary(e.target.value)}
                  size="sm"
                  flex={1}
                  minW="140px"
                />
                <Input
                  placeholder={t("networkOptimize.dns.secondary")}
                  value={customSecondary}
                  onChange={(e) => setCustomSecondary(e.target.value)}
                  size="sm"
                  flex={1}
                  minW="140px"
                />
                <Button
                  size="sm"
                  onClick={handleApplyCustomDns}
                  isLoading={isApplyingCustomDns}
                  bg={activeColor}
                  color={contrastText}
                  _hover={{ opacity: 0.9 }}
                  _active={{ transform: "scale(0.97)" }}
                >
                  {t("networkOptimize.dns.apply")}
                </Button>
              </HStack>
            </>
          );
          if (liquidGlassEnabled) {
            return <LiquidGlassCard w="full" p={4}>{customDnsContent}</LiquidGlassCard>;
          }
          return (
            <Box w="full" bg={cardBg} borderRadius="xl" border="1px solid" borderColor={cardBorder} p={4}>
              {customDnsContent}
            </Box>
          );
        })()}
      </Box>

      {/* Section 2: 网络优化项 */}
      <Box w="full">
        <Heading
          as="h3"
          fontSize="md"
          fontWeight="bold"
          color={headingColor}
          mb={3}
          position="relative"
          pl={3}
          sx={{
            "&::before": {
              content: '""',
              position: "absolute",
              left: 0,
              top: "50%",
              transform: "translateY(-50%)",
              width: "3px",
              height: "16px",
              borderRadius: "full",
              bg: activeColor,
            },
          }}
        >
          {t("networkOptimize.batch.title")}
        </Heading>
        <SimpleGrid columns={{ base: 1, md: 2 }} spacing={3}>
          {networkOptimizerItems.map((item) => (
            <OptimizeCard key={item.id} item={item} />
          ))}
        </SimpleGrid>
      </Box>
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
