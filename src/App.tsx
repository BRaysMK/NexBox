import React, { lazy, Suspense } from "react";
import { Routes, Route, useLocation } from "react-router-dom";
import { AnimatePresence } from "framer-motion";
import { MainLayout } from "./components/ui/main-layout";
import { AnimatedPage, type TransitionMode, readTransitionMode } from "./components/ui/animated-page";
const HomePage = lazy(() => import("./pages/HomePage"));
const HardwarePage = lazy(() => import("./pages/HardwarePage"));
const ToolsPage = lazy(() => import("./pages/ToolsPage"));
const OptimizePage = lazy(() => import("./pages/OptimizePage"));
const MemoryLimitPage = lazy(() => import("./pages/MemoryLimitPage"));
const MemoryCleanupPage = lazy(() => import("./pages/MemoryCleanupPage"));
const AceOptimizePage = lazy(() => import("./pages/AntiCheatOptimizePage"));
const DisplayFilterPage = lazy(() => import("./pages/DisplayFilterPage"));
const SettingsPage = lazy(() => import("./pages/SettingsPage"));
const CrosshairPage = lazy(() => import("./pages/CrosshairPage"));
const DiskHealthPage = lazy(() => import("./pages/DiskHealthPage"));
const OverlayPanelPage = lazy(() => import("./pages/OverlayPanelPage"));
const DeltaForcePage = lazy(() => import("./pages/DeltaForcePage"));
const LocalGunCodesPage = lazy(() => import("./pages/LocalGunCodesPage"));
const OtherGunCodePlatformsPage = lazy(() => import("./pages/OtherGunCodePlatformsPage"));
const MoodPage = lazy(() => import("./pages/MoodPage"));
const BuiltinToolsPage = lazy(() => import("./pages/BuiltinToolsPage"));
const GpuRenamePage = lazy(() => import("./pages/GpuRenamePage"));
const ResolutionConverterPage = lazy(() => import("./pages/ResolutionConverterPage"));
const ShaderCachePage = lazy(() => import("./pages/ShaderCachePage"));
const PowerManagementPage = lazy(() => import("./pages/PowerManagementPage"));
const StorageCleanPage = lazy(() => import("./pages/StorageCleanPage"));
const StartupManagerPage = lazy(() => import("./pages/StartupManagerPage"));
const SystemOptimizerPage = lazy(() => import("./pages/SystemOptimizerPage"));
const NetworkOptimizerPage = lazy(() => import("./pages/NetworkOptimizerPage"));
const PeripheralOptimizePage = lazy(() => import("./pages/PeripheralOptimizePage"));
const WindowsUpdatePage = lazy(() => import("./pages/WindowsUpdatePage"));
const DLSSPresetPage = lazy(() => import("./pages/DLSSPresetPage"));
const NvidiaDriverPage = lazy(() => import("./pages/NvidiaDriverPage"));
const NvidiaDriverDownloadPage = lazy(() => import("./pages/NvidiaDriverDownloadPage"));
const EpicFreePage = lazy(() => import("./pages/EpicFreePage"));
const SteamPage = lazy(() => import("./pages/SteamPage"));
const TrayMenuPage = lazy(() => import("./pages/TrayMenuPage"));
const DesktopLyricsPage = lazy(() => import("./pages/DesktopLyricsPage"));
const LyricsUnlockBtnPage = lazy(() => import("./pages/LyricsUnlockBtnPage"));
const VerticalOverlayPage = lazy(() => import("./pages/VerticalOverlayPage"));
const SensorMonitorPage = lazy(() => import("./pages/SensorMonitorPage"));
const RuntimeRepairPage = lazy(() => import("./pages/RuntimeRepairPage"));
const VtxVirtualizationPage = lazy(() => import("./pages/VtxVirtualizationPage"));
const AudioEqPage = lazy(() => import("./pages/AudioEqPage"));
const AutoClickerPage = lazy(() => import("./pages/AutoClickerPage"));
const GameProcessOptimizePage = lazy(() => import("./pages/GameProcessOptimizePage"));
const CpuSchedulerPage = lazy(() => import("./pages/CpuSchedulerPage"));
const SpeedTestPage = lazy(() => import("./pages/SpeedTestPage"));
const CustomPage = lazy(() => import("./pages/CustomPage"));
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

import { UpdateModal } from "./components/UpdateModal";
import { SplashScreen } from "./components/SplashScreen";
import { useAppStartup } from "./contexts/app-startup-context";
import { MusicProvider } from "./contexts/music-context";
const MusicPage = lazy(() => import("./pages/MusicPage"));
import { ImportantAnnouncementModal } from "./components/ImportantAnnouncementModal";
import { DynamicIslandHost } from "./components/ui/dynamic-island";

function App() {
  const { isStartupComplete } = useAppStartup();
  const location = useLocation();

  // Tray menu: render standalone, no main layout
  if (location.pathname === "/tray-menu") {
    return <TrayMenuPage />;
  }

  // Desktop lyrics window: render standalone, no main layout
  if (location.pathname === "/desktop-lyrics") {
    return <DesktopLyricsPage />;
  }

  // Lyrics unlock button window: tiny standalone overlay
  if (location.pathname === "/lyrics-unlock-btn") {
    return <LyricsUnlockBtnPage />;
  }

  // Vertical overlay window: standalone, no main layout
  if (location.pathname === "/vertical-overlay") {
    return <VerticalOverlayPage />;
  }

  // Sensor monitor window: standalone, no main layout
  if (location.pathname === "/sensor-monitor") {
    return <SensorMonitorPage />;
  }

  // 开机自启(--autostart)模式：后端已离屏预热加载本窗口，前端初始化完成后隐藏到托盘，
  // 复用 minimize_to_tray 正确更新后端可见性并触发 EcoQoS。
  useEffect(() => {
    (async () => {
      try {
        const autostart = await invoke<boolean>("is_autostart_mode");
        if (autostart) await invoke("minimize_to_tray");
      } catch (e) {
        console.error("autostart hide check failed:", e);
      }
    })();
  }, []);

  const [pageTransitionMode, setPageTransitionMode] = useState<TransitionMode>("fade");

  useEffect(() => {
    setPageTransitionMode(readTransitionMode());

    const handler = () => setPageTransitionMode(readTransitionMode());

    window.addEventListener("page-transition-setting-changed", handler);
    return () => window.removeEventListener("page-transition-setting-changed", handler);
  }, []);

  return (
    <MusicProvider>
      <>
        {!isStartupComplete && <SplashScreen />}
        {/* <MiniMusicPlayer /> */}
        <MainLayout>
        {pageTransitionMode !== "off" ? (
          <AnimatePresence mode="wait" initial={false}>
            <Suspense fallback={null}>
            <Routes location={location} key={location.pathname}>
              <Route path="/" element={<AnimatedPage><HomePage /></AnimatedPage>} />
              <Route path="/hardware" element={<AnimatedPage><HardwarePage /></AnimatedPage>} />
              <Route path="/tools" element={<AnimatedPage><ToolsPage /></AnimatedPage>} />
              <Route path="/builtin-tools" element={<AnimatedPage><BuiltinToolsPage /></AnimatedPage>} />
              <Route path="/optimization" element={<AnimatedPage><OptimizePage /></AnimatedPage>} />
              <Route path="/optimize" element={<AnimatedPage><OptimizePage /></AnimatedPage>} />
              <Route path="/optimize/memory-cleanup" element={<AnimatedPage><MemoryCleanupPage /></AnimatedPage>} />
              <Route path="/optimize/ace-optimize" element={<AnimatedPage><AceOptimizePage /></AnimatedPage>} />
              <Route path="/optimize/game-process-optimize" element={<AnimatedPage><GameProcessOptimizePage /></AnimatedPage>} />
              <Route path="/optimize/memory-limit" element={<AnimatedPage><MemoryLimitPage /></AnimatedPage>} />
              <Route path="/display-filter" element={<AnimatedPage><DisplayFilterPage /></AnimatedPage>} />
              <Route path="/settings" element={<AnimatedPage><SettingsPage /></AnimatedPage>} />
              <Route path="/crosshair" element={<AnimatedPage><CrosshairPage /></AnimatedPage>} />
              <Route path="/autoclicker" element={<AnimatedPage><AutoClickerPage /></AnimatedPage>} />
              <Route path="/disk-health" element={<AnimatedPage><DiskHealthPage /></AnimatedPage>} />
              <Route path="/overlay-panel" element={<AnimatedPage><OverlayPanelPage /></AnimatedPage>} />
              <Route path="/delta-force" element={<AnimatedPage><DeltaForcePage /></AnimatedPage>} />
              <Route path="/delta-force/other-platforms" element={<AnimatedPage><OtherGunCodePlatformsPage /></AnimatedPage>} />
              <Route path="/delta-force/local-gun-codes" element={<AnimatedPage><LocalGunCodesPage /></AnimatedPage>} />
              <Route path="/mood" element={<AnimatedPage><MoodPage /></AnimatedPage>} />
              <Route path="/gpu-rename" element={<AnimatedPage><GpuRenamePage /></AnimatedPage>} />
              <Route path="/resolution-converter" element={<AnimatedPage><ResolutionConverterPage /></AnimatedPage>} />
              <Route path="/optimize/shader-cache" element={<AnimatedPage><ShaderCachePage /></AnimatedPage>} />
              <Route path="/optimize/power-management" element={<AnimatedPage><PowerManagementPage /></AnimatedPage>} />
              <Route path="/optimize/storage-clean" element={<AnimatedPage><StorageCleanPage /></AnimatedPage>} />
              <Route path="/optimize/startup-manager" element={<AnimatedPage><StartupManagerPage /></AnimatedPage>} />
              <Route path="/optimize/system-optimizer" element={<AnimatedPage><SystemOptimizerPage /></AnimatedPage>} />
            <Route path="/optimize/network-optimizer" element={<AnimatedPage><NetworkOptimizerPage /></AnimatedPage>} />
            <Route path="/optimize/peripheral-optimize" element={<AnimatedPage><PeripheralOptimizePage /></AnimatedPage>} />
            <Route path="/optimize/windows-update" element={<AnimatedPage><WindowsUpdatePage /></AnimatedPage>} />
            <Route path="/optimize/cpu-scheduler" element={<AnimatedPage><CpuSchedulerPage /></AnimatedPage>} />
              <Route path="/dlss-preset" element={<AnimatedPage><DLSSPresetPage /></AnimatedPage>} />
              <Route path="/audio-eq" element={<AnimatedPage><AudioEqPage /></AnimatedPage>} />
              <Route path="/nvidia-driver" element={<AnimatedPage><NvidiaDriverPage /></AnimatedPage>} />
              <Route path="/nvidia-driver-download" element={<AnimatedPage><NvidiaDriverDownloadPage /></AnimatedPage>} />
            <Route path="/steam" element={<AnimatedPage><SteamPage /></AnimatedPage>} />
              <Route path="/epic-free" element={<AnimatedPage><EpicFreePage /></AnimatedPage>} />
              <Route path="/music" element={<AnimatedPage><MusicPage /></AnimatedPage>} />
              <Route path="/custom" element={<AnimatedPage><CustomPage /></AnimatedPage>} />
              <Route path="/speedtest" element={<AnimatedPage><SpeedTestPage /></AnimatedPage>} />
              <Route path="/runtime-repair" element={<AnimatedPage><RuntimeRepairPage /></AnimatedPage>} />
              <Route path="/vtx-virtualization" element={<AnimatedPage><VtxVirtualizationPage /></AnimatedPage>} />
        </Routes>
            </Suspense>
      </AnimatePresence>
    ) : (
      <Suspense fallback={null}>
      <Routes location={location}>
            <Route path="/" element={<AnimatedPage><HomePage /></AnimatedPage>} />
            <Route path="/hardware" element={<AnimatedPage><HardwarePage /></AnimatedPage>} />
            <Route path="/tools" element={<AnimatedPage><ToolsPage /></AnimatedPage>} />
            <Route path="/builtin-tools" element={<AnimatedPage><BuiltinToolsPage /></AnimatedPage>} />
            <Route path="/optimization" element={<AnimatedPage><OptimizePage /></AnimatedPage>} />
            <Route path="/optimize" element={<AnimatedPage><OptimizePage /></AnimatedPage>} />
            <Route path="/optimize/memory-cleanup" element={<AnimatedPage><MemoryCleanupPage /></AnimatedPage>} />
            <Route path="/optimize/ace-optimize" element={<AnimatedPage><AceOptimizePage /></AnimatedPage>} />
            <Route path="/optimize/game-process-optimize" element={<AnimatedPage><GameProcessOptimizePage /></AnimatedPage>} />
            <Route path="/optimize/memory-limit" element={<AnimatedPage><MemoryLimitPage /></AnimatedPage>} />
            <Route path="/display-filter" element={<AnimatedPage><DisplayFilterPage /></AnimatedPage>} />
            <Route path="/settings" element={<AnimatedPage><SettingsPage /></AnimatedPage>} />
            <Route path="/crosshair" element={<AnimatedPage><CrosshairPage /></AnimatedPage>} />
            <Route path="/autoclicker" element={<AnimatedPage><AutoClickerPage /></AnimatedPage>} />
            <Route path="/overlay-panel" element={<AnimatedPage><OverlayPanelPage /></AnimatedPage>} />
            <Route path="/delta-force" element={<AnimatedPage><DeltaForcePage /></AnimatedPage>} />
            <Route path="/delta-force/other-platforms" element={<AnimatedPage><OtherGunCodePlatformsPage /></AnimatedPage>} />
            <Route path="/mood" element={<AnimatedPage><MoodPage /></AnimatedPage>} />
            <Route path="/gpu-rename" element={<AnimatedPage><GpuRenamePage /></AnimatedPage>} />
            <Route path="/resolution-converter" element={<AnimatedPage><ResolutionConverterPage /></AnimatedPage>} />
            <Route path="/optimize/shader-cache" element={<AnimatedPage><ShaderCachePage /></AnimatedPage>} />
            <Route path="/optimize/power-management" element={<AnimatedPage><PowerManagementPage /></AnimatedPage>} />
            <Route path="/optimize/storage-clean" element={<AnimatedPage><StorageCleanPage /></AnimatedPage>} />
            <Route path="/optimize/startup-manager" element={<AnimatedPage><StartupManagerPage /></AnimatedPage>} />
            <Route path="/optimize/system-optimizer" element={<AnimatedPage><SystemOptimizerPage /></AnimatedPage>} />
            <Route path="/optimize/network-optimizer" element={<AnimatedPage><NetworkOptimizerPage /></AnimatedPage>} />
            <Route path="/optimize/peripheral-optimize" element={<AnimatedPage><PeripheralOptimizePage /></AnimatedPage>} />
              <Route path="/optimize/windows-update" element={<AnimatedPage><WindowsUpdatePage /></AnimatedPage>} />
            <Route path="/optimize/cpu-scheduler" element={<AnimatedPage><CpuSchedulerPage /></AnimatedPage>} />
              <Route path="/dlss-preset" element={<AnimatedPage><DLSSPresetPage /></AnimatedPage>} />
              <Route path="/audio-eq" element={<AnimatedPage><AudioEqPage /></AnimatedPage>} />
              <Route path="/nvidia-driver" element={<AnimatedPage><NvidiaDriverPage /></AnimatedPage>} />
              <Route path="/nvidia-driver-download" element={<AnimatedPage><NvidiaDriverDownloadPage /></AnimatedPage>} />
              <Route path="/disk-health" element={<AnimatedPage><DiskHealthPage /></AnimatedPage>} />
            <Route path="/epic-free" element={<AnimatedPage><EpicFreePage /></AnimatedPage>} />
              <Route path="/steam" element={<AnimatedPage><SteamPage /></AnimatedPage>} />
            <Route path="/music" element={<AnimatedPage><MusicPage /></AnimatedPage>} />
            <Route path="/custom" element={<AnimatedPage><CustomPage /></AnimatedPage>} />
            <Route path="/speedtest" element={<AnimatedPage><SpeedTestPage /></AnimatedPage>} />
            <Route path="/runtime-repair" element={<AnimatedPage><RuntimeRepairPage /></AnimatedPage>} />
            <Route path="/vtx-virtualization" element={<AnimatedPage><VtxVirtualizationPage /></AnimatedPage>} />
      </Routes>
      </Suspense>
    )}

      </MainLayout>

      <UpdateModal />
      <ImportantAnnouncementModal />
      <DynamicIslandHost />
      </>
      </MusicProvider>
  );
}

export default App;
