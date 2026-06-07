import React, { useState, useEffect, useCallback, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Box, VStack, HStack, Flex, Text, Button, IconButton,
  FormControl, FormLabel, Input,
  useColorModeValue, useToast, Tooltip,
  Badge, Spinner,
} from '@chakra-ui/react';
import { Switch as AntSwitch } from 'antd';
import { invoke } from '@tauri-apps/api/core';
import { useEscapeKey } from '../../hooks';

import { GlobalAdvancedConfigPanel } from '../GlobalAdvancedConfigPanel/GlobalAdvancedConfigPanel';
import './SettingsWindow.css';

export const SettingsWindow: React.FC<{ onClose: () => void }> = ({ onClose }) => {
  const [loading, setLoading] = useState(true);
  const [usePrivateServer, setUsePrivateServer] = useState(false);
  // 私有服务器输入框状态（受控组件）
  const [privateEasytierServer, setPrivateEasytierServer] = useState('wss://mctiers.pmhs.top');
  const [privateSignalingServer, setPrivateSignalingServer] = useState('wss://mctier.pmhs.top/signaling');
  // 用ref保存完整设置
  const settingsRef = useRef<Record<string, any>>({});
  const toast = useToast();

  // 主题色变量
  const cardBg = useColorModeValue('white', 'rgba(255,255,255,0.04)');
  const cardBorder = useColorModeValue('gray.200', 'rgba(255,255,255,0.08)');
  const textColor = useColorModeValue('gray.800', 'rgba(255,255,255,0.9)');
  const mutedTextColor = useColorModeValue('gray.500', 'rgba(255,255,255,0.5)');
  const labelColor = useColorModeValue('gray.700', 'rgba(255,255,255,0.85)');
  const descColor = useColorModeValue('gray.400', 'rgba(255,255,255,0.38)');
  const inputBg = useColorModeValue('gray.100', 'rgba(255,255,255,0.06)');
  const subFormBg = useColorModeValue('gray.50', 'rgba(0,0,0,0.2)');
  const toggleHoverBg = useColorModeValue('gray.50', 'rgba(255,255,255,0.03)');
  const cardHoverBorder = useColorModeValue('gray.300', 'rgba(126,211,33,0.15)');
  const headerBorder = useColorModeValue('gray.200', 'rgba(126,211,33,0.12)');
const titleColor = useColorModeValue('gray.800', 'rgba(255,255,255,0.92)');
const windowBg = useColorModeValue(
    'linear-gradient(150deg, #f5f7fa 0%, #fafbfc 50%, #f0f2f5 100%)',
    'linear-gradient(150deg, #111827 0%, #0f1a2e 50%, #141a14 100%)'
  );

  // ===== 主题感知颜色变量（替换硬编码 white） =====
  const cardHeaderBorder = useColorModeValue('gray.200', 'rgba(255,255,255,0.06)');
  const closeBtnColor = useColorModeValue('gray.500', 'rgba(255,255,255,0.5)');
  const subformBorder = useColorModeValue('gray.200', 'rgba(255,255,255,0.05)');
  const inputBorderLight = useColorModeValue('gray.300', 'rgba(255,255,255,0.1)');
  const placeholderLight = useColorModeValue('gray.400', 'rgba(255,255,255,0.28)');
  const inputHoverBg = useColorModeValue('gray.100', 'rgba(255,255,255,0.08)');
  const inputFocusBg = useColorModeValue('gray.50', 'rgba(255,255,255,0.09)');

  useEscapeKey(onClose, true);

  const showToast = (title: string, status: 'success' | 'error' | 'info' | 'warning', duration = 2000) => {
    toast({ title, status, duration, isClosable: true, position: 'top' });
  };

  // 提取加载设置的逻辑为独立函数
  const loadSettings = useCallback(async () => {
    const timeoutId = setTimeout(() => {
      console.error('加载设置超时');
      showToast('加载设置超时，请重试', 'error');
      setLoading(false);
    }, 5000);

    try {
      console.log('开始加载设置...');
      const settings = await invoke<any>('get_settings');
      console.log('设置加载成功:', settings);
      clearTimeout(timeoutId);

      const ups = settings.usePrivateServer || false;
      setUsePrivateServer(ups);
      setPrivateEasytierServer(settings.privateEasytierServer ?? 'wss://mctiers.pmhs.top');
      setPrivateSignalingServer(settings.privateSignalingServer ?? 'wss://mctier.pmhs.top/signaling');

      settingsRef.current = {
        lobbyName: settings.lobbyName || '',
        lobbyPassword: settings.lobbyPassword || '',
        playerName: settings.playerName || '',
        usePrivateServer: ups,
        privateEasytierServer: settings.privateEasytierServer ?? 'wss://mctiers.pmhs.top',
        privateSignalingServer: settings.privateSignalingServer ?? 'wss://mctier.pmhs.top/signaling',
        enableExitNode: settings.enableExitNode || false,
        enableAsExitNode: settings.enableAsExitNode || false,
        proxyCidrs: settings.proxyCidrs || '',
        exitNodes: settings.exitNodes || '',
        subnetProxyCidrs: settings.subnetProxyCidrs || '',
      };
    } catch (e) {
      clearTimeout(timeoutId);
      console.error('加载设置失败:', e);
      showToast('加载设置失败，将使用默认配置', 'error', 3000);

      settingsRef.current = {
        lobbyName: '',
        lobbyPassword: '',
        playerName: '',
        usePrivateServer: false,
        privateEasytierServer: 'wss://mctiers.pmhs.top',
        privateSignalingServer: 'wss://mctier.pmhs.top/signaling',
        enableExitNode: false,
        enableAsExitNode: false,
        proxyCidrs: '',
        exitNodes: '',
        subnetProxyCidrs: '',
      };
      setUsePrivateServer(false);
      setPrivateEasytierServer('wss://mctiers.pmhs.top');
      setPrivateSignalingServer('wss://mctier.pmhs.top/signaling');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  // 监听配置导入事件
  useEffect(() => {
    const handleConfigImported = () => {
      console.log('检测到配置导入，重新加载设置...');
      loadSettings();
    };
    window.addEventListener('configImported', handleConfigImported);
    return () => window.removeEventListener('configImported', handleConfigImported);
  }, [loadSettings]);

  // 保存设置
  const saveAll = useCallback(async (patch?: Record<string, any>) => {
    const merged = { ...settingsRef.current, ...patch };

    // 同步 Switch 输入框状态
    if (patch?.usePrivateServer !== undefined) setUsePrivateServer(patch.usePrivateServer);
    if (patch?.privateEasytierServer !== undefined) setPrivateEasytierServer(patch.privateEasytierServer);
    if (patch?.privateSignalingServer !== undefined) setPrivateSignalingServer(patch.privateSignalingServer);

    // 合并当前输入框的值
    merged.privateEasytierServer = privateEasytierServer;
    merged.privateSignalingServer = privateSignalingServer;

    settingsRef.current = merged;

    try {
      await invoke('save_settings', {
        lobbyName: merged.lobbyName || null,
        lobbyPassword: merged.lobbyPassword || null,
        playerName: merged.playerName || null,
        usePrivateServer: merged.usePrivateServer ?? false,
        privateEasytierServer: merged.privateEasytierServer?.trim() || null,
        privateSignalingServer: merged.privateSignalingServer?.trim() || null,
        enableExitNode: merged.enableExitNode !== undefined ? merged.enableExitNode : null,
        enableAsExitNode: merged.enableAsExitNode !== undefined ? merged.enableAsExitNode : null,
        proxyCidrs: merged.proxyCidrs?.trim() || null,
        exitNodes: merged.exitNodes?.trim() || null,
        subnetProxyCidrs: merged.subnetProxyCidrs?.trim() || null,
      });
      console.log('设置已保存:', merged);
      showToast('已保存', 'success', 1000);
    } catch (e) {
      console.error('保存设置失败:', e);
      showToast('保存失败', 'error');
    }
  }, [privateEasytierServer, privateSignalingServer]);

  const containerVariants = {
    hidden: { opacity: 0 },
    visible: { opacity: 1, transition: { staggerChildren: 0.07, delayChildren: 0.05 } },
  };
  const itemVariants = {
    hidden: { opacity: 0, y: 18 },
    visible: { opacity: 1, y: 0, transition: { duration: 0.35, ease: [0.4, 0, 0.2, 1] } },
  };

  // ===== 卡片头部渲染 =====
  const CardHeader = ({ icon, title, color = 'green' }: { icon: React.ReactNode; title: string; color?: string }) => {
    const iconColors: Record<string, { bg: string; color: string; darkBg: string; darkColor: string }> = {
      green: { bg: 'rgba(106, 164, 56, 0.15)', color: 'rgba(126, 211, 33, 0.9)', darkBg: 'rgba(106, 164, 56, 0.15)', darkColor: 'rgba(126, 211, 33, 0.9)' },
      blue: { bg: 'rgba(139, 111, 71, 0.2)', color: 'rgba(255, 215, 0, 0.95)', darkBg: 'rgba(139, 111, 71, 0.2)', darkColor: 'rgba(255, 215, 0, 0.95)' },
      cyan: { bg: 'rgba(0, 188, 212, 0.15)', color: 'rgba(0, 229, 255, 0.9)', darkBg: 'rgba(0, 188, 212, 0.2)', darkColor: 'rgba(0, 229, 255, 0.9)' },
      purple: { bg: 'rgba(156, 39, 176, 0.2)', color: 'rgba(186, 104, 200, 0.95)', darkBg: 'rgba(156, 39, 176, 0.2)', darkColor: 'rgba(186, 104, 200, 0.95)' },
    };
    const ic = iconColors[color] || iconColors.green;
    const iconBg = useColorModeValue(ic.bg, ic.darkBg);
    const iconColor = useColorModeValue(ic.color, ic.darkColor);

    return (
      <HStack gap={2} px={3.5} py={2.5} borderBottom="1px solid" borderColor={cardHeaderBorder}>
        <Flex w="24px" h="24px" borderRadius="6px" bg={iconBg} color={iconColor} align="center" justify="center" flexShrink={0}>
          {icon}
        </Flex>
        <Text fontSize="xs" fontWeight="semibold" textTransform="uppercase" letterSpacing="0.8px" color={mutedTextColor}>
          {title}
        </Text>
      </HStack>
    );
  };

  if (loading) {
    return (
      <Box w="100%" h="100%" className="settings-window" bg={windowBg} position="relative" overflow="hidden">
        <Box className="settings-drag-area" data-tauri-drag-region />
        <Flex flex={1} align="center" justify="center">
          <Spinner size="lg" color="rgba(126, 211, 33, 0.8)" thickness="3px" />
        </Flex>
      </Box>
    );
  }

  return (
    <Box w="100%" h="100%" className="settings-window" position="relative" overflow="hidden"
    bg={windowBg}
  >
      <Box className="settings-drag-area" h="28px" flexShrink={0} data-tauri-drag-region />
      {/* 装饰背景光球 */}
      <Box className="settings-bg-orb settings-bg-orb-1" />
      <Box className="settings-bg-orb settings-bg-orb-2" />

      <Box className="settings-window-scroll" flex={1} overflowY="auto" overflowX="hidden" position="relative" zIndex={1}>
        <motion.div className="settings-window-inner" variants={containerVariants} initial="hidden" animate="visible"
          style={{ padding: '4px 16px 20px', display: 'flex', flexDirection: 'column', gap: '16px' }}
        >
          {/* 头部 */}
          <motion.div variants={itemVariants}>
            <HStack justify="space-between" py={2.5} pb={1.5} borderBottom="1px solid" borderColor={headerBorder} mb={0.5}>
              <HStack gap={2}>
                <Flex w="30px" h="30px" borderRadius="8px" bg="rgba(126, 211, 33, 0.1)" border="1px solid rgba(126, 211, 33, 0.2)" align="center" justify="center" flexShrink={0}>
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="rgba(126,211,33,0.95)">
                    <path d="M12 15.5A3.5 3.5 0 0 1 8.5 12 3.5 3.5 0 0 1 12 8.5a3.5 3.5 0 0 1 3.5 3.5 3.5 3.5 0 0 1-3.5 3.5m7.43-2.92c.04-.34.07-.69.07-1.08s-.03-.74-.07-1.08l2.32-1.82c.21-.17.27-.46.13-.7l-2.2-3.81c-.13-.24-.41-.32-.65-.24l-2.74 1.1c-.57-.44-1.18-.81-1.86-1.09L14.05 2.1c-.04-.27-.28-.46-.55-.46h-3c-.28 0-.5.19-.55.46L9.5 4.86C8.82 5.14 8.2 5.5 7.64 5.95L4.9 4.85c-.24-.09-.52 0-.65.24L2.05 8.9c-.14.24-.08.53.13.7L4.5 11.5c-.04.34-.07.7-.07 1.08s.03.74.07 1.08L2.18 15.48c-.21.17-.27.46-.13.7l2.2 3.81c.13.24.41.32.65.24l2.74-1.1c.57.44 1.18.81 1.86 1.09l.45 2.76c.05.27.27.46.55.46h3c.28 0 .5-.19.55-.46l.45-2.76c.68-.28 1.3-.65 1.86-1.09l2.74 1.1c.24.09.52 0 .65-.24l2.2-3.81c.14-.24.08-.53-.13-.7l-2.32-1.9z" />
                  </svg>
                </Flex>
                <Text fontSize="md" fontWeight="bold" color={titleColor} letterSpacing="0.5px">
                  设置
                </Text>
              </HStack>
              <motion.div
                whileHover={{ scale: 1.15, rotate: 90 }}
                whileTap={{ scale: 0.9 }}
                transition={{ type: 'spring', stiffness: 400, damping: 17 }}
                style={{ display: 'inline-flex' }}
              >
                <IconButton
                  aria-label="关闭设置"
                  icon={
                    <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
                      <line x1="1" y1="1" x2="13" y2="13" />
                      <line x1="13" y1="1" x2="1" y2="13" />
                    </svg>
                  }
                  size="sm"
                  variant="ghost"
                  color={closeBtnColor}
                  onClick={onClose}
                  _hover={{ bg: 'rgba(220,60,60,0.2)', color: 'rgba(255,120,120,0.9)' }}
                  transition="none"
                />
              </motion.div>
            </HStack>
          </motion.div>

          {/* Card 2: 私有服务器 */}
          <motion.div variants={itemVariants}>
            <Box bg={cardBg} border="1px solid" borderColor={cardBorder} borderRadius="10px" overflow="hidden"
              _hover={{ borderColor: cardHoverBorder }}
              transition="border-color 0.25s"
            >
              <CardHeader
                icon={
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z" />
                  </svg>
                }
                title="私有服务器"
                color="blue"
              />
              <HStack justify="space-between" px={3.5} py={2.5} gap={3}
                _hover={{ bg: toggleHoverBg }}
                transition="background 0.2s"
              >
                <VStack align="flex-start" gap={0.5} flex={1} minW={0}>
                  <Text fontSize="sm" fontWeight="medium" color={textColor} whiteSpace="nowrap">
                    使用私有服务器
                  </Text>
                  <Text fontSize="xs" color={descColor} whiteSpace="nowrap" overflow="hidden" textOverflow="ellipsis">
                    启用后可配置自己部署的服务器
                  </Text>
                </VStack>
                <Flex align="center" gap={1.5}>
                  <Tooltip
                    label="MCTier 官网提供后端源码与私有化部署教学，您可以自行搭建私有服务器"
                    placement="right"
                    bg="rgba(0,0,0,0.85)"
                    color="white"
                    borderRadius="md"
                    p={2}
                    fontSize="xs"
                  >
                    <Flex w="18px" h="18px" borderRadius="full" bg="rgba(255,255,255,0.08)" align="center" justify="center" cursor="help"
                      _hover={{ bg: 'rgba(255,255,255,0.15)', transform: 'scale(1.1)' }}
                      transition="all 0.2s"
                    >
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="rgba(255,255,255,0.6)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                        <circle cx="12" cy="12" r="10"></circle>
                        <line x1="12" y1="16" x2="12" y2="12"></line>
                        <line x1="12" y1="8" x2="12.01" y2="8"></line>
                      </svg>
                    </Flex>
                  </Tooltip>
                  <AntSwitch
                    checked={usePrivateServer}
                    onChange={async (checked) => {
                      setUsePrivateServer(checked);
                      await saveAll({ usePrivateServer: checked });
                    }}
                  />
                </Flex>
              </HStack>
              <AnimatePresence>
                {usePrivateServer && (
                  <motion.div
                    initial={{ opacity: 0, height: 0 }}
                    animate={{ opacity: 1, height: 'auto' }}
                    exit={{ opacity: 0, height: 0 }}
                    transition={{ duration: 0.3 }}
                    style={{ overflow: 'hidden' }}
                  >
                    <VStack spacing={3} p={3.5} bg={subFormBg} borderTop="1px solid" borderColor={subformBorder}>
                      <FormControl>
                        <FormLabel fontSize="xs" color={mutedTextColor} mb={1} fontWeight="medium">
                          EasyTier 节点服务器
                        </FormLabel>
                        <Input
                          value={privateEasytierServer}
                          onChange={(e) => setPrivateEasytierServer(e.target.value)}
                          onBlur={() => saveAll()}
                          placeholder="wss://mctiers.pmhs.top"
                          size="sm"
                          bg={inputBg}
                          border="1px solid"
                          borderColor={inputBorderLight}
                          color={textColor}
                          borderRadius="7px"
                          fontSize="sm"
                          _placeholder={{ color: placeholderLight }}
                          _hover={{ borderColor: 'rgba(126,211,33,0.3)', bg: inputHoverBg }}
                          _focus={{ borderColor: 'rgba(126,211,33,0.55)', boxShadow: '0 0 0 2px rgba(126,211,33,0.12)', bg: inputFocusBg }}
                        />
                      </FormControl>
                      <FormControl>
                        <FormLabel fontSize="xs" color={mutedTextColor} mb={1} fontWeight="medium">
                          WebRTC 信令服务器
                        </FormLabel>
                        <Input
                          value={privateSignalingServer}
                          onChange={(e) => setPrivateSignalingServer(e.target.value)}
                          onBlur={() => saveAll()}
                          placeholder="wss://mctier.pmhs.top/signaling"
                          size="sm"
                          bg={inputBg}
                          border="1px solid"
                          borderColor={inputBorderLight}
                          color={textColor}
                          borderRadius="7px"
                          fontSize="sm"
                          _placeholder={{ color: placeholderLight }}
                          _hover={{ borderColor: 'rgba(126,211,33,0.3)', bg: inputHoverBg }}
                          _focus={{ borderColor: 'rgba(126,211,33,0.55)', boxShadow: '0 0 0 2px rgba(126,211,33,0.12)', bg: inputFocusBg }}
                        />
                      </FormControl>
                    </VStack>
                  </motion.div>
                )}
              </AnimatePresence>
            </Box>
          </motion.div>

          {/* Card 3: 自定义 EasyTier 节点 */}
          <motion.div variants={itemVariants}>
            <Box bg={cardBg} border="1px solid" borderColor={cardBorder} borderRadius="10px" overflow="hidden"
              _hover={{ borderColor: cardHoverBorder }}
              transition="border-color 0.25s"
            >
              <CardHeader
                icon={
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z" />
                    <path d="M12 10h-2v2H9v-2H7V9h2V7h1v2h2v1z" />
                  </svg>
                }
                title="自定义 EasyTier 节点"
                color="cyan"
              />
              <Box px={3.5} pt={2} pb={0}>
                <Text fontSize="sm" color={mutedTextColor} lineHeight="1.5" mb={1}>
                  配置自定义 EasyTier 节点，可在创建/加入大厅时选择使用
                </Text>
                <Text fontSize="xs" color={descColor} lineHeight="1.6">
                  • 在创建/加入大厅界面的服务器下拉列表中选择节点<br />
                  • 每次组网只使用一个选定的节点<br />
                  • 可添加多个备用节点供选择使用
                </Text>
              </Box>
              <CustomNodeManager />
            </Box>
          </motion.div>

          {/* Card 4: 全局 EasyTier 高级配置 */}
          <motion.div variants={itemVariants}>
            <Box bg={cardBg} border="1px solid" borderColor={cardBorder} borderRadius="10px" overflow="hidden"
              _hover={{ borderColor: cardHoverBorder }}
              transition="border-color 0.25s"
            >
              <CardHeader
                icon={
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M12 15.5A3.5 3.5 0 0 1 8.5 12 3.5 3.5 0 0 1 12 8.5a3.5 3.5 0 0 1 3.5 3.5 3.5 3.5 0 0 1-3.5 3.5m7.43-2.92c.04-.34.07-.69.07-1.08s-.03-.74-.07-1.08l2.32-1.82c.21-.17.27-.46.13-.70l-2.2-3.81c-.13-.24-.41-.32-.65-.24l-2.74 1.1c-.57-.44-1.18-.81-1.86-1.09L14.05 2.1c-.04-.27-.28-.46-.55-.46h-3c-.28 0-.5.19-.55.46L9.5 4.86C8.82 5.14 8.2 5.5 7.64 5.95L4.9 4.85c-.24-.09-.52 0-.65.24L2.05 8.9c-.14.24-.08.53.13.70L4.5 11.5c-.04.34-.07.7-.07 1.08s.03.74.07 1.08L2.18 15.48c-.21.17-.27.46-.13.70l2.2 3.81c.13.24.41.32.65.24l2.74-1.1c.57.44 1.18.81 1.86 1.09l.45 2.76c.05.27.27.46.55.46h3c.28 0 .5-.19.55-.46l.45-2.76c.68-.28 1.3-.65 1.86-1.09l2.74 1.1c.24.09.52 0 .65-.24l2.2-3.81c.14-.24.08-.53-.13-.70l-2.32-1.9z" />
                  </svg>
                }
                title="全局 EasyTier 高级配置"
                color="green"
              />
              <Box px={3.5} pt={2} pb={0}>
                <Text fontSize="sm" color={mutedTextColor} lineHeight="1.5" mb={1}>
                  配置 EasyTier 的高级参数，这些配置将作为默认配置应用于所有大厅
                </Text>
              </Box>
              <GlobalAdvancedConfigPanel />
            </Box>
          </motion.div>

          {/* Card 5: 配置管理 */}
          <motion.div variants={itemVariants}>
            <Box bg={cardBg} border="1px solid" borderColor={cardBorder} borderRadius="10px" overflow="hidden"
              _hover={{ borderColor: cardHoverBorder }}
              transition="border-color 0.25s"
            >
              <CardHeader
                icon={
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M14 2H6c-1.1 0-1.99.9-1.99 2L4 20c0 1.1.89 2 1.99 2H18c1.1 0 2-.9 2-2V8l-6-6zm2 16H8v-2h8v2zm0-4H8v-2h8v2zm-3-5V3.5L18.5 9H13z" />
                  </svg>
                }
                title="配置管理"
                color="purple"
              />
              <Box px={3.5} pt={2} pb={0}>
                <Text fontSize="sm" color={mutedTextColor} lineHeight="1.5" mb={1}>
                  导出或导入所有配置项，方便备份和迁移
                </Text>
              </Box>
              <ConfigManager />
            </Box>
          </motion.div>
        </motion.div>
      </Box>
    </Box>
  );
};

// ===== 自定义节点管理组件 =====
interface EasyTierNode {
  name: string;
  address: string;
}

const DEFAULT_BUILTIN_NODE: EasyTierNode = {
  name: '明月清风节点',
  address: 'wss://qtet-public.070219.xyz'
};

const CustomNodeManager: React.FC = () => {
  const [nodes, setNodes] = useState<EasyTierNode[]>([DEFAULT_BUILTIN_NODE]);
  const [loading, setLoading] = useState(true);
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [editForm, setEditForm] = useState<EasyTierNode>({ name: '', address: '' });
  const toast = useToast();

  const textColor = useColorModeValue('gray.800', 'rgba(255,255,255,0.9)');
  const mutedTextColor = useColorModeValue('gray.500', 'rgba(255,255,255,0.6)');
  const inputBg = useColorModeValue('gray.100', 'rgba(255,255,255,0.06)');
  const cardBg = useColorModeValue('white', 'rgba(255,255,255,0.04)');
  const cardBorder = useColorModeValue('gray.200', 'rgba(255,255,255,0.08)');

  // 自定义节点组件主题变量
  const nodeInputBorder = useColorModeValue('gray.300', 'rgba(255,255,255,0.1)');
  const nodePlaceholder = useColorModeValue('gray.400', 'rgba(255,255,255,0.3)');
  const addNodeBorder = useColorModeValue('gray.300', 'rgba(255,255,255,0.2)');
  const addNodeBg = useColorModeValue('gray.50', 'rgba(255,255,255,0.03)');
  const addNodeText = useColorModeValue('gray.600', 'rgba(255,255,255,0.7)');
  const addNodeHoverBorder = useColorModeValue('gray.400', 'rgba(126,211,33,0.5)');
  const addNodeHoverBg = useColorModeValue('rgba(126,211,33,0.08)', 'rgba(126,211,33,0.1)');
  const addNodeHoverText = useColorModeValue('rgba(126,211,33,0.8)', 'rgba(126,211,33,0.9)');

  const showToast = (title: string, status: 'success' | 'error' | 'info' | 'warning') => {
    toast({ title, status, duration: 2000, isClosable: true, position: 'top' });
  };

  const loadNodes = useCallback(async () => {
    try {
      const settings = await invoke<any>('get_settings');
      const customNodes = settings.customEasytierNodes || [];
      setNodes([DEFAULT_BUILTIN_NODE, ...customNodes]);
    } catch (error) {
      console.error('加载节点列表失败:', error);
      showToast('加载节点列表失败', 'error');
      setNodes([DEFAULT_BUILTIN_NODE]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadNodes(); }, [loadNodes]);

  useEffect(() => {
    const handleConfigImported = () => loadNodes();
    window.addEventListener('configImported', handleConfigImported);
    return () => window.removeEventListener('configImported', handleConfigImported);
  }, [loadNodes]);

  const saveNodes = async (newNodes: EasyTierNode[]) => {
    try {
      const customNodesOnly = newNodes.filter(node => node.address !== DEFAULT_BUILTIN_NODE.address);
      await invoke('save_settings', {
        autoStartup: false, autoLobbyEnabled: false,
        lobbyName: null, lobbyPassword: null, playerName: null,
        useDomain: false, virtualDomain: null,
        usePrivateServer: false, privateEasytierServer: null, privateSignalingServer: null,
        alwaysOnTop: null, rememberWindowPosition: null,
        customEasytierNodes: customNodesOnly,
        voiceVolume: null,
      });
      setNodes(newNodes);
      showToast('节点列表已保存', 'success');
    } catch (error) {
      console.error('保存节点列表失败:', error);
      showToast('保存节点列表失败', 'error');
    }
  };

  const handleAdd = () => {
    setNodes([...nodes, { name: '', address: '' }]);
    setEditingIndex(nodes.length);
    setEditForm({ name: '', address: '' });
  };

  const handleEdit = (index: number) => {
    setEditingIndex(index);
    setEditForm({ ...nodes[index] });
  };

  const handleSave = async () => {
    if (!editForm.name.trim()) { showToast('请输入节点名称', 'error'); return; }
    if (!editForm.address.trim()) { showToast('请输入节点地址', 'error'); return; }
    if (!/^(tcp|udp|ws|wss|txt):\/\/.+$/.test(editForm.address.trim())) {
      showToast('节点地址格式错误，应以 tcp://、udp://、ws://、wss:// 或 txt:// 开头', 'error');
      return;
    }
    const newNodes = [...nodes];
    if (editingIndex !== null) {
      if (editingIndex >= newNodes.length) newNodes.push(editForm);
      else newNodes[editingIndex] = editForm;
      await saveNodes(newNodes);
      setEditingIndex(null);
      setEditForm({ name: '', address: '' });
    }
  };

  const handleCancelEdit = () => {
    // 如果正在添加新节点（末尾占位），移除占位
    if (editingIndex !== null && editingIndex >= nodes.length - 1) {
      setNodes(nodes.slice(0, -1));
    }
    setEditingIndex(null);
    setEditForm({ name: '', address: '' });
  };

  const handleDelete = async (index: number) => {
    if (index === 0) { showToast('默认备用节点不可删除', 'info'); return; }
    const newNodes = nodes.filter((_, i) => i !== index);
    await saveNodes(newNodes);
  };

  if (loading) {
    return <Box p={4} textAlign="center" color={mutedTextColor} fontSize="sm">加载中...</Box>;
  }

  return (
    <Box px={3.5} pb={3.5}>
      <VStack spacing={3}>
        {nodes.map((node, index) => (
          <motion.div
            key={index}
            initial={{ opacity: 0, y: -10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.2 }}
            style={{ width: '100%' }}
          >
            <Box
              bg={cardBg}
              border="1px solid"
              borderColor={cardBorder}
              borderRadius="8px"
              p={3}
            >
              {editingIndex === index ? (
                <VStack spacing={2}>
                  <Input
                    placeholder="节点名称"
                    value={editForm.name}
                    onChange={(e) => setEditForm({ ...editForm, name: e.target.value })}
                    maxLength={32}
                    size="sm"
                    bg={inputBg}
                    border="1px solid"
                    borderColor={nodeInputBorder}
                    color={textColor}
                    borderRadius="6px"
                    _placeholder={{ color: nodePlaceholder }}
                  />
                  <Input
                    placeholder="节点地址 (例如: wss://example.com)"
                    value={editForm.address}
                    onChange={(e) => setEditForm({ ...editForm, address: e.target.value })}
                    size="sm"
                    bg={inputBg}
                    border="1px solid"
                    borderColor={nodeInputBorder}
                    color={textColor}
                    borderRadius="6px"
                    _placeholder={{ color: nodePlaceholder }}
                  />
                  <HStack gap={2} w="full">
                    <Button size="sm" flex={1} onClick={handleSave}
                      bg="linear-gradient(135deg, #11998e 0%, #38ef7d 100%)"
                      color="white"
                      fontWeight="semibold"
                      borderRadius="6px"
                      _hover={{ bg: 'linear-gradient(135deg, #0d8074 0%, #2dd46b 100%)' }}
                      as={motion.button}
                      whileHover={{ scale: 1.02 }}
                      whileTap={{ scale: 0.98 }}
                    >
                      保存
                    </Button>
                    <Button size="sm" flex={1} onClick={handleCancelEdit}
                      bg="rgba(255,255,255,0.1)"
                      color={textColor}
                      fontWeight="semibold"
                      borderRadius="6px"
                      _hover={{ bg: 'rgba(255,255,255,0.15)' }}
                      as={motion.button}
                      whileHover={{ scale: 1.02 }}
                      whileTap={{ scale: 0.98 }}
                    >
                      取消
                    </Button>
                  </HStack>
                </VStack>
              ) : (
                <HStack gap={3} align="center">
                  <VStack align="flex-start" flex={1} minW={0} spacing={0.5}>
                    <HStack gap={1.5}>
                      <Text fontSize="sm" fontWeight="semibold" color={textColor} noOfLines={1}>
                        {node.name}
                      </Text>
                      {index === 0 && (
                        <Badge
                          bg="linear-gradient(135deg, rgba(0, 229, 255, 0.2), rgba(0, 188, 212, 0.2))"
                          border="1px solid rgba(0, 229, 255, 0.4)"
                          borderRadius="4px"
                          fontSize="10px"
                          fontWeight="semibold"
                          color="rgba(0, 229, 255, 0.9)"
                          letterSpacing="0.5px"
                          px={1.5}
                          py={0.5}
                        >
                          内置
                        </Badge>
                      )}
                    </HStack>
                    <Text fontSize="xs" color={mutedTextColor} fontFamily="'Consolas', 'Monaco', monospace" noOfLines={1}>
                      {node.address}
                    </Text>
                  </VStack>
                  <HStack gap={2}>
                    {index !== 0 && (
                      <>
                        <IconButton
                          aria-label="编辑节点"
                          icon={
                            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                              <path d="M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34c-.39-.39-1.02-.39-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z"/>
                            </svg>
                          }
                          size="sm"
                          variant="ghost"
                          bg="rgba(24, 144, 255, 0.15)"
                          color="#1890ff"
                          borderRadius="6px"
                          _hover={{ bg: 'rgba(24, 144, 255, 0.25)' }}
                          onClick={() => handleEdit(index)}
                          as={motion.button}
                          whileHover={{ scale: 1.1 }}
                          whileTap={{ scale: 0.9 }}
                        />
                        <IconButton
                          aria-label="删除节点"
                          icon={
                            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                              <path d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"/>
                            </svg>
                          }
                          size="sm"
                          variant="ghost"
                          bg="rgba(255, 77, 79, 0.15)"
                          color="#ff4d4f"
                          borderRadius="6px"
                          _hover={{ bg: 'rgba(255, 77, 79, 0.25)' }}
                          onClick={() => handleDelete(index)}
                          as={motion.button}
                          whileHover={{ scale: 1.1 }}
                          whileTap={{ scale: 0.9 }}
                        />
                      </>
                    )}
                  </HStack>
                </HStack>
              )}
            </Box>
          </motion.div>
        ))}

        {/* 添加节点按钮 */}
        {editingIndex === null && (
          <Button
            w="full"
            h="44px"
            border="2px dashed"
            borderColor={addNodeBorder}
            bg={addNodeBg}
            color={addNodeText}
            fontSize="sm"
            fontWeight="semibold"
            borderRadius="8px"
            display="flex"
            alignItems="center"
            justifyContent="center"
            gap={2}
            _hover={{
              borderColor: addNodeHoverBorder,
              bg: addNodeHoverBg,
              color: addNodeHoverText,
            }}
            onClick={handleAdd}
            as={motion.button}
            whileHover={{ scale: 1.02 }}
            whileTap={{ scale: 0.98 }}
            leftIcon={
              <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                <path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z"/>
              </svg>
            }
          >
            添加节点
          </Button>
        )}
      </VStack>
    </Box>
  );
};

// ===== 配置管理组件 =====
const ConfigManager: React.FC = () => {
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState(false);

  const handleExport = async () => {
    try {
      setExporting(true);
      const defaultFileName = `mctier_config_${new Date().toISOString().slice(0, 10)}.json`;
      try {
        const filePath = await invoke<string | null>('select_save_location', { defaultName: defaultFileName });
        if (!filePath) { setExporting(false); return; }
        await invoke('export_config', { exportPath: filePath });
      } catch (error) {
        console.error('导出配置失败:', error);
      }
    } catch (error) {
      console.error('导出配置失败:', error);
    } finally {
      setExporting(false);
    }
  };

  const handleImport = async () => {
    try {
      setImporting(true);
      try {
        const filePath = await invoke<string | null>('select_file');
        if (!filePath) { setImporting(false); return; }
        await invoke('import_config', { importPath: filePath });
        window.dispatchEvent(new CustomEvent('configImported'));
      } catch (error) {
        console.error('导入配置失败:', error);
      }
    } catch (error) {
      console.error('导入配置失败:', error);
    } finally {
      setImporting(false);
    }
  };

  return (
    <Box px={3.5} pb={3.5}>
      <HStack gap={3}>
        <Button
          flex={1}
          display="flex"
          alignItems="center"
          justifyContent="center"
          gap={2}
          py={3}
          px={4}
          bg="linear-gradient(135deg, rgba(90, 148, 40, 0.15) 0%, rgba(90, 148, 40, 0.08) 100%)"
          border="1.5px solid rgba(90, 148, 40, 0.3)"
          borderRadius="8px"
          color="rgba(150, 200, 100, 0.95)"
          fontSize="sm"
          fontWeight="semibold"
          isDisabled={exporting}
          _hover={!exporting ? { transform: 'translateY(-1px)', boxShadow: '0 4px 12px rgba(90, 148, 40, 0.2)', borderColor: 'rgba(90, 148, 40, 0.4)' } : {}}
          _active={{ transform: 'scale(0.98)' }}
          onClick={handleExport}
          as={motion.button}
          whileHover={!exporting ? { scale: 1.02 } : {}}
          whileTap={{ scale: 0.98 }}
          leftIcon={
            exporting ? (
              <Spinner size="sm" speed="0.6s" color="currentColor" />
            ) : (
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                <polyline points="7 10 12 15 17 10"></polyline>
                <line x1="12" y1="15" x2="12" y2="3"></line>
              </svg>
            )
          }
        >
          {exporting ? '导出中...' : '导出配置'}
        </Button>
        <Button
          flex={1}
          display="flex"
          alignItems="center"
          justifyContent="center"
          gap={2}
          py={3}
          px={4}
          bg="linear-gradient(135deg, rgba(0, 188, 212, 0.15) 0%, rgba(0, 188, 212, 0.08) 100%)"
          border="1.5px solid rgba(0, 188, 212, 0.3)"
          borderRadius="8px"
          color="rgba(0, 229, 255, 0.9)"
          fontSize="sm"
          fontWeight="semibold"
          isDisabled={importing}
          _hover={!importing ? { transform: 'translateY(-1px)', boxShadow: '0 4px 12px rgba(0, 188, 212, 0.2)', borderColor: 'rgba(0, 188, 212, 0.4)' } : {}}
          _active={{ transform: 'scale(0.98)' }}
          onClick={handleImport}
          as={motion.button}
          whileHover={!importing ? { scale: 1.02 } : {}}
          whileTap={{ scale: 0.98 }}
          leftIcon={
            importing ? (
              <Spinner size="sm" speed="0.6s" color="currentColor" />
            ) : (
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                <polyline points="17 8 12 3 7 8"></polyline>
                <line x1="12" y1="3" x2="12" y2="15"></line>
              </svg>
            )
          }
        >
          {importing ? '导入中...' : '导入配置'}
        </Button>
      </HStack>
    </Box>
  );
};