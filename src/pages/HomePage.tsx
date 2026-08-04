import { Box, Text, Flex, useColorModeValue, HStack, VStack } from "@chakra-ui/react";
import { useTranslation } from "react-i18next";
import GameLauncher from "@/components/GameLauncher";
import { TodayPopularity, useTodayPopularityEnabled } from "@/components/TodayPopularity";
import { AnnouncementCard, useAnnouncementEnabled } from "@/components/AnnouncementCard";
import { RandomQuote, useRandomQuoteEnabled } from "@/components/RandomQuote";
import { useState, useEffect, useRef } from "react";
import HardwareModelCard from "@/components/HardwareModelCard";
import GameWinKeyCard from "@/components/GameWinKeyCard";
import { FeedbackBanner, useFeedbackEnabled } from "@/components/FeedbackBanner";
import { store } from "@/lib/store";
import { getGreeting, rollEasterEgg, EASTER_EGG_TEXT } from "@/lib/greetings";
import { invoke } from "@tauri-apps/api/core";

export default function HomePage() {
  const { t } = useTranslation();
  const textColor = useColorModeValue("gray.800", "#ffffff");
  const [greetingText, setGreetingText] = useState("");
  const usernameRef = useRef("");
  const [gameLauncherEnabled, setGameLauncherEnabled] = useState(true);
  const [homeHardwareModelEnabled, setHomeHardwareModelEnabled] = useState(true);
  const [gameWinKeyCardEnabled, setGameWinKeyCardEnabled] = useState(true);

  const computeGreeting = () => {
    if (rollEasterEgg()) return EASTER_EGG_TEXT;
    return getGreeting(new Date(), usernameRef.current).text;
  };

  // 获取系统用户名
  useEffect(() => {
    (async () => {
      try {
        usernameRef.current = await invoke<string>("get_system_username");
      } catch {
        usernameRef.current = "";
      }
      setGreetingText(computeGreeting());
    })();
  }, []);

  // 每分钟刷新一次，跨时段自动切换问候
  useEffect(() => {
    const timer = setInterval(() => setGreetingText(computeGreeting()), 60000);
    setGreetingText(computeGreeting());
    return () => clearInterval(timer);
  }, []);

  const todayPopularityEnabled = useTodayPopularityEnabled();
  const feedbackEnabled = useFeedbackEnabled();
  const announcementEnabled = useAnnouncementEnabled();
  const randomQuoteEnabled = useRandomQuoteEnabled();
  useEffect(() => {
    (async () => {
      const saved = await store.get<boolean>("nexbox_game_launcher_enabled");
      if (saved !== null && saved !== undefined) {
        setGameLauncherEnabled(saved);
      } else {
        // 兼容旧 localStorage
        const ls = localStorage.getItem("nexbox_game_launcher_enabled");
        if (ls !== null) {
          setGameLauncherEnabled(ls === "true");
        }
      }
    })();

    const handleGameLauncherChange = (e: CustomEvent) => {
      setGameLauncherEnabled(e.detail);
    };

    window.addEventListener("game-launcher-setting-changed", handleGameLauncherChange as EventListener);
    
    return () => {
      window.removeEventListener("game-launcher-setting-changed", handleGameLauncherChange as EventListener);
    };
  }, []);

  useEffect(() => {
    (async () => {
      let saved = await store.get<boolean>("nexbox_home_hardware_model_enabled");
      if (saved !== null && saved !== undefined) {
        setHomeHardwareModelEnabled(saved);
      } else {
        const ls = localStorage.getItem("nexbox_home_hardware_model_enabled");
        if (ls !== null) setHomeHardwareModelEnabled(ls === "true");
      }
    })();

    const handler = (e: CustomEvent) => {
      setHomeHardwareModelEnabled(e.detail);
    };

    window.addEventListener("home-hardware-model-setting-changed", handler as EventListener);
    return () => window.removeEventListener("home-hardware-model-setting-changed", handler as EventListener);
  }, []);

  useEffect(() => {
    (async () => {
      let saved = await store.get<boolean>("nexbox_game_win_key_card_enabled");
      if (saved !== null && saved !== undefined) {
        setGameWinKeyCardEnabled(saved);
      } else {
        const ls = localStorage.getItem("nexbox_game_win_key_card_enabled");
        if (ls !== null) setGameWinKeyCardEnabled(ls === "true");
      }
    })();

    const handler = (e: CustomEvent) => {
      setGameWinKeyCardEnabled(e.detail);
    };

    window.addEventListener("game-win-key-card-setting-changed", handler as EventListener);
    return () => window.removeEventListener("game-win-key-card-setting-changed", handler as EventListener);
  }, []);

  return (
    <Box pt={8} pr={4} pb={4} pl={4} h="calc(100vh - 120px)" position="relative" overflowX="hidden">
      <Flex gap={6} h="100%" align="flex-start">
        <Box flex={1}>
          <Text fontSize="3xl" fontWeight="bold" color={textColor}>
            {greetingText || t("home.title")}
          </Text>
          {(todayPopularityEnabled || announcementEnabled || randomQuoteEnabled) && (
            <HStack mt={3} spacing={3}>
              {todayPopularityEnabled && <TodayPopularity />}
              {announcementEnabled && <AnnouncementCard />}
              {randomQuoteEnabled && <RandomQuote />}
            </HStack>
          )}
        </Box>
        {feedbackEnabled && (
          <Box pt={12}>
            <FeedbackBanner />
          </Box>
        )}
      </Flex>

      {(gameWinKeyCardEnabled || homeHardwareModelEnabled) && (
        <Box position="absolute" bottom={4} left={4}>
          <VStack spacing={2} align="stretch">
            {gameWinKeyCardEnabled && <GameWinKeyCard />}
            {homeHardwareModelEnabled && <HardwareModelCard />}
          </VStack>
        </Box>
      )}

      {gameLauncherEnabled && (
        <Box
          position="absolute"
          bottom={4}
          right={4}
        >
          <GameLauncher />
        </Box>
      )}
    </Box>
  );
}
