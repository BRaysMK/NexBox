import { useState } from "react";
import { motion } from "framer-motion";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface FinishPageProps {
  targetDir: string;
}

export default function FinishPage({ targetDir }: FinishPageProps) {
  const { t } = useTranslation();
  const [launch, setLaunch] = useState(true);

  const handleFinish = async () => {
    if (launch) {
      try {
        await invoke("launch_installed_app", { targetDir });
      } catch { }
    }
    // 更新场景：调度安装程序自删除
    try {
      await invoke("schedule_installer_cleanup");
    } catch { }
    try {
      await getCurrentWindow().close();
    } catch { }
  };

  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.95 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.95 }}
      transition={{ duration: 0.3 }}
      style={{ display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", flex: 1, gap: 16 }}
    >
      <motion.div
        initial={{ scale: 0 }}
        animate={{ scale: 1 }}
        transition={{ type: "spring", stiffness: 200, damping: 15 }}
      >
        <svg width="72" height="72" viewBox="0 0 72 72" fill="none">
          <circle cx="36" cy="36" r="36" fill="rgba(34, 197, 94, 0.12)" />
          <circle cx="36" cy="36" r="28" fill="rgba(34, 197, 94, 0.2)" />
          <path d="M28 36l6 6 10-12" stroke="#22c55e" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" fill="none" />
        </svg>
      </motion.div>

      <h1 className="page-title" style={{ textAlign: "center" }}>
        {t("finish_title")}
      </h1>

      <p className="page-subtitle" style={{ textAlign: "center" }}>
        {t("finish_desc")}
      </p>

      <label className="checkbox-label" style={{ marginTop: 8 }}>
        <input
          type="checkbox"
          checked={launch}
          onChange={(e) => setLaunch(e.target.checked)}
        />
        <span className="checkbox-mark" />
        {t("finish_launch")}
      </label>

      <motion.button
        className="btn-primary"
        onClick={handleFinish}
        whileHover={{ scale: 1.02 }}
        whileTap={{ scale: 0.98 }}
        style={{ marginTop: 16, padding: "10px 48px", fontSize: 15 }}
      >
        {t("finish_done")}
      </motion.button>
    </motion.div>
  );
}
