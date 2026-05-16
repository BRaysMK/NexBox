import { Box, Text, Flex, useColorModeValue, VStack, HStack } from "@chakra-ui/react";
import { useTranslation } from "react-i18next";
import QuickTools from "@/components/QuickTools";
import CustomHtmlWidget from "@/components/CustomHtmlWidget";
import { TodayPopularity, useTodayPopularityEnabled } from "@/components/TodayPopularity";
import { AnnouncementCard, useAnnouncementEnabled } from "@/components/AnnouncementCard";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useState, useEffect } from "react";

export default function HomePage() {
  const { t } = useTranslation();
  const textColor = useColorModeValue("gray.800", "#ffffff");
  const [quickToolsEnabled, setQuickToolsEnabled] = useState(true);
  const [customHtmlEnabled, setCustomHtmlEnabled] = useState(true);
  const todayPopularityEnabled = useTodayPopularityEnabled();
  const announcementEnabled = useAnnouncementEnabled();

  useEffect(() => {
    const savedQuickTools = localStorage.getItem("nexbox_quick_tools_enabled");
    if (savedQuickTools !== null) {
      setQuickToolsEnabled(savedQuickTools === "true");
    }

    const savedCustomHtml = localStorage.getItem("nexbox_custom_html_enabled");
    if (savedCustomHtml !== null) {
      setCustomHtmlEnabled(savedCustomHtml === "true");
    }

    const handleQuickToolsChange = (e: CustomEvent) => {
      setQuickToolsEnabled(e.detail);
    };

    const handleCustomHtmlChange = (e: CustomEvent) => {
      setCustomHtmlEnabled(e.detail);
    };

    window.addEventListener("quick-tools-setting-changed", handleQuickToolsChange as EventListener);
    window.addEventListener("custom-html-setting-changed", handleCustomHtmlChange as EventListener);
    
    return () => {
      window.removeEventListener("quick-tools-setting-changed", handleQuickToolsChange as EventListener);
      window.removeEventListener("custom-html-setting-changed", handleCustomHtmlChange as EventListener);
    };
  }, []);

  return (
    <Box pt={8} pr={quickToolsEnabled ? 4 : 0} pb={customHtmlEnabled ? 4 : 0} pl={customHtmlEnabled ? 4 : 0} h="calc(100vh - 120px)" position="relative">
      <Flex gap={6} h="100%">
        <Box flex={1}>
          <Text fontSize="3xl" fontWeight="bold" color={textColor}>
            {t("home.title")}
          </Text>
          {(todayPopularityEnabled || announcementEnabled) && (
            <HStack mt={3} spacing={3}>
              {todayPopularityEnabled && <TodayPopularity />}
              {announcementEnabled && <AnnouncementCard />}
            </HStack>
          )}
        </Box>

        {quickToolsEnabled && (
          <LiquidGlassCard w="280px" p={4} h="fit-content">
            <QuickTools />
          </LiquidGlassCard>
        )}
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
    </Box>
  );
}
