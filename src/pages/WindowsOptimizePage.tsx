import {
  Box,
  Heading,
  VStack,
  Text,
  HStack,
  useColorModeValue,
  Button,
  Checkbox,
  Card,
  CardBody,
  Badge,
  useToast,
} from "@chakra-ui/react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { ArrowLeft, Zap } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";

interface OptimizationOption {
  id: string;
  key: string;
  recommended: boolean;
}

const optimizationOptions: OptimizationOption[] = [
  { id: "temp-file-cleanup", key: "optimization.options.tempFileCleanup", recommended: true },
  { id: "privacy-service-optimize", key: "optimization.options.privacyServiceOptimize", recommended: true },
  { id: "close-wallpaper-engine", key: "optimization.options.closeWallpaperEngine", recommended: true },
  { id: "high-performance-power", key: "optimization.options.highPerformancePower", recommended: true },
  { id: "flush-dns", key: "optimization.options.flushDns", recommended: false },
];

export default function WindowsOptimizePage() {
  const [selectedOptions, setSelectedOptions] = useState<string[]>([]);
  const [isOptimizing, setIsOptimizing] = useState(false);
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const navigate = useNavigate();
  const toast = useToast();

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const cardBg = useColorModeValue("white", "#111111");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const textColor = useColorModeValue("gray.700", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const optionBg = useColorModeValue("rgba(152,221,208,0.1)", "rgba(152,221,208,0.15)");

  const toggleOption = (id: string) => {
    setSelectedOptions((prev) =>
      prev.includes(id) ? prev.filter((item) => item !== id) : [...prev, id]
    );
  };

  const toggleAll = () => {
    if (selectedOptions.length === optimizationOptions.length) {
      setSelectedOptions([]);
    } else {
      setSelectedOptions(optimizationOptions.map((opt) => opt.id));
    }
  };

  const startOptimization = async () => {
    if (selectedOptions.length === 0) {
      toast({
        title: t("optimization.pleaseSelectOptions"),
        status: "warning",
        duration: 3000,
        isClosable: true,
      });
      return;
    }

    setIsOptimizing(true);
    
    try {
      toast({
        title: t("optimization.starting"),
        status: "info",
        duration: 2000,
        isClosable: true,
      });

      const results: string[] = [];

      if (selectedOptions.includes("temp-file-cleanup")) {
        const result: any = await invoke("clean_temp_files");
        results.push(`${t("optimization.results.tempFiles")}: ${result.message || t("optimization.results.tempFilesComplete")}`);
      }

      if (selectedOptions.includes("privacy-service-optimize")) {
        const result: any = await invoke("optimize_privacy_services");
        results.push(`${t("optimization.results.serviceOptimize")}: ${result.message || t("optimization.results.serviceOptimizeComplete")}`);
      }

      if (selectedOptions.includes("close-wallpaper-engine")) {
        const result: any = await invoke("kill_wallpaper_engine");
        results.push(`Wallpaper Engine: ${result.message}`);
      }

      if (selectedOptions.includes("flush-dns")) {
        const result: any = await invoke("flush_dns");
        results.push(`DNS: ${result.message}`);
      }

      if (selectedOptions.includes("high-performance-power")) {
        const result: any = await invoke("set_high_performance_power_plan");
        results.push(`${t("optimization.results.powerPlan")}: ${result.message}`);
      }

      if (results.length === 1) {
        toast({
          title: t("optimization.completed"),
          description: results[0],
          status: "success",
          duration: 5000,
          isClosable: true,
        });
      } else {
        toast({
          title: t("optimization.completed"),
          status: "success",
          duration: 5000,
          isClosable: true,
        });
      }
    } catch (error) {
      toast({
        title: t("optimization.error"),
        description: String(error),
        status: "error",
        duration: 5000,
        isClosable: true,
      });
    } finally {
      setIsOptimizing(false);
    }
  };

  const content = (
    <VStack align="start" spacing={6}>
      <HStack justifyContent="space-between" alignItems="center" w="full">
        <Button
          variant="ghost"
          leftIcon={<ArrowLeft size={18} />}
          onClick={() => navigate("/optimize")}
          color={headingColor}
        >
          {t("tests.back") || "返回"}
        </Button>
        <Heading size="lg" color={headingColor} fontWeight="700">
          {t("optimization.windowsTitle") || "Windows 优化"}
        </Heading>
        <Box w="100px" />
      </HStack>

      {liquidGlassEnabled ? (
        <LiquidGlassCard
          w="full"
          boxShadow="2xl"
          overflow="hidden"
          position="relative"
          p={6}
        >
          <VStack align="start" spacing={5}>
            <HStack justify="space-between" w="full">
              <HStack>
                <Checkbox
                  isChecked={selectedOptions.length === optimizationOptions.length}
                  isIndeterminate={
                    selectedOptions.length > 0 && selectedOptions.length < optimizationOptions.length
                  }
                  onChange={toggleAll}
                  colorScheme="teal"
                  size="lg"
                  iconColor="#1a1a1a"
                >
                  <Text fontWeight="600" color={textColor} fontSize="md">
                    {t("optimization.selectAll")}
                  </Text>
                </Checkbox>
              </HStack>
              <Badge 
                bg="#98DDD0"
                color="#1a1a1a"
                fontSize="xs" 
                px={3} 
                py={1} 
                borderRadius="full"
                fontWeight="600"
              >
                {selectedOptions.length} / {optimizationOptions.length}
              </Badge>
            </HStack>

            <VStack align="start" spacing={3} w="full">
              {optimizationOptions.map((option) => (
                <HStack
                  key={option.id}
                  w="full"
                  justify="space-between"
                  p={3}
                  borderRadius="xl"
                  bg={optionBg}
                  border="1px solid"
                  borderColor="transparent"
                  transition="all 0.2s cubic-bezier(0.4, 0, 0.2, 1)"
                  _hover={{
                    borderColor: "#98DDD0",
                    bg: useColorModeValue("rgba(152,221,208,0.2)", "rgba(152,221,208,0.25)"),
                    transform: "translateX(4px)",
                  }}
                >
                  <Checkbox
                    isChecked={selectedOptions.includes(option.id)}
                    onChange={() => toggleOption(option.id)}
                    colorScheme="teal"
                    size="md"
                    iconColor="#1a1a1a"
                  >
                    <Text color={textColor} fontSize="sm" fontWeight="500">
                      {t(option.key)}
                    </Text>
                  </Checkbox>
                  {option.recommended && (
                    <Badge 
                      bg="#98DDD0"
                      color="#1a1a1a"
                      fontSize="xs" 
                      px={2.5} 
                      py={0.5} 
                      borderRadius="full"
                      fontWeight="600"
                    >
                      {t("optimization.recommended")}
                    </Badge>
                  )}
                </HStack>
              ))}
            </VStack>

            <Box w="full" pt={6}>
              <Text 
                fontSize="xs" 
                color={subTextColor} 
                textAlign="center" 
                mb={5}
                opacity={0.8}
              >
                {t("optimization.tip")}
              </Text>
              <Button
                bg="#98DDD0"
                color="#1a1a1a"
                size="lg"
                w="full"
                maxW="240px"
                mx="auto"
                display="block"
                onClick={startOptimization}
                isLoading={isOptimizing}
                loadingText={t("optimization.optimizing")}
                leftIcon={isOptimizing ? undefined : <Zap size={20} fill="currentColor" />}
                borderRadius="2xl"
                fontWeight="700"
                fontSize="md"
                height="56px"
                boxShadow="0 4px 20px -5px rgba(152, 221, 208, 0.5)"
                _hover={{
                  bg: "#7ED0C2",
                  transform: "translateY(-2px)",
                  boxShadow: "0 6px 25px -5px rgba(152, 221, 208, 0.6)",
                }}
                _active={{
                  bg: "#6BC4B5",
                  transform: "translateY(0)",
                }}
                transition="all 0.2s cubic-bezier(0.4, 0, 0.2, 1)"
              >
                {t("optimization.startButton")}
              </Button>
            </Box>
          </VStack>
        </LiquidGlassCard>
      ) : (
        <Card
          bg={cardBg}
          borderColor={cardBorder}
          borderWidth="1px"
          w="full"
          boxShadow="2xl"
          overflow="hidden"
          position="relative"
        >
          <CardBody p={6}>
            <VStack align="start" spacing={5}>
              <HStack justify="space-between" w="full">
                <HStack>
                  <Checkbox
                    isChecked={selectedOptions.length === optimizationOptions.length}
                    isIndeterminate={
                      selectedOptions.length > 0 && selectedOptions.length < optimizationOptions.length
                    }
                    onChange={toggleAll}
                    colorScheme="teal"
                    size="lg"
                    iconColor="#1a1a1a"
                  >
                    <Text fontWeight="600" color={textColor} fontSize="md">
                      {t("optimization.selectAll")}
                    </Text>
                  </Checkbox>
                </HStack>
                <Badge 
                  bg="#98DDD0"
                  color="#1a1a1a"
                  fontSize="xs" 
                  px={3} 
                  py={1} 
                  borderRadius="full"
                  fontWeight="600"
                >
                  {selectedOptions.length} / {optimizationOptions.length}
                </Badge>
              </HStack>

              <VStack align="start" spacing={3} w="full">
                {optimizationOptions.map((option) => (
                  <HStack
                    key={option.id}
                    w="full"
                    justify="space-between"
                    p={3}
                    borderRadius="xl"
                    bg={optionBg}
                    border="1px solid"
                    borderColor="transparent"
                    transition="all 0.2s cubic-bezier(0.4, 0, 0.2, 1)"
                  >
                    <Checkbox
                      isChecked={selectedOptions.includes(option.id)}
                      onChange={() => toggleOption(option.id)}
                      colorScheme="teal"
                      size="md"
                      iconColor="#1a1a1a"
                    >
                      <Text color={textColor} fontSize="sm" fontWeight="500">
                        {t(option.key)}
                      </Text>
                    </Checkbox>
                    {option.recommended && (
                      <Badge 
                        bg="#98DDD0"
                        color="#1a1a1a"
                        fontSize="xs" 
                        px={2.5} 
                        py={0.5} 
                        borderRadius="full"
                        fontWeight="600"
                      >
                        {t("optimization.recommended")}
                      </Badge>
                    )}
                  </HStack>
                ))}
              </VStack>

              <Box w="full" pt={6}>
                <Button
                  bg="#98DDD0"
                  color="#1a1a1a"
                  size="lg"
                  w="full"
                  maxW="240px"
                  mx="auto"
                  display="block"
                  onClick={startOptimization}
                  isLoading={isOptimizing}
                  borderRadius="2xl"
                  fontWeight="700"
                  fontSize="md"
                  height="56px"
                >
                  {t("optimization.startButton")}
                </Button>
              </Box>
            </VStack>
          </CardBody>
        </Card>
      )}
    </VStack>
  );

  return (
    <Box pt={8}>
      {content}
    </Box>
  );
}
