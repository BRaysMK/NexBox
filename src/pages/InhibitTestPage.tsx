import {
  Box,
  Heading,
  VStack,
  Text,
  useColorModeValue,
  Button,
  HStack,
  Flex,
} from "@chakra-ui/react";
import { ArrowLeft, Trophy, Timer, Ban } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useState, useRef, useEffect, useCallback } from "react";
import { LiquidGlassButton } from "@/components/special/liquid-glass-button";
import { motion, AnimatePresence } from "framer-motion";

type GameState = "idle" | "playing" | "finished";
type SignalType = "go" | "nogo";
type FeedbackType = "success" | "error" | null;

interface Signal {
  type: SignalType;
  id: number;
}

interface Feedback {
  type: FeedbackType;
  points: number;
  id: number;
}

const GAME_DURATION = 30;
const SIGNAL_DISPLAY_MS = 1200;
const SIGNAL_GAP_MS = 400;
const GO_RATIO = 0.7;

export default function InhibitTestPage() {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const navigate = useNavigate();

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const cardBg = useColorModeValue("white", "#111111");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const subTextColor = useColorModeValue("gray.500", "#888888");

  const [gameState, setGameState] = useState<GameState>("idle");
  const [score, setScore] = useState(0);
  const [errors, setErrors] = useState(0);
  const [misses, setMisses] = useState(0);
  const [timeLeft, setTimeLeft] = useState(GAME_DURATION);
  const [currentSignal, setCurrentSignal] = useState<Signal | null>(null);
  const [bestScore, setBestScore] = useState<number | null>(null);
  const [feedback, setFeedback] = useState<Feedback | null>(null);

  const timerRef = useRef<NodeJS.Timeout | null>(null);
  const signalTimerRef = useRef<NodeJS.Timeout | null>(null);
  const signalStartRef = useRef<number>(0);
  const respondedRef = useRef(false);

  const generateSignal = useCallback(() => {
    const isGo = Math.random() < GO_RATIO;
    return {
      type: (isGo ? "go" : "nogo") as SignalType,
      id: Date.now() + Math.random(),
    };
  }, []);

  const showSignal = useCallback(() => {
    respondedRef.current = false;
    const signal = generateSignal();
    setCurrentSignal(signal);
    signalStartRef.current = Date.now();

    signalTimerRef.current = setTimeout(() => {
      if (!respondedRef.current && signal.type === "go") {
        setMisses((prev) => prev + 1);
      }
      setCurrentSignal(null);
      signalTimerRef.current = setTimeout(() => {
        showSignal();
      }, SIGNAL_GAP_MS);
    }, SIGNAL_DISPLAY_MS);
  }, [generateSignal]);

  const startGame = () => {
    setGameState("playing");
    setScore(0);
    setErrors(0);
    setMisses(0);
    setTimeLeft(GAME_DURATION);
    setCurrentSignal(null);
    showSignal();
  };

  const endGame = () => {
    if (timerRef.current) clearInterval(timerRef.current);
    if (signalTimerRef.current) clearTimeout(signalTimerRef.current);
    setCurrentSignal(null);
    setGameState("finished");
    const final = score - errors * 2;
    if (bestScore === null || final > bestScore) {
      setBestScore(final);
    }
  };

  const handleClick = () => {
    if (gameState !== "playing" || !currentSignal) return;
    if (respondedRef.current) return;

    respondedRef.current = true;
    const rt = Date.now() - signalStartRef.current;

    if (currentSignal.type === "go") {
      let points = 1;
      if (rt < 150) {
        points = 2;
      } else if (rt < 400) {
        points = 3;
      } else if (rt < 700) {
        points = 2;
      }
      setScore((prev) => prev + points);
      setFeedback({ type: "success", points, id: Date.now() });
    } else {
      setErrors((prev) => prev + 1);
      setFeedback({ type: "error", points: -2, id: Date.now() });
    }

    setTimeout(() => setFeedback(null), 600);
  };

  const exitGame = () => {
    if (timerRef.current) clearInterval(timerRef.current);
    if (signalTimerRef.current) clearTimeout(signalTimerRef.current);
    setGameState("idle");
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
      if (timerRef.current) clearInterval(timerRef.current);
    };
  }, [gameState]);

  useEffect(() => {
    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
      if (signalTimerRef.current) clearTimeout(signalTimerRef.current);
    };
  }, []);

  const accentGo = "#22C55E";
  const accentNoGo = "#EF4444";
  const finalScore = score - errors * 2;

  const content = (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.4 }}
    >
      <VStack align="stretch" spacing={6}>
        <HStack justifyContent="space-between" alignItems="center">
          <Button
            variant="ghost"
            leftIcon={<ArrowLeft size={18} />}
            onClick={() => {
              exitGame();
              navigate("/tests");
            }}
            color={headingColor}
          >
            {t("tests.back") || "返回"}
          </Button>
          <Heading size="lg" color={headingColor}>
            {t("tests.inhibitTitle") || "抑制能力测试"}
          </Heading>
          <Box w="100px" />
        </HStack>

        {gameState !== "idle" && (
          <motion.div
            initial={{ opacity: 0, y: -10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.3 }}
          >
            <Flex justifyContent="center" gap={6} flexWrap="wrap">
              <LiquidGlassCard p={4} minW="130px" textAlign="center">
                <VStack spacing={1}>
                  <Timer size={24} color="#3B82F6" />
                  <Text color={subTextColor} fontSize="sm">
                    {t("tests.inhibitTime") || "时间"}
                  </Text>
                  <Text color={headingColor} fontSize="xl" fontWeight="bold">
                    {timeLeft}s
                  </Text>
                </VStack>
              </LiquidGlassCard>

              <LiquidGlassCard p={4} minW="130px" textAlign="center">
                <VStack spacing={1}>
                  <Ban size={24} color="#EF4444" />
                  <Text color={subTextColor} fontSize="sm">
                    {t("tests.inhibitScore") || "得分"}
                  </Text>
                  <Text color={headingColor} fontSize="xl" fontWeight="bold">
                    {gameState === "finished" ? finalScore : score}
                  </Text>
                </VStack>
              </LiquidGlassCard>

              {bestScore !== null && (
                <LiquidGlassCard p={4} minW="130px" textAlign="center">
                  <VStack spacing={1}>
                    <Trophy size={24} color="#F59E0B" />
                    <Text color={subTextColor} fontSize="sm">
                      {t("tests.inhibitBest") || "最佳"}
                    </Text>
                    <Text color={headingColor} fontSize="xl" fontWeight="bold">
                      {bestScore}
                    </Text>
                  </VStack>
                </LiquidGlassCard>
              )}
            </Flex>
          </motion.div>
        )}

        <motion.div
          key={gameState}
          initial={{ opacity: 0, scale: 0.98 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ duration: 0.25 }}
        >
          <Box
            w="full"
            h="400px"
            borderRadius="2xl"
            bg={useColorModeValue("#F3F4F6", "#000000")}
            position="relative"
            overflow="hidden"
            cursor="pointer"
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
                gap={4}
              >
                <motion.div
                  initial={{ opacity: 0, y: 20 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: 0.1, duration: 0.4 }}
                >
                  <VStack spacing={4} textAlign="center" maxW="450px">
                    <HStack spacing={6}>
                      <HStack spacing={2}>
                        <Box w={6} h={6} borderRadius="full" bg={accentGo} />
                        <Text color={headingColor} fontWeight="bold">
                          {t("tests.inhibitGo") || "GO 点击"}
                        </Text>
                      </HStack>
                      <HStack spacing={2}>
                        <Box w={6} h={6} borderRadius="full" bg={accentNoGo} />
                        <Text color={headingColor} fontWeight="bold">
                          {t("tests.inhibitNoGo") || "NO 不动"}
                        </Text>
                      </HStack>
                    </HStack>
                    <Text color={subTextColor} fontSize="md">
                      {t("tests.inhibitInstruction") || "看到绿色快速点击得分，看到红色保持不动。越快越准，分数越高！"}
                    </Text>
                  </VStack>
                </motion.div>
                <motion.div
                  initial={{ opacity: 0, scale: 0.9 }}
                  animate={{ opacity: 1, scale: 1 }}
                  transition={{ delay: 0.2, duration: 0.3 }}
                >
                  <LiquidGlassButton onClick={startGame} colorScheme="green" size="lg">
                    {t("tests.inhibitStart") || "开始游戏"}
                  </LiquidGlassButton>
                </motion.div>
              </Flex>
            )}

            {gameState === "playing" && currentSignal && (
              <Flex
                w="full"
                h="400px"
                alignItems="center"
                justifyContent="center"
              >
                <Box position="relative">
                  <motion.div
                    key={currentSignal.id}
                    initial={{ scale: 0, opacity: 0 }}
                    animate={{ scale: 1, opacity: 1 }}
                    transition={{ type: "spring", stiffness: 400, damping: 20 }}
                  >
                    <Box
                      w="160px"
                      h="160px"
                      borderRadius="full"
                      bg={
                        currentSignal.type === "go"
                          ? accentGo
                          : accentNoGo
                      }
                      boxShadow={
                        currentSignal.type === "go"
                          ? "0 0 60px rgba(34, 197, 94, 0.6), 0 0 120px rgba(34, 197, 94, 0.3)"
                          : "0 0 60px rgba(239, 68, 68, 0.6), 0 0 120px rgba(239, 68, 68, 0.3)"
                      }
                      display="flex"
                      alignItems="center"
                      justifyContent="center"
                      transition="transform 0.1s"
                      _active={{ transform: "scale(0.95)" }}
                    >
                      <Text
                        color="white"
                        fontSize="4xl"
                        fontWeight="extrabold"
                        letterSpacing="wider"
                      >
                        {currentSignal.type === "go" ? "GO" : "NO"}
                      </Text>
                    </Box>
                  </motion.div>

                  <AnimatePresence>
                    {feedback && (
                      <motion.div
                        key={feedback.id}
                        initial={{ opacity: 0, scale: 0.5, y: 0 }}
                        animate={{ opacity: 1, scale: 1, y: -30 }}
                        exit={{ opacity: 0, y: -80 }}
                        transition={{ duration: 0.5 }}
                        style={{
                          position: "absolute",
                          top: "50%",
                          left: "50%",
                          transform: "translate(-50%, -50%)",
                        }}
                      >
                        <Box
                          px={5}
                          py={3}
                          borderRadius="xl"
                          bg={feedback.type === "success" ? "rgba(34, 197, 94, 0.95)" : "rgba(239, 68, 68, 0.95)"}
                          boxShadow={
                            feedback.type === "success"
                              ? "0 0 40px rgba(34, 197, 94, 0.8)"
                              : "0 0 40px rgba(239, 68, 68, 0.8)"
                          }
                        >
                          <Text color="white" fontSize="3xl" fontWeight="bold">
                            {feedback.type === "success" ? `+${feedback.points}` : feedback.points}
                          </Text>
                        </Box>
                      </motion.div>
                    )}
                  </AnimatePresence>
                </Box>
              </Flex>
            )}

            {gameState === "playing" && !currentSignal && (
              <Flex
                w="full"
                h="400px"
                alignItems="center"
                justifyContent="center"
              >
                <Text color={subTextColor} fontSize="xl">
                  {t("tests.inhibitWait") || "准备..."}
                </Text>
              </Flex>
            )}

            {gameState === "finished" && (
              <Flex
                w="full"
                h="full"
                alignItems="center"
                justifyContent="center"
                flexDirection="column"
                gap={4}
              >
                <motion.div
                  initial={{ opacity: 0, scale: 0.8 }}
                  animate={{ opacity: 1, scale: 1 }}
                  transition={{ type: "spring", delay: 0.1, duration: 0.5 }}
                >
                  <VStack spacing={3} textAlign="center">
                    <Text color={headingColor} fontSize="4xl" fontWeight="bold">
                      {finalScore}
                    </Text>
                    <Text color={subTextColor} fontSize="lg">
                      {t("tests.inhibitTotalScore") || "总分"}
                    </Text>
                    <HStack spacing={6} mt={2}>
                      <VStack spacing={0}>
                        <Text color={accentGo} fontSize="lg" fontWeight="bold">
                          {score}
                        </Text>
                        <Text color={subTextColor} fontSize="xs">
                          {t("tests.inhibitGoCount") || "正确GO"}
                        </Text>
                      </VStack>
                      <VStack spacing={0}>
                        <Text color={accentNoGo} fontSize="lg" fontWeight="bold">
                          {errors}
                        </Text>
                        <Text color={subTextColor} fontSize="xs">
                          {t("tests.inhibitNoGoError") || "误点NO"}
                        </Text>
                      </VStack>
                      <VStack spacing={0}>
                        <Text fontSize="lg" fontWeight="bold" color={useColorModeValue("gray.600", "gray.400")}>
                          {misses}
                        </Text>
                        <Text color={subTextColor} fontSize="xs">
                          {t("tests.inhibitMiss") || "漏掉GO"}
                        </Text>
                      </VStack>
                    </HStack>
                  </VStack>
                </motion.div>
                <motion.div
                  initial={{ opacity: 0, y: 20 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: 0.2, duration: 0.3 }}
                >
                  <LiquidGlassButton onClick={startGame} colorScheme="green" size="lg">
                    {t("tests.inhibitPlayAgain") || "再玩一次"}
                  </LiquidGlassButton>
                </motion.div>
              </Flex>
            )}
          </Box>
        </motion.div>
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
