import { Box, Text, Flex, useColorModeValue, HStack } from "@chakra-ui/react";
import { useTranslation } from "react-i18next";
import CustomHtmlWidget from "@/components/CustomHtmlWidget";
import GameLauncher from "@/components/GameLauncher";
import { TodayPopularity, useTodayPopularityEnabled } from "@/components/TodayPopularity";
import { AnnouncementCard, useAnnouncementEnabled } from "@/components/AnnouncementCard";
import { RandomQuote, useRandomQuoteEnabled } from "@/components/RandomQuote";
import { useState, useEffect } from "react";

export default function HomePage() {
  const { t } = useTranslation();
  const textColor = useColorModeValue("gray.800", "#ffffff");
  const [customHtmlEnabled, setCustomHtmlEnabled] = useState(true);
  const [gameLauncherEnabled, setGameLauncherEnabled] = useState(true);
  const todayPopularityEnabled = useTodayPopularityEnabled();
  const announcementEnabled = useAnnouncementEnabled();
  const randomQuoteEnabled = useRandomQuoteEnabled();

  useEffect(() => {
    const savedCustomHtml = localStorage.getItem("nexbox_custom_html_enabled");
    if (savedCustomHtml !== null) {
      setCustomHtmlEnabled(savedCustomHtml === "true");
    }

    const savedGameLauncher = localStorage.getItem("nexbox_game_launcher_enabled");
    if (savedGameLauncher !== null) {
      setGameLauncherEnabled(savedGameLauncher === "true");
    }

    const handleCustomHtmlChange = (e: CustomEvent) => {
      setCustomHtmlEnabled(e.detail);
    };

    const handleGameLauncherChange = (e: CustomEvent) => {
      setGameLauncherEnabled(e.detail);
    };

    window.addEventListener("custom-html-setting-changed", handleCustomHtmlChange as EventListener);
    window.addEventListener("game-launcher-setting-changed", handleGameLauncherChange as EventListener);
    
    return () => {
      window.removeEventListener("custom-html-setting-changed", handleCustomHtmlChange as EventListener);
      window.removeEventListener("game-launcher-setting-changed", handleGameLauncherChange as EventListener);
    };
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

      {customHtmlEnabled && (
        <Box
          position="absolute"
          bottom={4}
          left={4}
        >
          <CustomHtmlWidget />
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
