import { Box, Text, Flex, useColorModeValue, HStack } from "@chakra-ui/react";
import { useTranslation } from "react-i18next";
import GameLauncher from "@/components/GameLauncher";
import { TodayPopularity, useTodayPopularityEnabled } from "@/components/TodayPopularity";
import { AnnouncementCard, useAnnouncementEnabled } from "@/components/AnnouncementCard";
import { RandomQuote, useRandomQuoteEnabled } from "@/components/RandomQuote";
import { useState, useEffect } from "react";
import HardwareModelCard from "@/components/HardwareModelCard";

export default function HomePage() {
  const { t } = useTranslation();
  const textColor = useColorModeValue("gray.800", "#ffffff");
  const [gameLauncherEnabled, setGameLauncherEnabled] = useState(true);
  const [homeHardwareModelEnabled, setHomeHardwareModelEnabled] = useState(true);
  const todayPopularityEnabled = useTodayPopularityEnabled();
  const announcementEnabled = useAnnouncementEnabled();
  const randomQuoteEnabled = useRandomQuoteEnabled();

  useEffect(() => {
    const savedGameLauncher = localStorage.getItem("nexbox_game_launcher_enabled");
    if (savedGameLauncher !== null) {
      setGameLauncherEnabled(savedGameLauncher === "true");
    }

    const handleGameLauncherChange = (e: CustomEvent) => {
      setGameLauncherEnabled(e.detail);
    };

    window.addEventListener("game-launcher-setting-changed", handleGameLauncherChange as EventListener);
    
    return () => {
      window.removeEventListener("game-launcher-setting-changed", handleGameLauncherChange as EventListener);
    };
  }, []);

  useEffect(() => {
    const saved = localStorage.getItem("nexbox_home_hardware_model_enabled");
    if (saved !== null) {
      setHomeHardwareModelEnabled(saved === "true");
    }

    const handler = (e: CustomEvent) => {
      setHomeHardwareModelEnabled(e.detail);
    };

    window.addEventListener("home-hardware-model-setting-changed", handler as EventListener);
    return () => window.removeEventListener("home-hardware-model-setting-changed", handler as EventListener);
  }, []);

  return (
    <Box pt={8} pr={4} pb={4} pl={4} h="calc(100vh - 120px)" position="relative">
      <Flex gap={6} h="100%">
        <Box flex={1}>
          <Text fontSize="3xl" fontWeight="bold" color={textColor}>
            {t("home.title")}
          </Text>
          {(todayPopularityEnabled || announcementEnabled || randomQuoteEnabled) && (
            <HStack mt={3} spacing={3}>
              {todayPopularityEnabled && <TodayPopularity />}
              {announcementEnabled && <AnnouncementCard />}
              {randomQuoteEnabled && <RandomQuote />}
            </HStack>
          )}
        </Box>
      </Flex>

      {homeHardwareModelEnabled && (
        <Box position="absolute" bottom={4} left={4}>
          <HardwareModelCard />
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
