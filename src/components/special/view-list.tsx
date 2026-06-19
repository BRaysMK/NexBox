import {
  Box,
  VStack,
  HStack,
  Text,
  useColorModeValue,
} from "@chakra-ui/react";
import { ChevronRight } from "lucide-react";
import { Link } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { LiquidGlassCard } from "./liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import type { ViewItem } from "./view-types";

interface ViewListProps {
  tools: ViewItem[];
}

export function ViewList({ tools }: ViewListProps) {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const listBg = useColorModeValue("white", "#111111");
  const listBorder = useColorModeValue("gray.200", "#333333");
  const hoverBg = useColorModeValue("gray.50", "#1a1a1a");

  const listCardContent = (tool: ViewItem) => {
    const IconComponent = tool.icon;
    return (
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
        <HStack spacing={4} align="center">
          <Box
            w={10}
            h={10}
            borderRadius="lg"
            bg={`${tool.color}20`}
            display="flex"
            alignItems="center"
            justifyContent="center"
            color={tool.color}
            flexShrink={0}
          >
            <IconComponent size={22} />
          </Box>
          <VStack align="start" spacing={0} flex={1} minW={0}>
            <Text
              color={headingColor}
              fontSize="md"
              fontWeight="bold"
              noOfLines={1}
            >
              {t(tool.titleKey)}
            </Text>
            <Text color={subTextColor} fontSize="sm" noOfLines={1}>
              {t(tool.descriptionKey)}
            </Text>
          </VStack>
          <ChevronRight size={18} color={subTextColor} />
        </HStack>
      </>
    );
  };

  return (
    <VStack w="full" spacing={3}>
      {tools.map((tool) => (
        <Link key={tool.id} to={tool.path} style={{ width: "100%" }}>
          {liquidGlassEnabled ? (
            <LiquidGlassCard w="full" cursor="pointer" p={4} position="relative">
              {listCardContent(tool)}
            </LiquidGlassCard>
          ) : (
            <Box
              bg={listBg}
              borderRadius="xl"
              border="1px solid"
              borderColor={listBorder}
              p={4}
              cursor="pointer"
              transition="all 0.2s"
              _hover={{ borderColor: tool.color, bg: hoverBg }}
              position="relative"
            >
              {listCardContent(tool)}
            </Box>
          )}
        </Link>
      ))}
    </VStack>
  );
}
