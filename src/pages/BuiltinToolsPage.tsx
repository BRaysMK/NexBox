import { useState } from "react";
import {
  Box,
  Heading,
  VStack,
  Flex,
  useColorModeValue,
} from "@chakra-ui/react";
import {
  Palette,
  Crosshair,
  Layout,
  Cpu,
  Monitor,
  Zap,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { ViewGrid } from "@/components/special/view-grid";
import { ViewList } from "@/components/special/view-list";
import { LayoutToggle, type LayoutMode } from "@/components/special/layout-toggle";
import type { ViewItem } from "@/components/special/view-types";

const tools: ViewItem[] = [
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

export default function BuiltinToolsPage() {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const [layoutMode, setLayoutMode] = useState<LayoutMode>("grid");

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const cardBg = useColorModeValue("white", "#111111");
  const cardBorder = useColorModeValue("gray.200", "#333333");

  const content = (
    <VStack align="start" spacing={6}>
      <Flex w="full" justify="space-between" align="center">
        <Heading size="lg" color={headingColor}>
          {t("builtinTools.title")}
        </Heading>
        <LayoutToggle mode={layoutMode} onChange={setLayoutMode} />
      </Flex>
      {layoutMode === "grid" ? (
        <ViewGrid tools={tools} />
      ) : (
        <ViewList tools={tools} />
      )}
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
