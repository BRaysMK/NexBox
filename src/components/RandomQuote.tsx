import { Box, Text, VStack, useColorModeValue } from "@chakra-ui/react";
import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";

const quotes = [
  { text: "游戏不仅仅是游戏，游戏可以走进生活，改变生活", author: "刘旭东" },
  { text: "游戏可以重开，人生没有读档。", author: "木流" },
  { text: "真正的对手，从来都是自己。", author: "木流" },
  { text: "我希望你是在享受游戏。", author: "木流" },
  { text: "输赢是一瞬的结果，尽兴才是整场的意义。", author: "木流" },
  { text: "放下胜负心，游戏才真正属于自己。", author: "木流" },
  { text: "为了点游戏币，人都不做了。", author: "老飞宇66" },
  { text: "真正的强者不一定是赢家。", author: "深蓝" },
  { text: "当你觉得精疲力尽的时候，就是突破自己的最佳时机。", author: "老黑" },
  { text: "游戏怎么了，游戏提供了千千万万就业岗位。", author: "木流" },
  { text: "不是游戏害人，是不懂节制的心态害人。", author: "木流" },
  { text: "游戏非洪水猛兽，害人的是放纵，不是游戏本身。", author: "木流" },
  { text: "我们是在玩游戏，不是游戏玩我们。", author: "木流" },
  { text: "要懂得分清游戏和现实。", author: "木流" },
];

function getRandomQuote() {
  const index = Math.floor(Math.random() * quotes.length);
  return quotes[index];
}

export function useRandomQuoteEnabled() {
  const [enabled, setEnabled] = useState(true);

  useEffect(() => {
    const saved = localStorage.getItem("nexbox_random_quote_enabled");
    if (saved !== null) {
      setEnabled(saved === "true");
    }
  }, []);

  useEffect(() => {
    const handler = (e: CustomEvent) => setEnabled(e.detail);
    window.addEventListener("random-quote-setting-changed", handler as EventListener);
    return () => window.removeEventListener("random-quote-setting-changed", handler as EventListener);
  }, []);

  return enabled;
}

export function RandomQuote() {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();

  const [quote, setQuote] = useState(() => getRandomQuote());

  const labelColor = useColorModeValue("gray.500", "#cccccc");
  const textColor = useColorModeValue("gray.700", "#e0e0e0");
  const authorColor = useColorModeValue("gray.400", "#888888");
  const cardBg = useColorModeValue("white", "#1a1a1a");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const hoverBorderColor = useColorModeValue("purple.500", "#b794f4");

  const handleRefresh = () => {
    setQuote(getRandomQuote());
  };

  const cardContent = (
    <VStack spacing={0.5} align="flex-start" minW="90px">
      <Text fontSize="2xs" color={labelColor}>
        {t("home.randomQuote")}
      </Text>
      <Box cursor="pointer" onClick={handleRefresh} userSelect="none">
        <Text fontSize="sm" color={textColor} fontWeight="medium" whiteSpace="nowrap">
          {quote.text}
        </Text>
        <Text fontSize="2xs" color={authorColor} textAlign="right">
          -{quote.author}
        </Text>
      </Box>
    </VStack>
  );

  if (liquidGlassEnabled) {
    return (
      <LiquidGlassCard py={2} px={3} minW="90px">
        {cardContent}
      </LiquidGlassCard>
    );
  }

  return (
    <Box
      bg={cardBg}
      borderRadius="xl"
      border="1px solid"
      borderColor={borderColor}
      py={2}
      px={3}
      minW="90px"
      cursor="pointer"
      _hover={{ borderColor: hoverBorderColor }}
      transition="border-color 0.2s"
    >
      {cardContent}
    </Box>
  );
}
