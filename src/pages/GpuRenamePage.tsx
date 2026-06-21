import {
  Box,
  Button,
  Heading,
  Text,
  VStack,
  useColorModeValue,
  useToast,
  HStack,
  IconButton,
  Card,
  CardBody,
  Input,
  Tabs,
  TabList,
  Tab,
  TabPanels,
  TabPanel,
} from "@chakra-ui/react";
import { CustomSelect } from "@/components/special/custom-select";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { AnimatedPage } from "@/components/ui/animated-page";
import { useNavigate } from "react-router-dom";
import { ArrowLeft, Monitor } from "lucide-react";
import { useThemeColor } from "@/contexts/theme-color-context";
import { hexToRgba } from "@/lib/color-utils";

interface GpuInfo {
  original_name: string;
  current_name: string;
  is_backed_up: boolean;
}

interface GpuOption {
  id: string;
  name: string;
  category: string;
}

interface GpuRenameResult {
  success: boolean;
  message: string;
}

export default function GpuRenamePage() {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const toast = useToast();
  const navigate = useNavigate();

  const { getActiveColor, getContrastTextColor } = useThemeColor();
  const primaryColor = getActiveColor();
  const contrastText = getContrastTextColor();

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const textColor = useColorModeValue("gray.600", "#a0a0a0");
  const cardBg = useColorModeValue("white", "#111111");
  const cardBorder = useColorModeValue("gray.200", "#333333");

  const [gpuInfo, setGpuInfo] = useState<GpuInfo | null>(null);
  const [gpuOptions, setGpuOptions] = useState<GpuOption[]>([]);
  const [selectedOption, setSelectedOption] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [applying, setApplying] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const [tabIndex, setTabIndex] = useState(0);
  const [customName, setCustomName] = useState("");

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    try {
      setLoading(true);
      const [info, options] = await Promise.all([
        invoke<GpuInfo>("get_gpu_info"),
        invoke<GpuOption[]>("get_gpu_options"),
      ]);
      setGpuInfo(info);
      setGpuOptions(options);
    } catch (error) {
      toast({
        title: t("gpuRename.loadError"),
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    } finally {
      setLoading(false);
    }
  };

  const handleApply = async () => {
    let targetName = "";

    if (tabIndex === 1) {
      if (!customName.trim()) {
        toast({
          title: t("gpuRename.selectGpu"),
          status: "warning",
          duration: 2000,
          isClosable: true,
        });
        return;
      }
      targetName = customName.trim();
    } else {
      if (!selectedOption) {
        toast({
          title: t("gpuRename.selectGpu"),
          status: "warning",
          duration: 2000,
          isClosable: true,
        });
        return;
      }
      const selectedGpu = gpuOptions.find((opt) => opt.id === selectedOption);
      if (!selectedGpu) return;
      targetName = selectedGpu.name;
    }

    try {
      setApplying(true);
      const result = await invoke<GpuRenameResult>("apply_gpu_rename", {
        newName: targetName,
      });

      if (result.success) {
        toast({
          title: t("gpuRename.success"),
          description: result.message,
          status: "success",
          duration: 3000,
          isClosable: true,
        });
        await loadData();
      } else {
        toast({
          title: t("gpuRename.error"),
          description: result.message,
          status: "error",
          duration: 3000,
          isClosable: true,
        });
      }
    } catch (error) {
      toast({
        title: t("gpuRename.error"),
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    } finally {
      setApplying(false);
    }
  };

  const handleRestore = async () => {
    try {
      setRestoring(true);
      const result = await invoke<GpuRenameResult>("restore_gpu_name");

      if (result.success) {
        toast({
          title: t("gpuRename.restored"),
          description: result.message,
          status: "success",
          duration: 3000,
          isClosable: true,
        });
        await loadData();
      } else {
        toast({
          title: t("gpuRename.error"),
          description: result.message,
          status: "error",
          duration: 3000,
          isClosable: true,
        });
      }
    } catch (error) {
      toast({
        title: t("gpuRename.error"),
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    } finally {
      setRestoring(false);
    }
  };

  const content = (
    <VStack align="start" spacing={6}>
      <HStack>
        <IconButton
          aria-label={t("builtinTools.back")}
          icon={<ArrowLeft size={20} />}
          variant="ghost"
          onClick={() => navigate("/builtin-tools")}
          color={headingColor}
        />
        <Monitor size={28} color={headingColor} />
        <Heading size="lg" color={headingColor} fontWeight="700">
          {t("gpuRename.title")}
        </Heading>
      </HStack>

      {loading ? (
        <Text color={textColor}>{t("gpuRename.loading")}</Text>
      ) : (
        <>
          <VStack align="start" spacing={3} w="full">
            <HStack spacing={6} w="full" align="start">
              <Box flex={1}>
                <Text color={textColor} fontSize="sm" mb={1}>
                  {t("gpuRename.currentGpu")}
                </Text>
                <Text color={headingColor} fontWeight="medium" wordBreak="break-all">
                  {gpuInfo?.current_name || "-"}
                </Text>
              </Box>

              <Box flex={1}>
                <Text color={textColor} fontSize="sm" mb={1}>
                  {t("gpuRename.backupStatus")}
                </Text>
                <Text
                  color={gpuInfo?.is_backed_up ? "green.500" : "orange.500"}
                  fontWeight="medium"
                >
                  {gpuInfo?.is_backed_up
                    ? t("gpuRename.backedUp")
                    : t("gpuRename.notBackedUp")}
                </Text>
              </Box>
            </HStack>

            {gpuInfo?.is_backed_up && (
              <Box w="full">
                <Text color={textColor} fontSize="sm" mb={1}>
                  {t("gpuRename.originalGpu")}
                </Text>
                <Text color={headingColor} fontWeight="medium" wordBreak="break-all">
                  {gpuInfo.original_name}
                </Text>
              </Box>
            )}
          </VStack>

          <Tabs index={tabIndex} onChange={setTabIndex} variant="enclosed" w="full" mt={4}>
            <TabList>
              <Tab color={textColor} _selected={{ color: headingColor, fontWeight: "600" }}>{t("gpuRename.presetTab")}</Tab>
              <Tab color={textColor} _selected={{ color: headingColor, fontWeight: "600" }}>{t("gpuRename.customTab")}</Tab>
            </TabList>
            <TabPanels>
              <TabPanel px={0}>
                <Box w="full">
                  <Text color={textColor} fontSize="sm" mb={2} fontWeight="600">
                    {t("gpuRename.lowEnd")}
                  </Text>
                  <CustomSelect
                    value={selectedOption}
                    onChange={setSelectedOption}
                    options={gpuOptions
                      .filter(option => option.category === "low-end")
                      .map(option => ({ value: option.id, label: option.name }))}
                    placeholder={t("gpuRename.selectPlaceholder")}
                    width="100%"
                  />
                </Box>
                <Box w="full" mt={4}>
                  <Text color={textColor} fontSize="sm" mb={2} fontWeight="600">
                    {t("gpuRename.highEnd")}
                  </Text>
                  <CustomSelect
                    value={selectedOption}
                    onChange={setSelectedOption}
                    options={gpuOptions
                      .filter(option => option.category === "high-end")
                      .map(option => ({ value: option.id, label: option.name }))}
                    placeholder={t("gpuRename.selectPlaceholder")}
                    width="100%"
                  />
                </Box>
              </TabPanel>
              <TabPanel px={0}>
                <Box w="full">
                  <Text color={textColor} fontSize="sm" mb={2} fontWeight="600">
                    {t("gpuRename.customTab")}
                  </Text>
                  <Input
                    value={customName}
                    onChange={(e) => setCustomName(e.target.value)}
                    placeholder={t("gpuRename.customPlaceholder")}
                    color={headingColor}
                    borderColor={cardBorder}
                    _focus={{ borderColor: primaryColor }}
                  />
                </Box>
              </TabPanel>
            </TabPanels>
          </Tabs>

          <VStack align="start" spacing={3} w="full" mt={4}>
            <Button
              bg={primaryColor}
              color={contrastText}
              onClick={handleApply}
              isLoading={applying}
              loadingText={t("gpuRename.applying")}
              w="full"
              _hover={{
                bg: hexToRgba(primaryColor, 0.8),
                transform: "translateY(-1px)",
                boxShadow: `0 4px 12px ${hexToRgba(primaryColor, 0.3)}`,
              }}
              _active={{
                bg: hexToRgba(primaryColor, 0.6),
                transform: "translateY(0)",
              }}
              transition="all 0.2s"
            >
              {t("gpuRename.apply")}
            </Button>

            {gpuInfo?.is_backed_up && (
              <Button
                colorScheme="orange"
                onClick={handleRestore}
                isLoading={restoring}
                loadingText={t("gpuRename.restoring")}
                w="full"
              >
                {t("gpuRename.restore")}
              </Button>
            )}
          </VStack>

          <Box w="full" mt={4} p={4} bg="blue.50" borderRadius="md" borderWidth="1px" borderColor="blue.200">
            <Text color="blue.700" fontSize="sm" whiteSpace="pre-line">
              {t("gpuRename.note")}
            </Text>
          </Box>
        </>
      )}
    </VStack>
  );

  return (
    <AnimatedPage>
      <Box pt={8}>
        {liquidGlassEnabled ? (
          <LiquidGlassCard
            w="full"
            boxShadow="2xl"
            overflow="hidden"
            position="relative"
            p={6}
          >
            {content}
          </LiquidGlassCard>
        ) : (
          <Card
            bg={cardBg}
            borderColor={cardBorder}
            borderWidth="1px"
            w="full"
            boxShadow="2xl"
            overflow="hidden"
            position="relative"
          >
            <CardBody p={6}>
              {content}
            </CardBody>
          </Card>
        )}
      </Box>
    </AnimatedPage>
  );
}
