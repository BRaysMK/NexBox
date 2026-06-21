import { Box, Text, VStack, useColorModeValue } from "@chakra-ui/react";
import { keyframes } from "@emotion/react";
import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";

const bounceKeyframes = keyframes`
  0% { transform: scale(1); }
  30% { transform: scale(1.3); }
  100% { transform: scale(1); }
`;

function getTodayKey(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export function useTodayPopularityEnabled() {
  const [enabled, setEnabled] = useState(true);

  useEffect(() => {
    const saved = localStorage.getItem("nexbox_today_popularity_enabled");
    if (saved !== null) {
      setEnabled(saved === "true");
    }
  }, []);

  useEffect(() => {
    const handler = (e: CustomEvent) => setEnabled(e.detail);
    window.addEventListener("today-popularity-setting-changed", handler as EventListener);
    return () => window.removeEventListener("today-popularity-setting-changed", handler as EventListener);
  }, []);

  return enabled;
}

export function TodayPopularity() {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();

  const [value, setValue] = useState<number | null>(null);
  const [animating, setAnimating] = useState(false);

  const valueColor = useColorModeValue("purple.500", "#b794f4");
  const labelColor = useColorModeValue("gray.500", "#cccccc");
  const cardBg = useColorModeValue("white", "#111111");
  const borderColor = useColorModeValue("gray.200", "#333333");

  useEffect(() => {
    const todayKey = getTodayKey();
    const savedDate = localStorage.getItem("nexbox_today_popularity_date");
    const savedValue = localStorage.getItem("nexbox_today_popularity_value");

    if (savedDate === todayKey && savedValue !== null) {
      setValue(Number(savedValue));
    }
  }, []);

  const generate = useCallback(() => {
    const todayKey = getTodayKey();
    const savedDate = localStorage.getItem("nexbox_today_popularity_date");

    if (savedDate === todayKey && value !== null) {
      return;
    }

    const randomValue = Math.floor(Math.random() * 101);
    setValue(randomValue);
    setAnimating(true);
    localStorage.setItem("nexbox_today_popularity_date", todayKey);
    localStorage.setItem("nexbox_today_popularity_value", String(randomValue));
    setTimeout(() => setAnimating(false), 600);
  }, [value]);

  const cardContent = (
    <VStack spacing={0} align="center">
      <Text fontSize="2xs" color={labelColor}>
        {t("home.todayPopularity")}
      </Text>
      <Box
        cursor="pointer"
        onClick={generate}
        userSelect="none"
        animation={animating ? `${bounceKeyframes} 0.6s ease` : undefined}
      >
        <Text
          fontSize="2xl"
          fontWeight="bold"
          color={value !== null ? valueColor : labelColor}
          transition="color 0.3s"
        >
          {value !== null ? value : "?"}
        </Text>
      </Box>
    </VStack>
  );

  if (liquidGlassEnabled) {
    return (
      <LiquidGlassCard py={2} px={3} w="90px">
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
      w="90px"
    >
      {cardContent}
    </Box>
  );
}