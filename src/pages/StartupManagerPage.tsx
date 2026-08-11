import {
  Box,
  Flex,
  Text,
  Heading,
  VStack,
  HStack,
  Button,
  useColorModeValue,
  useToast,
  Spinner,
  Table,
  Thead,
  Tbody,
  Tr,
  Th,
  Td,
  IconButton,
  Tooltip,
  Badge,
} from "@chakra-ui/react";
import { AnimatePresence, motion } from "framer-motion";
import { useTransitionMode, getVariants, getTransitionConfig } from "@/components/ui/animated-page";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { LiquidGlassButton } from "@/components/special/liquid-glass-button";
import { useTranslation } from "react-i18next";
import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Ban,
  RotateCcw,
  RefreshCw,
  ArrowLeft,
  FolderOpen,
  Search,
  FileCode,
} from "lucide-react";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import { useNavigate } from "react-router-dom";

interface StartupItem {
  name: string;
  file_location: string;
  location_type: string;
  item_type: string;
  reg_key_path: string | null;
  reg_value_name: string | null;
  folder_path: string | null;
  raw_registry_value: string | null;
  is_disabled: boolean;
}

export default function StartupManagerPage() {
  const { t } = useTranslation();
  const toast = useToast();
  const { liquidGlassEnabled } = useBackground();
  const { config: themeConfig, getContrastTextColor } = useThemeColor();
  const navigate = useNavigate();

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const subTextColor = useColorModeValue("gray.500", "#ffffff");
  const tableBg = liquidGlassEnabled
    ? "rgba(255,255,255,0.7)"
    : useColorModeValue("#ffffff", "#1a1a1a");
  const tableBorder = liquidGlassEnabled
    ? "rgba(255,255,255,0.3)"
    : useColorModeValue("gray.200", "#333333");
  const headerBg = useColorModeValue("gray.50", "#1e2024");
  const pathColor = useColorModeValue("gray.500", "#888888");
  const hoverBg = useColorModeValue("gray.50", "#252525");
  const deleteColor = useColorModeValue("red.500", "red.400");
  const enableColor = useColorModeValue("teal.600", "teal.300");

  const [items, setItems] = useState<StartupItem[]>([]);
  const [isScanning, setIsScanning] = useState(false);
  const [disablingItems, setDisablingItems] = useState<Set<string>>(new Set());

  const doScan = useCallback(
    async (showSpinner = true) => {
      if (showSpinner) setIsScanning(true);
      try {
        const result = await invoke<StartupItem[]>("scan_startup_items");
        setItems(result);
      } catch (error) {
        console.error("Failed to scan startup items:", error);
        toast({
          title: t("optimization.startupManager.scanError"),
          description: String(error),
          status: "error",
          duration: 3000,
          isClosable: true,
        });
      }
      if (showSpinner) setIsScanning(false);
    },
    [t, toast]
  );

  useEffect(() => {
    doScan();
  }, [doScan]);

  const handleDisable = async (item: StartupItem, index: number) => {
    const key = `${item.name}-${index}`;
    setDisablingItems((prev) => new Set(prev).add(key));
    try {
      await invoke("disable_startup_item", { item });
      toast({
        title: t("optimization.startupManager.disableSuccess", { name: item.name }),
        status: "success",
        duration: 2000,
        isClosable: true,
      });
      await doScan(false);
    } catch (error) {
      console.error("Failed to disable startup item:", error);
      toast({
        title: t("optimization.startupManager.disableError"),
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    }
    setDisablingItems((prev) => {
      const next = new Set(prev);
      next.delete(key);
      return next;
    });
  };

  const handleEnable = async (item: StartupItem, index: number) => {
    const key = `${item.name}-${index}`;
    setDisablingItems((prev) => new Set(prev).add(key));
    try {
      await invoke("enable_startup_item", { item });
      toast({
        title: t("optimization.startupManager.enableSuccess", { name: item.name }),
        status: "success",
        duration: 2000,
        isClosable: true,
      });
      await doScan(false);
    } catch (error) {
      console.error("Failed to enable startup item:", error);
      toast({
        title: t("optimization.startupManager.enableError"),
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    }
    setDisablingItems((prev) => {
      const next = new Set(prev);
      next.delete(key);
      return next;
    });
  };

  const handleLocateFile = async (fileLocation: string, itemType: string, rawRegistryValue: string | null) => {
    try {
      await invoke("locate_startup_file", { fileLocation, itemType, rawRegistryValue });
    } catch (error) {
      console.error("Failed to locate file:", error);
      toast({
        title: t("optimization.startupManager.locateError"),
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    }
  };

  const handleFindInRegistry = async (regKeyPath: string, locationType: string) => {
    try {
      await invoke("find_startup_key_in_registry", { regKeyPath, locationType });
    } catch (error) {
      console.error("Failed to open registry:", error);
      toast({
        title: t("optimization.startupManager.registryError"),
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    }
  };

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
                        返回
        </Button>
        <Heading size="lg" color={headingColor} fontWeight="700">
          {t("optimization.startupManager.title")}
        </Heading>
        <Box w="100px" />
      </HStack>

      <HStack spacing={3} justify="space-between">
        <HStack spacing={2}>
          <Text color={subTextColor} fontSize="sm">
            {t("optimization.startupManager.totalItems", { count: items.length })}
          </Text>
        </HStack>
        <LiquidGlassButton
          leftIcon={<RefreshCw size={16} />}
          onClick={doScan}
          isLoading={isScanning}
          size="sm"
          variant="outline"
          colorScheme="gray"
        >
          {t("optimization.startupManager.refresh")}
        </LiquidGlassButton>
      </HStack>

      {isScanning ? (
        <VStack py={12}>
          <Spinner size="lg" color="teal.500" />
          <Text color={subTextColor}>{t("optimization.startupManager.scanning")}</Text>
        </VStack>
      ) : items.length === 0 ? (
        <VStack py={12}>
          <Text color={subTextColor}>{t("optimization.startupManager.noItems")}</Text>
        </VStack>
      ) : (
        <Box
          bg={tableBg}
          borderRadius="xl"
          border="1px solid"
          borderColor={tableBorder}
          overflow="hidden"
        >
          <Table variant="unstyled" size="sm" layout="fixed">
            <Thead bg={headerBg}>
              <Tr>
                <Th px={4} py={3} color={subTextColor} fontSize="xs" textTransform="uppercase" letterSpacing="wider" w="30%">
                  {t("optimization.startupManager.columnName")}
                </Th>
                <Th px={4} py={3} color={subTextColor} fontSize="xs" textTransform="uppercase" letterSpacing="wider" w="45%">
                  {t("optimization.startupManager.columnPath")}
                </Th>
                <Th px={4} py={3} color={subTextColor} fontSize="xs" textTransform="uppercase" letterSpacing="wider" w="140px">
                  {t("optimization.startupManager.columnActions")}
                </Th>
              </Tr>
            </Thead>
            <Tbody>
              {items.map((item, index) => {
                const key = `${item.name}-${index}`;
                const isDisabling = disablingItems.has(key);
                return (
                  <Tr
                    key={key}
                    _hover={{ bg: hoverBg }}
                    transition="background 0.15s"
                    opacity={isDisabling ? 0.5 : item.is_disabled ? 0.6 : 1}
                  >
                    <Td px={4} py={3}>
                      <Flex align="center" gap={2}>
                        <Box
                          w={8}
                          h={8}
                          borderRadius="md"
                          bg={`${themeConfig.primaryColor}15`}
                          display="flex"
                          alignItems="center"
                          justifyContent="center"
                          color={themeConfig.primaryColor}
                          flexShrink={0}
                        >
                          {item.item_type === "Registry" ? (
                            <Search size={14} />
                          ) : (
                            <FileCode size={14} />
                          )}
                        </Box>
                        <Text
                          color={item.is_disabled ? pathColor : headingColor}
                          fontWeight="medium"
                          fontSize="sm"
                          noOfLines={1}
                          textDecoration={item.is_disabled ? "line-through" : "none"}
                        >
                          {item.name}
                        </Text>
                        {item.is_disabled && (
                          <Badge
                            colorScheme="gray"
                            variant="subtle"
                            borderRadius="full"
                            px={2}
                            fontSize="xs"
                            flexShrink={0}
                          >
                            {t("optimization.startupManager.disabled")}
                          </Badge>
                        )}
                      </Flex>
                    </Td>
                    <Td px={4} py={3}>
                      <Tooltip label={item.file_location} placement="top">
                        <Text
                          color={pathColor}
                          fontSize="xs"
                          noOfLines={1}
                          fontFamily="mono"
                        >
                          {item.file_location || "-"}
                        </Text>
                      </Tooltip>
                    </Td>
                    <Td px={4} py={3}>
                      <HStack spacing={1}>
                        <Tooltip label={t("optimization.startupManager.locateFile")} placement="top">
                          <IconButton
                            aria-label={t("optimization.startupManager.locateFile")}
                            icon={<FolderOpen size={14} />}
                            size="sm"
                            variant="ghost"
                            onClick={() => handleLocateFile(item.file_location, item.item_type, item.raw_registry_value)}
                            isDisabled={!item.file_location}
                          />
                        </Tooltip>
                        {item.item_type === "Registry" && item.reg_key_path && (
                          <Tooltip label={t("optimization.startupManager.findInRegistry")} placement="top">
                            <IconButton
                              aria-label={t("optimization.startupManager.findInRegistry")}
                              icon={<Search size={14} />}
                              size="sm"
                              variant="ghost"
                              onClick={() =>
                                handleFindInRegistry(item.reg_key_path!, item.location_type)
                              }
                            />
                          </Tooltip>
                        )}
                        {item.is_disabled ? (
                          <Tooltip label={t("optimization.startupManager.enable")} placement="top">
                            <IconButton
                              aria-label={t("optimization.startupManager.enable")}
                              icon={<RotateCcw size={14} />}
                              size="sm"
                              variant="ghost"
                              colorScheme="teal"
                              color={enableColor}
                              onClick={() => handleEnable(item, index)}
                              isLoading={isDisabling}
                            />
                          </Tooltip>
                        ) : (
                          <Tooltip label={t("optimization.startupManager.disable")} placement="top">
                            <IconButton
                              aria-label={t("optimization.startupManager.disable")}
                              icon={<Ban size={14} />}
                              size="sm"
                              variant="ghost"
                              colorScheme="red"
                              color={deleteColor}
                              onClick={() => handleDisable(item, index)}
                              isLoading={isDisabling}
                            />
                          </Tooltip>
                        )}
                      </HStack>
                    </Td>
                  </Tr>
                );
              })}
            </Tbody>
          </Table>
        </Box>
      )}

      <Box
        p={5}
        borderRadius="xl"
        border="1px solid"
        borderColor={useColorModeValue("blue.200", "rgba(66,153,225,0.2)")}
        bg={useColorModeValue("#ebf8ff", "rgba(26,54,93,0.5)")}
      >
        <VStack align="start" spacing={2}>
          <Text
            fontSize="sm"
            fontWeight="bold"
            color={themeConfig.primaryColor}
          >
            {t("optimization.startupManager.tipTitle")}
          </Text>
          <Text
            fontSize="xs"
            color={useColorModeValue("gray.600", "rgba(200,200,200,0.85)")}
            lineHeight="tall"
          >
            {t("optimization.startupManager.tipDescription")}
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
