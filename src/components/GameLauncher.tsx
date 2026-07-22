import {
  Box,
  Text,
  Flex,
  Icon,
  Image,
  useColorModeValue,
  IconButton,
  useDisclosure,
  Modal,
  ModalOverlay,
  ModalContent,
  ModalHeader,
  ModalBody,
  ModalCloseButton,
  ModalFooter,
  Button,
  Input,
  VStack,
  Spinner,
} from "@chakra-ui/react";
import { Gamepad2, Plus, X, FolderOpen } from "lucide-react";
import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { LiquidGlassButton } from "@/components/special/liquid-glass-button";

// 过长文字轮播组件
function MarqueeText({ text, color }: { text: string; color: string }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const hiddenRef = useRef<HTMLSpanElement>(null);
  const textRef = useRef<HTMLSpanElement>(null);
  const [shouldScroll, setShouldScroll] = useState(false);
  const [offset, setOffset] = useState(0);
  const animRef = useRef<number | null>(null);

  // 用隐藏的完整文本测量真实宽度，不受 noOfLines 影响
  useEffect(() => {
    const check = () => {
      if (containerRef.current && hiddenRef.current) {
        const textW = hiddenRef.current.scrollWidth;
        const containerW = containerRef.current.clientWidth;
        setShouldScroll(textW > containerW);
      }
    };
    check();
    const observer = new ResizeObserver(check);
    if (containerRef.current) observer.observe(containerRef.current);
    return () => observer.disconnect();
  }, [text]);

  useEffect(() => {
    if (!shouldScroll) { setOffset(0); return; }
    let startTime: number | null = null;
    const totalDuration = 12000;
    const pauseRatio = 0.08;

    const animate = (timestamp: number) => {
      if (!startTime) startTime = timestamp;
      const elapsed = (timestamp - startTime) % totalDuration;
      const progress = elapsed / totalDuration;

      let pos = 0;
      if (progress >= pauseRatio && progress <= 1 - pauseRatio) {
        const scrollProgress = (progress - pauseRatio) / (1 - 2 * pauseRatio);
        if (textRef.current && containerRef.current) {
          const textW = textRef.current.scrollWidth;
          const containerW = containerRef.current.clientWidth;
          const maxScroll = textW - containerW;
          pos = maxScroll > 0 ? scrollProgress : 0;
        }
      }
      if (textRef.current && containerRef.current) {
        const textW = textRef.current.scrollWidth;
        const containerW = containerRef.current.clientWidth;
        const maxScroll = textW - containerW;
        setOffset(maxScroll > 0 ? -pos * maxScroll : 0);
      }
      animRef.current = requestAnimationFrame(animate);
    };

    animRef.current = requestAnimationFrame(animate);
    return () => { if (animRef.current) cancelAnimationFrame(animRef.current); };
  }, [shouldScroll, text]);

  return (
    <Box ref={containerRef} flex={1} overflow="hidden" position="relative" h="20px" display="flex" alignItems="center">
      {/* 隐藏的测量用文本，始终能获取完整宽度 */}
      <Text
        as="span"
        fontSize="sm"
        fontWeight="medium"
        color="transparent"
        whiteSpace="nowrap"
        position="absolute"
        pointerEvents="none"
        ref={hiddenRef}
        style={{ opacity: 0 }}
      >
        {text}
      </Text>
      {shouldScroll ? (
        <Text
          as="span"
          fontSize="sm"
          fontWeight="medium"
          color={color}
          whiteSpace="nowrap"
          ref={textRef}
          style={{ transform: `translateX(${offset}px)`, display: "inline-block" }}
        >
          {text}
        </Text>
      ) : (
        <Text fontSize="sm" fontWeight="medium" color={color} noOfLines={1}>
          {text}
        </Text>
      )}
    </Box>
  );
}

interface GameShortcut {
  id: string;
  name: string;
  path: string;
  isDefault?: boolean;
}

const STORAGE_KEY = "nexbox_game_launcher_games";
const ICON_CACHE_KEY = "nexbox_game_icons";
const SIZE_KEY = "nexbox_game_launcher_size";

const MIN_WIDTH = 180;
const MAX_WIDTH = 600;
const MIN_HEIGHT = 150;
const MAX_HEIGHT = 800;

export default function GameLauncher() {
  const { t } = useTranslation();
  const [games, setGames] = useState<GameShortcut[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isLaunching, setIsLaunching] = useState<string | null>(null);
  const [icons, setIcons] = useState<Record<string, string>>({});
  const loadingIcons = useRef<Set<string>>(new Set());
  const { isOpen, onOpen, onClose } = useDisclosure();
  const [newGameName, setNewGameName] = useState("");
  const [newGamePath, setNewGamePath] = useState("");

  const [cardSize, setCardSize] = useState<{ width: number; height: number }>(() => {
    try {
      const saved = localStorage.getItem(SIZE_KEY);
      if (saved) return JSON.parse(saved);
    } catch {}
    return { width: 180, height: MIN_HEIGHT };
  });
  const resizeRef = useRef<{
    edge: "left" | "top" | "corner";
    startX: number;
    startY: number;
    startW: number;
    startH: number;
  } | null>(null);

  const updateCardSize = useCallback((size: { width: number; height: number }) => {
    setCardSize(size);
    localStorage.setItem(SIZE_KEY, JSON.stringify(size));
  }, []);

  const onResizeStart = useCallback(
    (edge: "left" | "top" | "corner") => (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      resizeRef.current = {
        edge,
        startX: e.clientX,
        startY: e.clientY,
        startW: cardSize.width,
        startH: cardSize.height,
      };

      const cursorMap = { left: "col-resize", top: "row-resize", corner: "nwse-resize" };
      document.body.style.cursor = cursorMap[edge];
      document.body.style.userSelect = "none";

      const onMove = (ev: MouseEvent) => {
        if (!resizeRef.current) return;
        const dx = ev.clientX - resizeRef.current.startX;
        const dy = ev.clientY - resizeRef.current.startY;
        let w = resizeRef.current.startW;
        let h = resizeRef.current.startH;
        if (edge === "left" || edge === "corner") {
          w = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, resizeRef.current.startW - dx));
        }
        if (edge === "top" || edge === "corner") {
          h = Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, resizeRef.current.startH - dy));
        }
        // 限制不能超出视口（卡片固定在 bottom/right，太宽/太高会顶出屏幕）
        const maxW = Math.min(MAX_WIDTH, window.innerWidth - 40);
        const maxH = Math.min(MAX_HEIGHT, window.innerHeight - 140);
        w = Math.max(MIN_WIDTH, Math.min(maxW, w));
        h = Math.max(MIN_HEIGHT, Math.min(maxH, h));
        updateCardSize({ width: Math.round(w), height: Math.round(h) });
      };

      const onUp = () => {
        resizeRef.current = null;
        document.removeEventListener("mousemove", onMove);
        document.removeEventListener("mouseup", onUp);
        document.body.style.cursor = "";
        document.body.style.userSelect = "";
      };

      document.addEventListener("mousemove", onMove);
      document.addEventListener("mouseup", onUp);
    },
    [cardSize, updateCardSize],
  );

  const titleColor = useColorModeValue("gray.800", "#e0e0e0");
  const descColor = useColorModeValue("gray.500", "#888888");
  const cardBg = useColorModeValue("gray.100", "#222222");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const headerColor = useColorModeValue("gray.800", "#ffffff");
  const inputBg = useColorModeValue("white", "#1a1a1a");

  useEffect(() => {
    loadGames();
  }, []);

  const loadGames = async () => {
    setIsLoading(true);
    try {
      const savedGames = localStorage.getItem(STORAGE_KEY);
      const gameList: GameShortcut[] = savedGames ? JSON.parse(savedGames) : [];

      // 加载缓存的图标
      const savedIcons = localStorage.getItem(ICON_CACHE_KEY);
      if (savedIcons) {
        try { setIcons(JSON.parse(savedIcons)); } catch {}
      }

      setGames(gameList);
    } catch (error) {
      console.error("Failed to load games:", error);
    } finally {
      setIsLoading(false);
    }
  };

  // 当 games 变化时，为没有图标的游戏提取图标
  useEffect(() => {
    games.forEach((game) => {
      if (icons[game.id] || loadingIcons.current.has(game.id)) return;
      loadingIcons.current.add(game.id);
      fetchIcon(game);
    });
  }, [games]);

  const fetchIcon = async (game: GameShortcut) => {
    try {
      const dataUri = await invoke<string>("get_file_icon", { filePath: game.path });
      setIcons((prev) => {
        const next = { ...prev, [game.id]: dataUri };
        // 缓存到 localStorage
        localStorage.setItem(ICON_CACHE_KEY, JSON.stringify(next));
        return next;
      });
    } catch (error) {
      console.warn("Failed to get icon for", game.name, error);
    }
  };

  const saveGames = (gameList: GameShortcut[]) => {
    const userGames = gameList.filter((g) => !g.isDefault);
    localStorage.setItem(STORAGE_KEY, JSON.stringify(userGames));
  };

  const handleLaunch = async (game: GameShortcut) => {
    setIsLaunching(game.id);
    try {
      await invoke("launch_game", { gamePath: game.path });
    } catch (error) {
      console.error("Failed to launch game:", error);
    } finally {
      setIsLaunching(null);
    }
  };

  const handleRemove = (gameId: string) => {
    const newGames = games.filter((g) => g.id !== gameId);
    setGames(newGames);
    saveGames(newGames);
  };

  const handleSelectPath = async () => {
    try {
      const selected = await invoke<string | null>("select_exe_file");
      if (selected) {
        setNewGamePath(selected);
        if (!newGameName) {
          const fileName = selected.split(/[/\\]/).pop() || "";
          setNewGameName(fileName.replace(/\.(exe|lnk)$/i, ""));
        }
      }
    } catch (error) {
      console.error("Failed to select file:", error);
    }
  };

  const handleAddGame = () => {
    if (!newGameName.trim() || !newGamePath.trim()) return;

    const newGame: GameShortcut = {
      id: `custom-${Date.now()}`,
      name: newGameName.trim(),
      path: newGamePath.trim(),
      isDefault: false,
    };

    const newGames = [...games, newGame];
    setGames(newGames);
    saveGames(newGames);
    setNewGameName("");
    setNewGamePath("");
    onClose();
  };

  const userGames = games.filter((g) => !g.isDefault);
  const defaultGames = games.filter((g) => g.isDefault);

  const cornerColor = useColorModeValue("rgba(0,0,0,0.25)", "rgba(255,255,255,0.25)");
  const handleHoverBg = useColorModeValue("rgba(0,0,0,0.12)", "rgba(255,255,255,0.12)");

  return (
    <LiquidGlassCard
      p={3}
      w={`${cardSize.width}px`}
      h={`${cardSize.height}px`}
      position="relative"
      overflow="hidden"
      display="flex"
      flexDirection="column"
    >
      {/* 左边缘拖拽手柄 */}
      <Box
        position="absolute"
        left={0}
        top={0}
        bottom={0}
        w="5px"
        cursor="col-resize"
        zIndex={20}
        transition="background 0.15s"
        _hover={{ bg: handleHoverBg }}
        onMouseDown={onResizeStart("left")}
      />

      {/* 上边缘拖拽手柄 */}
      <Box
        position="absolute"
        left={0}
        top={0}
        right={0}
        h="5px"
        cursor="row-resize"
        zIndex={20}
        transition="background 0.15s"
        _hover={{ bg: handleHoverBg }}
        onMouseDown={onResizeStart("top")}
      />

      {/* 右下角拖拽手柄 */}
      <Box
        position="absolute"
        right={0}
        bottom={0}
        w="14px"
        h="14px"
        cursor="nwse-resize"
        zIndex={21}
        onMouseDown={onResizeStart("corner")}
      >
        <Box
          position="absolute"
          right="2px"
          bottom="2px"
          w="8px"
          h="8px"
          borderRight="2px solid"
          borderBottom="2px solid"
          borderColor={cornerColor}
          borderRadius="0 0 2px 0"
        />
      </Box>

      <Flex justify="space-between" align="center" mb={3} flexShrink={0}>
        <Flex align="center" gap={2}>
          <Icon as={Gamepad2} boxSize={4} color={headerColor} />
          <Text fontSize="sm" fontWeight="semibold" color={headerColor}>
            {t("gameLauncher.title")}
          </Text>
        </Flex>
        <IconButton
          aria-label="添加游戏"
          icon={<Icon as={Plus} boxSize={4} />}
          size="xs"
          variant="ghost"
          onClick={onOpen}
        />
      </Flex>

      <Box flex={1} overflowY="auto" overflowX="hidden" minH={0}>
        {isLoading ? (
          <Flex justify="center" py={4}>
            <Spinner size="sm" color={descColor} />
          </Flex>
        ) : games.length === 0 ? (
          <LiquidGlassCard
            isDashed
            p={3}
            textAlign="center"
            cursor="pointer"
            onClick={onOpen}
          >
            <Icon as={Plus} boxSize={5} color={descColor} mb={1} />
            <Text fontSize="xs" color={descColor}>
              {t("gameLauncher.addGame")}
            </Text>
          </LiquidGlassCard>
        ) : (
          <VStack spacing={2} align="stretch">
            {userGames.map((game) => (
              <Flex
                key={game.id}
                align="center"
                gap={2}
                p={2}
                borderRadius="md"
                bg={cardBg}
                cursor={isLaunching === game.id ? "wait" : "pointer"}
                onClick={() => handleLaunch(game)}
                position="relative"
                role="group"
                _hover={{ bg: useColorModeValue("gray.200", "#2a2a2a") }}
                transition="all 0.2s"
              >
                {icons[game.id] && (
                  <Image
                    src={icons[game.id]}
                    boxSize="20px"
                    borderRadius="4px"
                    flexShrink={0}
                    alt=""
                  />
                )}
                <MarqueeText text={game.name} color={titleColor} />
                {isLaunching === game.id ? (
                  <Spinner size="xs" color={descColor} />
                ) : (
                  <IconButton
                    aria-label="删除"
                    icon={<Icon as={X} boxSize={3} />}
                    size="xs"
                    variant="ghost"
                    opacity={0}
                    _groupHover={{ opacity: 1 }}
                    onClick={(e) => {
                      e.stopPropagation();
                      handleRemove(game.id);
                    }}
                  />
                )}
              </Flex>
            ))}

            {defaultGames.map((game) => (
              <Flex
                key={game.id}
                align="center"
                gap={2}
                p={2}
                borderRadius="md"
                bg={cardBg}
                cursor={isLaunching === game.id ? "wait" : "pointer"}
                onClick={() => handleLaunch(game)}
                _hover={{ bg: useColorModeValue("gray.200", "#2a2a2a") }}
                transition="all 0.2s"
              >
                {icons[game.id] && (
                  <Image
                    src={icons[game.id]}
                    boxSize="20px"
                    borderRadius="4px"
                    flexShrink={0}
                    alt=""
                  />
                )}
                <MarqueeText text={game.name} color={titleColor} />
                {isLaunching === game.id && (
                  <Spinner size="xs" color={descColor} />
                )}
              </Flex>
            ))}
          </VStack>
        )}
      </Box>

      <Modal isOpen={isOpen} onClose={onClose} isCentered>
        <ModalOverlay />
        <ModalContent bg={useColorModeValue("white", "#111111")} borderRadius="xl">
          <ModalHeader color={titleColor}>{t("gameLauncher.addGame")}</ModalHeader>
          <ModalCloseButton />
          <ModalBody>
            <VStack spacing={4}>
              <Box w="full">
                <Text fontSize="sm" color={descColor} mb={2}>
                  {t("gameLauncher.gameName")}
                </Text>
                <Input
                  value={newGameName}
                  onChange={(e) => setNewGameName(e.target.value)}
                  placeholder={t("gameLauncher.gameNamePlaceholder")}
                  bg={inputBg}
                  border="1px solid"
                  borderColor={borderColor}
                />
              </Box>
              <Box w="full">
                <Text fontSize="sm" color={descColor} mb={2}>
                  {t("gameLauncher.gamePath")}
                </Text>
                <Flex gap={2}>
                  <Input
                    value={newGamePath}
                    onChange={(e) => setNewGamePath(e.target.value)}
                    placeholder={t("gameLauncher.gamePathPlaceholder")}
                    bg={inputBg}
                    border="1px solid"
                    borderColor={borderColor}
                    flex={1}
                  />
                  <IconButton
                    aria-label="选择文件"
                    icon={<Icon as={FolderOpen} />}
                    onClick={handleSelectPath}
                    variant="outline"
                    border="1px solid"
                    borderColor={borderColor}
                  />
                </Flex>
              </Box>
            </VStack>
          </ModalBody>
          <ModalFooter>
            <Button variant="ghost" mr={3} onClick={onClose}>
              {t("common.cancel")}
            </Button>
            <LiquidGlassButton
              onClick={handleAddGame}
              isDisabled={!newGameName.trim() || !newGamePath.trim()}
            >
              {t("common.add")}
            </LiquidGlassButton>
          </ModalFooter>
        </ModalContent>
      </Modal>
    </LiquidGlassCard>
  );
}
