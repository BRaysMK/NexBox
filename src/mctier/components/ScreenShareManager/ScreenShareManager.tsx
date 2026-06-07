import React, { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Box, Flex, HStack, VStack, Text, Button, Input, Switch,
  Modal, ModalOverlay, ModalContent, ModalHeader, ModalBody, ModalFooter, ModalCloseButton,
  InputGroup, InputRightElement, IconButton, Tooltip,
  useToast, useColorModeValue,
} from '@chakra-ui/react';
import { getCurrentWindow, PhysicalSize } from '@tauri-apps/api/window';
import { useAppStore } from '../../stores';
import { screenShareService } from '../../services/screenShare/ScreenShareService';
import { ScreenShareIcon, InfoIcon } from '../icons';
import { useThemeColor } from '@/contexts/theme-color-context';
import type { ScreenShare } from '../../types';
import './ScreenShareManager.css';

/**
 * 屏幕共享管理器组件
 * 完全独立管理屏幕共享状态，不依赖父组件
 */
export const ScreenShareManager: React.FC = () => {
  const { currentPlayerId } = useAppStore();

  // 主题适配
  const toast = useToast();
  const { getActiveColor, getHoverColor, getBorderColor, getContrastTextColor } = useThemeColor();

  const bg = useColorModeValue('white', '#111111');
  const cardBg = useColorModeValue('gray.50', 'rgba(255,255,255,0.03)');
  const cardBgHover = useColorModeValue('gray.100', 'rgba(255,255,255,0.05)');
  const borderColor = useColorModeValue('gray.200', 'rgba(255,255,255,0.08)');
  const borderColorHover = useColorModeValue('gray.300', 'rgba(255,255,255,0.15)');
  const textColor = useColorModeValue('gray.800', 'white');
  const textColorStrong = useColorModeValue('gray.900', 'rgba(255,255,255,0.9)');
  const mutedText = useColorModeValue('gray.500', 'rgba(255,255,255,0.5)');
  const hintBg = useColorModeValue('blue.50', 'rgba(24, 144, 255, 0.1)');
  const hintBorder = useColorModeValue('blue.200', 'rgba(24, 144, 255, 0.2)');
  const hintColor = useColorModeValue('blue.600', 'rgba(24, 144, 255, 0.9)');
  const hintText = useColorModeValue('blue.700', 'rgba(255,255,255,0.8)');
  const activeColor = getActiveColor();
  const contrastText = getContrastTextColor();
  const barBg = useColorModeValue('gray.50', 'rgba(255,255,255,0.03)');
  const barBorder = useColorModeValue('gray.200', 'rgba(255,255,255,0.1)');
  const emptyColor = useColorModeValue('gray.400', 'rgba(255,255,255,0.5)');
  const modalOptionBg = useColorModeValue('gray.50', 'rgba(255,255,255,0.05)');
  const myShareBg = useColorModeValue('green.50', 'rgba(255,255,255,0.04)');
  const myShareBorder = useColorModeValue('green.200', 'rgba(255,255,255,0.12)');
  const viewingBg = useColorModeValue('green.50', 'rgba(82, 196, 26, 0.08)');
  const viewingBorder = useColorModeValue('green.300', 'rgba(82, 196, 26, 0.4)');
  const hasPasswordBg = useColorModeValue('green.50', 'rgba(82, 196, 26, 0.03)');
  const hasPasswordBorder = useColorModeValue('green.200', 'rgba(82, 196, 26, 0.3)');
  const beingViewedBg = useColorModeValue('red.50', 'rgba(239, 68, 68, 0.08)');
  const beingViewedBorder = useColorModeValue('red.200', 'rgba(239, 68, 68, 0.4)');
  const passwordBadgeBg = useColorModeValue('green.100', 'rgba(82, 196, 26, 0.15)');
  const passwordBadgeColor = useColorModeValue('green.600', 'rgba(82, 196, 26, 0.9)');
  const viewingBadgeBg = useColorModeValue('red.100', 'rgba(239, 68, 68, 0.15)');
  const viewingBadgeColor = useColorModeValue('red.600', 'rgba(239, 68, 68, 0.9)');

  const [activeShares, setActiveShares] = useState<ScreenShare[]>([]);
  const [myShareId, setMyShareId] = useState<string | null>(null);
  const [showStartModal, setShowStartModal] = useState(false);
  const [requirePassword, setRequirePassword] = useState(false);
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [viewingShareId, setViewingShareId] = useState<string | null>(null);
  const [passwordInput, setPasswordInput] = useState('');
  const [showPasswordModal, setShowPasswordModal] = useState(false);
  const [showPasswordInput, setShowPasswordInput] = useState(false);
  const [selectedShare, setSelectedShare] = useState<ScreenShare | null>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const [originalWindowSize, setOriginalWindowSize] = useState<{ width: number; height: number } | null>(null);
  const [pendingStream, setPendingStream] = useState<MediaStream | null>(null);

  // 组件挂载时检查是否有活跃的共享
  useEffect(() => {
    const checkActiveShare = () => {
      const shares = screenShareService.getActiveShares();
      const myShare = shares.find(share => share.playerId === currentPlayerId);
      if (myShare) {
        console.log('🔍 [ScreenShareManager] 检测到活跃的共享:', myShare.id);
        setMyShareId(myShare.id);
      }
      setActiveShares(shares);
      console.log('📋 [ScreenShareManager] 立即加载共享列表:', shares.length, '个共享');
    };

    checkActiveShare();

    const handleScreenShareError = (event: any) => {
      const { error } = event.detail;
      console.error('❌ [ScreenShareManager] 屏幕共享错误:', error);
    };

    window.addEventListener('screen-share-error', handleScreenShareError);

    return () => {
      window.removeEventListener('screen-share-error', handleScreenShareError);
    };
  }, [currentPlayerId]);

  // 监听viewingShareId和pendingStream变化，自动播放视频
  useEffect(() => {
    if (viewingShareId && pendingStream && videoRef.current) {
      console.log('📺 [ScreenShareManager] useEffect: 检测到viewingShareId和pendingStream，开始播放视频');

      const playVideo = async () => {
        try {
          if (!videoRef.current) return;

          videoRef.current.srcObject = pendingStream;

          videoRef.current.onloadedmetadata = () => {
            console.log('📺 [ScreenShareManager] 视频元数据已加载');
          };

          videoRef.current.onplay = () => {
            console.log('✅ [ScreenShareManager] 视频开始播放');
          };

          videoRef.current.onerror = (e) => {
            console.error('❌ [ScreenShareManager] 视频错误:', e);
          };

          await videoRef.current.play();
          console.log('✅ [ScreenShareManager] 视频播放成功');
          setPendingStream(null);
        } catch (playError: any) {
          if (playError.name === 'AbortError') {
            console.log('⚠️ [ScreenShareManager] 视频播放被中断（正常行为）');
          } else {
            console.error('❌ [ScreenShareManager] 视频播放失败:', playError);
            toast({ title: '视频播放失败', status: 'error', duration: 3000, isClosable: true });
          }
        }
      };

      playVideo();
    }
  }, [viewingShareId, pendingStream]);

  // 监听共享列表变化
  useEffect(() => {
    if (viewingShareId) {
      const share = activeShares.find(s => s.id === viewingShareId);
      if (!share) {
        console.log('⚠️ [ScreenShareManager] 正在查看的共享已停止，自动退出查看界面');
        toast({ title: '共享者已停止屏幕共享', status: 'info', duration: 3000, isClosable: true });
        handleStopViewing();
      }
    }
  }, [activeShares, viewingShareId]);

  // 轮询获取共享列表
  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const shares = screenShareService.getActiveShares();
        setActiveShares(shares);
      } catch (error) {
        console.error('获取共享列表失败:', error);
      }
    }, 1000);

    return () => clearInterval(interval);
  }, []);

  // 开始共享
  const handleStartSharingInternal = async () => {
    try {
      console.log('🖥️ 开始屏幕共享...');

      const shareId = await screenShareService.startSharing(
        requirePassword,
        requirePassword ? password : undefined
      );

      setMyShareId(shareId);
      setShowStartModal(false);
      setPassword('');
      toast({ title: '屏幕共享已启动', status: 'success', duration: 3000, isClosable: true });

      console.log('✅ 屏幕共享已启动:', shareId);
    } catch (error: any) {
      console.error('❌ 启动屏幕共享失败:', error);

      if (error.name === 'NotAllowedError') {
        toast({ title: '用户拒绝了屏幕共享权限', status: 'error', duration: 3000, isClosable: true });
      } else if (error.name === 'NotFoundError') {
        toast({ title: '未找到可共享的屏幕', status: 'error', duration: 3000, isClosable: true });
      } else {
        toast({ title: '启动屏幕共享失败', status: 'error', duration: 3000, isClosable: true });
      }
    }
  };

  // 停止共享
  const handleStopSharingInternal = () => {
    if (myShareId) {
      console.log('🛑 [ScreenShareManager] 停止屏幕共享:', myShareId);
      screenShareService.stopSharing(myShareId);
      setMyShareId(null);
      toast({ title: '屏幕共享已停止', status: 'success', duration: 3000, isClosable: true });
    }
  };

  // 查看屏幕
  const handleViewScreen = async (share: ScreenShare) => {
    try {
      if (share.requirePassword) {
        setSelectedShare(share);
        setShowPasswordModal(true);
        return;
      }

      console.log('👀 [ScreenShareManager] 开始查看屏幕:', share.id);

      try {
        const appWindow = getCurrentWindow();
        const currentSize = await appWindow.innerSize();
        setOriginalWindowSize({ width: currentSize.width, height: currentSize.height });
        await appWindow.setSize(new PhysicalSize(1280, 800));
        await appWindow.setResizable(true);
      } catch (error) {
        console.error('❌ [ScreenShareManager] 调整窗口大小失败:', error);
      }

      const stream = await screenShareService.requestViewScreen(share.id);

      setPendingStream(stream);
      setViewingShareId(share.id);

      toast({ title: `正在查看 ${share.playerName} 的屏幕`, status: 'success', duration: 3000, isClosable: true });
    } catch (error) {
      console.error('❌ [ScreenShareManager] 查看屏幕失败:', error);
      toast({ title: '查看屏幕失败', status: 'error', duration: 3000, isClosable: true });
    }
  };

  // 验证密码并查看
  const handlePasswordSubmit = async () => {
    if (!selectedShare) return;

    if (!passwordInput.trim()) {
      toast({ title: '请输入密码', status: 'warning', duration: 3000, isClosable: true });
      return;
    }

    try {
      console.log('👀 [ScreenShareManager] 验证密码后开始查看屏幕:', selectedShare.id);

      const timeoutPromise = new Promise<never>((_, reject) => {
        setTimeout(() => {
          reject(new Error('等待响应超时，请检查密码是否正确或信令服务器是否正常'));
        }, 30000);
      });

      const stream = await Promise.race([
        screenShareService.requestViewScreen(selectedShare.id, passwordInput),
        timeoutPromise
      ]);

      setShowPasswordModal(false);
      setPasswordInput('');
      setShowPasswordInput(false);

      const shareToView = selectedShare;
      setSelectedShare(null);

      try {
        const appWindow = getCurrentWindow();
        const currentSize = await appWindow.innerSize();
        setOriginalWindowSize({ width: currentSize.width, height: currentSize.height });
        await appWindow.setSize(new PhysicalSize(1280, 800));
        await appWindow.setResizable(true);
      } catch (error) {
        console.error('❌ [ScreenShareManager] 调整窗口大小失败:', error);
      }

      setPendingStream(stream);
      setViewingShareId(shareToView.id);

      toast({ title: `正在查看 ${shareToView.playerName} 的屏幕`, status: 'success', duration: 3000, isClosable: true });
    } catch (error: any) {
      console.error('❌ [ScreenShareManager] 查看屏幕失败:', error);
      const errorMessage = error?.message || '查看屏幕失败';
      toast({ title: errorMessage, status: 'error', duration: 3000, isClosable: true });
    }
  };

  // 停止查看屏幕
  const handleStopViewing = async () => {
    if (videoRef.current) {
      videoRef.current.srcObject = null;
    }

    if (viewingShareId) {
      screenShareService.stopViewingScreen(viewingShareId);
    }

    setViewingShareId(null);
    setPendingStream(null);

    if (originalWindowSize) {
      try {
        const appWindow = getCurrentWindow();
        await appWindow.setSize(new PhysicalSize(originalWindowSize.width, originalWindowSize.height));
        await appWindow.setResizable(true);
      } catch (error) {
        console.error('❌ [ScreenShareManager] 恢复窗口大小失败:', error);
      }
      setOriginalWindowSize(null);
    }

    toast({ title: '已停止查看屏幕', status: 'info', duration: 3000, isClosable: true });
  };

  // 获取共享项的样式
  const getShareItemStyle = (isMyShare: boolean, isViewing: boolean, hasPassword: boolean, isBeingViewed: boolean) => {
    let itemBg = cardBg;
    let itemBorder = borderColor;

    if (isMyShare) {
      itemBg = myShareBg;
      itemBorder = myShareBorder;
    }
    if (isViewing) {
      itemBg = viewingBg;
      itemBorder = viewingBorder;
    }
    if (hasPassword) {
      itemBg = hasPasswordBg;
      itemBorder = hasPasswordBorder;
    }
    if (isBeingViewed) {
      itemBg = beingViewedBg;
      itemBorder = beingViewedBorder;
    }

    return { background: itemBg, borderColor: itemBorder };
  };

  return (
    <Box className="screen-share-manager" bg={bg}>
      {/* 全屏视频播放器 */}
      <AnimatePresence>
        {viewingShareId && (
          <motion.div
            className="fullscreen-viewer"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.3 }}
          >
            <div className="viewer-controls-bar">
              <div className="viewer-info-text">
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
                  <line x1="8" y1="21" x2="16" y2="21" />
                  <line x1="12" y1="17" x2="12" y2="21" />
                </svg>
                <span>
                  {activeShares.find(s => s.id === viewingShareId)?.playerName || '未知玩家'} 的屏幕
                </span>
              </div>

              <motion.button
                className="stop-viewing-btn"
                onClick={handleStopViewing}
                whileHover={{ scale: 1.05 }}
                whileTap={{ scale: 0.95 }}
                title="停止查看"
              >
                <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <line x1="18" y1="6" x2="6" y2="18" />
                  <line x1="6" y1="6" x2="18" y2="18" />
                </svg>
              </motion.button>
            </div>

            <video
              ref={videoRef}
              className="fullscreen-video"
              autoPlay
              playsInline
            />
          </motion.div>
        )}
      </AnimatePresence>

      {/* 共享列表 */}
      <div className="screen-share-list">
        {/* 提示信息 */}
        <Flex
          className="screen-share-hint"
          bg={hintBg}
          border="1px solid"
          borderColor={hintBorder}
          color={hintText}
        >
          <Box color={hintColor} flexShrink={0}><InfoIcon size={14} /></Box>
          <Text fontSize="12px">每个屏幕同时仅支持被一名玩家查看</Text>
        </Flex>

        {activeShares.length === 0 ? (
          <Flex direction="column" align="center" justify="center" gap={3} color={emptyColor} textAlign="center" minH="300px">
            <Box opacity={0.3}><ScreenShareIcon size={48} /></Box>
            <Text fontSize="14px" m={0}>当前没有玩家共享屏幕</Text>
            <Text fontSize="12px" opacity={0.7} m={0}>点击"开始共享"按钮分享你的屏幕</Text>
          </Flex>
        ) : (
          <AnimatePresence mode="popLayout">
            {activeShares.map((share) => {
              const isMyShare = share.playerId === currentPlayerId;
              const isViewing = viewingShareId === share.id;
              const hasPassword = share.requirePassword && !isMyShare;
              const isBeingViewed = !!share.viewerId;

              return (
                <motion.div
                  key={share.id}
                  className={`share-item ${isMyShare ? 'my-share' : ''} ${isViewing ? 'viewing' : ''} ${hasPassword ? 'has-password' : ''} ${isBeingViewed ? 'being-viewed' : ''}`}
                  initial={{ opacity: 0, y: 20 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -20 }}
                  transition={{ duration: 0.3 }}
                  style={getShareItemStyle(isMyShare, isViewing, hasPassword, isBeingViewed)}
                >
                  <div className="share-item-content">
                    <div className="share-player-details">
                      <Text fontSize="13px" fontWeight={600} color={textColorStrong} isTruncated>
                        {share.playerName || '未知玩家'}
                        {isMyShare && ' (我)'}
                      </Text>
                      <Text fontSize="11px" color={mutedText}>
                        创建时间: {new Date(share.startTime).toLocaleTimeString()}
                      </Text>
                      {isBeingViewed && (
                        <Text fontSize="11px" color="red.500" fontWeight={500}>
                          正在被 {share.viewerName} 查看
                        </Text>
                      )}
                    </div>

                    <div className="share-badges">
                      {share.requirePassword && (
                        <Tooltip label="需要密码" placement="top" hasArrow>
                          <Flex
                            align="center"
                            justify="center"
                            w="24px"
                            h="24px"
                            borderRadius="4px"
                            bg={passwordBadgeBg}
                            color={passwordBadgeColor}
                            flexShrink={0}
                          >
                            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                              <rect x="5" y="11" width="14" height="10" rx="2" ry="2" />
                              <path d="M7 11V7a5 5 0 0 1 10 0v4" />
                            </svg>
                          </Flex>
                        </Tooltip>
                      )}
                      {isBeingViewed && (
                        <Tooltip label="正在被查看" placement="top" hasArrow>
                          <Flex
                            align="center"
                            justify="center"
                            w="24px"
                            h="24px"
                            borderRadius="4px"
                            bg={viewingBadgeBg}
                            color={viewingBadgeColor}
                            flexShrink={0}
                            className="viewing-badge"
                          >
                            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                              <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
                              <circle cx="12" cy="12" r="3" />
                            </svg>
                          </Flex>
                        </Tooltip>
                      )}
                    </div>
                  </div>

                  {!isBeingViewed && (
                    <Button
                      size="sm"
                      variant="outline"
                      colorScheme="green"
                      leftIcon={
                        isViewing ? (
                          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                            <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/>
                          </svg>
                        ) : (
                          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
                            <circle cx="12" cy="12" r="3" />
                          </svg>
                        )
                      }
                      onClick={() => handleViewScreen(share)}
                      isDisabled={isViewing}
                      flexShrink={0}
                    >
                      {isViewing ? '查看中' : '查看'}
                    </Button>
                  )}
                </motion.div>
              );
            })}
          </AnimatePresence>
        )}
      </div>

      {/* 底部控制栏 */}
      <div className="screen-share-bottom-bar" style={{ background: barBg, borderTop: `1px solid ${barBorder}` }}>
        {!myShareId ? (
          <Button
            colorScheme="green"
            leftIcon={<ScreenShareIcon size={16} />}
            onClick={() => setShowStartModal(true)}
            as={motion.button}
            whileHover={{ scale: 1.05 }}
            whileTap={{ scale: 0.95 }}
          >
            开始共享
          </Button>
        ) : (
          <Button
            colorScheme="red"
            leftIcon={
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <rect x="6" y="6" width="12" height="12" />
              </svg>
            }
            onClick={handleStopSharingInternal}
            as={motion.button}
            whileHover={{ scale: 1.05 }}
            whileTap={{ scale: 0.95 }}
          >
            停止共享
          </Button>
        )}
      </div>

      {/* 开始共享模态框 */}
      <Modal isOpen={showStartModal} onClose={() => { setShowStartModal(false); setPassword(''); setRequirePassword(false); }} isCentered>
        <ModalOverlay />
        <ModalContent>
          <ModalHeader>开始屏幕共享</ModalHeader>
          <ModalCloseButton />
          <ModalBody>
            <VStack gap={4} py={2}>
              <Flex justify="space-between" align="center" w="100%" p={3} borderRadius="8px" bg={modalOptionBg}>
                <Text fontSize="14px" color={textColor}>需要密码才能查看</Text>
                <Switch isChecked={requirePassword} onChange={(e) => setRequirePassword(e.target.checked)} colorScheme="green" />
              </Flex>

              {requirePassword && (
                <motion.div
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: 'auto' }}
                  exit={{ opacity: 0, height: 0 }}
                  style={{ width: '100%', overflow: 'hidden' }}
                >
                  <InputGroup>
                    <Input
                      type={showPassword ? 'text' : 'password'}
                      placeholder="设置查看密码"
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                      maxLength={20}
                    />
                    <InputRightElement>
                      <IconButton
                        aria-label={showPassword ? '隐藏密码' : '显示密码'}
                        icon={<Text fontSize="sm">{showPassword ? '🙈' : '👁'}</Text>}
                        size="sm"
                        variant="ghost"
                        onClick={() => setShowPassword(!showPassword)}
                      />
                    </InputRightElement>
                  </InputGroup>
                </motion.div>
              )}

              <Flex align="center" gap={2} p={3} borderRadius="8px" bg={hintBg} border="1px solid" borderColor={hintBorder} w="100%">
                <Box color={hintColor} flexShrink={0}><InfoIcon size={16} /></Box>
                <Text fontSize="13px" color={hintText}>其他玩家将能够实时查看你的屏幕</Text>
              </Flex>
            </VStack>
          </ModalBody>
          <ModalFooter>
            <Button variant="ghost" mr={3} onClick={() => { setShowStartModal(false); setPassword(''); setRequirePassword(false); }}>取消</Button>
            <Button colorScheme="green" onClick={handleStartSharingInternal}>开始共享</Button>
          </ModalFooter>
        </ModalContent>
      </Modal>

      {/* 密码验证模态框 */}
      <Modal isOpen={showPasswordModal} onClose={() => { setShowPasswordModal(false); setPasswordInput(''); setShowPasswordInput(false); setSelectedShare(null); }} isCentered>
        <ModalOverlay />
        <ModalContent>
          <ModalHeader>输入密码</ModalHeader>
          <ModalCloseButton />
          <ModalBody>
            <VStack gap={4} py={2}>
              <Text fontSize="14px" color={mutedText}>该屏幕共享需要密码才能查看</Text>
              <InputGroup>
                <Input
                  autoFocus
                  type={showPasswordInput ? 'text' : 'password'}
                  placeholder="请输入密码"
                  value={passwordInput}
                  onChange={(e) => setPasswordInput(e.target.value)}
                  onKeyDown={(e) => { if (e.key === 'Enter') handlePasswordSubmit(); }}
                  maxLength={20}
                />
                <InputRightElement>
                  <IconButton
                    aria-label={showPasswordInput ? '隐藏密码' : '显示密码'}
                    icon={<Text fontSize="sm">{showPasswordInput ? '🙈' : '👁'}</Text>}
                    size="sm"
                    variant="ghost"
                    onClick={() => setShowPasswordInput(!showPasswordInput)}
                  />
                </InputRightElement>
              </InputGroup>
            </VStack>
          </ModalBody>
          <ModalFooter>
            <Button variant="ghost" mr={3} onClick={() => { setShowPasswordModal(false); setPasswordInput(''); setShowPasswordInput(false); setSelectedShare(null); }}>取消</Button>
            <Button colorScheme="green" onClick={handlePasswordSubmit}>确认</Button>
          </ModalFooter>
        </ModalContent>
      </Modal>
    </Box>
  );
};
