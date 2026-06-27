import {
  Box,
  Heading,
  VStack,
  Text,
  HStack,
  SimpleGrid,
  useColorModeValue,
  Button,
  useToast,
  Spinner,
  Alert,
  AlertIcon,
  AlertTitle,
  AlertDescription,
  Code,
} from "@chakra-ui/react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import { hexToRgba } from "@/lib/color-utils";
import { ArrowLeft, ShieldCheck, ShieldX, RefreshCw, Play, Terminal } from "lucide-react";
import { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface ActivationStatus {
  name: string;
  description: string;
  license_status: number;
  license_status_text: string;
  partial_product_key: string;
  is_activated: boolean;
}

interface ActivationMethod {
  id: string;
  icon: string;
}

const methods: ActivationMethod[] = [
  { id: "hwid", icon: "🔑" },
  { id: "kms", icon: "☁️" },
  { id: "tsforge", icon: "⚡" },
];

export default function ActivationPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const toast = useToast();
  const { liquidGlassEnabled } = useBackground();

  const [status, setStatus] = useState<ActivationStatus | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [selectedMethod, setSelectedMethod] = useState<string>("hwid");
  const [isActivating, setIsActivating] = useState(false);
  const [logLines, setLogLines] = useState<string[]>([]);
  const logEndRef = useRef<HTMLDivElement>(null);

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const cardBg = useColorModeValue("white", "#111111");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const textColor = useColorModeValue("gray.700", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const logBg = useColorModeValue("gray.900", "#0a0a0a");
  const { getActiveColor, getContrastTextColor } = useThemeColor();
  const primaryColor = getActiveColor();
  const contrastText = getContrastTextColor();
  const themeColorHex = primaryColor || "#98DDD0";
  const themeColorRgba = (opacity: number) => hexToRgba(themeColorHex, opacity);
  const optionBg = useColorModeValue(themeColorRgba(0.1), themeColorRgba(0.15));

  useEffect(() => {
    loadStatus();
  }, []);

  useEffect(() => {
    logEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [logLines]);

  const loadStatus = async () => {
    setIsLoading(true);
    try {
      const result: ActivationStatus = await invoke("check_windows_activation");
      setStatus(result);
    } catch (error) {
      console.error("Failed to check activation status:", error);
    } finally {
      setIsLoading(false);
    }
  };

  const handleActivate = async () => {
    setLogLines([]);
    setIsActivating(true);

    const unlisten = await listen<string>("activation-output", (event) => {
      setLogLines((prev) => [...prev, event.payload]);
    });

    try {
      await invoke("run_windows_activation", { method: selectedMethod });
      toast({
        title: t("activation.activationSuccess"),
        status: "success",
        duration: 3000,
        isClosable: true,
      });
      await loadStatus();
    } catch (error) {
      toast({
        title: t("activation.activationFailed"),
        description: String(error),
        status: "error",
        duration: 5000,
        isClosable: true,
      });
    } finally {
      unlisten();
      setIsActivating(false);
    }
  };

  function MethodCard({ method }: { method: ActivationMethod }) {
    const isSelected = selectedMethod === method.id;
    const content = (
      <VStack justify="center" align="center" spacing={2} py={6} px={3}>
        <Text fontSize="2xl">{method.icon}</Text>
        <Box
          w="14px" h="14px" borderRadius="full"
          border="2px solid"
          borderColor={isSelected ? themeColorHex : subTextColor}
          bg={isSelected ? themeColorHex : "transparent"}
          display="flex" alignItems="center" justifyContent="center"
        >
          {isSelected && <Box w="4px" h="4px" borderRadius="full" bg="#1a1a1a" />}
        </Box>
        <Text color={headingColor} fontSize="sm" fontWeight="bold" textAlign="center">
          {t(`activation.methods.${method.id}.title`)}
        </Text>
        <Text color={subTextColor} fontSize="xs" textAlign="center" noOfLines={2}>
          {t(`activation.methods.${method.id}.desc`)}
        </Text>
      </VStack>
    );

    const props = {
      cursor: "pointer" as const,
      border: isSelected ? `2px solid ${themeColorHex}` : ("2px solid transparent" as string),
      onClick: () => setSelectedMethod(method.id),
    };

    if (liquidGlassEnabled) {
      return <LiquidGlassCard p={0} {...props}>{content}</LiquidGlassCard>;
    }

    return (
      <Box
        w="full" borderRadius="xl"
        bg={isSelected ? optionBg : cardBg}
        border="2px solid"
        borderColor={isSelected ? themeColorHex : cardBorder}
        transition="all 0.2s cubic-bezier(0.4, 0, 0.2, 1)"
        _hover={{ borderColor: themeColorHex }}
        {...props}
      >
        {content}
      </Box>
    );
  }

  const content = (
    <VStack align="start" spacing={6}>
      <HStack spacing={3}>
        <Box
          as="button"
          display="flex" alignItems="center" justifyContent="center"
          w={9} h={9} borderRadius="lg"
          _hover={{ bg: useColorModeValue("gray.100", "rgba(255,255,255,0.08)") }}
          onClick={() => navigate("/builtin-tools")}
          color={headingColor}
        >
          <ArrowLeft size={20} />
        </Box>
        <Heading size="lg" color={headingColor}>
          {t("activation.pageTitle")}
        </Heading>
      </HStack>

      <Box w="full">
        <Heading as="h3" fontSize="md" fontWeight="bold" color={headingColor} mb={3}>
          {t("activation.currentStatus")}
        </Heading>
        <Box
          w="full" p={4} borderRadius="xl"
          bg={useColorModeValue("gray.50", "rgba(255,255,255,0.03)")}
          border="1px solid" borderColor={cardBorder}
        >
          {isLoading ? (
            <HStack spacing={3}>
              <Spinner size="sm" color={themeColorHex} />
              <Text color={subTextColor} fontSize="sm">{t("activation.checking")}</Text>
            </HStack>
          ) : status ? (
            <VStack align="start" spacing={2}>
              <HStack spacing={2}>
                {status.is_activated ? (
                  <ShieldCheck size={20} color="green" />
                ) : (
                  <ShieldX size={20} color="red" />
                )}
                <Text color={headingColor} fontWeight="bold" fontSize="sm">
                  {status.is_activated ? t("activation.activated") : t("activation.notActivated")}
                </Text>
              </HStack>
              {status.name && <Text color={textColor} fontSize="sm">{status.name}</Text>}
              {status.description && <Text color={subTextColor} fontSize="xs">{status.description}</Text>}
              <Text color={subTextColor} fontSize="xs">{t("activation.licenseStatus")}: {status.license_status_text} ({status.license_status})</Text>
              {status.partial_product_key && (
                <Text color={subTextColor} fontSize="xs">KEY: {status.partial_product_key}</Text>
              )}
            </VStack>
          ) : (
            <Text color={subTextColor} fontSize="sm">{t("activation.unableToCheck")}</Text>
          )}
          <Button
            size="xs" mt={2}
            leftIcon={<RefreshCw size={12} />}
            onClick={loadStatus}
            variant="ghost"
            color={themeColorHex}
            isLoading={isLoading}
          >
            {t("activation.refresh")}
          </Button>
        </Box>
      </Box>

      <Box w="full">
        <Heading as="h3" fontSize="md" fontWeight="bold" color={headingColor} mb={3}>
          {t("activation.selectMethod")}
        </Heading>
        <SimpleGrid columns={{ base: 1, md: 3 }} spacing={3}>
          {methods.map((m) => <MethodCard key={m.id} method={m} />)}
        </SimpleGrid>
      </Box>

      <HStack spacing={3} w="full" justify="center">
        <Button
          size="lg"
          onClick={handleActivate}
          isLoading={isActivating}
          loadingText={t("activation.activating")}
          leftIcon={<Play size={18} />}
          bg={themeColorHex}
          color={contrastText}
          _hover={{ opacity: 0.9 }}
          _active={{ transform: "scale(0.97)" }}
          px={8}
        >
          {t("activation.startActivation")}
        </Button>
      </HStack>

      <Box w="full">
        <HStack spacing={2} mb={2}>
          <Terminal size={16} color={subTextColor} />
          <Text color={subTextColor} fontSize="sm" fontWeight="medium">
            {t("activation.outputLog")}
          </Text>
        </HStack>
        <Box
          w="full"
          h="200px"
          bg={logBg}
          borderRadius="lg"
          p={3}
          overflowY="auto"
          fontFamily="monospace"
          fontSize="xs"
          color="#e0e0e0"
          border="1px solid"
          borderColor={cardBorder}
        >
          {logLines.length === 0 ? (
            <Text color="#666" fontStyle="italic">
              {t("activation.waitingForOutput")}
            </Text>
          ) : (
            logLines.map((line, i) => (
              <Text key={i} noOfLines={0} wordBreak="break-all">
                {line}
              </Text>
            ))
          )}
          <div ref={logEndRef} />
        </Box>
      </Box>

      <Text color={subTextColor} fontSize="xs">
        * {t("activation.adminRequired")}
      </Text>
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
        borderWidth="1px" borderColor={cardBorder}
        w="full" boxShadow="2xl" overflow="hidden" position="relative" p={6}
      >
        {content}
      </Box>
    </Box>
  );
}
