import {
  Box,
  Heading,
  VStack,
  Text,
  HStack,
  useColorModeValue,
  Button,
  Radio,
  RadioGroup,
  Card,
  CardBody,
  Badge,
  useToast,
  Alert,
  AlertIcon,
  AlertDescription,
} from "@chakra-ui/react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { ArrowLeft, AlertTriangle, Cpu } from "lucide-react";
import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";

interface MemoryLimitOption {
  id: string;
  label: string;
  limit_gb: number;
  min_physical_gb: number;
}

interface MemoryLimitStatus {
  physical_memory_gb: number;
  physical_memory_mb: number;
  current_limit_mb: number | null;
  available_options: MemoryLimitOption[];
}

interface MemoryLimitResult {
  success: boolean;
  message: string;
  limit_mb: number | null;
  requires_restart: boolean;
}

export default function MemoryLimitPage() {
  const [memoryStatus, setMemoryStatus] = useState<MemoryLimitStatus | null>(null);
  const [selectedLimit, setSelectedLimit] = useState<string>("");
  const [isLoading, setIsLoading] = useState(true);
  const [isApplying, setIsApplying] = useState(false);
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

  useEffect(() => {
    loadMemoryStatus();
  }, []);

  const loadMemoryStatus = async () => {
    try {
      const status: MemoryLimitStatus = await invoke("get_memory_limit_status");
      setMemoryStatus(status);
      
      if (status.current_limit_mb) {
        const currentLimitGB = (status.current_limit_mb / 1024).toFixed(1);
        const matchingOption = status.available_options.find(
          (opt) => opt.limit_gb.toString() === currentLimitGB
        );
        if (matchingOption) {
          setSelectedLimit(matchingOption.id);
        }
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
      setIsLoading(false);
    }
  };

  const applyLimit = async () => {
    if (!selectedLimit) {
      toast({
        title: t("optimization.pleaseSelectOptions"),
        status: "warning",
        duration: 3000,
        isClosable: true,
      });
      return;
    }

    const option = memoryStatus?.available_options.find((opt) => opt.id === selectedLimit);
    if (!option) return;

    setIsApplying(true);
    try {
      const result: MemoryLimitResult = await invoke("set_memory_limit", {
        limitGb: option.limit_gb,
      });

      if (result.success) {
        toast({
          title: t("optimization.memoryLimit.limitApplied"),
          description: `${result.message}\n${t("optimization.memoryLimit.requiresRestart")}`,
          status: "success",
          duration: 7000,
          isClosable: true,
        });
        await loadMemoryStatus();
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
      setIsApplying(false);
    }
  };

  const restoreLimit = async () => {
    setIsApplying(true);
    try {
      const result: MemoryLimitResult = await invoke("restore_memory_limit");

      if (result.success) {
        toast({
          title: t("optimization.memoryLimit.limitRestored"),
          description: `${result.message}\n${t("optimization.memoryLimit.requiresRestart")}`,
          status: "success",
          duration: 7000,
          isClosable: true,
        });
        setSelectedLimit("");
        await loadMemoryStatus();
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
      setIsApplying(false);
    }
  };

  const formatMemory = (mb: number | null) => {
    if (mb === null) return t("optimization.memoryLimit.noLimit");
    return `${(mb / 1024).toFixed(1)} GB`;
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
          {t("optimization.memoryLimit.title")}
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
            {isLoading ? (
              <Text color={subTextColor}>{t("optimization.starting")}</Text>
            ) : (
              <>
                <Box w="full">
                  <Text fontWeight="600" color={textColor} fontSize="md" mb={3}>
                    {t("optimization.memoryLimit.currentStatus")}
                  </Text>
                  <VStack
                    align="start"
                    spacing={2}
                    p={4}
                    borderRadius="xl"
                    bg={optionBg}
                    w="full"
                  >
                    <HStack justify="space-between" w="full">
                      <Text color={subTextColor} fontSize="sm">
                        {t("optimization.memoryLimit.physicalMemory")}:
                      </Text>
                      <Text color={textColor} fontWeight="600" fontSize="sm">
                        {memoryStatus?.physical_memory_gb.toFixed(1)} GB
                      </Text>
                    </HStack>
                    <HStack justify="space-between" w="full">
                      <Text color={subTextColor} fontSize="sm">
                        {t("optimization.memoryLimit.currentLimit")}:
                      </Text>
                      <Badge
                        bg={memoryStatus?.current_limit_mb ? "#FF6B9D" : "#98DDD0"}
                        color="#1a1a1a"
                        fontSize="sm"
                        px={3}
                        py={1}
                        borderRadius="full"
                        fontWeight="600"
                      >
                        {formatMemory(memoryStatus?.current_limit_mb || null)}
                      </Badge>
                    </HStack>
                  </VStack>
                </Box>

                <Box w="full">
                  <Text fontWeight="600" color={textColor} fontSize="md" mb={3}>
                    {t("optimization.memoryLimit.selectLimit")}
                  </Text>
                  <RadioGroup
                    onChange={setSelectedLimit}
                    value={selectedLimit}
                    colorScheme="teal"
                  >
                    <VStack align="start" spacing={3} w="full">
                      {memoryStatus?.available_options.map((option) => (
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
                            bg: useColorModeValue(
                              "rgba(152,221,208,0.2)",
                              "rgba(152,221,208,0.25)"
                            ),
                            transform: "translateX(4px)",
                          }}
                        >
                          <Radio value={option.id}>
                            <Text color={textColor} fontSize="sm" fontWeight="500">
                              {t(`optimization.memoryLimit.options.${option.id}`)}
                            </Text>
                          </Radio>
                        </HStack>
                      ))}
                    </VStack>
                  </RadioGroup>
                </Box>

                <Alert
                  status="warning"
                  borderRadius="xl"
                  bg={useColorModeValue("orange.50", "rgba(255, 165, 0, 0.1)")}
                  borderLeft="4px solid"
                  borderColor="orange.400"
                >
                  <AlertIcon as={AlertTriangle} color="orange.500" />
                  <AlertDescription color={textColor} fontSize="sm">
                    <strong>{t("optimization.memoryLimit.warning")}:</strong>{" "}
                    {t("optimization.memoryLimit.warningText")}
                  </AlertDescription>
                </Alert>

                <HStack spacing={4} w="full" pt={2}>
                  <Button
                    bg="#98DDD0"
                    color="#1a1a1a"
                    size="lg"
                    flex={1}
                    onClick={applyLimit}
                    isLoading={isApplying}
                    loadingText={t("optimization.optimizing")}
                    leftIcon={<Cpu size={20} />}
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
                    {t("optimization.memoryLimit.applyLimit")}
                  </Button>
                  <Button
                    bg="#FF6B9D"
                    color="#ffffff"
                    size="lg"
                    flex={1}
                    onClick={restoreLimit}
                    isLoading={isApplying}
                    loadingText={t("optimization.optimizing")}
                    leftIcon={<AlertTriangle size={20} />}
                    borderRadius="2xl"
                    fontWeight="700"
                    fontSize="md"
                    height="56px"
                    boxShadow="0 4px 20px -5px rgba(255, 107, 157, 0.5)"
                    _hover={{
                      bg: "#FF5A8E",
                      transform: "translateY(-2px)",
                      boxShadow: "0 6px 25px -5px rgba(255, 107, 157, 0.6)",
                    }}
                    _active={{
                      bg: "#FF4A7E",
                      transform: "translateY(0)",
                    }}
                    transition="all 0.2s cubic-bezier(0.4, 0, 0.2, 1)"
                  >
                    {t("optimization.memoryLimit.restoreLimit")}
                  </Button>
                </HStack>
              </>
            )}
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
              {isLoading ? (
                <Text color={subTextColor}>{t("optimization.starting")}</Text>
              ) : (
                <>
                  <Box w="full">
                    <Text fontWeight="600" color={textColor} fontSize="md" mb={3}>
                      {t("optimization.memoryLimit.currentStatus")}
                    </Text>
                    <VStack
                      align="start"
                      spacing={2}
                      p={4}
                      borderRadius="xl"
                      bg={optionBg}
                      w="full"
                    >
                      <HStack justify="space-between" w="full">
                        <Text color={subTextColor} fontSize="sm">
                          {t("optimization.memoryLimit.physicalMemory")}:
                        </Text>
                        <Text color={textColor} fontWeight="600" fontSize="sm">
                          {memoryStatus?.physical_memory_gb.toFixed(1)} GB
                        </Text>
                      </HStack>
                      <HStack justify="space-between" w="full">
                        <Text color={subTextColor} fontSize="sm">
                          {t("optimization.memoryLimit.currentLimit")}:
                        </Text>
                        <Badge
                          bg={memoryStatus?.current_limit_mb ? "#FF6B9D" : "#98DDD0"}
                          color="#1a1a1a"
                          fontSize="sm"
                          px={3}
                          py={1}
                          borderRadius="full"
                          fontWeight="600"
                        >
                          {formatMemory(memoryStatus?.current_limit_mb || null)}
                        </Badge>
                      </HStack>
                    </VStack>
                  </Box>

                  <Box w="full">
                    <Text fontWeight="600" color={textColor} fontSize="md" mb={3}>
                      {t("optimization.memoryLimit.selectLimit")}
                    </Text>
                    <RadioGroup
                      onChange={setSelectedLimit}
                      value={selectedLimit}
                      colorScheme="teal"
                    >
                      <VStack align="start" spacing={3} w="full">
                        {memoryStatus?.available_options.map((option) => (
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
                            <Radio value={option.id}>
                              <Text color={textColor} fontSize="sm" fontWeight="500">
                                {t(`optimization.memoryLimit.options.${option.id}`)}
                              </Text>
                            </Radio>
                          </HStack>
                        ))}
                      </VStack>
                    </RadioGroup>
                  </Box>

                  <Alert
                    status="warning"
                    borderRadius="xl"
                    bg={useColorModeValue("orange.50", "rgba(255, 165, 0, 0.1)")}
                    borderLeft="4px solid"
                    borderColor="orange.400"
                  >
                    <AlertIcon as={AlertTriangle} color="orange.500" />
                    <AlertDescription color={textColor} fontSize="sm">
                      <strong>{t("optimization.memoryLimit.warning")}:</strong>{" "}
                      {t("optimization.memoryLimit.warningText")}
                    </AlertDescription>
                  </Alert>

                  <HStack spacing={4} w="full" pt={2}>
                    <Button
                      bg="#98DDD0"
                      color="#1a1a1a"
                      size="lg"
                      flex={1}
                      onClick={applyLimit}
                      isLoading={isApplying}
                      borderRadius="2xl"
                      fontWeight="700"
                      fontSize="md"
                      height="56px"
                    >
                      {t("optimization.memoryLimit.applyLimit")}
                    </Button>
                    <Button
                      bg="#FF6B9D"
                      color="#ffffff"
                      size="lg"
                      flex={1}
                      onClick={restoreLimit}
                      isLoading={isApplying}
                      borderRadius="2xl"
                      fontWeight="700"
                      fontSize="md"
                      height="56px"
                    >
                      {t("optimization.memoryLimit.restoreLimit")}
                    </Button>
                  </HStack>
                </>
              )}
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
