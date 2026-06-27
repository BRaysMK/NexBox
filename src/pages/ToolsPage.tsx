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
  useToast,
  IconButton,
  Tooltip,
} from "@chakra-ui/react";
import { AnimatePresence, motion } from "framer-motion";
import { useTransitionMode, getVariants, getTransitionConfig } from "@/components/ui/animated-page";
import { LiquidGlassMenuItem } from "@/components/special/liquid-glass-menu-item";
import { LiquidGlassToolCard } from "@/components/special/liquid-glass-tool-card";
import { useThemeColor } from "@/contexts/theme-color-context";
import {
  Cpu,
  Zap,
  Wrench,
  Layers,
  Network,
  TrendingUp,
  Play,
  Circle,
  Monitor,
  Bot,
  Volume2,
  Trash2,
  Plus,
  X,
  ExternalLink,
  Shield,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAppStartup } from "@/contexts/app-startup-context";
import { Image } from "@chakra-ui/react";
import { LazyStore } from "@tauri-apps/plugin-store";

const SETTINGS_FILE = "settings.json";
const store = new LazyStore(SETTINGS_FILE);
const CUSTOM_TOOLS_KEY = "custom-added-tools";

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
  website_url: string | null;
  check_executable: string | null;
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
  const { getActiveColor } = useThemeColor();

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
  categoryLabels,
  customToolPath,
  onAddCustomTool,
  onRemoveCustomTool,
}: {
  tool: ThirdPartyTool;
  initialInstalled: boolean;
  categoryLabels: Record<string, string>;
  customToolPath?: string;
  onAddCustomTool?: (toolId: string, filePath: string) => void;
  onRemoveCustomTool?: (toolId: string) => void;
}) {
  const { t } = useTranslation();
  const [installed, setInstalled] = useState(initialInstalled);
  const [isAdding, setIsAdding] = useState(false);
  const toast = useToast();

  const iconColor = useColorModeValue("gray.700", "#cccccc");
  const titleColor = useColorModeValue("gray.800", "#e0e0e0");
  const descColor = useColorModeValue("gray.500", "#888888");
  const { getActiveColor } = useThemeColor();

  const isCustomAdded = !!customToolPath;
  const isInstalled = installed || isCustomAdded;

  useEffect(() => {
    setInstalled(initialInstalled);
  }, [initialInstalled]);

  const getToolIcon = (toolId: string) => {
    switch (toolId) {
      case "memreduct":
        return Zap;
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

  const handleRun = async () => {
    try {
      if (customToolPath) {
        await invoke("launch_game", { gamePath: customToolPath });
      } else {
        await invoke("run_tool", { toolId: tool.id });
      }
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

  const handleOpenWebsite = async () => {
    if (!tool.website_url) return;
    try {
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(tool.website_url);
    } catch (error) {
      console.error("Failed to open website:", error);
    }
  };

  const handleClick = () => {
    if (isInstalled) {
      handleRun();
    } else {
      handleOpenWebsite();
    }
  };

  const handleCustomButtonClick = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (isAdding) return;
    setIsAdding(true);
    try {
      const selectedPath = await invoke<string | null>("select_exe_file");
      if (selectedPath) {
        onAddCustomTool?.(tool.id, selectedPath);
        toast({
          title: t("tools.customAdded.addSuccess"),
          status: "success",
          duration: 2000,
          isClosable: true,
        });
      }
    } catch (error) {
      console.error("Failed to select file:", error);
      toast({
        title: t("tools.customAdded.addFailed"),
        status: "error",
        duration: 2000,
        isClosable: true,
      });
    } finally {
      setIsAdding(false);
    }
  };

  const handleRemoveCustom = (e: React.MouseEvent) => {
    e.stopPropagation();
    onRemoveCustomTool?.(tool.id);
    toast({
      title: t("tools.customAdded.removed"),
      status: "info",
      duration: 2000,
      isClosable: true,
    });
  };

  return (
    <LiquidGlassToolCard
      size="md"
      cursor="pointer"
      onClick={handleClick}
      position="relative"
    >
      {isInstalled && !isCustomAdded && (
        <Box position="absolute" top={3} right={3}>
          <Icon as={Circle} boxSize={3} fill="green.400" color="green.400" />
        </Box>
      )}

      {isCustomAdded && (
        <HStack position="absolute" top={2} right={2} spacing={1}>
          <Icon as={Circle} boxSize={3} fill="green.400" color="green.400" />
          <Tooltip label={t("tools.customAdded.remove")} placement="top">
            <IconButton
              aria-label={t("tools.customAdded.remove")}
              icon={<Icon as={X} boxSize={3} />}
              size="xs"
              variant="ghost"
              colorScheme="red"
              onClick={handleRemoveCustom}
            />
          </Tooltip>
        </HStack>
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

          {!isInstalled && (
            <HStack spacing={1} color={getActiveColor()}>
              <Icon as={ExternalLink} boxSize={3} />
              <Text fontSize="xs">{t("tools.buttons.website")}</Text>
            </HStack>
          )}

          {isInstalled && (
            <HStack spacing={1} color="green.500">
              <Icon as={Play} boxSize={3} />
              <Text fontSize="xs">{t("tools.buttons.run")}</Text>
            </HStack>
          )}
        </Box>
      </VStack>

      {!isInstalled && (
        <Tooltip
          label={t("tools.customAdded.add")}
          placement="top"
        >
          <IconButton
            aria-label={t("tools.customAdded.add")}
            icon={<Icon as={Plus} boxSize={3} />}
            size="xs"
            position="absolute"
            bottom={3}
            right={3}
            borderRadius="full"
            variant="outline"
            colorScheme="gray"
            bg="transparent"
            isLoading={isAdding}
            isDisabled={isAdding}
            _hover={{
              bg: useColorModeValue("gray.100", "gray.700"),
            }}
            onClick={handleCustomButtonClick}
          />
        </Tooltip>
      )}
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

function OfficialToolSection({
  activeCategory,
}: {
  activeCategory: string;
}) {
  const { t } = useTranslation();
  const sectionTitleColor = useColorModeValue("gray.800", "#ffffff");
  const dividerColor = useColorModeValue("gray.200", "#333333");
  const iconColor = useColorModeValue("gray.700", "#cccccc");
  const titleColor = useColorModeValue("gray.800", "#e0e0e0");
  const descColor = useColorModeValue("gray.500", "#888888");

  // Official recommendation only shows when category is "all"
  if (activeCategory !== "all") return null;

  const handleOpenMCTier = () => {
    import("@tauri-apps/plugin-shell").then(({ open }) => {
      open("https://mctier.pmhs.top/");
    }).catch(() => {
      window.open("https://mctier.pmhs.top/", "_blank");
    });
  };

  const handleOpenSjmcl = () => {
    import("@tauri-apps/plugin-shell").then(({ open }) => {
      open("https://mc.sjtu.cn/sjmcl/");
    }).catch(() => {
      window.open("https://mc.sjtu.cn/sjmcl/", "_blank");
    });
  };

  const handleOpenDdegame = () => {
    import("@tauri-apps/plugin-shell").then(({ open }) => {
      open("https://www.ddegame.cn/");
    }).catch(() => {
      window.open("https://www.ddegame.cn/", "_blank");
    });
  };

  const handleOpenHuorong = () => {
    import("@tauri-apps/plugin-shell").then(({ open }) => {
      open("https://www.huorong.cn/");
    }).catch(() => {
      window.open("https://www.huorong.cn/", "_blank");
    });
  };

  return (
    <Box mb={8}>
      <HStack mb={4} spacing={3}>
        <Text fontSize={"lg"} fontWeight={"bold"} color={sectionTitleColor}>
          {t("tools.officialTools")}
        </Text>
        <Badge fontSize={"xs"} colorScheme={"blue"}>
          {t("tools.recommended")}
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
        <LiquidGlassToolCard size={"md"} onClick={handleOpenMCTier}>
          <VStack align={"start"} spacing={3}>
            <Flex
              h={12}
              w={12}
              align={"center"}
              justify={"center"}
              borderRadius={"lg"}
              bg={useColorModeValue("gray.100", "#222222")}
              overflow={"hidden"}
            >
              <Image
                src={getToolIconImage("mctier") || ""}
                alt={"MCTier"}
                w={"32px"}
                h={"32px"}
                objectFit={"contain"}
                fallback={<ExternalLink size={24} color={iconColor} />}
              />
            </Flex>
            <Box flex={1} w={"full"}>
              <HStack justify={"space-between"} align={"start"} mb={1}>
                <Text fontSize={"sm"} fontWeight={"semibold"} color={titleColor}>
                  MCTier
                </Text>
                <Badge colorScheme={"blue"} fontSize={"xs"} variant={"subtle"}>
                  {t("tools.recommended")}
                </Badge>
              </HStack>
              <Text fontSize={"xs"} color={descColor} lineHeight={"short"}>
                {t("tools.mctierDesc")}
              </Text>
            </Box>
          </VStack>
        </LiquidGlassToolCard>

        <LiquidGlassToolCard size={"md"} onClick={handleOpenSjmcl}>
          <VStack align={"start"} spacing={3}>
            <Flex
              h={12}
              w={12}
              align={"center"}
              justify={"center"}
              borderRadius={"lg"}
              bg={useColorModeValue("gray.100", "#222222")}
              overflow={"hidden"}
            >
              <Image
                src={getToolIconImage("sjmcl") || ""}
                alt={"SJMCL"}
                w={"32px"}
                h={"32px"}
                objectFit={"contain"}
                fallback={<ExternalLink size={24} color={iconColor} />}
              />
            </Flex>
            <Box flex={1} w={"full"}>
              <HStack justify={"space-between"} align={"start"} mb={1}>
                <Text fontSize={"sm"} fontWeight={"semibold"} color={titleColor}>
                  SJMCL
                </Text>
                <Badge colorScheme={"blue"} fontSize={"xs"} variant={"subtle"}>
                  {t("tools.recommended")}
                </Badge>
              </HStack>
              <Text fontSize={"xs"} color={descColor} lineHeight={"short"}>
                {t("tools.sjmclDesc")}
              </Text>
            </Box>
          </VStack>
        </LiquidGlassToolCard>

        <LiquidGlassToolCard size={"md"} onClick={handleOpenDdegame}>
          <VStack align={"start"} spacing={3}>
            <Flex
              h={12}
              w={12}
              align={"center"}
              justify={"center"}
              borderRadius={"lg"}
              bg={useColorModeValue("gray.100", "#222222")}
              overflow={"hidden"}
            >
              <Image
                src={getToolIconImage("ddegame") || ""}
                alt={"东东电竞"}
                w={"32px"}
                h={"32px"}
                objectFit={"contain"}
                fallback={<ExternalLink size={24} color={iconColor} />}
              />
            </Flex>
            <Box flex={1} w={"full"}>
              <HStack justify={"space-between"} align={"start"} mb={1}>
                <Text fontSize={"sm"} fontWeight={"semibold"} color={titleColor}>
                  东东电竞
                </Text>
                <Badge colorScheme={"blue"} fontSize={"xs"} variant={"subtle"}>
                  {t("tools.recommended")}
                </Badge>
              </HStack>
              <Text fontSize={"xs"} color={descColor} lineHeight={"short"}>
                {t("tools.ddegameDesc")}
              </Text>
            </Box>
          </VStack>
        </LiquidGlassToolCard>

        <LiquidGlassToolCard size={"md"} onClick={handleOpenHuorong}>
          <VStack align={"start"} spacing={3}>
            <Flex
              h={12}
              w={12}
              align={"center"}
              justify={"center"}
              borderRadius={"lg"}
              bg={useColorModeValue("gray.100", "#222222")}
              overflow={"hidden"}
            >
              <Image
                src={getToolIconImage("huorong") || ""}
                alt={"火绒安全"}
                w={"32px"}
                h={"32px"}
                objectFit={"contain"}
                fallback={<Shield size={24} color={iconColor} />}
              />
            </Flex>
            <Box flex={1} w={"full"}>
              <HStack justify={"space-between"} align={"start"} mb={1}>
                <Text fontSize={"sm"} fontWeight={"semibold"} color={titleColor}>
                  火绒安全
                </Text>
                <Badge colorScheme={"blue"} fontSize={"xs"} variant={"subtle"}>
                  {t("tools.recommended")}
                </Badge>
              </HStack>
              <Text fontSize={"xs"} color={descColor} lineHeight={"short"}>
                {t("tools.huorongDesc")}
              </Text>
            </Box>
          </VStack>
        </LiquidGlassToolCard>
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
  const { tools, initTools } = useAppStartup();
  const [customToolPaths, setCustomToolPaths] = useState<Record<string, string>>({});

  const sectionTitleColor = useColorModeValue("gray.800", "#ffffff");
  const dividerColor = useColorModeValue("gray.200", "#333333");

  useEffect(() => {
    if (tools.length === 0) {
      initTools();
    }
  }, []);

  useEffect(() => {
    const loadCustomTools = async () => {
      try {
        const savedTools = await store.get<Record<string, string>>(CUSTOM_TOOLS_KEY);
        if (savedTools && typeof savedTools === "object") {
          setCustomToolPaths(savedTools);
        }
      } catch (error) {
        console.error("Failed to load custom tools:", error);
      }
    };
    loadCustomTools();
  }, []);

  const addCustomTool = useCallback(async (toolId: string, filePath: string) => {
    setCustomToolPaths((prev) => {
      const newPaths = { ...prev, [toolId]: filePath };
      store.set(CUSTOM_TOOLS_KEY, newPaths);
      store.save();
      return newPaths;
    });
  }, []);

  const removeCustomTool = useCallback(async (toolId: string) => {
    setCustomToolPaths((prev) => {
      const newPaths = { ...prev };
      delete newPaths[toolId];
      store.set(CUSTOM_TOOLS_KEY, newPaths);
      store.save();
      return newPaths;
    });
  }, []);

  const filteredTools =
    activeCategory === "all"
      ? tools
      : tools.filter((tool) => tool.category === activeCategory);

  const sortedTools = [...filteredTools].sort((a, b) => {
    const aInstalled = customToolPaths[a.id] ? 1 : 0;
    const bInstalled = customToolPaths[b.id] ? 1 : 0;
    return bInstalled - aInstalled;
  });

  if (filteredTools.length === 0) return null;

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
        {sortedTools.map((tool) => (
          <ThirdPartyToolCard 
            key={tool.id} 
            tool={tool} 
            initialInstalled={!!customToolPaths[tool.id]} 
            categoryLabels={categoryLabels}
            customToolPath={customToolPaths[tool.id]}
            onAddCustomTool={addCustomTool}
            onRemoveCustomTool={removeCustomTool}
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
  const transitionMode = useTransitionMode();

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
          {transitionMode !== "off" ? (
            <motion.div
              key={activeCategory}
              initial="initial"
              animate="enter"
              exit="exit"
              variants={getVariants(transitionMode)}
              transition={getTransitionConfig(transitionMode)}
              style={{ position: 'relative', zIndex: 1 }}
            >
              <Heading size="lg" color={headingColor} mb={6}>
                {t("tools.title")}
              </Heading>

              <OfficialToolSection activeCategory={activeCategory} />

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
          ) : (
            <div key={activeCategory} style={{ position: 'relative', zIndex: 1 }}>
              <Heading size="lg" color={headingColor} mb={6}>
                {t("tools.title")}
              </Heading>

              <OfficialToolSection activeCategory={activeCategory} />

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
            </div>
          )}
        </AnimatePresence>
      </Box>
    </Flex>
  );
}
