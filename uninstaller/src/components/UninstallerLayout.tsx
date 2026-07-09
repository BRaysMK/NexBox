import type { ReactNode } from "react";

interface UninstallerLayoutProps {
  children: ReactNode;
  showFooter?: boolean;
  onPrimary?: () => void;
  primaryLabel?: string;
  primaryDisabled?: boolean;
}

export default function UninstallerLayout({
  children,
  showFooter = true,
  onPrimary,
  primaryLabel,
  primaryDisabled = false,
}: UninstallerLayoutProps) {
  return (
    <div className="installer-app">
      <div className="installer-body">
        <div className="installer-content">
          {children}
        </div>
      </div>

      {showFooter && (
        <div className="installer-footer">
          <div style={{ flex: 1 }} />
          {onPrimary && (
            <button className="btn-primary" onClick={onPrimary} disabled={primaryDisabled}>
              {primaryLabel || "开始卸载"}
            </button>
          )}
        </div>
      )}
    </div>
  );
}
