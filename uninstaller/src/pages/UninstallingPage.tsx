import { useEffect, useRef, useState } from "react";
import { motion } from "framer-motion";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";

interface UninstallingPageProps {
  onComplete: () => void;
  onError: (msg: string) => void;
}

export default function UninstallingPage({
  onComplete,
  onError,
}: UninstallingPageProps) {
  const { t } = useTranslation();
  const [progress, setProgress] = useState(0);
  const [statusText, setStatusText] = useState(t("uninstall_deleting"));
  const hasStarted = useRef(false);

  useEffect(() => {
    if (hasStarted.current) return;
    hasStarted.current = true;

    const doUninstall = async () => {
      try {
        setStatusText(t("uninstall_deleting"));
        for (let i = 0; i <= 60; i += 5) {
          setProgress(i);
          await new Promise((r) => setTimeout(r, 30));
        }

        await invoke("start_uninstall");

        setStatusText(t("uninstall_shortcuts"));
        for (let i = 60; i <= 80; i += 3) {
          setProgress(i);
          await new Promise((r) => setTimeout(r, 20));
        }

        setStatusText(t("uninstall_registry"));
        for (let i = 80; i < 100; i += 2) {
          setProgress(i);
          await new Promise((r) => setTimeout(r, 15));
        }

        setProgress(100);
        setStatusText(t("uninstall_done"));
        await new Promise((r) => setTimeout(r, 500));
        onComplete();
      } catch (err: any) {
        onError(String(err));
      }
    };

    doUninstall();
  }, []);

  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      className="progress-container"
    >
      <div style={{ width: "100%", maxWidth: 340 }}>
        <div className="progress-bar">
          <div
            className="progress-bar-fill"
            style={{ width: `${progress}%` }}
          />
        </div>
        <div style={{
          textAlign: "center",
          marginTop: 10,
          fontSize: 22,
          fontWeight: 700,
          color: "#1a202c",
        }}>
          {progress}%
        </div>
      </div>

      <motion.p
        key={statusText}
        initial={{ opacity: 0, y: 5 }}
        animate={{ opacity: 1, y: 0 }}
        style={{ fontSize: 14, color: "#64748b" }}
      >
        {statusText}
      </motion.p>
    </motion.div>
  );
}
