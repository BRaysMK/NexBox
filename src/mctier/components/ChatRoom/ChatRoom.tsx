import React, { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Box,
  Text,
  Textarea,
  Button,
  IconButton,
  HStack,
  Modal,
  ModalOverlay,
  ModalContent,
  ModalBody,
  ModalCloseButton,
  useToast,
  useColorModeValue,
} from '@chakra-ui/react';
import { SendOutlined } from '@ant-design/icons';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '../../stores';
import { p2pChatService } from '../../services/chat/P2PChatService';
import { EmojiPicker } from '../EmojiPicker/EmojiPicker';
import { EmojiIcon, ImageIcon } from '../icons';
import type { ChatMessage } from '../../types';
import './ChatRoom.css';

export const ChatRoom: React.FC = () => {
  const { currentPlayerId, chatMessages, addChatMessage, config } = useAppStore();
  const [inputValue, setInputValue] = useState('');
  const [isAtBottom, setIsAtBottom] = useState(true);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [hasMoreMessages, setHasMoreMessages] = useState(true);
  const [displayedMessageCount, setDisplayedMessageCount] = useState(30);
  const [lastReadMessageIndex, setLastReadMessageIndex] = useState(0);
  const [showEmojiPicker, setShowEmojiPicker] = useState(false);
  const [isUploading, setIsUploading] = useState(false);
  const [previewImage, setPreviewImage] = useState<string | null>(null);
  const [downloadingImageId, setDownloadingImageId] = useState<string | null>(null);
  const [downloadedImages, setDownloadedImages] = useState<Map<string, string>>(new Map());

  const toast = useToast();

  // 主题模式颜色
  const msgListBg = useColorModeValue('gray.50', 'transparent');
  const mutedTextColor = useColorModeValue('gray.400', 'whiteAlpha.500');
  const ownAvatarBg = useColorModeValue(
    'linear-gradient(135deg, #95ec69 0%, #7cc856 100%)',
    'linear-gradient(135deg, rgba(80, 80, 90, 0.4) 0%, rgba(60, 60, 70, 0.4) 100%)'
  );
  const otherAvatarBg = useColorModeValue(
    'linear-gradient(135deg, #e2e8f0 0%, #cbd5e0 100%)',
    'linear-gradient(135deg, rgba(255, 255, 255, 0.15) 0%, rgba(255, 255, 255, 0.08) 100%)'
  );
  const avatarBorder = useColorModeValue('2px solid #e2e8f0', '2px solid rgba(255, 255, 255, 0.1)');
  const avatarIconColor = useColorModeValue('#718096', 'rgba(255, 255, 255, 0.6)');
  const authorColor = useColorModeValue('gray.500', 'whiteAlpha.600');
  const otherMsgBg = useColorModeValue(
    'linear-gradient(135deg, #edf2f7 0%, #e2e8f0 100%)',
    'linear-gradient(135deg, rgba(80, 80, 90, 0.6) 0%, rgba(60, 60, 70, 0.6) 100%)'
  );
  const otherMsgColor = useColorModeValue('gray.800', 'whiteAlpha.900');
  const timeColor = useColorModeValue('gray.400', 'whiteAlpha.400');
  const downloadBtnBg = useColorModeValue('rgba(0, 0, 0, 0.4)', 'rgba(0, 0, 0, 0.6)');
  const downloadBtnBorder = useColorModeValue('1px solid rgba(255, 255, 255, 0.3)', '1px solid rgba(255, 255, 255, 0.2)');
  const downloadBtnHoverBg = useColorModeValue('rgba(0, 0, 0, 0.6)', 'rgba(0, 0, 0, 0.8)');
  const newMsgBg = useColorModeValue('rgba(160, 160, 170, 0.9)', 'rgba(100, 100, 120, 0.9)');
  const newMsgShadow = useColorModeValue('0 2px 8px rgba(0, 0, 0, 0.15)', '0 2px 8px rgba(0, 0, 0, 0.3)');
  const badgeBorder = useColorModeValue('2px solid white', '2px solid rgba(20, 30, 25, 0.95)');
  const previewOverlayBg = useColorModeValue('rgba(0, 0, 0, 0.7)', 'rgba(0, 0, 0, 0.9)');
  const inputAreaBg = useColorModeValue('white', 'rgba(30, 30, 40, 0.95)');
  const inputAreaBorder = useColorModeValue('1px solid #e2e8f0', '1px solid rgba(255, 255, 255, 0.1)');
  const btnColor = useColorModeValue('gray.500', 'whiteAlpha.700');
  const btnHoverColor = useColorModeValue('gray.700', 'whiteAlpha.900');
  const btnHoverBg = useColorModeValue('gray.100', 'whiteAlpha.100');
  const inputBg = useColorModeValue('gray.50', 'rgba(255,255,255,0.05)');
  const inputBorder = useColorModeValue('gray.200', 'rgba(255,255,255,0.1)');
  const inputColor = useColorModeValue('gray.800', 'white');
  const inputPlaceholder = useColorModeValue({ color: 'gray.400' }, { color: 'rgba(255,255,255,0.4)' });
  const inputHoverBorder = useColorModeValue('gray.300', 'rgba(255,255,255,0.2)');
  // 消息尾巴 CSS 变量
  const ownTailColor = useColorModeValue('#7cc856', '#7cc856');
  const otherTailColor = useColorModeValue('#e2e8f0', 'rgba(60, 60, 70, 0.6)');

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const messagesContainerRef = useRef<HTMLDivElement>(null);
  const lastScrollTop = useRef(0);
  const textAreaRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // 计算未读消息数量（只计算其他人发送的消息）
  const unreadMessages = chatMessages.filter((msg, index) =>
    msg.playerId !== currentPlayerId && index >= lastReadMessageIndex
  );
  const hasUnreadMessages = unreadMessages.length > 0;

  // 获取MiniWindow的已读消息标记函数
  const markMessagesAsRead = () => {
    window.dispatchEvent(new CustomEvent('markChatMessagesAsRead'));
  };

  // 设置全局标志：当前在聊天室界面
  useEffect(() => {
    (window as any).__isInChatRoom__ = true;
    console.log('✅ 已设置全局标志：当前在聊天室界面');

    return () => {
      (window as any).__isInChatRoom__ = false;
      console.log('✅ 已清除全局标志：离开聊天室界面');
    };
  }, []);

  // 监听滚动位置
  const handleScroll = () => {
    if (!messagesContainerRef.current) return;

    const { scrollTop, scrollHeight, clientHeight } = messagesContainerRef.current;
    const isBottom = Math.abs(scrollHeight - clientHeight - scrollTop) < 50;

    setIsAtBottom(isBottom);

    if (isBottom) {
      setLastReadMessageIndex(chatMessages.length);
      markMessagesAsRead();
    }

    if (scrollTop < 100 && scrollTop < lastScrollTop.current && !isLoadingMore && hasMoreMessages) {
      loadMoreMessages();
    }

    lastScrollTop.current = scrollTop;
  };

  const loadMoreMessages = async () => {
    if (isLoadingMore || !hasMoreMessages) return;

    setIsLoadingMore(true);
    await new Promise(resolve => setTimeout(resolve, 500));

    const newCount = displayedMessageCount + 30;
    setDisplayedMessageCount(newCount);

    if (newCount >= chatMessages.length) {
      setHasMoreMessages(false);
    }

    setIsLoadingMore(false);
  };

  const scrollToBottom = (smooth = true) => {
    if (messagesEndRef.current) {
      messagesEndRef.current.scrollIntoView({
        behavior: smooth ? 'smooth' : 'auto',
        block: 'end'
      });
      setLastReadMessageIndex(chatMessages.length);
      markMessagesAsRead();
    }
  };

  useEffect(() => {
    if (chatMessages.length > 0) {
      if (isAtBottom) {
        scrollToBottom();
      }
    }
  }, [chatMessages.length, isAtBottom]);

  const handleSendMessage = async () => {
    if (!inputValue.trim() || !currentPlayerId) return;

    const messageContent = inputValue.trim();
    setInputValue('');

    try {
      const optimisticMessage: ChatMessage = {
        id: `msg-${currentPlayerId}-${Date.now()}`,
        playerId: currentPlayerId,
        playerName: config.playerName || '我',
        content: messageContent,
        timestamp: Date.now(),
        type: 'text',
      };

      addChatMessage(optimisticMessage);
      console.log('✅ [ChatRoom] 乐观更新：本地显示消息');

      await p2pChatService.sendTextMessage(messageContent);
      console.log('✅ [ChatRoom] 文本消息已发送到P2P网络');
    } catch (error) {
      console.error('发送聊天消息失败:', error);
      toast({ title: '发送消息失败', status: 'error', duration: 2000 });
      setInputValue(messageContent);
    }
  };

  const optimizeImage = async (file: File): Promise<string> => {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = (e) => {
        const img = new Image();
        img.onload = () => {
          const canvas = document.createElement('canvas');
          const ctx = canvas.getContext('2d');
          if (!ctx) {
            reject(new Error('无法创建canvas上下文'));
            return;
          }
          canvas.width = img.width;
          canvas.height = img.height;
          ctx.drawImage(img, 0, 0);
          const optimizedDataUrl = canvas.toDataURL('image/jpeg', 0.92);
          console.log('🖼️ 图片优化完成:', {
            原始大小: file.size,
            优化后大小: Math.round(optimizedDataUrl.length * 0.75),
            压缩率: Math.round((1 - (optimizedDataUrl.length * 0.75) / file.size) * 100) + '%'
          });
          resolve(optimizedDataUrl);
        };
        img.onerror = () => reject(new Error('图片加载失败'));
        img.src = e.target?.result as string;
      };
      reader.onerror = () => reject(new Error('文件读取失败'));
      reader.readAsDataURL(file);
    });
  };

  const handleImageUpload = async () => {
    if (isUploading) return;

    try {
      setIsUploading(true);

      const input = document.createElement('input');
      input.type = 'file';
      input.accept = 'image/*';

      const resetLoading = () => {
        setTimeout(() => {
          if (!input.files || input.files.length === 0) {
            console.log('⚠️ [ChatRoom] 用户取消了文件选择');
            setIsUploading(false);
          }
        }, 100);
      };

      window.addEventListener('focus', resetLoading, { once: true });

      input.onchange = async (e) => {
        window.removeEventListener('focus', resetLoading);

        const file = (e.target as HTMLInputElement).files?.[0];
        if (!file) {
          setIsUploading(false);
          return;
        }

        if (file.size > 10 * 1024 * 1024) {
          toast({ title: '图片大小不能超过10MB', status: 'error', duration: 2000 });
          setIsUploading(false);
          return;
        }

        console.log('📁 选择的图片文件:', file.name, '大小:', file.size);

        try {
          const optimizedDataUrl = await optimizeImage(file);
          console.log('📤 发送优化后的图片消息');

          const optimisticMessage: ChatMessage = {
            id: `msg-${currentPlayerId}-${Date.now()}`,
            playerId: currentPlayerId!,
            playerName: config.playerName || '我',
            content: '[图片]',
            timestamp: Date.now(),
            type: 'image',
            imageData: optimizedDataUrl,
          };

          addChatMessage(optimisticMessage);
          console.log('✅ [ChatRoom] 乐观更新：本地显示图片');

          await p2pChatService.sendImageMessage(optimizedDataUrl);
          toast({ title: '图片发送成功', status: 'success', duration: 1500 });

          setTimeout(() => scrollToBottom(), 100);
        } catch (error) {
          console.error('发送图片失败:', error);
          toast({ title: '发送图片失败', status: 'error', duration: 2000 });
        } finally {
          setIsUploading(false);
        }
      };

      input.click();
    } catch (error) {
      console.error('上传图片失败:', error);
      toast({ title: '上传图片失败', status: 'error', duration: 2000 });
      setIsUploading(false);
    }
  };

  const handlePaste = async (e: React.ClipboardEvent) => {
    const items = e.clipboardData?.items;
    if (!items) return;

    for (let i = 0; i < items.length; i++) {
      const item = items[i];
      if (item.type.indexOf('image') !== -1) {
        e.preventDefault();

        const file = item.getAsFile();
        if (!file) continue;

        if (file.size > 10 * 1024 * 1024) {
          toast({ title: '图片大小不能超过10MB', status: 'error', duration: 2000 });
          return;
        }

        try {
          setIsUploading(true);

          const optimizedDataUrl = await optimizeImage(file);
          console.log('📤 发送粘贴的优化图片');

          const optimisticMessage: ChatMessage = {
            id: `msg-${currentPlayerId}-${Date.now()}`,
            playerId: currentPlayerId!,
            playerName: config.playerName || '我',
            content: '[图片]',
            timestamp: Date.now(),
            type: 'image',
            imageData: optimizedDataUrl,
          };

          addChatMessage(optimisticMessage);
          console.log('✅ [ChatRoom] 乐观更新：本地显示粘贴的图片');

          await p2pChatService.sendImageMessage(optimizedDataUrl);
          toast({ title: '图片发送成功', status: 'success', duration: 1500 });

          setTimeout(() => scrollToBottom(), 100);
          setIsUploading(false);
        } catch (error) {
          console.error('粘贴图片失败:', error);
          toast({ title: '粘贴图片失败', status: 'error', duration: 2000 });
          setIsUploading(false);
        }

        break;
      }
    }
  };

  const handleDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();

    const files = e.dataTransfer?.files;
    if (!files || files.length === 0) return;

    const file = files[0];

    if (!file.type.startsWith('image/')) {
      toast({ title: '只能拖拽图片文件', status: 'error', duration: 2000 });
      return;
    }

    if (file.size > 10 * 1024 * 1024) {
      toast({ title: '图片大小不能超过10MB', status: 'error', duration: 2000 });
      return;
    }

    try {
      setIsUploading(true);

      const optimizedDataUrl = await optimizeImage(file);
      console.log('📤 发送拖拽的优化图片');

      const optimisticMessage: ChatMessage = {
        id: `msg-${currentPlayerId}-${Date.now()}`,
        playerId: currentPlayerId!,
        playerName: config.playerName || '我',
        content: '[图片]',
        timestamp: Date.now(),
        type: 'image',
        imageData: optimizedDataUrl,
      };

      addChatMessage(optimisticMessage);
      console.log('✅ [ChatRoom] 乐观更新：本地显示拖拽的图片');

      await p2pChatService.sendImageMessage(optimizedDataUrl);
      toast({ title: '图片发送成功', status: 'success', duration: 1500 });

      setTimeout(() => scrollToBottom(), 100);
      setIsUploading(false);
    } catch (error) {
      console.error('拖拽图片失败:', error);
      toast({ title: '拖拽图片失败', status: 'error', duration: 2000 });
      setIsUploading(false);
    }
  };

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      e.stopPropagation();
      handleSendMessage();
    }
  };

  const handleEmojiSelect = (emoji: string) => {
    setInputValue(prev => prev + emoji);
    setShowEmojiPicker(false);

    if (textAreaRef.current) {
      textAreaRef.current.focus();
    }
  };

  const handleDownloadImage = async (imageData: string, messageId: string) => {
    try {
      console.log('🖼️ 开始下载图片...');
      setDownloadingImageId(messageId);

      const base64Data = imageData.split(',')[1];

      const filePath = await invoke<string>('save_chat_image', {
        imageData: base64Data,
      });

      console.log('✅ 图片已保存到:', filePath);

      setDownloadedImages(prev => new Map(prev).set(messageId, filePath));
      setDownloadingImageId(null);

      setTimeout(() => {
        setDownloadedImages(prev => {
          const newMap = new Map(prev);
          newMap.delete(messageId);
          return newMap;
        });
      }, 3000);

    } catch (error) {
      console.error('❌ 下载图片失败:', error);
      toast({ title: '下载图片失败', status: 'error', duration: 2000 });
      setDownloadingImageId(null);
    }
  };

  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp);
    const hours = date.getHours().toString().padStart(2, '0');
    const minutes = date.getMinutes().toString().padStart(2, '0');
    return `${hours}:${minutes}`;
  };

  const displayedMessages = chatMessages.slice(-displayedMessageCount);

  return (
    <Box
      display="flex"
      flexDirection="column"
      h="100%"
      flex={1}
      bg={msgListBg}
      overflow="hidden"
      onDrop={handleDrop}
      onDragOver={handleDragOver}
      style={{ '--chat-own-tail-color': ownTailColor, '--chat-other-tail-color': otherTailColor } as React.CSSProperties}
    >
      <Box
        className="chat-messages-scroll"
        ref={messagesContainerRef}
        onScroll={handleScroll}
        flex={1}
        overflowY="auto"
        overflowX="hidden"
        px="56px"
        pt="24px"
        pb="16px"
        display="flex"
        flexDirection="column"
        gap="12px"
        position="relative"
      >
        {isLoadingMore && (
          <Text textAlign="center" py={3} color={mutedTextColor} fontSize="xs">
            加载中...
          </Text>
        )}

        {!hasMoreMessages && chatMessages.length > displayedMessageCount && (
          <Text textAlign="center" py={3} color={mutedTextColor} fontSize="xs">
            没有更多消息了
          </Text>
        )}

        <AnimatePresence mode="popLayout">
          {displayedMessages.map((message) => {
            const isOwnMessage = message.playerId === currentPlayerId;

            return (
              <motion.div
                key={message.id}
                className={`chat-message ${isOwnMessage ? 'own' : 'other'}`}
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -20 }}
                transition={{ duration: 0.2 }}
              >
                {/* 头像 */}
                <Box
                  className="message-avatar"
                  w="36px"
                  h="36px"
                  borderRadius="50%"
                  bg={isOwnMessage ? ownAvatarBg : otherAvatarBg}
                  display="flex"
                  alignItems="center"
                  justifyContent="center"
                  flexShrink={0}
                  border={avatarBorder}
                  position="absolute"
                  top={0}
                >
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="20" height="20" color={avatarIconColor}>
                    <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
                    <circle cx="12" cy="7" r="4" />
                  </svg>
                </Box>

                <Text
                  className="message-author-outside"
                  position="absolute"
                  top="-18px"
                  fontSize="11px"
                  fontWeight={600}
                  color={authorColor}
                  whiteSpace="nowrap"
                >
                  {message.playerName}
                  {isOwnMessage && ' (我)'}
                </Text>

                <Box
                  className={`message-content ${isOwnMessage ? 'own' : 'other'}`}
                  py="8px"
                  px="12px"
                  borderRadius="12px"
                  fontSize="14px"
                  lineHeight={1.4}
                  wordBreak="break-word"
                  whiteSpace="pre-wrap"
                  maxW="100%"
                  w="fit-content"
                  position="relative"
                  userSelect="text"
                  cursor="text"
                  bg={isOwnMessage
                    ? 'linear-gradient(135deg, #95ec69 0%, #7cc856 100%)'
                    : otherMsgBg
                  }
                  color={isOwnMessage ? '#2c2c2c' : otherMsgColor}
                  zIndex={1}
                >
                  {message.type === 'image' && message.imageData ? (
                    <Box position="relative" display="inline-block">
                      <img
                        src={message.imageData}
                        alt="聊天图片"
                        style={{
                          display: 'block',
                          maxWidth: '100%',
                          maxHeight: '300px',
                          width: 'auto',
                          height: 'auto',
                          borderRadius: '8px',
                          cursor: 'pointer',
                          objectFit: 'contain' as const,
                        }}
                        onClick={() => setPreviewImage(message.imageData!)}
                      />
                      <IconButton
                        aria-label="下载图片"
                        icon={
                          downloadingImageId === message.id ? (
                            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="downloading-icon">
                              <circle cx="12" cy="12" r="10" opacity="0.25"/>
                              <path d="M12 2 A10 10 0 0 1 22 12" strokeLinecap="round"/>
                            </svg>
                          ) : (
                            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                              <polyline points="7 10 12 15 17 10"></polyline>
                              <line x1="12" y1="15" x2="12" y2="3"></line>
                            </svg>
                          )
                        }
                        size="xs"
                        position="absolute"
                        bottom="6px"
                        right="6px"
                        borderRadius="50%"
                        bg={downloadBtnBg}
                        backdropFilter="blur(10px)"
                        border={downloadBtnBorder}
                        color="white"
                        opacity={0}
                        _groupHover={{ opacity: 1 }}
                        _hover={{ bg: downloadBtnHoverBg, transform: 'scale(1.1)' }}
                        transition="all 0.3s ease"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleDownloadImage(message.imageData!, message.id);
                        }}
                        isDisabled={downloadingImageId === message.id}
                        title="下载图片"
                        className="image-download-btn"
                      />
                      {downloadedImages.has(message.id) && (
                        <Text
                          className="download-success-tip"
                          position="absolute"
                          bottom="-32px"
                          left="50%"
                          transform="translateX(-50%)"
                          bg="rgba(82, 196, 26, 0.95)"
                          color="white"
                          py="6px"
                          px="14px"
                          borderRadius="6px"
                          fontSize="12px"
                          whiteSpace="nowrap"
                          boxShadow="0 2px 12px rgba(82, 196, 26, 0.5)"
                          maxW="400px"
                          overflow="hidden"
                          textOverflow="ellipsis"
                        >
                          已保存至 {downloadedImages.get(message.id)?.replace(/\\[^\\]+$/, '')}
                        </Text>
                      )}
                    </Box>
                  ) : (
                    message.content
                  )}
                </Box>

                <Text
                  className={`message-time-outside ${isOwnMessage ? 'own' : 'other'}`}
                  position="absolute"
                  bottom="-18px"
                  fontSize="10px"
                  color={timeColor}
                  whiteSpace="nowrap"
                >
                  {formatTime(message.timestamp)}
                </Text>
              </motion.div>
            );
          })}
        </AnimatePresence>

        <div ref={messagesEndRef} />
      </Box>

      {/* 新消息提示 */}
      <AnimatePresence>
        {hasUnreadMessages && !isAtBottom && (
          <motion.div
            initial={{ opacity: 0, scale: 0.8 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.8 }}
            onClick={() => scrollToBottom()}
            title="滚动到底部"
            style={{
              position: 'absolute',
              bottom: '80px',
              right: '16px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              width: '40px',
              height: '40px',
              background: newMsgBg,
              borderRadius: '50%',
              cursor: 'pointer',
              boxShadow: newMsgShadow,
              zIndex: 10,
            }}
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ color: 'white' }}>
              <path d="M12 5v14M19 12l-7 7-7-7"/>
            </svg>
            {hasUnreadMessages && (
              <Box
                position="absolute"
                top="-2px"
                right="-2px"
                w="12px"
                h="12px"
                bg="#ef4444"
                borderRadius="50%"
                border={badgeBorder}
              />
            )}
          </motion.div>
        )}
      </AnimatePresence>

      {/* 图片预览模态框 */}
      <Modal isOpen={!!previewImage} onClose={() => setPreviewImage(null)} size="full" isCentered>
        <ModalOverlay bg={previewOverlayBg} />
        <ModalContent
          bg="transparent"
          boxShadow="none"
          maxW="90vw"
          maxH="90vh"
          display="flex"
          alignItems="center"
          justifyContent="center"
        >
          <ModalCloseButton color="white" zIndex={2} />
          <ModalBody display="flex" alignItems="center" justifyContent="center" p={0}>
            {previewImage && (
              <img
                src={previewImage}
                alt="预览"
                onClick={() => setPreviewImage(null)}
                style={{
                  maxWidth: '100%',
                  maxHeight: '80vh',
                  objectFit: 'contain',
                  borderRadius: '8px',
                  boxShadow: '0 8px 32px rgba(0, 0, 0, 0.5)',
                  cursor: 'pointer',
                }}
              />
            )}
          </ModalBody>
        </ModalContent>
      </Modal>

      {/* Emoji选择器 */}
      {showEmojiPicker && (
        <Box
          position="absolute"
          bottom="70px"
          left="16px"
          right="16px"
          zIndex={1000}
        >
          <EmojiPicker
            onSelect={handleEmojiSelect}
            onClose={() => setShowEmojiPicker(false)}
          />
        </Box>
      )}

      {/* 底栏输入区域 */}
      <motion.div
        initial={{ y: 100, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{
          type: 'spring',
          stiffness: 300,
          damping: 30,
          delay: 0.1
        }}
      >
        <HStack
          gap="8px"
          px="16px"
          py="16px"
          bg={inputAreaBg}
          borderTop={inputAreaBorder}
          alignItems="flex-end"
        >
          <IconButton
            aria-label="选择表情"
            icon={<EmojiIcon size={22} />}
            variant="ghost"
            onClick={() => setShowEmojiPicker(!showEmojiPicker)}
            title="选择表情"
            color={btnColor}
            _hover={{ color: btnHoverColor, bg: btnHoverBg }}
          />

          <IconButton
            aria-label="发送图片"
            icon={<ImageIcon size={22} />}
            variant="ghost"
            onClick={handleImageUpload}
            isLoading={isUploading}
            title="发送图片"
            color={btnColor}
            _hover={{ color: btnHoverColor, bg: btnHoverBg }}
          />

          <Textarea
            ref={textAreaRef}
            value={inputValue}
            onChange={(e) => setInputValue(e.target.value)}
            onKeyDown={handleKeyDown}
            onPaste={handlePaste}
            placeholder="Shift+Enter换行"
            maxLength={500}
            flex={1}
            minH="32px"
            maxH="80px"
            p={2}
            fontSize="sm"
            bg={inputBg}
            border="1px solid"
            borderColor={inputBorder}
            color={inputColor}
            _placeholder={inputPlaceholder}
            _hover={{ borderColor: inputHoverBorder }}
            _focus={{ borderColor: '#52c41a', boxShadow: '0 0 0 1px #52c41a' }}
            resize="none"
          />

          <Button
            colorScheme="green"
            size="sm"
            leftIcon={<SendOutlined />}
            onClick={handleSendMessage}
            isDisabled={!inputValue.trim()}
            px={3}
            minW="40px"
            h="32px"
            borderRadius="8px"
          >
          </Button>
        </HStack>
      </motion.div>

      {/* 隐藏的文件输入 */}
      <input
        ref={fileInputRef}
        type="file"
        accept="image/*"
        style={{ display: 'none' }}
        onChange={(e) => {
          const file = e.target.files?.[0];
          if (file) {
            console.log('选择的文件:', file);
          }
        }}
      />
    </Box>
  );
};
