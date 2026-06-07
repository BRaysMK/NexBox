import {
  Box,
  Heading,
  VStack,
  SimpleGrid,
  Text,
  useColorModeValue,
} from "@chakra-ui/react";
import { Zap, Target, Focus, MousePointerClick, Ban, Grid3X3, MousePointer2 } from "lucide-react";
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
        borderColor={cardBorder}
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

export default function TestsPage() {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const cardBg = useColorModeValue("white", "#111111");
  const cardBorder = useColorModeValue("gray.200", "#333333");

  const tools: ToolItem[] = [
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

  const content = (
    <VStack align="start" spacing={6}>
      <Heading size="lg" color={headingColor}>
        {t("tests.title")}
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
