import { useState, useEffect } from "react";
import { motion } from "framer-motion";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";

interface SelectDirPageProps {
  onDirChange: (dir: string) => void;
  onValidChange: (valid: boolean) => void;
  onShortcutChange?: (create: boolean) => void;
  createDesktopShortcut?: boolean;
}

const REQUIRED_SPACE = 80_000_000;

function formatSize(bytes: number): string {
  if (bytes >= 1_000_000_000) return (bytes / 1_000_000_000).toFixed(1) + " GB";
  if (bytes >= 1_000_000) return (bytes / 1_000_000).toFixed(0) + " MB";
  if (bytes >= 1_000) return (bytes / 1_000).toFixed(0) + " KB";
  return bytes + " B";
}

export default function SelectDirPage({ onDirChange, onValidChange, onShortcutChange, createDesktopShortcut = true }: SelectDirPageProps) {
  const { t } = useTranslation();
  const [dir, setDir] = useState("");
  const [available, setAvailable] = useState<number>(-1);
  const [checking, setChecking] = useState(true);

  useEffect(() => {
    invoke<string>("get_default_install_path").then((path) => {
      setDir(path);
      onDirChange(path);
      checkSpace(path);
    });
  }, []);

  useEffect(() => {
    const valid = available >= REQUIRED_SPACE;
    onValidChange(valid);
  }, [available]);

  const checkSpace = async (path: string) => {
    try {
      setChecking(true);
      const space = await invoke<number>("check_disk_space", { path });
      setAvailable(space);
    } catch {
      setAvailable(0);
    } finally {
      setChecking(false);
    }
  };

  const handleBrowse = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t("dir_title"),
      });
      if (selected) {
        const target = `${selected}\\NexBox`;
        setDir(target);
        onDirChange(target);
        setAvailable(-1);
        checkSpace(target);
      }
    } catch { }
  };

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    setDir(val);
    onDirChange(val);
    setAvailable(-1);
    checkSpace(val);
  };

  const enoughSpace = available >= REQUIRED_SPACE;

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -20 }}
      transition={{ duration: 0.3 }}
      style={{ display: "flex", flexDirection: "column", flex: 1 }}
    >
      <h2 className="page-title">{t("dir_title")}</h2>
      <p className="page-subtitle">{t("dir_desc")}</p>

      <div className="glass-card" style={{ marginBottom: 20 }}>
        <div style={{ fontSize: 13, color: "#64748b", marginBottom: 8 }}>{t("dir_title")}</div>
        <div className="dir-selector">
          <input
            type="text"
            value={dir}
            onChange={handleInputChange}
            placeholder="C:\\Program Files\\NexBox"
          />
          <button onClick={handleBrowse}>{t("dir_browse")}</button>
        </div>
      </div>

      <div className="glass-card" style={{ display: "flex", gap: 24 }}>
        <div>
          <div style={{ fontSize: 12, color: "#94a3b8", marginBottom: 4 }}>{t("dir_space")}</div>
          <div style={{ fontSize: 16, fontWeight: 600, color: enoughSpace ? "#22c55e" : "#ef4444" }}>
            {checking ? (
              <span className="space-spinner" />
            ) : (
              formatSize(available)
            )}
          </div>
        </div>
        <div>
          <div style={{ fontSize: 12, color: "#94a3b8", marginBottom: 4 }}>{t("dir_required")}</div>
          <div style={{ fontSize: 16, fontWeight: 600, color: "#64748b" }}>
            {formatSize(REQUIRED_SPACE)}
          </div>
        </div>
      </div>

      <label className="checkbox-label" style={{ marginTop: 16 }}>
        <input
          type="checkbox"
          checked={createDesktopShortcut}
          onChange={(e) => onShortcutChange?.(e.target.checked)}
        />
        <span className="checkbox-mark" />
        {t("创建桌面快捷方式")}
      </label>

      {!enoughSpace && !checking && (
        <div style={{ marginTop: 12, fontSize: 13, color: "#ef4444" }}>
          {t("error_space")}
        </div>
      )}
    </motion.div>
  );
}
