import {
  Box,
  Flex,
  Grid,
  Text,
  Heading,
  Icon,
  useColorModeValue,
  Badge,
  VStack,
  HStack,
  Divider,
  Progress,
  useToast,
  IconButton,
} from "@chakra-ui/react";
import { AnimatePresence, motion } from "framer-motion";
import { LiquidGlassMenuItem } from "@/components/special/liquid-glass-menu-item";
import { LiquidGlassToolCard } from "@/components/special/liquid-glass-tool-card";
import {
  Cpu,
  Zap,
  Wrench,
  Layers,
  Network,
  TrendingUp,
  Download,
  Play,
  Circle,
  Monitor,
  Bot,
  RefreshCw,
  Volume2,
  Trash2,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useAppStartup } from "@/contexts/app-startup-context";
import { Image } from "@chakra-ui/react";
import { useThemeColor } from "@/contexts/theme-color-context";

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

const handleToolClick = async (toolId: string) => {
};

interface ToolCard {
  id: string;
  title: string;
  description: string;
  icon: React.ElementType;
  category: "hardware" | "assistant" | "network" | "optimization";
  type: "builtin" | "thirdparty";
}

const getTools = (t: (key: string) => string): ToolCard[] => [
];

const getMenuItems = (t: (key: string) => string) => [
  { id: "hardware", label: t("tools.hardware"), icon: Wrench },
  { id: "assistant", label: t("tools.assistant"), icon: Layers },
  { id: "network", label: t("tools.network"), icon: Network },
  { id: "optimization", label: t("tools.optimization"), icon: TrendingUp },
];

const getCategoryLabels = (t: (key: string) => string): Record<string, string> => ({
  hardware: t("tools.hardware"),
  assistant: t("tools.assistant"),
  network: t("tools.network"),
  optimization: t("tools.optimization"),
});

const categoryColors: Record<string, string> = {
  hardware: "blue",
  assistant: "purple",
  network: "green",
  optimization: "orange",
};

function ToolCardComponent({
  tool,
  categoryLabels,
}: {
  tool: ToolCard;
  categoryLabels: Record<string, string>;
}) {
  const iconColor = useColorModeValue("gray.700", "#cccccc");
  const titleColor = useColorModeValue("gray.800", "#e0e0e0");
  const descColor = useColorModeValue("gray.500", "#888888");

  return (
    <LiquidGlassToolCard
      size="md"
      onClick={() => handleToolClick(tool.id)}
    >
      <VStack align="start" spacing={3}>
        <Flex
          h={12}
          w={12}
          align="center"
          justify="center"
          borderRadius="lg"
          bg={useColorModeValue("gray.100", "#222222")}
        >
          <Icon as={tool.icon} boxSize={6} color={iconColor} />
        </Flex>
        <Box flex={1} w="full">
          <HStack justify="space-between" align="start" mb={1}>
            <Text fontSize="sm" fontWeight="semibold" color={titleColor}>
              {tool.title}
            </Text>
            <Badge colorScheme={categoryColors[tool.category]} fontSize="xs" variant="subtle">
              {categoryLabels[tool.category]}
            </Badge>
          </HStack>
          <Text fontSize="xs" color={descColor} lineHeight="short">
            {tool.description}
          </Text>
        </Box>
      </VStack>
    </LiquidGlassToolCard>
  );
}

function ThirdPartyToolCard({
  tool,
  initialInstalled,
  onStatusChange,
  onInstallComplete,
  categoryLabels,
}: {
  tool: ThirdPartyTool;
  initialInstalled: boolean;
  onStatusChange?: (id: string, status: ToolStatus) => void;
  onInstallComplete?: () => void;
  categoryLabels: Record<string, string>;
}) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<ToolStatus>({
    installed: initialInstalled,
    checking: false,
    downloading: false,
    downloadProgress: 0,
  });
  const toast = useToast();

  const iconColor = useColorModeValue("gray.700", "#cccccc");
  const titleColor = useColorModeValue("gray.800", "#e0e0e0");
  const descColor = useColorModeValue("gray.500", "#888888");

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
      case "msi-afterburner":
        return Monitor;
      case "geek":
        return Trash2;
      default:
        return Wrench;
    }
  };

  const toolIconImage = getToolIconImage(tool.id);
  const FallbackIcon = getToolIcon(tool.id);

  useEffect(() => {
    const unlisten = listen<{ tool_id: string; progress: number }>(
      "tool-download-progress",
      (event) => {
        const { tool_id, progress } = event.payload;
        if (tool_id === tool.id) {
          const newStatus = { ...status, downloadProgress: progress };
          setStatus(newStatus);
          onStatusChange?.(tool.id, newStatus);
        }
      },
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [tool.id, status, onStatusChange]);

  const updateStatus = (newStatus: ToolStatus) => {
    setStatus(newStatus);
    onStatusChange?.(tool.id, newStatus);
  };

  const startInstallationPolling = () => {
    const pollInterval = setInterval(async () => {
      try {
        const installed = await invoke<boolean>("check_tool_installed", { toolId: tool.id });
        if (installed) {
          clearInterval(pollInterval);
          const newStatus = {
            installed: true,
            checking: false,
            downloading: false,
            downloadProgress: 100,
          };
          updateStatus(newStatus);
          toast({
            title: t("tools.messages.installComplete"),
            description: t("tools.messages.installCompleteDesc"),
            status: "success",
            duration: 3000,
            isClosable: true,
          });
          onInstallComplete?.();
        }
      } catch (error) {
        console.error("Failed to check tool installed:", error);
      }
    }, 3000);

    setTimeout(() => {
      clearInterval(pollInterval);
    }, 300000);
  };

  const handleDownload = async () => {
    updateStatus({ ...status, downloading: true, downloadProgress: 0 });

    try {
      const filePath = await invoke<string>("download_tool", { toolId: tool.id });

      if (tool.tool_type === "install") {
        await invoke("open_tool_installer", { filePath });
        updateStatus({ ...status, downloading: false });
        toast({
          title: t("tools.messages.installerStartedAlt"),
          description: t("tools.messages.installerStartedDescAlt"),
          status: "info",
          duration: 5000,
          isClosable: true,
        });
        startInstallationPolling();
      } else {
        toast({
          title: t("tools.messages.downloadComplete"),
          description: t("tools.messages.downloadCompleteDesc"),
          status: "success",
          duration: 3000,
          isClosable: true,
        });
        const newStatus = {
          installed: true,
          checking: false,
          downloading: false,
          downloadProgress: 100,
        };
        updateStatus(newStatus);
        onInstallComplete?.();
      }
    } catch (error) {
      console.error("Failed to download tool:", error);
      toast({
        title: t("tools.messages.downloadFailed"),
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
      updateStatus({ ...status, downloading: false });
    }
  };

  const handleRun = async () => {
    try {
      await invoke("run_tool", { toolId: tool.id });
    } catch (error) {
      console.error("Failed to run tool:", error);
      toast({
        title: t("tools.messages.runFailed"),
        description: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    }
  };

  const handleClick = () => {
    if (status.downloading || status.checking) return;
    if (status.installed) {
      handleRun();
    } else {
      handleDownload();
    }
  };

  return (
    <LiquidGlassToolCard
      size="md"
      cursor={status.downloading || status.checking ? "wait" : "pointer"}
      onClick={handleClick}
      position="relative"
    >
      {status.installed && (
        <Box position="absolute" top={3} right={3}>
          <Icon as={Circle} boxSize={3} fill="green.400" color="green.400" />
        </Box>
      )}

      <VStack align="start" spacing={3}>
        <Flex
          h={12}
          w={12}
          align="center"
          justify="center"
          borderRadius="lg"
          bg={useColorModeValue("gray.100", "#222222")}
          overflow="hidden"
        >
          {toolIconImage ? (
            <Image
              src={toolIconImage}
              alt={tool.name}
              w="32px"
              h="32px"
              objectFit="contain"
              fallback={<Icon as={FallbackIcon} boxSize={6} color={iconColor} />}
            />
          ) : (
            <Icon as={FallbackIcon} boxSize={6} color={iconColor} />
          )}
        </Flex>
        <Box flex={1} w="full">
          <HStack justify="space-between" align="start" mb={1}>
            <Text fontSize="sm" fontWeight="semibold" color={titleColor}>
              {t(`tools.tools.${tool.id}`, tool.name)}
            </Text>
            <Badge
              colorScheme={categoryColors[tool.category] || "gray"}
              fontSize="xs"
              variant="subtle"
            >
              {categoryLabels[tool.category] || tool.category}
            </Badge>
          </HStack>
          <Text fontSize="xs" color={descColor} lineHeight="short" mb={2}>
            {t(`tools.descriptions.${tool.id}`, tool.description)}
          </Text>

          {status.downloading && (
            <Box w="full">
              <Progress
                value={status.downloadProgress}
                size="sm"
                colorScheme="teal"
                borderRadius="full"
              />
              <Text fontSize="xs" color={descColor} mt={1}>
                {t("tools.status.downloading")} {status.downloadProgress}%
              </Text>
            </Box>
          )}

          {!status.downloading && !status.installed && !status.checking && (
            <HStack spacing={1} color="teal.500">
              <Icon as={Download} boxSize={3} />
              <Text fontSize="xs">{t("tools.buttons.download")}</Text>
            </HStack>
          )}

          {!status.downloading && !status.installed && status.checking && (
            <HStack spacing={1} color="gray.500">
              <Text fontSize="xs">{t("tools.status.scanning")}</Text>
            </HStack>
          )}

          {!status.downloading && status.installed && (
            <HStack spacing={1} color="green.500">
              <Icon as={Play} boxSize={3} />
              <Text fontSize="xs">{t("tools.buttons.run")}</Text>
            </HStack>
          )}
        </Box>
      </VStack>
    </LiquidGlassToolCard>
  );
}

function ToolSection({
  title,
  tools: sectionTools,
  activeCategory,
  categoryLabels,
}: {
  title: string;
  tools: ToolCard[];
  activeCategory: string;
  categoryLabels: Record<string, string>;
}) {
  const filteredTools =
    activeCategory === "all"
      ? sectionTools
      : sectionTools.filter((tool) => tool.category === activeCategory);

  if (filteredTools.length === 0) return null;

  const sectionTitleColor = useColorModeValue("gray.800", "#ffffff");
  const dividerColor = useColorModeValue("gray.200", "#333333");

  return (
    <Box mb={8}>
      <HStack mb={4} spacing={3}>
        <Text fontSize="lg" fontWeight="bold" color={sectionTitleColor}>
          {title}
        </Text>
        <Badge fontSize="xs" colorScheme="gray">
          {filteredTools.length}
        </Badge>
      </HStack>
      <Divider borderColor={dividerColor} mb={4} />
      <Grid
        templateColumns={{
          base: "1fr",
          sm: "repeat(2, 1fr)",
          md: "repeat(3, 1fr)",
        }}
        gap={4}
      >
        {filteredTools.map((tool) => (
          <ToolCardComponent key={tool.id} tool={tool} categoryLabels={categoryLabels} />
        ))}
      </Grid>
    </Box>
  );
}

function ThirdPartyToolSection({
  title,
  activeCategory,
  categoryLabels,
}: {
  title: string;
  activeCategory: string;
  categoryLabels: Record<string, string>;
}) {
  const { t } = useTranslation();
  const { tools, refreshTools } = useAppStartup();
  const [localStatus, setLocalStatus] = useState<Record<string, ToolStatus>>({});
  const [isRefreshing, setIsRefreshing] = useState(false);

  const sectionTitleColor = useColorModeValue("gray.800", "#ffffff");
  const dividerColor = useColorModeValue("gray.200", "#333333");

  useEffect(() => {
    if (tools.length > 0) {
      const map: Record<string, ToolStatus> = {};
      for (const { tool, installed } of tools) {
        map[tool.id] = {
          installed,
          checking: false,
          downloading: false,
          downloadProgress: 0,
        };
      }
      setLocalStatus(map);
    }
  }, [tools]);

  const handleRefresh = async () => {
    setIsRefreshing(true);
    await refreshTools();
    setTimeout(() => setIsRefreshing(false), 500);
  };

  const filteredTools =
    activeCategory === "all"
      ? tools
      : tools.filter(({ tool }) => tool.category === activeCategory);

  const sortedTools = [...filteredTools].sort((a, b) => {
    const aInstalled = a.installed ? 1 : 0;
    const bInstalled = b.installed ? 1 : 0;
    return bInstalled - aInstalled;
  });

  if (filteredTools.length === 0) return null;

  return (
    <Box mb={8}>
      <HStack mb={4} spacing={3} justify="space-between">
        <HStack spacing={3}>
          <Text fontSize="lg" fontWeight="bold" color={sectionTitleColor}>
            {title}
          </Text>
          <Badge fontSize="xs" colorScheme="gray">
            {filteredTools.length}
          </Badge>
        </HStack>
        <IconButton
          aria-label={t("tools.ariaLabels.refreshToolList")}
          icon={<Icon as={RefreshCw} boxSize={4} />}
          size="sm"
          variant="ghost"
          onClick={handleRefresh}
          isLoading={isRefreshing}
        />
      </HStack>
      <Divider borderColor={dividerColor} mb={4} />
      <Grid
        templateColumns={{
          base: "1fr",
          sm: "repeat(2, 1fr)",
          md: "repeat(3, 1fr)",
        }}
        gap={4}
      >
        {sortedTools.map(({ tool, installed }) => (
          <ThirdPartyToolCard 
            key={tool.id} 
            tool={tool} 
            initialInstalled={installed} 
            onStatusChange={(id, status) => {
              setLocalStatus(prev => ({ ...prev, [id]: status }));
            }}
            onInstallComplete={refreshTools}
            categoryLabels={categoryLabels} 
          />
        ))}
      </Grid>
    </Box>
  );
}

export default function ToolsPage() {
  const [activeCategory, setActiveCategory] = useState<string>("all");
  const { t } = useTranslation();
  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const { config } = useThemeColor();

  const tools = getTools(t);
  const menuItems = getMenuItems(t);
  const categoryLabels = getCategoryLabels(t);

  const builtinTools = tools.filter((tool) => tool.type === "builtin");

  const pageVariants = {
    initial: { opacity: 0, y: 10 },
    in: { opacity: 1, y: 0 },
    out: { opacity: 0, y: -10 },
  };

  const pageTransition = {
    type: "tween",
    ease: "easeOut",
    duration: 0.25,
  };

  return (
    <Flex gap={6} pt={8}>
      <Box w="180px" flexShrink={0} position="sticky" top={8} alignSelf="flex-start">
        <VStack spacing={0.5} align="stretch">
          <LiquidGlassMenuItem
            isActive={activeCategory === "all"}
            onClick={() => setActiveCategory("all")}
            icon={Layers}
          >
            {t("tools.all")}
          </LiquidGlassMenuItem>
          {menuItems.map((item) => {
            const Icon = item.icon;
            const isActive = activeCategory === item.id;

            return (
              <LiquidGlassMenuItem
                key={item.id}
                isActive={isActive}
                onClick={() => setActiveCategory(item.id)}
                icon={Icon}
              >
                {item.label}
              </LiquidGlassMenuItem>
            );
          })}
        </VStack>
      </Box>

      <Box 
        flex={1} 
        overflowY="auto"
        sx={{
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
        }}
      >
        <AnimatePresence mode="wait">
          <motion.div
            key={activeCategory}
            initial="initial"
            animate="in"
            exit="out"
            variants={pageVariants}
            transition={pageTransition}
            style={{ position: 'relative', zIndex: 1 }}
          >
            <Heading size="lg" color={headingColor} mb={6}>
              {t("tools.title")}
            </Heading>

            <ToolSection
              title={t("tools.builtinTools")}
              tools={builtinTools}
              activeCategory={activeCategory}
              categoryLabels={categoryLabels}
            />
            <ThirdPartyToolSection
              title={t("tools.thirdpartyTools")}
              activeCategory={activeCategory}
              categoryLabels={categoryLabels}
            />
          </motion.div>
        </AnimatePresence>
      </Box>
    </Flex>
  );
}
