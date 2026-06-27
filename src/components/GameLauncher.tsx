import {
  Box,
  Text,
  Flex,
  Icon,
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
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { LiquidGlassButton } from "@/components/special/liquid-glass-button";

interface GameShortcut {
  id: string;
  name: string;
  path: string;
  isDefault?: boolean;
}

const STORAGE_KEY = "nexbox_game_launcher_games";

export default function GameLauncher() {
  const { t } = useTranslation();
  const [games, setGames] = useState<GameShortcut[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isLaunching, setIsLaunching] = useState<string | null>(null);
  const { isOpen, onOpen, onClose } = useDisclosure();
  const [newGameName, setNewGameName] = useState("");
  const [newGamePath, setNewGamePath] = useState("");

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

      setGames(gameList);
    } catch (error) {
      console.error("Failed to load games:", error);
    } finally {
      setIsLoading(false);
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
          setNewGameName(fileName.replace(/\.exe$/i, ""));
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

  return (
    <LiquidGlassCard p={3} w="200px">
      <Flex justify="space-between" align="center" mb={3}>
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
              <Text
                fontSize="sm"
                fontWeight="medium"
                color={titleColor}
                flex={1}
                noOfLines={1}
              >
                {game.name}
              </Text>
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
              p={2}
              borderRadius="md"
              bg={cardBg}
              cursor={isLaunching === game.id ? "wait" : "pointer"}
              onClick={() => handleLaunch(game)}
              _hover={{ bg: useColorModeValue("gray.200", "#2a2a2a") }}
              transition="all 0.2s"
            >
              <Text
                fontSize="sm"
                fontWeight="medium"
                color={titleColor}
                flex={1}
                noOfLines={1}
              >
                {game.name}
              </Text>
              {isLaunching === game.id && (
                <Spinner size="xs" color={descColor} />
              )}
            </Flex>
          ))}
        </VStack>
      )}

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
