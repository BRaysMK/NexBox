import React, { useState, useEffect, useRef, useCallback } from "react";
import {
  Modal,
  ModalOverlay,
  ModalContent,
  ModalHeader,
  ModalBody,
  ModalCloseButton,
  Box,
  Text,
  HStack,
  VStack,
  Input,
  useColorModeValue,
  IconButton,
  Flex,
  Badge,
  Tooltip,
} from "@chakra-ui/react";
import { useTranslation } from "react-i18next";
import { motion, AnimatePresence } from "framer-motion";
import { FiSend, FiTrash2, FiPlus, FiDelete, FiGlobe, FiSquare } from "react-icons/fi";
import { AiOutlineBranches, AiOutlineExclamationCircle } from "react-icons/ai";
import { LiquidGlassButton } from "@/components/special/liquid-glass-button";
import { useThemeColor } from "@/contexts/theme-color-context";
import { hexToRgba } from "@/lib/color-utils";
import { store } from "@/lib/store";
import {
  ChatMessage,
  AiMemoryEntry,
  sendChatStream,
  getMemory,
  addMemory,
  deleteMemory,
  cancelStream,
} from "@/lib/ai";
import boxcatImg from "@/assets/boxcat.png";

interface AiChatModalProps {
  isOpen: boolean;
  onClose: () => void;
}

const HISTORY_KEY = "nexbox_ai_chat_history";
const WEB_KEY = "nexbox_ai_web_enabled";
const PROVIDER_HINT = "此AI接入图吧工具箱WinUI3官方API";

export default function AiChatModal({ isOpen, onClose }: AiChatModalProps) {
  const { t } = useTranslation();
  const { getActiveColor, getHoverColor, getContrastTextColor } = useThemeColor();
  const primary = getActiveColor();

  // 配色（主题色驱动）
  const textColor = useColorModeValue("gray.800", "#ffffff");
  const subTextColor = useColorModeValue("gray.600", "#A0AEC0");
  const modalBg = useColorModeValue("rgba(255,255,255,0.92)", "rgba(26,26,26,0.92)");
  const userBubbleBg = `linear-gradient(135deg, ${getHoverColor()}, ${hexToRgba(primary, 0.85)})`;
  const userTextColor = getContrastTextColor();
  const aiBubbleBg = useColorModeValue("rgba(0,0,0,0.05)", "rgba(255,255,255,0.07)");

  // 对话状态
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const historyLoadedRef = useRef(false);
  const stopRef = useRef(false);

  // 联网开关
  const [webEnabled, setWebEnabled] = useState(false);

  // 记忆状态
  const [memories, setMemories] = useState<AiMemoryEntry[]>([]);
  const [showMemory, setShowMemory] = useState(false);
  const [memoryInput, setMemoryInput] = useState("");
  const [memoryBusy, setMemoryBusy] = useState(false);

  const loadMemories = useCallback(async () => {
    try {
      setMemories(await getMemory());
    } catch {
      // 忽略
    }
  }, []);

  // 初始化：加载持久化的消息历史、联网开关
  useEffect(() => {
    if (!historyLoadedRef.current) {
      (async () => {
        try {
          const saved = await store.get<ChatMessage[]>(HISTORY_KEY);
          if (saved && Array.isArray(saved) && saved.length > 0) {
            setMessages(saved);
          } else {
            const ls = localStorage.getItem(HISTORY_KEY);
            if (ls) {
              try {
                const parsed = JSON.parse(ls) as ChatMessage[];
                if (Array.isArray(parsed) && parsed.length > 0) setMessages(parsed);
                else setMessages([welcome(t)]);
              } catch {
                setMessages([welcome(t)]);
              }
            } else {
              setMessages([welcome(t)]);
            }
          }
        } catch {
          setMessages([welcome(t)]);
        }

        try {
          const w = await store.get<boolean>(WEB_KEY);
          if (w !== null && w !== undefined) setWebEnabled(w);
          else {
            const ls = localStorage.getItem(WEB_KEY);
            if (ls !== null) setWebEnabled(ls === "true");
          }
        } catch {
          // ignore
        }

        // 异步加载完成后再允许持久化，避免初始空数组覆盖历史记录
        historyLoadedRef.current = true;
      })();
    }
  }, []);

  // 记忆面板打开时才加载（按需）
  useEffect(() => {
    if (showMemory) loadMemories();
  }, [showMemory, loadMemories]);

  // 自动滚动到底部
  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: "smooth" });
  }, [messages, loading]);

  // 打开弹窗时直接跳到最新消息（等弹窗渲染出最终高度后再滚动）
  useEffect(() => {
    if (!isOpen) return;
    const timer = setTimeout(() => {
      scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight });
    }, 60);
    return () => clearTimeout(timer);
  }, [isOpen]);

  // 持久化消息历史（仅保留 user/assistant，跳过空消息）
  useEffect(() => {
    if (!historyLoadedRef.current) return;
    const trimmed = messages.filter(
      (m) => (m.role === "user" || m.role === "assistant") && m.content.trim().length > 0
    );
    const last = trimmed.slice(-100);
    store.set(HISTORY_KEY, last).catch(() => {});
    try {
      localStorage.setItem(HISTORY_KEY, JSON.stringify(last));
    } catch {
      // ignore quota
    }
  }, [messages]);

  // 持久化联网开关
  useEffect(() => {
    store.set(WEB_KEY, webEnabled).catch(() => {});
    try {
      localStorage.setItem(WEB_KEY, String(webEnabled));
    } catch {
      // ignore
    }
  }, [webEnabled]);

  const handleSend = async () => {
    const text = input.trim();
    if (!text || loading) return;

    stopRef.current = false;
    const userMsg: ChatMessage = { role: "user", content: text };
    const history = [...messages.filter((m) => m.role !== "system" && m.role !== "search"), userMsg];

    // 联网搜索时在 user 与 assistant 之间插入一条 search 消息（结果展示在 AI 输出上方）
    if (webEnabled) {
      setMessages([
        ...history,
        { role: "search", content: "", search: { query: text, items: [] } },
        { role: "assistant", content: "" },
      ]);
    } else {
      setMessages([...history, { role: "assistant", content: "" }]);
    }
    setInput("");
    setLoading(true);

    try {
      let acc = "";
      const full = await sendChatStream(
        history,
        (delta) => {
          if (stopRef.current) return;
          acc += delta;
          setMessages((prev) => {
            const next = [...prev];
            const last = next[next.length - 1];
            if (last?.role === "assistant") {
              next[next.length - 1] = { ...last, content: acc };
            } else {
              next.push({ role: "assistant", content: acc });
            }
            return next;
          });
        },
        webEnabled,
        () => {
          stopRef.current = true;
        },
        {
          onSearchStart: (query) => {
            // 更新 search 消息的查询词（后端已剥离「搜索」等动词前缀）
            setMessages((prev) =>
              prev.map((m) =>
                m.role === "search" && m.search?.query === text
                  ? { ...m, search: { query, items: [] } }
                  : m
              )
            );
          },
          onSearchResult: (result) => {
            setMessages((prev) =>
              prev.map((m) =>
                m.role === "search" && m.search?.query === result.query
                  ? { ...m, search: { query: result.query, items: result.items, done: true } }
                  : m
              )
            );
          },
        }
      );
      // 未被打断时用完整内容兜底；已打断则保留已累积的部分
      if (!stopRef.current && full) {
        setMessages((prev) => {
          const next = [...prev];
          const last = next[next.length - 1];
          if (last?.role === "assistant" && last.content.length < full.length) {
            next[next.length - 1] = { ...last, content: full };
          }
          return next;
        });
      }
    } catch (e) {
      const errMsg = e instanceof Error ? e.message : String(e);
      setMessages((prev) => [...prev, { role: "assistant", content: errMsg }]);
    } finally {
      setLoading(false);
      stopRef.current = false;
    }
  };

  const handleStop = () => {
    stopRef.current = true;
    cancelStream();
  };

  const handleClear = () => {
    if (loading) {
      stopRef.current = true;
      cancelStream();
    }
    setMessages([welcome(t)]);
    setInput("");
  };

  const handleAddMemory = async () => {
    const content = memoryInput.trim();
    if (!content || memoryBusy) return;
    setMemoryBusy(true);
    try {
      await addMemory(content);
      setMemoryInput("");
      await loadMemories();
    } catch (e) {
      const errMsg = e instanceof Error ? e.message : String(e);
      setMemories((prev) => [...prev, { id: "__error__", content: errMsg, created_at: "", builtin: false }]);
    } finally {
      setMemoryBusy(false);
    }
  };

  const handleDeleteMemory = async (id: string) => {
    try {
      await deleteMemory(id);
      await loadMemories();
    } catch {
      // 忽略
    }
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  // 记忆面板只显示用户自定义记忆（不显示系统/内置）
  const customMemories = memories.filter((m) => !m.builtin);

  const avatarPulse = {
    boxShadow: [
      `0 0 12px ${hexToRgba(primary, 0.45)}`,
      `0 0 24px ${hexToRgba(primary, 0.9)}`,
      `0 0 12px ${hexToRgba(primary, 0.45)}`,
    ],
  };

  return (
    <Modal isOpen={isOpen} onClose={onClose} isCentered size="xl" scrollBehavior="inside">
      <ModalOverlay backdropFilter="blur(6px)" />
      <ModalContent
        bg={modalBg}
        border="1px solid"
        borderColor={hexToRgba(primary, 0.55)}
        borderRadius="2xl"
        maxH="85vh"
        overflow="hidden"
        backdropFilter="blur(20px)"
        boxShadow={`0 0 0 1px ${hexToRgba(primary, 0.2)}, 0 0 24px ${hexToRgba(primary, 0.25)}, 0 8px 40px rgba(0,0,0,0.35)`}
      >
        {/* 顶部：盒子喵信息 */}
        <ModalHeader pb={3}>
          <HStack spacing={3} justify="space-between" w="full" pr={10}>
            <HStack spacing={3} minW={0}>
              {/* 猫娘头像：主题色光晕 */}
              <motion.div
                animate={avatarPulse}
                transition={{ duration: 2.5, repeat: Infinity, ease: "easeInOut" }}
                style={{ borderRadius: "50%" }}
              >
                <Box
                  w="44px"
                  h="44px"
                  borderRadius="full"
                  overflow="hidden"
                  bg={`linear-gradient(135deg, ${getHoverColor()}, ${primary})`}
                  border="2px solid"
                  borderColor={hexToRgba(primary, 0.7)}
                  display="flex"
                  alignItems="center"
                  justifyContent="center"
                  flexShrink={0}
                >
                  <img
                    src={boxcatImg}
                    alt="BoxCat"
                    style={{ width: "100%", height: "100%", objectFit: "cover", display: "block" }}
                  />
                </Box>
              </motion.div>
              <VStack align="start" spacing={0} minW={0}>
                <Text fontSize="lg" fontWeight="bold" color={textColor}>
                  {t("ai.name", "盒子喵")}
                </Text>
                <Text fontSize="xs" color={subTextColor}>
                  {t("ai.subtitle", "新境盒专属 AI 助手")}
                </Text>
              </VStack>
            </HStack>
            {/* 叹号 + Tooltip */}
            <Tooltip label={PROVIDER_HINT} placement="bottom" hasArrow openDelay={100}>
              <Box
                as="span"
                display="inline-flex"
                alignItems="center"
                justifyContent="center"
                color={primary}
                cursor="help"
                aria-label={PROVIDER_HINT}
                _hover={{ color: getHoverColor() }}
                transition="color 0.2s"
              >
                <AiOutlineExclamationCircle size={20} />
              </Box>
            </Tooltip>
          </HStack>
        </ModalHeader>
        <ModalCloseButton />

        <ModalBody p={0}>
          <Flex direction="column" h="55vh">
            {/* 消息列表 */}
            <Box
              ref={scrollRef}
              flex={1}
              overflowY="auto"
              px={4}
              py={2}
              sx={{
                scrollbarGutter: "stable",
                "&::-webkit-scrollbar": { width: "5px" },
                "&::-webkit-scrollbar-track": { background: "transparent" },
                "&::-webkit-scrollbar-thumb": {
                  background: `${primary}88`,
                  borderRadius: "3px",
                },
                "&::-webkit-scrollbar-thumb:hover": { background: primary },
              }}
            >
              <AnimatePresence>
                {messages.map((msg, i) => {
                  // 搜索结果消息：显示在 AI 输出上方（user 之后、assistant 之前）
                  if (msg.role === "search") {
                    const s = msg.search;
                    const searching = !s || (!s.done && s.items.length === 0 && loading);
                    const noResult = s?.done === true && s.items.length === 0;
                    return (
                      <motion.div
                        key={i}
                        initial={{ opacity: 0, y: 8 }}
                        animate={{ opacity: 1, y: 0 }}
                        exit={{ opacity: 0 }}
                        transition={{ duration: 0.2 }}
                      >
                        <HStack align="flex-start" spacing={2} mb={3}>
                          <Box w="30px" flexShrink={0} />
                          <Box
                            flex={1}
                            px={3}
                            py={2}
                            borderRadius="xl"
                            border="1px solid"
                            borderColor={hexToRgba(primary, 0.35)}
                            bg={hexToRgba(primary, 0.08)}
                          >
                            {searching ? (
                              <HStack spacing={2}>
                                <motion.div
                                  animate={{ opacity: [1, 0.3, 1] }}
                                  transition={{ duration: 1.2, repeat: Infinity }}
                                >
                                  <FiGlobe size={14} color={primary} />
                                </motion.div>
                                <Text fontSize="sm" color={subTextColor}>
                                  {t("ai.searching", "正在联网搜索：")}
                                  <Text as="span" fontWeight="semibold" color={primary}>
                                    「{s?.query ?? ""}」
                                  </Text>
                                </Text>
                              </HStack>
                            ) : noResult ? (
                              <HStack spacing={2}>
                                <FiGlobe size={14} color={primary} />
                                <Text fontSize="sm" color={subTextColor}>
                                  {t("ai.searchNoResult", "没有找到相关网页结果喵")}
                                </Text>
                              </HStack>
                            ) : (
                              <VStack align="start" spacing={1}>
                                <HStack spacing={2}>
                                  <FiGlobe size={13} color={primary} />
                                  <Text fontSize="xs" fontWeight="semibold" color={primary}>
                                    {t("ai.searchResultTitle", "联网搜索结果")}
                                  </Text>
                                </HStack>
                                {s?.items.slice(0, 5).map((r, idx) => (
                                  <HStack key={idx} spacing={2} align="flex-start" w="full">
                                    <Text fontSize="xs" color={subTextColor} minW="14px">
                                      {idx + 1}.
                                    </Text>
                                    <VStack align="start" spacing={0} minW={0} flex={1}>
                                      {r.url ? (
                                        <Box
                                          as="a"
                                          href={r.url}
                                          target="_blank"
                                          rel="noreferrer"
                                          fontSize="sm"
                                          fontWeight="semibold"
                                          color={primary}
                                          noOfLines={1}
                                          _hover={{ textDecoration: "underline" }}
                                        >
                                          {r.title}
                                        </Box>
                                      ) : (
                                        <Text fontSize="sm" fontWeight="semibold" color={textColor} noOfLines={1}>
                                          {r.title}
                                        </Text>
                                      )}
                                      {r.snippet && (
                                        <Text fontSize="xs" color={subTextColor} noOfLines={2}>
                                          {r.snippet}
                                        </Text>
                                      )}
                                    </VStack>
                                  </HStack>
                                ))}
                              </VStack>
                            )}
                          </Box>
                        </HStack>
                      </motion.div>
                    );
                  }

                  const isUser = msg.role === "user";
                  return (
                    <motion.div
                      key={i}
                      initial={{ opacity: 0, y: 8 }}
                      animate={{ opacity: 1, y: 0 }}
                      exit={{ opacity: 0 }}
                      transition={{ duration: 0.2 }}
                    >
                      <HStack
                        align="flex-start"
                        spacing={2}
                        justify={isUser ? "flex-end" : "flex-start"}
                        mb={3}
                      >
                        {!isUser && (
                          <Box
                            w="30px"
                            h="30px"
                            borderRadius="full"
                            overflow="hidden"
                            bg={`linear-gradient(135deg, ${getHoverColor()}, ${primary})`}
                            display="flex"
                            alignItems="center"
                            justifyContent="center"
                            flexShrink={0}
                            boxShadow={`0 0 8px ${hexToRgba(primary, 0.6)}`}
                          >
                            <img
                              src={boxcatImg}
                              alt="BoxCat"
                              style={{ width: "100%", height: "100%", objectFit: "cover", display: "block" }}
                            />
                          </Box>
                        )}
                        <Box
                          maxW="80%"
                          px={3}
                          py={2}
                          borderRadius="xl"
                          bg={isUser ? userBubbleBg : aiBubbleBg}
                          color={isUser ? userTextColor : textColor}
                          borderTopRightRadius={isUser ? 4 : "xl"}
                          borderTopLeftRadius={isUser ? "xl" : 4}
                          whiteSpace="pre-wrap"
                          wordBreak="break-word"
                          boxShadow={
                            isUser
                              ? `0 6px 18px ${hexToRgba(primary, 0.35)}, 0 0 14px ${hexToRgba(primary, 0.2)}`
                              : "0 2px 8px rgba(0,0,0,0.18)"
                          }
                        >
                          {msg.content ||
                            (loading && i === messages.length - 1 && (
                              <HStack spacing={2}>
                                <motion.span
                                  animate={{ opacity: [1, 0.3, 1] }}
                                  transition={{ duration: 1.2, repeat: Infinity }}
                                >
                                  {t("ai.thinking", "思考中")}…
                                </motion.span>
                              </HStack>
                            ))}
                        </Box>
                      </HStack>
                    </motion.div>
                  );
                })}
              </AnimatePresence>
            </Box>

            {/* 记忆管理面板：过滤掉 builtin（不显示系统记忆） */}
            {showMemory && (
              <Box px={4} py={2} borderTop="1px solid" borderColor={useColorModeValue("gray.200", "#2d2d2d")}>
                <HStack justify="space-between" mb={2}>
                  <Text fontSize="sm" fontWeight="semibold" color={textColor}>
                    {t("ai.memoryTitle", "记忆管理")}
                  </Text>
                  <Badge bg={hexToRgba(primary, 0.18)} color={getContrastTextColor()} variant="solid">
                    {customMemories.length}
                  </Badge>
                </HStack>
                <Box
                  maxH="160px"
                  overflowY="auto"
                  mb={2}
                  sx={{
                    "&::-webkit-scrollbar": { width: "4px" },
                    "&::-webkit-scrollbar-track": { background: "transparent" },
                    "&::-webkit-scrollbar-thumb": {
                      background: `${primary}88`,
                      borderRadius: "2px",
                    },
                    "&::-webkit-scrollbar-thumb:hover": { background: primary },
                  }}
                >
                  {customMemories.length === 0 ? (
                    <Text fontSize="xs" color={subTextColor} py={2}>
                      {t("ai.memoryEmpty", "还没有自定义记忆，给盒子喵加点吧喵~")}
                    </Text>
                  ) : (
                    customMemories.map((m) => (
                      <HStack key={m.id} justify="space-between" py={1} spacing={2}>
                        <Text fontSize="sm" color={subTextColor} flex={1} noOfLines={2}>
                          {m.content}
                        </Text>
                        <IconButton
                          aria-label="delete"
                          size="xs"
                          variant="ghost"
                          color={getContrastTextColor()}
                          icon={<FiDelete />}
                          onClick={() => handleDeleteMemory(m.id)}
                        />
                      </HStack>
                    ))
                  )}
                </Box>
                <HStack spacing={2}>
                  <Input
                    size="sm"
                    placeholder={t("ai.memoryPlaceholder", "给盒子喵记一条关于你的信息…")}
                    value={memoryInput}
                    onChange={(e) => setMemoryInput(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && handleAddMemory()}
                    disabled={memoryBusy}
                  />
                  <LiquidGlassButton size="sm" onClick={handleAddMemory} isDisabled={memoryBusy}>
                    <FiPlus />
                  </LiquidGlassButton>
                </HStack>
              </Box>
            )}

            {/* 底部操作栏 + 输入框 */}
            <Box px={4} py={3} borderTop="1px solid" borderColor={useColorModeValue("gray.200", "#2d2d2d")}>
              {/* 联网状态提示条 */}
              {webEnabled && (
                <HStack spacing={2} mb={2} px={2} py={1} borderRadius="md" bg={hexToRgba(primary, 0.12)}>
                  <motion.div
                    animate={{ opacity: [1, 0.5, 1] }}
                    transition={{ duration: 1.5, repeat: Infinity }}
                  >
                    <FiGlobe size={13} color={primary} />
                  </motion.div>
                  <Text fontSize="xs" color={primary}>
                    {t("ai.webActive", "联网搜索已开启，回答会基于最新网络信息喵~")}
                  </Text>
                </HStack>
              )}
              <HStack spacing={2}>
                <Tooltip label={t("ai.clear", "清空对话")}>
                  <IconButton
                    aria-label="clear"
                    size="sm"
                    variant="ghost"
                    color={getContrastTextColor()}
                    icon={<FiTrash2 />}
                    onClick={handleClear}
                  />
                </Tooltip>
                <Tooltip label={t("ai.memory", "记忆管理")}>
                  <IconButton
                    aria-label="memory"
                    size="sm"
                    variant="ghost"
                    color={showMemory ? primary : getContrastTextColor()}
                    icon={<AiOutlineBranches />}
                    onClick={() => setShowMemory((v) => !v)}
                  />
                </Tooltip>
                <Tooltip
                  label={webEnabled ? t("ai.webOn", "联网搜索：开") : t("ai.webOff", "联网搜索：关")}
                  placement="top"
                  hasArrow
                >
                  <Box
                    as="button"
                    aria-label="web-search"
                    display="inline-flex"
                    alignItems="center"
                    gap="6px"
                    px={2.5}
                    h="32px"
                    borderRadius="full"
                    cursor="pointer"
                    userSelect="none"
                    onClick={() => setWebEnabled((v) => !v)}
                    bg={webEnabled ? hexToRgba(primary, 0.18) : "transparent"}
                    border="1px solid"
                    borderColor={webEnabled ? hexToRgba(primary, 0.6) : hexToRgba(getContrastTextColor(), 0.25)}
                    color={webEnabled ? primary : getContrastTextColor()}
                    boxShadow={webEnabled ? `0 0 12px ${hexToRgba(primary, 0.45)}` : undefined}
                    transition="all 0.2s"
                    _hover={{ borderColor: hexToRgba(primary, 0.8), color: primary }}
                    _active={{ transform: "scale(0.95)" }}
                    fontSize="xs"
                    fontWeight="semibold"
                  >
                    <FiGlobe size={14} />
                    <Text as="span" whiteSpace="nowrap">
                      {t("ai.web", "联网")}
                    </Text>
                    <Box
                      w="6px"
                      h="6px"
                      borderRadius="full"
                      bg={webEnabled ? primary : "transparent"}
                      border={webEnabled ? "none" : `1px solid ${hexToRgba(getContrastTextColor(), 0.4)}`}
                    />
                  </Box>
                </Tooltip>
                <Input
                  flex={1}
                  value={input}
                  onChange={(e) => setInput(e.target.value)}
                  onKeyDown={onKeyDown}
                  placeholder={
                    webEnabled
                      ? t("ai.inputPlaceholderWeb", "联网搜索中…问点最新的事情喵~")
                      : t("ai.inputPlaceholder", "跟盒子喵说点什么喵~")
                  }
                  isDisabled={loading}
                  border="1px solid"
                  borderColor={hexToRgba(primary, 0.45)}
                  borderRadius="full"
                  boxShadow={loading ? undefined : `0 0 10px ${hexToRgba(primary, 0.15)}`}
                  _hover={{ borderColor: hexToRgba(primary, 0.7) }}
                  _focus={{
                    borderColor: primary,
                    boxShadow: `0 0 0 1px ${hexToRgba(primary, 0.5)}, 0 0 16px ${hexToRgba(primary, 0.35)}`,
                  }}
                  _disabled={{ opacity: 0.6 }}
                />
                <Tooltip label={loading ? t("ai.stop", "停止输出") : t("ai.send", "发送")}>
                  <LiquidGlassButton
                    onClick={loading ? handleStop : handleSend}
                    isDisabled={!loading && !input.trim()}
                    size="md"
                    boxShadow={`0 6px 18px ${hexToRgba(primary, 0.45)}`}
                  >
                    {loading ? <FiSquare size={16} /> : <FiSend />}
                  </LiquidGlassButton>
                </Tooltip>
              </HStack>
            </Box>
          </Flex>
        </ModalBody>
      </ModalContent>
    </Modal>
  );
}

function welcome(t: any): ChatMessage {
  return {
    role: "assistant",
    content: t("ai.welcome", "喵呜~ 我是盒子喵！关于新境盒的任何问题都可以问我哦~(≧▽≦)喵~"),
  };
}