import { useEffect, useRef, useState, useCallback, useMemo, memo } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Box,
  VStack,
  HStack,
  Input,
  InputGroup,
  InputLeftElement,
  Button,
  Text,
  Spinner,
  useColorModeValue,
  useToast,
  IconButton,
  Tooltip,
  Menu,
  MenuButton,
  MenuList,
  MenuItem,
  Image as ChakraImage,
  Heading,
  Portal,
  Fade,
  Popover,
  PopoverTrigger,
  PopoverContent,
  PopoverBody,
  Switch,
  SimpleGrid,
} from "@chakra-ui/react";
import {
  Search,
  Volume2,
  VolumeX,
  ListMusic,
  Repeat,
  Repeat1,
  Shuffle,
  Music as MusicIcon,
  Heart,
  ArrowLeft,
  Sparkles,
  ChevronDown,
  MonitorSpeaker,
  Settings,
  User,
  MicVocal,
  Palette,
  Droplets,
  TrendingUp,
  Film,
} from "lucide-react";
import { useMusicStore, coverProxyUrl, stopTimeSync } from "@/stores/music-store";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import type { Song, Playlist, Artist } from "@/types/music";
import { MusicLoginSection } from "@/components/MusicLoginSection";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import { buildKaraokeLines } from "@/lib/karaoke-lyrics";
import { KaraokeLyricsView } from "@/components/KaraokeLyricsView";
import { CustomColorPicker } from "@/components/special/custom-color-picker";
import { VirtualList } from "@/components/VirtualList";
import { DesktopLyricsSettingsModal } from "@/components/DesktopLyricsSettingsModal";
import { useCoverColor } from "@/hooks/use-cover-color";
import { motion, AnimatePresence } from "framer-motion";

// ═══════════════════════════════════════════════
// 动画变体定义
// ═══════════════════════════════════════════════
const listContainerVariants = {
  hidden: { opacity: 0 },
  visible: {
    opacity: 1,
    transition: { staggerChildren: 0.04, delayChildren: 0.05 },
  },
};

const listItemVariants = {
  hidden: { opacity: 0, y: 8 },
  visible: { opacity: 1, y: 0, transition: { duration: 0.25, ease: "easeOut" } },
  exit: { opacity: 0, transition: { duration: 0.15 } },
};

const dropdownVariants = {
  hidden: { opacity: 0, y: -8, scale: 0.98 },
  visible: { opacity: 1, y: 0, scale: 1, transition: { duration: 0.2, ease: "easeOut" } },
  exit: { opacity: 0, y: -4, scale: 0.98, transition: { duration: 0.12, ease: "easeIn" } },
};

const tabContentVariants = {
  hidden: { opacity: 0, x: 12 },
  visible: { opacity: 1, x: 0, transition: { duration: 0.2, ease: "easeOut" } },
  exit: { opacity: 0, x: -8, transition: { duration: 0.12, ease: "easeIn" } },
};

const scrollbarSx = (color: string) => ({
  scrollbarGutter: "stable",
  "&::-webkit-scrollbar": { width: "4px" },
  "&::-webkit-scrollbar-thumb": { background: color, borderRadius: "2px" },
  "&::-webkit-scrollbar-track": { background: "transparent" },
});

// 原生 range slider 样式：用 CSS 变量传递颜色，伪元素控制轨道和滑块外观
const rangeSliderSx = {
  "&": {
    appearance: "none",
    WebkitAppearance: "none",
    height: "6px",
    borderRadius: "3px",
    outline: "none",
    cursor: "pointer",
  },
  "&::-webkit-slider-runnable-track": {
    height: "6px",
    borderRadius: "3px",
  },
  "&::-webkit-slider-thumb": {
    WebkitAppearance: "none",
    width: "12px",
    height: "12px",
    borderRadius: "50%",
    marginTop: "-3px",
    border: "none",
  },
  "&::-moz-range-track": {
    height: "6px",
    borderRadius: "3px",
  },
  "&::-moz-range-thumb": {
    width: "12px",
    height: "12px",
    borderRadius: "50%",
    border: "none",
  },
};

/**
 * 生成滑块进度背景的 inline style。
 * 必须用 style 而非 sx 传递动态 background，否则 Emotion 会为
 * 每个唯一百分比值生成一条新 CSS 规则，播放越久 <style> 标签越大，
 * 浏览器样式重计算越慢 —— 这是"播放越久越卡"的根因。
 */
function sliderBgStyle(activeColor: string, pct: number, trackBg: string) {
  return {
    background: `linear-gradient(to right, ${activeColor} 0%, ${activeColor} ${pct}%, ${trackBg} ${pct}%, ${trackBg} 100%)`,
  };
}

// ── 圆角播放控制图标 ──
// 用 SVG path + Q 曲线实现圆角三角形，避免有棱有角

const PlayBtn = ({ size = 24 }: { size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor">
    <path d="M 8 5 Q 7 4 6 5 L 6 19 Q 7 20 8 19 L 19 13 Q 20 12 19 11 Z" />
  </svg>
);

const PauseIcon = ({ size = 24 }: { size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor">
    <rect x="6" y="4" width="4" height="16" rx="2" />
    <rect x="14" y="4" width="4" height="16" rx="2" />
  </svg>
);

const SkipBackBtn = ({ size = 24 }: { size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor">
    <rect x="4" y="5" width="3" height="14" rx="1.5" />
    <polygon points="16,6 8,12 16,18" stroke="currentColor" strokeWidth={2.5} strokeLinejoin="round" />
  </svg>
);

const SkipForwardBtn = ({ size = 24 }: { size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor">
    <polygon points="8,6 16,12 8,18" stroke="currentColor" strokeWidth={2.5} strokeLinejoin="round" />
    <rect x="17" y="5" width="3" height="14" rx="1.5" />
  </svg>
);

// ═══════════════════════════════════════════════
// 音质选项配置
// ═══════════════════════════════════════════════
const QUALITY_OPTIONS: { value: string; label: string; desc: string; svip: boolean }[] = [
  { value: "jymaster", label: "超清母带", desc: "SVIP 专属", svip: true },
  { value: "hires",    label: "高清臻音", desc: "1999k", svip: false },
  { value: "lossless", label: "无损",     desc: "1411k", svip: false },
  { value: "exhigh",   label: "极高",     desc: "999k", svip: false },
  { value: "standard", label: "标准",     desc: "128k", svip: false },
];

// ═══════════════════════════════════════════════
// LRC 解析工具：将 [mm:ss.xx] 格式的歌词文本解析为行数组
// ═══════════════════════════════════════════════
interface LyricLine {
  time: number; // 秒
  text: string;
  translation?: string;
}

function parseLrc(lyric: string, translation?: string): LyricLine[] {
  if (!lyric) return [];
  const lines: LyricLine[] = [];
  const transMap = new Map<number, string>();

  // 解析翻译歌词
  if (translation) {
    const transLines = translation.split("\n");
    for (const line of transLines) {
      const match = line.match(/\[(\d+):(\d+(?:\.\d+)?)\]/);
      if (match) {
        const time = parseInt(match[1]) * 60 + parseFloat(match[2]);
        const text = line.replace(/\[\d+:\d+(?:\.\d+)?\]/, "").trim();
        if (text) transMap.set(time, text);
      }
    }
  }

  // 解析主歌词
  const mainLines = lyric.split("\n");
  for (const line of mainLines) {
    const matches = [...line.matchAll(/\[(\d+):(\d+(?:\.\d+)?)\]/g)];
    if (matches.length === 0) continue;
    const text = line.replace(/\[\d+:\d+(?:\.\d+)?\]/g, "").trim();
    if (!text) continue;
    for (const m of matches) {
      const time = parseInt(m[1]) * 60 + parseFloat(m[2]);
      lines.push({ time, text, translation: transMap.get(time) });
    }
  }

  lines.sort((a, b) => a.time - b.time);
  return lines;
}

// ═══════════════════════════════════════════════
// 歌词滚动组件：超长歌词轮播显示
// ═══════════════════════════════════════════════
function LyricMarquee({ text, isActive, isTranslation, fontSize, activeColor, textColor, subTextColor }: {
  text: string; isActive: boolean; isTranslation?: boolean; fontSize: number;
  activeColor: string; textColor: string; subTextColor: string;
}) {
  const textRef = useRef<HTMLSpanElement>(null);
  const [overflows, setOverflows] = useState(false);

  useEffect(() => {
    if (textRef.current) {
      const parent = textRef.current.parentElement;
      if (parent) setOverflows(textRef.current.scrollWidth > parent.clientWidth);
    }
  }, [text]);

  return (
    <Box
      overflow="hidden"
      whiteSpace="nowrap"
      textAlign="center"
      w="100%"
    >
      {isActive && overflows ? (
        <Box
          as="span"
          display="inline-block"
          whiteSpace="nowrap"
          fontSize={`${fontSize}px`}
          fontWeight="bold"
          color={isTranslation ? subTextColor : activeColor}
          textAlign="center"
          sx={{
            animation: `lyricScroll ${Math.max(text.length * 0.08, 3)}s linear infinite`,
            "@keyframes lyricScroll": {
              "0%": { transform: "translateX(0)" },
              "100%": { transform: "translateX(-50%)" },
            },
          }}
        >
          {text}&nbsp;&nbsp;&nbsp;{text}
        </Box>
      ) : (
        <Box
          as="span"
          ref={textRef}
          display="inline-block"
          whiteSpace="nowrap"
          fontSize={`${isTranslation ? fontSize - 2 : fontSize}px`}
          fontWeight={isActive ? "bold" : "normal"}
          color={isTranslation ? subTextColor : (isActive ? activeColor : textColor)}
          sx={overflows ? { textOverflow: "ellipsis", overflow: "hidden", maxWidth: "100%" } : {}}
        >
          {text}
        </Box>
      )}
    </Box>
  );
}

// ═══════════════════════════════════════════════
// ProgressSection — 独立的进度条组件
// 自己管理 timeupdate 监听，不触发 ExpandedPlayer 重渲染
// ═══════════════════════════════════════════════
const ProgressSection = memo(function ProgressSection({
  activeColor,
  subTextColor,
  sliderTrackBg,
  audioRef,
  currentSongId,
}: {
  activeColor: string;
  subTextColor: string;
  sliderTrackBg: string;
  audioRef: HTMLAudioElement | null;
  currentSongId?: string | number;
}) {
  const [localCurrentTime, setLocalCurrentTime] = useState(0);
  const [localDuration, setLocalDuration] = useState(0);
  const isUserSeekingRef = useRef(false);
  const pendingSeekRef = useRef(0);

  // 切歌时重置
  useEffect(() => {
    setLocalCurrentTime(0);
    setLocalDuration(audioRef?.duration && isFinite(audioRef.duration) ? audioRef.duration : 0);
  }, [currentSongId]); // eslint-disable-line react-hooks/exhaustive-deps

  // 监听 audio timeupdate（含挂载时同步当前时间）
  useEffect(() => {
    if (!audioRef) return;
    if (audioRef.duration && isFinite(audioRef.duration)) {
      setLocalDuration(audioRef.duration);
    }
    // 挂载时同步当前时间（修复暂停后收起/展开进度条归零的问题）
    if (!isUserSeekingRef.current) {
      setLocalCurrentTime(audioRef.currentTime);
    }
    const onTimeUpdate = () => {
      if (isUserSeekingRef.current) return;
      setLocalCurrentTime(audioRef.currentTime);
    };
    const onLoadedMetadata = () => {
      if (isFinite(audioRef.duration)) setLocalDuration(audioRef.duration);
    };
    audioRef.addEventListener("timeupdate", onTimeUpdate);
    audioRef.addEventListener("loadedmetadata", onLoadedMetadata);
    return () => {
      audioRef.removeEventListener("timeupdate", onTimeUpdate);
      audioRef.removeEventListener("loadedmetadata", onLoadedMetadata);
    };
  }, [audioRef]);

  const handleSeekDrag = useCallback((v: number) => {
    if (!isUserSeekingRef.current) return;
    pendingSeekRef.current = v;
    setLocalCurrentTime(v);
  }, []);

  const handleSeekCommit = useCallback(() => {
    if (pendingSeekRef.current !== 0 || isUserSeekingRef.current) {
      const targetTime = pendingSeekRef.current;
      useMusicStore.getState().seekTo(targetTime);
      setTimeout(() => {
        isUserSeekingRef.current = false;
        setLocalCurrentTime(targetTime);
      }, 300);
    }
  }, []);

  return (
    <HStack spacing={3} w="100%" maxW="600px">
      <Text color={subTextColor} fontSize="xs" w="45px" textAlign="center">
        {formatTime(localCurrentTime)}
      </Text>
      <Box
        as="input"
        type="range"
        min={0}
        max={localDuration || 100}
        step={0.1}
        value={localCurrentTime}
        onMouseDown={() => { isUserSeekingRef.current = true; }}
        onTouchStart={() => { isUserSeekingRef.current = true; }}
        onChange={(e) => handleSeekDrag(parseFloat((e.target as HTMLInputElement).value))}
        onMouseUp={handleSeekCommit}
        onTouchEnd={handleSeekCommit}
        tabIndex={-1}
        aria-hidden="true"
        flex={1}
        style={sliderBgStyle(activeColor, localDuration ? (localCurrentTime / localDuration) * 100 : 0, sliderTrackBg)}
        sx={{
          ...rangeSliderSx,
          "&::-webkit-slider-thumb": { ...rangeSliderSx["&::-webkit-slider-thumb"], background: activeColor },
          "&::-moz-range-thumb": { ...rangeSliderSx["&::-moz-range-thumb"], background: activeColor },
          "&::-webkit-slider-runnable-track": { ...rangeSliderSx["&::-webkit-slider-runnable-track"], background: "transparent" },
          "&::-moz-range-track": { ...rangeSliderSx["&::-moz-range-track"], background: "transparent" },
        }}
      />
      <Text color={subTextColor} fontSize="xs" w="45px" textAlign="center">
        {formatTime(localDuration)}
      </Text>
    </HStack>
  );
});

// ═══════════════════════════════════════════════
// ExpandedPlayer — 展开的全屏播放器
// 点击播放器封面展开，左侧封面+信息，右侧歌词
// ═══════════════════════════════════════════════
interface ExpandedPlayerProps {
  onClose: () => void;
}

const ExpandedPlayer = memo(function ExpandedPlayer({ onClose }: ExpandedPlayerProps) {
  const currentSong = useMusicStore((s) => s.currentSong);
  const isPlaying = useMusicStore((s) => s.isPlaying);
  const volume = useMusicStore((s) => s.volume);
  const playMode = useMusicStore((s) => s.playMode);
  const proxyPort = useMusicStore((s) => s.proxyPort);
  const audioRef = useMusicStore((s) => s.audioRef);
  const currentLyrics = useMusicStore((s) => s.currentLyrics);
  const loadingLyrics = useMusicStore((s) => s.loadingLyrics);
  const likedSongIds = useMusicStore((s) => s.likedSongIds);
  const loginInfo = useMusicStore((s) => s.loginInfo);
  const playbackQuality = useMusicStore((s) => s.playbackQuality);
  const currentQuality = useMusicStore((s) => s.currentQuality);
  const currentBitrate = useMusicStore((s) => s.currentBitrate);
  const lyricsFontSize = useMusicStore((s) => s.lyricsFontSize);
  const lyricsHighlightColor = useMusicStore((s) => s.lyricsHighlightColor);
  const expandedStyle = useMusicStore((s) => s.expandedStyle);
  const dynamicEnabled = useMusicStore((s) => s.dynamicEnabled);
  const coverFilmEffect = useMusicStore((s) => s.coverFilmEffect);
  const desktopLyricsVisible = useMusicStore((s) => s.desktopLyricsVisible);
  const playQueue = useMusicStore((s) => s.playQueue);
  const currentIndex = useMusicStore((s) => s.currentIndex);

  const [isClosing, setIsClosing] = useState(false);

  const { getActiveColor, getHoverColor, getContrastTextColor } = useThemeColor();
  const activeColor = getActiveColor();
  const contrastText = getContrastTextColor();
  const hoverBg = getHoverColor(false);

  // 封面主色提取
  const coverUrl = currentSong ? coverProxyUrl(currentSong.cover, proxyPort) : "";
  const coverColor = useCoverColor(coverUrl);

  // 现代模式：根据封面颜色决定文字色和背景
  const [cr, cg, cb] = coverColor.rgb;
  const modernBgSolid = coverColor.hex;
  const modernBgDark = `rgb(${Math.round(cr * 0.25)},${Math.round(cg * 0.25)},${Math.round(cb * 0.25)})`;
  const modernBgGradient = dynamicEnabled
    ? `linear-gradient(135deg, ${modernBgSolid} 0%, ${modernBgSolid} 40%, ${modernBgDark} 100%)`
    : `linear-gradient(135deg, ${modernBgSolid} 0%, ${modernBgSolid} 40%, ${modernBgDark} 100%)`;
  // 动态模式通过 CSS animation 实现渐变色流动
  const modernBgDynamic = `linear-gradient(45deg, ${modernBgSolid}, ${modernBgDark}, ${modernBgSolid})`;
  const modernBgFinal = dynamicEnabled ? modernBgDynamic : modernBgGradient;
  const modernTextColor = coverColor.isLight ? "#1a1a2e" : "#f0f0f0";
  const modernSubTextColor = coverColor.isLight ? "#4a4a5e" : "#b0b0b0";
  const modernBorderColor = coverColor.isLight ? "rgba(0,0,0,0.12)" : "rgba(255,255,255,0.15)";
  const modernHoverBg = coverColor.isLight ? "rgba(0,0,0,0.06)" : "rgba(255,255,255,0.12)";

  const isModern = expandedStyle === "modern";

  const bgColor = useColorModeValue("rgba(255,255,255,0.25)", "rgba(0,0,0,0.25)");
  const glassBorderColor = useColorModeValue("rgba(255,255,255,0.2)", "rgba(255,255,255,0.1)");
  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const sliderTrackBg = useColorModeValue("rgba(0,0,0,0.1)", "rgba(255,255,255,0.9)");

  // 文字颜色覆写（现代模式）
  const effectiveTextColor = isModern ? modernTextColor : textColor;
  const effectiveSubTextColor = isModern ? modernSubTextColor : subTextColor;
  const effectiveHoverBg = isModern ? modernHoverBg : hoverBg;

  // 下拉菜单配色：白底黑字
  const menuBg = "white";
  const menuBorder = "rgba(0,0,0,0.1)";
  const menuText = "#1a1a2e";
  const menuMuted = "#666";

  const ModeIcon = playMode === "one" ? Repeat1 : playMode === "shuffle" ? Shuffle : Repeat;

  // memoize scrollbarSx，避免每次渲染创建新对象导致 KaraokeLyricsView 不必要重渲染
  const memoScrollbarSx = useMemo(() => scrollbarSx(activeColor), [activeColor]);

  // 歌词加载由 playSong 在 URL 获取前并行触发，不再需要此处 useEffect 重复加载

  // 歌词解析：优先 YRC 逐字歌词，降级为 LRC 逐行歌词
  const karaokeLines = useMemo(() => {
    return buildKaraokeLines(currentLyrics);
  }, [currentLyrics]);

  // 关闭动画定时器清理，防止组件卸载后定时器仍触发
  const closeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    return () => {
      if (closeTimerRef.current) clearTimeout(closeTimerRef.current);
    };
  }, []);

  if (!currentSong) return null;

  const isLiked = likedSongIds.has(currentSong.id);

  const handleCloseWithAnimation = useCallback(() => {
    setIsClosing(true);
    closeTimerRef.current = setTimeout(() => onClose(), 300);
  }, [onClose]);

  return (
    <Box
      position="absolute"
      top={0}
      left={0}
      right={0}
      bottom={0}
      zIndex={9999}
      bg={isModern ? modernBgFinal : bgColor}
      backdropFilter={isModern ? "none" : "blur(20px)"}
      borderRadius="xl"
      overflow="hidden"
      boxShadow="xl"
      sx={{
        "@keyframes expandedPlayerSlideUp": {
          from: { transform: "translateY(100%)", opacity: 0 },
          to: { transform: "translateY(0)", opacity: 1 },
        },
        "@keyframes expandedPlayerSlideDown": {
          from: { transform: "translateY(0)", opacity: 1 },
          to: { transform: "translateY(100%)", opacity: 0 },
        },
        display: "flex",
        flexDirection: "column",
        WebkitBackdropFilter: isModern ? "none" : "blur(20px)",
        animation: (() => {
          const slide = isClosing
            ? "expandedPlayerSlideDown 0.3s cubic-bezier(0.4, 0, 0.2, 1) forwards"
            : "expandedPlayerSlideUp 0.35s cubic-bezier(0.4, 0, 0.2, 1)";
          const dynamic = (dynamicEnabled && isModern) ? ", dynamicBg 8s ease infinite" : "";
          return `${slide}${dynamic}`;
        })(),
        ...(dynamicEnabled && isModern ? {
          backgroundSize: "400% 400%",
          "@keyframes dynamicBg": {
            "0%": { backgroundPosition: "0% 50%" },
            "50%": { backgroundPosition: "100% 50%" },
            "100%": { backgroundPosition: "0% 50%" },
          },
        } : {}),
      }}
    >
      {/* 顶部栏：关闭按钮 */}
      <HStack justify="space-between" p={4} flexShrink={0}>
        <HStack spacing={3}>
          <Tooltip label="收起">
            <IconButton
              aria-label="Close"
              icon={<ChevronDown size={24} />}
              size="sm"
              variant="ghost"
              onClick={handleCloseWithAnimation}
              sx={{ color: effectiveTextColor, _hover: { bg: effectiveHoverBg } }}
            />
          </Tooltip>
          <Text color={effectiveSubTextColor} fontSize="sm">正在播放</Text>
        </HStack>
      </HStack>

      {/* 主体：左（封面+信息）+ 右（歌词） */}
      <HStack flex={1} spacing={8} px={8} pb={2} align="stretch" overflow="hidden" minH={0}>
        {/* 左侧：封面 + 歌曲信息 */}
        <VStack spacing={6} align="center" justify="center" flex={1} minW={0}>
          {/* 碟片模式 */}
          {coverFilmEffect ? (
            <Box
              position="relative"
              w={{ base: "240px", md: "320px", lg: "360px" }}
              h={{ base: "240px", md: "320px", lg: "360px" }}
            >
              {/* 唱臂（磁头）— SVG 弯臂 + 圆点支座 + 数据线接头 */}
              <Box
                position="absolute"
                top={{ base: "-15px", md: "-18px", lg: "-20px" }}
                right={{ base: "-8px", md: "-10px", lg: "-12px" }}
                w={{ base: "48px", md: "56px", lg: "62px" }}
                h={{ base: "180px", md: "220px", lg: "250px" }}
                zIndex={5}
                sx={{
                  transformOrigin: "top right",
                  transform: isPlaying
                    ? "rotate(30deg)"
                    : "rotate(-10deg)",
                  transition: "transform 0.6s cubic-bezier(0.4, 0, 0.2, 1)",
                }}
              >
                <svg
                  viewBox="0 0 48 220"
                  style={{
                    overflow: "visible",
                    width: "100%",
                    height: "100%",
                  }}
                  preserveAspectRatio="xMidYMid meet"
                >
                  {/* 圆点支座（旋转轴） */}
                  <circle cx="42" cy="8" r="7" fill="url(#pivotGrad)" stroke="rgba(255,255,255,0.2)" strokeWidth="0.5" />
                  <circle cx="40" cy="6" r="2" fill="rgba(255,255,255,0.3)" />
                  {/* 唱臂杆 — 垂直段 + 30° 弯折 */}
                  <path
                    d="M 42 14 L 42 135 Q 42 145, 30 155 L 18 175"
                    fill="none"
                    stroke="url(#armGrad)"
                    strokeWidth="4"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                  {/* 唱臂内层高光 */}
                  <path
                    d="M 42 14 L 42 135 Q 42 145, 30 155 L 18 175"
                    fill="none"
                    stroke="rgba(255,255,255,0.12)"
                    strokeWidth="1.5"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                  {/* 数据线接头磁头 — 金属外壳 + 插槽 + 触点 */}
                  {/* 金属外壳主体 */}
                  <rect
                    x="9" y="170" width="18" height="22" rx="4"
                    fill="url(#headShellGrad)"
                    stroke="rgba(0,0,0,0.3)"
                    strokeWidth="0.5"
                  />
                  {/* 外壳顶部高光 */}
                  <rect x="10.5" y="171.5" width="15" height="4" rx="2" fill="url(#headTopShine)" />
                  {/* 左右金属侧面高光 */}
                  <rect x="9.5" y="172" width="1.5" height="18" rx="0.75" fill="rgba(255,255,255,0.25)" />
                  <rect x="25" y="172" width="1.5" height="18" rx="0.75" fill="rgba(0,0,0,0.15)" />
                  {/* 底部插槽开口（黑色凹槽） */}
                  <rect x="12" y="184" width="12" height="6" rx="1.5" fill="#000" />
                  {/* 插槽内的金属触点条 */}
                  <rect x="13" y="185.5" width="10" height="1" rx="0.5" fill="#555" />
                  <rect x="13" y="187.5" width="10" height="1" rx="0.5" fill="#444" />
                  {/* 底部连接处缩窄颈 */}
                  <rect x="14" y="192" width="8" height="4" rx="1" fill="url(#neckGrad)" />
                  {/* 唱针针尖 */}
                  <path d="M 18 196 L 18 202 L 16.5 204" fill="none" stroke="#999" strokeWidth="0.8" strokeLinecap="round" />

                  {/* 渐变定义 */}
                  <defs>
                    <radialGradient id="pivotGrad" cx="35%" cy="30%">
                      <stop offset="0%" stopColor="#aaa" />
                      <stop offset="60%" stopColor="#555" />
                      <stop offset="100%" stopColor="#222" />
                    </radialGradient>
                    <linearGradient id="armGrad" x1="0" y1="0" x2="1" y2="0">
                      <stop offset="0%" stopColor="#666" />
                      <stop offset="50%" stopColor="#3a3a3a" />
                      <stop offset="100%" stopColor="#222" />
                    </linearGradient>
                    <linearGradient id="headShellGrad" x1="0" y1="0" x2="1" y2="1">
                      <stop offset="0%" stopColor="#e8e8e8" />
                      <stop offset="35%" stopColor="#aaa" />
                      <stop offset="65%" stopColor="#888" />
                      <stop offset="100%" stopColor="#555" />
                    </linearGradient>
                    <linearGradient id="headTopShine" x1="0" y1="0" x2="1" y2="0">
                      <stop offset="0%" stopColor="rgba(255,255,255,0.1)" />
                      <stop offset="40%" stopColor="rgba(255,255,255,0.45)" />
                      <stop offset="60%" stopColor="rgba(255,255,255,0.45)" />
                      <stop offset="100%" stopColor="rgba(255,255,255,0.05)" />
                    </linearGradient>
                    <linearGradient id="neckGrad" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="0%" stopColor="#666" />
                      <stop offset="100%" stopColor="#333" />
                    </linearGradient>
                  </defs>
                </svg>
              </Box>

              {/* 黑胶碟片底盘 */}
              <Box
                position="absolute"
                top={0}
                left={0}
                w="100%"
                h="100%"
                borderRadius="50%"
                sx={{
                  background:
                    "radial-gradient(circle at center, #1a1a1a 0%, #0a0a0a 100%)",
                  boxShadow:
                    "0 4px 16px rgba(0,0,0,0.5), 0 0 0 1px rgba(255,255,255,0.05)",
                }}
              />
              {/* 碟片同心圆纹理 — 预渲染为伪元素避免重绘 */}
              <Box
                position="absolute"
                top={0}
                left={0}
                w="100%"
                h="100%"
                borderRadius="50%"
                pointerEvents="none"
                sx={{
                  background:
                    "repeating-radial-gradient(circle at center, transparent 0px, transparent 3px, rgba(255,255,255,0.02) 3px, rgba(255,255,255,0.02) 4px)",
                  willChange: "auto",
                }}
              />
              {/* 旋转的封面区域 — 用包裹层居中，内层只做旋转 */}
              <Box
                position="absolute"
                top="50%"
                left="50%"
                sx={{
                  transform: "translate(-50%, -50%)",
                }}
              >
                <Box
                  w={{ base: "150px", md: "200px", lg: "220px" }}
                  h={{ base: "150px", md: "200px", lg: "220px" }}
                  borderRadius="50%"
                  overflow="hidden"
                  sx={{
                    animation: "vinylSpin 12s linear infinite",
                    animationPlayState: isPlaying ? "running" : "paused",
                    boxShadow: "0 0 0 2px rgba(255,255,255,0.08)",
                    willChange: "transform",
                    backfaceVisibility: "hidden",
                  }}
                >
                  <ChakraImage
                    src={coverProxyUrl(currentSong.cover, proxyPort)}
                    alt=""
                    w="100%"
                    h="100%"
                    objectFit="cover"
                    fallback={<Box w="100%" h="100%" bg="gray.700" />}
                  />
                  {/* 封面上的高光反射 */}
                  <Box
                    position="absolute"
                    top={0}
                    left={0}
                    w="100%"
                    h="100%"
                    pointerEvents="none"
                    sx={{
                      background:
                        "linear-gradient(135deg, rgba(255,255,255,0.15) 0%, transparent 40%, transparent 60%, rgba(0,0,0,0.2) 100%)",
                    }}
                  />
                </Box>
              </Box>
            </Box>
          ) : (
            /* 普通模式 */
            <Box
              position="relative"
              borderRadius="2xl"
              overflow="hidden"
              boxShadow="2xl"
              sx={{
                transition: "transform 0.3s ease",
                _hover: { transform: "scale(1.02)" },
              }}
            >
              <ChakraImage
                src={coverProxyUrl(currentSong.cover, proxyPort)}
                alt=""
                w={{ base: "200px", md: "280px", lg: "320px" }}
                h={{ base: "200px", md: "280px", lg: "320px" }}
                objectFit="cover"
                fallback={<Box w="280px" h="280px" bg="gray.700" borderRadius="2xl" />}
              />
            </Box>
          )}

          <VStack spacing={1} align="center" maxW="400px">
            <Text color={effectiveTextColor} fontSize="xl" fontWeight="bold" noOfLines={1} textAlign="center">
              {currentSong.name}
            </Text>
            <Text color={effectiveSubTextColor} fontSize="md" noOfLines={1} textAlign="center">
              {currentSong.artist}
            </Text>
            {currentSong.album && (
              <Text color={effectiveSubTextColor} fontSize="sm" noOfLines={1} textAlign="center">
                {currentSong.album}
              </Text>
            )}
          </VStack>
        </VStack>

        {/* 右侧：歌词 */}
        <VStack flex={1} align="stretch" minW={0} h="100%" overflow="hidden" justify="flex-start">
          <KaraokeLyricsView
            lines={karaokeLines}
            loading={loadingLyrics}
            fontSize={lyricsFontSize}
            activeColor={activeColor}
            highlightColor={lyricsHighlightColor}
            textColor={effectiveTextColor}
            subTextColor={effectiveSubTextColor}
            scrollbarSx={memoScrollbarSx}
            audioRef={audioRef}
            isPlaying={isPlaying}
          />
        </VStack>
      </HStack>

      {/* 底部：播放控制 + 进度条（全宽居中） */}
      <VStack spacing={4} w="100%" flexShrink={0} pb={4} px={8}>
        {/* 控制按钮：主按钮居中，红心在左侧，音质在右侧 */}
        <Box position="relative" w="100%">
          {/* 左下方红心 + 样式切换 */}
          <Box position="absolute" left={0} top="50%" transform="translateY(-50%)" zIndex={1} display="flex" alignItems="center" gap={1}>
            {loginInfo?.logged_in && (
              <Tooltip label={isLiked ? "取消红心" : "红心"}>
                <IconButton
                  aria-label="Like"
                  icon={<Heart size={20} fill={isLiked ? "#e53e3e" : "none"} />}
                  size="md"
                  variant="ghost"
                  onClick={() => useMusicStore.getState().toggleLike(currentSong.id)}
                  sx={{
                    color: isLiked ? "#e53e3e" : effectiveTextColor,
                    _hover: { bg: effectiveHoverBg },
                  }}
                />
              </Tooltip>
            )}
            <Tooltip label={coverFilmEffect ? "关闭碟片模式" : "开启碟片模式"}>
              <IconButton
                aria-label="Toggle film effect"
                icon={<Film size={18} />}
                size="sm"
                variant="ghost"
                onClick={() => useMusicStore.getState().setCoverFilmEffect(!coverFilmEffect)}
                sx={{
                  color: coverFilmEffect ? activeColor : effectiveTextColor,
                  _hover: { bg: effectiveHoverBg },
                }}
              />
            </Tooltip>
            <Tooltip label={isModern ? "切换通透样式" : "切换现代样式"}>
              <IconButton
                aria-label="Toggle style"
                icon={isModern ? <Droplets size={18} /> : <Palette size={18} />}
                size="sm"
                variant="ghost"
                onClick={() => {
                  const next = expandedStyle === "glass" ? "modern" : "glass";
                  useMusicStore.getState().setExpandedStyle(next);
                }}
                sx={{ color: effectiveTextColor, _hover: { bg: effectiveHoverBg } }}
              />
            </Tooltip>
            {isModern && (
              <HStack spacing={1}>
                <Text fontSize="xs" color={effectiveSubTextColor} fontWeight="medium">动态</Text>
                <Switch
                  size="sm"
                  isChecked={dynamicEnabled}
                  onChange={(e) => useMusicStore.getState().setDynamicEnabled(e.target.checked)}
                  sx={{
                    "& .chakra-switch__track": { bg: "rgba(255,255,255,0.3)" },
                    "& .chakra-switch__track[data-checked]": { bg: `${activeColor} !important` },
                  }}
                />
              </HStack>
            )}
          </Box>
          <HStack spacing={4} justify="center">
          <Tooltip label={playMode === "one" ? "单曲循环" : playMode === "shuffle" ? "随机播放" : "列表循环"}>
            <IconButton
              aria-label="Play mode"
              icon={<ModeIcon size={20} />}
              size="md"
              variant="ghost"
              sx={{ color: playMode !== "list" ? activeColor : effectiveSubTextColor, _hover: { bg: effectiveHoverBg } }}
              onClick={() => useMusicStore.getState().togglePlayMode()}
            />
          </Tooltip>
          <IconButton
            aria-label="Prev"
            icon={<SkipBackBtn size={24} />}
            size="md"
            variant="ghost"
            onClick={() => useMusicStore.getState().prevTrack()}
            sx={{ color: effectiveTextColor, _hover: { bg: effectiveHoverBg } }}
          />
          <IconButton
            aria-label="Play/Pause"
            icon={isPlaying ? <PauseIcon size={24} /> : <PlayBtn size={24} />}
            size="md"
            variant="ghost"
            sx={{ color: effectiveTextColor, _hover: { bg: activeColor, color: contrastText } }}
            onClick={() => useMusicStore.getState().togglePlay()}
          />
          <IconButton
            aria-label="Next"
            icon={<SkipForwardBtn size={24} />}
            size="md"
            variant="ghost"
            onClick={() => useMusicStore.getState().nextTrack()}
            sx={{ color: effectiveTextColor, _hover: { bg: effectiveHoverBg } }}
          />
          {/* 音量控制：悬停向右展开滑块 */}
          <Box
            role="group"
            position="relative"
            sx={{
              "&:hover .volume-slider": {
                width: "80px",
                opacity: 1,
                ml: "6px",
              },
            }}
          >
            <IconButton
              aria-label="Mute"
              icon={volume === 0 ? <VolumeX size={20} /> : <Volume2 size={20} />}
              size="md"
              variant="ghost"
              onClick={() => {
                const s = useMusicStore.getState();
                s.setVolume(volume === 0 ? s.prevVolume : 0);
              }}
              sx={{ color: effectiveTextColor, _hover: { bg: effectiveHoverBg } }}
            />
            <Box
              className="volume-slider"
              as="input"
              type="range"
              min={0}
              max={1}
              step={0.01}
              value={volume}
              onChange={(e) => useMusicStore.getState().setVolume(parseFloat((e.target as HTMLInputElement).value))}
              tabIndex={-1}
              style={sliderBgStyle(activeColor, volume * 100, sliderTrackBg)}
              sx={{
                ...rangeSliderSx,
                position: "absolute",
                left: "100%",
                top: "50%",
                transform: "translateY(-50%)",
                width: "0px",
                opacity: 0,
                ml: "0px",
                transition: "width 0.25s ease, opacity 0.2s ease, margin-left 0.25s ease",
                cursor: "pointer",
                "&::-webkit-slider-thumb": {
                  ...rangeSliderSx["&::-webkit-slider-thumb"],
                  background: activeColor,
                },
                "&::-moz-range-thumb": {
                  ...rangeSliderSx["&::-moz-range-thumb"],
                  background: activeColor,
                },
                "&::-webkit-slider-runnable-track": {
                  ...rangeSliderSx["&::-webkit-slider-runnable-track"],
                  background: "transparent",
                },
                "&::-moz-range-track": {
                  ...rangeSliderSx["&::-moz-range-track"],
                  background: "transparent",
                },
              }}
            />
          </Box>
        </HStack>

          {/* 歌词高亮颜色 + 歌词字体大小 + 音质选择 - 右侧 */}
          <Box position="absolute" right={0} top="50%" transform="translateY(-50%)">
            <HStack spacing={2} align="center">
              {/* 歌词高亮颜色选择器 */}
              <Tooltip label="歌词高亮颜色">
                <Box>
                  <CustomColorPicker
                    color={lyricsHighlightColor}
                    onChange={(c) => useMusicStore.getState().setLyricsHighlightColor(c)}
                    compact
                  />
                </Box>
              </Tooltip>
              <Tooltip label={`歌词字号: ${lyricsFontSize}px`}>
                <HStack spacing={1} align="center">
                  <Text fontSize="xs" color={effectiveSubTextColor} fontWeight="bold" flexShrink={0}>A</Text>
                  <Box
                    as="input"
                    type="range"
                    min={17}
                    max={28}
                    step={1}
                    value={lyricsFontSize}
                    onChange={(e) => useMusicStore.getState().setLyricsFontSize(parseInt((e.target as HTMLInputElement).value))}
                    tabIndex={-1}
                    w="60px"
                    style={sliderBgStyle(activeColor, ((lyricsFontSize - 17) / 11) * 100, sliderTrackBg)}
                    sx={{
                      ...rangeSliderSx,
                      "&::-webkit-slider-thumb": { ...rangeSliderSx["&::-webkit-slider-thumb"], background: activeColor },
                      "&::-moz-range-thumb": { ...rangeSliderSx["&::-moz-range-thumb"], background: activeColor },
                      "&::-webkit-slider-runnable-track": { ...rangeSliderSx["&::-webkit-slider-runnable-track"], background: "transparent" },
                      "&::-moz-range-track": { ...rangeSliderSx["&::-moz-range-track"], background: "transparent" },
                    }}
                  />
                </HStack>
              </Tooltip>
              <Popover placement="top-end" isLazy strategy="fixed">
                <Tooltip label="音质选择">
                  <PopoverTrigger>
                    <IconButton
                      aria-label="Quality"
                      icon={<Box as="span" fontSize="11px" fontWeight="bold">{currentQuality || QUALITY_OPTIONS.find((o) => o.value === playbackQuality)?.label || "高清臻音"}</Box>}
                      size="md"
                      variant="ghost"
                      sx={{ color: activeColor, minW: "auto", px: 2, _hover: { bg: effectiveHoverBg } }}
                    />
                  </PopoverTrigger>
                </Tooltip>
                <Portal>
                  <Fade in>
                    <PopoverContent w="180px" bg={menuBg} border="1px solid" borderColor={menuBorder} borderRadius="lg" boxShadow="lg">
                      <PopoverBody p={1}>
                        {QUALITY_OPTIONS.map((opt) => {
                          const isSvip = loginInfo?.is_svip ?? false;
                          const locked = opt.svip && !isSvip;
                          return (
                            <HStack
                              key={opt.value}
                              spacing={3}
                              px={3}
                              py={1.5}
                              cursor={locked ? "not-allowed" : "pointer"}
                              opacity={locked ? 0.4 : 1}
                              bg={opt.value === playbackQuality ? `${activeColor}22` : "transparent"}
                              _hover={locked ? {} : { bg: "rgba(0,0,0,0.05)" }}
                              borderRadius="md"
                              onClick={() => { if (!locked) useMusicStore.getState().setPlaybackQuality(opt.value as any); }}
                            >
                              <Text fontSize="sm" fontWeight={opt.value === playbackQuality ? "bold" : "normal"} color={menuText}>{opt.label}</Text>
                              <Text fontSize="xs" color={menuMuted}>{opt.desc}</Text>
                            </HStack>
                          );
                        })}
                      </PopoverBody>
                    </PopoverContent>
                  </Fade>
                </Portal>
              </Popover>
            <Tooltip label={desktopLyricsVisible ? "关闭桌面歌词" : "开启桌面歌词"}>
              <IconButton
                aria-label="Desktop lyrics"
                icon={<MonitorSpeaker size={20} />}
                size="md"
                variant="ghost"
                onClick={() => useMusicStore.getState().toggleDesktopLyrics()}
                sx={{
                  color: desktopLyricsVisible ? activeColor : effectiveTextColor,
                  _hover: { bg: effectiveHoverBg },
                  opacity: desktopLyricsVisible ? 1 : 0.7,
                }}
              />
            </Tooltip>
            <Popover placement="top-end" isLazy strategy="fixed">
              <Tooltip label="播放队列">
                <PopoverTrigger>
                  <IconButton
                    aria-label="Queue"
                    icon={<ListMusic size={20} />}
                    size="md"
                    variant="ghost"
                    sx={{ color: effectiveTextColor, _hover: { bg: effectiveHoverBg } }}
                  />
                </PopoverTrigger>
              </Tooltip>
              <Portal>
                <Fade in>
                  <PopoverContent
                    w="260px"
                    bg={menuBg}
                    border="1px solid"
                    borderColor={menuBorder}
                    borderRadius="lg"
                    boxShadow="lg"
                  >
                    <PopoverBody p={1}>
                      {playQueue.length === 0 ? (
                        <Text color={menuMuted} fontSize="sm" px={3} py={2}>播放列表为空</Text>
                      ) : (
                        <VirtualList
                          items={playQueue}
                          itemHeight={48}
                          height={Math.min(384, playQueue.length * 48)}
                          scrollToIndex={currentIndex}
                          getKey={(s, i) => `${s.id}-${i}`}
                          renderItem={(s, i) => (
                            <HStack
                              spacing={2}
                              px={3}
                              h="100%"
                              cursor="pointer"
                              bg={i === currentIndex ? `${activeColor}22` : "transparent"}
                              _hover={{ bg: "rgba(0,0,0,0.05)" }}
                              borderRadius="md"
                              overflow="hidden"
                              onClick={() => useMusicStore.getState().playSong(s, playQueue)}
                            >
                              <Text fontSize="xs" color={(i === currentIndex) ? activeColor : menuMuted} w="20px" flexShrink={0}>
                                {i === currentIndex ? "▶" : i + 1}
                              </Text>
                              <VStack spacing={0} flex={1} minW={0} align="start">
                                <Text
                                  fontSize="sm"
                                  fontWeight={(i === currentIndex) ? "bold" : "normal"}
                                  color={(i === currentIndex) ? activeColor : menuText}
                                  w="100%"
                                  overflow="hidden"
                                  textOverflow="ellipsis"
                                  whiteSpace="nowrap"
                                >
                                  {s.name}
                                </Text>
                                <Text
                                  fontSize="xs"
                                  color={menuMuted}
                                  w="100%"
                                  overflow="hidden"
                                  textOverflow="ellipsis"
                                  whiteSpace="nowrap"
                                >
                                  {s.artist}
                                </Text>
                              </VStack>
                            </HStack>
                          )}
                        />
                      )}
                    </PopoverBody>
                  </PopoverContent>
                </Fade>
              </Portal>
            </Popover>
            </HStack>
          </Box>
        </Box>

        {/* 进度条 — 独立组件，自己管理 timeupdate，不触发 ExpandedPlayer 重渲染 */}
        <ProgressSection
          activeColor={activeColor}
          subTextColor={effectiveSubTextColor}
          sliderTrackBg={sliderTrackBg}
          audioRef={audioRef}
          currentSongId={currentSong.id}
        />
      </VStack>
    </Box>
  );
});


// ═══════════════════════════════════════════════
// 播放时间格式化工具
// ═══════════════════════════════════════════════
const formatTime = (time: number): string => {
  if (isNaN(time)) return "0:00";
  const m = Math.floor(time / 60);
  const s = Math.floor(time % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
};

// ═══════════════════════════════════════════════
// 播放器进度条 — 独立迷你组件
// 自己管理 timeupdate，播放期间只有本组件随进度重渲染
// PlayerBar 本体不会被 timeupdate 触发重渲染
// ═══════════════════════════════════════════════
const PlayerProgress = memo(function PlayerProgress({
  activeColor,
  subTextColor,
  sliderTrackBg,
  currentSong,
  hidden,
}: {
  activeColor: string;
  subTextColor: string;
  sliderTrackBg: string;
  currentSong: Song | null;
  hidden?: boolean;
}) {
  const audioRef = useMusicStore((s) => s.audioRef);
  const storeDuration = useMusicStore((s) => s.duration);
  const [localCurrentTime, setLocalCurrentTime] = useState(0);
  const [localDuration, setLocalDuration] = useState(0);
  const isUserSeekingRef = useRef(false);
  const pendingSeekRef = useRef(0);

  useEffect(() => {
    setLocalDuration(storeDuration);
  }, [storeDuration]);

  useEffect(() => {
    if (!audioRef || hidden) return;
    if (audioRef.duration && isFinite(audioRef.duration)) {
      setLocalDuration(audioRef.duration);
    }
    if (!isUserSeekingRef.current) {
      setLocalCurrentTime(audioRef.currentTime);
    }
    const onTimeUpdate = () => {
      if (isUserSeekingRef.current) return;
      setLocalCurrentTime(audioRef.currentTime);
    };
    const onLoadedMetadata = () => {
      setLocalDuration(audioRef.duration);
    };
    audioRef.addEventListener("timeupdate", onTimeUpdate);
    audioRef.addEventListener("loadedmetadata", onLoadedMetadata);
    return () => {
      audioRef.removeEventListener("timeupdate", onTimeUpdate);
      audioRef.removeEventListener("loadedmetadata", onLoadedMetadata);
    };
  }, [audioRef, hidden]);

  useEffect(() => {
    setLocalCurrentTime(0);
  }, [currentSong]);

  const handleSeekDrag = useCallback((v: number) => {
    if (!isUserSeekingRef.current) return;
    pendingSeekRef.current = v;
    setLocalCurrentTime(v);
  }, []);

  const handleSeekCommit = useCallback(() => {
    if (pendingSeekRef.current !== 0 || isUserSeekingRef.current) {
      const targetTime = pendingSeekRef.current;
      useMusicStore.getState().seekTo(targetTime);
      setTimeout(() => {
        isUserSeekingRef.current = false;
        setLocalCurrentTime(targetTime);
      }, 300);
    }
  }, []);

  return (
    <HStack spacing={2}>
      <Text color={subTextColor} fontSize="xs" w="40px" textAlign="center">
        {formatTime(localCurrentTime)}
      </Text>
      <Box
        as="input"
        type="range"
        min={0}
        max={localDuration || 100}
        step={0.1}
        value={localCurrentTime}
        onMouseDown={() => { isUserSeekingRef.current = true; }}
        onTouchStart={() => { isUserSeekingRef.current = true; }}
        onChange={(e) => handleSeekDrag(parseFloat((e.target as HTMLInputElement).value))}
        onMouseUp={handleSeekCommit}
        onTouchEnd={handleSeekCommit}
        tabIndex={-1}
        aria-hidden="true"
        flex={1}
        style={sliderBgStyle(activeColor, localDuration ? (localCurrentTime / localDuration) * 100 : 0, sliderTrackBg)}
        sx={{
          ...rangeSliderSx,
          "&::-webkit-slider-thumb": { ...rangeSliderSx["&::-webkit-slider-thumb"], background: activeColor },
          "&::-moz-range-thumb": { ...rangeSliderSx["&::-moz-range-thumb"], background: activeColor },
          "&::-webkit-slider-runnable-track": { ...rangeSliderSx["&::-webkit-slider-runnable-track"], background: "transparent" },
          "&::-moz-range-track": { ...rangeSliderSx["&::-moz-range-track"], background: "transparent" },
        }}
      />
      <Text color={subTextColor} fontSize="xs" w="40px" textAlign="center">
        {formatTime(localDuration)}
      </Text>
    </HStack>
  );
});

// 关键：用 local state 监听 timeupdate，播放期间不更新 store
// 这样搜索框等组件完全不受播放影响
// ═══════════════════════════════════════════════
const PlayerBar = memo(function PlayerBar({ onExpand, hidden }: { onExpand?: () => void; hidden?: boolean }) {
  const currentSong = useMusicStore((s) => s.currentSong);
  const isPlaying = useMusicStore((s) => s.isPlaying);
  const volume = useMusicStore((s) => s.volume);
  const playMode = useMusicStore((s) => s.playMode);
  const playQueue = useMusicStore((s) => s.playQueue);
  const currentIndex = useMusicStore((s) => s.currentIndex);
  const proxyPort = useMusicStore((s) => s.proxyPort);
  const playbackQuality = useMusicStore((s) => s.playbackQuality);
  const currentQuality = useMusicStore((s) => s.currentQuality);
  const currentBitrate = useMusicStore((s) => s.currentBitrate);
  const loginInfo = useMusicStore((s) => s.loginInfo);
  const desktopLyricsVisible = useMusicStore((s) => s.desktopLyricsVisible);
  const [dlSettingsOpen, setDlSettingsOpen] = useState(false);

  const { getActiveColor, getHoverColor, getContrastTextColor, getBorderColor } = useThemeColor();

  const activeColor = getActiveColor();
  const contrastText = getContrastTextColor();
  const hoverBg = getHoverColor(false);

  const borderColor = useColorModeValue("gray.200", "#333333");
  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const dropdownBg = useColorModeValue("white", "#1a1a1a");
  const sliderTrackBg = useColorModeValue("rgba(255,255,255,0.9)", "#333333");

  const ModeIcon = playMode === "one" ? Repeat1 : playMode === "shuffle" ? Shuffle : Repeat;

  if (!currentSong) {
    return (
      <LiquidGlassCard
        p={3}
        flexShrink={0}
        sx={{ marginTop: "auto", position: "relative" }}
      >
        <HStack spacing={4} align="center" justify="center" h="56px">
          <Text color={subTextColor} fontSize="sm">未播放音乐</Text>
        </HStack>
      </LiquidGlassCard>
    );
  }

  return (
    <>
    <LiquidGlassCard
      p={3}
      flexShrink={0}
      cursor={onExpand ? "pointer" : "default"}
      onClick={(e) => {
        const target = e.target as HTMLElement;
        if (!target.closest("button, input, [role='menubutton'], [role='slider']")) {
          onExpand?.();
        }
      }}
      sx={{ marginTop: "auto", position: "relative" }}
    >
      <VStack spacing={2} align="stretch">
        <HStack spacing={4} align="center">
          {/* 左侧：封面 + 标题 */}
          <Box
            as="button"
            onClick={onExpand}
            cursor={onExpand ? "pointer" : "default"}
            borderRadius="md"
            overflow="hidden"
            flexShrink={0}
            sx={{ border: "none", bg: "transparent", p: 0 }}
            _hover={onExpand ? { transform: "scale(1.05)", transition: "transform 0.2s" } : {}}
          >
            <ChakraImage
              src={coverProxyUrl(currentSong.cover, proxyPort)}
              alt=""
              w="48px"
              h="48px"
              borderRadius="md"
              objectFit="cover"
              fallback={<Box w="48px" h="48px" borderRadius="md" bg="gray.700" />}
            />
          </Box>
          <VStack spacing={0} align="start" flexShrink={0} minW={0} maxW="200px">
            <Text color={textColor} fontWeight="medium" fontSize="sm" noOfLines={1}>
              {currentSong.name}
            </Text>
            <Text color={subTextColor} fontSize="xs" noOfLines={1}>
              {currentSong.artist}
            </Text>
          </VStack>

          {/* 中间：播放控制按钮组（绝对居中） */}
          <HStack spacing={1} position="absolute" left="50%" transform="translateX(-50%)">
            <Tooltip label="上一首">
              <IconButton aria-label="Prev" icon={<SkipBackBtn size={18} />} size="sm" variant="ghost" onClick={() => useMusicStore.getState().prevTrack()} />
            </Tooltip>
            <Tooltip label={isPlaying ? "暂停" : "播放"}>
               <IconButton
                aria-label="Play/Pause"
                icon={isPlaying ? <PauseIcon size={18} /> : <PlayBtn size={18} />}
                size="sm"
                variant="ghost"
                sx={{ color: textColor, _hover: { bg: activeColor, color: contrastText } }}
                onClick={() => useMusicStore.getState().togglePlay()}
              />
            </Tooltip>
            <Tooltip label="下一首">
              <IconButton aria-label="Next" icon={<SkipForwardBtn size={18} />} size="sm" variant="ghost" onClick={() => useMusicStore.getState().nextTrack()} />
            </Tooltip>
            <Tooltip label={playMode === "one" ? "单曲循环" : playMode === "shuffle" ? "随机播放" : "列表循环"}>
              <IconButton
                aria-label="Play mode"
                icon={<ModeIcon size={16} />}
                size="sm"
                variant="ghost"
                sx={{ color: playMode !== "list" ? activeColor : subTextColor, _hover: { bg: hoverBg } }}
                onClick={() => useMusicStore.getState().togglePlayMode()}
              />
            </Tooltip>
          </HStack>

          {/* 右侧：音质 + 音量 + 播放队列 */}
          <HStack spacing={1} w="180px" ml="auto">
            <Menu>
              <Tooltip label="音质选择">
                <MenuButton
                  as={IconButton}
                  aria-label="Quality"
                  size="sm"
                  variant="ghost"
                  sx={{
                    fontSize: "10px",
                    fontWeight: "bold",
                    color: activeColor,
                    minW: "auto",
                    px: 1,
                    _hover: { bg: hoverBg },
                  }}
                >
                  {currentQuality || QUALITY_OPTIONS.find((o) => o.value === playbackQuality)?.label || "高清臻音"}
                </MenuButton>
              </Tooltip>
              <Portal>
                <MenuList minW="180px" bg={dropdownBg} borderColor={borderColor}>
                  {QUALITY_OPTIONS.map((opt) => {
                    const isSvip = loginInfo?.is_svip ?? false;
                    const locked = opt.svip && !isSvip;
                    return (
                      <MenuItem
                        key={opt.value}
                        onClick={() => {
                          if (!locked) useMusicStore.getState().setPlaybackQuality(opt.value as any);
                        }}
                        bg={opt.value === playbackQuality ? `${activeColor}33` : undefined}
                        opacity={locked ? 0.4 : 1}
                        cursor={locked ? "not-allowed" : "pointer"}
                      >
                        <HStack spacing={3} w="100%" justify="space-between">
                          <Text fontSize="sm" fontWeight={opt.value === playbackQuality ? "bold" : "normal"} color={textColor}>
                            {opt.label}
                          </Text>
                          <Text fontSize="xs" color={subTextColor}>
                            {opt.desc}
                          </Text>
                        </HStack>
                      </MenuItem>
                    );
                  })}
                </MenuList>
              </Portal>
            </Menu>
            <Tooltip label="静音">
              <IconButton
                aria-label="Mute"
                icon={volume === 0 ? <VolumeX size={16} /> : <Volume2 size={16} />}
                size="sm"
                variant="ghost"
                onClick={() => {
                  const s = useMusicStore.getState();
                  s.setVolume(volume === 0 ? s.prevVolume : 0);
                }}
              />
            </Tooltip>
            <Box
            as="input"
            type="range"
            min={0}
            max={1}
            step={0.01}
            value={volume}
            onChange={(e) => useMusicStore.getState().setVolume(parseFloat((e.target as HTMLInputElement).value))}
            tabIndex={-1}
            w="60px"
            style={sliderBgStyle(activeColor, volume * 100, sliderTrackBg)}
            sx={{
              ...rangeSliderSx,
              "&::-webkit-slider-thumb": {
                ...rangeSliderSx["&::-webkit-slider-thumb"],
                background: activeColor,
              },
              "&::-moz-range-thumb": {
                ...rangeSliderSx["&::-moz-range-thumb"],
                background: activeColor,
              },
              "&::-webkit-slider-runnable-track": {
                ...rangeSliderSx["&::-webkit-slider-runnable-track"],
                background: "transparent",
              },
              "&::-moz-range-track": {
                ...rangeSliderSx["&::-moz-range-track"],
                background: "transparent",
              },
            }}
          />
          </HStack>

          {/* 桌面歌词开关 + 设置 */}
          <HStack spacing={1} flexShrink={0}>
            <Tooltip label={desktopLyricsVisible ? "关闭桌面歌词" : "打开桌面歌词"}>
              <IconButton
                aria-label="Desktop Lyrics"
                icon={<MonitorSpeaker size={16} />}
                size="sm"
                variant="ghost"
                sx={{
                  color: desktopLyricsVisible ? activeColor : subTextColor,
                  _hover: { bg: hoverBg },
                }}
                onClick={() => useMusicStore.getState().toggleDesktopLyrics()}
              />
            </Tooltip>
            <Tooltip label="桌面歌词设置">
              <IconButton
                aria-label="Lyrics Settings"
                icon={<Settings size={16} />}
                size="sm"
                variant="ghost"
                sx={{ color: textColor, _hover: { bg: hoverBg } }}
                onClick={() => setDlSettingsOpen(true)}
              />
            </Tooltip>
          </HStack>

          <Menu isLazy>
            {({ onClose }) => (
              <>
                <Tooltip label="播放队列">
                  <MenuButton as={IconButton} aria-label="Queue" icon={<ListMusic size={18} />} size="sm" variant="ghost" />
                </Tooltip>
                <MenuList minW="240px" py={1} bg={dropdownBg} borderColor={borderColor}>
                  {playQueue.length === 0 ? (
                    <Text px={3} py={2} fontSize="sm" color={subTextColor}>播放列表为空</Text>
                  ) : (
                    <VirtualList
                      items={playQueue}
                      itemHeight={48}
                      height={Math.min(288, playQueue.length * 48)}
                      scrollToIndex={currentIndex}
                      getKey={(s, i) => `${s.id}-${i}`}
                      renderItem={(s, i) => (
                        <VStack
                          spacing={0}
                          h="100%"
                          justify="center"
                          align="start"
                          px={3}
                          cursor="pointer"
                          bg={i === currentIndex ? `${activeColor}33` : undefined}
                          _hover={{ bg: hoverBg }}
                          onClick={() => { useMusicStore.getState().playSong(s, playQueue); onClose(); }}
                        >
                          <Text
                            fontSize="sm"
                            fontWeight={i === currentIndex ? "bold" : "normal"}
                            color={i === currentIndex ? activeColor : textColor}
                            w="100%"
                            overflow="hidden"
                            textOverflow="ellipsis"
                            whiteSpace="nowrap"
                          >
                            {i === currentIndex ? "▶" : i + 1}. {s.name}
                          </Text>
                          <Text
                            fontSize="xs"
                            color={subTextColor}
                            w="100%"
                            overflow="hidden"
                            textOverflow="ellipsis"
                            whiteSpace="nowrap"
                          >
                            {s.artist}
                          </Text>
                        </VStack>
                      )}
                    />
                  )}
                </MenuList>
              </>
            )}
          </Menu>
        </HStack>

        {/* 进度条 — 独立迷你组件，播放期间只有本组件随 timeupdate 重渲染 */}
        <PlayerProgress
          activeColor={activeColor}
          subTextColor={subTextColor}
          sliderTrackBg={sliderTrackBg}
          currentSong={currentSong}
          hidden={hidden}
        />
      </VStack>
    </LiquidGlassCard>
    <DesktopLyricsSettingsModal isOpen={dlSettingsOpen} onClose={() => setDlSettingsOpen(false)} />
    </>
    );
  });

// ═══════════════════════════════════════════════
// SongRow — memoized，避免不必要重渲染
// ═══════════════════════════════════════════════
interface SongRowProps {
  song: Song;
  index: number;
  queue: Song[];
  isCurrent: boolean;
  isPlaying: boolean;
  isLiked: boolean;
  isLoggedIn: boolean;
  proxyPort: number;
  activeColor: string;
  hoverBg: string;
  itemHoverBg: string;
  itemActiveBg: string;
  textColor: string;
  subTextColor: string;
  liquidGlassEnabled: boolean;
  onPlay: (song: Song, queue: Song[]) => void;
  onTogglePlay: () => void;
  onToggleLike: (songId: string) => void;
  onArtistClick?: (artist: Artist) => void;
}

const SongRow = memo(function SongRow({
  song,
  index,
  queue,
  isCurrent,
  isPlaying,
  isLiked,
  isLoggedIn,
  proxyPort,
  activeColor,
  hoverBg,
  itemHoverBg,
  itemActiveBg,
  textColor,
  subTextColor,
  liquidGlassEnabled,
  onPlay,
  onTogglePlay,
  onToggleLike,
  onArtistClick,
}: SongRowProps) {
  return (
    <HStack
      key={`${song.provider}-${song.id}-${index}`}
      spacing={3}
      p={2}
      borderRadius="lg"
      cursor="pointer"
      _hover={{ bg: liquidGlassEnabled ? hoverBg : itemHoverBg }}
      bg={isCurrent ? itemActiveBg : "transparent"}
      onClick={() => onPlay(song, queue)}
      transition="background 0.15s"
    >
      <ChakraImage
        src={coverProxyUrl(song.cover, proxyPort)}
        alt=""
        w="40px"
        h="40px"
        borderRadius="md"
        objectFit="cover"
        fallback={<Box w="40px" h="40px" borderRadius="md" bg="gray.700" />}
      />
      <VStack spacing={0} align="start" flex={1} minW={0}>
        <Text color={textColor} fontSize="sm" noOfLines={1} fontWeight={isCurrent ? "bold" : "normal"}>
          {song.name}
        </Text>
        <HStack spacing={1} minW={0}>
          {song.artists.length > 0 && song.artists[0].id && onArtistClick ? (
            <Text
              color={subTextColor}
              fontSize="xs"
              noOfLines={1}
              cursor="pointer"
              _hover={{ color: activeColor, textDecoration: "underline" }}
              onClick={(e) => {
                e.stopPropagation();
                onArtistClick(song.artists[0]);
              }}
            >
              {song.artist}
            </Text>
          ) : (
            <Text color={subTextColor} fontSize="xs" noOfLines={1}>
              {song.artist}
            </Text>
          )}
          {song.album && (
            <Text color={subTextColor} fontSize="xs" noOfLines={1}>
              {" "}- {song.album}
            </Text>
          )}
        </HStack>
      </VStack>
      <Text color={subTextColor} fontSize="xs" flexShrink={0}>
        {formatTime(song.duration / 1000)}
      </Text>
      {/* 语言标签 */}
      {song.language > 0 && song.language <= 4 && (
        <Box
          as="span"
          fontSize="10px"
          color={subTextColor}
          bg={useColorModeValue("gray.100", "rgba(255,255,255,0.08)")}
          px={1.5}
          py={0.5}
          borderRadius="sm"
          flexShrink={0}
          lineHeight="1.2"
        >
          {["", "华语", "日语", "韩语", "欧美"][song.language]}
        </Box>
      )}
      {isLoggedIn && (
        <Tooltip label={isLiked ? "取消红心" : "红心"}>
          <IconButton
            aria-label="Like"
            icon={<Heart size={14} fill={isLiked ? "#e53e3e" : "none"} color={isLiked ? "#e53e3e" : "currentColor"} />}
            size="xs"
            variant="ghost"
            onClick={(e) => {
              e.stopPropagation();
              onToggleLike(song.id);
            }}
          />
        </Tooltip>
      )}
      <Tooltip label="播放">
        <IconButton
          aria-label="Play"
          icon={isCurrent && isPlaying ? <PauseIcon size={14} /> : <PlayBtn size={14} />}
          size="xs"
          variant="ghost"
          sx={{ color: activeColor, _hover: { bg: hoverBg } }}
          onClick={(e) => {
            e.stopPropagation();
            if (isCurrent) {
              onTogglePlay();
            } else {
              onPlay(song, queue);
            }
          }}
        />
      </Tooltip>
    </HStack>
  );
});

// ═══════════════════════════════════════════════
// SearchBox — 独立 memo 组件，管理搜索状态
// 不订阅 currentTime/duration，播放时不会重渲染
// ═══════════════════════════════════════════════
interface SearchBoxProps {
  onUnifiedSearch: (searchInput: string) => void;
  onArtistClick?: (artist: Artist) => void;
}

const SearchBox = memo(function SearchBox({
  onUnifiedSearch,
  onArtistClick,
}: SearchBoxProps) {
  // ── 非受控 input：用 ref 跟踪值，不使用 value prop ──
  // 这样即使组件重渲染，input 也不会丢失焦点
  const inputRef = useRef<HTMLInputElement>(null);
  const searchInputRef = useRef("");
  const searchBoxRef = useRef<HTMLDivElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // 订阅 searching + likedSongIds，让爱心状态实时更新；不订阅 currentTime/duration
  const likedSongIds = useMusicStore((s) => s.likedSongIds);
  const searching = useMusicStore((s) => s.searching);
  const searchingArtists = useMusicStore((s) => s.searchingArtists);

  const [showSearchDropdown, setShowSearchDropdown] = useState(false);
  const [dropdownResults, setDropdownResults] = useState<Song[]>([]);

  // actions 是稳定的
  const storeActionsRef = useRef(useMusicStore.getState());
  const storeActions = storeActionsRef.current;

  const { liquidGlassEnabled } = useBackground();
  const { getActiveColor, getHoverColor, getContrastTextColor, getBorderColor } = useThemeColor();

  const activeColor = getActiveColor();
  const contrastText = getContrastTextColor();
  const hoverBg = getHoverColor(false);
  const themeBorder = getBorderColor();

  const bgColor = useColorModeValue("white", "#111111");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const itemHoverBg = useColorModeValue("gray.50", "rgba(255,255,255,0.05)");
  const itemActiveBg = useColorModeValue(`${activeColor}22`, "rgba(255,255,255,0.08)");
  const dropdownBg = useColorModeValue("white", "#1a1a1a");
  const glassInputBg = useColorModeValue("rgba(255,255,255,0.15)", "rgba(0,0,0,0.25)");
  const dropdownBorder = useColorModeValue("gray.200", "#333333");

  // 点击外部关闭搜索下拉
  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (searchBoxRef.current && !searchBoxRef.current.contains(e.target as Node)) {
        setShowSearchDropdown(false);
      }
    };
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  // 卸载时清理 debounce 定时器，防止回调操作已卸载组件
  useEffect(() => {
    return () => {
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
        debounceRef.current = null;
      }
    };
  }, []);

  // ── 搜索：边输入边出预览（debounce 300ms）──
  // 非受控：直接从 input ref 读取值，不触发 setState
  const handleInputChange = useCallback((value: string) => {
    searchInputRef.current = value;
    if (debounceRef.current) clearTimeout(debounceRef.current);
    if (!value.trim()) {
      setDropdownResults([]);
      setShowSearchDropdown(false);
      return;
    }
    debounceRef.current = setTimeout(async () => {
      await storeActions.search(value);
      const results = useMusicStore.getState().searchResults;
      setDropdownResults(results);
      setShowSearchDropdown(true);
    }, 300);
  }, [storeActions]);

  // 为了让 Input 有稳定的 onChange handler（防止失焦），将它提取为 useCallback
  const handleSearchChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    handleInputChange(e.currentTarget.value);
  }, [handleInputChange]);

  // ── 回车：统一搜索，同时搜歌曲、歌单和歌手 ──
  const handleSearchEnter = useCallback(() => {
    const value = searchInputRef.current;
    if (!value.trim()) return;
    if (debounceRef.current) clearTimeout(debounceRef.current);
    Promise.all([
      storeActions.search(value),
      storeActions.searchArtists(value),
      storeActions.searchPlaylists(value),
    ]).then(() => {
      onUnifiedSearch(value);
      setShowSearchDropdown(false);
    });
  }, [storeActions, onUnifiedSearch]);

  // ── 搜索按钮：统一搜索 ──
  const handleSearchButtonClick = useCallback(() => {
    const value = searchInputRef.current;
    if (!value.trim()) return;
    if (debounceRef.current) clearTimeout(debounceRef.current);
    Promise.all([
      storeActions.search(value),
      storeActions.searchArtists(value),
      storeActions.searchPlaylists(value),
    ]).then(() => {
      onUnifiedSearch(value);
      setShowSearchDropdown(false);
    });
  }, [storeActions, onUnifiedSearch]);

  // ── 回调函数（稳定引用）──
  const onPlay = useCallback((song: Song, queue: Song[]) => {
    useMusicStore.getState().playSong(song, queue);
  }, []);
  const onTogglePlay = useCallback(() => {
    useMusicStore.getState().togglePlay();
  }, []);
  const onToggleLike = useCallback((songId: string) => {
    useMusicStore.getState().toggleLike(songId);
  }, []);

  // ── 渲染歌曲行 ──
  const renderSongRow = useCallback((song: Song, index: number, queue: Song[]) => {
    const state = useMusicStore.getState();
    return (
      <SongRow
        key={`${song.provider}-${song.id}-${index}`}
        song={song}
        index={index}
        queue={queue}
        isCurrent={state.currentSong?.id === song.id}
        isPlaying={state.isPlaying}
        isLiked={likedSongIds.has(song.id)}
        isLoggedIn={!!state.loginInfo?.logged_in}
        proxyPort={state.proxyPort}
        activeColor={activeColor}
        hoverBg={hoverBg}
        itemHoverBg={itemHoverBg}
        itemActiveBg={itemActiveBg}
        textColor={textColor}
        subTextColor={subTextColor}
        liquidGlassEnabled={liquidGlassEnabled}
        onPlay={onPlay}
        onTogglePlay={onTogglePlay}
        onToggleLike={onToggleLike}
        onArtistClick={onArtistClick}
      />
    );
  }, [likedSongIds, activeColor, hoverBg, itemHoverBg, itemActiveBg, textColor, subTextColor, liquidGlassEnabled, onPlay, onTogglePlay, onToggleLike, onArtistClick]);

  return (
    <Box ref={searchBoxRef} position="relative" flexShrink={0}>
      <motion.div
        initial={{ opacity: 0, y: -6 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3, ease: "easeOut" }}
      >
      <HStack spacing={2}>
        <InputGroup size="md">
          <InputLeftElement pointerEvents="none">
            <Search size={16} color={subTextColor} />
          </InputLeftElement>
          {/* 非受控 input：不使用 value prop，用 ref 跟踪值 */}
          {/* 这样即使组件因任何原因重渲染，input 焦点也不会丢失 */}
          <Input
            ref={inputRef}
            placeholder="搜索歌曲和歌手... (回车查看全部)"
            defaultValue=""
            onChange={handleSearchChange}
            onKeyDown={(e) => e.key === "Enter" && handleSearchEnter()}
            bg={liquidGlassEnabled ? glassInputBg : bgColor}
            borderColor={liquidGlassEnabled ? themeBorder : borderColor}
            borderRadius="xl"
            transition="border-color 0.2s, box-shadow 0.2s"
            _focus={{ borderColor: activeColor, boxShadow: `0 0 0 2px ${activeColor}33, 0 0 0 1px ${activeColor}` }}
          />
        </InputGroup>
        <Button
          leftIcon={<Search size={16} />}
          onClick={handleSearchButtonClick}
          isLoading={searching || searchingArtists}
          size="md"
          borderRadius="xl"
          flexShrink={0}
          sx={{
            bg: activeColor,
            color: contrastText,
            _hover: { bg: activeColor, filter: "brightness(0.9)" },
            _active: { bg: activeColor, filter: "brightness(0.8)" },
          }}
        >
          搜索
        </Button>
      </HStack>
      </motion.div>

      {/* 搜索下拉预览 */}
      <AnimatePresence>
        {showSearchDropdown && dropdownResults.length > 0 && (
          <motion.div
            key="search-dropdown"
            initial="hidden"
            animate="visible"
            exit="exit"
            variants={dropdownVariants}
            style={{
              padding: "8px",
              position: "absolute",
              top: "50px",
              left: 0,
              right: 0,
              zIndex: 30,
              maxHeight: "320px",
              overflowY: "auto",
              background: dropdownBg,
              borderRadius: "0.5rem",
              boxShadow: "0 4px 16px rgba(0,0,0,0.2)",
            }}
          >
            <motion.div
              variants={listContainerVariants}
              initial="hidden"
              animate="visible"
              style={{ display: "flex", flexDirection: "column", gap: "4px" }}
            >
              {dropdownResults.slice(0, 6).map((song, i) => (
                <motion.div key={`${song.provider}-${song.id}-${i}`} variants={listItemVariants}>
                  {renderSongRow(song, i, dropdownResults)}
                </motion.div>
              ))}
              <motion.div variants={listItemVariants}>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => {
                    const val = searchInputRef.current;
                    Promise.all([
                      storeActions.search(val),
                      storeActions.searchArtists(val),
                      storeActions.searchPlaylists(val),
                    ]).then(() => {
                      onUnifiedSearch(val);
                      setShowSearchDropdown(false);
                    });
                  }}
                  sx={{ color: activeColor, _hover: { bg: hoverBg } }}
                >
                  查看全部 ({dropdownResults.length})
                </Button>
              </motion.div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </Box>
  );
});

// ═══════════════════════════════════════════════
// Main MusicPage
// ═══════════════════════════════════════════════
export default function MusicPage() {
  // ── 使用独立选择器，每个字段用 Object.is 比较 ──
  // 比 useShallow 更可靠：播放时 timeupdate 只改 currentTime，
  // 这些选择器都不会触发重渲染
  const currentSong = useMusicStore((s) => s.currentSong);
  const isPlaying = useMusicStore((s) => s.isPlaying);
  const searchResults = useMusicStore((s) => s.searchResults);
  const userPlaylists = useMusicStore((s) => s.userPlaylists);
  const leftPlaylistTracks = useMusicStore((s) => s.leftPlaylistTracks);
  const leftPlaylistMeta = useMusicStore((s) => s.leftPlaylistMeta);
  const rightPlaylistTracks = useMusicStore((s) => s.rightPlaylistTracks);
  const rightPlaylistMeta = useMusicStore((s) => s.rightPlaylistMeta);
  const likedSongIds = useMusicStore((s) => s.likedSongIds);
  const dailyRecommendPlaylists = useMusicStore((s) => s.dailyRecommendPlaylists);
  const recommendSongs = useMusicStore((s) => s.recommendSongs);
  const loginInfo = useMusicStore((s) => s.loginInfo);
  const playbackSource = useMusicStore((s) => s.playbackSource);

  const providerName = useMemo(() => {
    const map: Record<string, string> = {
      netease: "网易云音乐",
      kugou: "酷狗音乐",
      qqmusic: "QQ 音乐",
    };
    return map[playbackSource] ?? playbackSource;
  }, [playbackSource]);
  const searching = useMusicStore((s) => s.searching);
  const loadingPlaylists = useMusicStore((s) => s.loadingPlaylists);
  const userPlaylistsError = useMusicStore((s) => s.userPlaylistsError);
  const loadingLeftTracks = useMusicStore((s) => s.loadingLeftTracks);
  const loadingRightTracks = useMusicStore((s) => s.loadingRightTracks);
  const proxyPort = useMusicStore((s) => s.proxyPort);
  const artistSearchResults = useMusicStore((s) => s.artistSearchResults);
  const artistSongs = useMusicStore((s) => s.artistSongs);
  const selectedArtist = useMusicStore((s) => s.selectedArtist);
  const searchingArtists = useMusicStore((s) => s.searchingArtists);
  const loadingArtistSongs = useMusicStore((s) => s.loadingArtistSongs);
  const playlistSearchResults = useMusicStore((s) => s.playlistSearchResults);
  const searchingPlaylists = useMusicStore((s) => s.searchingPlaylists);
  const musicToast = useMusicStore((s) => s.musicToast);

  const toast = useToast();

  // 监听 musicToast 变化，弹出提示
  useEffect(() => {
    if (musicToast) {
      toast({
        title: "提示",
        description: musicToast.message,
        status: "warning",
        duration: 3000,
        isClosable: true,
      });
      useMusicStore.setState({ musicToast: null });
    }
  }, [musicToast, toast]);

  // actions 是稳定的，用 useRef 只获取一次，避免每次渲染重新创建导致 useCallback 失效
  const storeActionsRef = useRef(useMusicStore.getState());
  const storeActions = storeActionsRef.current;

  const audioRef = useRef<HTMLAudioElement | null>(null);
  const [viewMode, setViewMode] = useState<"main" | "unifiedSearch" | "fullArtistList" | "artistDetail">("main");
  const [searchInput, setSearchInput] = useState("");
  const [searchTab, setSearchTab] = useState<"songs" | "playlists" | "artists">("songs");
  const previousViewRef = useRef<typeof viewMode>("main");
  const [leftPanelView, setLeftPanelView] = useState<"playlists" | "tracks">("playlists");
  const [rightPanelView, setRightPanelView] = useState<"recommendations" | "tracks" | "daily">("recommendations");
  const [expandedPlayer, setExpandedPlayer] = useState(false);

  // 搜索结果中展开的歌单
  const [searchExpandedPlaylist, setSearchExpandedPlaylist] = useState<Playlist | null>(null);
  const [searchExpandedTracks, setSearchExpandedTracks] = useState<Song[]>([]);
  const [searchLoadingExpanded, setSearchLoadingExpanded] = useState(false);

  const officialCharts = useMusicStore((s) => s.officialCharts);

  const { liquidGlassEnabled } = useBackground();
  const { getActiveColor, getHoverColor, getContrastTextColor, getBorderColor } = useThemeColor();

  const activeColor = getActiveColor();
  const hoverBg = getHoverColor(false);

  const borderColor = useColorModeValue("gray.200", "#333333");
  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const itemHoverBg = useColorModeValue("gray.50", "rgba(255,255,255,0.05)");
  const itemActiveBg = useColorModeValue(`${activeColor}22`, "rgba(255,255,255,0.08)");

  // memoize scrollbarSx，避免每次渲染创建新对象导致子组件不必要重渲染
  const memoScrollbarSx = useMemo(() => scrollbarSx(activeColor), [activeColor]);

  useEffect(() => {
    const storeState = useMusicStore.getState();
    const audio = storeState.audioRef ?? new Audio();
    const isExisting = !!storeState.audioRef;

    audioRef.current = audio;
    storeActions.setAudioRef(audio);

    if (!isExisting) {
      audio.addEventListener("ended", () => {
        useMusicStore.getState().nextTrack();
      });

      let recovering = false;
      audio.addEventListener("error", async () => {
        if (recovering) return;
        recovering = true;
        const state = useMusicStore.getState();
        if (state.currentSong && state.isPlaying) {
          const savedTime = audio.currentTime;
          try {
            await state.playSong(state.currentSong);
            audio.currentTime = savedTime;
          } catch {}
        }
        recovering = false;
      });
    }

    const initAndResume = async () => {
      await storeActions.init();
      const state = useMusicStore.getState();
      if (state.currentSong && !isExisting) {
        state.playSong(state.currentSong);
      }
    };
    initAndResume();

    return () => {
      // 离开页面不暂停 — Audio 留在 store 中继续播放
      // 停止桌面歌词时间同步定时器，避免 100ms 间隔的空转
      stopTimeSync();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 自动加载推荐歌单（登录后且无推荐数据时）
  useEffect(() => {
    if (loginInfo?.logged_in && dailyRecommendPlaylists.length === 0) {
      storeActions.loadRecommendations();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loginInfo?.logged_in]);

  const handleBack = useCallback(() => {
    if (viewMode === "artistDetail") {
      setViewMode(previousViewRef.current);
    } else {
      setViewMode("main");
      storeActions.clearArtistState();
    }
  }, [viewMode, storeActions]);

  const handleBackToMain = useCallback(() => {
    setViewMode("main");
    storeActions.clearArtistState();
  }, [storeActions]);

  // 展开播放器时加载歌词
  const handleExpandPlayer = useCallback(() => {
    const song = useMusicStore.getState().currentSong;
    if (song) {
      useMusicStore.getState().loadLyricsForSong(song);
    }
    setExpandedPlayer(true);
  }, []);

  const handleCloseExpandedPlayer = useCallback(() => {
    setExpandedPlayer(false);
  }, []);

  // 统一搜索：进入综合搜索结果页
  const handleUnifiedSearch = useCallback((input: string) => {
    setSearchInput(input);
    setSearchExpandedPlaylist(null);
    setSearchExpandedTracks([]);
    setViewMode("unifiedSearch");
  }, []);

  // 从统一搜索进入全部歌手列表
  const handleShowAllArtists = useCallback(() => {
    setViewMode("fullArtistList");
  }, []);

  // 从全部歌手列表返回统一搜索
  const handleBackToUnifiedSearch = useCallback(() => {
    setViewMode("unifiedSearch");
  }, []);

  // 点击歌手卡片进入歌手详情
  const handleArtistClick = useCallback((artist: Artist) => {
    previousViewRef.current = viewMode;
    const patched = { ...artist };
    useMusicStore.setState({ selectedArtist: patched });
    storeActions.loadArtistSongs(patched.mid || patched.id || "");
    setViewMode("artistDetail");
    // 歌手可能没有头像（从歌曲卡片进入时），异步搜索补齐
    if (!patched.pic_url && patched.name) {
      const cmd = playbackSource === "kugou" ? "kugou_artist_search" : "music_artist_search";
      invoke<Artist[]>(cmd, { keywords: patched.name, limit: 10 }).then((results) => {
        const match = results.find((a) => a.id === artist.id || a.name === artist.name);
        if (match?.pic_url) {
          useMusicStore.setState({ selectedArtist: { ...patched, pic_url: match.pic_url } });
        }
      }).catch(() => {});
    }
  }, [storeActions, viewMode, playbackSource]);

// ── 我的歌单点击：在左侧面板切换到曲目视图 ──
const handlePlaylistClick = useCallback((pl: Playlist) => {
storeActions.loadLeftPlaylistTracks(pl.id);
setLeftPanelView("tracks");
setRightPanelView("recommendations");
}, [storeActions]);

  const handleBackToPlaylists = useCallback(() => {
    setLeftPanelView("playlists");
  }, []);

// ── 推荐歌单点击：在右侧面板切换到曲目视图 ──
const handleRecPlaylistClick = useCallback((pl: Playlist) => {
storeActions.loadRightPlaylistTracks(pl.id);
setRightPanelView("tracks");
}, [storeActions]);

// ── 官方榜单点击：QQ 榜单走榜单歌曲接口，其他平台走歌单歌曲接口 ──
const handleChartClick = useCallback((pl: Playlist) => {
  useMusicStore.setState({ rightPlaylistMeta: pl });
  setRightPanelView("tracks");
  if (playbackSource === "qqmusic") {
    storeActions.loadRightRankTracks(pl.id);
  } else {
    storeActions.loadRightPlaylistTracks(pl.id);
  }
}, [playbackSource, storeActions]);

// 榜单平铺：酷狗/QQ 用网格铺满面板，网易云保持横向滚动
const chartIsGrid = playbackSource === "kugou" || playbackSource === "qqmusic";

  const handleBackToRecommendations = useCallback(() => {
    setRightPanelView("recommendations");
  }, []);

  // ── 每日推荐入口：切换右侧面板到每日推荐歌曲列表 ──
  const handleDailyRecommendClick = useCallback(() => {
    setRightPanelView("daily");
  }, []);

  // ── 收藏/取消收藏歌单 ──
  const handleTogglePlaylistSubscribe = useCallback((pl: Playlist) => {
    useMusicStore.getState().togglePlaylistSubscribe(pl.id, pl.subscribed);
  }, []);

  // ── 搜索结果中点击歌单：在搜索页内加载曲目 ──
  const handleSearchPlaylistClick = useCallback(async (pl: Playlist) => {
    setSearchExpandedPlaylist(pl);
    setSearchExpandedTracks([]);
    setSearchLoadingExpanded(true);
    try {
      const cmd = pl.provider === "kugou" ? "kugou_playlist_tracks" : "music_playlist_tracks";
      const result = await invoke<[Playlist, Song[]]>(cmd, { id: pl.id });
      setSearchExpandedTracks(result[1]);
    } catch {
      setSearchExpandedTracks([]);
    } finally {
      setSearchLoadingExpanded(false);
    }
  }, []);

  // 判断是否为用户自建歌单（在我的歌单里且 subscribed=false）
  const isOwnPlaylist = useCallback((pl: Playlist) => {
    return userPlaylists.some((p) => p.id === pl.id && !p.subscribed);
  }, [userPlaylists]);

  // ── 回调函数 ──
  const onPlay = useCallback((song: Song, queue: Song[]) => {
    useMusicStore.getState().playSong(song, queue);
  }, []);
  const onTogglePlay = useCallback(() => {
    useMusicStore.getState().togglePlay();
  }, []);
  const onToggleLike = useCallback((songId: string) => {
    useMusicStore.getState().toggleLike(songId);
  }, []);

  // ── 渲染歌曲行 ──
  const renderSongRow = useCallback((song: Song, index: number, queue: Song[]) => (
    <SongRow
      key={`${song.provider}-${song.id}-${index}`}
      song={song}
      index={index}
      queue={queue}
      isCurrent={currentSong?.id === song.id}
      isPlaying={isPlaying}
      isLiked={likedSongIds.has(song.id)}
      isLoggedIn={!!loginInfo?.logged_in}
      proxyPort={proxyPort}
      activeColor={activeColor}
      hoverBg={hoverBg}
      itemHoverBg={itemHoverBg}
      itemActiveBg={itemActiveBg}
      textColor={textColor}
      subTextColor={subTextColor}
      liquidGlassEnabled={liquidGlassEnabled}
      onPlay={onPlay}
      onTogglePlay={onTogglePlay}
      onToggleLike={onToggleLike}
      onArtistClick={handleArtistClick}
    />
  ), [currentSong, isPlaying, likedSongIds, loginInfo, proxyPort, activeColor, hoverBg, itemHoverBg, itemActiveBg, textColor, subTextColor, liquidGlassEnabled, onPlay, onTogglePlay, onToggleLike, handleArtistClick]);

  // VirtualList renderItem 回调（useCallback 稳定引用，避免 VirtualList memo 失效）
  const renderArtistSongItem = useCallback(
    (song: Song, i: number) => renderSongRow(song, i, artistSongs),
    [renderSongRow, artistSongs]
  );
  const renderLeftTrackItem = useCallback(
    (song: Song, i: number) => renderSongRow(song, i, leftPlaylistTracks),
    [renderSongRow, leftPlaylistTracks]
  );
  const renderRightTrackItem = useCallback(
    (song: Song, i: number) => renderSongRow(song, i, rightPlaylistTracks),
    [renderSongRow, rightPlaylistTracks]
  );
  const renderDailyTrackItem = useCallback(
    (song: Song, i: number) => renderSongRow(song, i, recommendSongs),
    [renderSongRow, recommendSongs]
  );

  // ── 渲染歌单行（可自定义 onClick）──
  const renderPlaylistRow = (pl: Playlist, prefix?: string, onClick?: (pl: Playlist) => void) => (
    <HStack
      key={`${prefix || ""}${pl.id}`}
      spacing={3}
      p={2}
      borderRadius="lg"
      cursor="pointer"
      _hover={{ bg: liquidGlassEnabled ? hoverBg : itemHoverBg }}
      onClick={() => (onClick || handlePlaylistClick)(pl)}
      transition="background 0.15s"
      bg={leftPlaylistMeta?.id === pl.id || rightPlaylistMeta?.id === pl.id ? itemActiveBg : "transparent"}
    >
      <ChakraImage
        src={coverProxyUrl(pl.cover, proxyPort)}
        alt=""
        w="44px"
        h="44px"
        borderRadius="md"
        objectFit="cover"
        fallback={<Box w="44px" h="44px" borderRadius="md" bg="gray.700" />}
      />
      <VStack spacing={0} align="start" flex={1} minW={0}>
        <Text color={textColor} fontSize="sm" fontWeight="medium" noOfLines={1}>
          {pl.name}
        </Text>
        <Text color={subTextColor} fontSize="xs">
          {pl.track_count} 首 {pl.creator ? `· ${pl.creator}` : ""}
        </Text>
      </VStack>
      {loginInfo?.logged_in && pl.provider === "netease" && !isOwnPlaylist(pl) && (
        <IconButton
          aria-label={pl.subscribed ? "取消收藏" : "收藏歌单"}
          icon={<Heart size={16} fill={pl.subscribed ? "#e53e3e" : "none"} />}
          size="sm"
          variant="ghost"
          title={pl.subscribed ? "取消收藏" : "收藏歌单"}
          onClick={(e) => {
            e.stopPropagation();
            handleTogglePlaylistSubscribe(pl);
          }}
          sx={{
            color: pl.subscribed ? "#e53e3e" : subTextColor,
            _hover: { bg: hoverBg },
            flexShrink: 0,
          }}
        />
      )}
    </HStack>
  );

  // ═══════════════════════════════════════════════
  // 歌手详情视图
  // ═══════════════════════════════════════════════
  if (viewMode === "artistDetail") {
    return (
      <VStack
        spacing={4}
        align="stretch"
        w="100%"
        h="calc(100vh - 120px)"
        overflow="hidden"
        sx={{ maxWidth: "100%", overflowX: "hidden", position: "relative" }}
      >
        <HStack spacing={3} flexShrink={0}>
          <Tooltip label="返回">
            <IconButton
              aria-label="Back"
              icon={<ArrowLeft size={18} />}
              size="sm"
              variant="ghost"
              onClick={handleBack}
              sx={{ color: activeColor, _hover: { bg: hoverBg } }}
            />
          </Tooltip>
          {selectedArtist && (
            <HStack spacing={3}>
              <ChakraImage
                src={coverProxyUrl(selectedArtist.pic_url || "", proxyPort)}
                alt=""
                w="56px"
                h="56px"
                borderRadius="full"
                objectFit="cover"
                fallback={<Box w="56px" h="56px" borderRadius="full" bg="gray.700" />}
              />
              <VStack spacing={0} align="start">
                <Text fontSize="xl" fontWeight="bold" color={textColor}>
                  {selectedArtist.name}
                </Text>
                <Text color={subTextColor} fontSize="sm">
                  {artistSongs.length} 首热门歌曲
                </Text>
              </VStack>
            </HStack>
          )}
          {!selectedArtist && (
            <Text fontSize="lg" fontWeight="bold" color={textColor}>
              歌手歌曲
            </Text>
          )}
        </HStack>

        <SearchBox
          onUnifiedSearch={handleUnifiedSearch}
          onArtistClick={handleArtistClick}
        />

        <LiquidGlassCard p={4} flex={1} display="flex" flexDirection="column" overflow="hidden">
          {loadingArtistSongs ? (
            <VStack py={12}>
              <Spinner size="lg" sx={{ color: activeColor }} />
              <Text color={subTextColor} fontSize="sm">加载中...</Text>
            </VStack>
          ) : artistSongs.length > 0 ? (
            <VirtualList
              items={artistSongs}
              itemHeight={60}
              renderItem={renderArtistSongItem}
              getKey={(song, i) => `${song.provider}-${song.id}-${i}`}
              emptyText="暂无歌曲"
              resetKey={selectedArtist?.id}
              scrollbarSx={memoScrollbarSx}
            />
          ) : (
            <VStack py={12} spacing={2}>
              <MusicIcon size={32} color={subTextColor} />
              <Text color={subTextColor} fontSize="sm">暂无歌曲</Text>
            </VStack>
          )}
        </LiquidGlassCard>

        <PlayerBar onExpand={handleExpandPlayer} hidden={expandedPlayer} />

        {/* 展开的播放器 */}
        {expandedPlayer && <ExpandedPlayer onClose={handleCloseExpandedPlayer} />}
      </VStack>
    );
  }

  // ═══════════════════════════════════════════════
  // 统一搜索结果视图：三标签切换 (单曲 / 歌单 / 歌手)
  // ═══════════════════════════════════════════════
  if (viewMode === "unifiedSearch") {
    const isLoading = searching || searchingArtists || searchingPlaylists;
    const hasAnyResults = searchResults.length > 0 || playlistSearchResults.length > 0 || artistSearchResults.length > 0;

    const tabBarBg = useColorModeValue("gray.100", "rgba(255,255,255,0.04)");
    const tabActiveBg = useColorModeValue("white", "rgba(255,255,255,0.1)");

    const tabs: { key: "songs" | "playlists" | "artists"; label: string; icon: React.ReactNode; count: number }[] = [
      { key: "songs", label: "单曲", icon: <MusicIcon size={14} />, count: searchResults.length },
      { key: "playlists", label: "歌单", icon: <ListMusic size={14} />, count: playlistSearchResults.length },
      { key: "artists", label: "歌手", icon: <User size={14} />, count: artistSearchResults.length },
    ];

    // 展开的歌单曲目视图
    if (searchExpandedPlaylist) {
      return (
        <VStack
          spacing={4}
          align="stretch"
          w="100%"
          h="calc(100vh - 120px)"
          overflow="hidden"
          sx={{ maxWidth: "100%", overflowX: "hidden", position: "relative" }}
        >
          <HStack spacing={3} flexShrink={0}>
            <Tooltip label="返回搜索结果">
              <IconButton
                aria-label="Back to search"
                icon={<ArrowLeft size={18} />}
                size="sm"
                variant="ghost"
                onClick={() => { setSearchExpandedPlaylist(null); setSearchExpandedTracks([]); }}
                sx={{ color: activeColor, _hover: { bg: hoverBg } }}
              />
            </Tooltip>
            <ChakraImage
              src={coverProxyUrl(searchExpandedPlaylist.cover, proxyPort)}
              alt=""
              w="40px"
              h="40px"
              borderRadius="md"
              objectFit="cover"
              fallback={<Box w="40px" h="40px" borderRadius="md" bg="gray.700" />}
            />
            <VStack spacing={0} align="start">
              <Text fontSize="md" fontWeight="bold" color={textColor} noOfLines={1}>
                {searchExpandedPlaylist.name}
              </Text>
              <Text color={subTextColor} fontSize="xs">
                {searchExpandedPlaylist.track_count} 首 {searchExpandedPlaylist.creator ? `· ${searchExpandedPlaylist.creator}` : ""}
              </Text>
            </VStack>
          </HStack>

          <SearchBox
            onUnifiedSearch={handleUnifiedSearch}
            onArtistClick={handleArtistClick}
          />

          <LiquidGlassCard p={4} flex={1} display="flex" flexDirection="column" overflow="hidden">
            {searchLoadingExpanded ? (
              <VStack py={12}>
                <Spinner size="lg" sx={{ color: activeColor }} />
                <Text color={subTextColor} fontSize="sm">加载曲目中...</Text>
              </VStack>
            ) : searchExpandedTracks.length > 0 ? (
              <Box flex={1} overflowY="scroll" sx={memoScrollbarSx}>
                <VStack spacing={1} align="stretch">
                  {searchExpandedTracks.map((song, i) => renderSongRow(song, i, searchExpandedTracks))}
                </VStack>
              </Box>
            ) : (
              <VStack py={12} spacing={2}>
                <MusicIcon size={32} color={subTextColor} />
                <Text color={subTextColor} fontSize="sm">暂无曲目</Text>
              </VStack>
            )}
          </LiquidGlassCard>

          <PlayerBar onExpand={handleExpandPlayer} hidden={expandedPlayer} />
          {expandedPlayer && <ExpandedPlayer onClose={handleCloseExpandedPlayer} />}
        </VStack>
      );
    }

    return (
      <VStack
        spacing={4}
        align="stretch"
        w="100%"
        h="calc(100vh - 120px)"
        overflow="hidden"
        sx={{ maxWidth: "100%", overflowX: "hidden", position: "relative" }}
      >
        <HStack spacing={3} flexShrink={0}>
          <Tooltip label="返回">
            <IconButton
              aria-label="Back"
              icon={<ArrowLeft size={18} />}
              size="sm"
              variant="ghost"
              onClick={handleBackToMain}
              sx={{ color: activeColor, _hover: { bg: hoverBg } }}
            />
          </Tooltip>
          <Text fontSize="lg" fontWeight="bold" color={textColor}>
            搜索 "{searchInput}"
          </Text>
        </HStack>

        <SearchBox
          onUnifiedSearch={handleUnifiedSearch}
          onArtistClick={handleArtistClick}
        />

        {/* 三标签导航栏 */}
        <HStack
          spacing={1}
          flexShrink={0}
          bg={liquidGlassEnabled ? "rgba(255,255,255,0.08)" : tabBarBg}
          p={1}
          borderRadius="xl"
          sx={
            liquidGlassEnabled
              ? {
                  backdropFilter: "blur(12px)",
                  WebkitBackdropFilter: "blur(12px)",
                  border: "1px solid rgba(255,255,255,0.1)",
                }
              : {}
          }
        >
          {tabs.map((tab) => (
            <Button
              key={tab.key}
              size="sm"
              variant="ghost"
              onClick={() => setSearchTab(tab.key)}
              borderRadius="lg"
              flex={1}
              sx={{
                bg: searchTab === tab.key
                  ? (liquidGlassEnabled ? "rgba(255,255,255,0.2)" : tabActiveBg)
                  : "transparent",
                color: searchTab === tab.key ? activeColor : subTextColor,
                fontWeight: searchTab === tab.key ? "bold" : "normal",
                boxShadow: searchTab === tab.key ? "sm" : "none",
                _hover: { bg: searchTab === tab.key ? undefined : hoverBg },
              }}
            >
              <HStack spacing={1.5}>
                {tab.icon}
                <Text fontSize="sm">{tab.label}</Text>
                {tab.count > 0 && (
                  <Text fontSize="xs" color={searchTab === tab.key ? activeColor : subTextColor}> ({tab.count})</Text>
                )}
              </HStack>
            </Button>
          ))}
        </HStack>

        <LiquidGlassCard p={4} flex={1} display="flex" flexDirection="column" overflow="hidden">
          {isLoading ? (
            <VStack py={12}>
              <Spinner size="lg" sx={{ color: activeColor }} />
              <Text color={subTextColor} fontSize="sm">搜索中...</Text>
            </VStack>
          ) : !hasAnyResults ? (
            <VStack py={12} spacing={2}>
              <Search size={32} color={subTextColor} />
              <Text color={subTextColor} fontSize="sm">没有找到相关内容</Text>
            </VStack>
          ) : (
            <Box flex={1} overflowY="auto" overflowX="hidden" sx={memoScrollbarSx}>
              <AnimatePresence mode="wait">
                <motion.div key={searchTab} variants={tabContentVariants} initial="hidden" animate="visible" exit="exit">
              {/* ── 单曲标签 ── */}
              {searchTab === "songs" && (
                searchResults.length > 0 ? (
                  <motion.div variants={listContainerVariants} initial="hidden" animate="visible" style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
                    {searchResults.map((song, i) => (
                      <motion.div key={`${song.provider}-${song.id}-${i}`} variants={listItemVariants}>
                        {renderSongRow(song, i, searchResults)}
                      </motion.div>
                    ))}
                  </motion.div>
                ) : (
                  <VStack py={12} spacing={2}>
                    <MusicIcon size={32} color={subTextColor} />
                    <Text color={subTextColor} fontSize="sm">未找到相关单曲</Text>
                  </VStack>
                )
              )}

              {/* ── 歌单标签 ── */}
              {searchTab === "playlists" && (
                playlistSearchResults.length > 0 ? (
                  <motion.div variants={listContainerVariants} initial="hidden" animate="visible" style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
                    {playlistSearchResults.map((pl) => (
                      <motion.div key={`search-pl-${pl.id}`} variants={listItemVariants}>
                        <HStack
                          spacing={3}
                          p={2}
                          borderRadius="lg"
                          cursor="pointer"
                          _hover={{ bg: liquidGlassEnabled ? hoverBg : itemHoverBg }}
                          onClick={() => handleSearchPlaylistClick(pl)}
                          transition="background 0.15s"
                        >
                          <ChakraImage
                            src={coverProxyUrl(pl.cover, proxyPort)}
                            alt=""
                            w="48px"
                            h="48px"
                            borderRadius="md"
                            objectFit="cover"
                            fallback={<Box w="48px" h="48px" borderRadius="md" bg="gray.700" />}
                          />
                          <VStack spacing={0} align="start" flex={1} minW={0}>
                            <Text color={textColor} fontSize="sm" fontWeight="medium" noOfLines={1}>
                              {pl.name}
                            </Text>
                            <Text color={subTextColor} fontSize="xs">
                              {pl.track_count} 首 {pl.creator ? `· ${pl.creator}` : ""}
                            </Text>
                          </VStack>
                          {loginInfo?.logged_in && pl.provider === "netease" && (
                            <IconButton
                              aria-label={pl.subscribed ? "取消收藏" : "收藏歌单"}
                              icon={<Heart size={14} fill={pl.subscribed ? "#e53e3e" : "none"} />}
                              size="xs"
                              variant="ghost"
                              title={pl.subscribed ? "取消收藏" : "收藏歌单"}
                              onClick={(e) => {
                                e.stopPropagation();
                                handleTogglePlaylistSubscribe(pl);
                              }}
                              sx={{
                                color: pl.subscribed ? "#e53e3e" : subTextColor,
                                _hover: { bg: hoverBg },
                                flexShrink: 0,
                              }}
                            />
                          )}
                          <Tooltip label="查看曲目">
                            <IconButton
                              aria-label="Play"
                              icon={<PlayBtn size={14} />}
                              size="xs"
                              variant="ghost"
                              sx={{ color: activeColor, _hover: { bg: hoverBg } }}
                              onClick={(e) => {
                                e.stopPropagation();
                                handleSearchPlaylistClick(pl);
                              }}
                            />
                          </Tooltip>
                        </HStack>
                      </motion.div>
                    ))}
                  </motion.div>
                ) : (
                  <VStack py={12} spacing={2}>
                    <ListMusic size={32} color={subTextColor} />
                    <Text color={subTextColor} fontSize="sm">未找到相关歌单</Text>
                  </VStack>
                )
              )}

              {/* ── 歌手标签 ── */}
              {searchTab === "artists" && (
                artistSearchResults.length > 0 ? (
                  <motion.div variants={listContainerVariants} initial="hidden" animate="visible" style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
                    <HStack spacing={3} flexWrap="wrap">
                      {artistSearchResults.map((artist) => (
                        <motion.div key={artist.id || artist.name} variants={listItemVariants} style={{ flex: "1 1 calc(50% - 6px)", minWidth: "140px", maxWidth: "calc(50% - 6px)" }}>
                          <Box
                            as="button"
                            p={3}
                            w="100%"
                            borderRadius="lg"
                            cursor="pointer"
                            onClick={() => handleArtistClick(artist)}
                            _hover={{ transform: "scale(1.02)" }}
                            transition="transform 0.15s"
                            bg={liquidGlassEnabled ? "rgba(255,255,255,0.08)" : itemHoverBg}
                            border="1px solid"
                            borderColor="transparent"
                            sx={
                              liquidGlassEnabled
                                ? { backdropFilter: "blur(8px)", WebkitBackdropFilter: "blur(8px)" }
                                : {}
                            }
                          >
                            <VStack spacing={2} align="center">
                              <Box w="60px" h="60px" borderRadius="full" overflow="hidden" flexShrink={0}>
                                <ChakraImage
                                  src={coverProxyUrl(artist.pic_url || "", proxyPort)}
                                  alt=""
                                  w="60px"
                                  h="60px"
                                  objectFit="cover"
                                  fallback={
                                    <Box w="60px" h="60px" bg="gray.700" display="flex" alignItems="center" justifyContent="center">
                                      <User size={28} color={subTextColor} />
                                    </Box>
                                  }
                                />
                              </Box>
                              <VStack spacing={0} w="100%">
                                <Text color={textColor} fontSize="sm" fontWeight="medium" noOfLines={1} textAlign="center">
                                  {artist.name}
                                </Text>
                                <Text color={subTextColor} fontSize="xs">
                                  {artist.music_size != null ? `${artist.music_size} 首` : ""}
                                </Text>
                              </VStack>
                            </VStack>
                          </Box>
                        </motion.div>
                      ))}
                    </HStack>
                    {artistSearchResults.length > 4 && (
                      <HStack justify="center" pt={2}>
                        <Button
                          size="xs"
                          variant="ghost"
                          onClick={handleShowAllArtists}
                          sx={{ color: activeColor, _hover: { bg: hoverBg } }}
                        >
                          查看全部 ({artistSearchResults.length}) →
                        </Button>
                      </HStack>
                    )}
                  </motion.div>
                ) : (
                  <VStack py={12} spacing={2}>
                    <User size={32} color={subTextColor} />
                    <Text color={subTextColor} fontSize="sm">未找到相关歌手</Text>
                  </VStack>
                )
              )}
                </motion.div>
              </AnimatePresence>
            </Box>
          )}
        </LiquidGlassCard>

        <PlayerBar onExpand={handleExpandPlayer} hidden={expandedPlayer} />

        {/* 展开的播放器 */}
        {expandedPlayer && <ExpandedPlayer onClose={handleCloseExpandedPlayer} />}
      </VStack>
    );
  }

  // ═══════════════════════════════════════════════
  // 全部歌手列表视图
  // ═══════════════════════════════════════════════
  if (viewMode === "fullArtistList") {
    return (
      <VStack
        spacing={4}
        align="stretch"
        w="100%"
        h="calc(100vh - 120px)"
        overflow="hidden"
        sx={{ maxWidth: "100%", overflowX: "hidden", position: "relative" }}
      >
        <HStack spacing={3} flexShrink={0}>
          <Tooltip label="返回">
            <IconButton
              aria-label="Back"
              icon={<ArrowLeft size={18} />}
              size="sm"
              variant="ghost"
              onClick={handleBackToUnifiedSearch}
              sx={{ color: activeColor, _hover: { bg: hoverBg } }}
            />
          </Tooltip>
          <Text fontSize="lg" fontWeight="bold" color={textColor}>
            全部歌手 - "{searchInput}"
          </Text>
          <Text color={subTextColor} fontSize="sm">
            ({artistSearchResults.length})
          </Text>
        </HStack>

        <SearchBox
          onUnifiedSearch={handleUnifiedSearch}
          onArtistClick={handleArtistClick}
        />

        <LiquidGlassCard p={4} flex={1} display="flex" flexDirection="column" overflow="hidden">
          <Box flex={1} overflowY="scroll" sx={memoScrollbarSx}>
            <VStack spacing={2} align="stretch">
              {artistSearchResults.map((artist) => (
                <HStack
                  key={artist.id || artist.name}
                  spacing={3}
                  p={3}
                  borderRadius="lg"
                  cursor="pointer"
                  _hover={{ bg: itemHoverBg }}
                  onClick={() => handleArtistClick(artist)}
                  transition="background 0.15s"
                >
                  <Box w="48px" h="48px" borderRadius="md" overflow="hidden" flexShrink={0}>
                    <ChakraImage
                      src={coverProxyUrl(artist.pic_url || "", proxyPort)}
                      alt=""
                      w="48px"
                      h="48px"
                      objectFit="cover"
                      fallback={
                        <Box
                          w="48px"
                          h="48px"
                          bg="gray.700"
                          display="flex"
                          alignItems="center"
                          justifyContent="center"
                        >
                          <User size={20} color={subTextColor} />
                        </Box>
                      }
                    />
                  </Box>
                  <VStack spacing={0} align="start" flex={1} minW={0}>
                    <Text color={textColor} fontSize="sm" fontWeight="medium" noOfLines={1}>
                      {artist.name}
                    </Text>
                    <Text color={subTextColor} fontSize="xs">
                      {artist.music_size != null ? `${artist.music_size} 首歌曲` : "点击查看热门歌曲"}
                    </Text>
                  </VStack>
                  <MicVocal size={16} color={subTextColor} />
                </HStack>
              ))}
            </VStack>
          </Box>
        </LiquidGlassCard>

        <PlayerBar onExpand={handleExpandPlayer} hidden={expandedPlayer} />

        {/* 展开的播放器 */}
        {expandedPlayer && <ExpandedPlayer onClose={handleCloseExpandedPlayer} />}
      </VStack>
    );
  }

  // ═══════════════════════════════════════════════
  // 主视图
  // ═══════════════════════════════════════════════
  return (
    <VStack
      spacing={4}
      align="stretch"
      w="100%"
      h="calc(100vh - 120px)"
      overflow="hidden"
      sx={{ maxWidth: "100%", overflowX: "hidden", position: "relative" }}
    >
      {/* 标题 + 登录 */}
      <HStack justify="space-between" w="100%" flexShrink={0}>
        <HStack spacing={3}>
          <MusicIcon size={24} color={activeColor} />
          <Heading size="md" color={textColor}>
            音乐播放器
          </Heading>
          <Box
            px={2.5}
            py={0.5}
            borderRadius="md"
            bg={useColorModeValue("gray.100", "rgba(255,255,255,0.08)")}
            border="1px solid"
            borderColor={useColorModeValue("gray.200", "rgba(255,255,255,0.12)")}
          >
            <Text fontSize="xs" color={subTextColor} fontWeight="medium" whiteSpace="nowrap">
              当前平台：{providerName}
            </Text>
          </Box>
        </HStack>
        <MusicLoginSection />
      </HStack>

      {/* 主内容区：左右 50/50 */}
      <HStack spacing={4} align="stretch" flex={1} w="100%" minH={0} overflow="hidden">
        {/* ══ 左侧：我的歌单 / 歌单曲目 ══ */}
        {loginInfo?.logged_in ? (
          <LiquidGlassCard p={4} flex={1} display="flex" flexDirection="column" overflow="hidden" minW={0}>
            {leftPanelView === "tracks" ? (
              <>
                {/* 歌单曲目视图 */}
                <HStack spacing={2} mb={3} flexShrink={0}>
                  <Tooltip label="返回歌单列表">
                    <IconButton
                      aria-label="Back"
                      icon={<ArrowLeft size={16} />}
                      size="sm"
                      variant="ghost"
                      onClick={handleBackToPlaylists}
                      sx={{ color: activeColor, _hover: { bg: hoverBg } }}
                    />
                  </Tooltip>
                  <Text fontSize="sm" fontWeight="bold" color={textColor} noOfLines={1}>
                    {leftPlaylistMeta?.name || "曲目列表"}
                  </Text>
                  <Text color={subTextColor} fontSize="xs" flexShrink={0}>
                    ({leftPlaylistTracks.length} 首)
                  </Text>
                </HStack>
                <VirtualList
                  items={leftPlaylistTracks}
                  itemHeight={60}
                  renderItem={renderLeftTrackItem}
                  getKey={(song, i) => `${song.provider}-${song.id}-${i}`}
                  loading={loadingLeftTracks}
                  loadingText="加载曲目中..."
                  emptyText="暂无曲目"
                  resetKey={leftPlaylistMeta?.id}
                  scrollbarSx={memoScrollbarSx}
                  onEndReached={() => storeActions.loadMoreLeftPlaylistTracks()}
                  hasMore={(leftPlaylistMeta?.track_count ?? 0) > leftPlaylistTracks.length}
                />
              </>
            ) : (
              <>
                {/* 歌单列表视图 */}
                <Text fontSize="sm" fontWeight="bold" color={textColor} mb={3} flexShrink={0}>
                  我的歌单
                </Text>
                <Box flex={1} overflowY="auto" sx={memoScrollbarSx}>
                  {loadingPlaylists ? (
                    <VStack py={6}><Spinner size="sm" sx={{ color: activeColor }} /></VStack>
                  ) : userPlaylists.length > 0 ? (
                    <motion.div variants={listContainerVariants} initial="hidden" animate="visible" style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
                      {userPlaylists.map((pl) => (
                        <motion.div key={pl.id} variants={listItemVariants}>{renderPlaylistRow(pl)}</motion.div>
                      ))}
                    </motion.div>
                  ) : (
                    <VStack py={4} spacing={2}>
                      <Text color={subTextColor} fontSize="xs" textAlign="center">
                        {userPlaylistsError ? "歌单获取失败" : "暂无歌单"}
                      </Text>
                      {userPlaylistsError ? (
                        <>
                          <Text color={subTextColor} fontSize="2xs" textAlign="center" wordBreak="break-all" px={2}>
                            {userPlaylistsError}
                          </Text>
                          <Button
                            size="xs"
                            variant="ghost"
                            color={activeColor}
                            onClick={() => useMusicStore.getState().openLoginWindow(playbackSource)}
                            alignSelf="center"
                          >
                            重新登录
                          </Button>
                        </>
                      ) : null}
                    </VStack>
                  )}
                </Box>
              </>
            )}
          </LiquidGlassCard>
        ) : (
          <LiquidGlassCard p={4} flex={1} display="flex" flexDirection="column" alignItems="center" justifyContent="center" overflow="hidden">
            <VStack spacing={3}>
              <MusicIcon size={32} color={subTextColor} />
              <Text color={subTextColor} fontSize="sm" textAlign="center">登录后查看歌单</Text>
            </VStack>
          </LiquidGlassCard>
        )}

        {/* ══ 右侧：搜索 + 推荐 ══ */}
        <VStack spacing={4} align="stretch" flex={1} minW={0} overflow="hidden">
          {/* 搜索框 — 独立 memo 组件，播放时不会因重渲染而失焦 */}
          <SearchBox
            onUnifiedSearch={handleUnifiedSearch}
            onArtistClick={handleArtistClick}
          />

          {/* 推荐歌单 / 推荐歌单曲目 */}
          <LiquidGlassCard p={4} flex={1} display="flex" flexDirection="column" overflow="hidden">
            {rightPanelView === "daily" ? (
              <>
                {/* 每日推荐曲目视图 */}
                <HStack spacing={2} mb={3} flexShrink={0}>
                  <Tooltip label="返回推荐歌单">
                    <IconButton
                      aria-label="Back"
                      icon={<ArrowLeft size={16} />}
                      size="sm"
                      variant="ghost"
                      onClick={handleBackToRecommendations}
                      sx={{ color: activeColor, _hover: { bg: hoverBg } }}
                    />
                  </Tooltip>
                  <Text fontSize="sm" fontWeight="bold" color={textColor} noOfLines={1}>
                    每日推荐
                  </Text>
                  <Text color={subTextColor} fontSize="xs" flexShrink={0}>
                    ({recommendSongs.length} 首)
                  </Text>
                </HStack>
                <VirtualList
                  items={recommendSongs}
                  itemHeight={60}
                  renderItem={renderDailyTrackItem}
                  getKey={(song, i) => `${song.provider}-${song.id}-${i}`}
                  loading={false}
                  loadingText="加载推荐中..."
                  emptyText="每日推荐无法获取（请确认已登录网易云）"
                  resetKey="daily"
                  scrollbarSx={memoScrollbarSx}
                  hasMore={false}
                />
              </>
            ) : rightPanelView === "tracks" ? (
              <>
                {/* 推荐歌单曲目视图 */}
                <HStack spacing={2} mb={3} flexShrink={0}>
                  <Tooltip label="返回推荐歌单">
                    <IconButton
                      aria-label="Back"
                      icon={<ArrowLeft size={16} />}
                      size="sm"
                      variant="ghost"
                      onClick={handleBackToRecommendations}
                      sx={{ color: activeColor, _hover: { bg: hoverBg } }}
                    />
                  </Tooltip>
                  <Text fontSize="sm" fontWeight="bold" color={textColor} noOfLines={1}>
                    {rightPlaylistMeta?.name || "曲目列表"}
                  </Text>
                  <Text color={subTextColor} fontSize="xs" flexShrink={0}>
                    ({rightPlaylistTracks.length} 首)
                  </Text>
                </HStack>
                <VirtualList
                  items={rightPlaylistTracks}
                  itemHeight={60}
                  renderItem={renderRightTrackItem}
                  getKey={(song, i) => `${song.provider}-${song.id}-${i}`}
                  loading={loadingRightTracks}
                  loadingText="加载曲目中..."
                  emptyText="暂无曲目"
                  resetKey={rightPlaylistMeta?.id}
                  scrollbarSx={memoScrollbarSx}
                  onEndReached={() => storeActions.loadMoreRightPlaylistTracks()}
                  hasMore={(rightPlaylistMeta?.track_count ?? 0) > rightPlaylistTracks.length}
                />
              </>
            ) : (
              <>
                {loginInfo?.logged_in && playbackSource !== "kugou" && playbackSource !== "qqmusic" && (
                  <>
                <HStack justify="space-between" mb={3} flexShrink={0}>
                  <HStack spacing={2}>
                    <Sparkles size={16} color={activeColor} />
                    <Text fontSize="sm" fontWeight="bold" color={textColor}>
                      推荐歌单
                    </Text>
                  </HStack>
                  <Button
                    size="xs"
                    variant="ghost"
                    onClick={() => storeActions.loadRecommendations()}
                    sx={{ color: activeColor, _hover: { bg: hoverBg } }}
                  >
                    刷新
                  </Button>
                </HStack>
                  </>
                )}

                {/* 官方榜单 */}
                {loginInfo?.logged_in && (
                <VStack spacing={2} align="stretch" mb={2} flexShrink={chartIsGrid ? undefined : 0} flex={chartIsGrid ? 1 : undefined} overflowY={chartIsGrid ? "auto" : undefined} sx={chartIsGrid ? memoScrollbarSx : undefined}>
                  <HStack spacing={1.5}>
                    <TrendingUp size={13} color={activeColor} />
                    <Text fontSize="2xs" fontWeight="bold" color={subTextColor}>官方榜单</Text>
                  </HStack>
                  {chartIsGrid ? (
                    /* 酷狗/QQ: 网格布局铺满面板 */
                    <Box>
                      <SimpleGrid columns={3} spacing={2}>
                        {officialCharts.length > 0 ? officialCharts.map((chart) => (
                          <VStack
                            key={chart.id}
                            spacing={0.5}
                            cursor="pointer"
                            onClick={() => playbackSource === "qqmusic" ? handleChartClick(chart) : handleRecPlaylistClick(chart)}
                            _hover={{ transform: "scale(1.04)" }}
                            transition="transform 0.15s"
                          >
                            <Box
                              w="100%"
                              borderRadius="lg"
                              overflow="hidden"
                              sx={{ aspectRatio: "1 / 1" }}
                            >
                              <ChakraImage
                                src={coverProxyUrl(chart.cover, proxyPort)}
                                alt=""
                                w="100%"
                                h="100%"
                                objectFit="cover"
                                fallback={
                                  <Box w="100%" h="100%" bg="gray.700" display="flex" alignItems="center" justifyContent="center">
                                    <TrendingUp size={14} color={subTextColor} />
                                  </Box>
                                }
                              />
                            </Box>
                            <Text color={textColor} fontSize="xs" fontWeight="medium" noOfLines={1} textAlign="center" w="100%">
                              {chart.name}
                            </Text>
                          </VStack>
                        )) : (
                          <>
                            {[0, 1, 2, 3, 4, 5].map((i) => (
                              <Box key={i} borderRadius="lg" bg={itemHoverBg} sx={{ aspectRatio: "1 / 1" }} />
                            ))}
                          </>
                        )}
                      </SimpleGrid>
                    </Box>
                  ) : (
                    /* 网易云: 横向滚动 */
                    <HStack spacing={1.5} minW={0} overflowX="auto" sx={memoScrollbarSx}
                      onWheel={(e) => {
                        e.currentTarget.scrollLeft += e.deltaY;
                      }}
                    >
                    {/* 每日推荐入口（网易云） */}
                    {playbackSource === "netease" && (
                      <VStack
                        spacing={0.5}
                        cursor="pointer"
                        minW="60px"
                        maxW="70px"
                        flexShrink={0}
                        onClick={handleDailyRecommendClick}
                        _hover={{ transform: "scale(1.04)" }}
                        transition="transform 0.15s"
                      >
                        <Box
                          w="100%"
                          borderRadius="lg"
                          overflow="hidden"
                          sx={{ aspectRatio: "1 / 1" }}
                        >
                          <Box
                            w="100%"
                            h="100%"
                            bgGradient="linear(135deg, #f6b26b 0%, #e06666 100%)"
                            display="flex"
                            alignItems="center"
                            justifyContent="center"
                          >
                            <Sparkles size={16} color="#fff" />
                          </Box>
                        </Box>
                        <Text color={textColor} fontSize="xs" fontWeight="medium" noOfLines={1} textAlign="center">
                          每日推荐
                        </Text>
                      </VStack>
                    )}
                    {officialCharts.length > 0 ? officialCharts.map((chart) => (
                      <VStack
                        key={chart.id}
                        spacing={0.5}
                        cursor="pointer"
                        minW="60px"
                        maxW="70px"
                        flexShrink={0}
                        onClick={() => handleChartClick(chart)}
                        _hover={{ transform: "scale(1.04)" }}
                        transition="transform 0.15s"
                      >
                        <Box
                          w="100%"
                          borderRadius="lg"
                          overflow="hidden"
                          sx={{ aspectRatio: "1 / 1" }}
                        >
                          <ChakraImage
                            src={coverProxyUrl(chart.cover, proxyPort)}
                            alt=""
                            w="100%"
                            h="100%"
                            objectFit="cover"
                            fallback={
                              <Box w="100%" h="100%" bg="gray.700" display="flex" alignItems="center" justifyContent="center">
                                <TrendingUp size={14} color={subTextColor} />
                              </Box>
                            }
                          />
                        </Box>
                        <Text color={textColor} fontSize="xs" fontWeight="medium" noOfLines={1} textAlign="center">
                          {chart.name}
                        </Text>
                      </VStack>
                    )) : (
                      <>
                        {[0, 1, 2, 3, 4, 5].map((i) => (
                          <Box key={i} minW="60px" maxW="70px" flexShrink={0} borderRadius="lg" bg={itemHoverBg} sx={{ aspectRatio: "1 / 1" }} />
                        ))}
                      </>
                    )}
                    </HStack>
                  )}
                </VStack>
                )}

                {playbackSource !== "kugou" && playbackSource !== "qqmusic" && (
                <Box flex={1} overflowY="auto" sx={memoScrollbarSx}>
                  {!loginInfo?.logged_in ? (
                    <VStack py={8} spacing={3}>
                      <MusicIcon size={32} color={subTextColor} />
                      <Text color={subTextColor} fontSize="sm" textAlign="center">登录后查看推荐内容</Text>
                    </VStack>
                  ) : dailyRecommendPlaylists.length === 0 ? (
                    <VStack py={8} spacing={3}>
                      <Sparkles size={32} color={subTextColor} />
                      <Text color={subTextColor} fontSize="sm" textAlign="center">点击刷新加载推荐</Text>
                    </VStack>
                  ) : (
                    <motion.div variants={listContainerVariants} initial="hidden" animate="visible" style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
                      {dailyRecommendPlaylists.map((pl) => (
                        <motion.div key={`rec-${pl.id}`} variants={listItemVariants}>{renderPlaylistRow(pl, "rec-", handleRecPlaylistClick)}</motion.div>
                      ))}
                    </motion.div>
                  )}
                </Box>
                )}
              </>
            )}
          </LiquidGlassCard>
        </VStack>
      </HStack>

      {/* 底部播放器 */}
      <PlayerBar onExpand={handleExpandPlayer} hidden={expandedPlayer} />

      {/* 展开的播放器 */}
      {expandedPlayer && <ExpandedPlayer onClose={handleCloseExpandedPlayer} />}
    </VStack>
  );
}
