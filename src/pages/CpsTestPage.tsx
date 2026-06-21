import {
  Box,
  Heading,
  VStack,
  Text,
  useColorModeValue,
  Button,
  HStack,
  Flex,
  SimpleGrid,
  useBreakpointValue,
} from "@chakra-ui/react";
import { ArrowLeft, Trophy, Timer, MousePointer2, Zap } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useState, useRef, useEffect, useCallback } from "react";
import { LiquidGlassButton } from "@/components/special/liquid-glass-button";
import { motion } from "framer-motion";

type GameState = "idle" | "playing" | "finished";

const TIME_OPTIONS = [5, 10, 30, 60];

export default function CpsTestPage() {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const navigate = useNavigate();

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const cardBg = useColorModeValue("white", "#111111");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const optionBg = useColorModeValue("rgba(152,221,208,0.1)", "rgba(152,221,208,0.15)");
  const clickAreaBg = useColorModeValue("gray.100", "rgba(255,255,255,0.03)");

  const [gameState, setGameState] = useState<GameState>("idle");
  const [selectedTime, setSelectedTime] = useState<number>(10);
  const [timeLeft, setTimeLeft] = useState<number>(0);
  const [clicks, setClicks] = useState(0);
  const [bestCps, setBestCps] = useState<number | null>(null);
  const [burstClicks, setBurstClicks] = useState<{ id: number; x: number; y: number }[]>([]);

  const timerRef = useRef<NodeJS.Timeout | null>(null);
  const clicksRef = useRef(0);
  const burstIdRef = useRef(0);

  const isSmall = useBreakpointValue({ base: true, md: false });

  const startGame = () => {
    setGameState("playing");
    setTimeLeft(selectedTime);
    setClicks(0);
    clicksRef.current = 0;
    setBurstClicks([]);
  };

  const endGame = useCallback(() => {
    if (timerRef.current) {
      clearInterval(timerRef.current);
    }
    setGameState("finished");
    const finalClicks = clicksRef.current;
    setClicks(finalClicks);
    const cps = finalClicks / selectedTime;
    if (bestCps === null || cps > bestCps) {
      setBestCps(cps);
    }
  }, [selectedTime, bestCps]);

  const handleClick = (e: React.MouseEvent) => {
    if (gameState !== "playing") return;

    clicksRef.current += 1;
    setClicks(clicksRef.current);

    const rect = e.currentTarget.getBoundingClientRect();
    const x = ((e.clientX - rect.left) / rect.width) * 100;
    const y = ((e.clientY - rect.top) / rect.height) * 100;
    const id = burstIdRef.current++;
    setBurstClicks((prev) => [...prev.slice(-19), { id, x, y }]);

    setTimeout(() => {
      setBurstClicks((prev) => prev.filter((b) => b.id !== id));
    }, 400);
  };

  useEffect(() => {
    if (gameState === "playing" && timeLeft > 0) {
      timerRef.current = setInterval(() => {
        setTimeLeft((prev) => {
          if (prev <= 1) {
            endGame();
            return 0;
          }
          return prev - 1;
        });
      }, 1000);
    }

    return () => {
      if (timerRef.current) {
        clearInterval(timerRef.current);
      }
    };
  }, [gameState, timeLeft, endGame]);

  const content = (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.4 }}
    >
      <VStack align="stretch" spacing={5}>
        <HStack justifyContent="space-between" alignItems="center">
          <Button
            variant="ghost"
            leftIcon={<ArrowLeft size={18} />}
            onClick={() => navigate("/tests")}
            color={headingColor}
          >
            {t("tests.back") || "返回"}
          </Button>
          <Heading size="lg" color={headingColor}>
            {t("tests.cpsTitle") || "手速测试"}
          </Heading>
          <Box w="100px" />
        </HStack>

        {gameState === "idle" && (
          <>
            <Box w="full">
              <Text fontWeight="600" color={headingColor} fontSize="md" mb={3}>
                {t("tests.cpsSelectTime") || "选择时间"}
              </Text>
              <SimpleGrid columns={4} spacing={3} w="full">
                {TIME_OPTIONS.map((time) => {
                  const isSelected = selectedTime === time;
                  return (
                    <Box
                      key={time}
                      w="full"
                      py={isSmall ? 3 : 4}
                      px={2}
                      borderRadius="xl"
                      bg={isSelected ? "rgba(152,221,208,0.2)" : optionBg}
                      border="2px solid"
                      borderColor={isSelected ? "#98DDD0" : "transparent"}
                      cursor="pointer"
                      transition="all 0.2s cubic-bezier(0.4, 0, 0.2, 1)"
                      _hover={{
                        borderColor: "#98DDD0",
                        bg: useColorModeValue(
                          "rgba(152,221,208,0.2)",
                          "rgba(152,221,208,0.25)"
                        ),
                      }}
                      onClick={() => setSelectedTime(time)}
                    >
                      <VStack justify="center" align="center" spacing={0}>
                        <Text color={headingColor} fontSize="xl" fontWeight="700">
                          {time}
                        </Text>
                        <Text color={subTextColor} fontSize="xs">
                          {t("tests.cpsSeconds") || "秒"}
                        </Text>
                      </VStack>
                    </Box>
                  );
                })}
              </SimpleGrid>
            </Box>

            <Flex w="full" justify="center">
              <motion.div
                initial={{ opacity: 0, scale: 0.9 }}
                animate={{ opacity: 1, scale: 1 }}
                transition={{ delay: 0.15, duration: 0.3 }}
              >
                <LiquidGlassButton
                  onClick={startGame}
                  colorScheme="teal"
                  size="lg"
                  leftIcon={<Zap size={20} />}
                >
                  {t("tests.cpsStart") || "开始测试"}
                </LiquidGlassButton>
              </motion.div>
            </Flex>
          </>
        )}

        {gameState !== "idle" && (
          <Flex justifyContent="center" gap={3} flexWrap="wrap">
            <LiquidGlassCard p={3} minW="120px" textAlign="center">
              <VStack spacing={0}>
                <Timer size={20} color="#3B82F6" />
                <Text color={subTextColor} fontSize="xs">
                  {t("tests.aimTime") || "时间"}
                </Text>
                <Text color={headingColor} fontSize="xl" fontWeight="bold">
                  {timeLeft}s
                </Text>
              </VStack>
            </LiquidGlassCard>

            <LiquidGlassCard p={3} minW="120px" textAlign="center">
              <VStack spacing={0}>
                <MousePointer2 size={20} color="#10B981" />
                <Text color={subTextColor} fontSize="xs">
                  {t("tests.cpsClicks") || "点击"}
                </Text>
                <Text color={headingColor} fontSize="xl" fontWeight="bold">
                  {clicks}
                </Text>
              </VStack>
            </LiquidGlassCard>

            {gameState === "finished" && (
              <>
                <LiquidGlassCard p={3} minW="120px" textAlign="center">
                  <VStack spacing={0}>
                    <Zap size={20} color="#F59E0B" />
                    <Text color={subTextColor} fontSize="xs">
                      {t("tests.cpsResult") || "CPS"}
                    </Text>
                    <Text color={headingColor} fontSize="xl" fontWeight="bold">
                      {(clicks / selectedTime).toFixed(1)}
                    </Text>
                  </VStack>
                </LiquidGlassCard>

                {bestCps !== null && (
                  <LiquidGlassCard p={3} minW="120px" textAlign="center">
                    <VStack spacing={0}>
                      <Trophy size={20} color="#8B5CF6" />
                      <Text color={subTextColor} fontSize="xs">
                        {t("tests.aimBest") || "最佳"}
                      </Text>
                      <Text color={headingColor} fontSize="xl" fontWeight="bold">
                        {bestCps.toFixed(1)}
                      </Text>
                    </VStack>
                  </LiquidGlassCard>
                )}
              </>
            )}
          </Flex>
        )}

        <Box
          w="full"
          h={isSmall ? "280px" : "360px"}
          borderRadius="2xl"
          bg={clickAreaBg}
          position="relative"
          overflow="hidden"
          border="1px solid"
          borderColor={cardBorder}
          cursor={gameState === "playing" ? "pointer" : "default"}
          onClick={handleClick}
          userSelect="none"
        >
          {gameState === "idle" && (
            <Flex
              w="full"
              h="full"
              alignItems="center"
              justifyContent="center"
              flexDirection="column"
              gap={3}
              p={4}
            >
              <Text color={subTextColor} fontSize="lg" textAlign="center">
                {t("tests.cpsInstruction") || "选择时间后点击下方按钮开始测试"}
              </Text>
              <Text color={subTextColor} fontSize="sm" textAlign="center">
                {t("tests.cpsInstruction2") || "在规定时间内快速点击，测试你的手速！"}
              </Text>
            </Flex>
          )}

          {gameState === "playing" && (
            <Flex
              w="full"
              h="full"
              alignItems="center"
              justifyContent="center"
              flexDirection="column"
              p={4}
            >
              <Text
                color={headingColor}
                fontSize={isSmall ? "3xl" : "5xl"}
                fontWeight="bold"
                pointerEvents="none"
              >
                {t("tests.cpsClickHere") || "点击这里！"}
              </Text>
              <Text color={subTextColor} fontSize="sm" mt={2} pointerEvents="none">
                {t("tests.cpsClickCount", { clicks })}
              </Text>
            </Flex>
          )}

          {gameState === "playing" &&
            burstClicks.map((burst) => (
              <motion.div
                key={burst.id}
                initial={{ opacity: 1, scale: 1, y: 0 }}
                animate={{ opacity: 0, scale: 1.5, y: -20 }}
                transition={{ duration: 0.35 }}
                style={{
                  position: "absolute",
                  left: `${burst.x}%`,
                  top: `${burst.y}%`,
                  pointerEvents: "none",
                  transform: "translate(-50%, -50%)",
                }}
              >
                <Box
                  w="24px"
                  h="24px"
                  borderRadius="full"
                  bg="rgba(152,221,208,0.6)"
                  border="2px solid #98DDD0"
                />
              </motion.div>
            ))}

          {gameState === "finished" && (
            <Flex
              w="full"
              h="full"
              alignItems="center"
              justifyContent="center"
              flexDirection="column"
              gap={4}
              p={4}
            >
              <motion.div
                initial={{ opacity: 0, scale: 0.8 }}
                animate={{ opacity: 1, scale: 1 }}
                transition={{ type: "spring", delay: 0.1, duration: 0.5 }}
              >
                <VStack spacing={1} textAlign="center">
                  <Text color={subTextColor} fontSize="sm">
                    {t("tests.cpsResult") || "每秒点击次数"}
                  </Text>
                  <Text color={headingColor} fontSize="5xl" fontWeight="bold">
                    {(clicks / selectedTime).toFixed(1)}
                  </Text>
                  <Text color={subTextColor} fontSize="sm">
                    {t("tests.cpsTotal", { clicks, time: selectedTime })}
                  </Text>
                </VStack>
              </motion.div>
              <motion.div
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.2, duration: 0.3 }}
              >
                <LiquidGlassButton onClick={startGame} colorScheme="teal" size="lg">
                  {t("tests.cpsAgain") || "再来一次"}
                </LiquidGlassButton>
              </motion.div>
            </Flex>
          )}
        </Box>
      </VStack>
    </motion.div>
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
