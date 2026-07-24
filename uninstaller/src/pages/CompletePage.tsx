import { motion } from "framer-motion";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

export default function CompletePage() {
  const { t } = useTranslation();

  const handleClose = async () => {
    // Spawn detached cleanup batch first, then close immediately
    invoke("self_delete").catch(() => {});
    getCurrentWindow().close().catch(() => {});
  };

  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.95 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.95 }}
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
        {t("complete_title")}
      </h1>

      <p className="page-subtitle" style={{ textAlign: "center" }}>
        {t("complete_desc")}
      </p>

      <motion.button
        className="btn-primary"
        onClick={handleClose}
        whileHover={{ scale: 1.02 }}
        whileTap={{ scale: 0.98 }}
        style={{ marginTop: 16, padding: "10px 48px", fontSize: 15 }}
      >
        {t("complete_btn")}
      </motion.button>
    </motion.div>
  );
}
