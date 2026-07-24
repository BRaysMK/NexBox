import { useState, useCallback } from "react";
import { AnimatePresence } from "framer-motion";
import TitleBar from "./components/TitleBar";
import UninstallerLayout from "./components/UninstallerLayout";
import ConfirmPage from "./pages/ConfirmPage";
import UninstallingPage from "./pages/UninstallingPage";
import CompletePage from "./pages/CompletePage";

export default function App() {
  const [step, setStep] = useState(1);
  const [error, setError] = useState("");

  const handleStartUninstall = useCallback(() => {
    setStep(2);
  }, []);

  const handleUninstallComplete = useCallback(() => {
    setStep(3);
  }, []);

  const handleUninstallError = useCallback((msg: string) => {
    setError(msg);
    alert(msg);
  }, []);

  return (
    <>
      <TitleBar />
      <UninstallerLayout
        showFooter={step === 1}
        onPrimary={step === 1 ? handleStartUninstall : undefined}
        primaryLabel={step === 1 ? "开始卸载" : undefined}
      >
        <AnimatePresence mode="wait">
          {step === 1 && (
            <ConfirmPage key="confirm" onStart={handleStartUninstall} />
          )}
          {step === 2 && (
            <UninstallingPage
              key="uninstall"
              onComplete={handleUninstallComplete}
              onError={handleUninstallError}
            />
          )}
          {step === 3 && <CompletePage key="complete" />}
        </AnimatePresence>
      </UninstallerLayout>
    </>
  );
}
