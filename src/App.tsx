import { Routes, Route, useLocation } from "react-router-dom";
import { AnimatePresence } from "framer-motion";
import { MainLayout } from "./components/ui/main-layout";
import { AnimatedPage, type TransitionMode, readTransitionMode } from "./components/ui/animated-page";
import HomePage from "./pages/HomePage";
import HardwarePage from "./pages/HardwarePage";
import ToolsPage from "./pages/ToolsPage";
import OptimizePage from "./pages/OptimizePage";
import MemoryLimitPage from "./pages/MemoryLimitPage";
import MemoryCleanupPage from "./pages/MemoryCleanupPage";
import AceOptimizePage from "./pages/AceOptimizePage";
import DisplayFilterPage from "./pages/DisplayFilterPage";
import SettingsPage from "./pages/SettingsPage";
import CrosshairPage from "./pages/CrosshairPage";
import ScreenRecordPage from "./pages/ScreenRecordPage";
import OverlayPanelPage from "./pages/OverlayPanelPage";
import DeltaForcePage from "./pages/DeltaForcePage";
import OtherGunCodePlatformsPage from "./pages/OtherGunCodePlatformsPage";
import MoodPage from "./pages/MoodPage";
import BuiltinToolsPage from "./pages/BuiltinToolsPage";
import GpuRenamePage from "./pages/GpuRenamePage";
import ResolutionConverterPage from "./pages/ResolutionConverterPage";
import ShaderCachePage from "./pages/ShaderCachePage";
import PowerManagementPage from "./pages/PowerManagementPage";
import StorageCleanPage from "./pages/StorageCleanPage";
import StartupManagerPage from "./pages/StartupManagerPage";
import SystemOptimizerPage from "./pages/SystemOptimizerPage";
import NetworkOptimizerPage from "./pages/NetworkOptimizerPage";
import PeripheralOptimizePage from "./pages/PeripheralOptimizePage";
import DLSSPresetPage from "./pages/DLSSPresetPage";
import TestsPage from "./pages/TestsPage";
import EpicFreePage from "./pages/EpicFreePage";
import ReactionTestPage from "./pages/ReactionTestPage";
import WidgetIslandPage from "./pages/WidgetIslandPage";
import DynamicIslandPage from "./pages/DynamicIslandPage";
import AimTestPage from "./pages/AimTestPage";
import FocusTestPage from "./pages/FocusTestPage";
import ChoiceTestPage from "./pages/ChoiceTestPage";
import InhibitTestPage from "./pages/InhibitTestPage";
import SchulteTestPage from "./pages/SchulteTestPage";
import CpsTestPage from "./pages/CpsTestPage";
import { useState, useEffect } from "react";

import {
  Box,
  Modal,
  ModalOverlay,
  ModalContent,
  ModalHeader,
  ModalCloseButton,
  ModalBody,
  ModalFooter,
  Button,
  Progress,
  useColorModeValue,
  Text,
  VStack,
  HStack,
  useToast,
} from "@chakra-ui/react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { fetchLatestRelease, compareVersions, type GiteeRelease } from "@/lib/update-checker";
import { LiquidGlassButton } from "@/components/special/liquid-glass-button";
import { LuDownload, LuRefreshCw } from "react-icons/lu";
import { SplashScreen } from "./components/SplashScreen";
import { useAppStartup } from "./contexts/app-startup-context";
import { MusicProvider } from "./contexts/music-context";
import { MiniMusicPlayer } from "./components/MiniMusicPlayer";
import { ImportantAnnouncementModal } from "./components/ImportantAnnouncementModal";

const CURRENT_VERSION = "4.2.6";

function App() {
  const { t } = useTranslation();
  const { isStartupComplete } = useAppStartup();
  const location = useLocation();
  const toast = useToast();

  // Widget window: render WidgetIslandPage standalone, no main layout
  if (location.pathname === "/widget") {
    return <WidgetIslandPage />;
  }
  const [latestRelease, setLatestRelease] = useState<GiteeRelease | null>(null);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [isDownloading, setIsDownloading] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [isDownloadComplete, setIsDownloadComplete] = useState(false);
  const [downloadedFilePath, setDownloadedFilePath] = useState<string>("");
  const [pageTransitionMode, setPageTransitionMode] = useState<TransitionMode>("fade");

  const labelColor = useColorModeValue("gray.700", "#e0e0e0");
  const subLabelColor = useColorModeValue("gray.500", "#888888");
  const modalBg = useColorModeValue("white", "#111111");
  const modalBorderColor = useColorModeValue("gray.200", "#333333");

  useEffect(() => {
    const unlisten = listen<{ progress: number; total: number }>("download-progress", (event) => {
      setDownloadProgress(event.payload.progress);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    setPageTransitionMode(readTransitionMode());

    const handler = () => setPageTransitionMode(readTransitionMode());

    window.addEventListener("page-transition-setting-changed", handler);
    return () => window.removeEventListener("page-transition-setting-changed", handler);
  }, []);



  useEffect(() => {
    const checkUpdateOnStartup = async () => {
      if (!isStartupComplete) return;

      try {
        const release = await fetchLatestRelease();
        if (release) {
          const hasUpdate = compareVersions(CURRENT_VERSION, release.tag_name);
          if (hasUpdate) {
            setLatestRelease(release);
            setIsModalOpen(true);
          }
        }
      } catch (error) {
        console.error("Failed to check for updates on startup:", error);
      }
    };

    checkUpdateOnStartup();
  }, [isStartupComplete]);

  const handleOpenLink = async (url: string) => {
    try {
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(url);
    } catch (error) {
      console.error("Failed to open link:", error);
    }
  };

  const handleDownload = async () => {
    if (!latestRelease) return;

    setIsDownloading(true);
    setDownloadProgress(0);
    setIsDownloadComplete(false);

    try {
      const asset = latestRelease.assets.find(
        (a) => a.name.endsWith(".msi") || a.name.endsWith(".exe"),
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
    <MusicProvider>
      <>
        {!isStartupComplete && <SplashScreen />}
        {/* <MiniMusicPlayer /> */}
        <MainLayout>
        {pageTransitionMode !== "off" ? (
          <AnimatePresence mode="wait">
            <Routes location={location} key={location.pathname}>
              <Route path="/" element={<AnimatedPage><HomePage /></AnimatedPage>} />
              <Route path="/hardware" element={<AnimatedPage><HardwarePage /></AnimatedPage>} />
              <Route path="/tools" element={<AnimatedPage><ToolsPage /></AnimatedPage>} />
              <Route path="/builtin-tools" element={<AnimatedPage><BuiltinToolsPage /></AnimatedPage>} />
              <Route path="/optimization" element={<AnimatedPage><OptimizePage /></AnimatedPage>} />
              <Route path="/optimize" element={<AnimatedPage><OptimizePage /></AnimatedPage>} />
              <Route path="/optimize/memory-cleanup" element={<AnimatedPage><MemoryCleanupPage /></AnimatedPage>} />
              <Route path="/optimize/ace-optimize" element={<AnimatedPage><AceOptimizePage /></AnimatedPage>} />
              <Route path="/optimize/memory-limit" element={<AnimatedPage><MemoryLimitPage /></AnimatedPage>} />
              <Route path="/display-filter" element={<AnimatedPage><DisplayFilterPage /></AnimatedPage>} />
              <Route path="/settings" element={<AnimatedPage><SettingsPage /></AnimatedPage>} />
              <Route path="/crosshair" element={<AnimatedPage><CrosshairPage /></AnimatedPage>} />
              <Route path="/screen-record" element={<AnimatedPage><ScreenRecordPage /></AnimatedPage>} />
              <Route path="/overlay-panel" element={<AnimatedPage><OverlayPanelPage /></AnimatedPage>} />
              <Route path="/delta-force" element={<AnimatedPage><DeltaForcePage /></AnimatedPage>} />
              <Route path="/delta-force/other-platforms" element={<AnimatedPage><OtherGunCodePlatformsPage /></AnimatedPage>} />
              <Route path="/mood" element={<AnimatedPage><MoodPage /></AnimatedPage>} />
              <Route path="/tests" element={<AnimatedPage><TestsPage /></AnimatedPage>} />
              <Route path="/tests/reaction" element={<AnimatedPage><ReactionTestPage /></AnimatedPage>} />
              <Route path="/tests/aim" element={<AnimatedPage><AimTestPage /></AnimatedPage>} />
              <Route path="/tests/focus" element={<AnimatedPage><FocusTestPage /></AnimatedPage>} />
              <Route path="/tests/choice" element={<AnimatedPage><ChoiceTestPage /></AnimatedPage>} />
              <Route path="/tests/inhibit" element={<AnimatedPage><InhibitTestPage /></AnimatedPage>} />
              <Route path="/tests/schulte" element={<AnimatedPage><SchulteTestPage /></AnimatedPage>} />
              <Route path="/tests/cps" element={<AnimatedPage><CpsTestPage /></AnimatedPage>} />
              <Route path="/gpu-rename" element={<AnimatedPage><GpuRenamePage /></AnimatedPage>} />
              <Route path="/resolution-converter" element={<AnimatedPage><ResolutionConverterPage /></AnimatedPage>} />
              <Route path="/optimize/shader-cache" element={<AnimatedPage><ShaderCachePage /></AnimatedPage>} />
              <Route path="/optimize/power-management" element={<AnimatedPage><PowerManagementPage /></AnimatedPage>} />
              <Route path="/optimize/storage-clean" element={<AnimatedPage><StorageCleanPage /></AnimatedPage>} />
              <Route path="/optimize/startup-manager" element={<AnimatedPage><StartupManagerPage /></AnimatedPage>} />
              <Route path="/optimize/system-optimizer" element={<AnimatedPage><SystemOptimizerPage /></AnimatedPage>} />
            <Route path="/optimize/network-optimizer" element={<AnimatedPage><NetworkOptimizerPage /></AnimatedPage>} />
            <Route path="/optimize/peripheral-optimize" element={<AnimatedPage><PeripheralOptimizePage /></AnimatedPage>} />
              <Route path="/dlss-preset" element={<AnimatedPage><DLSSPresetPage /></AnimatedPage>} />
              <Route path="/epic-free" element={<AnimatedPage><EpicFreePage /></AnimatedPage>} />
              <Route path="/dynamic-island" element={<AnimatedPage><DynamicIslandPage /></AnimatedPage>} />
        </Routes>
      </AnimatePresence>
    ) : (
      <Routes location={location} key={location.pathname}>
            <Route path="/" element={<AnimatedPage><HomePage /></AnimatedPage>} />
            <Route path="/hardware" element={<AnimatedPage><HardwarePage /></AnimatedPage>} />
            <Route path="/tools" element={<AnimatedPage><ToolsPage /></AnimatedPage>} />
            <Route path="/builtin-tools" element={<AnimatedPage><BuiltinToolsPage /></AnimatedPage>} />
            <Route path="/optimization" element={<AnimatedPage><OptimizePage /></AnimatedPage>} />
            <Route path="/optimize" element={<AnimatedPage><OptimizePage /></AnimatedPage>} />
            <Route path="/optimize/memory-cleanup" element={<AnimatedPage><MemoryCleanupPage /></AnimatedPage>} />
            <Route path="/optimize/ace-optimize" element={<AnimatedPage><AceOptimizePage /></AnimatedPage>} />
            <Route path="/optimize/memory-limit" element={<AnimatedPage><MemoryLimitPage /></AnimatedPage>} />
            <Route path="/display-filter" element={<AnimatedPage><DisplayFilterPage /></AnimatedPage>} />
            <Route path="/settings" element={<AnimatedPage><SettingsPage /></AnimatedPage>} />
            <Route path="/crosshair" element={<AnimatedPage><CrosshairPage /></AnimatedPage>} />
            <Route path="/overlay-panel" element={<AnimatedPage><OverlayPanelPage /></AnimatedPage>} />
            <Route path="/delta-force" element={<AnimatedPage><DeltaForcePage /></AnimatedPage>} />
            <Route path="/delta-force/other-platforms" element={<AnimatedPage><OtherGunCodePlatformsPage /></AnimatedPage>} />
            <Route path="/mood" element={<AnimatedPage><MoodPage /></AnimatedPage>} />
            <Route path="/tests" element={<AnimatedPage><TestsPage /></AnimatedPage>} />
            <Route path="/tests/reaction" element={<AnimatedPage><ReactionTestPage /></AnimatedPage>} />
            <Route path="/tests/aim" element={<AnimatedPage><AimTestPage /></AnimatedPage>} />
            <Route path="/tests/focus" element={<AnimatedPage><FocusTestPage /></AnimatedPage>} />
            <Route path="/tests/choice" element={<AnimatedPage><ChoiceTestPage /></AnimatedPage>} />
            <Route path="/tests/inhibit" element={<AnimatedPage><InhibitTestPage /></AnimatedPage>} />
            <Route path="/tests/schulte" element={<AnimatedPage><SchulteTestPage /></AnimatedPage>} />
            <Route path="/tests/cps" element={<AnimatedPage><CpsTestPage /></AnimatedPage>} />
            <Route path="/gpu-rename" element={<AnimatedPage><GpuRenamePage /></AnimatedPage>} />
            <Route path="/resolution-converter" element={<AnimatedPage><ResolutionConverterPage /></AnimatedPage>} />
            <Route path="/optimize/shader-cache" element={<AnimatedPage><ShaderCachePage /></AnimatedPage>} />
            <Route path="/optimize/power-management" element={<AnimatedPage><PowerManagementPage /></AnimatedPage>} />
            <Route path="/optimize/storage-clean" element={<AnimatedPage><StorageCleanPage /></AnimatedPage>} />
            <Route path="/optimize/startup-manager" element={<AnimatedPage><StartupManagerPage /></AnimatedPage>} />
            <Route path="/optimize/system-optimizer" element={<AnimatedPage><SystemOptimizerPage /></AnimatedPage>} />
            <Route path="/optimize/network-optimizer" element={<AnimatedPage><NetworkOptimizerPage /></AnimatedPage>} />
            <Route path="/optimize/peripheral-optimize" element={<AnimatedPage><PeripheralOptimizePage /></AnimatedPage>} />
            <Route path="/dlss-preset" element={<AnimatedPage><DLSSPresetPage /></AnimatedPage>} />
            <Route path="/epic-free" element={<AnimatedPage><EpicFreePage /></AnimatedPage>} />
            <Route path="/dynamic-island" element={<AnimatedPage><DynamicIslandPage /></AnimatedPage>} />
      </Routes>
    )}

      </MainLayout>

      <Modal
        isOpen={isModalOpen}
        onClose={() => !isDownloading && !isDownloadComplete && setIsModalOpen(false)}
        isCentered
        closeOnOverlayClick={!isDownloading && !isDownloadComplete}
      >
        <ModalOverlay />
        <ModalContent bg={modalBg} borderColor={modalBorderColor} borderRadius="xl">
          <ModalHeader color={labelColor}>
            {t("settings.aboutSettings.updateModal.title")}
          </ModalHeader>
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
                  <Progress
                    value={downloadProgress}
                    size="sm"
                    colorScheme="teal"
                    borderRadius="full"
                  />
                  <Text color={subLabelColor} fontSize="xs" mt={1}>
                    {t("settings.aboutSettings.updateModal.downloading")} {downloadProgress}%
                  </Text>
                </Box>
              )}
              {isDownloadComplete && (
                <Box
                  w="full"
                  p={3}
                  borderRadius="lg"
                  bg={useColorModeValue("green.50", "rgba(72, 187, 120, 0.1)")}
                  border="1px solid"
                  borderColor={useColorModeValue("green.200", "rgba(72, 187, 120, 0.3)")}
                >
                  <Text
                    color={useColorModeValue("green.600", "green.300")}
                    fontSize="sm"
                    fontWeight="medium"
                  >
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
                  {isDownloading
                    ? t("settings.aboutSettings.updateModal.downloading")
                    : t("settings.aboutSettings.updateModal.download")}
                </LiquidGlassButton>
              </>
            )}
          </ModalFooter>
        </ModalContent>
      </Modal>
      <ImportantAnnouncementModal />
      </>
      </MusicProvider>
  );
}

export default App;
