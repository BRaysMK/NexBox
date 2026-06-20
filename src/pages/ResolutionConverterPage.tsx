import {
  Box,
  Flex,
  Heading,
  Text,
  VStack,
  HStack,
  useColorModeValue,
  SimpleGrid,
  IconButton,
  useBreakpointValue,
} from "@chakra-ui/react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import { hexToRgba } from "@/lib/color-utils";
import { Monitor, ArrowLeft } from "lucide-react";
import { useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";

type ResolutionType = "1K" | "1.5K" | "2K" | "2.5K" | "3K" | "4K";

interface ResolutionInfo {
  width: number;
  height: number;
  ratio: string;
  ratioLabel: string;
}

const RESOLUTION_PRESETS: Record<ResolutionType, { width: number; height: number }> = {
  "1K": { width: 1920, height: 1080 },
  "1.5K": { width: 1920, height: 1200 },
  "2K": { width: 2560, height: 1440 },
  "2.5K": { width: 2560, height: 1600 },
  "3K": { width: 3200, height: 1800 },
  "4K": { width: 3840, height: 2160 },
};

const ASPECT_RATIOS = [
  { ratio: "16:9", widthRatio: 16, heightRatio: 9, color: "#4A90E2" },
  { ratio: "4:3", widthRatio: 4, heightRatio: 3, color: "#FF6B9D" },
  { ratio: "16:10", widthRatio: 16, heightRatio: 10, color: "#98DDD0" },
];

function calculateResolution(
  baseHeight: number,
  widthRatio: number,
  heightRatio: number
): number {
  return Math.round((baseHeight * widthRatio) / heightRatio);
}

function ResolutionCard({
  resolution,
  color,
  isActive,
}: {
  resolution: ResolutionInfo;
  color: string;
  isActive: boolean;
}) {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const textColor = useColorModeValue("gray.700", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");

  const aspectRatioBox = useMemo(() => {
    const maxWidth = 120;
    const maxHeight = 80;
    const ratio = resolution.width / resolution.height;
    let boxWidth: number;
    let boxHeight: number;

    if (ratio > maxWidth / maxHeight) {
      boxWidth = maxWidth;
      boxHeight = maxWidth / ratio;
    } else {
      boxHeight = maxHeight;
      boxWidth = maxHeight * ratio;
    }

    return { width: boxWidth, height: boxHeight };
  }, [resolution.width, resolution.height]);

  const cardContent = (
    <VStack spacing={4} align="stretch">
      <HStack justify="space-between">
        <Text
          fontSize="sm"
          fontWeight="600"
          color={color}
          bg={`${color}20`}
          px={3}
          py={1}
          borderRadius="full"
        >
          {resolution.ratioLabel}
        </Text>
        {isActive && (
          <Text fontSize="xs" color={color} fontWeight="500">
            {t("resolutionConverter.standard")}
          </Text>
        )}
      </HStack>

      <VStack spacing={2}>
        <Text
          fontSize="2xl"
          fontWeight="bold"
          color={textColor}
          letterSpacing="tight"
        >
          {resolution.width} × {resolution.height}
        </Text>
        <HStack spacing={4} fontSize="sm" color={subTextColor}>
          <HStack spacing={1}>
            <Text>{t("resolutionConverter.width")}:</Text>
            <Text fontWeight="600" color={textColor}>
              {resolution.width}
            </Text>
          </HStack>
          <HStack spacing={1}>
            <Text>{t("resolutionConverter.height")}:</Text>
            <Text fontWeight="600" color={textColor}>
              {resolution.height}
            </Text>
          </HStack>
        </HStack>
      </VStack>

      <Flex justify="center" pt={2}>
        <Box
          width={`${aspectRatioBox.width}px`}
          height={`${aspectRatioBox.height}px`}
          border="2px solid"
          borderColor={color}
          borderRadius="md"
          position="relative"
          bg={`${color}10`}
        >
          <Box
            position="absolute"
            bottom={-6}
            left="50%"
            transform="translateX(-50%)"
            fontSize="xs"
            color={subTextColor}
            whiteSpace="nowrap"
          >
            {resolution.ratio}
          </Box>
        </Box>
      </Flex>
    </VStack>
  );

  return (
    <LiquidGlassCard
      p={6}
      minH="220px"
      position="relative"
      transition="all 0.2s"
      _hover={{
        transform: "translateY(-2px)",
      }}
    >
      {isActive && (
        <Box
          position="absolute"
          top={0}
          left={0}
          right={0}
          h="3px"
          bg={color}
        />
      )}
      {cardContent}
    </LiquidGlassCard>
  );
}

function ResolutionSelector({
  selected,
  onSelect,
}: {
  selected: ResolutionType;
  onSelect: (type: ResolutionType) => void;
}) {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const { getActiveColor, getContrastTextColor } = useThemeColor();
  const primaryColor = getActiveColor();
  const contrastText = getContrastTextColor();
  const textColor = useColorModeValue("gray.700", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const buttonBg = useColorModeValue("gray.100", "#222222");
  const activeBg = primaryColor;

  const options: { type: ResolutionType; label: string; subLabel: string }[] = [
    { type: "1K", label: t("resolutionConverter.resolution1K"), subLabel: "1920×1080" },
    { type: "1.5K", label: t("resolutionConverter.resolution1_5K"), subLabel: "1920×1200" },
    { type: "2K", label: t("resolutionConverter.resolution2K"), subLabel: "2560×1440" },
    { type: "2.5K", label: t("resolutionConverter.resolution2_5K"), subLabel: "2560×1600" },
    { type: "3K", label: t("resolutionConverter.resolution3K"), subLabel: "3200×1800" },
    { type: "4K", label: t("resolutionConverter.resolution4K"), subLabel: "3840×2160" },
  ];

  return (
    <SimpleGrid columns={{ base: 2, md: 3, lg: 6 }} spacing={3} w="full">
      {options.map((option) => {
        const isActive = selected === option.type;
        return (
          <LiquidGlassCard
            key={option.type}
            p={4}
            cursor="pointer"
            onClick={() => onSelect(option.type)}
            _hover={{
              transform: "translateY(-2px)",
            }}
            position="relative"
          >
            {isActive && (
              <Box
                position="absolute"
                top={0}
                left={0}
                right={0}
                h="3px"
                bg={contrastText}
                opacity={0.3}
              />
            )}
            <VStack spacing={1}>
              <Monitor size={24} color={isActive ? activeBg : undefined} />
              <Text fontSize="md" fontWeight="600" color={isActive ? activeBg : textColor}>
                {option.label}
              </Text>
              <Text fontSize="xs" color={isActive ? activeBg : subTextColor}>
                {option.subLabel}
              </Text>
            </VStack>
          </LiquidGlassCard>
        );
      })}
    </SimpleGrid>
  );
}

export default function ResolutionConverterPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { liquidGlassEnabled } = useBackground();
  const { getActiveColor } = useThemeColor();
  const primaryColor = getActiveColor();
  const [selectedResolution, setSelectedResolution] = useState<ResolutionType>("1K");

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const textColor = useColorModeValue("gray.700", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const cardBg = useColorModeValue("white", "#111111");
  const cardBorder = useColorModeValue("gray.200", "#333333");

  const resolutions = useMemo(() => {
    const base = RESOLUTION_PRESETS[selectedResolution];
    return ASPECT_RATIOS.map((aspect) => {
      const width = calculateResolution(
        base.height,
        aspect.widthRatio,
        aspect.heightRatio
      );
      return {
        width,
        height: base.height,
        ratio: aspect.ratio,
        ratioLabel: t(`resolutionConverter.ratio${aspect.ratio.replace(":", "")}`),
        color: aspect.color,
        isActive: aspect.ratio === "16:9",
      };
    });
  }, [selectedResolution, t]);

  const content = (
    <VStack align="start" spacing={6}>
      <HStack justify="space-between" w="full">
        <HStack>
          <IconButton
            aria-label={t("builtinTools.back")}
            icon={<ArrowLeft size={20} />}
            variant="ghost"
            onClick={() => navigate("/builtin-tools")}
            color={headingColor}
          />
          <Monitor size={28} color={headingColor} />
          <Heading size="lg" color={headingColor} fontWeight="700">
            {t("resolutionConverter.title")}
          </Heading>
        </HStack>
      </HStack>

      <VStack align="start" spacing={4} w="full">
        <Text color={textColor} fontSize="md" fontWeight="600">
          {t("resolutionConverter.selectResolution")}
        </Text>
        <ResolutionSelector
          selected={selectedResolution}
          onSelect={setSelectedResolution}
        />
      </VStack>

      <VStack align="start" spacing={4} w="full">
        <Text color={textColor} fontSize="md" fontWeight="600">
          {t("resolutionConverter.aspectRatios")}
        </Text>
        <SimpleGrid columns={{ base: 1, md: 3 }} spacing={4} w="full">
          {resolutions.map((res) => (
            <ResolutionCard
              key={res.ratio}
              resolution={res}
              color={res.color}
              isActive={res.isActive}
            />
          ))}
        </SimpleGrid>
      </VStack>

      <Box
        w="full"
        p={4}
        borderRadius="xl"
        bg={hexToRgba(primaryColor, 0.1)}
        border="1px solid"
        borderColor={hexToRgba(primaryColor, 0.3)}
      >
        <Text color={subTextColor} fontSize="xs">
          {t("resolutionConverter.tip")}
        </Text>
      </Box>
    </VStack>
  );

  return (
    <Box pt={8}>
      {liquidGlassEnabled ? (
        <LiquidGlassCard
          w="full"
          boxShadow="2xl"
          overflow="hidden"
          position="relative"
          p={6}
        >
          {content}
        </LiquidGlassCard>
      ) : (
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
      )}
    </Box>
  );
}
