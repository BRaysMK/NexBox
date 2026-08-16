import { Box, Text, Flex, useColorModeValue, HStack, VStack } from "@chakra-ui/react";
import { useTranslation } from "react-i18next";
import GameLauncher from "@/components/GameLauncher";
import { TodayPopularity, useTodayPopularityEnabled } from "@/components/TodayPopularity";
import { AnnouncementCard, useAnnouncementEnabled } from "@/components/AnnouncementCard";
import { RandomQuote, useRandomQuoteEnabled } from "@/components/RandomQuote";
import { useState, useEffect, useRef } from "react";
import HardwareModelCard from "@/components/HardwareModelCard";
import GameWinKeyCard from "@/components/GameWinKeyCard";
import RandomImageCard, { useRandomImageEnabled } from "@/components/RandomImageCard";
import AiChatEntryCard, { useAiEntryEnabled } from "@/components/ai/AiChatEntryCard";
import { FeedbackBanner, useFeedbackEnabled } from "@/components/FeedbackBanner";
import { store } from "@/lib/store";
import { initVisibility, subscribeVisibility } from "@/lib/ui-visibility";
import { getGreeting, rollEasterEgg, EASTER_EGG_TEXT } from "@/lib/greetings";
import { invoke } from "@tauri-apps/api/core";

/**
 * 将问候语末尾的颜文字/表情块拆出来，用于整体换行（nowrap），
 * 避免颜文字被折行拆成上下两行。找不到颜文字起始括号时原样返回。
 */
function splitEmojiBlock(text: string): { main: string; emoji: string } {
  const startChars = ["(", "（", "[", "【", "｟", "〘", "「", "『", "［"];
  let pos = -1;
  for (const c of startChars) {
    const idx = text.lastIndexOf(c);
    if (idx > pos) pos = idx;
  }
  if (pos <= 0) return { main: text, emoji: "" };
  return { main: text.slice(0, pos).trimEnd(), emoji: text.slice(pos) };
}

export default function HomePage() {
  const { t } = useTranslation();
  const textColor = useColorModeValue("gray.800", "#ffffff");
  const [greetingText, setGreetingText] = useState("");
  const usernameRef = useRef("");
  const [gameLauncherEnabled, setGameLauncherEnabled] = useState(() => initVisibility("nexbox_game_launcher_enabled"));
  const [homeHardwareModelEnabled, setHomeHardwareModelEnabled] = useState(() => initVisibility("nexbox_home_hardware_model_enabled"));
  const [gameWinKeyCardEnabled, setGameWinKeyCardEnabled] = useState(() => initVisibility("nexbox_game_win_key_card_enabled"));

  const computeGreeting = () => {
    if (rollEasterEgg()) return EASTER_EGG_TEXT;
    return getGreeting(new Date(), usernameRef.current).text;
  };

  // 获取用户名：优先使用自定义标题用户名，留空时回退到系统用户名
  useEffect(() => {
    (async () => {
      try {
        const custom = await store.get<string>("nexbox_home_username");
        if (custom && custom.trim()) {
          usernameRef.current = custom.trim();
        } else {
          const ls = localStorage.getItem("nexbox_home_username");
          if (ls && ls.trim()) {
            usernameRef.current = ls.trim();
          } else {
            usernameRef.current = await invoke<string>("get_system_username");
          }
        }
      } catch {
        usernameRef.current = "";
      }
      setGreetingText(computeGreeting());
    })();

    const handleUsernameChange = () => {
      (async () => {
        try {
          const custom = await store.get<string>("nexbox_home_username");
          if (custom && custom.trim()) {
            usernameRef.current = custom.trim();
          } else {
            const ls = localStorage.getItem("nexbox_home_username");
            usernameRef.current = ls && ls.trim() ? ls.trim() : await invoke<string>("get_system_username");
          }
        } catch {
          usernameRef.current = "";
        }
        setGreetingText(computeGreeting());
      })();
    };

    window.addEventListener("home-username-setting-changed", handleUsernameChange);
    return () => window.removeEventListener("home-username-setting-changed", handleUsernameChange);
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
  const randomImageEnabled = useRandomImageEnabled();
  const aiEntryEnabled = useAiEntryEnabled();
  useEffect(() =>
    subscribeVisibility("nexbox_game_launcher_enabled", "game-launcher-setting-changed", setGameLauncherEnabled, store),
  []);

  useEffect(() =>
    subscribeVisibility("nexbox_home_hardware_model_enabled", "home-hardware-model-setting-changed", setHomeHardwareModelEnabled, store),
  []);

  useEffect(() =>
    subscribeVisibility("nexbox_game_win_key_card_enabled", "game-win-key-card-setting-changed", setGameWinKeyCardEnabled, store),
  []);

  const greeting = greetingText || t("home.title");
  const { main: greetingMain, emoji: greetingEmoji } = splitEmojiBlock(greeting);

  return (
    <Box pt={8} pr={4} pb={4} pl={4} h="calc(100vh - 120px)" position="relative" overflowX="hidden">
      <Flex gap={6} h="100%" align="flex-start">
        <Box flex={1}>
          <Text fontSize="3xl" fontWeight="bold" color={textColor} lineHeight="1.4">
            {greetingEmoji ? (
              <>
                {greetingMain}
                <span style={{ whiteSpace: "nowrap" }}>{greetingEmoji}</span>
              </>
            ) : (
              greeting
            )}
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

      {(randomImageEnabled || gameWinKeyCardEnabled || homeHardwareModelEnabled || aiEntryEnabled) && (
        <Box position="absolute" bottom={4} left={4}>
          <VStack spacing={2} align="stretch">
            {randomImageEnabled && <RandomImageCard />}
            {gameWinKeyCardEnabled && <GameWinKeyCard />}
            {homeHardwareModelEnabled && <HardwareModelCard />}
            {aiEntryEnabled && !homeHardwareModelEnabled && <AiChatEntryCard />}
          </VStack>
          {/* 盒子喵浮在硬件型号右侧，不参与文档流，避免撑宽下方卡片 */}
          {aiEntryEnabled && homeHardwareModelEnabled && (
            <Box position="absolute" left="100%" ml={2} bottom={0}>
              <AiChatEntryCard />
            </Box>
          )}
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
