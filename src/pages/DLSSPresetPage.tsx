import {
  Box,
  Text,
  Heading,
  VStack,
  HStack,
  useColorModeValue,
  Button,
  Badge,
  useToast,
  Grid,
  Divider,
  IconButton,
} from "@chakra-ui/react";
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { Cpu, ArrowLeft, Zap } from "lucide-react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useNavigate } from "react-router-dom";

interface DLSSModelPreset {
  id: string;
  name: string;
  description: string;
  recommended: boolean;
}

interface DLSSApplyResult {
  success: boolean;
  message: string;
  preset: string;
}

const DLSS_MODEL_IDS = ["A", "B", "C", "D", "E", "F", "G", "J", "K", "L", "M"];

const getDLSSPresets = (t: (key: string) => string): DLSSModelPreset[] => [
  { id: "A", name: t("deltaForce.dlssModels.A.name"), description: t("deltaForce.dlssModels.A.description"), recommended: false },
  { id: "B", name: t("deltaForce.dlssModels.B.name"), description: t("deltaForce.dlssModels.B.description"), recommended: false },
  { id: "C", name: t("deltaForce.dlssModels.C.name"), description: t("deltaForce.dlssModels.C.description"), recommended: false },
  { id: "D", name: t("deltaForce.dlssModels.D.name"), description: t("deltaForce.dlssModels.D.description"), recommended: false },
  { id: "E", name: t("deltaForce.dlssModels.E.name"), description: t("deltaForce.dlssModels.E.description"), recommended: false },
  { id: "F", name: t("deltaForce.dlssModels.F.name"), description: t("deltaForce.dlssModels.F.description"), recommended: false },
  { id: "G", name: t("deltaForce.dlssModels.G.name"), description: t("deltaForce.dlssModels.G.description"), recommended: false },
  { id: "J", name: t("deltaForce.dlssModels.J.name"), description: t("deltaForce.dlssModels.J.description"), recommended: false },
  { id: "K", name: t("deltaForce.dlssModels.K.name"), description: t("deltaForce.dlssModels.K.description"), recommended: true },
  { id: "L", name: t("deltaForce.dlssModels.L.name"), description: t("deltaForce.dlssModels.L.description"), recommended: true },
  { id: "M", name: t("deltaForce.dlssModels.M.name"), description: t("deltaForce.dlssModels.M.description"), recommended: true },
];

function SectionCard({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  const { liquidGlassEnabled } = useBackground();
  const cardBg = useColorModeValue("white", "#111111");
  const borderColor = useColorModeValue("gray.200", "#333333");

  if (liquidGlassEnabled) {
    return (
      <LiquidGlassCard p={5}>
        <VStack align="stretch" spacing={4}>
          <Text fontWeight="semibold" fontSize="md" color="white">{title}</Text>
          {children}
        </VStack>
      </LiquidGlassCard>
    );
  }

  return (
    <Box bg={cardBg} borderRadius="xl" p={5} border="1px solid" borderColor={borderColor}>
      <VStack align="stretch" spacing={4}>
        <Text fontWeight="semibold" fontSize="md" color="white">{title}</Text>
        {children}
      </VStack>
    </Box>
  );
}

function DLSSCard() {
  const { t } = useTranslation();
  const toast = useToast();
  const [selectedPreset, setSelectedPreset] = useState("K");
  const [isApplying, setIsApplying] = useState(false);
  const dlssPresets = getDLSSPresets(t);

  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const presetBg = useColorModeValue("gray.50", "#1a1a1a");
  const presetActiveBg = useColorModeValue("teal.500", "teal.500");

  const handleApply = async () => {
    setIsApplying(true);
    try {
      const result = await invoke<DLSSApplyResult>("apply_dlss_model_preset", {
        preset: selectedPreset,
      });
      toast({
        title: result.message,
        status: "success",
        duration: 3000,
        isClosable: true,
      });
    } catch (error) {
      toast({
        title: String(error),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    }
    setIsApplying(false);
  };

  const currentPreset = dlssPresets.find(p => p.id === selectedPreset);

  return (
    <SectionCard title={t("deltaForce.dlssPreset")}>
      <Text fontSize="sm" color={subTextColor} mb={4}>
        {t("deltaForce.dlssPresetDesc")}
      </Text>

      <Grid templateColumns="repeat(4, 1fr)" gap={3} mb={4}>
        {dlssPresets.map(preset => (
          <Box
            key={preset.id}
            onClick={() => setSelectedPreset(preset.id)}
            bg={selectedPreset === preset.id ? presetActiveBg : presetBg}
            color={selectedPreset === preset.id ? "white" : textColor}
            borderRadius="lg"
            p={3}
            textAlign="center"
            cursor="pointer"
            position="relative"
            _hover={{ transform: "scale(1.02)" }}
            transition="all 0.2s"
          >
            <Text fontWeight="bold" fontSize="sm">{preset.name}</Text>
            {preset.recommended && (
              <Badge
                position="absolute"
                top="-8px"
                right="-8px"
                colorScheme="green"
                fontSize="8px"
                px={1}
              >
                {t("deltaForce.recommended")}
              </Badge>
            )}
          </Box>
        ))}
      </Grid>

      <HStack justify="space-between" mb={4}>
        <Text fontSize="sm" color={subTextColor}>
          {t("deltaForce.currentSelection")}: {currentPreset?.name} - {currentPreset?.description}
        </Text>
      </HStack>

      <Divider mb={4} />

      <Button
        onClick={handleApply}
        isLoading={isApplying}
        colorScheme="teal"
        w="full"
        leftIcon={<Cpu size={16} />}
      >
        {t("deltaForce.applyPreset")}
      </Button>

      <Text fontSize="xs" color={subTextColor} mt={2} textAlign="center">
        {t("deltaForce.dlssNote")}
      </Text>
    </SectionCard>
  );
}

export default function DLSSPresetPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const headingColor = useColorModeValue("gray.900", "#ffffff");

  return (
    <Box pt={8} pb={8}>
      <HStack spacing={3} mb={6}>
        <IconButton
          aria-label="返回"
          icon={<ArrowLeft size={20} />}
          variant="ghost"
          onClick={() => navigate("/builtin-tools")}
          color={headingColor}
        />
        <Zap size={28} color={headingColor} />
        <Heading size="lg" color={headingColor} fontWeight="700">
          {t("dlssPreset.title")}
        </Heading>
      </HStack>

      <VStack align="stretch" spacing={5}>
        <DLSSCard />
      </VStack>
    </Box>
  );
}
