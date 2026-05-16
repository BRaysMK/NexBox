import {
  Box,
  Heading,
  VStack,
  SimpleGrid,
  Text,
  useColorModeValue,
} from "@chakra-ui/react";
import { Palette, Crosshair, Layout, Cpu, Monitor, Zap } from "lucide-react";
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
  beta?: boolean;
}

function ToolCard({ tool }: { tool: ToolItem }) {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const cardBg = useColorModeValue("white", "#111111");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const IconComponent = tool.icon;

  const cardContent = (
    <>
      {tool.beta && (
        <Box
          position="absolute"
          top={2}
          right={2}
          fontSize="10px"
          fontWeight="700"
          color="#FF6B9D"
          bg="rgba(255,107,157,0.1)"
          px={1.5}
          py={0.5}
          borderRadius="full"
          zIndex={1}
        >
          BETA
        </Box>
      )}
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
    </>
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
        borderColor="transparent"
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

export default function BuiltinToolsPage() {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const cardBg = useColorModeValue("white", "#111111");
  const cardBorder = useColorModeValue("gray.200", "#333333");

  const tools: ToolItem[] = [
    {
      id: "display-filter",
      path: "/display-filter",
      icon: Palette,
      titleKey: "sidebar.displayFilter",
      descriptionKey: "builtinTools.displayFilterDesc",
      color: "#98DDD0",
    },
    {
      id: "crosshair",
      path: "/crosshair",
      icon: Crosshair,
      titleKey: "sidebar.crosshair",
      descriptionKey: "builtinTools.crosshairDesc",
      color: "#FF6B9D",
    },
    {
      id: "overlay-panel",
      path: "/overlay-panel",
      icon: Layout,
      titleKey: "sidebar.overlayPanel",
      descriptionKey: "builtinTools.overlayPanelDesc",
      color: "#9B59B6",
    },
    {
      id: "gpu-rename",
      path: "/gpu-rename",
      icon: Cpu,
      titleKey: "sidebar.gpuRename",
      descriptionKey: "builtinTools.gpuRenameDesc",
      color: "#F39C12",
    },
    {
      id: "resolution-converter",
      path: "/resolution-converter",
      icon: Monitor,
      titleKey: "sidebar.resolutionConverter",
      descriptionKey: "builtinTools.resolutionConverterDesc",
      color: "#4A90E2",
    },
    {
      id: "dlss-preset",
      path: "/dlss-preset",
      icon: Zap,
      titleKey: "sidebar.dlssPreset",
      descriptionKey: "builtinTools.dlssPresetDesc",
      color: "#76B900",
    },
  ];

  const content = (
    <VStack align="start" spacing={6}>
      <Heading size="lg" color={headingColor}>
        {t("builtinTools.title")}
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
