import { useEffect, useRef, useState, useCallback, memo } from "react";
import { useShallow } from "zustand/react/shallow";
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
  IconButton,
  Tooltip,
  Slider,
  SliderTrack,
  SliderFilledTrack,
  SliderThumb,
  Menu,
  MenuButton,
  MenuList,
  MenuItem,
  Image as ChakraImage,
  Heading,
} from "@chakra-ui/react";
import {
  Search,
  Play,
  Pause,
  SkipBack,
  SkipForward,
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
} from "lucide-react";
import { useMusicStore, coverProxyUrl } from "@/stores/music-store";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import type { Song, Playlist } from "@/types/music";
import { MusicLoginSection } from "@/components/MusicLoginSection";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";

const scrollbarSx = (color: string) => ({
  "&::-webkit-scrollbar": { width: "4px" },
  "&::-webkit-scrollbar-thumb": { background: color, borderRadius: "2px" },
  "&::-webkit-scrollbar-track": { background: "transparent" },
});

// ═══════════════════════════════════════════════
// PlayerBar — 独立组件，订阅 currentTime/duration
// 避免播放时主页面频繁重渲染
// ═══════════════════════════════════════════════
const PlayerBar = memo(function PlayerBar() {
  // 只订阅播放器需要的字段，避免搜索等状态变化导致不必要的重渲染
  const { currentSong, isPlaying, currentTime, duration, volume, playMode, playQueue, currentIndex, proxyPort } = useMusicStore(
    useShallow((s) => ({
      currentSong: s.currentSong,
      isPlaying: s.isPlaying,
      currentTime: s.currentTime,
      duration: s.duration,
      volume: s.volume,
      playMode: s.playMode,
      playQueue: s.playQueue,
      currentIndex: s.currentIndex,
      proxyPort: s.proxyPort,
    }))
  );
  const { getActiveColor, getHoverColor, getContrastTextColor, getBorderColor } = useThemeColor();

  const activeColor = getActiveColor();
  const contrastText = getContrastTextColor();
  const hoverBg = getHoverColor(false);

  const borderColor = useColorModeValue("gray.200", "#333333");
  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const dropdownBg = useColorModeValue("white", "#1a1a1a");

  const ModeIcon = playMode === "one" ? Repeat1 : playMode === "shuffle" ? Shuffle : Repeat;

  const formatTime = (time: number): string => {
    if (isNaN(time)) return "0:00";
    const m = Math.floor(time / 60);
    const s = Math.floor(time % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  };

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
    <LiquidGlassCard
      p={3}
      flexShrink={0}
      sx={{ marginTop: "auto", position: "relative" }}
    >
      <VStack spacing={2} align="stretch">
        <HStack spacing={4} align="center">
          {/* 左侧：封面 + 标题 */}
          <ChakraImage
            src={coverProxyUrl(currentSong.cover, proxyPort)}
            alt=""
            w="48px"
            h="48px"
            borderRadius="md"
            objectFit="cover"
            fallback={<Box w="48px" h="48px" borderRadius="md" bg="gray.700" />}
          />
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
              <IconButton aria-label="Prev" icon={<SkipBack size={18} />} size="sm" variant="ghost" onClick={() => useMusicStore.getState().prevTrack()} />
            </Tooltip>
            <Tooltip label={isPlaying ? "暂停" : "播放"}>
              <IconButton
                aria-label="Play/Pause"
                icon={isPlaying ? <Pause size={20} /> : <Play size={20} />}
                size="sm"
                sx={{
                  bg: activeColor,
                  color: contrastText,
                  _hover: { bg: activeColor, filter: "brightness(0.9)" },
                  borderRadius: "md",
                }}
                onClick={() => useMusicStore.getState().togglePlay()}
              />
            </Tooltip>
            <Tooltip label="下一首">
              <IconButton aria-label="Next" icon={<SkipForward size={18} />} size="sm" variant="ghost" onClick={() => useMusicStore.getState().nextTrack()} />
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

          {/* 右侧：音量 + 播放队列 */}
          <HStack spacing={2} w="120px" ml="auto">
            <Tooltip label="静音">
              <IconButton
                aria-label="Mute"
                icon={volume === 0 ? <VolumeX size={16} /> : <Volume2 size={16} />}
                size="sm"
                variant="ghost"
                onClick={() => useMusicStore.getState().setVolume(volume === 0 ? 0.7 : 0)}
              />
            </Tooltip>
            <Slider value={volume} onChange={(v) => useMusicStore.getState().setVolume(v)} min={0} max={1} step={0.01} size="sm">
              <SliderTrack bg={borderColor} h="4px" borderRadius="full">
                <SliderFilledTrack bg={activeColor} borderRadius="full" />
              </SliderTrack>
              <SliderThumb w="10px" h="10px" bg={activeColor} />
            </Slider>
          </HStack>

          <Menu>
            <Tooltip label="播放队列">
              <MenuButton as={IconButton} aria-label="Queue" icon={<ListMusic size={18} />} size="sm" variant="ghost" />
            </Tooltip>
            <MenuList maxH="300px" overflowY="auto" minW="240px" bg={dropdownBg} borderColor={borderColor}>
              {playQueue.map((s, i) => (
                <MenuItem
                  key={`${s.id}-${i}`}
                  onClick={() => useMusicStore.getState().playSong(s, playQueue)}
                  bg={i === currentIndex ? `${activeColor}33` : undefined}
                >
                  <Text noOfLines={1} fontSize="sm" color={i === currentIndex ? activeColor : textColor}>
                    {i + 1}. {s.name} - {s.artist}
                  </Text>
                </MenuItem>
              ))}
            </MenuList>
          </Menu>
        </HStack>

        {/* 进度条 */}
        <HStack spacing={2}>
          <Text color={subTextColor} fontSize="xs" w="40px" textAlign="center">
            {formatTime(currentTime)}
          </Text>
          <Slider
            value={currentTime}
            onChange={(v) => useMusicStore.getState().seekTo(v)}
            min={0}
            max={duration || 100}
            step={0.1}
            size="sm"
            flex={1}
          >
            <SliderTrack bg={borderColor} h="4px" borderRadius="full">
              <SliderFilledTrack bg={activeColor} borderRadius="full" />
            </SliderTrack>
            <SliderThumb w="10px" h="10px" bg={activeColor} />
          </Slider>
          <Text color={subTextColor} fontSize="xs" w="40px" textAlign="center">
            {formatTime(duration)}
          </Text>
        </HStack>
      </VStack>
    </LiquidGlassCard>
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
}: SongRowProps) {
  const formatTime = (time: number): string => {
    if (isNaN(time)) return "0:00";
    const m = Math.floor(time / 60);
    const s = Math.floor(time % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  };

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
        <Text color={subTextColor} fontSize="xs" noOfLines={1}>
          {song.artist} {song.album ? `- ${song.album}` : ""}
        </Text>
      </VStack>
      <Text color={subTextColor} fontSize="xs" flexShrink={0}>
        {formatTime(song.duration / 1000)}
      </Text>
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
          icon={isCurrent && isPlaying ? <Pause size={14} /> : <Play size={14} />}
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
// Main MusicPage
// ═══════════════════════════════════════════════
export default function MusicPage() {
  // ── 用 useShallow 精确订阅，排除 currentTime/duration ──
  // 这样播放时 timeupdate 不会导致整个页面重渲染
  const {
    currentSong,
    isPlaying,
    searchResults,
    userPlaylists,
    currentPlaylistTracks,
    currentPlaylistMeta,
    likedSongIds,
    recommendations,
    loginInfo,
    searching,
    loadingPlaylists,
    loadingTracks,
    proxyPort,
  } = useMusicStore(
    useShallow((s) => ({
      currentSong: s.currentSong,
      isPlaying: s.isPlaying,
      searchResults: s.searchResults,
      userPlaylists: s.userPlaylists,
      currentPlaylistTracks: s.currentPlaylistTracks,
      currentPlaylistMeta: s.currentPlaylistMeta,
      likedSongIds: s.likedSongIds,
      recommendations: s.recommendations,
      loginInfo: s.loginInfo,
      searching: s.searching,
      loadingPlaylists: s.loadingPlaylists,
      loadingTracks: s.loadingTracks,
      proxyPort: s.proxyPort,
    }))
  );

  // actions 是稳定的，用 useRef 只获取一次，避免每次渲染重新创建导致 useCallback 失效
  const storeActionsRef = useRef(useMusicStore.getState());
  const storeActions = storeActionsRef.current;

  const audioRef = useRef<HTMLAudioElement | null>(null);
  const [searchInput, setSearchInput] = useState("");
  const [searchMode, setSearchMode] = useState(false);
  const [showSearchDropdown, setShowSearchDropdown] = useState(false);
  const [leftPanelView, setLeftPanelView] = useState<"playlists" | "tracks">("playlists");
  const [dropdownResults, setDropdownResults] = useState<Song[]>([]);
  const searchBoxRef = useRef<HTMLDivElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

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

  useEffect(() => {
    const audio = document.createElement("audio");
    audio.style.display = "none";
    document.body.appendChild(audio);
    audioRef.current = audio;
    storeActions.setAudioRef(audio);

    audio.addEventListener("timeupdate", () => {
      useMusicStore.getState().setCurrentTime(audio.currentTime);
    });
    audio.addEventListener("loadedmetadata", () => {
      useMusicStore.getState().setDuration(audio.duration);
    });
    audio.addEventListener("ended", () => {
      useMusicStore.getState().nextTrack();
    });

    storeActions.init();

    return () => {
      audio.pause();
      audio.src = "";
      audio.remove();
      storeActions.setAudioRef(null);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 自动加载推荐歌单（登录后且无推荐数据时）
  useEffect(() => {
    if (loginInfo?.logged_in && recommendations.length === 0) {
      storeActions.loadRecommendations();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loginInfo?.logged_in]);

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

  // ── 搜索：边输入边出预览（debounce 300ms）──
  const handleInputChange = useCallback((value: string) => {
    setSearchInput(value);
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

  // ── 回车：进入全屏搜索结果 ──
  const handleSearchEnter = useCallback(() => {
    if (!searchInput.trim()) return;
    if (debounceRef.current) clearTimeout(debounceRef.current);
    storeActions.search(searchInput).then(() => {
      setSearchMode(true);
      setShowSearchDropdown(false);
    });
  }, [searchInput, storeActions]);

  // ── 搜索按钮：进入全屏搜索结果 ──
  const handleSearchButtonClick = useCallback(() => {
    if (!searchInput.trim()) return;
    if (debounceRef.current) clearTimeout(debounceRef.current);
    storeActions.search(searchInput).then(() => {
      setSearchMode(true);
      setShowSearchDropdown(false);
    });
  }, [searchInput, storeActions]);

  const handleBack = useCallback(() => {
    setSearchMode(false);
  }, []);

  // ── 歌单点击：切换到曲目视图 + 重新加载红心列表 ──
  const handlePlaylistClick = useCallback((pl: Playlist) => {
    storeActions.loadPlaylistTracks(pl.id);
    storeActions.loadLikedList();
    setLeftPanelView("tracks");
  }, [storeActions]);

  const handleBackToPlaylists = useCallback(() => {
    setLeftPanelView("playlists");
  }, []);

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
  const renderSongRow = (song: Song, index: number, queue: Song[]) => (
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
    />
  );

  // ── 渲染歌单行 ──
  const renderPlaylistRow = (pl: Playlist, prefix?: string) => (
    <HStack
      key={`${prefix || ""}${pl.id}`}
      spacing={3}
      p={2}
      borderRadius="lg"
      cursor="pointer"
      _hover={{ bg: liquidGlassEnabled ? hoverBg : itemHoverBg }}
      onClick={() => handlePlaylistClick(pl)}
      transition="background 0.15s"
      bg={currentPlaylistMeta?.id === pl.id ? itemActiveBg : "transparent"}
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
    </HStack>
  );

  // ═══════════════════════════════════════════════
  // 搜索结果全屏视图
  // ═══════════════════════════════════════════════
  if (searchMode) {
    return (
      <VStack
        spacing={4}
        align="stretch"
        w="100%"
        h="calc(100vh - 120px)"
        overflow="hidden"
        sx={{ maxWidth: "100%", overflowX: "hidden" }}
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
          <Text fontSize="lg" fontWeight="bold" color={textColor}>
            搜索结果 "{searchInput}"
          </Text>
          <Text color={subTextColor} fontSize="sm">
            ({searchResults.length})
          </Text>
        </HStack>

        <LiquidGlassCard p={4} flex={1} display="flex" flexDirection="column" overflow="hidden">
          {searching ? (
            <VStack py={12}>
              <Spinner size="lg" sx={{ color: activeColor }} />
              <Text color={subTextColor} fontSize="sm">搜索中...</Text>
            </VStack>
          ) : searchResults.length > 0 ? (
            <VStack
              spacing={1}
              align="stretch"
              overflowY="auto"
              flex={1}
              sx={scrollbarSx(activeColor)}
            >
              {searchResults.map((song, i) => renderSongRow(song, i, searchResults))}
            </VStack>
          ) : (
            <VStack py={12} spacing={2}>
              <MusicIcon size={32} color={subTextColor} />
              <Text color={subTextColor} fontSize="sm">没有找到相关音乐</Text>
            </VStack>
          )}
        </LiquidGlassCard>

        <PlayerBar />
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
      sx={{ maxWidth: "100%", overflowX: "hidden" }}
    >
      {/* 标题 + 登录 */}
      <HStack justify="space-between" w="100%" flexShrink={0}>
        <HStack spacing={3}>
          <MusicIcon size={24} color={activeColor} />
          <Heading size="md" color={textColor}>
            音乐播放器
          </Heading>
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
                    {currentPlaylistMeta?.name || "曲目列表"}
                  </Text>
                  <Text color={subTextColor} fontSize="xs" flexShrink={0}>
                    ({currentPlaylistTracks.length} 首)
                  </Text>
                </HStack>
                <Box flex={1} overflowY="auto" sx={scrollbarSx(activeColor)}>
                  {loadingTracks ? (
                    <VStack py={6}><Spinner size="sm" sx={{ color: activeColor }} /></VStack>
                  ) : currentPlaylistTracks.length > 0 ? (
                    <VStack spacing={1} align="stretch">
                      {currentPlaylistTracks.map((song, i) => renderSongRow(song, i, currentPlaylistTracks))}
                    </VStack>
                  ) : (
                    <Text color={subTextColor} fontSize="xs" py={4} textAlign="center">暂无曲目</Text>
                  )}
                </Box>
              </>
            ) : (
              <>
                {/* 歌单列表视图 */}
                <Text fontSize="sm" fontWeight="bold" color={textColor} mb={3} flexShrink={0}>
                  我的歌单
                </Text>
                <Box flex={1} overflowY="auto" sx={scrollbarSx(activeColor)}>
                  {loadingPlaylists ? (
                    <VStack py={6}><Spinner size="sm" sx={{ color: activeColor }} /></VStack>
                  ) : userPlaylists.length > 0 ? (
                    <VStack spacing={1} align="stretch">
                      {userPlaylists.map((pl) => renderPlaylistRow(pl))}
                    </VStack>
                  ) : (
                    <Text color={subTextColor} fontSize="xs" py={4} textAlign="center">暂无歌单</Text>
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
          {/* 搜索框 */}
          <Box ref={searchBoxRef} position="relative" flexShrink={0}>
            <HStack spacing={2}>
              <InputGroup size="md">
                <InputLeftElement pointerEvents="none">
                  <Search size={16} color={subTextColor} />
                </InputLeftElement>
                <Input
                  placeholder="搜索歌曲、歌手... (回车查看全部)"
                  value={searchInput}
                  onChange={(e) => handleInputChange(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && handleSearchEnter()}
                  bg={liquidGlassEnabled ? glassInputBg : bgColor}
                  borderColor={liquidGlassEnabled ? themeBorder : borderColor}
                  borderRadius="xl"
                  _focus={{ borderColor: activeColor, boxShadow: `0 0 0 1px ${activeColor}` }}
                />
              </InputGroup>
              <Button
                leftIcon={<Search size={16} />}
                onClick={handleSearchButtonClick}
                isLoading={searching}
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

            {/* 搜索下拉预览 — 不使用液态玻璃 */}
            {showSearchDropdown && dropdownResults.length > 0 && (
              <Box
                p={2}
                position="absolute"
                top="50px"
                left={0}
                right={0}
                zIndex={30}
                maxH="320px"
                overflowY="auto"
                bg={dropdownBg}
                border="1px solid"
                borderColor={dropdownBorder}
                borderRadius="lg"
                boxShadow="lg"
                sx={scrollbarSx(activeColor)}
              >
                <VStack spacing={1} align="stretch">
                  {dropdownResults.slice(0, 6).map((song, i) => renderSongRow(song, i, dropdownResults))}
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => {
                      setSearchMode(true);
                      setShowSearchDropdown(false);
                    }}
                    sx={{ color: activeColor, _hover: { bg: hoverBg } }}
                  >
                    查看全部 ({dropdownResults.length})
                  </Button>
                </VStack>
              </Box>
            )}
          </Box>

          {/* 推荐歌单 */}
          <LiquidGlassCard p={4} flex={1} display="flex" flexDirection="column" overflow="hidden">
            <HStack justify="space-between" mb={3} flexShrink={0}>
              <HStack spacing={2}>
                <Sparkles size={16} color={activeColor} />
                <Text fontSize="sm" fontWeight="bold" color={textColor}>
                  推荐歌单
                </Text>
              </HStack>
              {loginInfo?.logged_in && (
                <Button
                  size="xs"
                  variant="ghost"
                  onClick={() => storeActions.loadRecommendations()}
                  sx={{ color: activeColor, _hover: { bg: hoverBg } }}
                >
                  刷新
                </Button>
              )}
            </HStack>

            <Box flex={1} overflowY="auto" sx={scrollbarSx(activeColor)}>
              {!loginInfo?.logged_in ? (
                <VStack py={8} spacing={3}>
                  <MusicIcon size={32} color={subTextColor} />
                  <Text color={subTextColor} fontSize="sm" textAlign="center">登录后查看推荐内容</Text>
                </VStack>
              ) : recommendations.length === 0 ? (
                <VStack py={8} spacing={3}>
                  <Sparkles size={32} color={subTextColor} />
                  <Text color={subTextColor} fontSize="sm" textAlign="center">点击刷新加载推荐</Text>
                </VStack>
              ) : (
                <VStack spacing={2} align="stretch">
                  {recommendations.map((pl) => renderPlaylistRow(pl, "rec-"))}
                </VStack>
              )}
            </Box>
          </LiquidGlassCard>
        </VStack>
      </HStack>

      {/* 底部播放器 */}
      <PlayerBar />
    </VStack>
  );
}
