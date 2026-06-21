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
} from "@chakra-ui/react";
import { ArrowLeft, Trophy, Timer, Zap } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useState, useRef, useEffect, useCallback } from "react";
import { LiquidGlassButton } from "@/components/special/liquid-glass-button";
import { motion } from "framer-motion";

type GameState = "idle" | "playing" | "finished";

interface ChoiceItem {
  emoji: string;
  name: string;
}

const ITEMS: ChoiceItem[] = [
  { emoji: "🍎", name: "苹果" },
  { emoji: "🐶", name: "狗" },
  { emoji: "🚗", name: "汽车" },
  { emoji: "🌙", name: "月亮" },
  { emoji: "⭐", name: "星星" },
  { emoji: "🔥", name: "火" },
  { emoji: "💧", name: "水" },
  { emoji: "🌲", name: "树" },
  { emoji: "🌸", name: "花" },
  { emoji: "🎵", name: "音乐" },
  { emoji: "📚", name: "书" },
  { emoji: "⚽", name: "球" },
  { emoji: "✈️", name: "飞机" },
  { emoji: "🏠", name: "房子" },
  { emoji: "🌈", name: "彩虹" },
  { emoji: "🐱", name: "猫" },
  { emoji: "🐟", name: "鱼" },
  { emoji: "🍕", name: "披萨" },
  { emoji: "🎸", name: "吉他" },
  { emoji: "👑", name: "皇冠" },
];

const GAME_DURATION = 30;
const OPTIONS_COUNT = 4;

function shuffleArray<T>(array: T[]): T[] {
  const shuffled = [...array];
  for (let i = shuffled.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [shuffled[i], shuffled[j]] = [shuffled[j], shuffled[i]];
  }
  return shuffled;
}

export default function ChoiceTestPage() {
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
  const [target, setTarget] = useState<ChoiceItem | null>(null);
  const [options, setOptions] = useState<ChoiceItem[]>([]);
  const [roundKey, setRoundKey] = useState(0);

  const timerRef = useRef<NodeJS.Timeout | null>(null);

  const generateRound = useCallback(() => {
    const shuffled = shuffleArray(ITEMS);
    const targetItem = shuffled[0];
    const otherItems = shuffled.slice(1, OPTIONS_COUNT);
    const optionItems = shuffleArray([targetItem, ...otherItems]);

    setTarget(targetItem);
    setOptions(optionItems);
    setRoundKey((prev) => prev + 1);
  }, []);

  const startGame = () => {
    setGameState("playing");
    setScore(0);
    setTimeLeft(GAME_DURATION);
    generateRound();
  };

  const endGame = () => {
    if (timerRef.current) clearInterval(timerRef.current);
    setGameState("finished");
    if (bestScore === null || score > bestScore) {
      setBestScore(score);
    }
  };

  const handleChoice = (item: ChoiceItem) => {
    if (gameState !== "playing" || !target) return;

    if (item.name === target.name) {
      setScore((prev) => prev + 1);
    }
    generateRound();
  };

  const exitGame = () => {
    if (timerRef.current) clearInterval(timerRef.current);
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
    };
  }, []);

  const accentColor = "#F59E0B";

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
            {t("tests.choiceTitle") || "选择测试"}
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
                    {t("tests.choiceTime") || "时间"}
                  </Text>
                  <Text color={headingColor} fontSize="xl" fontWeight="bold">
                    {timeLeft}s
                  </Text>
                </VStack>
              </LiquidGlassCard>

              <LiquidGlassCard p={4} minW="150px" textAlign="center">
                <VStack spacing={1}>
                  <Zap size={24} color={accentColor} />
                  <Text color={subTextColor} fontSize="sm">
                    {t("tests.choiceScore") || "分数"}
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
                      {t("tests.choiceBest") || "最佳"}
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
            borderRadius="2xl"
            bg={useColorModeValue("#F3F4F6", "#000000")}
            position="relative"
            overflow="hidden"
            p={{ base: 4, md: 8 }}
            minH="400px"
            userSelect="none"
          >
            {gameState === "idle" && (
              <Flex
                w="full"
                minH="400px"
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
                      {t("tests.choiceInstruction1") || "屏幕中央会显示目标，请尽快在下方选项中选择对应项目"}
                    </Text>
                    <Text color={subTextColor} fontSize="md">
                      {t("tests.choiceInstruction2") || "快速准确地做出选择，30秒内尽可能获得高分！"}
                    </Text>
                  </VStack>
                </motion.div>
                <motion.div
                  initial={{ opacity: 0, scale: 0.9 }}
                  animate={{ opacity: 1, scale: 1 }}
                  transition={{ delay: 0.2, duration: 0.3 }}
                >
                  <LiquidGlassButton onClick={startGame} colorScheme="yellow" size="lg">
                    {t("tests.choiceStart") || "开始游戏"}
                  </LiquidGlassButton>
                </motion.div>
              </Flex>
            )}

            {gameState === "playing" && target && (
              <motion.div
                key={roundKey}
                initial={{ opacity: 0, y: -10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.2 }}
              >
                <Flex
                  justifyContent="center"
                  alignItems="center"
                  mb={8}
                  mt={4}
                >
                  <Box
                    bg={useColorModeValue("white", "#000000")}
                    px={10}
                    py={6}
                    borderRadius="2xl"
                    border="3px solid"
                    borderColor={`${accentColor}60`}
                    boxShadow="lg"
                  >
                    <VStack spacing={3}>
                      <Text fontSize="5xl">{target.emoji}</Text>
                      <Text
                        color={headingColor}
                        fontSize="2xl"
                        fontWeight="bold"
                      >
                        {target.name}
                      </Text>
                    </VStack>
                  </Box>
                </Flex>

                <SimpleGrid columns={{ base: 2, md: 4 }} spacing={4}>
                  {options.map((item, index) => (
                    <motion.div
                      key={`${item.name}-${roundKey}`}
                      initial={{ opacity: 0, scale: 0.9 }}
                      animate={{ opacity: 1, scale: 1 }}
                      transition={{ delay: index * 0.05, duration: 0.2 }}
                    >
                      <Box
                        as="button"
                        onClick={() => handleChoice(item)}
                        w="full"
                        py={4}
                        px={3}
                        borderRadius="xl"
                        bg={useColorModeValue("white", "#000000")}
                        border="2px solid"
                        borderColor={useColorModeValue("gray.200", "#333333")}
                        cursor="pointer"
                        transition="all 0.15s"
                        _hover={{
                          borderColor: accentColor,
                          bg: `${accentColor}15`,
                          transform: "scale(1.03)",
                        }}
                        _active={{ transform: "scale(0.97)" }}
                      >
                        <VStack spacing={1}>
                          <Text fontSize="2xl">{item.emoji}</Text>
                          <Text
                            color={headingColor}
                            fontSize="md"
                            fontWeight="medium"
                          >
                            {item.name}
                          </Text>
                        </VStack>
                      </Box>
                    </motion.div>
                  ))}
                </SimpleGrid>
              </motion.div>
            )}

            {gameState === "finished" && (
              <Flex
                w="full"
                minH="400px"
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
                      {t("tests.choiceTotal") || "分！"}
                    </Text>
                    {score > 0 && (
                      <Text color={subTextColor} fontSize="sm">
                        {t("tests.choiceAvg") || "平均每题"} {(GAME_DURATION / Math.max(score, 1)).toFixed(1)}s
                      </Text>
                    )}
                  </VStack>
                </motion.div>
                <motion.div
                  initial={{ opacity: 0, y: 20 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: 0.2, duration: 0.3 }}
                >
                  <LiquidGlassButton onClick={startGame} colorScheme="yellow" size="lg">
                    {t("tests.choicePlayAgain") || "再玩一次"}
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
