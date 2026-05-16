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
import { ArrowLeft, Trophy, Timer, MousePointer2 } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useState, useRef, useEffect, useCallback } from "react";
import { LiquidGlassButton } from "@/components/special/liquid-glass-button";
import { motion } from "framer-motion";

type GameState = "idle" | "playing" | "finished";

interface Target {
  id: number;
  x: number;
  y: number;
}

const GAME_DURATION = 30;
const TARGET_SIZE = 60;

export default function AimTestPage() {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const navigate = useNavigate();

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const cardBg = useColorModeValue("white", "#111111");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const subTextColor = useColorModeValue("gray.500", "#888888");

  const [gameState, setGameState] = useState<GameState>("idle");
  const [score, setScore] = useState(0);
  const [timeLeft, setTimeLeft] = useState(GAME_DURATION);
  const [target, setTarget] = useState<Target | null>(null);
  const [bestScore, setBestScore] = useState<number | null>(null);
  const gameAreaRef = useRef<HTMLDivElement>(null);
  const timerRef = useRef<NodeJS.Timeout | null>(null);

  const generateTarget = useCallback(() => {
    if (!gameAreaRef.current) return;

    const rect = gameAreaRef.current.getBoundingClientRect();
    const maxX = rect.width - TARGET_SIZE - 20;
    const maxY = rect.height - TARGET_SIZE - 20;

    const newTarget: Target = {
      id: Date.now(),
      x: 10 + Math.random() * maxX,
      y: 10 + Math.random() * maxY,
    };

    setTarget(newTarget);
  }, []);

  const startGame = () => {
    setGameState("playing");
    setScore(0);
    setTimeLeft(GAME_DURATION);
    generateTarget();
  };

  const endGame = () => {
    if (timerRef.current) {
      clearInterval(timerRef.current);
    }
    setGameState("finished");
    setTarget(null);
    if (bestScore === null || score > bestScore) {
      setBestScore(score);
    }
  };

  const handleTargetClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (gameState !== "playing") return;

    setScore((prev) => prev + 1);
    generateTarget();
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
  }, [gameState, timeLeft]);

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
            onClick={() => navigate("/tests")}
            color={headingColor}
          >
            {t("tests.back") || "返回"}
          </Button>
          <Heading size="lg" color={headingColor}>
            {t("tests.aimTitle") || "瞄准测试"}
          </Heading>
          <Box w="100px" />
        </HStack>

        {gameState !== "idle" && (
          <motion.div
            initial={{ opacity: 0, y: -10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.3 }}
          >
            <Flex justifyContent="center" gap={8} flexWrap="wrap">
              <LiquidGlassCard p={4} minW="150px" textAlign="center">
                <VStack spacing={1}>
                  <Timer size={24} color="#3B82F6" />
                  <Text color={subTextColor} fontSize="sm">
                    {t("tests.aimTime") || "时间"}
                  </Text>
                  <Text color={headingColor} fontSize="xl" fontWeight="bold">
                    {timeLeft}s
                  </Text>
                </VStack>
              </LiquidGlassCard>

              <LiquidGlassCard p={4} minW="150px" textAlign="center">
                <VStack spacing={1}>
                  <MousePointer2 size={24} color="#10B981" />
                  <Text color={subTextColor} fontSize="sm">
                    {t("tests.aimScore") || "分数"}
                  </Text>
                  <Text color={headingColor} fontSize="xl" fontWeight="bold">
                    {score}
                  </Text>
                </VStack>
              </LiquidGlassCard>

              {bestScore !== null && (
                <LiquidGlassCard p={4} minW="150px" textAlign="center">
                  <VStack spacing={1}>
                    <Trophy size={24} color="#F59E0B" />
                    <Text color={subTextColor} fontSize="sm">
                      {t("tests.aimBest") || "最佳"}
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
            ref={gameAreaRef}
            w="full"
            h="400px"
            borderRadius="2xl"
            bg={useColorModeValue("#F3F4F6", "#000000")}
            position="relative"
            overflow="hidden"
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
                  <Text color={subTextColor} fontSize="lg" textAlign="center">
                    {t("tests.aimInstruction") || "点击开始，30秒内尽可能点击更多目标！"}
                  </Text>
                </motion.div>
                <motion.div
                  initial={{ opacity: 0, scale: 0.9 }}
                  animate={{ opacity: 1, scale: 1 }}
                  transition={{ delay: 0.2, duration: 0.3 }}
                >
                  <LiquidGlassButton onClick={startGame} colorScheme="green" size="lg">
                    {t("tests.aimStart") || "开始游戏"}
                  </LiquidGlassButton>
                </motion.div>
              </Flex>
            )}

            {gameState === "playing" && target && (
              <motion.div
                key={target.id}
                initial={{ scale: 0, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                exit={{ scale: 0, opacity: 0 }}
                transition={{ type: "spring", stiffness: 400, damping: 20 }}
                style={{ position: 'absolute', left: target.x, top: target.y }}
              >
                <Box
                  as="button"
                  onClick={handleTargetClick}
                  w={`${TARGET_SIZE}px`}
                  h={`${TARGET_SIZE}px`}
                  borderRadius="full"
                  bg="linear-gradient(135deg, #10B981 0%, #059669 100%)"
                  boxShadow="0 0 20px rgba(16, 185, 129, 0.5)"
                  cursor="pointer"
                  transition="transform 0.1s"
                  _hover={{ transform: "scale(1.1)" }}
                  _active={{ transform: "scale(0.95)" }}
                >
                  <Box
                    position="absolute"
                    top="50%"
                    left="50%"
                    transform="translate(-50%, -50%)"
                    w="20px"
                    h="20px"
                    borderRadius="full"
                    bg="white"
                    opacity="0.8"
                  />
                </Box>
              </motion.div>
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
                  <VStack spacing={2} textAlign="center">
                    <Text color={headingColor} fontSize="3xl" fontWeight="bold">
                      {score}
                    </Text>
                    <Text color={subTextColor} fontSize="lg">
                      {t("tests.aimTotal") || "个目标！"}
                    </Text>
                  </VStack>
                </motion.div>
                <motion.div
                  initial={{ opacity: 0, y: 20 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: 0.2, duration: 0.3 }}
                >
                  <LiquidGlassButton onClick={startGame} colorScheme="green" size="lg">
                    {t("tests.aimPlayAgain") || "再玩一次"}
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
