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
  Video,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { ViewGrid } from "@/components/special/view-grid";
import { ViewList } from "@/components/special/view-list";
import { LayoutToggle, type LayoutMode } from "@/components/special/layout-toggle";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { IslandIcon } from "@/components/special/island-icon";
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
  {
    id: "dynamic-island",
    path: "/dynamic-island",
    icon: IslandIcon,
    titleKey: "sidebar.dynamicIsland",
    descriptionKey: "builtinTools.dynamicIslandDesc",
    color: "#E91E63",
  },
  // {
  //   id: "screen-record",
  //   path: "/screen-record",
  //   icon: Video,
  //   titleKey: "sidebar.screenRecord",
  //   descriptionKey: "builtinTools.screenRecordDesc",
  //   color: "#E74C3C",
  // },
  // {
  //   id: "activation",
  //   path: "/activation",
  //   icon: ShieldCheck,
  //   titleKey: "activation.pageTitle",
  //   descriptionKey: "builtinTools.activationDesc",
  //   color: "#3182CE",
  // },
];

export default function BuiltinToolsPage() {
  const { t } = useTranslation();
  const [layoutMode, setLayoutMode] = useState<LayoutMode>("grid");

  const headingColor = useColorModeValue("gray.900", "#ffffff");

  const content = (
    <VStack align="start" spacing={6}>
      <Flex w="full" justify="space-between" align="center">
        <Heading size="lg" color={headingColor}>
          {t("builtinTools.title")}
        </Heading>
        <LiquidGlassCard display="inline-flex" p={1} boxShadow="sm">
          <LayoutToggle mode={layoutMode} onChange={setLayoutMode} />
        </LiquidGlassCard>
      </Flex>
      {layoutMode === "grid" ? (
        <ViewGrid tools={tools} />
      ) : (
        <ViewList tools={tools} />
      )}
    </VStack>
  );

  return (
    <Box pt={8}>
      {content}
    </Box>
  );
}
