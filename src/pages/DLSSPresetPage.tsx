import {
  Box,
  Text,
  Heading,
  VStack,
  HStack,
  useColorModeValue,
  Button,
  Badge,
  useToast,
  Grid,
  IconButton,
  Switch,
  Table,
  Thead,
  Tbody,
  Tr,
  Th,
  Td,
  Menu,
  MenuButton,
  MenuList,
  MenuItem,
  MenuOptionGroup,
  MenuItemOption,
  Spinner,
} from "@chakra-ui/react";
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { Cpu, ArrowLeft, Zap, Eye, FileText, Settings, ChevronDown, Check, Monitor } from "lucide-react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import { hexToRgba } from "@/lib/color-utils";
import { useNavigate } from "react-router-dom";
import { getHardwareInfo, GpuInfo, GpuVendor } from "@/lib/hardware";

interface DLSSModelPreset {
  id: string;
  name: string;
  description: string;
  recommended: boolean;
}

interface DLSSApplyResult {
  success: boolean;
  message: string;
  preset: string;
}

interface DLSSPresetStatus {
  preset: string;
  quality: number;
}

interface DLSSSettingsStatus {
  dlss_indicator_enabled: boolean;
}

interface PresetReference {
  id: string;
  gpuSeries: string;
  preset: string;
  driverVersion: string;
  typeLabel: string;
  presetColor: "purple" | "green";
}

const getDLSSPresets = (t: (key: string) => string): DLSSModelPreset[] => [
  { id: "A", name: t("deltaForce.dlssModels.A.name"), description: t("deltaForce.dlssModels.A.description"), recommended: false },
  { id: "B", name: t("deltaForce.dlssModels.B.name"), description: t("deltaForce.dlssModels.B.description"), recommended: false },
  { id: "C", name: t("deltaForce.dlssModels.C.name"), description: t("deltaForce.dlssModels.C.description"), recommended: false },
  { id: "D", name: t("deltaForce.dlssModels.D.name"), description: t("deltaForce.dlssModels.D.description"), recommended: false },
  { id: "E", name: t("deltaForce.dlssModels.E.name"), description: t("deltaForce.dlssModels.E.description"), recommended: false },
  { id: "F", name: t("deltaForce.dlssModels.F.name"), description: t("deltaForce.dlssModels.F.description"), recommended: false },
  { id: "G", name: t("deltaForce.dlssModels.G.name"), description: t("deltaForce.dlssModels.G.description"), recommended: false },
  { id: "J", name: t("deltaForce.dlssModels.J.name"), description: t("deltaForce.dlssModels.J.description"), recommended: false },
  { id: "K", name: t("deltaForce.dlssModels.K.name"), description: t("deltaForce.dlssModels.K.description"), recommended: true },
  { id: "L", name: t("deltaForce.dlssModels.L.name"), description: t("deltaForce.dlssModels.L.description"), recommended: true },
  { id: "M", name: t("deltaForce.dlssModels.M.name"), description: t("deltaForce.dlssModels.M.description"), recommended: true },
];

const getPresetReferences = (t: (key: string) => string): PresetReference[] => [
  { id: "1", gpuSeries: t("dlssPresetTable.rtx20Series"), preset: t("dlssPresetTable.presetK"), driverVersion: "581.08", typeLabel: t("dlssPresetTable.typeGeneralEnhance"), presetColor: "purple" },
  { id: "2", gpuSeries: t("dlssPresetTable.rtx30Series"), preset: t("dlssPresetTable.presetK"), driverVersion: "581.08", typeLabel: t("dlssPresetTable.typeGeneralEnhance"), presetColor: "purple" },
  { id: "3", gpuSeries: t("dlssPresetTable.rtx4060Ti"), preset: t("dlssPresetTable.presetK"), driverVersion: "581/595", typeLabel: t("dlssPresetTable.typePerformanceFirst"), presetColor: "purple" },
  { id: "4", gpuSeries: t("dlssPresetTable.rtx4060Ti"), preset: t("dlssPresetTable.presetM"), driverVersion: "581/595", typeLabel: t("dlssPresetTable.typeQualityEnhance"), presetColor: "green" },
  { id: "5", gpuSeries: t("dlssPresetTable.rtx50HighFps"), preset: t("dlssPresetTable.presetK"), driverVersion: "581/595", typeLabel: t("dlssPresetTable.typeFpsPriority"), presetColor: "purple" },
  { id: "6", gpuSeries: t("dlssPresetTable.rtx50HighQuality"), preset: t("dlssPresetTable.presetM"), driverVersion: "581/595", typeLabel: t("dlssPresetTable.typeQualityEnhance"), presetColor: "green" },
];

function SectionCard({
  title,
  children,
  icon,
}: {
  title: string;
  children: React.ReactNode;
  icon?: React.ReactNode;
}) {
  const { liquidGlassEnabled } = useBackground();
  const { config: themeConfig } = useThemeColor();
  const cardBg = useColorModeValue("white", "#111111");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const headerColor = useColorModeValue("gray.900", "#ffffff");

  const headerContent = (
    <HStack spacing={2}>
      <Box color={themeConfig.primaryColor}>{icon}</Box>
      <Text fontWeight="semibold" fontSize="md" color={headerColor}>{title}</Text>
    </HStack>
  );

  if (liquidGlassEnabled) {
    return (
      <LiquidGlassCard p={5}>
        <VStack align="stretch" spacing={4}>
          {headerContent}
          {children}
        </VStack>
      </LiquidGlassCard>
    );
  }

  return (
    <Box bg={cardBg} borderRadius="xl" p={5} border="1px solid" borderColor={borderColor}>
      <VStack align="stretch" spacing={4}>
        {headerContent}
        {children}
      </VStack>
    </Box>
  );
}

function DLSSCard() {
  const { t } = useTranslation();
  const { config: themeConfig, getContrastTextColor } = useThemeColor();
  const toast = useToast();
  const [selectedPreset, setSelectedPreset] = useState("K");
  const [isApplying, setIsApplying] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const dlssPresets = getDLSSPresets(t);

  useEffect(() => {
    const style = document.createElement("style");
    style.id = "chakra-menu-z-fix";
    style.textContent = `[data-popper-placement] { z-index: 99999 !important; }`;
    document.head.appendChild(style);
    return () => document.getElementById("chakra-menu-z-fix")?.remove();
  }, []);

  useEffect(() => {
    invoke<DLSSPresetStatus>("get_dlss_preset_status")
      .then((status) => {
        setSelectedPreset(status.preset);
      })
      .catch(() => {})
      .finally(() => setIsLoading(false));
  }, []);

  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const selectBg = useColorModeValue("white", "#1a1a1a");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const hoverBg = useColorModeValue("gray.100", "#252525");
  const menuListBg = useColorModeValue("white", "#1a1a1a");

  const handleApply = async () => {
    setIsApplying(true);
    try {
      const result = await invoke<DLSSApplyResult>("apply_dlss_model_preset", {
        preset: selectedPreset,
      });
      toast({
        title: result.message,
        status: "success",
        duration: 3000,
        isClosable: true,
      });
    } catch (error) {
      toast({
        title: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    }
    setIsApplying(false);
  };

  const currentPreset = dlssPresets.find(p => p.id === selectedPreset);

  return (
    <SectionCard title={t("deltaForce.dlssPreset")} icon={<Settings size={18} />}>
      <VStack align="stretch" spacing={4}>
        <Box>
          <Text fontSize="sm" fontWeight="medium" color={textColor} mb={2}>
            {t("dlssPreset.presetLabel")}
          </Text>
          <Menu matchWidth>
            <MenuButton
              as={Box}
              bg="transparent"
              p={0}
              border="none"
              w="full"
              cursor="pointer"
            >
              <LiquidGlassCard px={3} py={1.5}>
                <HStack justify="space-between">
                  <HStack spacing={2}>
                    <Badge bg={hexToRgba(themeConfig.primaryColor, 0.15)} color={themeConfig.primaryColor} borderRadius="full" px={2}>{currentPreset?.name}</Badge>
                    <Text fontSize="xs" color={subTextColor} noOfLines={1}>
                      {currentPreset?.description}
                    </Text>
                    {currentPreset?.recommended && (
                      <Badge colorScheme="green" fontSize="8px">推荐</Badge>
                    )}
                  </HStack>
                  <ChevronDown size={16} />
                </HStack>
              </LiquidGlassCard>
            </MenuButton>
            <MenuList bg={menuListBg} borderColor={borderColor} maxH="300px" overflowY="auto">
              {dlssPresets.map(preset => (
                <MenuItem
                  key={preset.id}
                  onClick={() => setSelectedPreset(preset.id)}
                  bg={selectedPreset === preset.id ? hoverBg : "transparent"}
                  _hover={{ bg: hoverBg }}
                >
                  <HStack spacing={3} w="full" justify="space-between">
                    <HStack spacing={2}>
                      <Badge bg={hexToRgba(themeConfig.primaryColor, 0.15)} color={themeConfig.primaryColor} borderRadius="full" px={2}>{preset.name}</Badge>
                      <Text fontSize="sm" color={textColor}>{preset.description}</Text>
                    </HStack>
                    <HStack spacing={2}>
                      {preset.recommended && (
                        <Badge colorScheme="green" fontSize="8px">{t("deltaForce.recommended")}</Badge>
                      )}
                      {selectedPreset === preset.id && <Check size={14} color={themeConfig.primaryColor} />}
                    </HStack>
                  </HStack>
                </MenuItem>
              ))}
            </MenuList>
          </Menu>
        </Box>

        <Button
          onClick={handleApply}
          isLoading={isApplying}
          bg={themeConfig.primaryColor}
          color={getContrastTextColor()}
          _hover={{ bg: themeConfig.primaryColor, filter: "brightness(0.9)" }}
          _active={{ bg: themeConfig.primaryColor, filter: "brightness(0.8)" }}
          w="full"
          leftIcon={<Cpu size={16} />}
          size="sm"
        >
          {t("deltaForce.applyPreset")}
        </Button>

        <Text fontSize="xs" color={subTextColor} textAlign="center">
          {t("deltaForce.dlssNote")}
        </Text>
      </VStack>
    </SectionCard>
  );
}

function GpuInfoCard() {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const { config: themeConfig } = useThemeColor();
  const [gpuInfo, setGpuInfo] = useState<GpuInfo | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");

  useEffect(() => {
    getHardwareInfo()
      .then((info) => {
        const nvidiaGpu = info.gpu.find((g) => g.vendor === GpuVendor.NVIDIA);
        setGpuInfo(nvidiaGpu || info.gpu[0] || null);
      })
      .catch(() => {})
      .finally(() => setIsLoading(false));
  }, []);

  const getVendorColor = (vendor: GpuVendor) => {
    switch (vendor) {
      case GpuVendor.NVIDIA:
        return "green";
      case GpuVendor.AMD:
        return "red";
      case GpuVendor.Intel:
        return "blue";
      default:
        return "gray";
    }
  };

  const cardContent = (
    <VStack align="stretch" spacing={3}>
      {isLoading ? (
        <HStack justify="center" py={2}>
          <Spinner size="sm" color={themeConfig.primaryColor} />
          <Text color={subTextColor} fontSize="sm">{t("deltaForce.loading")}</Text>
        </HStack>
      ) : gpuInfo ? (
        <>
          <HStack spacing={3}>
            <Box p={2} borderRadius="lg" bg={hexToRgba(themeConfig.primaryColor, 0.15)}>
              <Monitor size={20} color={themeConfig.primaryColor} />
            </Box>
            <VStack align="start" spacing={0} flex={1}>
              <Text fontSize="sm" fontWeight="bold" color={textColor} noOfLines={1}>
                {gpuInfo.name}
              </Text>
              <HStack spacing={2}>
                <Badge colorScheme={getVendorColor(gpuInfo.vendor)} fontSize="xs" borderRadius="full">
                  {gpuInfo.vendor}
                </Badge>
                {gpuInfo.memory_gb > 0 && (
                  <Text fontSize="xs" color={subTextColor}>
                    {gpuInfo.memory_gb.toFixed(1)}GB
                  </Text>
                )}
              </HStack>
            </VStack>
          </HStack>
          {gpuInfo.driver_version && gpuInfo.driver_version !== "未知" && (
            <HStack spacing={2} fontSize="xs" color={subTextColor}>
              <Text>{t("dlssPreset.driverVersion")}:</Text>
              <Text fontWeight="medium" color={textColor}>
                {gpuInfo.driver_version}
              </Text>
            </HStack>
          )}
        </>
      ) : (
        <Text color={subTextColor} fontSize="sm">{t("dlssPreset.noGpuFound")}</Text>
      )}
    </VStack>
  );

  if (liquidGlassEnabled) {
    return (
      <LiquidGlassCard p={4}>
        {cardContent}
      </LiquidGlassCard>
    );
  }

  return (
    <Box
      bg={useColorModeValue("white", "#111111")}
      borderRadius="xl"
      p={4}
      border="1px solid"
      borderColor={useColorModeValue("gray.200", "#333333")}
    >
      {cardContent}
    </Box>
  );
}

function DLSSIndicatorCard() {
  const { t } = useTranslation();
  const { config: themeConfig } = useThemeColor();
  const toast = useToast();
  const [isEnabled, setIsEnabled] = useState(false);
  const [isLoading, setIsLoading] = useState(false);

  const subTextColor = useColorModeValue("gray.500", "#888888");

  useEffect(() => {
    invoke<DLSSSettingsStatus>("get_dlss_settings_status")
      .then((status) => setIsEnabled(status.dlss_indicator_enabled))
      .catch(() => {});
  }, []);

  const handleToggle = async () => {
    setIsLoading(true);
    try {
      const result = await invoke<boolean>("toggle_dlss_indicator", {
        enable: !isEnabled,
      });
      setIsEnabled(result);
      toast({
        title: isEnabled ? t("dlssIndicator.disabled") : t("dlssIndicator.enabled"),
        status: "success",
        duration: 3000,
        isClosable: true,
      });
    } catch (error) {
      toast({
        title: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    }
    setIsLoading(false);
  };

  return (
    <SectionCard title={t("dlssIndicator.title")} icon={<Eye size={18} />}>
      <HStack justify="space-between">
        <VStack align="start" spacing={0} flex={1}>
          {isLoading ? (
            <HStack spacing={2}>
              <Spinner size="sm" color={themeConfig.primaryColor} />
              <Text fontSize="sm" color={themeConfig.primaryColor} fontWeight="medium">
                {isEnabled ? t("dlssIndicator.disabling") : t("dlssIndicator.enabling")}
              </Text>
            </HStack>
          ) : (
            <>
              <Text fontSize="sm" color={subTextColor}>
                {t("dlssIndicator.description")}
              </Text>
              <Text fontSize="xs" color={subTextColor}>
                {t("dlssIndicator.note")}
              </Text>
            </>
          )}
        </VStack>
        <Switch
          isChecked={isEnabled}
          onChange={handleToggle}
          isDisabled={isLoading}
          sx={{
            "& .chakra-switch__track[data-checked]": {
              bg: themeConfig.primaryColor,
            },
          }}
          size="md"
        />
      </HStack>
    </SectionCard>
  );
}

function PresetReferenceTable() {
  const { t } = useTranslation();
  const { config: themeConfig } = useThemeColor();
  const presetReferences = getPresetReferences(t);

  const tableBg = useColorModeValue("gray.50", "#1a1a1a");
  const headerBg = useColorModeValue("gray.100", "#252525");
  const textColor = useColorModeValue("gray.800", "#e0e0e0");

  return (
    <SectionCard title={t("dlssPresetTable.title")} icon={<FileText size={18} />}>
      <Text fontSize="sm" color={useColorModeValue("gray.500", "#888888")} mb={3}>
        {t("dlssPresetTable.description")}
      </Text>

      <Box overflowX="auto" borderRadius="lg" border="1px solid" borderColor={useColorModeValue("gray.200", "#333333")}>
        <Table variant="simple" size="sm">
          <Thead bg={headerBg}>
            <Tr>
              <Th color={textColor} textTransform="none">{t("dlssPresetTable.gpuSeries")}</Th>
              <Th color={textColor} textTransform="none">{t("dlssPresetTable.recommendedPreset")}</Th>
              <Th color={textColor} textTransform="none">{t("dlssPresetTable.driverVersion")}</Th>
              <Th color={textColor} textTransform="none">{t("dlssPresetTable.scenario")}</Th>
            </Tr>
          </Thead>
          <Tbody>
            {presetReferences.map((ref) => (
              <Tr key={ref.id} bg={tableBg}>
                <Td color={textColor}>{ref.gpuSeries}</Td>
                <Td>
                  <Badge
                    bg={hexToRgba(themeConfig.primaryColor, 0.15)}
                    color={themeConfig.primaryColor}
                    borderRadius="full"
                    px={3}
                    py={1}
                  >
                    {ref.preset}
                  </Badge>
                </Td>
                <Td color={textColor}>{ref.driverVersion}</Td>
                <Td color={textColor}>{ref.typeLabel}</Td>
              </Tr>
            ))}
          </Tbody>
        </Table>
      </Box>
    </SectionCard>
  );
}

export default function DLSSPresetPage() {
  const { t } = useTranslation();
  const { config: themeConfig } = useThemeColor();
  const navigate = useNavigate();
  const headingColor = useColorModeValue("gray.900", "#ffffff");

  return (
    <Box pt={8} pb={8}>
      <HStack spacing={3} mb={6}>
        <IconButton
          aria-label="返回"
          icon={<ArrowLeft size={20} />}
          variant="ghost"
          onClick={() => navigate("/builtin-tools")}
          color={headingColor}
        />
        <Zap size={28} color={themeConfig.primaryColor} />
        <Heading size="lg" color={headingColor} fontWeight="700">
          {t("dlssPreset.title")}
        </Heading>
      </HStack>

      <Grid templateColumns={{ base: "1fr", lg: "1fr 1fr" }} gap={5} mb={5}>
        <Box position="relative" zIndex={10}>
          <DLSSCard />
        </Box>
        <VStack align="stretch" spacing={4} justify="space-between">
          <GpuInfoCard />
          <DLSSIndicatorCard />
        </VStack>
      </Grid>

      <PresetReferenceTable />
    </Box>
  );
}
