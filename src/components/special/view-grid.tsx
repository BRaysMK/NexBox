import {
  Box,
  SimpleGrid,
  VStack,
  Text,
  useColorModeValue,
} from "@chakra-ui/react";
import { Link } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { LiquidGlassCard } from "./liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import type { ViewItem } from "./view-types";

interface ViewGridProps {
  tools: ViewItem[];
}

function GridCard({ tool }: { tool: ViewItem }) {
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

export function ViewGrid({ tools }: ViewGridProps) {
  return (
    <SimpleGrid columns={{ base: 1, md: 2, lg: 3 }} spacing={4} w="full">
      {tools.map((tool) => (
        <GridCard key={tool.id} tool={tool} />
      ))}
    </SimpleGrid>
  );
}
