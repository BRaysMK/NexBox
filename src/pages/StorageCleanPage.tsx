import {
  Box,
  Flex,
  Text,
  Heading,
  VStack,
  HStack,
  Badge,
  Checkbox,
  Button,
  useColorModeValue,
  useToast,
  Spinner,
  Divider,
  SimpleGrid,
  Icon,
} from "@chakra-ui/react";
import { AnimatePresence, motion } from "framer-motion";
import { useTransitionMode, getVariants, getTransitionConfig } from "@/components/ui/animated-page";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { LiquidGlassButton } from "@/components/special/liquid-glass-button";
import { useTranslation } from "react-i18next";
import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Trash2,
  RefreshCw,
  ArrowLeft,
  HardDrive,
  FileText,
  Image,
  AlertTriangle,
  Database,
  ShieldAlert,
  Folder,
  File,
} from "lucide-react";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import { useNavigate } from "react-router-dom";

interface CleanItem {
  id: string;
  name: string;
  path: string;
  exists: boolean;
  size_bytes: number;
  requires_admin: boolean;
  description: string;
}

interface ScanResult {
  items: CleanItem[];
  total_size: number;
  total_items: number;
}

interface CleanResult {
  success: boolean;
  message: string;
  freed_bytes: number;
  skipped_files: string[];
}

function formatSize(bytes: number): string {
  if (bytes === 0) return "0 B";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function getItemIcon(id: string): React.ComponentType<{ size?: number; strokeWidth?: number }> {
  switch (id) {
    case "temp_user":
    case "temp_system":
      return FileText;
    case "recycle_bin":
      return Trash2;
    case "thumbnail_cache":
      return Image;
    case "prefetch":
      return Database;
    case "wer_archive":
    case "wer_queue":
    case "crash_dumps":
    case "memory_dmp":
    case "minidump":
      return AlertTriangle;
    case "windows_logs":
      return FileText;
    case "d3dscache":
      return HardDrive;
    case "thumbs_db":
      return Image;
    default:
      return Folder;
  }
}

function CleanItemCard({
  item,
  isSelected,
  onToggleSelect,
  primaryColor,
}: {
  item: CleanItem;
  isSelected: boolean;
  onToggleSelect: () => void;
  primaryColor: string;
}) {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const headingColor = useColorModeValue("gray.800", "#e0e0e0");
  const descColor = useColorModeValue("gray.500", "#888888");
  const pathColor = useColorModeValue("gray.400", "#666666");
  const cardBg = liquidGlassEnabled
    ? "rgba(255,255,255,0.7)"
    : useColorModeValue("#ffffff", "#1a1a1a");
  const borderColor = liquidGlassEnabled
    ? "rgba(255,255,255,0.3)"
    : useColorModeValue("gray.200", "#333333");
  const cardHoverBg = liquidGlassEnabled
    ? "rgba(255,255,255,0.85)"
    : useColorModeValue("gray.50", "#252525");
  const cardHoverBorder = liquidGlassEnabled
    ? "rgba(255,255,255,0.5)"
    : useColorModeValue("gray.300", "#444444");
  const IconComponent = getItemIcon(item.id);

  const hasContent = item.exists && item.size_bytes > 0;

  return (
    <Box
      bg={cardBg}
      borderRadius="lg"
      border="1px solid"
      borderColor={isSelected ? primaryColor : borderColor}
      p={4}
      cursor={hasContent ? "pointer" : "not-allowed"}
      onClick={hasContent ? onToggleSelect : undefined}
      transition="all 0.2s ease"
      _hover={
        hasContent && !isSelected
          ? {
              borderColor: cardHoverBorder,
              bg: cardHoverBg,
            }
          : undefined
      }
      opacity={hasContent ? 1 : 0.45}
    >
      <Flex justify="space-between" align="start">
        <HStack spacing={3}>
          <Checkbox
            isChecked={isSelected}
            onChange={onToggleSelect}
            colorScheme={primaryColor === "#3182CE" ? "blue" : "teal"}
            isDisabled={!hasContent}
          />
          <Box
            w={10}
            h={10}
            borderRadius="lg"
            bg={hasContent ? `${primaryColor}20` : "gray.50"}
            display="flex"
            alignItems="center"
            justifyContent="center"
            color={hasContent ? primaryColor : "gray.400"}
          >
            <IconComponent size={20} />
          </Box>
          <VStack align="start" spacing={0}>
            <Text fontSize="md" fontWeight="semibold" color={headingColor}>
              {t(`storageClean.items.${item.id}.name`)}
            </Text>
            <Text fontSize="xs" color={pathColor}>
              {item.path}
            </Text>
          </VStack>
        </HStack>
        <VStack align="end" spacing={1}>
          {item.requires_admin && (
            <Badge
              size="sm"
              colorScheme="orange"
              variant="subtle"
              fontSize="xs"
            >
              {t("storageClean.adminRequired")}
            </Badge>
          )}
          <Badge
            size="sm"
            colorScheme={hasContent ? "blue" : "gray"}
            variant="subtle"
            fontSize="xs"
          >
            {formatSize(item.size_bytes)}
          </Badge>
        </VStack>
      </Flex>
      <Text fontSize="xs" color={descColor} mt={2}>
        {t(`storageClean.items.${item.id}.description`)}
      </Text>
    </Box>
  );
}

export default function StorageCleanPage() {
  const { t } = useTranslation();
  const toast = useToast();
  const { liquidGlassEnabled } = useBackground();
  const { config: themeConfig, getContrastTextColor } = useThemeColor();
  const navigate = useNavigate();

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const subTextColor = useColorModeValue("gray.500", "#888888");

  const statsBg = liquidGlassEnabled
    ? "rgba(255,255,255,0.6)"
    : useColorModeValue(
        "#f7fafc",
        "#1e2024"
      );
  const statsBorder = liquidGlassEnabled
    ? "rgba(255,255,255,0.3)"
    : useColorModeValue(
        "gray.200",
        "#2d3748"
      );

  const tipBg = liquidGlassEnabled
    ? "rgba(255,255,255,0.5)"
    : useColorModeValue(
        "#ebf8ff",
        "#1a365d"
      );
  const tipBorder = liquidGlassEnabled
    ? "rgba(255,255,255,0.25)"
    : useColorModeValue(
        "blue.200",
        "#2b6cb0"
      );
  const tipTitleColor = useColorModeValue(
    themeConfig.primaryColor,
    themeConfig.primaryColor
  );
  const tipTextColor = useColorModeValue(
    "gray.600",
    "rgba(200,200,200,0.85)"
  );

  const [scanResult, setScanResult] = useState<ScanResult | null>(null);
  const [isScanning, setIsScanning] = useState(false);
  const [isCleaning, setIsCleaning] = useState(false);
  const [selectedItems, setSelectedItems] = useState<Set<string>>(new Set());

  const doScan = useCallback(async () => {
    setIsScanning(true);
    try {
      const result = await invoke<ScanResult>("scan_storage_items");
      setScanResult(result);
      const defaultSelected = new Set(
        result.items
          .filter((item) => item.exists && item.size_bytes > 0 && !item.requires_admin)
          .map((item) => item.id)
      );
      setSelectedItems(defaultSelected);
    } catch (error) {
      console.error("Failed to scan storage items:", error);
      toast({
        title: t("storageClean.scanError") || "扫描失败",
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    }
    setIsScanning(false);
  }, [t, toast]);

  useEffect(() => {
    doScan();
  }, [doScan]);

  const handleToggleItem = (id: string) => {
    setSelectedItems((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const handleSelectAll = () => {
    if (scanResult) {
      const allIds = scanResult.items
        .filter((item) => item.exists && item.size_bytes > 0)
        .map((item) => item.id);
      setSelectedItems(new Set(allIds));
    }
  };

  const handleDeselectAll = () => {
    setSelectedItems(new Set());
  };

  const handleClean = async () => {
    if (selectedItems.size === 0) {
      toast({
        title: t("storageClean.noItemSelected"),
        status: "warning",
        duration: 2000,
        isClosable: true,
      });
      return;
    }

    setIsCleaning(true);
    try {
      const result = await invoke<CleanResult>("clean_storage_items", {
        itemIds: Array.from(selectedItems),
      });

      if (result.success) {
        toast({
          title: t("storageClean.cleanSuccess", { size: formatSize(result.freed_bytes) }),
          description: result.skipped_files.length > 0
            ? t("storageClean.skippedFiles", { count: result.skipped_files.length })
            : undefined,
          status: "success",
          duration: 4000,
          isClosable: true,
        });
      } else {
        toast({
          title: t("storageClean.cleanError"),
          description: result.message,
          status: "error",
          duration: 3000,
          isClosable: true,
        });
      }
    } catch (error) {
      console.error("Failed to clean storage items:", error);
      toast({
        title: t("storageClean.cleanError"),
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    }
    setIsCleaning(false);

    await doScan();
  };

  const selectedSize = scanResult
    ? scanResult.items
        .filter((item) => selectedItems.has(item.id))
        .reduce((sum, item) => sum + item.size_bytes, 0)
    : 0;

  const transitionMode = useTransitionMode();

  const content = (
    <VStack align="stretch" spacing={6} pt={8}>
      <HStack justifyContent="space-between" alignItems="center" w="full">
        <Button
          variant="ghost"
          leftIcon={<ArrowLeft size={18} />}
          onClick={() => navigate("/optimization")}
          color={headingColor}
        >
          {t("tests.back") || "返回"}
        </Button>
        <Heading size="lg" color={headingColor} fontWeight="700">
          {t("storageClean.title")}
        </Heading>
        <Box w="100px" />
      </HStack>

      {scanResult && (
        <Box
          p={4}
          borderRadius="xl"
          border="1px solid"
          borderColor={statsBorder}
          bg={statsBg}
        >
          <SimpleGrid columns={3} spacing={4}>
            <VStack align="center">
              <Icon as={HardDrive} color={themeConfig.primaryColor} boxSize={6} />
              <Text fontSize="sm" color={subTextColor}>
                {t("storageClean.totalScanned")}
              </Text>
              <Text fontSize="lg" fontWeight="bold" color={headingColor}>
                {formatSize(scanResult.total_size)}
              </Text>
            </VStack>
            <VStack align="center">
              <Icon as={Folder} color={themeConfig.primaryColor} boxSize={6} />
              <Text fontSize="sm" color={subTextColor}>
                {t("storageClean.itemsFound")}
              </Text>
              <Text fontSize="lg" fontWeight="bold" color={headingColor}>
                {scanResult.total_items}
              </Text>
            </VStack>
            <VStack align="center">
              <Icon as={Trash2} color={themeConfig.primaryColor} boxSize={6} />
              <Text fontSize="sm" color={subTextColor}>
                {t("storageClean.selectedSize")}
              </Text>
              <Text fontSize="lg" fontWeight="bold" color={headingColor}>
                {formatSize(selectedSize)}
              </Text>
            </VStack>
          </SimpleGrid>
        </Box>
      )}

      <HStack spacing={3} justify="space-between">
        <HStack spacing={2}>
          <Button size="sm" variant="outline" onClick={handleSelectAll}>
            {t("storageClean.selectAll")}
          </Button>
          <Button size="sm" variant="ghost" onClick={handleDeselectAll}>
            {t("storageClean.deselectAll")}
          </Button>
        </HStack>
      </HStack>

      {isScanning ? (
        <VStack py={8}>
          <Spinner size="lg" color="teal.500" />
          <Text color={subTextColor}>{t("storageClean.scanning")}</Text>
        </VStack>
      ) : (
        scanResult && (
          <SimpleGrid columns={{ base: 1, md: 2 }} spacing={3}>
            {scanResult.items.map((item) => (
              <CleanItemCard
                key={item.id}
                item={item}
                isSelected={selectedItems.has(item.id)}
                onToggleSelect={() => handleToggleItem(item.id)}
                primaryColor={themeConfig.primaryColor}
              />
            ))}
          </SimpleGrid>
        )
      )}

      <HStack spacing={3} justify="start">
        <LiquidGlassButton
          leftIcon={isCleaning ? <Spinner size="sm" /> : <Trash2 size={16} />}
          onClick={handleClean}
          isLoading={isCleaning}
          loadingText={t("storageClean.cleaning")}
          disabled={isScanning || selectedItems.size === 0}
          bg={themeConfig.primaryColor}
          color={getContrastTextColor()}
          _hover={{
            bg: themeConfig.primaryColor,
            filter: "brightness(0.9)",
          }}
          _active={{
            bg: themeConfig.primaryColor,
            filter: "brightness(0.8)",
          }}
        >
          {t("storageClean.cleanButton")}
        </LiquidGlassButton>
        <LiquidGlassButton
          leftIcon={<RefreshCw size={16} />}
          onClick={doScan}
          isLoading={isScanning}
          variant="outline"
          colorScheme="gray"
        >
          {t("storageClean.scanButton")}
        </LiquidGlassButton>
      </HStack>

      <Box
        p={5}
        borderRadius="xl"
        border="1px solid"
        borderColor={tipBorder}
        bg={tipBg}
      >
        <HStack mb={3}>
          <Text fontSize="sm" fontWeight="bold" color={tipTitleColor}>
            {t("storageClean.tip.title")}
          </Text>
        </HStack>
        <VStack align="start" spacing={2} pl={1}>
          <Text fontSize="xs" color={tipTextColor} lineHeight="tall">
            {t("storageClean.tip.description")}
          </Text>
          <Text fontSize="xs" color={tipTextColor} lineHeight="tall">
            {t("storageClean.tip.note1")}
          </Text>
          <Text fontSize="xs" color={tipTextColor} lineHeight="tall">
            {t("storageClean.tip.note2")}
          </Text>
        </VStack>
      </Box>
    </VStack>
  );

  return transitionMode !== "off" ? (
    <motion.div
      initial="initial"
      animate="enter"
      exit="exit"
      variants={getVariants(transitionMode)}
      transition={getTransitionConfig(transitionMode)}
    >
      {content}
    </motion.div>
  ) : (
    <div>
      {content}
    </div>
  );
}