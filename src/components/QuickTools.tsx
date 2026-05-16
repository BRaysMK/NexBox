import {
  Box,
  Text,
  Flex,
  Icon,
  useColorModeValue,
  IconButton,
  useDisclosure,
  Modal,
  ModalOverlay,
  ModalContent,
  ModalHeader,
  ModalBody,
  ModalCloseButton,
  Grid,
  Checkbox,
  VStack,
  HStack,
  Badge,
  Image,
} from "@chakra-ui/react";
import { useThemeColor } from "@/contexts/theme-color-context";
import {
  Cpu,
  Zap,
  Wrench,
  TrendingUp,
  Monitor,
  Plus,
  X,
  Network,
  Bot,
  Volume2,
} from "lucide-react";
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useAppStartup } from "@/contexts/app-startup-context";
import { LiquidGlassToolCard } from "@/components/special/liquid-glass-tool-card";
import { useTranslation } from "react-i18next";

const toolIcons = import.meta.glob<{ default: string }>(
  "@/assets/tools/*.{png,jpg,jpeg,svg,webp}",
  { eager: true }
);

function getToolIconImage(toolId: string): string | null {
  const normalizedId = toolId.toLowerCase();
  for (const [path, module] of Object.entries(toolIcons)) {
    const fileName = path.split("/").pop()?.split(".")[0]?.toLowerCase();
    if (fileName === normalizedId) {
      return module.default;
    }
  }
  return null;
}

interface ThirdPartyTool {
  id: string;
  name: string;
  description: string;
  category: string;
  tool_type: string;
  download_url: string;
  file_name: string;
  check_executable: string | null;
}

interface ToolWithStatus {
  tool: ThirdPartyTool;
  installed: boolean;
}

interface ToolStatus {
  installed: boolean;
  checking: boolean;
  downloading: boolean;
  downloadProgress: number;
}

const STORAGE_KEY = "nexbox_quick_tools";

const getToolIcon = (toolId: string) => {
  switch (toolId) {
    case "memreduct":
      return Zap;
    case "windows-core-optimizer":
      return Cpu;
    case "optimizer":
      return TrendingUp;
    case "cpu-z":
      return Cpu;
    case "gpu-z":
      return Monitor;
    case "clash-verge":
      return Network;
    case "gamepp":
      return Bot;
    case "fxsound":
      return Volume2;
    default:
      return Wrench;
  }
};

const categoryColors: Record<string, string> = {
  hardware: "blue",
  assistant: "purple",
  network: "green",
  optimization: "orange",
};

export default function QuickTools() {
  const [selectedToolIds, setSelectedToolIds] = useState<string[]>([]);
  const [toolStatuses, setToolStatuses] = useState<Record<string, ToolStatus>>({});
  const { tools, refreshTools } = useAppStartup();
  const { isOpen, onOpen, onClose } = useDisclosure();
  const { t } = useTranslation();
  const { config } = useThemeColor();

  const iconColor = useColorModeValue("gray.700", "#cccccc");
  const titleColor = useColorModeValue("gray.800", "#e0e0e0");
  const descColor = useColorModeValue("gray.500", "#888888");
  const cardBg = useColorModeValue("gray.100", "#222222");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const headerColor = useColorModeValue("gray.800", "#ffffff");
  const dialogBg = useColorModeValue("gray.50", "#111111");
  const dialogHoverBg = useColorModeValue("gray.100", "#222222");
  const dialogIconBg = useColorModeValue("gray.100", "#222222");

  useEffect(() => {
    loadSelectedTools();
  }, []);

  useEffect(() => {
    if (tools.length > 0) {
      const statuses: Record<string, ToolStatus> = {};
      for (const { tool, installed } of tools) {
        statuses[tool.id] = {
          installed,
          checking: false,
          downloading: false,
          downloadProgress: 0,
        };
      }
      setToolStatuses(statuses);
    }
  }, [tools]);

  useEffect(() => {
    const unlisten = listen<{ tool_id: string; progress: number }>(
      "tool-download-progress",
      (event) => {
        const { tool_id, progress } = event.payload;
        setToolStatuses((prev) => ({
          ...prev,
          [tool_id]: {
            ...prev[tool_id],
            downloadProgress: progress,
          },
        }));
      }
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const loadSelectedTools = () => {
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved) {
        setSelectedToolIds(JSON.parse(saved));
      }
    } catch (error) {
      console.error("Failed to load selected tools:", error);
    }
  };

  const saveSelectedTools = (ids: string[]) => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(ids));
    } catch (error) {
      console.error("Failed to save selected tools:", error);
    }
  };

  const handleToggleTool = (toolId: string) => {
    const newIds = selectedToolIds.includes(toolId)
      ? selectedToolIds.filter((id) => id !== toolId)
      : [...selectedToolIds, toolId];
    setSelectedToolIds(newIds);
    saveSelectedTools(newIds);
  };

  const handleRemoveTool = (toolId: string) => {
    const newIds = selectedToolIds.filter((id) => id !== toolId);
    setSelectedToolIds(newIds);
    saveSelectedTools(newIds);
  };

  const handleDownload = async (tool: ThirdPartyTool) => {
    setToolStatuses((prev) => ({
      ...prev,
      [tool.id]: {
        ...prev[tool.id],
        downloading: true,
        downloadProgress: 0,
      },
    }));

    try {
      const filePath = await invoke<string>("download_tool", { toolId: tool.id });

      if (tool.tool_type === "install") {
        await invoke("open_tool_installer", { filePath });
        setToolStatuses((prev) => ({
          ...prev,
          [tool.id]: {
            ...prev[tool.id],
            downloading: false,
          },
        }));
        startInstallationPolling(tool.id);
      } else {
        setToolStatuses((prev) => ({
          ...prev,
          [tool.id]: {
            installed: true,
            downloading: false,
            checking: false,
            downloadProgress: 100,
          },
        }));
        await refreshTools();
      }
    } catch (error) {
      console.error("Failed to download tool:", error);
      setToolStatuses((prev) => ({
        ...prev,
        [tool.id]: {
          ...prev[tool.id],
          downloading: false,
        },
      }));
    }
  };

  const startInstallationPolling = (toolId: string) => {
    const pollInterval = setInterval(async () => {
      try {
        const installed = await invoke<boolean>("check_tool_installed", { toolId });
        if (installed) {
          clearInterval(pollInterval);
          setToolStatuses((prev) => ({
            ...prev,
            [toolId]: {
              installed: true,
              checking: false,
              downloading: false,
              downloadProgress: 100,
            },
          }));
          await refreshTools();
        }
      } catch (error) {
        console.error("Failed to check tool installed:", error);
      }
    }, 3000);

    setTimeout(() => {
      clearInterval(pollInterval);
    }, 300000);
  };

  const handleRun = async (tool: ThirdPartyTool) => {
    try {
      await invoke("run_tool", { toolId: tool.id });
    } catch (error) {
      console.error("Failed to run tool:", error);
    }
  };

  const handleClick = (tool: ThirdPartyTool) => {
    const status = toolStatuses[tool.id];
    if (!status || status.downloading || status.checking) return;
    if (status.installed) {
      handleRun(tool);
    } else {
      handleDownload(tool);
    }
  };

  const selectedTools = tools.filter(({ tool }) => selectedToolIds.includes(tool.id)).map(({ tool }) => tool);

  return (
    <Box>
      <Flex justify="space-between" align="center" mb={3}>
        <Text fontSize="sm" fontWeight="semibold" color={headerColor}>
          {t("home.quickTools")}
        </Text>
        <IconButton
          aria-label="添加工具"
          icon={<Icon as={Plus} boxSize={4} />}
          size="xs"
          variant="ghost"
          onClick={onOpen}
        />
      </Flex>

      {selectedTools.length === 0 ? (
        <LiquidGlassToolCard
          isDashed
          p={4}
          textAlign="center"
          onClick={onOpen}
        >
          <Icon as={Plus} boxSize={6} color={descColor} mb={2} />
          <Text fontSize="xs" color={descColor}>
            {t("home.addQuickTools")}
          </Text>
        </LiquidGlassToolCard>
      ) : (
        <Grid templateColumns="repeat(2, 1fr)" gap={2}>
          {selectedTools.map((tool) => {
            const status = toolStatuses[tool.id] || {
              installed: false,
              checking: false,
              downloading: false,
              downloadProgress: 0,
            };
            const ToolIcon = getToolIcon(tool.id);
            const toolIconImage = getToolIconImage(tool.id);

            return (
              <LiquidGlassToolCard
                key={tool.id}
                p={3}
                position="relative"
                cursor={status.downloading || status.checking ? "wait" : "pointer"}
                onClick={() => handleClick(tool)}
              >
                <IconButton
                  aria-label="移除"
                  icon={<Icon as={X} boxSize={3} />}
                  size="xs"
                  position="absolute"
                  top={1}
                  right={1}
                  variant="ghost"
                  onClick={(e) => {
                    e.stopPropagation();
                    handleRemoveTool(tool.id);
                  }}
                />

                <Flex align="center" gap={2}>
                  <Flex
                    h={8}
                    w={8}
                    align="center"
                    justify="center"
                    borderRadius="md"
                    bg={cardBg}
                    flexShrink={0}
                    overflow="hidden"
                  >
                    {toolIconImage ? (
                      <Image
                        src={toolIconImage}
                        alt={tool.name}
                        w="24px"
                        h="24px"
                        objectFit="contain"
                        fallback={<Icon as={ToolIcon} boxSize={4} color={iconColor} />}
                      />
                    ) : (
                      <Icon as={ToolIcon} boxSize={4} color={iconColor} />
                    )}
                  </Flex>
                  <Box flex={1} minW={0}>
                    <Text
                      fontSize="xs"
                      fontWeight="medium"
                      color={titleColor}
                      noOfLines={1}
                    >
                      {t(`tools.tools.${tool.id}`, tool.name)}
                    </Text>
                    <Text fontSize="2xs" color={descColor}>
                      {status.checking
                        ? t("tools.status.scanning")
                        : status.downloading
                        ? `${status.downloadProgress}%`
                        : status.installed
                        ? t("tools.status.installed")
                        : t("tools.status.notInstalled")}
                    </Text>
                  </Box>
                </Flex>
              </LiquidGlassToolCard>
            );
          })}
        </Grid>
      )}

      <Modal isOpen={isOpen} onClose={onClose} size="lg" scrollBehavior="inside">
        <ModalOverlay />
        <ModalContent maxH="80vh">
          <ModalHeader>{t("home.selectTools")}</ModalHeader>
          <ModalCloseButton />
          <ModalBody pb={6} overflowY="auto" sx={{
            "&::-webkit-scrollbar": {
              width: "6px",
              height: "6px",
            },
            "&::-webkit-scrollbar-track": {
              background: "transparent",
              margin: "10px 0",
            },
            "&::-webkit-scrollbar-thumb": {
              background: config.primaryColor,
              borderRadius: "3px",
              minHeight: "40px",
            },
            "&::-webkit-scrollbar-thumb:hover": {
              background: config.primaryColor,
              opacity: 0.8,
              filter: "brightness(0.9)",
            },
          }}>
            <VStack align="stretch" spacing={2}>
              {tools.map(({ tool }) => {
                const ToolIcon = getToolIcon(tool.id);
                const toolIconImage = getToolIconImage(tool.id);
                
                return (
                  <Flex
                    key={tool.id}
                    align="center"
                    p={3}
                    bg={dialogBg}
                    borderRadius="md"
                    border="1px solid"
                    borderColor={borderColor}
                    cursor="pointer"
                    onClick={() => handleToggleTool(tool.id)}
                    _hover={{ bg: dialogHoverBg }}
                  >
                    <Checkbox
                      isChecked={selectedToolIds.includes(tool.id)}
                      onChange={() => handleToggleTool(tool.id)}
                      mr={3}
                    />
                    <Flex
                      h={8}
                      w={8}
                      align="center"
                      justify="center"
                      borderRadius="md"
                      bg={dialogIconBg}
                      mr={3}
                      overflow="hidden"
                    >
                      {toolIconImage ? (
                        <Image
                          src={toolIconImage}
                          alt={tool.name}
                          w="24px"
                          h="24px"
                          objectFit="contain"
                          fallback={<Icon as={ToolIcon} boxSize={4} color={iconColor} />}
                        />
                      ) : (
                        <Icon as={ToolIcon} boxSize={4} color={iconColor} />
                      )}
                    </Flex>
                    <Box flex={1}>
                      <HStack>
                        <Text fontWeight="medium" color={titleColor}>
                          {t(`tools.tools.${tool.id}`, tool.name)}
                        </Text>
                        <Badge
                          colorScheme={categoryColors[tool.category] || "gray"}
                          fontSize="xs"
                        >
                          {tool.category === "hardware"
                            ? t("tools.hardware")
                            : tool.category === "optimization"
                            ? t("tools.optimization")
                            : tool.category}
                        </Badge>
                      </HStack>
                      <Text fontSize="xs" color={descColor}>
                        {t(`tools.descriptions.${tool.id}`, tool.description)}
                      </Text>
                    </Box>
                  </Flex>
                );
              })}
            </VStack>
          </ModalBody>
        </ModalContent>
      </Modal>
    </Box>
  );
}
