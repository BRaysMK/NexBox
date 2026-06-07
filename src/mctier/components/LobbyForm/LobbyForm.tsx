import React, { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Box,
  VStack,
  HStack,
  Flex,
  FormControl,
  FormLabel,
  Input,
  Button,
  Text,
  useToast,
  IconButton,
  Tooltip,
  Modal,
  ModalOverlay,
  ModalContent,
  ModalHeader,
  ModalBody,
  ModalFooter,
  useColorModeValue,
  InputGroup,
  InputRightElement,
  Menu,
  MenuButton,
  MenuList,
  MenuItem,
} from '@chakra-ui/react';
import { invoke } from '@tauri-apps/api/core';
import { readText } from '@tauri-apps/plugin-clipboard-manager';
import { useAppStore } from '../../stores';
import type { Lobby, UserConfig } from '../../types';
import { WarningIcon, StarIcon, DiceIcon } from '../icons';
import { Switch as AntSwitch } from 'antd';
import { useEscapeKey } from '../../hooks';
import { FavoriteLobbyManager, type FavoriteLobby } from '../FavoriteLobbyManager/FavoriteLobbyManager';
import { useThemeColor } from '@/contexts/theme-color-context';
import { ViewIcon, ViewOffIcon } from '../icons';
import './LobbyForm.css';

interface LobbyFormProps {
  mode: 'create' | 'join';
  onClose: () => void;
}

interface LobbyFormValues {
  lobbyName: string;
  password: string;
  playerName: string;
  serverNode: string;
  customEasytierServer?: string;
  customSignalingServer?: string;
  useDomain: boolean;
}

// 官方 EasyTier 服务器节点（仅保留 CDN WebSockets）
const OFFICIAL_EASYTIER_SERVER = 'wss://mctiers.pmhs.top';

// 默认备用节点（与SettingsWindow中的定义保持一致）
const DEFAULT_BUILTIN_NODE = {
  name: '明月清风节点',
  address: 'wss://qtet-public.070219.xyz'
};

// EasyTier 公共节点
const EASYTIER_PUBLIC_NODES = [
  { name: 'EasyTier 公共节点 (zkitefly)', address: 'https://etnode.zkitefly.eu.org/node1' },
];

// 旧版官方节点（用于兼容历史配置，自动迁移到 WebSockets 节点）
const isLegacyOfficialServer = (server?: string) => {
  if (!server) return false;
  return (
    server === 'tcp://mctier.pmhs.top:11010' ||
    server === 'udp://mctier.pmhs.top:11010' ||
    server === 'wss://mctier.pmhs.top/signaling' ||
    server === 'ws://mctier.pmhs.top/signaling'
  );
};

// 自定义节点接口
interface CustomEasyTierNode {
  name: string;
  address: string;
}

// 获取服务器节点列表（包含官方节点、EasyTier公共节点、默认备用节点和自定义节点）
const getServerNodes = (customNodes: CustomEasyTierNode[]) => {
  const nodes = [
    { value: OFFICIAL_EASYTIER_SERVER, label: 'MCTier 官方服务器 (WebSockets)' },
  ];

  // 添加 EasyTier 官方公共节点
  EASYTIER_PUBLIC_NODES.forEach((node) => {
    nodes.push({
      value: node.address,
      label: `${node.name} (公共)`,
    });
  });

  // 添加备用节点
  nodes.push({
    value: DEFAULT_BUILTIN_NODE.address,
    label: `${DEFAULT_BUILTIN_NODE.name} (备用)`,
  });

  // 添加自定义节点
  customNodes.forEach((node) => {
    nodes.push({
      value: node.address,
      label: `${node.name} (自定义)`,
    });
  });

  nodes.push({ value: 'custom', label: '临时自定义服务器地址' });

  return nodes;
};

// 随机生成大厅名称的词库
const LOBBY_NAME_ADJECTIVES = [
  '快乐', '欢乐', '神秘', '梦幻', '传奇', '史诗', '超级', '极限',
  '无敌', '王者', '至尊', '荣耀', '辉煌', '璀璨', '闪耀', '炫酷',
  '疯狂', '狂野', '激情', '热血', '勇敢', '无畏', '坚韧', '强大',
  '幸运', '吉祥', '福星', '瑞雪', '春风', '夏日', '秋月', '冬雪',
];

const LOBBY_NAME_NOUNS = [
  '冒险', '探险', '旅程', '征途', '远征', '奇遇', '传说', '神话',
  '世界', '王国', '帝国', '领域', '天堂', '乐园', '家园', '基地',
  '联盟', '公会', '战队', '军团', '部落', '氏族', '家族', '团队',
  '小队', '组织', '势力', '阵营', '派系', '集团', '协会', '社团',
];

/**
 * 生成随机大厅名称
 */
const generateRandomLobbyName = (): string => {
  const adjective = LOBBY_NAME_ADJECTIVES[Math.floor(Math.random() * LOBBY_NAME_ADJECTIVES.length)];
  const noun = LOBBY_NAME_NOUNS[Math.floor(Math.random() * LOBBY_NAME_NOUNS.length)];
  const number = Math.floor(Math.random() * 1000);
  return `${adjective}的${noun}${number}`;
};

/**
 * 生成随机密码
 * 包含大小写字母和数字，长度12位
 */
const generateRandomPassword = (): string => {
  const lowercase = 'abcdefghijklmnopqrstuvwxyz';
  const uppercase = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ';
  const numbers = '0123456789';
  const allChars = lowercase + uppercase + numbers;

  let password = '';

  // 确保至少包含一个小写字母、一个大写字母和一个数字
  password += lowercase[Math.floor(Math.random() * lowercase.length)];
  password += uppercase[Math.floor(Math.random() * uppercase.length)];
  password += numbers[Math.floor(Math.random() * numbers.length)];

  // 填充剩余字符
  for (let i = 3; i < 12; i++) {
    password += allChars[Math.floor(Math.random() * allChars.length)];
  }

  // 打乱顺序
  return password.split('').sort(() => Math.random() - 0.5).join('');
};

/**
 * 大厅表单组件
 * 用于创建或加入大厅
 */
export const LobbyForm: React.FC<LobbyFormProps> = ({ mode, onClose }) => {
  const toast = useToast();
  const { setAppState, setLobby, config } = useAppStore();
  const { getActiveColor, getHoverColor, getBorderColor, getContrastTextColor } = useThemeColor();

  const [loading, setLoading] = useState(false);
  const [showCustomServer, setShowCustomServer] = useState(config.preferredServer === 'custom');
  const [showFavoritesModal, setShowFavoritesModal] = useState(false);
  const [showPassword, setShowPassword] = useState(false);

  // 表单状态
  const [formValues, setFormValues] = useState<Partial<LobbyFormValues>>({
    playerName: config.playerName || '',
    useDomain: false,
  });
  const [errors, setErrors] = useState<Record<string, string>>({});

  const [privateServerConfig, setPrivateServerConfig] = useState<{
    usePrivateServer: boolean;
    privateEasytierServer: string;
    privateSignalingServer: string;
  }>({
    usePrivateServer: false,
    privateEasytierServer: 'wss://mctiers.pmhs.top',
    privateSignalingServer: 'wss://mctier.pmhs.top/signaling',
  });
  // @ts-ignore - customNodes is used in useEffect to load custom nodes
  const [customNodes, setCustomNodes] = useState<CustomEasyTierNode[]>([]);
  const [serverNodes, setServerNodes] = useState(getServerNodes([]));

  // 滚动提示相关状态
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const [showScrollHint, setShowScrollHint] = useState(false);
  const [canScroll, setCanScroll] = useState(false);

  // 错误弹窗状态
  const [errorModal, setErrorModal] = useState<{ title: string; content: React.ReactNode; type: 'error' | 'warning'; onOk?: () => void } | null>(null);

  // 主题色
  const primaryColor = getActiveColor();
  const hoverBg = getHoverColor(true);
  const borderColor = getBorderColor();
  const contrastText = getContrastTextColor();

  const cardBg = useColorModeValue('white', '#111111');
  const cardBorder = useColorModeValue('gray.200', '#333333');
  const labelColor = useColorModeValue('gray.700', 'rgba(255,255,255,0.9)');
  const inputBg = useColorModeValue('gray.50', 'rgba(255,255,255,0.05)');
  const textColor = useColorModeValue('gray.800', 'rgba(255,255,255,0.9)');
  const mutedTextColor = useColorModeValue('gray.500', 'rgba(255,255,255,0.6)');
  
  // 下拉菜单专用颜色（适配亮暗主题）
  const menuBg = useColorModeValue('white', '#1a1a2e');
  const menuBorderColor = useColorModeValue('gray.200', 'rgba(255,255,255,0.1)');
  const menuHoverBg = useColorModeValue('gray.100', `${primaryColor}15`);
  const menuSelectedBg = useColorModeValue('gray.100', `${primaryColor}20`);
  const menuDividerColor = useColorModeValue('gray.200', 'rgba(255,255,255,0.08)');
  const menuShadow = useColorModeValue(
    '0 8px 32px rgba(0,0,0,0.15)',
    '0 8px 32px rgba(0,0,0,0.5)'
  );
  
  // 网络提示区域专用颜色（固定颜色，确保对比度）
  const tipBg = useColorModeValue('rgba(240, 240, 240, 0.7)', 'rgba(40, 40, 40, 0.7)');
  const tipBorder = useColorModeValue('#e0e0e0', '#444444');
  const tipText = useColorModeValue('#666666', 'rgba(255,255,255,0.8)');
  const tipStrong = useColorModeValue('#333333', 'rgba(255,255,255,0.95)');

  // ESC键返回
  useEscapeKey(() => {
    if (!loading) {
      handleCancel();
    }
  });

  // 检查是否可以滚动
  useEffect(() => {
    const checkScroll = () => {
      if (scrollContainerRef.current) {
        const { scrollHeight, clientHeight } = scrollContainerRef.current;
        const hasScroll = scrollHeight > clientHeight;
        setCanScroll(hasScroll);
        setShowScrollHint(hasScroll);
      }
    };

    // 初始检查
    checkScroll();

    // 监听窗口大小变化
    window.addEventListener('resize', checkScroll);

    // 延迟检查，确保内容已渲染
    const timer = setTimeout(checkScroll, 500);

    return () => {
      window.removeEventListener('resize', checkScroll);
      clearTimeout(timer);
    };
  }, [showCustomServer, privateServerConfig.usePrivateServer]);

  // 监听滚动事件，滚动后隐藏提示
  useEffect(() => {
    const handleScroll = () => {
      if (scrollContainerRef.current) {
        const { scrollTop } = scrollContainerRef.current;
        if (scrollTop > 20) {
          setShowScrollHint(false);
        }
      }
    };

    const container = scrollContainerRef.current;
    if (container) {
      container.addEventListener('scroll', handleScroll);
      return () => container.removeEventListener('scroll', handleScroll);
    }
  }, []);

  // 更新表单字段值
  const updateField = <K extends keyof LobbyFormValues>(field: K, value: LobbyFormValues[K]) => {
    setFormValues((prev) => ({ ...prev, [field]: value }));
    // 清除该字段的错误
    if (errors[field]) {
      setErrors((prev) => {
        const next = { ...prev };
        delete next[field];
        return next;
      });
    }
  };

  // 验证单个字段
  const validateField = (field: keyof LobbyFormValues, value: string): string | null => {
    switch (field) {
      case 'lobbyName':
        if (!value?.trim()) return '请输入大厅名称';
        if (!value.trim()) return '大厅名称不能为空白字符';
        if (value.trim().length < 4 || value.trim().length > 32) return '大厅名称长度为 4-32 个字符';
        if (!/^[\u4e00-\u9fa5a-zA-Z0-9_\-\s]+$/.test(value.trim())) return '大厅名称只能包含中文、字母、数字、下划线、连字符和空格';
        if (!/[a-zA-Z0-9\u4e00-\u9fa5]/.test(value.trim())) return '大厅名称必须包含至少一个字母或数字';
        return null;
      case 'password':
        if (!value?.trim()) return '请输入密码';
        if (!value.trim()) return '密码不能为空白字符';
        if (value.trim().length < 8 || value.trim().length > 32) return '密码长度为 8-32 个字符';
        if (!/[a-zA-Z]/.test(value.trim())) return '密码必须包含至少一个字母';
        if (!/[0-9]/.test(value.trim())) return '密码必须包含至少一个数字';
        return null;
      case 'playerName':
        if (!value?.trim()) return '请输入玩家名称';
        if (!value.trim()) return '玩家名称不能为空白字符';
        if (value.trim().length < 1 || value.trim().length > 8) return '玩家名称长度为 1-8 个字';
        return null;
      case 'serverNode':
        if (!value) return '请选择服务器节点';
        return null;
      default:
        return null;
    }
  };

  // 验证所有字段
  const validateAll = (): boolean => {
    const newErrors: Record<string, string> = {};

    const lobbyNameErr = validateField('lobbyName', formValues.lobbyName || '');
    if (lobbyNameErr) newErrors.lobbyName = lobbyNameErr;

    const passwordErr = validateField('password', formValues.password || '');
    if (passwordErr) newErrors.password = passwordErr;

    const playerNameErr = validateField('playerName', formValues.playerName || '');
    if (playerNameErr) newErrors.playerName = playerNameErr;

    const serverNodeErr = validateField('serverNode', formValues.serverNode || '');
    if (serverNodeErr) newErrors.serverNode = serverNodeErr;

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  // 一键随机生成大厅名称和密码
  const handleRandomGenerate = () => {
    const lobbyName = generateRandomLobbyName();
    const password = generateRandomPassword();

    setFormValues((prev) => ({
      ...prev,
      lobbyName,
      password,
    }));

    toast({
      title: '已随机生成大厅名称和密码',
      status: 'success',
      duration: 2000,
      isClosable: true,
    });
  };

  // 处理选择常用大厅
  const handleSelectFavorite = (lobby: FavoriteLobby) => {
    setFormValues((prev) => ({
      ...prev,
      lobbyName: lobby.name,
      password: lobby.password,
      playerName: lobby.playerName || config.playerName || '',
      useDomain: lobby.useDomain ?? false,
    }));
  };

  const resolvedPreferredServer =
    config.preferredServer === 'custom'
      ? 'custom'
      : isLegacyOfficialServer(config.preferredServer)
        ? OFFICIAL_EASYTIER_SERVER
        : config.preferredServer === OFFICIAL_EASYTIER_SERVER
          ? OFFICIAL_EASYTIER_SERVER
          : OFFICIAL_EASYTIER_SERVER;

  // 初始化服务器节点
  useEffect(() => {
    setFormValues((prev) => ({
      ...prev,
      serverNode: resolvedPreferredServer,
    }));
  }, []);

  // 组件加载时尝试从剪贴板自动识别大厅信息
  useEffect(() => {
    const autoFillFromClipboard = async () => {
      // 只在加入大厅模式下自动识别
      if (mode !== 'join') return;

      await recognizeClipboard(true); // 传入 true 表示是自动识别，不显示"剪贴板为空"提示
    };

    autoFillFromClipboard();
  }, [mode]);

  // 加载私有服务器配置和自定义节点
  useEffect(() => {
    const loadPrivateServerConfig = async () => {
      try {
        const settings = await invoke<any>('get_settings');
        setPrivateServerConfig({
          usePrivateServer: settings.usePrivateServer || false,
          // 使用 ?? 运算符，只在 null/undefined 时使用默认值
          privateEasytierServer: settings.privateEasytierServer ?? 'wss://mctiers.pmhs.top',
          privateSignalingServer: settings.privateSignalingServer ?? 'wss://mctier.pmhs.top/signaling',
        });

        // 加载自定义节点
        const nodes = settings.customEasytierNodes || [];
        setCustomNodes(nodes);
        setServerNodes(getServerNodes(nodes));

        console.log('已加载私有服务器配置:', settings);
        console.log('已加载自定义节点:', nodes);
      } catch (error) {
        console.error('加载私有服务器配置失败:', error);
      }
    };

    loadPrivateServerConfig();
  }, []);

  // 检测自动大厅配置，自动填充并提交
  useEffect(() => {
    const autoConfig = (window as any).__autoLobbyConfig;
    // 没有配置或不是创建模式就跳过
    if (!autoConfig || mode !== 'create') return;
    // 立即清除，防止重复触发
    delete (window as any).__autoLobbyConfig;
    const { lobbyName, lobbyPassword, playerName, useDomain } = autoConfig;
    setFormValues({
      lobbyName,
      password: lobbyPassword,
      playerName,
      useDomain: useDomain || false,
      serverNode: resolvedPreferredServer,
    });
    setTimeout(() => {
      handleSubmit();
    }, 300);
  }, [mode, config.preferredServer]);

  // 从剪贴板识别大厅信息的函数
  const recognizeClipboard = async (isAuto = false) => {
    try {
      const clipboardText = await readText();
      if (!clipboardText) {
        // 只在手动识别时提示剪贴板为空
        if (!isAuto) {
          toast({ title: '剪贴板为空', status: 'info', duration: 2000, isClosable: true });
        }
        return;
      }

      console.log('读取到剪贴板内容:', clipboardText);

      // 新格式匹配
      const lobbyNameMatch = clipboardText.match(/大厅名称：([^\r\n]+)/);
      const passwordMatch = clipboardText.match(/密码：([^\r\n]*)/);

      if (lobbyNameMatch && passwordMatch) {
        const lobbyName = lobbyNameMatch[1].trim();
        const password = passwordMatch[1].trim();

        console.log('匹配到大厅信息:', { lobbyName, password: password ? '***' : '(空)' });

        if (lobbyName.length >= 4 && password.length >= 8) {
          setFormValues((prev) => ({ ...prev, lobbyName, password }));
          toast({ title: '已自动识别并填写大厅信息', status: 'success', duration: 2000, isClosable: true });
          console.log('自动填写大厅信息成功');
          return;
        }
      }

      // 兼容旧格式：大厅名称|密码
      const parts = clipboardText.split('|');
      if (parts.length === 2) {
        const [lobbyName, password] = parts;

        if (lobbyName.trim().length >= 4 && password.trim().length >= 8) {
          setFormValues((prev) => ({ ...prev, lobbyName: lobbyName.trim(), password: password.trim() }));
          toast({ title: '已自动识别并填写大厅信息', status: 'success', duration: 2000, isClosable: true });
          console.log('自动填写大厅信息（旧格式）成功');
          return;
        }
      }

      // 如果没有匹配到任何格式，只在手动识别时提示
      if (!isAuto) {
        toast({ title: '剪贴板中没有识别到有效的大厅信息', status: 'warning', duration: 2000, isClosable: true });
      }
    } catch (error) {
      console.log('无法读取剪贴板或格式不匹配:', error);
      if (!isAuto) {
        toast({ title: '读取剪贴板失败，请检查权限', status: 'error', duration: 2000, isClosable: true });
      }
    }
  };

  const handleSubmit = async () => {
    // 先验证
    if (!validateAll()) {
      return;
    }

    try {
      setLoading(true);
      setAppState('connecting');

      const values = formValues as Required<LobbyFormValues>;

      // 确定实际使用的服务器地址
      let serverNode = values.serverNode;
      let signalingServer = 'wss://mctier.pmhs.top/signaling'; // 默认官方信令服务器

      // 如果启用了私有服务器，使用私有服务器配置（不添加默认备用节点）
      if (privateServerConfig.usePrivateServer) {
        serverNode = privateServerConfig.privateEasytierServer;
        signalingServer = privateServerConfig.privateSignalingServer;
        console.log('========================================');
        console.log('✅ 使用私有服务器配置（不添加默认备用节点）');
        console.log('  EasyTier 节点服务器:', serverNode);
        console.log('  信令服务器:', signalingServer);
        console.log('========================================');
      } else if (values.serverNode === 'custom') {
        // 使用临时自定义服务器（不添加默认备用节点）
        if (!values.customEasytierServer?.trim()) {
          toast({ title: '请输入 EasyTier 节点服务器地址', status: 'error', duration: 3000, isClosable: true });
          return;
        }
        if (!values.customSignalingServer?.trim()) {
          toast({ title: '请输入信令服务器地址', status: 'error', duration: 3000, isClosable: true });
          return;
        }
        serverNode = values.customEasytierServer.trim();
        signalingServer = values.customSignalingServer.trim();
        console.log('========================================');
        console.log('✅ 使用临时自定义服务器（不添加默认备用节点）');
        console.log('  EasyTier 节点服务器:', serverNode);
        console.log('  信令服务器:', signalingServer);
        console.log('========================================');
      } else {
        // 使用官方服务器或自定义节点（单节点模式）
        serverNode = values.serverNode;
        console.log('========================================');
        console.log('✅ 使用单节点模式');
        console.log('  EasyTier 节点服务器:', serverNode);
        console.log('  信令服务器:', signalingServer);
        console.log('========================================');
      }

      const commandName = mode === 'create' ? 'create_lobby' : 'join_lobby';

      // 获取当前玩家ID，如果不存在则生成一个新的
      let { currentPlayerId } = useAppStore.getState();

      if (!currentPlayerId) {
        const timestamp = Date.now();
        const randomSuffix = Math.random().toString(36).substring(2, 11);
        currentPlayerId = `player-${timestamp}-${randomSuffix}`;

        const { setCurrentPlayerId } = useAppStore.getState();
        setCurrentPlayerId(currentPlayerId);

        console.log('⚠️ playerId 不存在，已生成新的 ID:', currentPlayerId);
      }

      // 从配置中读取虚拟域名（添加超时保护）
      let virtualDomain: string | undefined = undefined;
      try {
        console.log('正在读取虚拟域名配置...');
        const settingsPromise = invoke<any>('get_settings');
        const timeoutPromise = new Promise((_, reject) =>
          setTimeout(() => reject(new Error('读取配置超时')), 3000)
        );

        const settings = await Promise.race([settingsPromise, timeoutPromise]) as any;
        virtualDomain = settings.virtualDomain || undefined;
        console.log('从配置中读取虚拟域名:', virtualDomain);
      } catch (error) {
        console.warn('读取虚拟域名配置失败:', error);
        virtualDomain = undefined;
      }

      console.log('准备调用后端命令:', commandName);
      console.log('参数:', {
        name: values.lobbyName.trim(),
        playerName: values.playerName.trim(),
        playerId: currentPlayerId,
        serverNode: serverNode,
        signalingServer: signalingServer,
        useDomain: values.useDomain === true,
        virtualDomain: virtualDomain,
      });

      // 调用后端命令
      const lobby = await invoke<Lobby>(commandName, {
        name: values.lobbyName.trim(),
        password: values.password.trim(),
        playerName: values.playerName.trim(),
        playerId: currentPlayerId,
        serverNode: serverNode,
        signalingServer: signalingServer,
        useDomain: values.useDomain === true,
        virtualDomain: virtualDomain,
      });

      console.log('✅ 后端命令调用成功，返回的大厅信息:', lobby);

      // 保存玩家名称到前端store
      const { updateConfig } = useAppStore.getState();
      updateConfig({ playerName: values.playerName.trim() });

      // 保存玩家名称到后端配置文件
      try {
        const currentConfig = await invoke<UserConfig>('get_config');
        await invoke('update_config', {
          config: {
            ...currentConfig,
            playerName: values.playerName.trim(),
          },
        });
        console.log('玩家名称已保存到配置文件');
      } catch (error) {
        console.warn('保存玩家名称到配置文件失败:', error);
      }

      console.log('✅ 大厅创建/加入成功，HTTP文件服务器将在添加共享时按需启动');

      // 更新状态
      setLobby(lobby);
      setAppState('in-lobby');

      toast({
        title: mode === 'create' ? '大厅创建成功！' : '成功加入大厅！',
        status: 'success',
        duration: 2000,
        isClosable: true,
      });

      // 关闭表单
      onClose();
    } catch (error) {
      console.error('操作失败:', error);
      console.error('错误详情:', JSON.stringify(error, null, 2));
      setAppState('error');

      // 提取详细的错误信息
      let errorMessage = '操作失败，请重试';

      if (typeof error === 'string') {
        errorMessage = error;
      } else if (error && typeof error === 'object') {
        if ('message' in error && typeof error.message === 'string') {
          errorMessage = error.message;
        } else if ('error' in error && typeof error.error === 'string') {
          errorMessage = error.error;
        } else {
          errorMessage = JSON.stringify(error);
        }
      }

      // 检查是否是权限相关的错误
      const isPermissionError =
        errorMessage.includes('拒绝访问') ||
        errorMessage.includes('Access is denied') ||
        errorMessage.includes('权限') ||
        errorMessage.includes('permission') ||
        errorMessage.includes('administrator') ||
        errorMessage.includes('740');

      // 检查是否是版本过低错误
      const isVersionError =
        errorMessage.includes('版本过低') ||
        errorMessage.includes('version') ||
        errorMessage.includes('更新');

      if (isPermissionError) {
        setErrorModal({
          title: '权限不足',
          type: 'error',
          content: (
            <Box>
              <Text>MCTier 需要管理员权限来创建虚拟网卡。</Text>
            </Box>
          ),
        });
      } else if (isVersionError) {
        setErrorModal({
          title: '需要更新',
          type: 'warning',
          content: (
            <VStack align="start" spacing={2}>
              <Text>{errorMessage}</Text>
              <Text fontSize="sm" color={mutedTextColor}>请访问 MCTier 官网下载最新版本</Text>
            </VStack>
          ),
          onOk: async () => {
            try {
              const { open } = await import('@tauri-apps/plugin-shell');
              await open('https://mctier.pmhs.top');
            } catch (err) {
              console.error('打开官网失败:', err);
            }
          },
        });
      } else {
        toast({
          title: mode === 'create' ? '创建大厅失败' : '加入大厅失败',
          description: errorMessage,
          status: 'error',
          duration: 8000,
          isClosable: true,
        });
      }
    } finally {
      setLoading(false);
    }
  };

  const handleCancel = () => {
    setAppState('idle');
    onClose();
  };

  return (
    <Box w="100%" h="100%" position="relative" overflow="hidden">
      {/* 顶部拖拽区域 */}
      <Box className="lobby-form-drag-area" data-tauri-drag-region />

      {/* 右上角按钮 */}
      <HStack gap={2} position="absolute" top={12} right={16} zIndex={20}>
        <Tooltip label="常用大厅信息">
          <IconButton
            aria-label="常用大厅"
            onClick={() => setShowFavoritesModal(true)}
            isDisabled={loading}
            variant="outline"
            size="sm"
            borderRadius="lg"
            color={textColor}
            borderColor={cardBorder}
            _hover={{ bg: hoverBg, borderColor: textColor }}
            _active={{ bg: 'rgba(255,255,255,0.1)' }}
          >
            <StarIcon size={18} />
          </IconButton>
        </Tooltip>

        {mode === 'create' ? (
          <Tooltip label="随机生成大厅名称和密码">
            <IconButton
              aria-label="随机生成"
              onClick={handleRandomGenerate}
              isDisabled={loading}
              variant="outline"
              size="sm"
              borderRadius="lg"
              color={textColor}
              borderColor={cardBorder}
              _hover={{ bg: hoverBg, borderColor: textColor }}
              _active={{ bg: 'rgba(255,255,255,0.1)' }}
            >
              <DiceIcon size={20} />
            </IconButton>
          </Tooltip>
        ) : (
          <Tooltip label="识别剪贴板中的大厅信息">
            <IconButton
              aria-label="识别剪贴板"
              onClick={() => recognizeClipboard(false)}
              isDisabled={loading}
              variant="outline"
              size="sm"
              borderRadius="lg"
              color={textColor}
              borderColor={cardBorder}
              _hover={{ bg: hoverBg, borderColor: textColor }}
              _active={{ bg: 'rgba(255,255,255,0.1)' }}
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" />
                <rect x="8" y="2" width="8" height="4" rx="1" ry="1" />
                <line x1="9" y1="12" x2="15" y2="12" />
                <line x1="9" y1="16" x2="15" y2="16" />
              </svg>
            </IconButton>
          </Tooltip>
        )}
      </HStack>

      <motion.div
        ref={scrollContainerRef}
        className="lobby-form-card"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ duration: 0.5, ease: 'easeOut' }}
      >
        <Box
          w="100%"
          minH="100%"
          bg={cardBg}
          border="1px solid"
          borderColor={cardBorder}
          borderRadius="2xl"
          px={16}
          py={12}
        >
          <Flex
            w="100%"
            minH="100%"
            direction="column"
            align="center"
            justify="center"
            gap={8}
          >
        {/* 标题栏 */}
        <motion.div
          initial={{ opacity: 0, y: -10 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.1, duration: 0.3 }}
        >
          <HStack justify="center" align="center" mb={5}>
            <Text
              fontSize="xl"
              fontWeight="bold"
              color={textColor}
              className="lobby-form-title"
            >
              {mode === 'create' ? '创建大厅' : '加入大厅'}
            </Text>
          </HStack>
        </motion.div>

        {/* 表单内容 */}
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ delay: 0.2, duration: 0.3 }}
        >
          <Box maxW="400px" mx="auto" w="full">
          <VStack spacing={4} align="stretch">
            {/* 大厅名称 */}
            <FormControl isInvalid={!!errors.lobbyName}>
              <FormLabel fontSize="xs" fontWeight="bold" color={labelColor}>
                大厅名称
              </FormLabel>
              <Input
                placeholder={mode === 'create' ? '输入大厅名称（至少4个字符）' : '输入要加入的大厅名称'}
                size="md"
                isDisabled={loading}
                value={formValues.lobbyName || ''}
                onChange={(e) => updateField('lobbyName', e.target.value)}
                bg={inputBg}
                borderColor={cardBorder}
                color={textColor}
                borderRadius="lg"
                _placeholder={{ color: mutedTextColor }}
                _hover={{ borderColor: borderColor }}
                _focus={{ borderColor: primaryColor, boxShadow: `0 0 0 4px ${primaryColor}15` }}
              />
              {errors.lobbyName && (
                <Text fontSize="xs" color="red.400" mt={1}>{errors.lobbyName}</Text>
              )}
            </FormControl>

            {/* 密码 */}
            <FormControl isInvalid={!!errors.password}>
              <FormLabel fontSize="xs" fontWeight="bold" color={labelColor}>
                密码
              </FormLabel>
              <InputGroup size="md">
                <Input
                  type={showPassword ? 'text' : 'password'}
                  placeholder="输入密码（至少8个字符，包含字母和数字）"
                  isDisabled={loading}
                  value={formValues.password || ''}
                  onChange={(e) => updateField('password', e.target.value)}
                  bg={inputBg}
                  borderColor={cardBorder}
                  color={textColor}
                  borderRadius="lg"
                  _placeholder={{ color: mutedTextColor }}
                  _hover={{ borderColor: borderColor }}
                  _focus={{ borderColor: primaryColor, boxShadow: `0 0 0 4px ${primaryColor}15` }}
                />
                <InputRightElement>
                  <IconButton
                    aria-label={showPassword ? '隐藏密码' : '显示密码'}
                    size="xs"
                    variant="ghost"
                    onClick={() => setShowPassword(!showPassword)}
                    icon={showPassword ? <ViewOffIcon /> : <ViewIcon />}
                  />
                </InputRightElement>
              </InputGroup>
              {errors.password && (
                <Text fontSize="xs" color="red.400" mt={1}>{errors.password}</Text>
              )}
            </FormControl>

            {/* 玩家名称 */}
            <FormControl isInvalid={!!errors.playerName}>
              <FormLabel fontSize="xs" fontWeight="bold" color={labelColor}>
                玩家名称
              </FormLabel>
              <Input
                placeholder="输入你的玩家名称（最多8个字）"
                size="md"
                isDisabled={loading}
                value={formValues.playerName || ''}
                onChange={(e) => updateField('playerName', e.target.value)}
                maxLength={8}
                bg={inputBg}
                borderColor={cardBorder}
                color={textColor}
                borderRadius="lg"
                _placeholder={{ color: mutedTextColor }}
                _hover={{ borderColor: borderColor }}
                _focus={{ borderColor: primaryColor, boxShadow: `0 0 0 4px ${primaryColor}15` }}
              />
              {errors.playerName && (
                <Text fontSize="xs" color="red.400" mt={1}>{errors.playerName}</Text>
              )}
            </FormControl>

            {/* 服务器节点 - 自定义 Chakra UI 下拉菜单 */}
            <FormControl isInvalid={!!errors.serverNode}>
              <FormLabel fontSize="xs" fontWeight="bold" color={labelColor}>
                服务器节点
                {privateServerConfig.usePrivateServer && (
                  <Text as="span" fontSize="xs" color={mutedTextColor} ml={1}>
                    （已启用私有服务器）
                  </Text>
                )}
              </FormLabel>
              <Menu>
                <MenuButton
                  as={Button}
                  rightIcon={
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <polyline points="6 9 12 15 18 9" />
                    </svg>
                  }
                  isDisabled={loading || privateServerConfig.usePrivateServer}
                  size="md"
                  w="full"
                  textAlign="left"
                  justifyContent="space-between"
                  fontWeight="normal"
                  bg={inputBg}
                  border="1px solid"
                  borderColor={errors.serverNode ? 'red.400' : cardBorder}
                  color={textColor}
                  borderRadius="lg"
                  _hover={{ borderColor: errors.serverNode ? 'red.400' : borderColor }}
                  _active={{ borderColor: primaryColor, boxShadow: `0 0 0 4px ${primaryColor}15` }}
                  _expanded={{ borderColor: primaryColor, boxShadow: `0 0 0 4px ${primaryColor}15` }}
                  px={4}
                  py={2}
                  h="auto"
                  minH="40px"
                  transition="all 0.2s"
                >
                  <HStack gap={2} overflow="hidden">
                    {/* 服务器节点图标 */}
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke={primaryColor} strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ flexShrink: 0 }}>
                      <rect x="2" y="2" width="20" height="8" rx="2" ry="2" />
                      <rect x="2" y="14" width="20" height="8" rx="2" ry="2" />
                      <line x1="6" y1="6" x2="6.01" y2="6" />
                      <line x1="6" y1="18" x2="6.01" y2="18" />
                    </svg>
                    <Text
                      fontSize="sm"
                      color={formValues.serverNode ? textColor : mutedTextColor}
                      noOfLines={1}
                    >
                      {formValues.serverNode
                        ? serverNodes.find(n => n.value === formValues.serverNode)?.label || formValues.serverNode
                        : '请选择服务器节点'}
                    </Text>
                  </HStack>
                </MenuButton>
                <MenuList
                  bg={menuBg}
                  border="1px solid"
                  borderColor={menuBorderColor}
                  borderRadius="lg"
                  boxShadow={menuShadow}
                  minW="320px"
                  py={1}
                  zIndex={100}
                >
                  {serverNodes.map((node, index) => (
                    <React.Fragment key={node.value}>
                      {/* 在"临时自定义服务器地址"前加分隔线 */}
                      {node.value === 'custom' && index > 0 && (
                        <Box mx={3} my={1} borderTop="1px solid" borderColor={menuDividerColor} />
                      )}
                      <MenuItem
                        value={node.value}
                        onClick={() => {
                          updateField('serverNode', node.value);
                          setShowCustomServer(node.value === 'custom');
                        }}
                        bg={formValues.serverNode === node.value ? menuSelectedBg : 'transparent'}
                        color={textColor}
                        fontSize="sm"
                        py={2.5}
                        px={4}
                        transition="all 0.15s"
                        _hover={{
                          bg: menuHoverBg,
                          color: textColor,
                        }}
                        icon={
                          formValues.serverNode === node.value ? (
                            <svg width="16" height="16" viewBox="0 0 24 24" fill={primaryColor}>
                              <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" />
                            </svg>
                          ) : (
                            // 空图标占位，保持对齐
                            <Box w="16px" />
                          )
                        }
                      >
                        <Text noOfLines={2} fontSize="sm">
                          {node.label}
                        </Text>
                      </MenuItem>
                    </React.Fragment>
                  ))}
                </MenuList>
              </Menu>
              {errors.serverNode && (
                <Text fontSize="xs" color="red.400" mt={1}>{errors.serverNode}</Text>
              )}
            </FormControl>

            {/* 自定义服务器（条件渲染） */}
            {showCustomServer && !privateServerConfig.usePrivateServer && (
              <>
                <FormControl>
                  <FormLabel fontSize="xs" fontWeight="bold" color={labelColor}>
                    临时 EasyTier 节点服务器
                  </FormLabel>
                  <Input
                    placeholder="例如：wss://mctiers.pmhs.top 或 tcp://your-server.com:11010"
                    size="md"
                    isDisabled={loading}
                    value={formValues.customEasytierServer || ''}
                    onChange={(e) => updateField('customEasytierServer', e.target.value)}
                    bg={inputBg}
                    borderColor={cardBorder}
                    color={textColor}
                    borderRadius="lg"
                    _placeholder={{ color: mutedTextColor }}
                    _hover={{ borderColor: borderColor }}
                    _focus={{ borderColor: primaryColor, boxShadow: `0 0 0 4px ${primaryColor}15` }}
                  />
                </FormControl>

                <FormControl>
                  <FormLabel fontSize="xs" fontWeight="bold" color={labelColor}>
                    临时 WebRTC 信令服务器
                  </FormLabel>
                  <Input
                    placeholder="例如：wss://mctier.pmhs.top/signaling"
                    size="md"
                    isDisabled={loading}
                    value={formValues.customSignalingServer || ''}
                    onChange={(e) => updateField('customSignalingServer', e.target.value)}
                    bg={inputBg}
                    borderColor={cardBorder}
                    color={textColor}
                    borderRadius="lg"
                    _placeholder={{ color: mutedTextColor }}
                    _hover={{ borderColor: borderColor }}
                    _focus={{ borderColor: primaryColor, boxShadow: `0 0 0 4px ${primaryColor}15` }}
                  />
                </FormControl>
              </>
            )}

            {/* 私有服务器提示 */}
            {privateServerConfig.usePrivateServer && (
              <Box
                p={3}
                bg={`${primaryColor}15`}
                border="1px solid"
                borderColor={`${primaryColor}30`}
                borderRadius="lg"
              >
                <Text fontSize="sm" color={primaryColor} fontWeight="medium" mb={1}>
                  ✓ 已启用私有服务器
                </Text>
                <Text fontSize="xs" color={mutedTextColor}>
                  EasyTier: {privateServerConfig.privateEasytierServer}
                  <br />
                  信令服务器: {privateServerConfig.privateSignalingServer}
                </Text>
              </Box>
            )}

            {/* 虚拟域名开关 */}
            <FormControl>
              <FormLabel fontSize="xs" fontWeight="bold" color={labelColor}>
                使用虚拟域名
                <Text as="span" fontSize="xs" color={mutedTextColor} ml={1}>
                  开启后，您的虚拟IP将显示为域名格式，便于记忆与访问
                </Text>
              </FormLabel>
              <AntSwitch
                checked={formValues.useDomain}
                onChange={(checked) => updateField('useDomain', checked)}
                disabled={loading}
              />
            </FormControl>

            {/* 按钮 */}
            <HStack spacing={3} pt={2}>
              <Button
                flex={1}
                size="lg"
                onClick={handleCancel}
                isDisabled={loading}
                borderRadius="xl"
                fontWeight="bold"
                variant="outline"
                borderColor={borderColor}
                color={primaryColor}
                _hover={{ bg: hoverBg, borderColor: primaryColor, transform: 'translateY(-2px)' }}
                _active={{ transform: 'scale(0.98)', bg: `${primaryColor}15` }}
                transition="all 0.2s"
              >
                取消
              </Button>
              <Button
                flex={1}
                size="lg"
                onClick={handleSubmit}
                isLoading={loading}
                borderRadius="xl"
                fontWeight="bold"
                bg={primaryColor}
                color="#ffffff"
                _hover={{ opacity: 0.9, transform: 'translateY(-2px)', boxShadow: `0 8px 24px ${primaryColor}50` }}
                _active={{ transform: 'scale(0.98)' }}
                _disabled={{ opacity: 0.6 }}
                transition="all 0.2s"
              >
                {mode === 'create' ? '创建' : '加入'}
              </Button>
            </HStack>
          </VStack>
          </Box>
        </motion.div>

        {/* 网络环境提示 */}
        <motion.div
          className="lobby-form-network-tip"
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.4, duration: 0.3 }}
          style={{
            background: tipBg,
            border: `1px solid ${tipBorder}`,
          }}
        >
          <Box maxW="400px" mx="auto" w="full">
            <Box className="network-tip-icon" color={primaryColor}>
              <WarningIcon size={20} />
            </Box>
            <div className="network-tip-content">
              <div className="network-tip-title" style={{ color: primaryColor }}>重要提示</div>
            <div className="network-tip-text" style={{ color: tipText }}>
              <strong style={{ color: tipStrong }}>网络环境：</strong>本软件使用纯 P2P 方式连接，为确保联机成功：
              <br />
              ✓ 推荐使用家庭 WiFi 网络
              <br />
              ✗ 不建议使用校园网、手机流量或热点
              <br />
              <br />
              <strong style={{ color: tipStrong }}>虚拟域名：</strong>虚拟域名仅能用于访问网站使用，Minecraft 多人游戏不支持使用虚拟域名。加入 Minecraft 服务器时，请使用虚拟IP+端口号（例如：10.126.126.1:25565）
              <br />
              <br />
              <strong style={{ color: tipStrong }}>代理工具：</strong>使用虚拟域名功能时，请务必关闭代理工具（如梯子、VPN等），否则域名解析将失效
            </div>
          </div>
          </Box>
        </motion.div>
          </Flex>
        </Box>
      </motion.div>

      {/* 滚动提示 - 悬浮在底部 */}
      <AnimatePresence>
        {showScrollHint && canScroll && (
          <motion.div
            className="scroll-hint-floating"
            style={{
              background: primaryColor,
              color: '#ffffff',
              boxShadow: `0 4px 20px ${primaryColor}50`,
            }}
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 20 }}
            transition={{ duration: 0.3 }}
          >
            <motion.div
              animate={{ y: [0, 6, 0] }}
              transition={{ duration: 1.5, repeat: Infinity, ease: "easeInOut" }}
            >
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M12 5v14M19 12l-7 7-7-7"/>
              </svg>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* 错误弹窗 */}
      <Modal
        isOpen={!!errorModal}
        onClose={() => setErrorModal(null)}
        isCentered
        closeOnOverlayClick={!loading}
      >
        <ModalOverlay />
        <ModalContent bg={cardBg} borderColor={cardBorder} borderRadius="xl">
          <ModalHeader color={textColor}>{errorModal?.title}</ModalHeader>
          <ModalBody>{errorModal?.content}</ModalBody>
          <ModalFooter>
            <Button
              onClick={() => {
                if (errorModal?.onOk) {
                  errorModal.onOk();
                }
                setErrorModal(null);
              }}
              colorScheme={errorModal?.type === 'warning' ? 'orange' : 'blue'}
              borderRadius="lg"
            >
              {errorModal?.type === 'warning' ? '前往官网' : '我知道了'}
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>

      {/* 常用大厅信息管理弹窗 */}
      <FavoriteLobbyManager
        visible={showFavoritesModal}
        onClose={() => setShowFavoritesModal(false)}
        onSelect={handleSelectFavorite}
      />
    </Box>
  );
};
