import { useState } from "react";
import {
  Box,
  Heading,
  VStack,
  Flex,
  useColorModeValue,
} from "@chakra-ui/react";
import {
  Cpu,
  Trash2,
  MemoryStick,
  Gauge,
  Zap,
  HardDrive,
  List,
  Settings2,
  Network,
  MousePointer2,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { ViewGrid } from "@/components/special/view-grid";
import { ViewList } from "@/components/special/view-list";
import { LayoutToggle, type LayoutMode } from "@/components/special/layout-toggle";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import type { ViewItem } from "@/components/special/view-types";

const tools: ViewItem[] = [
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
  {
    id: "startup-manager",
    path: "/optimize/startup-manager",
    icon: List,
    titleKey: "optimization.startupManager.title",
    descriptionKey: "optimization.startupManager.description",
    color: "#805AD5",
  },
  {
    id: "system-optimizer",
    path: "/optimize/system-optimizer",
    icon: Settings2,
    titleKey: "systemOptimizer.pageTitle",
    descriptionKey: "systemOptimizer.pageDesc",
    color: "#667EEA",
  },
  {
    id: "network-optimizer",
    path: "/optimize/network-optimizer",
    icon: Network,
    titleKey: "networkOptimize.pageTitle",
    descriptionKey: "networkOptimize.pageDesc",
    color: "#38A169",
  },
  {
    id: "peripheral-optimize",
    path: "/optimize/peripheral-optimize",
    icon: MousePointer2,
    titleKey: "peripheralOptimize.pageTitle",
    descriptionKey: "peripheralOptimize.pageDesc",
    color: "#E53E3E",
  },
];

export default function OptimizePage() {
  const { t } = useTranslation();
  const [layoutMode, setLayoutMode] = useState<LayoutMode>("grid");

  const headingColor = useColorModeValue("gray.900", "#ffffff");

  const content = (
    <VStack align="start" spacing={6}>
      <Flex w="full" justify="space-between" align="center">
        <Heading size="lg" color={headingColor}>
          {t("optimization.pageTitle")}
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
