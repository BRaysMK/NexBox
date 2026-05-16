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
import { ArrowLeft, Trophy, Timer, Target } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useState, useRef, useEffect, useCallback } from "react";
import { LiquidGlassButton } from "@/components/special/liquid-glass-button";
import { motion } from "framer-motion";

type GameState = "idle" | "playing" | "finished";

const GAME_DURATION = 30;
const BALL_BASE_SIZE = 80;
const BALL_MIN_SIZE = 50;
const BALL_MAX_SIZE = 110;
const BREATH_SPEED = 0.002;
const MOVE_SPEED = 2;

interface Ball {
  x: number;
  y: number;
  vx: number;
  vy: number;
  breathPhase: number;
}

export default function FocusTestPage() {
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
  const [bestScore, setBestScore] = useState<number | null>(null);
  const [ball, setBall] = useState<Ball>({ x: 200, y: 200, vx: 1, vy: 1, breathPhase: 0 });
  const [isHolding, setIsHolding] = useState(false);
  const [currentBallSize, setCurrentBallSize] = useState(BALL_BASE_SIZE);

  const gameAreaRef = useRef<HTMLDivElement>(null);
  const timerRef = useRef<NodeJS.Timeout | null>(null);
  const animationRef = useRef<number | null>(null);
  const scoreIntervalRef = useRef<NodeJS.Timeout | null>(null);

  const initBall = useCallback(() => {
    if (!gameAreaRef.current) return;
    const rect = gameAreaRef.current.getBoundingClientRect();
    const padding = BALL_MAX_SIZE;
    setBall({
      x: padding + Math.random() * (rect.width - padding * 2),
      y: padding + Math.random() * (rect.height - padding * 2),
      vx: (Math.random() - 0.5) * MOVE_SPEED * 2,
      vy: (Math.random() - 0.5) * MOVE_SPEED * 2,
      breathPhase: 0,
    });
  }, []);

  const startGame = () => {
    setGameState("playing");
    setScore(0);
    setTimeLeft(GAME_DURATION);
    setIsHolding(false);
    initBall();
  };

  const endGame = () => {
    if (timerRef.current) clearInterval(timerRef.current);
    if (animationRef.current) cancelAnimationFrame(animationRef.current);
    if (scoreIntervalRef.current) clearInterval(scoreIntervalRef.current);
    setGameState("finished");
    if (bestScore === null || score > bestScore) {
      setBestScore(score);
    }
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
    if (gameState !== "playing") return;

    const updateBall = () => {
      if (!gameAreaRef.current) return;

      const rect = gameAreaRef.current.getBoundingClientRect();
      const padding = BALL_MAX_SIZE / 2;

      setBall((prev) => {
        let newX = prev.x + prev.vx;
        let newY = prev.y + prev.vy;
        let newVx = prev.vx;
        let newVy = prev.vy;

        if (newX <= padding || newX >= rect.width - padding) {
          newVx = -prev.vx + (Math.random() - 0.5) * 0.5;
          newX = Math.max(padding, Math.min(rect.width - padding, newX));
        }
        if (newY <= padding || newY >= rect.height - padding) {
          newVy = -prev.vy + (Math.random() - 0.5) * 0.5;
          newY = Math.max(padding, Math.min(rect.height - padding, newY));
        }

        const speed = Math.sqrt(newVx * newVx + newVy * newVy);
        if (speed > MOVE_SPEED * 2) {
          newVx = (newVx / speed) * MOVE_SPEED * 2;
          newVy = (newVy / speed) * MOVE_SPEED * 2;
        }

        return {
          x: newX,
          y: newY,
          vx: newVx,
          vy: newVy,
          breathPhase: prev.breathPhase + BREATH_SPEED,
        };
      });

      animationRef.current = requestAnimationFrame(updateBall);
    };

    animationRef.current = requestAnimationFrame(updateBall);

    return () => {
      if (animationRef.current) cancelAnimationFrame(animationRef.current);
    };
  }, [gameState]);

  useEffect(() => {
    const size =
      BALL_BASE_SIZE +
      Math.sin(ball.breathPhase) * ((BALL_MAX_SIZE - BALL_MIN_SIZE) / 2);
    setCurrentBallSize(size);
  }, [ball.breathPhase]);

  useEffect(() => {
    if (gameState === "playing" && isHolding) {
      scoreIntervalRef.current = setInterval(() => {
        setScore((prev) => prev + 1);
      }, 100);
    } else {
      if (scoreIntervalRef.current) clearInterval(scoreIntervalRef.current);
    }

    return () => {
      if (scoreIntervalRef.current) clearInterval(scoreIntervalRef.current);
    };
  }, [gameState, isHolding]);

  const handleMouseDown = (e: React.MouseEvent) => {
    if (gameState !== "playing") return;
    if (!gameAreaRef.current) return;

    const rect = gameAreaRef.current.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;

    const distance = Math.sqrt(
      Math.pow(mouseX - ball.x, 2) + Math.pow(mouseY - ball.y, 2)
    );

    if (distance <= currentBallSize / 2) {
      setIsHolding(true);
    }
  };

  const handleMouseUp = () => {
    setIsHolding(false);
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    if (gameState !== "playing" || !isHolding) return;
    if (!gameAreaRef.current) return;

    const rect = gameAreaRef.current.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;

    const distance = Math.sqrt(
      Math.pow(mouseX - ball.x, 2) + Math.pow(mouseY - ball.y, 2)
    );

    if (distance > currentBallSize / 2) {
      setIsHolding(false);
    }
  };

  const handleMouseLeave = () => {
    setIsHolding(false);
  };

  const exitGame = () => {
    if (timerRef.current) clearInterval(timerRef.current);
    if (animationRef.current) cancelAnimationFrame(animationRef.current);
    if (scoreIntervalRef.current) clearInterval(scoreIntervalRef.current);
    setGameState("idle");
    setIsHolding(false);
  };

  useEffect(() => {
    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
      if (animationRef.current) cancelAnimationFrame(animationRef.current);
      if (scoreIntervalRef.current) clearInterval(scoreIntervalRef.current);
    };
  }, []);

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
            {t("tests.focusTitle") || "专注力测试"}
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
                  <Timer size={24} color="#8B5CF6" />
                  <Text color={subTextColor} fontSize="sm">
                    {t("tests.focusTime") || "时间"}
                  </Text>
                  <Text color={headingColor} fontSize="xl" fontWeight="bold">
                    {timeLeft}s
                  </Text>
                </VStack>
              </LiquidGlassCard>

              <LiquidGlassCard p={4} minW="150px" textAlign="center">
                <VStack spacing={1}>
                  <Target size={24} color="#EC4899" />
                  <Text color={subTextColor} fontSize="sm">
                    {t("tests.focusScore") || "分数"}
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
                      {t("tests.focusBest") || "最佳"}
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
            cursor="crosshair"
            onMouseDown={handleMouseDown}
            onMouseUp={handleMouseUp}
            onMouseMove={handleMouseMove}
            onMouseLeave={handleMouseLeave}
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
                  <VStack spacing={4} textAlign="center" maxW="400px">
                    <Text color={subTextColor} fontSize="lg">
                      {t("tests.focusInstruction1") || "按住鼠标追踪移动的能量球"}
                    </Text>
                    <Text color={subTextColor} fontSize="md">
                      {t("tests.focusInstruction2") || "保持光标在球体内获取分数，球体会移动并呼吸缩放"}
                    </Text>
                  </VStack>
                </motion.div>
                <motion.div
                  initial={{ opacity: 0, scale: 0.9 }}
                  animate={{ opacity: 1, scale: 1 }}
                  transition={{ delay: 0.2, duration: 0.3 }}
                >
                  <LiquidGlassButton onClick={startGame} colorScheme="purple" size="lg">
                    {t("tests.focusStart") || "开始游戏"}
                  </LiquidGlassButton>
                </motion.div>
              </Flex>
            )}

            {gameState === "playing" && (
              <>
                <Box
                  position="absolute"
                  left={ball.x - currentBallSize / 2}
                  top={ball.y - currentBallSize / 2}
                  w={`${currentBallSize}px`}
                  h={`${currentBallSize}px`}
                  borderRadius="full"
                  bg={
                    isHolding
                      ? "linear-gradient(135deg, #EC4899 0%, #8B5CF6 100%)"
                      : "linear-gradient(135deg, #8B5CF6 0%, #6366F1 100%)"
                  }
                  boxShadow={
                    isHolding
                      ? "0 0 40px rgba(236, 72, 153, 0.6), 0 0 80px rgba(139, 92, 246, 0.4)"
                      : "0 0 30px rgba(139, 92, 246, 0.5)"
                  }
                  transition="box-shadow 0.15s ease, background 0.15s ease"
                  display="flex"
                  alignItems="center"
                  justifyContent="center"
                  pointerEvents="none"
                >
                  <Box
                    w="35%"
                    h="35%"
                    borderRadius="full"
                    bg="rgba(255, 255, 255, 0.4)"
                    transform="translate(-20%, -20%)"
                  />
                </Box>

                <Box
                  position="absolute"
                  bottom={4}
                  left={4}
                  px={3}
                  py={1}
                  borderRadius="md"
                  bg={isHolding ? "rgba(236, 72, 153, 0.2)" : "rgba(139, 92, 246, 0.2)"}
                >
                  <Text color={isHolding ? "#EC4899" : "#8B5CF6"} fontSize="sm" fontWeight="medium">
                    {isHolding ? (t("tests.focusTracking") || "追踪中...") : (t("tests.focusHoldMouse") || "按住鼠标追踪")}
                  </Text>
                </Box>
              </>
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
                      {t("tests.focusTotal") || "分！"}
                    </Text>
                  </VStack>
                </motion.div>
                <motion.div
                  initial={{ opacity: 0, y: 20 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: 0.2, duration: 0.3 }}
                >
                  <LiquidGlassButton onClick={startGame} colorScheme="purple" size="lg">
                    {t("tests.focusPlayAgain") || "再玩一次"}
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
