import {
  Box,
  Flex,
  HStack,
  Text,
  Switch,
  Badge,
  VStack,
  Divider,
  useColorMode,
  useColorModeValue,
  Button,
  Input,
  Modal,
  ModalOverlay,
  ModalContent,
  ModalHeader,
  ModalCloseButton,
  ModalBody,
  ModalFooter,
  Progress,
  useToast,
} from "@chakra-ui/react";
import { AnimatePresence, motion } from "framer-motion";
import { useTransitionMode, getVariants, getTransitionConfig } from "@/components/ui/animated-page";

import {
  LuMonitor,
  LuInfo,
  LuSettings,
  LuChevronDown,
  LuCheck,
  LuImage,
  LuUpload,
  LuX,
  LuDownload,
  LuExternalLink,
  LuRefreshCw,
  LuPalette,
  LuWifi,
  LuGlobe,
  LuHeart,
  LuKeyboard,
} from "react-icons/lu";
import { RiBilibiliFill, RiTiktokFill } from "react-icons/ri";
import { useState, useRef, useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import { PRESET_COLORS, hexToRgba } from "@/lib/color-utils";
import { CustomColorPicker } from "@/components/special/custom-color-picker";
import { fetchLatestRelease, compareVersions, fetchReleaseByTag, type GiteeRelease } from "@/lib/update-checker";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { LiquidGlassButton } from "@/components/special/liquid-glass-button";
import { LiquidGlassMenuItem } from "@/components/special/liquid-glass-menu-item";
import { ThemeSwitch } from "@/components/special/theme-switch";
import { CustomSelect } from "@/components/special/custom-select";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { HotkeyRecorder } from "@/components/hotkey-recorder";
import { useAppStartup } from "@/contexts/app-startup-context";

const settingItems = [
  { id: "general", labelKey: "settings.general", icon: LuSettings },
  { id: "appearance", labelKey: "settings.appearance", icon: LuMonitor },
  { id: "hotkeys", labelKey: "settings.hotkeys", icon: LuKeyboard },
  { id: "network", labelKey: "settings.network", icon: LuWifi },
  { id: "sponsor", labelKey: "settings.sponsor", icon: LuHeart },
  { id: "about", labelKey: "settings.about", icon: LuInfo },
];

function GeneralSettings() {
  const { t, i18n } = useTranslation();
  const { config, getContrastTextColor } = useThemeColor();
  const { liquidGlassEnabled } = useBackground();
  const [language, setLanguage] = useState(i18n.language || "zh");
  const [todayPopularityEnabled, setTodayPopularityEnabled] = useState(true);
  const [announcementEnabled, setAnnouncementEnabled] = useState(true);
  const [randomQuoteEnabled, setRandomQuoteEnabled] = useState(true);
  const [gameLauncherEnabled, setGameLauncherEnabled] = useState(true);
  const [homeHardwareModelEnabled, setHomeHardwareModelEnabled] = useState(true);
  const [splashLogo, setSplashLogo] = useState<string | null>(null);
  const [closeBehavior, setCloseBehavior] = useState<string>(() => {
    return localStorage.getItem("nexbox_close_behavior") || "ask";
  });
  const [sidebarShowLabel, setSidebarShowLabel] = useState(false);
  const [pageTransitionMode, setPageTransitionMode] = useState<"slide" | "fade" | "off">("fade");
  const [autoStart, setAutoStart] = useState(false);
  const [autoStartLoading, setAutoStartLoading] = useState(true);
  const titleColor = useColorModeValue("gray.800", "#ffffff");
  const labelColor = useColorModeValue("gray.700", "#e0e0e0");
  const subLabelColor = useColorModeValue("gray.500", "#888888");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const splashLogoHoverBorder = useColorModeValue("blue.400", "blue.300");
  const segmentedControlBg = useColorModeValue(
    liquidGlassEnabled ? "rgba(255,255,255,0.24)" : "rgba(255,255,255,0.78)",
    liquidGlassEnabled ? "rgba(18,18,18,0.34)" : "rgba(18,18,18,0.58)"
  );
  const segmentedControlBorder = useColorModeValue(
    liquidGlassEnabled ? "rgba(255,255,255,0.34)" : "rgba(255,255,255,0.72)",
    liquidGlassEnabled ? "rgba(255,255,255,0.12)" : "rgba(255,255,255,0.08)"
  );
  const segmentedControlHoverBg = useColorModeValue(
    liquidGlassEnabled ? "rgba(255,255,255,0.18)" : "rgba(255,255,255,0.96)",
    liquidGlassEnabled ? "rgba(255,255,255,0.08)" : "rgba(255,255,255,0.08)"
  );
  const segmentedControlShadow = useColorModeValue(
    liquidGlassEnabled ? "0 16px 34px rgba(15, 23, 42, 0.14)" : "0 10px 30px rgba(15, 23, 42, 0.08)",
    liquidGlassEnabled ? "0 18px 38px rgba(0, 0, 0, 0.28)" : "0 12px 30px rgba(0, 0, 0, 0.24)"
  );
  const segmentedActiveBg = useColorModeValue(
    `linear-gradient(135deg, ${hexToRgba(config.primaryColor, liquidGlassEnabled ? 0.26 : 0.2)} 0%, ${hexToRgba(config.primaryColor, liquidGlassEnabled ? 0.42 : 0.34)} 100%)`,
    `linear-gradient(135deg, ${hexToRgba(config.primaryColor, liquidGlassEnabled ? 0.38 : 0.32)} 0%, ${hexToRgba(config.primaryColor, liquidGlassEnabled ? 0.24 : 0.18)} 100%)`
  );
  const segmentedActiveBorder = useColorModeValue(
    hexToRgba(config.primaryColor, liquidGlassEnabled ? 0.34 : 0.28),
    hexToRgba(config.primaryColor, liquidGlassEnabled ? 0.5 : 0.42)
  );
  const segmentedActiveShadow = useColorModeValue(
    `0 10px 24px ${hexToRgba(config.primaryColor, liquidGlassEnabled ? 0.22 : 0.18)}`,
    `0 12px 28px ${hexToRgba(config.primaryColor, liquidGlassEnabled ? 0.3 : 0.24)}`
  );
  const segmentedActiveOverlayGradient = useColorModeValue(
    liquidGlassEnabled
      ? "linear(to-b, rgba(255,255,255,0.5), rgba(255,255,255,0.12))"
      : "linear(to-b, rgba(255,255,255,0.42), rgba(255,255,255,0.08))",
    liquidGlassEnabled
      ? "linear(to-b, rgba(255,255,255,0.22), rgba(255,255,255,0.03))"
      : "linear(to-b, rgba(255,255,255,0.16), rgba(255,255,255,0.02))"
  );
  const segmentedContainerGlow = useColorModeValue(
    hexToRgba(config.primaryColor, liquidGlassEnabled ? 0.14 : 0.08),
    hexToRgba(config.primaryColor, liquidGlassEnabled ? 0.18 : 0.1)
  );
  const segmentedGlassSheen = useColorModeValue(
    "linear-gradient(135deg, rgba(255,255,255,0.58) 0%, rgba(255,255,255,0.08) 52%, rgba(255,255,255,0.02) 100%)",
    "linear-gradient(135deg, rgba(255,255,255,0.12) 0%, rgba(255,255,255,0.04) 52%, rgba(255,255,255,0.01) 100%)"
  );
  const segmentedActiveText = getContrastTextColor();

  useEffect(() => {
    const savedLang = i18n.language || "zh";
    setLanguage(savedLang);

    const savedTodayPopularity = localStorage.getItem("nexbox_today_popularity_enabled");
    if (savedTodayPopularity !== null) {
      setTodayPopularityEnabled(savedTodayPopularity === "true");
    }

    const savedAnnouncement = localStorage.getItem("nexbox_announcement_enabled");
    if (savedAnnouncement !== null) {
      setAnnouncementEnabled(savedAnnouncement === "true");
    }

    const savedRandomQuote = localStorage.getItem("nexbox_random_quote_enabled");
    if (savedRandomQuote !== null) {
      setRandomQuoteEnabled(savedRandomQuote === "true");
    }

    const savedGameLauncher = localStorage.getItem("nexbox_game_launcher_enabled");
    if (savedGameLauncher !== null) {
      setGameLauncherEnabled(savedGameLauncher === "true");
    }

    const savedHomeHardwareModel = localStorage.getItem("nexbox_home_hardware_model_enabled");
    if (savedHomeHardwareModel !== null) {
      setHomeHardwareModelEnabled(savedHomeHardwareModel === "true");
    }

    const savedSplashLogo = localStorage.getItem("nexbox_splash_logo");
    if (savedSplashLogo) {
      setSplashLogo(savedSplashLogo);
    }

    const savedCloseBehavior = localStorage.getItem("nexbox_close_behavior");
    if (savedCloseBehavior) {
      setCloseBehavior(savedCloseBehavior);
    }

    const savedSidebarShowLabel = localStorage.getItem("nexbox_sidebar_show_label");
    if (savedSidebarShowLabel !== null) {
      setSidebarShowLabel(savedSidebarShowLabel === "true");
    }

    const mode = localStorage.getItem("nexbox_page_transition") as "slide" | "fade" | "off" | null;
    if (mode && ["slide", "fade", "off"].includes(mode)) {
      setPageTransitionMode(mode);
    } else {
      const oldVal = localStorage.getItem("nexbox_page_transition_enabled");
      if (oldVal !== null) {
        const newMode = oldVal === "true" ? "slide" : "off";
        setPageTransitionMode(newMode);
        localStorage.setItem("nexbox_page_transition", newMode);
        localStorage.removeItem("nexbox_page_transition_enabled");
      }
    }

    invoke<boolean>("check_nexbox_auto_start")
      .then((enabled) => setAutoStart(enabled))
      .catch(() => {})
      .finally(() => setAutoStartLoading(false));
  }, [i18n.language]);

  const handleLanguageChange = (newLang: string) => {
    setLanguage(newLang);
    i18n.changeLanguage(newLang);
    localStorage.setItem("i18nextLng", newLang);
  };

  const handleTodayPopularityToggle = () => {
    const newValue = !todayPopularityEnabled;
    setTodayPopularityEnabled(newValue);
    localStorage.setItem("nexbox_today_popularity_enabled", String(newValue));
    window.dispatchEvent(new CustomEvent("today-popularity-setting-changed", { detail: newValue }));
  };

  const handleAnnouncementToggle = () => {
    const newValue = !announcementEnabled;
    setAnnouncementEnabled(newValue);
    localStorage.setItem("nexbox_announcement_enabled", String(newValue));
    window.dispatchEvent(new CustomEvent("announcement-setting-changed", { detail: newValue }));
  };

  const handleRandomQuoteToggle = () => {
    const newValue = !randomQuoteEnabled;
    setRandomQuoteEnabled(newValue);
    localStorage.setItem("nexbox_random_quote_enabled", String(newValue));
    window.dispatchEvent(new CustomEvent("random-quote-setting-changed", { detail: newValue }));
  };

  const handleGameLauncherToggle = () => {
    const newValue = !gameLauncherEnabled;
    setGameLauncherEnabled(newValue);
    localStorage.setItem("nexbox_game_launcher_enabled", String(newValue));
    window.dispatchEvent(new CustomEvent("game-launcher-setting-changed", { detail: newValue }));
  };

  const handleHomeHardwareModelToggle = () => {
    const newValue = !homeHardwareModelEnabled;
    setHomeHardwareModelEnabled(newValue);
    localStorage.setItem("nexbox_home_hardware_model_enabled", String(newValue));
    window.dispatchEvent(new CustomEvent("home-hardware-model-setting-changed", { detail: newValue }));
  };

  const handleSplashLogoUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      const reader = new FileReader();
      reader.onloadend = () => {
        const result = reader.result as string;
        setSplashLogo(result);
        localStorage.setItem("nexbox_splash_logo", result);
      };
      reader.readAsDataURL(file);
    }
    e.target.value = "";
  };

  const handleSplashLogoReset = () => {
    setSplashLogo(null);
    localStorage.removeItem("nexbox_splash_logo");
  };

  const handleCloseBehaviorChange = (value: string) => {
    setCloseBehavior(value);
    localStorage.setItem("nexbox_close_behavior", value);
    window.dispatchEvent(new CustomEvent("close-behavior-changed"));
  };

  const handleSidebarShowLabelChange = (value: string) => {
    const newValue = value === "true";
    setSidebarShowLabel(newValue);
    localStorage.setItem("nexbox_sidebar_show_label", String(newValue));
    window.dispatchEvent(new CustomEvent("sidebar-show-label-changed", { detail: newValue }));
  };

  const handleAutoStartToggle = () => {
    const newValue = !autoStart;
    invoke("set_nexbox_auto_start", { enable: newValue })
      .then(() => setAutoStart(newValue))
      .catch(() => {});
  };

  const handlePageTransitionChange = (newMode: "slide" | "fade" | "off") => {
    setPageTransitionMode(newMode);
    localStorage.setItem("nexbox_page_transition", newMode);
    window.dispatchEvent(new CustomEvent("page-transition-setting-changed", { detail: newMode }));
  };

  return (
    <Box>
      <Text fontSize="lg" fontWeight="bold" mb={6} color={titleColor}>
        {t("settings.generalSettings.title")}
      </Text>

      {/* 开机自启暂时隐藏 */}
      {/* <Box mb={6}>
        <Text
          fontSize="xs"
          fontWeight="semibold"
          color={subLabelColor}
          mb={3}
          textTransform="uppercase"
          letterSpacing="0.05em"
        >
          {t("settings.generalSettings.startup")}
        </Text>
        <LiquidGlassCard px={4} py={3} boxShadow="sm">
          <HStack justify="space-between" py={2}>
            <Text fontSize="sm" color={labelColor} fontWeight="medium">
              {t("settings.generalSettings.autoStartLabel")}
            </Text>
            <ThemeSwitch
              size="md"
              isChecked={autoStart}
              onChange={handleAutoStartToggle}
              isDisabled={autoStartLoading}
            />
          </HStack>
        </LiquidGlassCard>
      </Box> */}

      <Box mb={6}>
        <Text
          fontSize="xs"
          fontWeight="semibold"
          color={subLabelColor}
          mb={3}
          textTransform="uppercase"
          letterSpacing="0.05em"
        >
          {t("settings.generalSettings.language")}
        </Text>
        <LiquidGlassCard px={4} py={3} boxShadow="sm">
          <HStack justify="space-between">
            <Text fontSize="sm" color={labelColor}>
              {t("settings.generalSettings.languageLabel")}
            </Text>
            <CustomSelect
              value={language}
              onChange={handleLanguageChange}
              options={[
                { value: "zh", label: t("settings.generalSettings.languages.zh") },
                { value: "zh-TW", label: t("settings.generalSettings.languages.zh-TW") },
                { value: "en", label: t("settings.generalSettings.languages.en") },
                { value: "fr", label: t("settings.generalSettings.languages.fr") },
                { value: "ja", label: t("settings.generalSettings.languages.ja") },
                { value: "de", label: t("settings.generalSettings.languages.de") },
              ]}
              width="180px"
            />
          </HStack>
        </LiquidGlassCard>
      </Box>

      <Box mb={6}>
        <Text
          fontSize="xs"
          fontWeight="semibold"
          color={subLabelColor}
          mb={3}
          textTransform="uppercase"
          letterSpacing="0.05em"
        >
          {t("settings.generalSettings.homepage")}
        </Text>
        <LiquidGlassCard px={4} py={3} boxShadow="sm">
          <VStack spacing={0} align="stretch">
            <HStack justify="space-between" py={2}>
              <Box flex={1}>
                <Text fontSize="sm" color={labelColor} fontWeight="medium">
                  {t("settings.generalSettings.todayPopularityLabel")}
                </Text>
                <Text fontSize="xs" color={subLabelColor} mt={0.5}>
                  {t("settings.generalSettings.todayPopularityDesc")}
                </Text>
              </Box>
              <ThemeSwitch
                size="md"
                isChecked={todayPopularityEnabled}
                onChange={handleTodayPopularityToggle}
              />
            </HStack>
            <Divider />
            <HStack justify="space-between" py={2}>
              <Box flex={1}>
                <Text fontSize="sm" color={labelColor} fontWeight="medium">
                  {t("settings.generalSettings.announcementLabel")}
                </Text>
                <Text fontSize="xs" color={subLabelColor} mt={0.5}>
                  {t("settings.generalSettings.announcementDesc")}
                </Text>
              </Box>
              <ThemeSwitch
                size="md"
                isChecked={announcementEnabled}
                onChange={handleAnnouncementToggle}
              />
            </HStack>
            <Divider />
            <HStack justify="space-between" py={2}>
              <Box flex={1}>
                <Text fontSize="sm" color={labelColor} fontWeight="medium">
                  {t("settings.generalSettings.randomQuoteLabel")}
                </Text>
                <Text fontSize="xs" color={subLabelColor} mt={0.5}>
                  {t("settings.generalSettings.randomQuoteDesc")}
                </Text>
              </Box>
              <ThemeSwitch
                size="md"
                isChecked={randomQuoteEnabled}
                onChange={handleRandomQuoteToggle}
              />
            </HStack>
            <Divider />
            <HStack justify="space-between" py={2}>
              <Box flex={1}>
                <Text fontSize="sm" color={labelColor} fontWeight="medium">
                  {t("settings.generalSettings.gameLauncherLabel")}
                </Text>
                <Text fontSize="xs" color={subLabelColor} mt={0.5}>
                  {t("settings.generalSettings.gameLauncherDesc")}
                </Text>
              </Box>
              <ThemeSwitch
                size="md"
                isChecked={gameLauncherEnabled}
                onChange={handleGameLauncherToggle}
              />
            </HStack>
            <Divider />
            <HStack justify="space-between" py={2}>
              <Box flex={1}>
                <Text fontSize="sm" color={labelColor} fontWeight="medium">
                  {t("settings.generalSettings.homeHardwareModelLabel")}
                </Text>
                <Text fontSize="xs" color={subLabelColor} mt={0.5}>
                  {t("settings.generalSettings.homeHardwareModelDesc")}
                </Text>
              </Box>
              <ThemeSwitch
                size="md"
                isChecked={homeHardwareModelEnabled}
                onChange={handleHomeHardwareModelToggle}
              />
            </HStack>
          </VStack>
        </LiquidGlassCard>
      </Box>

      <Box mb={6}>
        <Text
          fontSize="xs"
          fontWeight="semibold"
          color={subLabelColor}
          mb={3}
          textTransform="uppercase"
          letterSpacing="0.05em"
        >
          {t("settings.generalSettings.splash")}
        </Text>
        <LiquidGlassCard px={4} py={3} boxShadow="sm">
          <VStack spacing={0} align="stretch">
            <HStack justify="space-between" align="center" py={2}>
              <Box flex={1}>
                <Text fontSize="sm" color={labelColor} fontWeight="medium">
                  {t("settings.generalSettings.splashLogoLabel")}
                </Text>
                <Text fontSize="xs" color={subLabelColor} mt={0.5}>
                  {t("settings.generalSettings.splashLogoDesc")}
                </Text>
                {splashLogo && (
                  <Button
                    size="xs"
                    variant="ghost"
                    mt={1.5}
                    onClick={handleSplashLogoReset}
                    leftIcon={<LuX size={12} />}
                    color={subLabelColor}
                    _hover={{ color: "red.400" }}
                  >
                    {t("settings.generalSettings.splashLogoReset")}
                  </Button>
                )}
              </Box>
              <Box
                w="48px"
                h="48px"
                borderRadius="md"
                overflow="hidden"
                border="1px solid"
                borderColor={cardBorder}
                cursor="pointer"
                onClick={() => {
                  const input = document.getElementById("splash-logo-upload") as HTMLInputElement;
                  input?.click();
                }}
                _hover={{ borderColor: splashLogoHoverBorder }}
                transition="all 0.2s"
                flexShrink={0}
              >
                <img
                  src={splashLogo || "/logo/Chinesew.png"}
                  alt="Splash Logo"
                  style={{ width: "100%", height: "100%", objectFit: "contain" }}
                />
              </Box>
            </HStack>
            <Divider />
            <HStack justify="space-between" py={2}>
              <Box flex={1}>
                <Text fontSize="sm" color={labelColor} fontWeight="medium">
                  {t("settings.generalSettings.pageTransitionLabel")}
                </Text>
                <Text fontSize="xs" color={subLabelColor} mt={0.5}>
                  {t("settings.generalSettings.pageTransitionDesc")}
                </Text>
              </Box>
              <HStack
                spacing={1}
                p={1}
                borderRadius="xl"
                border="1px solid"
                borderColor={segmentedControlBorder}
                bg={segmentedControlBg}
                boxShadow={segmentedControlShadow}
                backdropFilter={liquidGlassEnabled ? "blur(18px) saturate(160%)" : "blur(14px)"}
                position="relative"
                overflow="hidden"
              >
                {liquidGlassEnabled && (
                  <Box
                    position="absolute"
                    inset="0"
                    pointerEvents="none"
                    bgGradient={segmentedGlassSheen}
                    opacity={0.9}
                  />
                )}
                <Box
                  position="absolute"
                  inset="0"
                  pointerEvents="none"
                  borderRadius="inherit"
                  boxShadow={`inset 0 1px 0 rgba(255,255,255,0.28), inset 0 0 0 1px ${segmentedContainerGlow}`}
                  opacity={liquidGlassEnabled ? 1 : 0.72}
                />
                {(["slide", "fade", "off"] as const).map((mode) => (
                  <Box
                    key={mode}
                    as="button"
                    type="button"
                    minW="74px"
                    px={3.5}
                    py={2}
                    borderRadius="lg"
                    border="1px solid"
                    borderColor={pageTransitionMode === mode ? segmentedActiveBorder : "transparent"}
                    bg={pageTransitionMode === mode ? segmentedActiveBg : "transparent"}
                    color={pageTransitionMode === mode ? segmentedActiveText : subLabelColor}
                    fontSize="sm"
                    fontWeight={pageTransitionMode === mode ? "semibold" : "medium"}
                    letterSpacing="0.01em"
                    boxShadow={pageTransitionMode === mode ? segmentedActiveShadow : "none"}
                    position="relative"
                    transition="color 0.16s ease, transform 0.16s ease"
                    transform={pageTransitionMode === mode ? "translateY(-1px)" : "translateY(0)"}
                    _hover={{
                      bg: pageTransitionMode === mode ? segmentedActiveBg : segmentedControlHoverBg,
                      color: pageTransitionMode === mode ? segmentedActiveText : labelColor,
                    }}
                    _active={{
                      transform: pageTransitionMode === mode ? "translateY(0)" : "scale(0.98)",
                    }}
                    _focusVisible={{
                      outline: "none",
                      boxShadow: `0 0 0 3px ${hexToRgba(config.primaryColor, 0.24)}`,
                    }}
                    aria-pressed={pageTransitionMode === mode}
                    onClick={() => handlePageTransitionChange(mode)}
                  >
                    <Box
                      position="absolute"
                      inset="1px"
                      borderRadius="inherit"
                      opacity={pageTransitionMode === mode ? 1 : 0}
                      transition="none"
                      pointerEvents="none"
                      bgGradient={segmentedActiveOverlayGradient}
                    />
                    <Text position="relative" zIndex={1}>
                      {mode === "slide" ? t("settings.generalSettings.pageTransitionSlide", "滑动") :
                       mode === "fade" ? t("settings.generalSettings.pageTransitionFade", "淡化") :
                       t("settings.generalSettings.pageTransitionOff", "关闭")}
                    </Text>
                  </Box>
                ))}
              </HStack>
            </HStack>
          </VStack>
          <input
            id="splash-logo-upload"
            type="file"
            accept="image/*"
            style={{ display: "none" }}
            onChange={handleSplashLogoUpload}
          />
        </LiquidGlassCard>
      </Box>

      <Box mb={6}>
        <Text
          fontSize="xs"
          fontWeight="semibold"
          color={subLabelColor}
          mb={3}
          textTransform="uppercase"
          letterSpacing="0.05em"
        >
          {t("settings.generalSettings.closeBehavior")}
        </Text>
        <LiquidGlassCard px={4} py={3} boxShadow="sm">
          <HStack justify="space-between" py={2}>
            <Box flex={1}>
              <Text fontSize="sm" color={labelColor} fontWeight="medium">
                {t("settings.generalSettings.closeBehaviorLabel")}
              </Text>
              <Text fontSize="xs" color={subLabelColor} mt={0.5}>
                {t("settings.generalSettings.closeBehaviorDesc")}
              </Text>
            </Box>
            <CustomSelect
              value={closeBehavior}
              onChange={handleCloseBehaviorChange}
              options={[
                { value: "close", label: t("settings.generalSettings.closeDirectly") },
                { value: "minimize", label: t("settings.generalSettings.minimizeToTray") },
              ]}
              width="140px"
            />
          </HStack>
        </LiquidGlassCard>
      </Box>

      <Box mb={6}>
        <Text
          fontSize="xs"
          fontWeight="semibold"
          color={subLabelColor}
          mb={3}
          textTransform="uppercase"
          letterSpacing="0.05em"
        >
          {t("settings.navigation")}
        </Text>
        <LiquidGlassCard px={4} py={3} boxShadow="sm">
          <HStack justify="space-between">
            <Box flex={1}>
              <Text fontSize="sm" color={labelColor} fontWeight="medium">
                {t("settings.generalSettings.sidebarShowLabel")}
              </Text>
              <Text fontSize="xs" color={subLabelColor} mt={0.5}>
                {t("settings.generalSettings.sidebarShowLabelDesc")}
              </Text>
            </Box>
            <CustomSelect
              value={String(sidebarShowLabel)}
              onChange={handleSidebarShowLabelChange}
              options={[
                { value: "false", label: t("settings.generalSettings.sidebarShowLabelNoText") },
                { value: "true", label: t("settings.generalSettings.sidebarShowLabelWithText") },
              ]}
              width="100px"
            />
          </HStack>
        </LiquidGlassCard>
      </Box>
    </Box>
  );
}

function ThemeColorSettings() {
  const { t } = useTranslation();
  const {
    config,
    setPrimaryColor,
    resetToDefault,
  } = useThemeColor();
  
  const labelColor = useColorModeValue("gray.700", "#e0e0e0");
  const subLabelColor = useColorModeValue("gray.500", "#888888");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const presetBorderColor = useColorModeValue("gray.200", "#444444");
  const presetActiveBorderColor = useColorModeValue("gray.400", "#666666");
  
  const handlePresetClick = (color: string) => {
    setPrimaryColor(color);
  };
  
  return (
    <Box mb={6}>
      <Text
        fontSize="xs"
        fontWeight="semibold"
        color={subLabelColor}
        mb={3}
        textTransform="uppercase"
        letterSpacing="0.05em"
      >
        {t("settings.appearanceSettings.themeColor")}
      </Text>
      <LiquidGlassCard px={4} py={3} boxShadow="sm">
        <VStack spacing={4} align="stretch">
          <Box>
            <HStack mb={2}>
              <LuPalette size={14} />
              <Text fontSize="sm" color={labelColor} fontWeight="medium">
                {t("settings.appearanceSettings.presets")}
              </Text>
            </HStack>
            <HStack spacing={2} flexWrap="wrap">
              {PRESET_COLORS.map((preset) => (
                <Box
                  key={preset.value}
                  w="32px"
                  h="32px"
                  borderRadius="lg"
                  bg={preset.value}
                  cursor="pointer"
                  border="2px solid"
                  borderColor={config.primaryColor === preset.value ? presetActiveBorderColor : presetBorderColor}
                  boxShadow={config.primaryColor === preset.value ? "0 0 0 2px rgba(255,255,255,0.2)" : "none"}
                  onClick={() => handlePresetClick(preset.value)}
                  transition="all 0.2s"
                  _hover={{ transform: "scale(1.1)" }}
                  title={t(preset.labelKey)}
                />
              ))}
            </HStack>
          </Box>
          
          <Divider borderColor={cardBorder} />
          
          <Box>
            <Text fontSize="xs" color={subLabelColor} mb={2}>
              {t("settings.appearanceSettings.customColor")}
            </Text>
            <CustomColorPicker color={config.primaryColor} onChange={setPrimaryColor} />
          </Box>
          
          <HStack justify="flex-end">
            <Button
              size="xs"
              variant="ghost"
              onClick={resetToDefault}
            >
              {t("settings.appearanceSettings.resetToDefault")}
            </Button>
          </HStack>
        </VStack>
      </LiquidGlassCard>
    </Box>
  );
}

function AppearanceSettings() {
  const { t } = useTranslation();
  const { colorMode, toggleColorMode } = useColorMode();
  const {
    backgroundMode,
    customBgImages,
    activeBgIndex,
    dynamicBgVideo,
    setBackgroundMode,
    addCustomBgImage,
    removeCustomBgImage,
    setActiveBgIndex,
    setDynamicBgVideo,
    liquidGlassEnabled,
    setLiquidGlassEnabled,
    activePresetIndex,
    presetBackgrounds,
    setActivePresetIndex,
    carouselEnabled,
    setCarouselEnabled,
  } = useBackground();
  const videoPreviewSrc = useMemo(() => dynamicBgVideo ? convertFileSrc(dynamicBgVideo) : null, [dynamicBgVideo]);
  const titleColor = useColorModeValue("gray.800", "#ffffff");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const labelColor = useColorModeValue("gray.700", "#e0e0e0");
  const subLabelColor = useColorModeValue("gray.500", "#888888");
  const emptySlotBg = useColorModeValue("gray.100", "#1a1a1a");
  const emptySlotBorder = useColorModeValue("gray.200", "#333333");
  const activeSlotBorder = useColorModeValue("blue.400", "blue.300");
  const modeButtonActiveBg = useColorModeValue("blue.500", "blue.400");
  const modeButtonInactiveBg = useColorModeValue("gray.100", "#1a1a1a");
  const modeButtonInactiveBorder = useColorModeValue("gray.200", "#333333");
  const toast = useToast();

  const themeOptions = [
    { value: "light", label: "浅色" },
    { value: "dark", label: "深色" },
  ];

  const handleThemeChange = (value: string) => {
    if (
      (value === "dark" && colorMode === "light") ||
      (value === "light" && colorMode === "dark")
    ) {
      toggleColorMode();
    }
  };

  const handleImageUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      const reader = new FileReader();
      reader.onloadend = () => {
        const result = reader.result as string;
        const success = addCustomBgImage(result);
        if (!success) {
          toast({
            title: t("settings.appearanceSettings.maxImagesReached") || "最多只能添加3张背景图片",
            status: "warning",
            duration: 2000,
            isClosable: true,
          });
        }
      };
      reader.readAsDataURL(file);
    }
    e.target.value = "";
  };

  const handleVideoUpload = async () => {
    try {
      const filePath = await invoke<string | null>("pick_video_file");
      if (filePath) {
        setDynamicBgVideo(filePath);
      }
    } catch (error) {
      console.error("选择视频文件失败:", error);
    }
  };

  const handleAddClick = () => {
    if (customBgImages.length >= 3) {
      toast({
        title: t("settings.appearanceSettings.maxImagesReached") || "最多只能添加3张背景图片",
        status: "warning",
        duration: 2000,
        isClosable: true,
      });
      return;
    }
    const input = document.getElementById("bg-upload-new") as HTMLInputElement;
    input?.click();
  };

  const handleVideoAddClick = () => {
    handleVideoUpload();
  };

  return (
    <Box>
      <Text fontSize="lg" fontWeight="bold" mb={6} color={titleColor}>
        {t("settings.appearanceSettings.title")}
      </Text>

      <Box mb={6}>
        <Text
          fontSize="xs"
          fontWeight="semibold"
          color={subLabelColor}
          mb={3}
          textTransform="uppercase"
          letterSpacing="0.05em"
        >
          {t("settings.appearanceSettings.theme")}
        </Text>
        <LiquidGlassCard px={4} py={3} boxShadow="sm">
          <HStack justify="space-between">
            <Text fontSize="sm" color={labelColor}>
              {t("settings.appearanceSettings.themeStyle")}
            </Text>
            <CustomSelect
              value={colorMode}
              onChange={handleThemeChange}
              options={themeOptions}
              width="140px"
            />
          </HStack>
        </LiquidGlassCard>
      </Box>

      <Box mb={6}>
        <Text
          fontSize="xs"
          fontWeight="semibold"
          color={subLabelColor}
          mb={3}
          textTransform="uppercase"
          letterSpacing="0.05em"
        >
          {t("settings.appearanceSettings.liquidGlass")}
          <Badge
            ml={2}
            fontSize="0.6rem"
            colorScheme="purple"
            variant="subtle"
            px={2}
            py={0.5}
            borderRadius="full"
          >
            BETA
          </Badge>
        </Text>
        <LiquidGlassCard px={4} py={3} boxShadow="sm">
          <HStack justify="space-between">
            <Box flex={1}>
              <Text fontSize="sm" color={labelColor} fontWeight="medium">
                {t("settings.appearanceSettings.liquidGlassLabel")}
              </Text>
              <Text fontSize="xs" color={subLabelColor} mt={0.5}>
                {t("settings.appearanceSettings.liquidGlassDesc")}
              </Text>
            </Box>
            <ThemeSwitch
              size="md"
              isChecked={liquidGlassEnabled}
              onChange={() => setLiquidGlassEnabled(!liquidGlassEnabled)}
            />
          </HStack>
        </LiquidGlassCard>
      </Box>

      <ThemeColorSettings />

      <Box mb={6}>
        <Text
          fontSize="xs"
          fontWeight="semibold"
          color={subLabelColor}
          mb={3}
          textTransform="uppercase"
          letterSpacing="0.05em"
        >
          {t("settings.appearanceSettings.customBackground")}
        </Text>
        <LiquidGlassCard px={4} py={3} boxShadow="sm">
          <VStack spacing={4} align="stretch">
            <HStack justify="space-between">
              <Text fontSize="sm" color={labelColor} fontWeight="medium">
                {t("settings.appearanceSettings.customBackgroundLabel")}
              </Text>
              <ThemeSwitch
                size="md"
                isChecked={backgroundMode !== "none"}
                onChange={() => {
                  if (backgroundMode === "none") {
                    setBackgroundMode("image");
                  } else {
                    setBackgroundMode("none");
                  }
                }}
              />
            </HStack>

            {backgroundMode !== "none" && (
              <>
                <Divider borderColor={cardBorder} />

                <HStack spacing={2}>
                  <LiquidGlassCard
                    flex={1}
                    px={3}
                    py={2}
                    cursor="pointer"
                    onClick={() => setBackgroundMode("preset")}
                  >
                    <Text
                      fontSize="sm"
                      color={backgroundMode === "preset" ? "black" : labelColor}
                      textAlign="center"
                      fontWeight="medium"
                    >
                      {t("settings.appearanceSettings.presetBackground")}
                    </Text>
                  </LiquidGlassCard>
                  <LiquidGlassCard
                    flex={1}
                    px={3}
                    py={2}
                    cursor="pointer"
                    onClick={() => setBackgroundMode("image")}
                  >
                    <Text
                      fontSize="sm"
                      color={backgroundMode === "image" ? "black" : labelColor}
                      textAlign="center"
                      fontWeight="medium"
                    >
                      {t("settings.appearanceSettings.imageBackground")}
                    </Text>
                  </LiquidGlassCard>
                  <LiquidGlassCard
                    flex={1}
                    px={3}
                    py={2}
                    cursor="pointer"
                    onClick={() => setBackgroundMode("dynamic")}
                  >
                    <Text
                      fontSize="sm"
                      color={backgroundMode === "dynamic" ? "black" : labelColor}
                      textAlign="center"
                      fontWeight="medium"
                    >
                      {t("settings.appearanceSettings.dynamicBackground")}
                    </Text>
                  </LiquidGlassCard>
                </HStack>

                {backgroundMode === "preset" && (
                  <>
                    <HStack justify="space-between" w="full">
                      <Box>
                        <Text fontSize="sm" color={labelColor} fontWeight="medium">
                          {t("settings.appearanceSettings.carouselLabel")}
                        </Text>
                        <Text fontSize="xs" color={subLabelColor} mt={0.5}>
                          {t("settings.appearanceSettings.carouselDesc")}
                        </Text>
                      </Box>
                      <ThemeSwitch
                        size="md"
                        isChecked={carouselEnabled}
                        onChange={() => setCarouselEnabled(!carouselEnabled)}
                      />
                    </HStack>
                    <HStack spacing={2} justify="flex-end">
                      {presetBackgrounds.map((preset, index) => (
                        <Box
                          key={preset.id}
                          position="relative"
                          w="160px"
                          h="90px"
                          borderRadius="lg"
                          overflow="hidden"
                          border="2px solid"
                          borderColor={index === activePresetIndex ? activeSlotBorder : emptySlotBorder}
                          cursor="pointer"
                          onClick={() => setActivePresetIndex(index)}
                          transition="all 0.2s"
                          _hover={{ borderColor: activeSlotBorder, transform: "scale(1.02)" }}
                        >
                          <img
                            src={preset.path}
                            alt={preset.name}
                            style={{
                              width: "100%",
                              height: "100%",
                              objectFit: "cover",
                            }}
                          />
                          {index === activePresetIndex && (
                            <Box
                              position="absolute"
                              bottom={1}
                              left="50%"
                              transform="translateX(-50%)"
                              bg="blue.500"
                              borderRadius="full"
                              px={1.5}
                              py={0.5}
                            >
                              <LuCheck size={10} color="white" />
                            </Box>
                          )}
                        </Box>
                      ))}
                    </HStack>
                  </>
                )}

                {backgroundMode === "image" && (
                  <HStack spacing={2} justify="flex-end">
                    {[0, 1, 2].map((index) => (
                      <Box
                        key={index}
                        position="relative"
                        w="160px"
                        h="90px"
                        borderRadius="lg"
                        overflow="hidden"
                        border="2px solid"
                        borderColor={
                          customBgImages[index]
                            ? index === activeBgIndex
                              ? activeSlotBorder
                              : cardBorder
                            : emptySlotBorder
                        }
                        cursor={customBgImages[index] ? "pointer" : "default"}
                        onClick={() => {
                          if (customBgImages[index]) {
                            setActiveBgIndex(index);
                          }
                        }}
                        transition="all 0.2s"
                        _hover={
                          customBgImages[index]
                            ? { borderColor: activeSlotBorder, transform: "scale(1.02)" }
                            : {}
                        }
                      >
                        {customBgImages[index] ? (
                          <>
                            <img
                              src={customBgImages[index]}
                              alt={`Background ${index + 1}`}
                              style={{
                                width: "100%",
                                height: "100%",
                                objectFit: "cover",
                              }}
                            />
                            <Box
                              position="absolute"
                              top={1}
                              right={1}
                              onClick={(e) => {
                                e.stopPropagation();
                                removeCustomBgImage(index);
                              }}
                              bg="blackAlpha.600"
                              borderRadius="full"
                              p={0.5}
                              cursor="pointer"
                              _hover={{ bg: "blackAlpha.800" }}
                              transition="all 0.2s"
                            >
                              <LuX size={12} color="white" />
                            </Box>
                            {index === activeBgIndex && (
                              <Box
                                position="absolute"
                                bottom={1}
                                left="50%"
                                transform="translateX(-50%)"
                                bg="blue.500"
                                borderRadius="full"
                                px={1.5}
                                py={0.5}
                              >
                                <LuCheck size={10} color="white" />
                              </Box>
                            )}
                          </>
                        ) : (
                          <Flex
                            w="100%"
                            h="100%"
                            bg={emptySlotBg}
                            align="center"
                            justify="center"
                            cursor="pointer"
                            onClick={handleAddClick}
                            _hover={{ bg: useColorModeValue("gray.200", "#222222") }}
                            transition="all 0.2s"
                          >
                            <LuUpload size={18} color={subLabelColor} />
                          </Flex>
                        )}
                      </Box>
                    ))}
                    <input
                      id="bg-upload-new"
                      type="file"
                      accept="image/*"
                      style={{ display: "none" }}
                      onChange={handleImageUpload}
                    />
                  </HStack>
                )}

                {backgroundMode === "dynamic" && (
                  <HStack spacing={2} justify="flex-end">
                    <Box
                      position="relative"
                      w="160px"
                      h="90px"
                      borderRadius="lg"
                      overflow="hidden"
                      border="2px solid"
                      borderColor={dynamicBgVideo ? activeSlotBorder : emptySlotBorder}
                    >
                      {dynamicBgVideo ? (
                        <>
                          <video
                            src={videoPreviewSrc!}
                            style={{
                              width: "100%",
                              height: "100%",
                              objectFit: "cover",
                            }}
                            muted
                            loop
                            autoPlay
                          />
                          <Box
                            position="absolute"
                            top={1}
                            right={1}
                            onClick={() => setDynamicBgVideo(null)}
                            bg="blackAlpha.600"
                            borderRadius="full"
                            p={0.5}
                            cursor="pointer"
                            _hover={{ bg: "blackAlpha.800" }}
                            transition="all 0.2s"
                          >
                            <LuX size={12} color="white" />
                          </Box>
                        </>
                      ) : (
                        <Flex
                          w="100%"
                          h="100%"
                          bg={emptySlotBg}
                          align="center"
                          justify="center"
                          cursor="pointer"
                          onClick={handleVideoAddClick}
                          _hover={{ bg: useColorModeValue("gray.200", "#222222") }}
                          transition="all 0.2s"
                          flexDirection="column"
                          gap={1}
                        >
                          <LuUpload size={20} color={subLabelColor} />
                          <Text fontSize="xs" color={subLabelColor}>
                            {t("settings.appearanceSettings.uploadVideo")}
                          </Text>
                        </Flex>
                      )}
                    </Box>
                  </HStack>
                )}
              </>
            )}
          </VStack>
        </LiquidGlassCard>
      </Box>
    </Box>
  );
}

function NetworkSettings() {
  const { t } = useTranslation();
  const titleColor = useColorModeValue("gray.800", "#ffffff");
  const labelColor = useColorModeValue("gray.700", "#e0e0e0");
  const subLabelColor = useColorModeValue("gray.500", "#888888");

  const servers = [
    { id: "baidu", url: "https://www.baidu.com/img/flexible/logo/pc/result.png" },
    { id: "gitee", url: "https://gitee.com/favicon.ico" },
    { id: "github", url: "https://github.githubassets.com/favicons/favicon-dark.svg" },
    { id: "qq", url: "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'%3E%3Crect width='32' height='32' rx='6' fill='%2312B7F5'/%3E%3Ctext x='16' y='22' text-anchor='middle' fill='white' font-size='14' font-weight='bold' font-family='Arial,sans-serif'%3EQQ%3C/text%3E%3C/svg%3E" },
    { id: "aliyun", url: "https://img.alicdn.com/tfs/TB1_ZXuNcfpK1RjSZFOXXa6nFXa-32-32.ico" },
    { id: "wangyi", url: "https://www.163.com/favicon.ico" },
    { id: "bilibili", url: "https://www.bilibili.com/favicon.ico" },
    { id: "douyin", url: "https://www.douyin.com/favicon.ico" },
    { id: "jd", url: "https://www.jd.com/favicon.ico" },
    { id: "zhihu", url: "https://www.zhihu.com/favicon.ico" },
  ];

  const [latencies, setLatencies] = useState<Record<string, number | null>>({});
  const [testing, setTesting] = useState<Record<string, boolean>>({});
  const [testingAll, setTestingAll] = useState(false);
  const [imgErrors, setImgErrors] = useState<Record<string, boolean>>({});

  const testLatency = async (serverId: string, serverUrl: string) => {
    setTesting((prev) => ({ ...prev, [serverId]: true }));
    const startTime = performance.now();
    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 5000);
      
      await fetch(serverUrl, {
        mode: "no-cors",
        signal: controller.signal,
      });
      
      clearTimeout(timeoutId);
      const endTime = performance.now();
      const latency = Math.round(endTime - startTime);
      setLatencies((prev) => ({ ...prev, [serverId]: latency }));
    } catch {
      const endTime = performance.now();
      const elapsed = Math.round(endTime - startTime);
      if (elapsed >= 5000) {
        setLatencies((prev) => ({ ...prev, [serverId]: -1 }));
      } else {
        setLatencies((prev) => ({ ...prev, [serverId]: -2 }));
      }
    } finally {
      setTesting((prev) => ({ ...prev, [serverId]: false }));
    }
  };

  const testAll = async () => {
    setTestingAll(true);
    const promises = servers.map((server) =>
      testLatency(server.id, server.url)
    );
    await Promise.all(promises);
    setTestingAll(false);
  };

  const getLatencyColor = (latency: number | null | undefined): string => {
    if (latency === undefined || latency === null) return subLabelColor;
    if (latency < 0) return "#e53e3e";
    if (latency < 100) return "#38a169";
    if (latency < 300) return "#d69e2e";
    return "#e53e3e";
  };

  const getLatencyText = (latency: number | null | undefined): string => {
    if (latency === undefined || latency === null) return "--";
    if (latency === -1) return t("settings.networkSettings.timeout");
    if (latency === -2) return t("settings.networkSettings.unreachable");
    return `${latency} ms`;
  };

  return (
    <Box>
      <HStack mb={6} justify="space-between" align="center">
        <Box>
          <Text fontSize="lg" fontWeight="bold" color={titleColor}>
            {t("settings.networkSettings.title")}
          </Text>
          <Text fontSize="sm" color={subLabelColor} mt={1}>
            {t("settings.networkSettings.description")}
          </Text>
        </Box>
        <LiquidGlassButton
          size="sm"
          variant="solid"
          borderRadius="lg"
          onClick={testAll}
          isDisabled={testingAll}
          leftIcon={testingAll ? <LuRefreshCw className="animate-spin" size={14} /> : <LuGlobe size={14} />}
        >
          {testingAll ? t("settings.networkSettings.testingAll") : t("settings.networkSettings.testAll")}
        </LiquidGlassButton>
      </HStack>

      <Box>
        <Text
          fontSize="xs"
          fontWeight="semibold"
          color={subLabelColor}
          mb={3}
          textTransform="uppercase"
          letterSpacing="0.05em"
        >
          {t("settings.networkSettings.latency")}
        </Text>
        <VStack spacing={3} align="stretch">
          {servers.map((server) => {
            const latency = latencies[server.id];
            const isTesting = testing[server.id];
            const latencyColor = getLatencyColor(latency);

            return (
              <LiquidGlassCard key={server.id} px={4} py={3} boxShadow="sm">
                <HStack justify="space-between" align="center">
                  <HStack spacing={3}>
                    <Box
                      w="32px"
                      h="32px"
                      borderRadius="md"
                      overflow="hidden"
                      flexShrink={0}
                      bg={useColorModeValue("gray.50", "#1a1a1a")}
                      display="flex"
                      alignItems="center"
                      justifyContent="center"
                      position="relative"
                    >
                      {!imgErrors[server.id] ? (
                        <img
                          src={server.url}
                          alt=""
                          style={{ width: "20px", height: "20px", objectFit: "contain" }}
                          onError={() => {
                            setImgErrors((prev) => ({ ...prev, [server.id]: true }));
                          }}
                        />
                      ) : (
                        <Text fontSize="sm" fontWeight="bold" color={subLabelColor}>
                          {t(`settings.networkSettings.servers.${server.id}`).charAt(0)}
                        </Text>
                      )}
                    </Box>
                    <Text fontSize="sm" color={labelColor} fontWeight="medium">
                      {t(`settings.networkSettings.servers.${server.id}`)}
                    </Text>
                  </HStack>
                  <HStack spacing={3}>
                    <Text
                      fontSize="sm"
                      fontWeight="bold"
                      color={latencyColor}
                      minW="60px"
                      textAlign="right"
                    >
                      {getLatencyText(latency)}
                    </Text>
                    <LiquidGlassButton
                      size="xs"
                      variant="outline"
                      borderRadius="md"
                      onClick={() => testLatency(server.id, server.url)}
                      isDisabled={isTesting || testingAll}
                    >
                      {isTesting ? t("settings.networkSettings.testing") : t("settings.networkSettings.testButton")}
                    </LiquidGlassButton>
                  </HStack>
                </HStack>
              </LiquidGlassCard>
            );
          })}
        </VStack>
      </Box>
    </Box>
  );
}

interface SponsorItem {
  name: string;
  amount: string;
}

function SponsorSettings() {
  const { t } = useTranslation();
  const titleColor = useColorModeValue("gray.800", "#ffffff");
  const labelColor = useColorModeValue("gray.700", "#e0e0e0");
  const subLabelColor = useColorModeValue("gray.500", "#888888");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const { getActiveColor, getContrastTextColor } = useThemeColor();
  const [sponsors, setSponsors] = useState<SponsorItem[]>([]);
  const [sponsorsLoading, setSponsorsLoading] = useState(true);
  const [sponsorsError, setSponsorsError] = useState(false);

  useEffect(() => {
    invoke<{ update_time: string; list: SponsorItem[] }>("get_sponsors")
      .then((data) => setSponsors(data.list))
      .catch(() => setSponsorsError(true))
      .finally(() => setSponsorsLoading(false));
  }, []);

  return (
    <Box>
      <Text fontSize="lg" fontWeight="bold" mb={2} color={titleColor}>
        {t("settings.sponsorSettings.title")}
      </Text>
      <Text fontSize="sm" color={subLabelColor} mb={6}>
        {t("settings.sponsorSettings.description")}
      </Text>

      <HStack spacing={6} align="stretch" justify="center">
        <LiquidGlassCard p={6} boxShadow="sm" textAlign="center" maxW="240px">
          <VStack spacing={4}>
            <Box
              w="180px"
              h="180px"
              borderRadius="xl"
              overflow="hidden"
              border="1px solid"
              borderColor={cardBorder}
              bg={useColorModeValue("white", "#1a1a1a")}
              display="flex"
              alignItems="center"
              justifyContent="center"
            >
              <img
                src="/sponsor/wechat.png"
                alt={t("settings.sponsorSettings.wechat")}
                style={{ width: "100%", height: "100%", objectFit: "contain" }}
                onError={(e) => {
                  const target = e.target as HTMLImageElement;
                  target.style.display = "none";
                  target.parentElement!.innerHTML = `<span style="color: ${subLabelColor}; font-size: 12px;">${t("settings.sponsorSettings.placeholder")}</span>`;
                }}
              />
            </Box>
            <Text fontSize="sm" fontWeight="medium" color={labelColor}>
              {t("settings.sponsorSettings.wechat")}
            </Text>
          </VStack>
        </LiquidGlassCard>

        <LiquidGlassCard p={6} boxShadow="sm" textAlign="center" maxW="240px">
          <VStack spacing={4}>
            <Box
              w="180px"
              h="180px"
              borderRadius="xl"
              overflow="hidden"
              border="1px solid"
              borderColor={cardBorder}
              bg={useColorModeValue("white", "#1a1a1a")}
              display="flex"
              alignItems="center"
              justifyContent="center"
            >
              <img
                src="/sponsor/alipay.png"
                alt={t("settings.sponsorSettings.alipay")}
                style={{ width: "100%", height: "100%", objectFit: "contain" }}
                onError={(e) => {
                  const target = e.target as HTMLImageElement;
                  target.style.display = "none";
                  target.parentElement!.innerHTML = `<span style="color: ${subLabelColor}; font-size: 12px;">${t("settings.sponsorSettings.placeholder")}</span>`;
                }}
              />
            </Box>
            <Text fontSize="sm" fontWeight="medium" color={labelColor}>
              {t("settings.sponsorSettings.alipay")}
            </Text>
          </VStack>
        </LiquidGlassCard>
      </HStack>

      <Text fontSize="sm" color={subLabelColor} mt={6} textAlign="center">
        {t("settings.sponsorSettings.thankYou")}
      </Text>

      <Box mt={8}>
        <Text fontSize="lg" fontWeight="bold" mb={4} color={titleColor}>
          {t("settings.sponsorSettings.sponsorList.title")}
        </Text>
        {sponsorsLoading ? (
          <Text fontSize="sm" color={subLabelColor} p={4} textAlign="center">
            {t("settings.sponsorSettings.sponsorList.loading")}
          </Text>
        ) : sponsorsError ? (
          <Text fontSize="sm" color={subLabelColor} p={4} textAlign="center">
            {t("settings.sponsorSettings.sponsorList.error")}
          </Text>
        ) : sponsors.length === 0 ? (
          <Text fontSize="sm" color={subLabelColor} p={4} textAlign="center">
            {t("settings.sponsorSettings.sponsorList.empty")}
          </Text>
        ) : (
          <Flex flexWrap="wrap" gap={4} justify="center">
            {sponsors.map((sponsor, index) => (
              <LiquidGlassCard
                key={index}
                p={5}
                textAlign="center"
                minW="140px"
                flex="0 1 auto"
              >
                <Text fontSize="md" fontWeight="medium" color={labelColor} mb={1}>
                  {sponsor.name}
                </Text>
                <Box
                  display="inline-block"
                  px={3}
                  py={1}
                  borderRadius="lg"
                  bg={getActiveColor()}
                  color={getContrastTextColor()}
                  fontSize="sm"
                  fontWeight="medium"
                >
                  {sponsor.amount}
                </Box>
              </LiquidGlassCard>
            ))}
          </Flex>
        )}
      </Box>
    </Box>
  );
}

function AboutSettings() {
  const { t } = useTranslation();
  const toast = useToast();
  const titleColor = useColorModeValue("gray.800", "#ffffff");
  const labelColor = useColorModeValue("gray.700", "#e0e0e0");
  const subLabelColor = useColorModeValue("gray.500", "#888888");
  const dividerColor = useColorModeValue("gray.200", "#333333");
  const appNameColor = useColorModeValue("gray.400", "#888888");
  const logoSrc = useColorModeValue("/logo/NexBoxW.png", "/logo/NexBoxB.png");
  const modalBg = useColorModeValue("white", "#111111");
  const modalBorderColor = useColorModeValue("gray.200", "#333333");

  const currentVersion = "4.2.6";
  const [isChecking, setIsChecking] = useState(false);
  const [latestRelease, setLatestRelease] = useState<GiteeRelease | null>(null);
  const [isUpdateAvailable, setIsUpdateAvailable] = useState(false);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [isDownloading, setIsDownloading] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [isDownloadComplete, setIsDownloadComplete] = useState(false);
  const [downloadedFilePath, setDownloadedFilePath] = useState<string>("");
  const [currentRelease, setCurrentRelease] = useState<GiteeRelease | null>(null);
  const [isLoadingChangelog, setIsLoadingChangelog] = useState(true);

  useEffect(() => {
    const unlisten = listen<{ progress: number; total: number }>("download-progress", (event) => {
      setDownloadProgress(event.payload.progress);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const checkUpdateSilently = async () => {
      try {
        const release = await fetchLatestRelease();
        if (release) {
          setLatestRelease(release);
          const hasUpdate = compareVersions(currentVersion, release.tag_name);
          setIsUpdateAvailable(hasUpdate);
        }
      } catch (error) {
        console.error("Failed to check for updates silently:", error);
      }
    };

    const fetchCurrentRelease = async () => {
      try {
        setIsLoadingChangelog(true);
        const release = await fetchReleaseByTag(`v${currentVersion}`);
        if (release) {
          setCurrentRelease(release);
        } else {
          // 尝试不带 v 前缀
          const releaseWithoutV = await fetchReleaseByTag(currentVersion);
          setCurrentRelease(releaseWithoutV);
        }
      } catch (error) {
        console.error("Failed to fetch current release:", error);
      } finally {
        setIsLoadingChangelog(false);
      }
    };

    checkUpdateSilently();
    fetchCurrentRelease();
  }, []);

  const handleOpenLink = async (url: string) => {
    try {
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(url);
    } catch (error) {
      console.error("Failed to open link:", error);
    }
  };

  const handleCheckUpdate = async () => {
    setIsChecking(true);
    try {
      const release = await fetchLatestRelease();
      if (release) {
        setLatestRelease(release);
        const hasUpdate = compareVersions(currentVersion, release.tag_name);
        setIsUpdateAvailable(hasUpdate);
        if (hasUpdate) {
          setIsModalOpen(true);
        } else {
          toast({
            title: t("settings.aboutSettings.noUpdate") || "已是最新版本",
            status: "success",
            duration: 2000,
            isClosable: true,
          });
        }
      } else {
        toast({
          title: t("settings.aboutSettings.checkFailed") || "检查失败",
          status: "error",
          duration: 2000,
          isClosable: true,
        });
      }
    } catch (error) {
      console.error("Failed to check for updates:", error);
      toast({
        title: t("settings.aboutSettings.checkFailed") || "检查失败",
        status: "error",
        duration: 2000,
        isClosable: true,
      });
    } finally {
      setIsChecking(false);
    }
  };

  const handleDownload = async () => {
    if (!latestRelease) return;
    
    setIsDownloading(true);
    setDownloadProgress(0);
    setIsDownloadComplete(false);
    
    try {
      const asset = latestRelease.assets.find(a => 
        a.name.endsWith('.msi') || a.name.endsWith('.exe')
      );
      
      if (asset) {
        const filePath = await invoke<string>("download_update", {
          url: asset.browser_download_url,
          fileName: asset.name,
        });
        setDownloadedFilePath(filePath);
        setIsDownloadComplete(true);
      } else {
        await handleOpenLink(latestRelease.html_url);
        setIsDownloading(false);
      }
    } catch (error) {
      console.error("Failed to download:", error);
      toast({
        title: t("settings.aboutSettings.downloadFailed") || "下载失败",
        status: "error",
        duration: 2000,
        isClosable: true,
      });
      setIsDownloading(false);
    }
  };

  const handleInstall = async () => {
    if (!downloadedFilePath) return;
    
    try {
      await invoke("install_update", {
        filePath: downloadedFilePath,
      });
    } catch (error) {
      console.error("Failed to install:", error);
      toast({
        title: t("settings.aboutSettings.installFailed") || "安装失败",
        status: "error",
        duration: 2000,
        isClosable: true,
      });
    }
  };

  const handleSkip = async () => {
    if (downloadedFilePath) {
      try {
        await invoke("delete_download_file", {
          filePath: downloadedFilePath,
        });
      } catch (error) {
        console.error("Failed to delete file:", error);
      }
    }
    setIsModalOpen(false);
    setIsDownloadComplete(false);
    setDownloadedFilePath("");
    setIsDownloading(false);
  };

  return (
    <Box>
      <Text fontSize="lg" fontWeight="bold" mb={6} color={titleColor}>
        {t("settings.aboutSettings.title")}
      </Text>

      <LiquidGlassCard p={6} boxShadow="sm" mb={6}>
        <HStack mb={4}>
          <Box
            w="56px"
            h="56px"
            borderRadius="xl"
            flexShrink={0}
            boxShadow="md"
            overflow="hidden"
          >
            <img
              src={logoSrc}
              alt="NexBox Logo"
              style={{ width: "100%", height: "100%", objectFit: "cover" }}
            />
          </Box>
          <VStack align="start" spacing={1} ml={2}>
            <Text fontSize="2xl" fontWeight="bold" color={labelColor}>
              {t("common.appFullName")}
            </Text>
            <Text fontSize="sm" color={appNameColor}>
              {t("common.appName")}
            </Text>
          </VStack>
        </HStack>

        <Divider my={4} borderColor={dividerColor} />

        <Box>
          <HStack justify="space-between" mb={3}>
            <Text fontSize="sm" color={subLabelColor}>
              {t("settings.aboutSettings.version")}
            </Text>
            <HStack spacing={2}>
              <Text fontSize="sm" color={labelColor} fontWeight="medium">
                v{currentVersion}
              </Text>
              {isUpdateAvailable ? (
                <LiquidGlassButton
                  size="xs"
                  variant="solid"
                  borderRadius="lg"
                  colorScheme="orange"
                  onClick={() => setIsModalOpen(true)}
                  leftIcon={<LuDownload size={12} />}
                >
                  {t("settings.aboutSettings.newVersion")}
                </LiquidGlassButton>
              ) : (
                <LiquidGlassButton
                  size="xs"
                  variant="solid"
                  borderRadius="lg"
                  colorScheme="teal"
                  onClick={handleCheckUpdate}
                  isDisabled={isChecking}
                  leftIcon={isChecking ? <LuRefreshCw className="animate-spin" size={12} /> : undefined}
                >
                  {isChecking ? t("settings.aboutSettings.check") + "..." : t("settings.aboutSettings.check")}
                </LiquidGlassButton>
              )}
            </HStack>
          </HStack>
          <HStack justify="space-between">
            <Text fontSize="sm" color={subLabelColor}>
              {t("settings.aboutSettings.author")}
            </Text>
            <HStack spacing={2}>
              <Box
                w="24px"
                h="24px"
                borderRadius="md"
                display="flex"
                alignItems="center"
                justifyContent="center"
                color="#00A1D6"
                cursor="pointer"
                transition="all 0.2s"
                _hover={{ bg: "rgba(0, 161, 214, 0.1)", transform: "scale(1.1)" }}
                onClick={() => handleOpenLink("https://space.bilibili.com/1614951812")}
                title="Bilibili"
              >
                <RiBilibiliFill size={18} />
              </Box>
              <Box
                w="24px"
                h="24px"
                borderRadius="md"
                display="flex"
                alignItems="center"
                justifyContent="center"
                color="#000000"
                cursor="pointer"
                transition="all 0.2s"
                _hover={{ bg: "rgba(0, 0, 0, 0.1)", transform: "scale(1.1)" }}
                onClick={() => handleOpenLink("https://www.douyin.com/user/MS4wLjABAAAAytD1zP6zVeXgPQuG-PWHq4AhsZz9zNXPcJap2JVaoG88Ani9tmBj0FtH7DLrQWsH")}
                title="抖音"
              >
                <RiTiktokFill size={16} />
              </Box>
              <Text fontSize="sm" color={labelColor} fontWeight="medium">
                木流
              </Text>
            </HStack>
          </HStack>
          <Divider my={3} borderColor={dividerColor} />
          <HStack justify="space-between">
            <Text fontSize="sm" color={subLabelColor}>
              QQ 交流群
            </Text>
            <HStack spacing={2}>
              <Box
                w="24px"
                h="24px"
                borderRadius="md"
                display="flex"
                alignItems="center"
                justifyContent="center"
                cursor="pointer"
                transition="all 0.2s"
                _hover={{ transform: "scale(1.1)" }}
                onClick={() => handleOpenLink("https://qm.qq.com/q/lhjntH1V1S")}
                title="点击加入 QQ 群"
              >
                <img
                  src="https://img.icons8.com/color/96/qq.png"
                  alt="QQ"
                  style={{ width: "18px", height: "18px", objectFit: "contain" }}
                  onError={(e) => {
                    // Fallback to inline SVG if image fails to load
                    const target = e.target as HTMLImageElement;
                    target.style.display = "none";
                    const parent = target.parentElement!;
                    parent.innerHTML = `
                      <svg viewBox="0 0 24 24" width="18" height="18" fill="#12B7F5">
                        <path d="M12.002 2c-5.338 0-9.668 3.93-9.668 8.774 0 2.822 1.589 5.33 4.064 6.887l.113.072-.575 1.926c-.042.142.045.29.192.29.04 0 .081-.01.118-.03l2.485-1.347.201.012c.984.06 1.99.06 2.977 0l.202-.012 2.484 1.347c.038.02.079.03.119.03.146 0 .234-.148.192-.29l-.575-1.926.113-.072c2.476-1.557 4.065-4.065 4.065-6.887 0-4.845-4.33-8.774-9.668-8.774z"/>
                      </svg>
                    `;
                  }}
                />
              </Box>
              <Text fontSize="sm" color={labelColor} fontWeight="medium" userSelect="all">
                526045683
              </Text>
            </HStack>
          </HStack>
          <Divider my={3} borderColor={dividerColor} />
          <HStack justify="space-between">
            <Text fontSize="sm" color={subLabelColor}>
              {t("settings.aboutSettings.joinUs")}
            </Text>
            <HStack spacing={2}>
              <Box
                w="24px"
                h="24px"
                borderRadius="md"
                display="flex"
                alignItems="center"
                justifyContent="center"
                cursor="pointer"
                transition="all 0.2s"
                _hover={{ transform: "scale(1.1)", bg: "rgba(99, 102, 241, 0.1)" }}
                onClick={() => handleOpenLink("https://team.nexbox.top")}
                title={t("settings.aboutSettings.joinUs")}
              >
                <LuExternalLink size={16} />
              </Box>
            </HStack>
          </HStack>
        </Box>
      </LiquidGlassCard>

      <LiquidGlassCard p={6} boxShadow="sm" mb={6}>
        <Text fontSize="lg" fontWeight="bold" mb={4} color={titleColor}>
          {t("settings.aboutSettings.changelogTitle")}
        </Text>
        {isLoadingChangelog ? (
          <Box p={4} textAlign="center">
            <Text color={subLabelColor}>{t("settings.aboutSettings.loadingChangelog")}</Text>
          </Box>
        ) : currentRelease && currentRelease.body ? (
          <Box maxH="300px" overflowY="auto">
            <Text color={labelColor} fontSize="sm" whiteSpace="pre-wrap">
              {currentRelease.body}
            </Text>
          </Box>
        ) : (
          <Box p={4} textAlign="center">
            <Text color={subLabelColor}>{t("settings.aboutSettings.noChangelog")}</Text>
          </Box>
        )}
      </LiquidGlassCard>

      <Modal isOpen={isModalOpen} onClose={() => !isDownloading && !isDownloadComplete && setIsModalOpen(false)} isCentered closeOnOverlayClick={!isDownloading && !isDownloadComplete}>
        <ModalOverlay />
        <ModalContent bg={modalBg} borderColor={modalBorderColor} borderRadius="xl">
          <ModalHeader color={labelColor}>{t("settings.aboutSettings.updateModal.title")}</ModalHeader>
          {!isDownloading && !isDownloadComplete && <ModalCloseButton />}
          <ModalBody>
            <VStack align="start" spacing={4}>
              <HStack>
                <Text color={subLabelColor} fontSize="sm">
                  {t("settings.aboutSettings.updateModal.version")}:
                </Text>
                <Text color={labelColor} fontWeight="medium">
                  {latestRelease?.tag_name}
                </Text>
              </HStack>
              <Box w="full">
                <Text color={subLabelColor} fontSize="sm" mb={2}>
                  {t("settings.aboutSettings.updateModal.releaseNotes")}:
                </Text>
                <Box
                  p={3}
                  borderRadius="lg"
                  bg={useColorModeValue("gray.50", "#1a1a1a")}
                  maxH="200px"
                  overflowY="auto"
                >
                  <Text color={labelColor} fontSize="sm" whiteSpace="pre-wrap">
                    {latestRelease?.body || "无更新说明"}
                  </Text>
                </Box>
              </Box>
              {isDownloading && !isDownloadComplete && (
                <Box w="full">
                  <Progress value={downloadProgress} size="sm" colorScheme="teal" borderRadius="full" />
                  <Text color={subLabelColor} fontSize="xs" mt={1}>
                    {t("settings.aboutSettings.updateModal.downloading")} {downloadProgress}%
                  </Text>
                </Box>
              )}
              {isDownloadComplete && (
                <Box w="full" p={3} borderRadius="lg" bg={useColorModeValue("green.50", "rgba(72, 187, 120, 0.1)")} border="1px solid" borderColor={useColorModeValue("green.200", "rgba(72, 187, 120, 0.3)")}>
                  <Text color={useColorModeValue("green.600", "green.300")} fontSize="sm" fontWeight="medium">
                    {t("settings.aboutSettings.updateModal.downloadComplete")}
                  </Text>
                </Box>
              )}
            </VStack>
          </ModalBody>
          <ModalFooter>
            {isDownloadComplete ? (
              <>
                <Button variant="ghost" mr={3} onClick={handleSkip}>
                  {t("settings.aboutSettings.updateModal.skip")}
                </Button>
                <LiquidGlassButton
                  colorScheme="teal"
                  onClick={handleInstall}
                  leftIcon={<LuRefreshCw size={14} />}
                >
                  {t("settings.aboutSettings.updateModal.restartInstall")}
                </LiquidGlassButton>
              </>
            ) : (
              <>
                {!isDownloading && (
                  <Button variant="ghost" mr={3} onClick={() => setIsModalOpen(false)}>
                    {t("settings.aboutSettings.updateModal.cancel")}
                  </Button>
                )}
                <LiquidGlassButton
                  colorScheme="teal"
                  onClick={handleDownload}
                  isDisabled={isDownloading}
                  isLoading={isDownloading}
                  leftIcon={!isDownloading ? <LuDownload size={14} /> : undefined}
                >
                  {isDownloading ? t("settings.aboutSettings.updateModal.downloading") : t("settings.aboutSettings.updateModal.download")}
                </LiquidGlassButton>
              </>
            )}
          </ModalFooter>
        </ModalContent>
      </Modal>
    </Box>
  );
}

function HotkeySettings() {
  const { t } = useTranslation();
  const { overlayHotkey, saveOverlayHotkey, crosshairHotkey, saveCrosshairHotkey, filterHotkey, saveFilterHotkey, islandHotkey, saveIslandHotkey } = useAppStartup();
  const toast = useToast();
  const titleColor = useColorModeValue("gray.800", "#ffffff");
  const labelColor = useColorModeValue("gray.700", "#e0e0e0");
  const subLabelColor = useColorModeValue("gray.500", "#888888");

  return (
    <Box>
      <Text fontSize="lg" fontWeight="bold" mb={6} color={titleColor}>
        {t("hotkeySettings.title") || "热键设置"}
      </Text>

      <Box mb={6}>
        <Text
          fontSize="xs"
          fontWeight="semibold"
          color={subLabelColor}
          mb={3}
          textTransform="uppercase"
          letterSpacing="0.05em"
        >
          {t("hotkeySettings.overlay") || "悬浮框"}
        </Text>
        <LiquidGlassCard px={4} py={3} boxShadow="sm">
          <HStack justify="space-between">
            <Box flex={1}>
              <Text fontSize="sm" color={labelColor} fontWeight="medium">
                {t("hotkeySettings.overlayToggle") || "切换悬浮框"}
              </Text>
              <Text fontSize="xs" color={subLabelColor} mt={0.5}>
                {t("hotkeySettings.overlayToggleDesc") || "使用快捷键显示或隐藏悬浮框"}
              </Text>
            </Box>
            <HotkeyRecorder
              value={overlayHotkey}
              onChange={(val) => {
                saveOverlayHotkey(val);
                toast({
                  title: t("hotkeySettings.saved") || "快捷键已保存",
                  status: "success",
                  duration: 2000,
                  isClosable: true,
                });
              }}
            />
          </HStack>
        </LiquidGlassCard>
      </Box>

      <Box mb={6}>
        <Text
          fontSize="xs"
          fontWeight="semibold"
          color={subLabelColor}
          mb={3}
          textTransform="uppercase"
          letterSpacing="0.05em"
        >
          {t("hotkeySettings.crosshair") || "准心"}
        </Text>
        <LiquidGlassCard px={4} py={3} boxShadow="sm">
          <HStack justify="space-between">
            <Box flex={1}>
              <Text fontSize="sm" color={labelColor} fontWeight="medium">
                {t("hotkeySettings.crosshairToggle") || "切换准心"}
              </Text>
              <Text fontSize="xs" color={subLabelColor} mt={0.5}>
                {t("hotkeySettings.crosshairToggleDesc") || "使用快捷键显示或隐藏准心"}
              </Text>
            </Box>
            <HotkeyRecorder
              value={crosshairHotkey}
              onChange={(val) => {
                saveCrosshairHotkey(val);
                toast({
                  title: t("hotkeySettings.saved") || "快捷键已保存",
                  status: "success",
                  duration: 2000,
                  isClosable: true,
                });
              }}
            />
          </HStack>
        </LiquidGlassCard>
      </Box>

      <Box mb={6}>
        <Text
          fontSize="xs"
          fontWeight="semibold"
          color={subLabelColor}
          mb={3}
          textTransform="uppercase"
          letterSpacing="0.05em"
        >
          {t("hotkeySettings.filter") || "滤镜"}
        </Text>
        <LiquidGlassCard px={4} py={3} boxShadow="sm">
          <HStack justify="space-between">
            <Box flex={1}>
              <Text fontSize="sm" color={labelColor} fontWeight="medium">
                {t("hotkeySettings.filterToggle") || "切换滤镜"}
              </Text>
              <Text fontSize="xs" color={subLabelColor} mt={0.5}>
                {t("hotkeySettings.filterToggleDesc") || "使用快捷键开启或关闭滤镜"}
              </Text>
            </Box>
            <HotkeyRecorder
              value={filterHotkey}
              onChange={(val) => {
                saveFilterHotkey(val);
                toast({
                  title: t("hotkeySettings.saved") || "快捷键已保存",
                  status: "success",
                  duration: 2000,
                  isClosable: true,
                });
              }}
            />
          </HStack>
        </LiquidGlassCard>
      </Box>

      <Box mb={6}>
        <Text
          fontSize="xs"
          fontWeight="semibold"
          color={subLabelColor}
          mb={3}
          textTransform="uppercase"
          letterSpacing="0.05em"
        >
          {t("hotkeySettings.island") || "灵动岛"}
        </Text>
        <LiquidGlassCard px={4} py={3} boxShadow="sm">
          <HStack justify="space-between">
            <Box flex={1}>
              <Text fontSize="sm" color={labelColor} fontWeight="medium">
                {t("hotkeySettings.islandToggle") || "切换灵动岛"}
              </Text>
              <Text fontSize="xs" color={subLabelColor} mt={0.5}>
                {t("hotkeySettings.islandToggleDesc") || "使用快捷键显示或隐藏灵动岛"}
              </Text>
            </Box>
            <HotkeyRecorder
              value={islandHotkey}
              onChange={(val) => {
                saveIslandHotkey(val);
                toast({
                  title: t("hotkeySettings.saved") || "快捷键已保存",
                  status: "success",
                  duration: 2000,
                  isClosable: true,
                });
              }}
            />
          </HStack>
        </LiquidGlassCard>
      </Box>
    </Box>
  );
}

export default function SettingsPage() {
  const [activeItem, setActiveItem] = useState("general");
  const { t } = useTranslation();
  const { config } = useThemeColor();
  const transitionMode = useTransitionMode();

  return (
    <Flex gap={6} pt={8}>
      <Box w="180px" flexShrink={0} position="sticky" top={8} alignSelf="flex-start">
        <VStack spacing={0.5} align="stretch">
          {settingItems.map((item) => {
            const Icon = item.icon;
            const isActive = activeItem === item.id;

            return (
              <LiquidGlassMenuItem
                key={item.id}
                isActive={isActive}
                onClick={() => setActiveItem(item.id)}
                icon={item.icon}
              >
                {t(item.labelKey)}
              </LiquidGlassMenuItem>
            );
          })}
        </VStack>
      </Box>

      <Box flex={1}>
        <AnimatePresence mode="wait">
          {transitionMode !== "off" ? (
            <motion.div
              key={activeItem}
              initial="initial"
              animate="enter"
              exit="exit"
              variants={getVariants(transitionMode)}
              transition={getTransitionConfig(transitionMode)}
              style={{ position: 'relative', zIndex: 1 }}
            >
              {activeItem === "general" && <GeneralSettings />}
              {activeItem === "appearance" && <AppearanceSettings />}
              {activeItem === "hotkeys" && <HotkeySettings />}
              {activeItem === "network" && <NetworkSettings />}
              {activeItem === "sponsor" && <SponsorSettings />}
              {activeItem === "about" && <AboutSettings />}
            </motion.div>
          ) : (
            <div key={activeItem} style={{ position: 'relative', zIndex: 1 }}>
              {activeItem === "general" && <GeneralSettings />}
              {activeItem === "appearance" && <AppearanceSettings />}
              {activeItem === "hotkeys" && <HotkeySettings />}
              {activeItem === "network" && <NetworkSettings />}
              {activeItem === "sponsor" && <SponsorSettings />}
              {activeItem === "about" && <AboutSettings />}
            </div>
          )}
        </AnimatePresence>
      </Box>
    </Flex>
  );
}
