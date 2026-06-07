import React, { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { invoke } from '@tauri-apps/api/core';
import { Tooltip } from 'antd';
import { open } from '@tauri-apps/plugin-shell';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import {
  Box, Flex, HStack, VStack, Text, Heading, Button, IconButton,
  Slider, SliderTrack, SliderFilledTrack, SliderThumb,
  useColorModeValue, useToast,
  Modal as ChakraModal, ModalOverlay, ModalContent, ModalHeader, ModalBody, ModalCloseButton, Spinner,
} from '@chakra-ui/react';
import { useAppStore } from '../../stores';
import { webrtcClient } from '../../services';
import { p2pChatService } from '../../services/chat/P2PChatService';
import type { ChatMessage } from '../../types';
import { PlayerIcon, MicIcon, SpeakerIcon, CloseCircleIcon, CollapseIcon, CloseIcon, WarningTriangleIcon, InfoIcon, ScreenShareIcon } from '../icons';
import { ChatRoom } from '../ChatRoom/ChatRoom';
import { FileShareManagerNew } from '../FileShareManager/FileShareManagerNew';
import { ScreenShareManager } from '../ScreenShareManager/ScreenShareManager';
import { LobbySettingsModal } from '../LobbySettingsModal/LobbySettingsModal';
import { useThemeColor } from '@/contexts/theme-color-context';
import './MiniWindow.css';

/**
 * 迷你窗口组件
 * 显示精简的大厅信息和语音控制
 */
export const MiniWindow: React.FC = () => {
  const {
    lobby,
    players,
    micEnabled,
    globalMuted,
    mutedPlayers,
    toggleGlobalMute,
    togglePlayerMute,
    config,
    versionError,
    chatMessages,
    currentPlayerId,
    setPlayerVolume,
    getPlayerVolume,
    pendingLeaveLobby,
    setPendingLeaveLobby,
    shouldExitAfterLeave,
  } = useAppStore();

  const {
    config: themeConfig,
    getActiveColor,
    getHoverColor,
    getBorderColor,
    getContrastTextColor,
  } = useThemeColor();

  const bg = useColorModeValue('white', '#111111');
  const mutedBg = useColorModeValue('gray.50', 'rgba(255,255,255,0.02)');
  const borderColor = useColorModeValue('gray.200', 'rgba(255,255,255,0.12)');
  const textColor = useColorModeValue('gray.800', 'white');
  const mutedText = useColorModeValue('gray.500', 'rgba(255,255,255,0.5)');
  const cardBg = useColorModeValue('gray.50', 'rgba(255,255,255,0.03)');
  const btnColor = useColorModeValue('gray.700', 'white');
  const iconColor = useColorModeValue('gray.600', 'whiteAlpha.700');
  const toast = useToast();

  const [collapsed, setCollapsed] = useState(false);
  const [opacity, setOpacity] = useState(config.opacity ?? 0.95);
  const [isLeaving, setIsLeaving] = useState(false);
  const [showConnectionHelp, setShowConnectionHelp] = useState(false);
  const [currentView, setCurrentView] = useState<'lobby' | 'chat' | 'fileShare' | 'screenShare'>('lobby');
  const [chatOpenedWhenCollapsed, setChatOpenedWhenCollapsed] = useState(false); // 记录打开聊天室时窗口是否处于收起状态
  const [showLobbySettings, setShowLobbySettings] = useState(false); // 控制动态设置弹窗显示
  const [isRejoining, setIsRejoining] = useState(false); // 控制重新加入大厅的加载提示
  
  // 跟踪上次查看聊天室时的消息数量（只计算其他人的消息）
  const [lastViewedOthersMessageCount, setLastViewedOthersMessageCount] = useState(0);
  
  // 计算其他人发送的消息数量
  const othersMessages = chatMessages.filter(msg => msg.playerId !== currentPlayerId);
  const othersMessageCount = othersMessages.length;
  
  // 计算未读消息数量（只计算其他人的消息）
  const unreadCount = Math.max(0, othersMessageCount - lastViewedOthersMessageCount);
  
  // 调试日志 - 详细打印未读消息统计
  useEffect(() => {
    console.log('📊 [MiniWindow] 未读消息统计:', {
      currentPlayerId,
      totalMessages: chatMessages.length,
      othersMessageCount,
      lastViewedOthersMessageCount,
      unreadCount,
      hasUnreadMessages: unreadCount > 0,
      currentView,
      collapsed,
    });
    
    // 打印最近的几条消息
    if (chatMessages.length > 0) {
      console.log('📝 [MiniWindow] 最近的消息:', chatMessages.slice(-3).map(m => ({
        id: m.id,
        playerId: m.playerId,
        playerName: m.playerName,
        content: m.content.substring(0, 20),
        timestamp: new Date(m.timestamp).toLocaleTimeString(),
      })));
    }
  }, [chatMessages.length, unreadCount, currentView, collapsed]);



  // 监听版本错误（不自动跳转，保持在大厅界面显示错误提示）
  useEffect(() => {
    if (versionError) {
      console.log('检测到版本错误，显示更新提示界面');
    }
  }, [versionError]);

  // 组件加载时从配置中读取透明度并设置（进入大厅）
  // 组件卸载时恢复完全不透明（退出大厅）
  useEffect(() => {
    const setupOpacity = async () => {
      try {
        // 从配置中获取透明度，如果没有则使用默认值0.95
        const initialOpacity = config.opacity ?? 0.95;
        setOpacity(initialOpacity);
        
        // 设置窗口透明度
        await invoke('set_window_opacity', { opacity: initialOpacity });
        console.log('进入大厅，透明度已设置为:', initialOpacity);
      } catch (error) {
        console.error('设置透明度失败:', error);
      }
    };

    setupOpacity();

    // 组件卸载时恢复完全不透明
    return () => {
      const restoreOpacity = async () => {
        try {
          await invoke('set_window_opacity', { opacity: 1.0 });
          console.log('退出大厅，透明度已恢复为完全不透明');
        } catch (error) {
          console.error('恢复透明度失败:', error);
        }
      };
      restoreOpacity();
    };
  }, [config.opacity]);

  // 进入大厅时取消全局静音（听筒默认开启）
  useEffect(() => {
    if (globalMuted) {
      console.log('进入大厅，自动开启听筒');
      toggleGlobalMute();
    }
  }, []); // 只在组件挂载时执行一次

  // 初始化P2P聊天服务 - 在大厅界面就启动，不需要打开聊天室
  // 【修复】合并为一个useEffect，统一管理SSE生命周期，避免清理函数关闭连接后无法重建
  useEffect(() => {
    if (!lobby || !currentPlayerId || players.length === 0) {
      console.log('⚠️ 大厅或玩家ID未就绪，跳过P2P聊天服务初始化');
      return;
    }

    // 【修复】获取所有玩家的虚拟IP，并加入自己的IP以便SSE能接收其他玩家POST来的消息
    const playerIPs = players.map(p => p.virtualIp).filter(Boolean) as string[];
    if (lobby.virtualIp && !playerIPs.includes(lobby.virtualIp)) {
      playerIPs.push(lobby.virtualIp);
    }

    console.log('🚀 [MiniWindow] 初始化P2P聊天服务');
    console.log('  - 当前玩家ID:', currentPlayerId);
    console.log('  - 自己的虚拟IP:', lobby.virtualIp);
    console.log('  - 其他玩家IPs:', playerIPs);

    // 设置消息接收回调
    p2pChatService.onMessage((message) => {
      console.log('📨 [MiniWindow] 收到P2P消息:', message);

      // 查找发送者名称
      let senderName = '未知玩家';
      if (message.playerId === currentPlayerId) {
        senderName = config.playerName || '我';
      } else {
        // 从当前的players列表中查找
        const currentPlayers = useAppStore.getState().players;
        const sender = currentPlayers.find(p => p.id === message.playerId);
        senderName = sender?.name || '未知玩家';
      }

      // 添加到消息列表（通过getState获取，避免addChatMessage引用变化触发useEffect重执行）
      const chatMessage: ChatMessage = {
        id: message.id,
        playerId: message.playerId,
        playerName: senderName,
        content: message.content,
        timestamp: message.timestamp,
        type: message.type,
        imageData: message.imageData,
      };

      useAppStore.getState().addChatMessage(chatMessage);

      // 如果不在聊天室界面，播放新消息提示音
      if (!(window as any).__isInChatRoom__) {
        console.log('🔔 [MiniWindow] 不在聊天室，播放新消息提示音');
        // TODO: 播放提示音
      }
    });

    // 初始化P2P聊天服务（传入自己的虚拟IP用于过滤）
    p2pChatService.initialize(playerIPs, currentPlayerId, lobby.virtualIp, config.playerName);

    // 开始轮询消息
    p2pChatService.startPolling();
    console.log('✅ [MiniWindow] P2P聊天服务已启动');

    return () => {
      p2pChatService.stopPolling();
      console.log('✅ [MiniWindow] 已停止P2P聊天服务');
    };
  }, [lobby, currentPlayerId, config.playerName, players.length, lobby?.virtualIp]);

  // 监听ESC键返回大厅
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (currentView === 'chat') {
          setCurrentView('lobby');
          // 标记所有其他人的消息为已读
          setLastViewedOthersMessageCount(othersMessageCount);
        } else if (currentView === 'fileShare') {
          setCurrentView('lobby');
        } else if (currentView === 'screenShare') {
          setCurrentView('lobby');
        }
      }
    };

    // 监听来自ChatRoom的标记已读事件
    const handleMarkAsRead = () => {
      setLastViewedOthersMessageCount(othersMessageCount);
    };

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('markChatMessagesAsRead', handleMarkAsRead);
    
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('markChatMessagesAsRead', handleMarkAsRead);
    };
  }, [currentView, othersMessageCount]);

  const handleToggleMic = async () => {
    try {
      // 调用后端的toggle_mic命令
      await invoke<boolean>('toggle_mic');
      // 后端会发送mic-toggled事件，前端会自动更新UI
    } catch (error) {
      console.error('切换麦克风失败:', error);
    }
  };

  const handleToggleGlobalMute = async () => {
    try {
      console.log('切换全局静音状态...');
      toggleGlobalMute();
    } catch (error) {
      console.error('切换全局静音失败:', error);
    }
  };

  const handleMutePlayer = async (playerId: string) => {
    try {
      console.log('切换玩家静音状态:', playerId);
      togglePlayerMute(playerId);
    } catch (error) {
      console.error('切换玩家静音失败:', error);
    }
  };

  const handleLeaveLobby = async () => {
    try {
      console.log('🚪 开始退出大厅流程...');
      
      // 显示退出中的提示
      setIsLeaving(true);
      
      // 先恢复窗口大小（如果是收起状态）
      if (collapsed) {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const { LogicalSize } = await import('@tauri-apps/api/dpi');
        const appWindow = getCurrentWindow();
        await appWindow.setSize(new LogicalSize(320, 520));
        console.log('窗口大小已恢复');
      }
      
      // 1. 先清理WebRTC客户端（关闭所有连接和WebSocket）
      console.log('正在清理WebRTC客户端...');
      await webrtcClient.cleanup();
      console.log('✅ WebRTC客户端已清理');
      
      // 2. 重置P2P聊天服务
      console.log('正在重置P2P聊天服务...');
      p2pChatService.reset();
      console.log('✅ P2P聊天服务已重置');
      
      // 等待一小段时间，确保WebSocket完全关闭
      await new Promise(resolve => setTimeout(resolve, 300));
      
      // 2. 调用后端退出大厅（停止EasyTier和清理网络）
      console.log('正在调用后端退出大厅...');
      await invoke('leave_lobby');
      console.log('✅ 后端退出大厅成功');
      
      // 3. 停止HTTP文件服务器
      try {
        await invoke('stop_file_server');
        console.log('✅ HTTP文件服务器已停止');
      } catch (error) {
        console.error('❌ 停止HTTP文件服务器失败:', error);
        // 不中断流程
      }
      
      // 4. 更新前端状态返回主界面
      const { setAppState, clearLobby } = useAppStore.getState();
      clearLobby(); // 这会清理大厅、玩家列表和语音状态
      setAppState('idle');
      setIsLeaving(false);
      console.log('✅ 前端状态已清理，返回主界面');
    } catch (error) {
      console.error('❌ 退出大厅失败:', error);
      // 即使后端退出失败，也要清理前端状态并返回主界面
      try {
        await webrtcClient.cleanup();
      } catch (cleanupError) {
        console.error('❌ 清理WebRTC失败:', cleanupError);
      }
      
      const { setAppState, clearLobby } = useAppStore.getState();
      clearLobby(); // 这会清理大厅、玩家列表和语音状态
      setAppState('idle');
      setIsLeaving(false);
      console.log('⚠️ 已强制返回主界面');
    }
  };

  // 监听来自后端的退出请求（窗口关闭/托盘退出），触发返回主界面流程
  useEffect(() => {
    if (pendingLeaveLobby) {
      console.log(`收到 pendingLeaveLobby，shouldExitAfterLeave=${shouldExitAfterLeave}，开始退出流程...`);
      setPendingLeaveLobby(false);
      
      handleLeaveLobby().then(() => {
        // 退出大厅完成后，检查是否需要关闭软件
        if (shouldExitAfterLeave) {
          console.log('退出大厅完成，关闭软件...');
          import('@tauri-apps/api/core').then(({ invoke }) => {
            invoke('force_exit_app');
          });
        }
      });
    }
  }, [pendingLeaveLobby]);

  const handleToggleCollapse = async () => {
    try {
      console.log('收起按钮被点击，当前状态:', collapsed);
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      const { LogicalSize } = await import('@tauri-apps/api/dpi');
      const appWindow = getCurrentWindow();
      
      if (!collapsed) {
        // 收起：缩小窗口到只显示标题栏
        console.log('正在收起窗口...');
        await appWindow.setSize(new LogicalSize(320, 50));
        console.log('窗口已收起');
      } else {
        // 展开：恢复窗口大小
        console.log('正在展开窗口...');
        await appWindow.setSize(new LogicalSize(320, 520));
        console.log('窗口已展开');
      }
      
      setCollapsed(!collapsed);
    } catch (error) {
      console.error('切换窗口大小失败:', error);
      console.error('错误详情:', error);
    }
  };

  const handleOpacityChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const newOpacity = parseFloat(e.target.value);
    setOpacity(newOpacity);
    
    try {
      // 调用后端设置真实的窗口透明度
      await invoke('set_window_opacity', { opacity: newOpacity });
      console.log('窗口透明度已更改为:', newOpacity);
      
      // 保存透明度到配置文件
      await invoke('save_opacity', { opacity: newOpacity });
      console.log('透明度已保存到配置文件');
      
      // 更新前端 store 中的配置
      const { updateConfig } = useAppStore.getState();
      updateConfig({ opacity: newOpacity });
      console.log('前端 store 中的透明度已更新');
    } catch (error) {
      console.error('设置或保存窗口透明度失败:', error);
    }
  };

  // 处理动态设置保存后重新加入大厅
  const handleLobbySettingsSaved = async () => {
    console.log('🎯 [MiniWindow] handleLobbySettingsSaved 被调用了！');
    console.log('🎯 [MiniWindow] lobby:', lobby);
    console.log('🎯 [MiniWindow] currentPlayerId:', currentPlayerId);
    
    if (!lobby || !currentPlayerId) {
      console.error('❌ [MiniWindow] 验证失败：lobby 或 currentPlayerId 无效');
      toast({ title: '当前未在大厅中或玩家ID无效', status: 'error', duration: 3000, isClosable: true, position: 'top' });
      return;
    }

    console.log('📢 [MiniWindow] 大厅设置已保存，准备重新加入大厅...');

    // 先关闭大厅设置弹窗
    console.log('🚪 [MiniWindow] 正在关闭大厅设置弹窗...');
    setShowLobbySettings(false);
    console.log('✅ [MiniWindow] 大厅设置弹窗已关闭');
    
    // 等待弹窗完全关闭
    console.log('⏳ [MiniWindow] 等待弹窗完全关闭（200ms）...');
    await new Promise(resolve => setTimeout(resolve, 200));
    console.log('✅ [MiniWindow] 等待完成');

    // 显示重新加入大厅的加载提示（使用自定义遮罩层，和退出大厅一样）
    console.log('🎨 [MiniWindow] 显示重新加入大厅的加载提示...');
    setIsRejoining(true);
    console.log('✅ [MiniWindow] 加载提示已显示');

    try {
      // 1. 先退出当前大厅（不清理前端状态）
      console.log('🚪 [MiniWindow] 正在退出当前大厅...');
      await invoke('leave_lobby');
      console.log('✅ [MiniWindow] 已退出当前大厅');

      // 等待足够的时间确保资源完全释放（包括进程退出、网卡清理等）
      // stop_easytier 需要：3秒等待进程退出 + 0.5秒清理网卡 + 0.5秒清理配置 = 至少4秒
      console.log('⏳ [MiniWindow] 等待资源完全释放（5秒）...');
      await new Promise(resolve => setTimeout(resolve, 5000));
      console.log('✅ [MiniWindow] 资源释放等待完成');

      // 2. 重新加载配置
      console.log('📖 [MiniWindow] 正在重新加载配置...');
      const settings = await invoke<any>('get_settings');
      console.log('✅ [MiniWindow] 已重新加载配置');

      // 3. 使用新配置重新加入大厅
      console.log('🔌 [MiniWindow] 正在使用新配置重新加入大厅...');
      const serverNode = (settings.usePrivateServer && settings.privateEasytierServer)
        ? settings.privateEasytierServer 
        : 'wss://mctiers.pmhs.top';
      const signalingServer = (settings.usePrivateServer && settings.privateSignalingServer)
        ? settings.privateSignalingServer 
        : 'wss://mctier.pmhs.top/signaling';

      const useDomain = settings.useDomain || false;
      const virtualDomain = settings.virtualDomain || '';

      const newLobby = await invoke<any>('join_lobby', {
        name: lobby.name || '',
        password: lobby.password || '',
        playerName: config.playerName || '玩家',
        playerId: currentPlayerId,
        serverNode,
        signalingServer,
        useDomain: useDomain,
        virtualDomain: virtualDomain,
      });

      console.log('✅ [MiniWindow] 重新加入大厅成功:', newLobby);

      // 4. 更新前端状态
      const { setLobby } = useAppStore.getState();
      setLobby(newLobby);

      // 5. 重新初始化WebRTC
      console.log('🔄 [MiniWindow] 正在重新初始化WebRTC...');
      await webrtcClient.initialize(
        currentPlayerId,
        config.playerName || '玩家',
        lobby.name || '',
        lobby.password || '',
        virtualDomain,
        useDomain,
        signalingServer
      );
      console.log('✅ [MiniWindow] WebRTC重新初始化成功');

      // 关闭加载提示
      setIsRejoining(false);
      
      // 显示成功提示
      toast({ title: '设置已应用，重新加入大厅成功', status: 'success', duration: 2000, isClosable: true, position: 'top' });
    } catch (error) {
      console.error('❌ [MiniWindow] 重新加入大厅失败:', error);
      
      // 关闭加载提示
      setIsRejoining(false);
      
      toast({ title: `重新加入大厅失败: ${error}`, status: 'error', duration: 3000, isClosable: true, position: 'top' });
      
      // 如果失败，返回主界面
      const { setAppState, clearLobby } = useAppStore.getState();
      clearLobby();
      setAppState('idle');
    }
  };

  // 处理玩家音量变化
  const handlePlayerVolumeChange = (playerId: string, volume: number) => {
    setPlayerVolume(playerId, volume);
    console.log(`玩家 ${playerId} 音量已设置为: ${Math.round(volume * 100)}%`);
  };

  // 打开聊天室（从聊天室按钮）
  const handleOpenChatRoom = () => {
    setCurrentView('chat');
    // 记录打开聊天室时窗口是否处于收起状态
    setChatOpenedWhenCollapsed(collapsed);
    // 设置全局标志：当前在聊天室界面
    (window as any).__isInChatRoom__ = true;
    // 标记所有其他人的消息为已读
    setLastViewedOthersMessageCount(othersMessageCount);
    console.log(`✅ 打开聊天室，窗口${collapsed ? '收起' : '展开'}状态`);
  };

  // 关闭聊天室，返回大厅界面
  const handleCloseChatRoom = async () => {
    setCurrentView('lobby');
    // 清除全局标志：离开聊天室界面
    (window as any).__isInChatRoom__ = false;
    // 标记所有其他人的消息为已读
    setLastViewedOthersMessageCount(othersMessageCount);
    
    // 如果打开聊天室时窗口是收起状态，关闭时自动收起
    if (chatOpenedWhenCollapsed) {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const { LogicalSize } = await import('@tauri-apps/api/dpi');
        const appWindow = getCurrentWindow();
        await appWindow.setSize(new LogicalSize(320, 50));
        setCollapsed(true);
        console.log('✅ 聊天室关闭，窗口已自动收起');
      } catch (error) {
        console.error('自动收起窗口失败:', error);
      }
      setChatOpenedWhenCollapsed(false); // 重置标记
    } else {
      console.log('✅ 聊天室关闭，窗口保持展开状态');
    }
  };

  // 处理新消息按钮点击
  const handleNewMessageClick = async () => {
    try {
      // 记录当前窗口是否处于收起状态
      const wasCollapsed = collapsed;
      
      // 如果窗口是收起状态，先展开窗口
      if (collapsed) {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const { LogicalSize } = await import('@tauri-apps/api/dpi');
        const appWindow = getCurrentWindow();
        await appWindow.setSize(new LogicalSize(320, 520));
        setCollapsed(false);
        console.log('窗口已展开');
      }
      
      // 切换到聊天室视图
      setCurrentView('chat');
      // 记录打开聊天室时窗口是否处于收起状态
      setChatOpenedWhenCollapsed(wasCollapsed);
      // 设置全局标志：当前在聊天室界面
      (window as any).__isInChatRoom__ = true;
      // 标记所有其他人的消息为已读
      setLastViewedOthersMessageCount(othersMessageCount);
      console.log(`✅ 从${wasCollapsed ? '迷你窗口' : '大厅界面'}打开聊天室并标记消息为已读`);
    } catch (error) {
      console.error('打开聊天室失败:', error);
    }
  };

  // 打开 mcwifipnp 模组页面
  const handleOpenModPage = async () => {
    try {
      await open('https://www.mcmod.cn/class/4498.html');
    } catch (error) {
      console.error('打开模组页面失败:', error);
    }
  };

  // 打开官网
  const handleOpenWebsite = async () => {
    if (!versionError) return;
    
    try {
      // 确保URL以https://开头
      let url = versionError.downloadUrl;
      if (!url.startsWith('http://') && !url.startsWith('https://')) {
        url = `https://${url}`;
        console.log('自动添加https://前缀:', url);
      }
      
      await open(url);
      console.log('已打开官网:', url);
      toast({ title: '已在浏览器中打开官网', status: 'success', duration: 2000, isClosable: true, position: 'top' });
    } catch (error) {
      console.error('打开官网失败:', error);
      toast({ title: '打开官网失败，请手动复制链接', status: 'error', duration: 3000, isClosable: true, position: 'top' });
    }
  };

  // 复制官网链接
  const handleCopyWebsiteUrl = async () => {
    if (!versionError) return;
    
    try {
      await writeText(versionError.downloadUrl);
      toast({ title: '官网链接已复制到剪贴板', status: 'success', duration: 2000, isClosable: true, position: 'top' });
      console.log('已复制官网链接:', versionError.downloadUrl);
    } catch (error) {
      console.error('复制链接失败:', error);
      toast({ title: '复制失败，请手动复制', status: 'error', duration: 3000, isClosable: true, position: 'top' });
    }
  };

  // 复制虚拟IP或虚拟域名
  const handleCopyVirtualIp = async () => {
    if (!lobby) return;
    
    try {
      // 根据useDomain决定复制IP还是域名
      const textToCopy = (lobby.useDomain && lobby.virtualDomain) ? lobby.virtualDomain : lobby.virtualIp;
      if (!textToCopy) {
        toast({ title: '虚拟地址尚未获取', status: 'warning', duration: 2000, isClosable: true, position: 'top' });
        return;
      }
      
      await writeText(textToCopy);
      const label = (lobby.useDomain && lobby.virtualDomain) ? '虚拟域名' : '虚拟IP';
      toast({ title: `${label}已复制`, status: 'success', duration: 2000, isClosable: true, position: 'top' });
      console.log(`已复制${label}:`, textToCopy);
    } catch (error) {
      console.error('复制失败:', error);
      toast({ title: '复制失败，请重试', status: 'error', duration: 3000, isClosable: true, position: 'top' });
    }
  };

  // 复制大厅信息
  const handleCopyLobbyInfo = async () => {
    if (!lobby) return;
    
    try {
      // 新格式：
      // ———————— 邀请您加入大厅 ————————
      // 完整复制后打开 MCTier-加入大厅 界面（自动识别）
      // 大厅名称：XXX
      // 密码：XXX
      // —————— (https://mctier.pmhs.top) ——————
      const lobbyInfo = `——————— 邀请您加入大厅 ———————
完整复制后打开 MCTier-加入大厅 界面（自动识别）
大厅名称：${lobby.name}
密码：${lobby.password || ''}
————— https://mctier.pmhs.top —————`;
      
      await writeText(lobbyInfo);
      
      // 显示提示信息
      toast({
        title: '大厅信息已复制',
        description: '已将大厅信息复制到剪贴板。分享给好友后，好友打开联机点击"加入大厅"即可自动识别。',
        status: 'success',
        duration: 5000,
        isClosable: true,
        position: 'top',
      });
      
      console.log('已复制大厅信息:', lobbyInfo);
    } catch (error) {
      console.error('复制失败:', error);
      toast({ title: '复制失败，请重试', status: 'error', duration: 3000, isClosable: true, position: 'top' });
    }
  };

  return (
    <>
      {/* 版本错误全屏提示 - 完全覆盖大厅界面 */}
      {versionError && (
        <motion.div
          className="version-error-overlay"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ duration: 0.3 }}
        >
          <motion.div
            className="version-error-content"
            initial={{ scale: 0.9, y: 20 }}
            animate={{ scale: 1, y: 0 }}
            transition={{ duration: 0.4, ease: [0.4, 0, 0.2, 1] }}
          >
            {/* 警告图标 */}
            <motion.div
              className="version-error-icon-wrapper"
              initial={{ scale: 0 }}
              animate={{ scale: 1 }}
              transition={{ delay: 0.2, type: 'spring', stiffness: 200 }}
            >
              <WarningTriangleIcon size={80} className="version-error-icon" />
            </motion.div>

            {/* 标题 */}
            <h2 className="version-error-title">版本过低，无法连接</h2>

            {/* 版本信息 */}
            <div className="version-error-info">
              <div className="version-info-row">
                <span className="version-label">当前版本</span>
                <span className="version-value current">{versionError.currentVersion}</span>
              </div>
              <div className="version-info-row">
                <span className="version-label">最低要求</span>
                <span className="version-value required">{versionError.minimumVersion}</span>
              </div>
            </div>

            {/* 提示信息 */}
            <div className="version-error-message">
              <p>客户端版本过低，服务器已拒绝连接</p>
              <p>请下载最新版本以继续使用 MCTier</p>
            </div>

            {/* 官网链接 */}
            <div className="version-error-url">
              <div className="url-label">官网下载地址</div>
              <div className="url-box">
                <span className="url-text">{versionError.downloadUrl}</span>
                <motion.button
                  className="url-copy-btn"
                  onClick={handleCopyWebsiteUrl}
                  whileHover={{ scale: 1.05 }}
                  whileTap={{ scale: 0.95 }}
                  title="复制链接"
                >
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                  </svg>
                </motion.button>
              </div>
            </div>

            {/* 操作按钮 */}
            <div className="version-error-actions">
              <motion.button
                className="version-error-btn primary"
                onClick={handleOpenWebsite}
                whileHover={{ scale: 1.02 }}
                whileTap={{ scale: 0.98 }}
              >
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ marginRight: '10px' }}>
                  <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path>
                  <polyline points="15 3 21 3 21 9"></polyline>
                  <line x1="10" y1="14" x2="21" y2="3"></line>
                </svg>
                <span>前往官网下载</span>
              </motion.button>
            </div>
          </motion.div>
        </motion.div>
      )}

      {/* 退出大厅加载提示 */}
      <ChakraModal isOpen={isLeaving} isCentered onClose={() => {}} closeOnOverlayClick={false} closeOnEsc={false}>
        <ModalOverlay bg="blackAlpha.600" />
        <ModalContent
          bg={useColorModeValue('white', '#1e1e2e')}
          border="1px solid"
          borderColor={useColorModeValue('gray.200', 'rgba(255,255,255,0.1)')}
          borderRadius="2xl"
          boxShadow="2xl"
          maxW="300px"
          py={8}
        >
          <ModalBody textAlign="center" py={6}>
            <VStack spacing={4}>
              <Spinner size="xl" color={useColorModeValue('#5a9428', '#7ed321')} thickness="4px" />
              <Text fontSize="lg" fontWeight={600} color={useColorModeValue('gray.900', 'rgba(255,255,255,0.9)')}>
                正在退出大厅...
              </Text>
              <Text fontSize="sm" color={useColorModeValue('gray.500', 'rgba(255,255,255,0.6)')}>
                正在清理网络连接和虚拟网卡
              </Text>
            </VStack>
          </ModalBody>
        </ModalContent>
      </ChakraModal>

      {/* 重新加入大厅加载提示 */}
      <ChakraModal isOpen={isRejoining} isCentered onClose={() => {}} closeOnOverlayClick={false} closeOnEsc={false}>
        <ModalOverlay bg="blackAlpha.600" />
        <ModalContent
          bg={useColorModeValue('white', '#1e1e2e')}
          border="1px solid"
          borderColor={useColorModeValue('gray.200', 'rgba(255,255,255,0.1)')}
          borderRadius="2xl"
          boxShadow="2xl"
          maxW="400px"
        >
          <ModalBody textAlign="center" py={10}>
            <VStack spacing={5}>
              <Spinner size="xl" color={useColorModeValue('#5a9428', '#7ed321')} thickness="4px" />
              <Text fontSize="lg" fontWeight={700} color={useColorModeValue('gray.900', 'rgba(255,255,255,0.95)')}>
                正在重载设置...
              </Text>
              <Text fontSize="sm" color={useColorModeValue('gray.600', 'rgba(255,255,255,0.75)')}>
                正在重新配置并加入...
              </Text>
              <Text fontSize="xs" color={useColorModeValue('gray.400', 'rgba(255,255,255,0.5)')}>
                请稍等，这可能需要几秒钟...
              </Text>
            </VStack>
          </ModalBody>
        </ModalContent>
      </ChakraModal>

      {/* 联机帮助弹窗 */}
      <ChakraModal isOpen={showConnectionHelp} onClose={() => setShowConnectionHelp(false)} isCentered size="lg">
        <ModalOverlay bg="blackAlpha.600" />
        <ModalContent
          bg={useColorModeValue('white', '#1e1e2e')}
          border="1px solid"
          borderColor={useColorModeValue('gray.200', 'rgba(255,255,255,0.1)')}
          borderRadius="2xl"
          boxShadow="2xl"
        >
          <ModalHeader pb={2} pt={6} px={6}>
            <Text fontSize="lg" fontWeight={700} color={useColorModeValue('gray.900', 'white')}>MC联机帮助</Text>
          </ModalHeader>
          <ModalCloseButton color={useColorModeValue('gray.500', 'rgba(255,255,255,0.5)')} _hover={{ color: useColorModeValue('gray.700', 'white') }} />
          <ModalBody px={6} pb={8}>
            <Box lineHeight="tall">
              <Text mb={4} fontWeight="bold" color={useColorModeValue('#52c41a', '#7ed321')}>
                联机方式说明：
              </Text>

              <Box mb={3}>
                <Text fontWeight="semibold" fontSize="sm" color={useColorModeValue('gray.800', 'rgba(255,255,255,0.9)')}>
                  1. 双方都是正版：
                </Text>
                <Text fontSize="sm" mt={1} color={useColorModeValue('gray.600', 'rgba(255,255,255,0.7)')}>
                  房主对局域网开放后，其他玩家在多人游戏中使用{' '}
                  <Box as="span" display="inline-block" px={2} py={0.5} borderRadius="md" bg={useColorModeValue('gray.100', 'rgba(255,255,255,0.08)')} fontFamily="monospace">
                    房主虚拟IP:端口号
                  </Box>
                  {' '}加入
                </Text>
              </Box>

              <Box mb={3}>
                <Text fontWeight="semibold" fontSize="sm" color={useColorModeValue('gray.800', 'rgba(255,255,255,0.9)')}>
                  2. 房主离线模式，加入者正版：
                </Text>
                <Text fontSize="sm" mt={1} color={useColorModeValue('gray.600', 'rgba(255,255,255,0.7)')}>
                  加入者在多人游戏中使用{' '}
                  <Box as="span" display="inline-block" px={2} py={0.5} borderRadius="md" bg={useColorModeValue('gray.100', 'rgba(255,255,255,0.08)')} fontFamily="monospace">
                    房主虚拟IP:端口号
                  </Box>
                  {' '}加入
                </Text>
              </Box>

              <Box mb={3}>
                <Text fontWeight="semibold" fontSize="sm" color={useColorModeValue('gray.800', 'rgba(255,255,255,0.9)')}>
                  3. 房主正版，加入者离线模式：
                </Text>
                <Text fontSize="sm" mt={1} color={useColorModeValue('gray.600', 'rgba(255,255,255,0.7)')}>
                  房主需要安装{' '}
                  <Text as="u" cursor="pointer" color={useColorModeValue('#1890ff', '#63b3ed')} onClick={handleOpenModPage}>
                    mcwifipnp
                  </Text>
                  {' '}模组关闭正版验证
                </Text>
              </Box>

              <Box mb={4}>
                <Text fontWeight="semibold" fontSize="sm" color={useColorModeValue('gray.800', 'rgba(255,255,255,0.9)')}>
                  4. 双方都是离线模式：
                </Text>
                <Text fontSize="sm" mt={1} color={useColorModeValue('gray.600', 'rgba(255,255,255,0.7)')}>
                  房主需要安装{' '}
                  <Text as="u" cursor="pointer" color={useColorModeValue('#1890ff', '#63b3ed')} onClick={handleOpenModPage}>
                    mcwifipnp
                  </Text>
                  {' '}模组关闭正版验证
                </Text>
              </Box>

              <Box p={3} bg={useColorModeValue('yellow.50', 'rgba(255,193,7,0.1)')} borderRadius="lg" borderLeft="3px solid" borderColor={useColorModeValue('yellow.400', '#ffc107')}>
                <Text fontWeight="bold" color={useColorModeValue('yellow.600', '#ffc107')} fontSize="sm">
                  提示：
                </Text>
                <Text mt={1} fontSize="sm" color={useColorModeValue('yellow.700', 'rgba(255,255,255,0.7)')}>
                  虚拟IP显示在大厅信息中，端口号由房主在游戏内对局域网开放时显示
                </Text>
              </Box>
            </Box>
          </ModalBody>
        </ModalContent>
      </ChakraModal>

      {/* 根据当前视图显示不同内容 */}
      <AnimatePresence mode="wait">
        {currentView === 'chat' ? (
          <motion.div
            key="chat"
            className="chat-room-view"
            initial={{ opacity: 1 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 1 }}
            transition={{ duration: 0 }}
          >
            <HStack
              px="14px"
              py="10px"
              bg={mutedBg}
              borderBottom="1px solid"
              borderColor={borderColor}
              justify="space-between"
              align="center"
              minH="50px"
            >
              <Heading fontSize="14px" fontWeight={600} color={textColor} m={0}>
                聊天室
              </Heading>
              <IconButton
                aria-label="关闭聊天室"
                icon={<CloseIcon size={16} />}
                size="sm"
                variant="ghost"
                borderRadius="8px"
                bg={cardBg}
                color={iconColor}
                _hover={{ bg: 'rgba(239, 68, 68, 0.3)', color: '#ef4444', transform: 'scale(1.1)' }}
                onClick={handleCloseChatRoom}
                title="关闭聊天室 (ESC)"
              />
            </HStack>
            <ChatRoom />
          </motion.div>
        ) : currentView === 'fileShare' ? (
          <motion.div
            key="fileShare"
            className="file-share-view"
            initial={{ opacity: 1 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 1 }}
            transition={{ duration: 0 }}
            style={{
              background: useColorModeValue('white', 'linear-gradient(135deg, rgba(20, 20, 30, 0.98) 0%, rgba(25, 25, 35, 0.98) 100%)'),
            }}
          >
            <div
              className="file-share-header"
              style={{
                background: useColorModeValue('rgba(0,0,0,0.02)', 'rgba(255,255,255,0.03)'),
                borderBottom: `1px solid ${useColorModeValue('rgba(0,0,0,0.08)', 'rgba(255,255,255,0.08)')}`,
              }}
            >
              <div className="file-share-title-wrapper">
                <h3 className="file-share-title" style={{ color: textColor }}>文件夹共享</h3>
                <Tooltip
                  title="将您电脑中的任何文件夹共享到当前大厅中，提供给同大厅内的其他玩家访问并下载。"
                  placement="bottom"
                >
                  <div className="file-share-info-icon" style={{ color: mutedText }}>
                    <InfoIcon size={14} />
                  </div>
                </Tooltip>
              </div>
              <button
                className="back-button"
                onClick={() => setCurrentView('lobby')}
                title="返回大厅 (ESC)"
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  width: '28px',
                  height: '28px',
                  borderRadius: '6px',
                  border: 'none',
                  cursor: 'pointer',
                  color: mutedText,
                  background: useColorModeValue('transparent', 'rgba(255,255,255,0.05)'),
                  transition: 'all 0.2s ease',
                }}
                onMouseEnter={(e) => { e.currentTarget.style.background = useColorModeValue('rgba(0,0,0,0.06)', 'rgba(255,255,255,0.1)'); e.currentTarget.style.color = useColorModeValue('gray.700', 'white'); }}
                onMouseLeave={(e) => { e.currentTarget.style.background = useColorModeValue('transparent', 'rgba(255,255,255,0.05)'); e.currentTarget.style.color = mutedText; }}
              >
                <CloseIcon size={16} />
              </button>
            </div>
            {(() => {
              console.log('🎨 [MiniWindow] 正在渲染FileShareManagerNew组件，currentView:', currentView);
              return <FileShareManagerNew />;
            })()}
          </motion.div>
        ) : currentView === 'screenShare' ? (
          <motion.div
            key="screenShare"
            className="screen-share-view"
            initial={{ opacity: 1 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 1 }}
            transition={{ duration: 0 }}
            style={{
              background: useColorModeValue('white', 'linear-gradient(135deg, rgba(20, 20, 30, 0.98) 0%, rgba(25, 25, 35, 0.98) 100%)'),
            }}
          >
            <div
              className="screen-share-header"
              style={{
                background: useColorModeValue('rgba(0,0,0,0.02)', 'rgba(255,255,255,0.03)'),
                borderBottom: `1px solid ${useColorModeValue('rgba(0,0,0,0.08)', 'rgba(255,255,255,0.08)')}`,
              }}
            >
              <div className="screen-share-title-wrapper">
                <h3 className="screen-share-title" style={{ color: textColor }}>屏幕共享</h3>
                <Tooltip
                  title="将您的屏幕实时共享给大厅内的其他玩家查看，支持密码保护。"
                  placement="bottom"
                >
                  <div className="screen-share-info-icon" style={{ color: mutedText }}>
                    <InfoIcon size={14} />
                  </div>
                </Tooltip>
              </div>
              <div className="screen-share-controls">
                <button
                  className="back-button"
                  onClick={() => setCurrentView('lobby')}
                  title="返回大厅 (ESC)"
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    width: '28px',
                    height: '28px',
                    borderRadius: '6px',
                    border: 'none',
                    cursor: 'pointer',
                    color: mutedText,
                    background: useColorModeValue('transparent', 'rgba(255,255,255,0.05)'),
                    transition: 'all 0.2s ease',
                  }}
                  onMouseEnter={(e) => { e.currentTarget.style.background = useColorModeValue('rgba(0,0,0,0.06)', 'rgba(255,255,255,0.1)'); e.currentTarget.style.color = useColorModeValue('gray.700', 'white'); }}
                  onMouseLeave={(e) => { e.currentTarget.style.background = useColorModeValue('transparent', 'rgba(255,255,255,0.05)'); e.currentTarget.style.color = mutedText; }}
                >
                  <CloseIcon size={16} />
                </button>
              </div>
            </div>
            <ScreenShareManager />
          </motion.div>
        ) : (
          <motion.div
            key="lobby"
            className={`mini-window ${collapsed ? 'collapsed' : ''}`}
            style={{
              background: `rgba(${useColorModeValue('255, 255, 255', '20, 20, 30')}, ${opacity})` // 动态设置背景透明度
            }}
            initial={{ opacity: 1 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 1 }}
            transition={{ duration: 0 }}
          >
        <div className={`mini-window-header`}
          style={{
            background: useColorModeValue('rgba(0,0,0,0.02)', 'rgba(255,255,255,0.03)'),
            borderBottom: `1px solid ${useColorModeValue('rgba(0,0,0,0.08)', 'rgba(255,255,255,0.08)')}`,
            padding: '10px 14px',
            cursor: 'move',
          }}
        >
          <Heading as="h3" size="sm" color={textColor} display="flex" alignItems="center" gap={1} className="mini-window-title" style={{ margin: 0 }}>
            {collapsed && lobby ? (
              <>
                {lobby.name.length > 5 ? `${lobby.name.substring(0, 5)}...` : lobby.name} ({players.length + 1}人)
              </>
            ) : (
              '新境盒 | 联机大厅'
            )}
          </Heading>
          <HStack gap={1.5} className="mini-window-controls">
            {/* 收起状态下显示麦克风和听筒按钮 */}
            {collapsed && (
              <>
                <IconButton
                  aria-label={micEnabled ? '关闭麦克风 (Ctrl+M)' : '开启麦克风 (Ctrl+M)'}
                  icon={<MicIcon enabled={micEnabled} size={14} />}
                  size="sm"
                  variant="ghost"
                  colorScheme={!micEnabled ? 'red' : 'gray'}
                  color={!micEnabled ? undefined : btnColor}
                  className="mini-control-btn voice-btn"
                  onClick={handleToggleMic}
                  title={micEnabled ? '关闭麦克风 (Ctrl+M)' : '开启麦克风 (Ctrl+M)'}
                  as={motion.button}
                  whileHover={{ scale: 1.1 }}
                  whileTap={{ scale: 0.95 }}
                />
                <IconButton
                  aria-label={globalMuted ? '开启全局听筒 (Ctrl+T)' : '关闭全局听筒 (Ctrl+T)'}
                  icon={<SpeakerIcon muted={globalMuted} size={14} />}
                  size="sm"
                  variant="ghost"
                  colorScheme={globalMuted ? 'red' : 'gray'}
                  color={globalMuted ? undefined : btnColor}
                  className="mini-control-btn voice-btn"
                  onClick={handleToggleGlobalMute}
                  title={globalMuted ? '开启全局听筒 (Ctrl+T)' : '关闭全局听筒 (Ctrl+T)'}
                  as={motion.button}
                  whileHover={{ scale: 1.1 }}
                  whileTap={{ scale: 0.95 }}
                />
                {/* 新消息按钮 - 有新消息时显示并闪烁 */}
                {unreadCount > 0 && (
                  <IconButton
                    aria-label={`${unreadCount} 条新消息`}
                    icon={
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                        <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path>
                      </svg>
                    }
                    size="sm"
                    variant="ghost"
                    colorScheme="green"
                    className="mini-control-btn new-message-btn"
                    onClick={handleNewMessageClick}
                    title={`${unreadCount} 条新消息`}
                    as={motion.button}
                    whileHover={{ scale: 1.1 }}
                    whileTap={{ scale: 0.95 }}
                    initial={{ scale: 0 }}
                    animate={{ scale: 1 }}
                    exit={{ scale: 0 }}
                  />
                )}
              </>
            )}
            <IconButton
              aria-label="返回主界面"
              icon={<CloseCircleIcon size={16} />}
              size="sm"
              variant="ghost"
              color={btnColor}
              className="mini-control-btn close-btn"
              onClick={handleLeaveLobby}
              title="返回主界面"
              as={motion.button}
              whileHover={{ scale: 1.1 }}
              whileTap={{ scale: 0.95 }}
            />
          </HStack>
        </div>

        <AnimatePresence>
          {!collapsed && (
            <motion.div
              className="mini-window-content"
              initial={{ height: 0, opacity: 0, scale: 0.95 }}
              animate={{ height: 'auto', opacity: 1, scale: 1 }}
              exit={{ height: 0, opacity: 0, scale: 0.95 }}
              transition={{ 
                duration: 0.3, 
                ease: [0.4, 0, 0.2, 1],
                opacity: { duration: 0.2 }
              }}
            >
              {/* 大厅信息卡片 */}
              {lobby && (
                <motion.div
                  className="mini-lobby-card"
                  initial={{ opacity: 0, y: -10 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: 0.1, duration: 0.3 }}
                  style={{ borderRadius: '6px', padding: '8px 10px', background: cardBg, borderWidth: '1px', borderStyle: 'solid', borderColor }}
                >
                  <Flex justify="space-between" align="center" mb={1}>
                    <Heading as="h4" size="md" color={textColor} style={{ margin: 0 }}>
                      {lobby.name.length > 12 ? `${lobby.name.substring(0, 12)}...` : lobby.name}
                    </Heading>
                    <IconButton
                      aria-label="复制大厅信息"
                      icon={
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                          <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                        </svg>
                      }
                      size="xs"
                      variant="ghost"
                      color={btnColor}
                      className="copy-lobby-btn"
                      onClick={handleCopyLobbyInfo}
                      title="复制大厅信息"
                      as={motion.button}
                      whileHover={{ scale: 1.1 }}
                      whileTap={{ scale: 0.95 }}
                    />
                  </Flex>
                  <Flex align="center" gap={1} flexWrap="wrap">
                    <Text fontSize="xs" color={mutedText} fontWeight="medium">
                      {lobby.useDomain && lobby.virtualDomain ? '您的虚拟域名:' : '您的虚拟IP:'}
                    </Text>
                    <Button
                      size="xs"
                      variant="ghost"
                      bg="rgba(0,0,0,0.4)"
                      color="#52c41a"
                      fontFamily="'Consolas', 'Monaco', 'Courier New', monospace"
                      fontWeight="semibold"
                      px={2}
                      py={0}
                      minH="auto"
                      h="auto"
                      lineHeight="1.8"
                      borderRadius="3px"
                      className="virtual-ip-btn"
                      onClick={handleCopyVirtualIp}
                      title={lobby.useDomain && lobby.virtualDomain ? '点击复制虚拟域名' : '点击复制虚拟IP'}
                      as={motion.button}
                      whileHover={{ scale: 1.02 }}
                      whileTap={{ scale: 0.98 }}
                      _hover={{ bg: 'rgba(0,0,0,0.6)', color: '#73d13d' }}
                    >
                      {lobby.useDomain && lobby.virtualDomain ? lobby.virtualDomain : lobby.virtualIp || '获取中...'}
                    </Button>
                    <Button
                      size="xs"
                      variant="link"
                      color={mutedText}
                      fontWeight="medium"
                      ml="auto"
                      className="connection-help-link"
                      onClick={() => setShowConnectionHelp(true)}
                      as={motion.button}
                      whileHover={{ scale: 1.05 }}
                      whileTap={{ scale: 0.95 }}
                      _hover={{ color: 'white' }}
                    >
                      无法联机?
                    </Button>
                  </Flex>
                </motion.div>
              )}

              {/* 玩家列表 */}
              <motion.div
                className="mini-players-section"
                initial={{ opacity: 0, y: -10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.15, duration: 0.3 }}
              >
                <Heading as="h5" size="xs" color={mutedText} textTransform="uppercase" letterSpacing="0.6px" mb={1} style={{ margin: '0 0 3px 0' }}>
                  玩家列表 ({players.length + 1})
                </Heading>
                <div className="mini-player-list">
                  {/* 先显示当前玩家 */}
                  <motion.div
                    className="mini-player-item"
                    initial={{ opacity: 0, x: -20 }}
                    animate={{ opacity: 1, x: 0 }}
                    transition={{ 
                      type: 'spring',
                      stiffness: 500,
                      damping: 30
                    }}
                    style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '6px 8px', borderRadius: '5px', background: mutedBg, border: '1px solid', borderColor: useColorModeValue('rgba(0,0,0,0.06)', 'rgba(255,255,255,0.06)') }}
                  >
                    <Flex align="center" gap={2.5} flex={1} minW={0}>
                      <Box position="relative" w="28px" h="28px" flexShrink={0}>
                        <PlayerIcon className="mini-player-icon" />
                      </Box>
                      <VStack align="flex-start" gap={0.5} flex={1} minW={0}>
                        <Text fontSize="sm" fontWeight="medium" color={textColor} noOfLines={1}>
                          {useAppStore.getState().config.playerName || '我'} (我)
                        </Text>
                        <Button
                          size="xs"
                          variant="ghost"
                          bg="rgba(0,0,0,0.3)"
                          color="#52c41a"
                          fontFamily="'Consolas', 'Monaco', 'Courier New', monospace"
                          fontWeight="medium"
                          px={2}
                          py={0}
                          minH="auto"
                          h="auto"
                          lineHeight="1.8"
                          borderRadius="3px"
                          alignSelf="flex-start"
                          className="player-virtual-ip-btn"
                          onClick={handleCopyVirtualIp}
                          title={lobby?.useDomain && lobby?.virtualDomain ? '点击复制虚拟域名' : '点击复制虚拟IP'}
                          as={motion.button}
                          whileHover={{ scale: 1.02 }}
                          whileTap={{ scale: 0.98 }}
                          _hover={{ bg: 'rgba(0,0,0,0.5)', color: '#73d13d' }}
                        >
                          {lobby?.useDomain && lobby?.virtualDomain 
                            ? `虚拟域名: ${lobby.virtualDomain}` 
                            : `虚拟IP: ${lobby?.virtualIp || '10.126.126.1'}`
                          }
                        </Button>
                      </VStack>
                    </Flex>
                  </motion.div>

                  {/* 显示其他玩家 */}
                  <AnimatePresence mode="popLayout">
                    {players.map((player) => {
                      // 判断该玩家是否被静音（考虑全局静音和单独静音）
                      const isPlayerMuted = globalMuted || mutedPlayers.has(player.id);
                      // 获取该玩家的音量设置
                      const playerVolume = getPlayerVolume(player.id);
                      
                      return (
                        <motion.div
                          key={player.id}
                          className="mini-player-item"
                          layout
                          initial={{ opacity: 0, x: -20, scale: 0.9 }}
                          animate={{ opacity: 1, x: 0, scale: 1 }}
                          exit={{ opacity: 0, x: 20, scale: 0.9 }}
                          transition={{ 
                            duration: 0.3,
                            ease: [0.4, 0, 0.2, 1]
                          }}
                          style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '6px 8px', borderRadius: '5px', background: mutedBg, border: '1px solid', borderColor: useColorModeValue('rgba(0,0,0,0.06)', 'rgba(255,255,255,0.06)') }}
                        >
                          <Flex align="center" gap={2.5} flex={1} minW={0}>
                            <Box position="relative" w="28px" h="28px" flexShrink={0}>
                              <PlayerIcon className="mini-player-icon" />
                            </Box>
                            <VStack align="flex-start" gap={0.5} flex={1} minW={0}>
                              <Text fontSize="sm" fontWeight="medium" color={textColor} noOfLines={1}>
                                {player.name}
                              </Text>
                              <Button
                                size="xs"
                                variant="ghost"
                                bg="rgba(0,0,0,0.3)"
                                color="#52c41a"
                                fontFamily="'Consolas', 'Monaco', 'Courier New', monospace"
                                fontWeight="medium"
                                px={2}
                                py={0}
                                minH="auto"
                                h="auto"
                                lineHeight="1.8"
                                borderRadius="3px"
                                alignSelf="flex-start"
                                className="player-virtual-ip-btn"
                                onClick={async () => {
                                  try {
                                    const textToCopy = (player.useDomain && player.virtualDomain) 
                                      ? player.virtualDomain 
                                      : (player.virtualIp || lobby?.virtualIp || '10.126.126.1');
                                    await writeText(textToCopy);
                                    const label = (player.useDomain && player.virtualDomain) ? '虚拟域名' : '虚拟IP';
                                    toast({ title: `${label}已复制`, status: 'success', duration: 2000, isClosable: true, position: 'top' });
                                  } catch (error) {
                                      console.error('复制失败:', error);
                                      toast({ title: '复制失败，请重试', status: 'error', duration: 3000, isClosable: true, position: 'top' });
                                  }
                                }}
                                title={(player.useDomain && player.virtualDomain) ? '点击复制虚拟域名' : '点击复制虚拟IP'}
                                as={motion.button}
                                whileHover={{ scale: 1.02 }}
                                whileTap={{ scale: 0.98 }}
                                _hover={{ bg: 'rgba(0,0,0,0.5)', color: '#73d13d' }}
                              >
                                {(player.useDomain && player.virtualDomain)
                                  ? `虚拟域名: ${player.virtualDomain}` 
                                  : `虚拟IP: ${player.virtualIp || lobby?.virtualIp || '10.126.126.1'}`
                                }
                              </Button>
                              {/* 玩家独立音量控制 */}
                              <HStack gap={1.5} mt={0.5}>
                                <SpeakerIcon muted={isPlayerMuted} size={12} />
                                <Slider
                                  aria-label={`player-volume-${player.id}`}
                                  value={playerVolume}
                                  min={0}
                                  max={1}
                                  step={0.05}
                                  onChange={(val) => handlePlayerVolumeChange(player.id, val)}
                                  isDisabled={isPlayerMuted}
                                  w="80px"
                                  h="4px"
                                >
                                  <SliderTrack bg="rgba(255,255,255,0.1)">
                                    <SliderFilledTrack bg="#52c41a" />
                                  </SliderTrack>
                                  <SliderThumb boxSize="12px" bg="#52c41a" />
                                </Slider>
                                <Text fontSize="xs" color={mutedText} fontWeight="medium" minW="32px" textAlign="right">
                                  {Math.round(playerVolume * 100)}%
                                </Text>
                              </HStack>
                            </VStack>
                          </Flex>
                          <HStack gap={0.5}>
                            <IconButton
                              aria-label={isPlayerMuted ? '取消静音' : '静音此玩家'}
                              icon={<SpeakerIcon muted={isPlayerMuted} size={16} />}
                              size="xs"
                              variant="ghost"
                              color={isPlayerMuted ? '#ef4444' : 'white'}
                              className={`mini-action-btn ${isPlayerMuted ? 'muted' : ''}`}
                              onClick={() => handleMutePlayer(player.id)}
                              title={isPlayerMuted ? '取消静音' : '静音此玩家'}
                              as={motion.button}
                              whileHover={{ scale: 1.1 }}
                              whileTap={{ scale: 0.9 }}
                            />
                          </HStack>
                        </motion.div>
                      );
                    })}
                  </AnimatePresence>
                </div>
              </motion.div>

              {/* 大厅动态设置 */}
              <motion.div
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.2, duration: 0.3 }}
                style={{ display: 'flex', justifyContent: 'flex-end', padding: '4px 0' }}
              >
                <IconButton
                  aria-label="大厅动态设置"
                  icon={
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                      <path d="M12 15.5A3.5 3.5 0 0 1 8.5 12 3.5 3.5 0 0 1 12 8.5a3.5 3.5 0 0 1 3.5 3.5 3.5 3.5 0 0 1-3.5 3.5m7.43-2.92c.04-.34.07-.69.07-1.08s-.03-.74-.07-1.08l2.32-1.82c.21-.17.27-.46.13-.7l-2.2-3.81c-.13-.24-.41-.32-.65-.24l-2.74 1.1c-.57-.44-1.18-.81-1.86-1.09L14.05 2.1c-.04-.27-.28-.46-.55-.46h-3c-.28 0-.5.19-.55.46L9.5 4.86C8.82 5.14 8.2 5.5 7.64 5.95L4.9 4.85c-.24-.09-.52 0-.65.24L2.05 8.9c-.14.24-.08.53.13.7L4.5 11.5c-.04.34-.07.7-.07 1.08s.03.74.07 1.08L2.18 15.48c-.21.17-.27.46-.13.7l2.2 3.81c.13.24.41.32.65.24l2.74-1.1c.57.44 1.18.81 1.86 1.09l.45 2.76c.05.27.27.46.55.46h3c.28 0 .5-.19.55-.46l.45-2.76c.68-.28 1.3-.65 1.86-1.09l2.74 1.1c.24.09.52 0 .65-.24l2.2-3.81c.14-.24.08-.53-.13-.7l-2.32-1.9z" />
                    </svg>
                  }
                  size="sm"
                  variant="ghost"
                  color={iconColor}
                  className="mini-lobby-settings-btn"
                  onClick={() => setShowLobbySettings(true)}
                  title="大厅动态设置"
                  as={motion.button}
                  whileHover={{ scale: 1.1 }}
                  whileTap={{ scale: 0.95 }}
                />
              </motion.div>

              {/* 底部控制按钮 - 只有5个按钮 */}
              <motion.div
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.25, duration: 0.3 }}
                style={{ padding: '8px', borderRadius: '6px', display: 'flex', justifyContent: 'center', gap: '8px', background: mutedBg, borderWidth: '1px', borderStyle: 'solid', borderColor }}
              >
                <IconButton
                  aria-label={micEnabled ? '关闭麦克风 (Ctrl+M)' : '开启麦克风 (Ctrl+M)'}
                  icon={<MicIcon enabled={micEnabled} size={24} />}
                  variant="ghost"
                  className="mini-voice-btn"
                  onClick={handleToggleMic}
                  title={micEnabled ? '关闭麦克风 (Ctrl+M)' : '开启麦克风 (Ctrl+M)'}
                  colorScheme={!micEnabled ? 'red' : 'gray'}
                  as={motion.button}
                  whileHover={{ scale: 1.1 }}
                  whileTap={{ scale: 0.95 }}
                  w="48px"
                  h="48px"
                  borderRadius="full"
                  bg={!micEnabled ? 'rgba(239,68,68,0.3)' : useColorModeValue('rgba(0,0,0,0.06)', 'rgba(255,255,255,0.08)')}
                  color={!micEnabled ? '#ef4444' : btnColor}
                  _hover={{ bg: !micEnabled ? 'rgba(239,68,68,0.45)' : useColorModeValue('rgba(0,0,0,0.1)', 'rgba(255,255,255,0.15)') }}
                  boxShadow="0 2px 8px rgba(0,0,0,0.15)"
                />
                <IconButton
                  aria-label={globalMuted ? '开启全局听筒 (Ctrl+T)' : '关闭全局听筒 (Ctrl+T)'}
                  icon={<SpeakerIcon muted={globalMuted} size={24} />}
                  variant="ghost"
                  className="mini-voice-btn"
                  onClick={handleToggleGlobalMute}
                  title={globalMuted ? '开启全局听筒 (Ctrl+T)' : '关闭全局听筒 (Ctrl+T)'}
                  colorScheme={globalMuted ? 'red' : 'gray'}
                  as={motion.button}
                  whileHover={{ scale: 1.1 }}
                  whileTap={{ scale: 0.95 }}
                  w="48px"
                  h="48px"
                  borderRadius="full"
                  bg={globalMuted ? 'rgba(239,68,68,0.3)' : useColorModeValue('rgba(0,0,0,0.06)', 'rgba(255,255,255,0.08)')}
                  color={globalMuted ? '#ef4444' : btnColor}
                  _hover={{ bg: globalMuted ? 'rgba(239,68,68,0.45)' : useColorModeValue('rgba(0,0,0,0.1)', 'rgba(255,255,255,0.15)') }}
                  boxShadow="0 2px 8px rgba(0,0,0,0.15)"
                />
                <IconButton
                  aria-label="聊天室"
                  icon={
                    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path>
                    </svg>
                  }
                  variant="ghost"
                  className={`mini-voice-btn chat-btn ${unreadCount > 0 ? 'has-unread' : ''}`}
                  onClick={handleOpenChatRoom}
                  title="聊天室"
                  as={motion.button}
                  whileHover={{ scale: 1.1 }}
                  whileTap={{ scale: 0.95 }}
                  w="48px"
                  h="48px"
                  borderRadius="full"
                  bg={unreadCount > 0 ? 'rgba(82,196,26,0.3)' : useColorModeValue('rgba(0,0,0,0.06)', 'rgba(255,255,255,0.08)')}
                  color={unreadCount > 0 ? '#52c41a' : btnColor}
                  _hover={{ bg: unreadCount > 0 ? 'rgba(82,196,26,0.45)' : useColorModeValue('rgba(0,0,0,0.1)', 'rgba(255,255,255,0.15)') }}
                  boxShadow="0 2px 8px rgba(0,0,0,0.15)"
                />
                <IconButton
                  aria-label="文件夹共享"
                  icon={
                    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
                    </svg>
                  }
                  variant="ghost"
                  className="mini-voice-btn file-share-btn"
                  onClick={() => {
                    console.log('🖱️ [MiniWindow] 点击文件共享按钮，切换视图到fileShare');
                    setCurrentView('fileShare');
                  }}
                  title="文件夹共享"
                  as={motion.button}
                  whileHover={{ scale: 1.1 }}
                  whileTap={{ scale: 0.95 }}
                  w="48px"
                  h="48px"
                  borderRadius="full"
                  bg={useColorModeValue('rgba(0,0,0,0.06)', 'rgba(255,255,255,0.08)')}
                  color={btnColor}
                  _hover={{ bg: useColorModeValue('rgba(0,0,0,0.1)', 'rgba(255,255,255,0.15)') }}
                  boxShadow="0 2px 8px rgba(0,0,0,0.15)"
                />
                <IconButton
                  aria-label="屏幕共享"
                  icon={<ScreenShareIcon size={24} />}
                  variant="ghost"
                  className="mini-voice-btn screen-share-btn"
                  onClick={() => {
                    console.log('🖱️ [MiniWindow] 点击屏幕共享按钮，切换视图到screenShare');
                    setCurrentView('screenShare');
                  }}
                  title="屏幕共享"
                  as={motion.button}
                  whileHover={{ scale: 1.1 }}
                  whileTap={{ scale: 0.95 }}
                  w="48px"
                  h="48px"
                  borderRadius="full"
                  bg={useColorModeValue('rgba(0,0,0,0.06)', 'rgba(255,255,255,0.08)')}
                  color={btnColor}
                  _hover={{ bg: useColorModeValue('rgba(0,0,0,0.1)', 'rgba(255,255,255,0.15)') }}
                  boxShadow="0 2px 8px rgba(0,0,0,0.15)"
                />
              </motion.div>

              {/* 快捷键提示已移除 */}
            </motion.div>
          )}
        </AnimatePresence>
        </motion.div>
        )}
      </AnimatePresence>

      {/* 大厅动态设置弹窗 */}
      {lobby && (
        <LobbySettingsModal
          visible={showLobbySettings}
          onClose={() => setShowLobbySettings(false)}
          currentLobby={{
            name: lobby.name || '',
            password: lobby.password || '',
            virtualIp: lobby.virtualIp || '',
          }}
          onSettingsSaved={handleLobbySettingsSaved}
        />
      )}
    </>
  );
};
