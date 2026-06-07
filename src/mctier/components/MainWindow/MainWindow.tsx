import React, { useState, useEffect } from 'react';
import { Box, Flex, Text, useColorModeValue, useColorMode } from '@chakra-ui/react';
import { motion, AnimatePresence } from 'framer-motion';
import { Modal, ConfigProvider, theme as antdTheme, App as AntdApp } from 'antd';
import zhCN from 'antd/locale/zh_CN';
import { open } from '@tauri-apps/plugin-shell';
import { HousePlus, LogIn } from 'lucide-react';
import { useAppStore } from '../../stores';
import { LobbyForm } from '../LobbyForm/LobbyForm';
import { AboutWindow } from '../AboutWindow/AboutWindow';
import { SettingsWindow } from '../SettingsWindow';
import { SettingsIcon } from '../icons';
import { useEscapeKey } from '../../hooks';
import { useThemeColor } from '@/contexts/theme-color-context';
import './MainWindow.css';

function AntdWrapper({ children }: { children: React.ReactNode }) {
  const { colorMode } = useColorMode();
  const isDark = colorMode === 'dark';
  const antdConfig = {
    algorithm: isDark ? antdTheme.darkAlgorithm : antdTheme.defaultAlgorithm,
    token: {
      colorPrimary: '#5a9428',
      colorSuccess: '#52c41a',
      colorWarning: '#f59e0b',
      colorError: '#ef4444',
      borderRadius: 8,
      colorBgContainer: isDark ? 'rgba(30, 30, 45, 0.95)' : '#ffffff',
      colorBorder: isDark ? 'rgba(255, 255, 255, 0.1)' : '#e2e8f0',
      colorText: isDark ? 'rgba(255, 255, 255, 0.9)' : '#1a202c',
      colorTextSecondary: isDark ? 'rgba(255, 255, 255, 0.6)' : '#718096',
      fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", "Microsoft YaHei", sans-serif',
    },
  };
  return (
    <ConfigProvider locale={zhCN} theme={antdConfig}>
      <AntdApp>{children}</AntdApp>
    </ConfigProvider>
  );
}

export const MainWindow: React.FC = () => {
  const [showForm, setShowForm] = useState(false);
  const [formMode, setFormMode] = useState<'create' | 'join'>('create');
  const [showAbout, setShowAbout] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [enableGpuRendering, setEnableGpuRendering] = useState(true);

  const versionError = useAppStore((state) => state.versionError);
  const setVersionError = useAppStore((state) => state.setVersionError);

  const { getActiveColor, getHoverColor, getContrastTextColor } = useThemeColor();

  // 主题颜色
  const containerBg = useColorModeValue('white', '#111111');
  const containerBorder = useColorModeValue('gray.200', 'rgba(255,255,255,0.12)');
  const headingColor = useColorModeValue('gray.800', 'whiteAlpha.900');
  const mutedColor = useColorModeValue('gray.500', 'whiteAlpha.600');
  const cardTextColor = useColorModeValue('gray.700', 'whiteAlpha.900');

  const primaryColor = getActiveColor();
  const hoverBg = getHoverColor(true);
  const contrastText = getContrastTextColor();

  // 监听 GPU 渲染设置变化的全局事件
  useEffect(() => {
    const handleGpuRenderingChange = (event: CustomEvent) => {
      console.log('GPU 渲染设置已更改:', event.detail.enabled);
      setEnableGpuRendering(event.detail.enabled);
    };

    window.addEventListener('gpuRenderingChanged', handleGpuRenderingChange as EventListener);

    return () => {
      window.removeEventListener('gpuRenderingChanged', handleGpuRenderingChange as EventListener);
    };
  }, []);

  // ESC键返回
  useEscapeKey(() => {
    if (showForm) {
      handleCloseForm();
    } else if (showAbout) {
      handleCloseAbout();
    } else if (showSettings) {
      handleCloseSettings();
    }
  }, showForm || showAbout || showSettings);

  // 监听版本错误
  useEffect(() => {
    if (versionError) {
      Modal.warning({
        title: '版本过低',
        content: (
          <div style={{ lineHeight: '1.8' }}>
            <p style={{ marginBottom: '12px' }}>您的 MCTier 版本过低，无法连接到大厅。</p>
            <p style={{ marginBottom: '8px', color: 'rgba(255,255,255,0.8)' }}>
              当前版本: {versionError.currentVersion}
            </p>
            <p style={{ marginBottom: '12px', color: 'rgba(255,255,255,0.8)' }}>
              最低要求: {versionError.minimumVersion}
            </p>
            <p style={{ color: 'rgba(255,255,255,0.6)' }}>请前往官网下载最新版本</p>
          </div>
        ),
        okText: '前往官网',
        centered: true,
        onOk: async () => {
          try {
            let url = versionError.downloadUrl;
            if (!url.startsWith('http://') && !url.startsWith('https://')) {
              url = `https://${url}`;
            }
            await open(url);
          } catch (error) {
            console.error('打开官网失败:', error);
          }
          setVersionError(null);
        },
        onCancel: () => {
          setVersionError(null);
        },
      });
    }
  }, [versionError, setVersionError]);

  const handleCreateLobby = () => {
    setFormMode('create');
    setShowForm(true);
  };

  const handleJoinLobby = () => {
    setFormMode('join');
    setShowForm(true);
  };

  const handleCloseForm = () => {
    setShowForm(false);
  };

  const handleShowAbout = () => {
    setShowAbout(true);
  };

  const handleCloseAbout = () => {
    setShowAbout(false);
  };

  const handleShowSettings = () => {
    setShowSettings(true);
  };

  const handleCloseSettings = () => {
    setShowSettings(false);
  };

  if (showAbout) {
    return (
      <AntdWrapper>
        <AboutWindow onClose={handleCloseAbout} />
      </AntdWrapper>
    );
  }

  if (showForm) {
    return (
      <AntdWrapper>
        <LobbyForm mode={formMode} onClose={handleCloseForm} />
      </AntdWrapper>
    );
  }

  return (
    <>
      <Box w="100%" h="100%" position="relative">
        <Box
          w="100%"
          h="100%"
          bg={containerBg}
        border="1px solid"
        borderColor={containerBorder}
        borderRadius="2xl"
        px={16}
        py={12}
        position="relative"
        sx={{ WebkitBackfaceVisibility: 'hidden', backfaceVisibility: 'hidden' }}
      >
        {/* 设置按钮 - 右上角 */}
        <Box position="absolute" top={4} right={5} zIndex={10}>
          <motion.button
            onClick={handleShowSettings}
            whileHover={{ scale: 1.1, rotate: 30 }}
            whileTap={{ scale: 0.9 }}
            transition={{ type: 'spring', stiffness: 400, damping: 17 }}
            style={{
              background: containerBg,
              border: '1px solid ' + containerBorder,
              borderRadius: '8px',
              cursor: 'pointer',
              padding: '8px',
            }}
            title="设置"
          >
            <SettingsIcon size={20} color={mutedColor} />
          </motion.button>
        </Box>

        <Flex
          w="100%"
          h="100%"
          direction="column"
          align="center"
          justify="center"
          gap={10}
          userSelect="none"
        >

        {/* 标题区域 */}
        <Box textAlign="center">
          <motion.div
            initial={{ opacity: 0, y: -20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5, ease: [0.4, 0, 0.2, 1] }}
          >
            <Flex align="center" justify="center">
              <Box position="relative" display="inline-block">
                <Text fontSize="5xl" fontWeight="bold" color={headingColor} letterSpacing="tight">
                  联机
                </Text>
                <Box
                  as="span"
                  fontSize="xs"
                  fontWeight="extrabold"
                  px={1.5}
                  py={0.5}
                  borderRadius="sm"
                  bg="blue.500"
                  color="white"
                  letterSpacing="wider"
                  position="absolute"
                  top={1}
                  right="-14"
                >
                  BETA
                </Box>
              </Box>
            </Flex>
          </motion.div>
          <motion.div
            initial={{ opacity: 0, y: -15 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: 0.15, duration: 0.5, ease: [0.4, 0, 0.2, 1] }}
          >
            <Text fontSize="sm" color={mutedColor} mt={1}>
              基于 MCTier
            </Text>
          </motion.div>
        </Box>

        {/* 两张卡片 */}
        <Flex gap={6} direction={{ base: 'column', md: 'row' }}>
          {/* 创建大厅卡片 */}
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.5, ease: 'easeOut' }}
          >
            <Box
              as="button"
              onClick={handleCreateLobby}
              w="220px"
              h="180px"
              display="flex"
              flexDirection="column"
              alignItems="center"
              justifyContent="center"
              gap={4}
              borderRadius="2xl"
              bg={containerBg}
              border="1px solid"
              borderColor={containerBorder}
              cursor="pointer"
              transition="all 0.2s cubic-bezier(0.4, 0, 0.2, 1)"
              _hover={{
                bg: hoverBg,
                borderColor: primaryColor,
                boxShadow: 'xl',
              }}
            >
              <HousePlus size={40} color={primaryColor} />
              <Text fontSize="md" fontWeight="semibold" color={cardTextColor}>
                创建大厅
              </Text>
            </Box>
          </motion.div>

          {/* 加入大厅卡片 */}
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.5, ease: 'easeOut', delay: 0.1 }}
          >
            <Box
              as="button"
              onClick={handleJoinLobby}
              w="220px"
              h="180px"
              display="flex"
              flexDirection="column"
              alignItems="center"
              justifyContent="center"
              gap={4}
              borderRadius="2xl"
              bg={containerBg}
              border="1px solid"
              borderColor={containerBorder}
              cursor="pointer"
              transition="all 0.2s cubic-bezier(0.4, 0, 0.2, 1)"
              _hover={{
                bg: hoverBg,
                borderColor: primaryColor,
                boxShadow: 'xl',
              }}
            >
              <LogIn size={40} color={primaryColor} />
              <Text fontSize="md" fontWeight="semibold" color={cardTextColor}>
                加入大厅
              </Text>
            </Box>
          </motion.div>
        </Flex>{/* 关闭卡片内部Flex */}
        </Flex>{/* 关闭内层Flex */}
        </Box>{/* 关闭大卡片Box */}
      </Box>{/* 关闭外层容器Box */}

      {/* 设置界面 */}
      <AnimatePresence>
        {showSettings && (
          <motion.div
            key="settings-overlay"
            className="settings-overlay"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2 }}
            style={{ position: 'absolute', top: 0, left: 0, right: 0, bottom: 0, zIndex: 20 }}
          >
            <AntdWrapper>
              <SettingsWindow onClose={handleCloseSettings} />
            </AntdWrapper>
          </motion.div>
        )}
      </AnimatePresence>
    </>
  );
};