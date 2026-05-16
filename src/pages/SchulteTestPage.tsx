import {
  Box,
  Heading,
  VStack,
  Text,
  useColorModeValue,
  Button,
  HStack,
  Flex,
  Grid,
} from "@chakra-ui/react";
import { ArrowLeft, Trophy, Timer, Grid3X3 } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useState, useRef, useEffect, useCallback } from "react";
import { LiquidGlassButton } from "@/components/special/liquid-glass-button";
import { motion } from "framer-motion";

type GameState = "idle" | "playing" | "finished";

const GRID_SIZE = 5;
const TOTAL_NUMBERS = GRID_SIZE * GRID_SIZE;

function shuffleArray<T>(array: T[]): T[] {
  const shuffled = [...array];
  for (let i = shuffled.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [shuffled[i], shuffled[j]] = [shuffled[j], shuffled[i]];
  }
  return shuffled;
}

function formatTime(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const centiseconds = Math.floor((ms % 1000) / 10);
  return `${seconds}.${centiseconds.toString().padStart(2, "0")}s`;
}

export default function SchulteTestPage() {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const navigate = useNavigate();

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const cardBg = useColorModeValue("white", "#111111");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const cellBg = useColorModeValue("white", "#000000");
  const cellHoverBg = useColorModeValue("gray.50", "#1a1a1a");
  const cellActiveBg = useColorModeValue("#3B82F6", "#2563EB");

  const [gameState, setGameState] = useState<GameState>("idle");
  const [grid, setGrid] = useState<number[]>([]);
  const [currentTarget, setCurrentTarget] = useState(1);
  const [elapsedTime, setElapsedTime] = useState(0);
  const [bestTime, setBestTime] = useState<number | null>(null);
  const [clickedCells, setClickedCells] = useState<Set<number>>(new Set());

  const startTimeRef = useRef<number>(0);
  const timerRef = useRef<NodeJS.Timeout | null>(null);

  const generateGrid = useCallback(() => {
    const numbers = Array.from({ length: TOTAL_NUMBERS }, (_, i) => i + 1);
    setGrid(shuffleArray(numbers));
    setCurrentTarget(1);
    setClickedCells(new Set());
  }, []);

  const startGame = () => {
    generateGrid();
    setGameState("playing");
    setElapsedTime(0);
    startTimeRef.current = Date.now();
  };

  const endGame = () => {
    if (timerRef.current) clearInterval(timerRef.current);
    setGameState("finished");
    if (bestTime === null || elapsedTime < bestTime) {
      setBestTime(elapsedTime);
    }
  };

  const handleCellClick = (number: number, index: number) => {
    if (gameState !== "playing") return;
    if (clickedCells.has(index)) return;

    if (number === currentTarget) {
      setClickedCells((prev) => new Set([...prev, index]));
      if (currentTarget === TOTAL_NUMBERS) {
        endGame();
      } else {
        setCurrentTarget((prev) => prev + 1);
      }
    }
  };

  const exitGame = () => {
    if (timerRef.current) clearInterval(timerRef.current);
    setGameState("idle");
  };

  useEffect(() => {
    if (gameState === "playing") {
      timerRef.current = setInterval(() => {
        setElapsedTime(Date.now() - startTimeRef.current);
      }, 10);
    }

    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
    };
  }, [gameState]);

  useEffect(() => {
    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
    };
  }, []);

  const accentColor = "#3B82F6";

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
            {t("tests.schulteTitle") || "舒尔特方格"}
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
                  <Timer size={24} color={accentColor} />
                  <Text color={subTextColor} fontSize="sm">
                    {t("tests.schulteTime") || "用时"}
                  </Text>
                  <Text color={headingColor} fontSize="xl" fontWeight="bold">
                    {formatTime(elapsedTime)}
                  </Text>
                </VStack>
              </LiquidGlassCard>

              <LiquidGlassCard p={4} minW="150px" textAlign="center">
                <VStack spacing={1}>
                  <Grid3X3 size={24} color="#10B981" />
                  <Text color={subTextColor} fontSize="sm">
                    {t("tests.schulteProgress") || "进度"}
                  </Text>
                  <Text color={headingColor} fontSize="xl" fontWeight="bold">
                    {currentTarget - 1}/{TOTAL_NUMBERS}
                  </Text>
                </VStack>
              </LiquidGlassCard>

              {bestTime !== null && (
                <LiquidGlassCard p={4} minW="150px" textAlign="center">
                  <VStack spacing={1}>
                    <Trophy size={24} color="#F59E0B" />
                    <Text color={subTextColor} fontSize="sm">
                      {t("tests.schulteBest") || "最佳"}
                    </Text>
                    <Text color={headingColor} fontSize="xl" fontWeight="bold">
                      {formatTime(bestTime)}
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
            borderRadius="2xl"
            bg={useColorModeValue("#F3F4F6", "#000000")}
            position="relative"
            overflow="hidden"
            p={{ base: 4, md: 6 }}
            minH="450px"
            userSelect="none"
          >
            {gameState === "idle" && (
              <Flex
                w="full"
                minH="450px"
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
                  <VStack spacing={4} textAlign="center" maxW="500px">
                    <Text color={headingColor} fontSize="lg" fontWeight="bold">
                      5×5 {t("tests.schulteGrid") || "方格"}
                    </Text>
                    <Text color={subTextColor} fontSize="md">
                      {t("tests.schulteInstruction1") || "按顺序(1→2→3→...→25)尽快点击所有数字"}
                    </Text>
                    <Text color={subTextColor} fontSize="sm">
                      {t("tests.schulteInstruction2") || "眼睛注视中心，利用余光搜索，提升视觉广度与专注力"}
                    </Text>
                  </VStack>
                </motion.div>
                <motion.div
                  initial={{ opacity: 0, scale: 0.9 }}
                  animate={{ opacity: 1, scale: 1 }}
                  transition={{ delay: 0.2, duration: 0.3 }}
                >
                  <LiquidGlassButton onClick={startGame} colorScheme="blue" size="lg">
                    {t("tests.schulteStart") || "开始游戏"}
                  </LiquidGlassButton>
                </motion.div>
              </Flex>
            )}

            {gameState === "playing" && (
              <Flex
                w="full"
                direction="column"
                alignItems="center"
                justifyContent="center"
                gap={4}
              >
                <Box
                  w="full"
                  maxW="400px"
                  aspectRatio="1"
                  position="relative"
                >
                  <Grid
                    templateColumns={`repeat(${GRID_SIZE}, 1fr)`}
                    gap={2}
                    h="full"
                  >
                    {grid.map((number, index) => {
                      const isClicked = clickedCells.has(index);
                      const isNext = number === currentTarget && !isClicked;

                      return (
                        <motion.div
                          key={`${number}-${index}`}
                          initial={{ opacity: 0, scale: 0.8 }}
                          animate={{ opacity: 1, scale: 1 }}
                          transition={{ delay: index * 0.01, duration: 0.2 }}
                        >
                          <Box
                            as="button"
                            w="100%"
                            h="100%"
                            borderRadius="lg"
                            bg={isClicked ? cellActiveBg : cellBg}
                            color={isClicked ? "white" : headingColor}
                            fontSize={{ base: "lg", md: "2xl" }}
                            fontWeight="bold"
                            border="2px solid"
                            borderColor={isClicked ? cellActiveBg : useColorModeValue("gray.200", "#4A5568")}
                            cursor={isClicked ? "default" : "pointer"}
                            transition="all 0.15s"
                            _hover={!isClicked ? {
                              bg: cellHoverBg,
                              borderColor: accentColor,
                              transform: "scale(1.05)",
                            } : {}}
                            _active={!isClicked ? {
                              transform: "scale(0.95)",
                            } : {}}
                            onClick={() => handleCellClick(number, index)}
                            boxShadow={isNext ? `0 0 0 3px ${accentColor}40` : "none"}
                            opacity={isClicked ? 0.7 : 1}
                          >
                            {number}
                          </Box>
                        </motion.div>
                      );
                    })}
                  </Grid>
                </Box>

                <Text color={subTextColor} fontSize="sm">
                  {t("tests.schulteCurrent") || "当前目标"}: <Text as="span" color={accentColor} fontWeight="bold">{currentTarget}</Text>
                </Text>
              </Flex>
            )}

            {gameState === "finished" && (
              <Flex
                w="full"
                minH="450px"
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
                    <Text color={subTextColor} fontSize="lg">
                      {t("tests.schulteComplete") || "完成！"}
                    </Text>
                    <Text color={headingColor} fontSize="5xl" fontWeight="bold">
                      {formatTime(elapsedTime)}
                    </Text>
                    <Text color={subTextColor} fontSize="sm">
                      {t("tests.schulteAverage") || "平均每个数字"}: {(elapsedTime / TOTAL_NUMBERS / 1000).toFixed(2)}s
                    </Text>
                  </VStack>
                </motion.div>
                <motion.div
                  initial={{ opacity: 0, y: 20 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: 0.2, duration: 0.3 }}
                >
                  <LiquidGlassButton onClick={startGame} colorScheme="blue" size="lg">
                    {t("tests.schultePlayAgain") || "再玩一次"}
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
