import { useState, useCallback } from "react";
import { AnimatePresence } from "framer-motion";
import { useTranslation } from "react-i18next";
import TitleBar from "./components/TitleBar";
import InstallerLayout from "./components/InstallerLayout";
import WelcomePage from "./pages/WelcomePage";
import LicensePage from "./pages/LicensePage";
import SelectDirPage from "./pages/SelectDirPage";
import InstallingPage from "./pages/InstallingPage";
import FinishPage from "./pages/FinishPage";

export default function App() {
  const { t } = useTranslation();
  const [step, setStep] = useState(1);
  const [licenseAgreed, setLicenseAgreed] = useState(false);
  const [targetDir, setTargetDir] = useState("");
  const [dirValid, setDirValid] = useState(false);
  const [createDesktopShortcut, setCreateDesktopShortcut] = useState(true);
  const [error, setError] = useState("");

  const handleNext = useCallback(() => {
    if (step === 2 && !licenseAgreed) return;
    if (step === 3 && !dirValid) return;
    if (step < 5) setStep((s) => s + 1);
  }, [step, licenseAgreed, dirValid]);

  const handleBack = useCallback(() => {
    if (step > 1) setStep((s) => s - 1);
  }, [step]);

  const handleInstallComplete = useCallback(() => {
    setStep(5);
  }, []);

  const handleInstallError = useCallback((msg: string) => {
    setError(msg);
    alert(msg);
  }, []);

  const canGoNext = useCallback(() => {
    if (step === 1) return true;
    if (step === 2) return licenseAgreed;
    if (step === 3) return dirValid;
    return false;
  }, [step, licenseAgreed, dirValid]);

  const nextLabel = step === 1 ? "开始安装" : step === 3 ? t("btn_install") : undefined;

  return (
    <>
      <TitleBar />
      <InstallerLayout
        currentStep={step}
        canGoBack={step > 1 && step < 4}
        canGoNext={step < 4 ? canGoNext() : false}
        nextLabel={nextLabel}
        onBack={step > 1 && step < 4 ? handleBack : undefined}
        onNext={step < 4 ? handleNext : undefined}
        showCancel={false}
      >
        <AnimatePresence mode="wait">
          {step === 1 && <WelcomePage key="welcome" />}
          {step === 2 && <LicensePage key="license" onAgreed={setLicenseAgreed} />}
          {step === 3 && (
            <SelectDirPage
              key="dir"
              onDirChange={setTargetDir}
              onValidChange={setDirValid}
              onShortcutChange={setCreateDesktopShortcut}
              createDesktopShortcut={createDesktopShortcut}
            />
          )}
          {step === 4 && (
            <InstallingPage
              key="install"
              targetDir={targetDir}
              createDesktopShortcut={createDesktopShortcut}
              onComplete={handleInstallComplete}
              onError={handleInstallError}
            />
          )}
          {step === 5 && (
            <FinishPage key="finish" targetDir={targetDir} />
          )}
        </AnimatePresence>
      </InstallerLayout>
    </>
  );
}
