import { useState } from "react";
import {
  Box,
  Heading,
  VStack,
  Flex,
  useColorModeValue,
} from "@chakra-ui/react";
import {
  Zap,
  Target,
  Focus,
  MousePointerClick,
  Ban,
  Grid3X3,
  MousePointer2,
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
    id: "reaction",
    path: "/tests/reaction",
    icon: Zap,
    titleKey: "tests.reactionTitle",
    descriptionKey: "tests.reactionDesc",
    color: "#F59E0B",
  },
  {
    id: "aim",
    path: "/tests/aim",
    icon: Target,
    titleKey: "tests.aimTitle",
    descriptionKey: "tests.aimDesc",
    color: "#10B981",
  },
  {
    id: "focus",
    path: "/tests/focus",
    icon: Focus,
    titleKey: "tests.focusTitle",
    descriptionKey: "tests.focusDesc",
    color: "#8B5CF6",
  },
  {
    id: "choice",
    path: "/tests/choice",
    icon: MousePointerClick,
    titleKey: "tests.choiceTitle",
    descriptionKey: "tests.choiceDesc",
    color: "#F59E0B",
  },
  {
    id: "inhibit",
    path: "/tests/inhibit",
    icon: Ban,
    titleKey: "tests.inhibitTitle",
    descriptionKey: "tests.inhibitDesc",
    color: "#EF4444",
  },
  {
    id: "schulte",
    path: "/tests/schulte",
    icon: Grid3X3,
    titleKey: "tests.schulteTitle",
    descriptionKey: "tests.schulteDesc",
    color: "#3B82F6",
  },
  {
    id: "cps",
    path: "/tests/cps",
    icon: MousePointer2,
    titleKey: "tests.cpsTitle",
    descriptionKey: "tests.cpsDesc",
    color: "#06b6d4",
  },
];

export default function TestsPage() {
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
          {t("tests.title")}
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
