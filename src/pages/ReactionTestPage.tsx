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
import { ArrowLeft, Trophy } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useState, useRef, useEffect } from "react";
import { LiquidGlassButton } from "@/components/special/liquid-glass-button";
import { motion } from "framer-motion";

type GameState = "idle" | "waiting" | "ready" | "result" | "tooEarly";

export default function ReactionTestPage() {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const navigate = useNavigate();

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const cardBg = useColorModeValue("white", "#111111");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const subTextColor = useColorModeValue("gray.500", "#888888");

  const [gameState, setGameState] = useState<GameState>("idle");
  const [reactionTime, setReactionTime] = useState<number | null>(null);
  const [bestTime, setBestTime] = useState<number | null>(null);
  const [history, setHistory] = useState<number[]>([]);
  const startTimeRef = useRef<number>(0);
  const timeoutRef = useRef<NodeJS.Timeout | null>(null);

  const startTest = () => {
    setGameState("waiting");
    setReactionTime(null);
    const waitTime = 1500 + Math.random() * 3000;
    timeoutRef.current = setTimeout(() => {
      setGameState("ready");
      startTimeRef.current = Date.now();
    }, waitTime);
  };

  const handleClick = () => {
    if (gameState === "waiting") {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
      setGameState("tooEarly");
      return;
    }

    if (gameState === "ready") {
      const time = Date.now() - startTimeRef.current;
      setReactionTime(time);
      const newHistory = [...history, time];
      setHistory(newHistory);
      if (bestTime === null || time < bestTime) {
        setBestTime(time);
      }
      setGameState("result");
      return;
    }
  };

  const resetGame = () => {
    setGameState("idle");
    setReactionTime(null);
  };

  useEffect(() => {
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, []);

  const getBoxColor = () => {
    switch (gameState) {
      case "waiting":
        return "#EF4444";
      case "ready":
        return "#22C55E";
      case "tooEarly":
        return "#F59E0B";
      default:
        return useColorModeValue("#F3F4F6", "#000000");
    }
  };

  const getTextColor = () => {
    switch (gameState) {
      case "waiting":
      case "ready":
        return "white";
      case "tooEarly":
        return "#000000";
      default:
        return useColorModeValue("gray.900", "white");
    }
  };

  const getBoxText = () => {
    switch (gameState) {
      case "idle":
        return t("tests.reactionClickStart") || "点击开始";
      case "waiting":
        return t("tests.reactionWait") || "等待...";
      case "ready":
        return t("tests.reactionClick") || "点击!";
      case "tooEarly":
        return t("tests.reactionTooEarly") || "太早了!";
      case "result":
        return `${reactionTime} ${t("tests.reactionMs") || "ms"}`;
      default:
        return "";
    }
  };

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
            {t("tests.reactionTitle") || "反射弧测试"}
          </Heading>
          <Box w="100px" />
        </HStack>

        <motion.div
          key={gameState}
          initial={{ scale: 0.95, opacity: 0.7 }}
          animate={{ scale: 1, opacity: 1 }}
          transition={{ duration: 0.2 }}
        >
          <Box
            as="button"
            onClick={gameState === "idle" || gameState === "result" || gameState === "tooEarly" ? startTest : handleClick}
            w="full"
            h="300px"
            borderRadius="2xl"
            bg={getBoxColor()}
            display="flex"
            alignItems="center"
            justifyContent="center"
            cursor="pointer"
            transition="all 0.2s"
            _hover={{
              transform: gameState === "idle" || gameState === "result" || gameState === "tooEarly" ? "scale(1.02)" : "none",
            }}
          >
            <Text
              fontSize="4xl"
              fontWeight="bold"
              color={getTextColor()}
              textAlign="center"
            >
              {getBoxText()}
            </Text>
          </Box>
        </motion.div>

        {bestTime !== null && (
          <motion.div
            initial={{ opacity: 0, scale: 0.9 }}
            animate={{ opacity: 1, scale: 1 }}
            transition={{ delay: 0.1, duration: 0.3 }}
          >
            <Flex justifyContent="center" gap={4} flexWrap="wrap">
              <LiquidGlassCard p={4} minW="150px" textAlign="center">
                <VStack spacing={1}>
                  <Trophy size={24} color="#F59E0B" />
                  <Text color={subTextColor} fontSize="sm">
                    {t("tests.reactionBest") || "最佳"}
                  </Text>
                  <Text color={headingColor} fontSize="xl" fontWeight="bold">
                    {bestTime}ms
                  </Text>
                </VStack>
              </LiquidGlassCard>
            </Flex>
          </motion.div>
        )}

        {(gameState === "result" || gameState === "tooEarly") && (
          <motion.div
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.25 }}
          >
            <HStack justifyContent="center" spacing={4}>
              <LiquidGlassButton
                onClick={resetGame}
                colorScheme="gray"
              >
                {t("tests.reactionAgain") || "再来一次"}
              </LiquidGlassButton>
            </HStack>
          </motion.div>
        )}

        {history.length > 0 && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ delay: 0.2, duration: 0.3 }}
          >
            <Box>
              <Text color={subTextColor} mb={2} fontSize="sm">
                {t("tests.reactionHistory") || "历史记录"}
              </Text>
              <Flex gap={2} flexWrap="wrap">
                {history.map((time, index) => (
                  <motion.div
                    key={index}
                    initial={{ opacity: 0, scale: 0.8 }}
                    animate={{ opacity: 1, scale: 1 }}
                    transition={{ delay: index * 0.05 }}
                  >
                    <Box
                      bg={useColorModeValue("#F3F4F6", "#000000")}
                      px={3}
                      py={1}
                      borderRadius="md"
                    >
                      <Text color={headingColor} fontSize="sm">{time}ms</Text>
                    </Box>
                  </motion.div>
                ))}
              </Flex>
            </Box>
          </motion.div>
        )}
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
