/**
 * 桌面歌词控制栏（仅未锁定时悬浮显示）
 *
 * 锁定状态的解锁按钮由独立小窗口（/lyrics-unlock-btn）提供，
 * 不受 WebView2 穿透影响。
 *
 * [上一句] [播放/暂停] [下一句] [随机播放] | [锁定] | [关闭]
 */

import { memo } from "react";
import {
  SkipBack,
  SkipForward,
  Shuffle,
  Play,
  Pause,
  Lock,
  X,
} from "lucide-react";
import type { PlayMode } from "@/types/music";
import type { ControlAction } from "@/hooks/useDesktopLyricsSync";

interface LyricsControlBarProps {
  isPlaying: boolean;
  playMode: PlayMode;
  onControl: (action: ControlAction) => void;
}

const btnBase: React.CSSProperties = {
  background: "transparent",
  border: "none",
  cursor: "pointer",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  padding: "6px",
  borderRadius: "50%",
  color: "rgba(255,255,255,0.9)",
  transition: "background 0.15s ease, transform 0.1s ease",
};

function LyricsControlBarInner({
  isPlaying,
  playMode,
  onControl,
}: LyricsControlBarProps) {
  return (
    <div
      style={{
        position: "absolute",
        top: "8px",
        left: "50%",
        transform: "translateX(-50%)",
        display: "flex",
        alignItems: "center",
        gap: "4px",
        background: "rgba(0,0,0,0.5)",
        backdropFilter: "blur(16px)",
        WebkitBackdropFilter: "blur(16px)",
        borderRadius: "999px",
        padding: "4px 12px",
        transition: "opacity 0.2s ease",
        boxShadow: "0 4px 16px rgba(0,0,0,0.3)",
      }}
      onMouseDown={(e) => e.stopPropagation()}
    >
      <button
        style={btnBase}
        onClick={() => onControl("prev")}
        title="上一首"
        onMouseEnter={(e) => (e.currentTarget.style.background = "rgba(255,255,255,0.15)")}
        onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
      >
        <SkipBack size={16} />
      </button>

      <button
        style={{
          ...btnBase,
          background: "rgba(255,255,255,0.2)",
        }}
        onClick={() => onControl("play-pause")}
        title={isPlaying ? "暂停" : "播放"}
        onMouseEnter={(e) => (e.currentTarget.style.background = "rgba(255,255,255,0.3)")}
        onMouseLeave={(e) => (e.currentTarget.style.background = "rgba(255,255,255,0.2)")}
      >
        {isPlaying ? <Pause size={18} /> : <Play size={18} />}
      </button>

      <button
        style={btnBase}
        onClick={() => onControl("next")}
        title="下一首"
        onMouseEnter={(e) => (e.currentTarget.style.background = "rgba(255,255,255,0.15)")}
        onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
      >
        <SkipForward size={16} />
      </button>

      <button
        style={{
          ...btnBase,
          color: playMode === "shuffle" ? "#4FC3F7" : "rgba(255,255,255,0.9)",
        }}
        onClick={() => onControl("toggle-shuffle")}
        title={playMode === "shuffle" ? "随机播放中" : "随机播放"}
        onMouseEnter={(e) => (e.currentTarget.style.background = "rgba(255,255,255,0.15)")}
        onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
      >
        <Shuffle size={16} />
      </button>

      {/* 分隔线 */}
      <div
        style={{
          width: "1px",
          height: "16px",
          background: "rgba(255,255,255,0.2)",
          margin: "0 2px",
        }}
      />

      <button
        style={btnBase}
        onClick={() => onControl("lock")}
        title="锁定歌词"
        onMouseEnter={(e) => (e.currentTarget.style.background = "rgba(255,255,255,0.15)")}
        onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
      >
        <Lock size={16} />
      </button>

      {/* 分隔线 */}
      <div
        style={{
          width: "1px",
          height: "16px",
          background: "rgba(255,255,255,0.2)",
          margin: "0 2px",
        }}
      />

      <button
        style={btnBase}
        onClick={() => onControl("close")}
        title="关闭桌面歌词"
        onMouseEnter={(e) => (e.currentTarget.style.background = "rgba(255,255,255,0.15)")}
        onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
      >
        <X size={16} />
      </button>
    </div>
  );
}

export const LyricsControlBar = memo(LyricsControlBarInner);
