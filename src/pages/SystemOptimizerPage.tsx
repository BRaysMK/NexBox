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
  useColorModeValue,
  useToast,
  Spinner,
  Tooltip,
  Badge,
} from "@chakra-ui/react";
import { ArrowLeft } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { LazyStore } from "@tauri-apps/plugin-store";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import {
  optimizerItems,
  categoryLabels,
  categoryOrder,
  type OptimizerItem,
  type OptimizerCategory,
} from "@/config/system-optimizer";

const STORE_KEY = "system_optimizer_states";
const store = new LazyStore("settings.json");

export default function SystemOptimizerPage() {
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

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const cardBg = useColorModeValue("white", "#111111");
  const cardBorder = useColorModeValue("gray.200", "#333333");

  const activeColor = getActiveColor();
  const contrastText = getContrastTextColor();
  const hoverBg = getHoverColor(false);

  // 加载保存的状态并自动扫描
  useEffect(() => {
    let cancelled = false;
    async function init() {
      const startTime = Date.now();
      const [savedResult, scannedResult] = await Promise.allSettled([
        store.get<Record<string, boolean>>(STORE_KEY),
        invoke<Record<string, boolean>>("check_all_tweak_states"),
      ]);
      // 如果组件已卸载（StrictMode 首次 mount 的清理），放弃本次结果
      if (cancelled) return;
      const saved = savedResult.status === "fulfilled" && savedResult.value ? savedResult.value : {};
      const scanned = scannedResult.status === "fulfilled" ? scannedResult.value : {};
      // 确保 loading 至少显示 600ms
      const remaining = Math.max(0, 600 - (Date.now() - startTime));
      if (remaining > 0) {
        await new Promise((r) => setTimeout(r, remaining));
      }
      if (cancelled) return;
      // 一次性原子设置所有状态
      setSavedStates(saved);
      setScannedStates(scanned);
      setIsInitialScanning(false);
    }
    init();
    return () => {
      cancelled = true;
    };
  }, []);

  // 保存状态到持久化存储
  const persistStates = useCallback(async (states: Record<string, boolean>) => {
    try {
      await store.set(STORE_KEY, states);
      await store.save();
    } catch {}
  }, []);

  // 获取某个优化项的最终显示状态
  const getItemState = useCallback(
    (item: OptimizerItem): boolean => {
      // 已保存状态优先（用户主动操作的结果，覆盖扫描结果）
      if (savedStates[item.id] !== undefined) return savedStates[item.id];
      // 否则使用扫描结果
      const scanned = scannedStates[item.stateKey];
      if (scanned !== undefined) return scanned;
      return false;
    },
    [scannedStates, savedStates],
  );

  // 执行单个优化项（后端完成后前端再更新）
  const toggleItem = useCallback(
    async (item: OptimizerItem, enable: boolean) => {
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
            ? t("systemOptimizer.optimized")
            : t("systemOptimizer.reverted"),
          description: t(item.titleKey),
          status: "success",
          duration: 2000,
          isClosable: true,
        });
      } catch (err) {
        toast({
          title: t("systemOptimizer.operationError"),
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

  // 全部优化（后端完成后前端再更新）
  const handleBatchEnable = useCallback(async () => {
    setIsBatchOptimizing(true);
    try {
      await invoke("batch_enable_tweaks");
      const newSaved: Record<string, boolean> = {};
      const newScanned: Record<string, boolean> = {};
      for (const item of optimizerItems) {
        newSaved[item.id] = true;
        newScanned[item.stateKey] = true;
      }
      setSavedStates(newSaved);
      setScannedStates(newScanned);
      persistStates(newSaved);
      toast({
        title: t("systemOptimizer.batchOptimized"),
        status: "success",
        duration: 3000,
        isClosable: true,
      });
    } catch (err) {
      toast({
        title: t("systemOptimizer.batchError"),
        description: String(err),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    } finally {
      setIsBatchOptimizing(false);
    }
  }, [persistStates, toast, t]);

  // 全部取消优化（后端完成后前端再更新）
  const handleBatchDisable = useCallback(async () => {
    setIsBatchOptimizing(true);
    try {
      await invoke("batch_disable_tweaks");
      const newScanned: Record<string, boolean> = {};
      for (const item of optimizerItems) {
        newScanned[item.stateKey] = false;
      }
      setSavedStates({});
      setScannedStates(newScanned);
      persistStates({});
      toast({
        title: t("systemOptimizer.batchReverted"),
        status: "success",
        duration: 3000,
        isClosable: true,
      });
    } catch (err) {
      toast({
        title: t("systemOptimizer.batchError"),
        description: String(err),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    } finally {
      setIsBatchOptimizing(false);
    }
  }, [persistStates, toast, t]);

  // 优化项卡片
  function OptimizeCard({ item }: { item: OptimizerItem }) {
    const isOptimized = getItemState(item);
    const isToggling = togglingItems.has(item.id);
    const IconComponent = item.icon;

    const content = (
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
            <HStack spacing={2} align="center" flexWrap="wrap">
              <Text
                color={headingColor}
                fontSize="sm"
                fontWeight="bold"
                noOfLines={1}
              >
                {t(item.titleKey)}
              </Text>
              {item.requiresReboot && (
                <Tooltip label={t("systemOptimizer.requiresReboot")}>
                  <Badge
                    fontSize="9px"
                    colorScheme="orange"
                    variant="subtle"
                    borderRadius="full"
                    px={1.5}
                    lineHeight="1.2"
                  >
                    REBOOT
                  </Badge>
                </Tooltip>
              )}
            </HStack>
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
          {content}
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
      >
        {content}
      </Box>
    );
  }

  // 分类分组渲染
  function CategorySection({ category }: { category: OptimizerCategory }) {
    const items = optimizerItems.filter((item) => item.category === category);
    if (items.length === 0) return null;

    return (
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
          {t(categoryLabels[category])}
        </Heading>
        <SimpleGrid columns={{ base: 1, md: 2 }} spacing={3}>
          {items.map((item) => (
            <OptimizeCard key={item.id} item={item} />
          ))}
        </SimpleGrid>
      </Box>
    );
  }

  // 扫描中：只显示 loading（不渲染任何内容）
  if (isInitialScanning) {
    return (
      <Box pt={8}>
        {liquidGlassEnabled ? (
          <LiquidGlassCard w="full" boxShadow="2xl" overflow="hidden" position="relative" p={6}>
            <Flex w="full" minH="360px" align="center" justify="center" direction="column" gap={4}>
              <Spinner size="xl" color={activeColor} thickness="3px" />
              <Text color={subTextColor} fontSize="sm">
                {t("systemOptimizer.scanning")}
              </Text>
            </Flex>
          </LiquidGlassCard>
        ) : (
          <Box bg={cardBg} borderRadius="xl" borderWidth="1px" borderColor={cardBorder} w="full" boxShadow="2xl" overflow="hidden" position="relative" p={6}>
            <Flex w="full" minH="360px" align="center" justify="center" direction="column" gap={4}>
              <Spinner size="xl" color={activeColor} thickness="3px" />
              <Text color={subTextColor} fontSize="sm">
                {t("systemOptimizer.scanning")}
              </Text>
            </Flex>
          </Box>
        )}
      </Box>
    );
  }

  const content = (
    <VStack align="start" spacing={6}>
      {/* 标题和操作按钮 */}
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
            {t("systemOptimizer.pageTitle")}
          </Heading>
        </HStack>
        <HStack spacing={2}>
          <Button
            size="sm"
            onClick={handleBatchEnable}
            isLoading={isBatchOptimizing}
            loadingText={t("systemOptimizer.optimizing")}
            bg={activeColor}
            color={contrastText}
            _hover={{ opacity: 0.9 }}
            _active={{ transform: "scale(0.97)" }}
          >
            {t("systemOptimizer.batchEnable")}
          </Button>
          <Button
            size="sm"
            onClick={handleBatchDisable}
            isLoading={isBatchOptimizing}
            loadingText={t("systemOptimizer.optimizing")}
            variant="outline"
            sx={{
              borderColor: activeColor,
              color: activeColor,
              _hover: { bg: hoverBg },
            }}
          >
            {t("systemOptimizer.batchDisable")}
          </Button>
        </HStack>
      </Flex>

      {/* 优化项分类列表 */}
      {categoryOrder.map((cat) => (
        <CategorySection key={cat} category={cat} />
      ))}
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
