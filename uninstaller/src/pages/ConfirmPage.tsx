import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";

interface ConfirmPageProps {
  onStart: () => void;
}

export default function ConfirmPage({ onStart }: ConfirmPageProps) {
  const { t } = useTranslation();
  const [installDir, setInstallDir] = useState("");

  useEffect(() => {
    invoke<{ install_dir: string }>("get_install_info")
      .then((info) => setInstallDir(info.install_dir))
      .catch(() => setInstallDir("未知"));
  }, []);

  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.95 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.95 }}
      style={{ display: "flex", flexDirection: "column", flex: 1, gap: 20 }}
    >
      <div>
        <h1 className="page-title">{t("confirm_title")}</h1>
        <p className="page-subtitle">{t("confirm_desc")}</p>
      </div>

      <div className="glass-card">
        <div style={{ display: "flex", alignItems: "flex-start", gap: 12 }}>
          <svg width="20" height="20" viewBox="0 0 20 20" fill="none" style={{ flexShrink: 0, marginTop: 1 }}>
            <circle cx="10" cy="10" r="9" stroke="#f59e0b" strokeWidth="1.5" />
            <path d="M10 6v5" stroke="#f59e0b" strokeWidth="1.5" strokeLinecap="round" />
            <circle cx="10" cy="14" r="1" fill="#f59e0b" />
          </svg>
          <div>
            <p style={{ fontSize: 13, color: "#92400e", lineHeight: 1.6, marginBottom: 8 }}>
              {t("confirm_warning")}
            </p>
            <p style={{ fontSize: 12, color: "#64748b" }}>
              {t("confirm_location")}{" "}
              <span style={{ color: "#1a202c", fontWeight: 500 }}>{installDir}</span>
            </p>
          </div>
        </div>
      </div>
    </motion.div>
  );
}
