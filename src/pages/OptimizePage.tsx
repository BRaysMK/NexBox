import {
  Box,
  Heading,
  VStack,
  Text,
  useColorModeValue,
  SimpleGrid,
} from "@chakra-ui/react";
import {
  Cpu,
  Trash2,
  MemoryStick,
  Gauge,
  Zap,
  HardDrive,
} from "lucide-react";
import { Link } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";

interface ToolItem {
  id: string;
  path: string;
  icon: React.ComponentType<{ size?: number; strokeWidth?: number }>;
  titleKey: string;
  descriptionKey: string;
  color: string;
}

function ToolCard({ tool }: { tool: ToolItem }) {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const cardBg = useColorModeValue("white", "#111111");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const IconComponent = tool.icon;

  const cardContent = (
    <VStack align="start" spacing={4}>
      <Box
        w={12}
        h={12}
        borderRadius="xl"
        bg={`${tool.color}20`}
        display="flex"
        alignItems="center"
        justifyContent="center"
        color={tool.color}
      >
        <IconComponent size={28} />
      </Box>
      <VStack align="start" spacing={1}>
        <Text color={headingColor} fontSize="lg" fontWeight="bold">
          {t(tool.titleKey)}
        </Text>
        <Text color={subTextColor} fontSize="sm">
          {t(tool.descriptionKey)}
        </Text>
      </VStack>
    </VStack>
  );

  if (liquidGlassEnabled) {
    return (
      <Link to={tool.path}>
        <LiquidGlassCard
          w="full"
          h="full"
          minH="200px"
          cursor="pointer"
          p={6}
          position="relative"
        >
          {cardContent}
        </LiquidGlassCard>
      </Link>
    );
  }

  return (
    <Link to={tool.path}>
      <Box
        bg={cardBg}
        borderRadius="xl"
        p={6}
        minH="200px"
        cursor="pointer"
        border="2px solid"
        borderColor={borderColor}
        transition="all 0.2s"
        _hover={{
          borderColor: tool.color,
          bg: `${tool.color}10`,
        }}
        position="relative"
        overflow="hidden"
      >
        {cardContent}
      </Box>
    </Link>
  );
}

export default function OptimizePage() {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const cardBg = useColorModeValue("white", "#111111");
  const cardBorder = useColorModeValue("gray.200", "#333333");

  const tools: ToolItem[] = [
    {
      id: "storage-clean",
      path: "/optimize/storage-clean",
      icon: HardDrive,
      titleKey: "storageClean.title",
      descriptionKey: "storageClean.description",
      color: "#3182CE",
    },
    {
      id: "memory-cleanup",
      path: "/optimize/memory-cleanup",
      icon: MemoryStick,
      titleKey: "optimization.memoryCleanup.title",
      descriptionKey: "optimization.memoryCleanup.description",
      color: "#38A169",
    },
    {
      id: "ace-optimize",
      path: "/optimize/ace-optimize",
      icon: Gauge,
      titleKey: "optimization.aceOptimize.title",
      descriptionKey: "optimization.aceOptimize.description",
      color: "#DD6B20",
    },
    {
      id: "memory-limit",
      path: "/optimize/memory-limit",
      icon: Cpu,
      titleKey: "optimization.memoryLimit.title",
      descriptionKey: "optimization.memoryLimit.description",
      color: "#FF6B9D",
    },
    {
      id: "shader-cache",
      path: "/optimize/shader-cache",
      icon: Trash2,
      titleKey: "shaderCache.title",
      descriptionKey: "builtinTools.shaderCacheDesc",
      color: "#EF4444",
    },
    {
      id: "power-management",
      path: "/optimize/power-management",
      icon: Zap,
      titleKey: "optimization.powerManagement.title",
      descriptionKey: "optimization.powerManagement.description",
      color: "#F6AD55",
    },
  ];

  const content = (
    <VStack align="start" spacing={6}>
      <Heading size="lg" color={headingColor}>
        {t("optimization.pageTitle")}
      </Heading>
      <SimpleGrid columns={{ base: 1, md: 2, lg: 3 }} spacing={4} w="full">
        {tools.map((tool) => (
          <ToolCard key={tool.id} tool={tool} />
        ))}
      </SimpleGrid>
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
