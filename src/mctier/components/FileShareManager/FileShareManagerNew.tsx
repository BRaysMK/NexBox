/**
 * 文件共享管理器 - 全新重构版本
 * 专门为HTTP over WireGuard设计
 * 支持多选批量下载、断点续传、先压后发
 */

import React, { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Box, Flex, HStack, VStack, Text, Button, Input, Switch, Checkbox, Progress,
  Modal, ModalOverlay, ModalContent, ModalHeader, ModalBody, ModalFooter, ModalCloseButton,
  InputGroup, InputRightElement, IconButton, Badge, Tooltip,
  useToast, useColorModeValue, Spinner,
} from '@chakra-ui/react';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '../../stores/appStore';
import { useThemeColor } from '@/contexts/theme-color-context';
import type { SharedFolder, FileInfo } from '../../types/fileShare';
import { FolderIcon, DownloadIcon, ShareIcon, CloseIcon, BackIcon, TrashIcon } from '../icons';
import './FileShareManager.css';

// 简化的远程共享类型
interface SimpleRemoteShare {
  share: SharedFolder;
  ownerName: string;
  ownerIp: string;
}

// 下载任务状态
interface DownloadTask {
  id: string;
  fileName: string;
  fileSize: number;
  downloaded: number;
  status: 'downloading' | 'completed' | 'failed';
  url: string;
  savePath: string;
  headers?: HeadersInit;
  error?: string;
  abortController?: AbortController; // 用于取消下载
  speed?: number; // 下载速度（bytes/s）
  lastUpdateTime?: number; // 上次更新时间
  lastDownloaded?: number; // 上次下载的字节数
  isBatchDownload?: boolean; // 是否为批量下载（文件夹）
}

export const FileShareManagerNew: React.FC = () => {
  // 主题适配
  const toast = useToast();
  const { getActiveColor, getHoverColor, getBorderColor, getContrastTextColor } = useThemeColor();

  const bg = useColorModeValue('white', '#111111');
  const cardBg = useColorModeValue('gray.50', 'rgba(255,255,255,0.03)');
  const cardBgHover = useColorModeValue('gray.100', 'rgba(255,255,255,0.06)');
  const cardBgActive = useColorModeValue('gray.100', 'rgba(255,255,255,0.05)');
  const borderColor = useColorModeValue('gray.200', 'rgba(255,255,255,0.12)');
  const borderColorHover = useColorModeValue('gray.300', 'rgba(255,255,255,0.15)');
  const textColor = useColorModeValue('gray.800', 'white');
  const textColorStrong = useColorModeValue('gray.900', 'rgba(255,255,255,0.9)');
  const mutedText = useColorModeValue('gray.500', 'rgba(255,255,255,0.5)');
  const mutedTextHover = useColorModeValue('gray.600', 'rgba(255,255,255,0.8)');
  const sidebarBg = useColorModeValue('gray.100', 'rgba(20, 20, 30, 0.95)');
  const sidebarBorder = useColorModeValue('gray.200', 'rgba(255,255,255,0.1)');
  const activeColor = getActiveColor();
  const activeHoverBg = getHoverColor();
  const activeBorder = getBorderColor();
  const contrastText = getContrastTextColor();
  // 侧边栏标签页颜色
  const tabInactiveBg = useColorModeValue('rgba(0,0,0,0.04)', 'rgba(255,255,255,0.03)');
  const tabInactiveColor = useColorModeValue('gray.500', 'rgba(255,255,255,0.5)');
  const tabActiveBg = useColorModeValue('rgba(0,0,0,0.08)', 'rgba(255,255,255,0.08)');
  const fileItemBg = useColorModeValue('gray.50', 'rgba(255,255,255,0.05)');
  const fileItemBgHover = useColorModeValue('gray.100', 'rgba(255,255,255,0.08)');
  const fileItemBgClickable = useColorModeValue('gray.100', 'rgba(255,255,255,0.1)');
  const browserHeaderBg = useColorModeValue('gray.50', 'rgba(255,255,255,0.05)');
  const subtabBg = useColorModeValue('gray.100', 'rgba(255,255,255,0.03)');
  const subtabBorder = useColorModeValue('gray.200', 'rgba(255,255,255,0.1)');
  const cancelBtnBg = useColorModeValue('red.500', 'rgba(239, 68, 68, 0.9)');
  const cancelBtnBorder = useColorModeValue('red.600', 'rgba(239, 68, 68, 1)');
  const deleteBtnBg = useColorModeValue('red.50', 'rgba(239, 68, 68, 0.2)');
  const deleteBtnColor = useColorModeValue('red.500', '#ef4444');
  const deleteBtnBorder = useColorModeValue('red.200', 'rgba(239, 68, 68, 0.4)');
  const emptyColor = useColorModeValue('gray.400', 'rgba(255,255,255,0.4)');
  const loadingColor = useColorModeValue('gray.500', 'rgba(255,255,255,0.6)');
  const badgeBg = useColorModeValue('green.500', '#52c41a');
  const speedColor = useColorModeValue('blue.500', 'rgba(100, 200, 255, 0.9)');

  // 基础状态
  const [activeTab, setActiveTab] = useState<'local' | 'remote' | 'transfers'>('local');
  const [localShares, setLocalShares] = useState<SharedFolder[]>([]);
  const [remoteShares, setRemoteShares] = useState<SimpleRemoteShare[]>([]);
  const [showAddShare, setShowAddShare] = useState(false);

  // 文件浏览状态
  const [selectedShare, setSelectedShare] = useState<SimpleRemoteShare | null>(null);
  const [currentPath, setCurrentPath] = useState('');
  const [files, setFiles] = useState<FileInfo[]>([]);
  const [loadingFiles, setLoadingFiles] = useState(false);
  const [selectedFiles, setSelectedFiles] = useState<Set<string>>(new Set());

  // 下载状态
  const [downloads, setDownloads] = useState<DownloadTask[]>([]);
  const [transferSubTab, setTransferSubTab] = useState<'downloading' | 'completed'>('downloading');

  // 密码验证
  const [showPasswordModal, setShowPasswordModal] = useState(false);
  const [passwordInput, setPasswordInput] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [pendingShare, setPendingShare] = useState<SimpleRemoteShare | null>(null);
  const [sharePasswordMap, setSharePasswordMap] = useState<Record<string, string>>({});
  const pendingBrowsePathRef = useRef<string>('');

  // 加载提示状态（替代 message.loading）
  const [savingMessage, setSavingMessage] = useState<string | null>(null);

  // 从Store获取数据
  const { lobby, players, config } = useAppStore();

  // 加载本地共享
  const loadLocalShares = async () => {
    try {
      const shares = await invoke<SharedFolder[]>('get_local_shares');
      setLocalShares(shares);
    } catch (error) {
      console.error('加载本地共享失败:', error);
    }
  };

  const getShareKey = (ownerIp: string, shareId: string): string => `${ownerIp}__${shareId}`;

  const getSharePasswordHeader = (ownerIp: string, shareId: string, passwordOverride?: string): HeadersInit => {
    const shareKey = getShareKey(ownerIp, shareId);
    const password = passwordOverride ?? sharePasswordMap[shareKey];
    if (!password) return {};

    return {
      'x-share-password': password,
    };
  };

  // 加载远程共享 - 简化版本
  const loadRemoteShares = async () => {

    const allShares: SimpleRemoteShare[] = [];
    const now = Math.floor(Date.now() / 1000);

    // 1. 加载自己的共享
    if (lobby?.virtualIp) {
      try {
        const shares = await invoke<SharedFolder[]>('get_remote_shares', { peerIp: lobby.virtualIp });

        shares.forEach(share => {
          // 过滤掉过期的共享
          if (!share.expire_time || share.expire_time > now) {
            allShares.push({
              share,
              ownerName: `${config.playerName || '我'} (我)`,
              ownerIp: lobby.virtualIp!
            });
          }
        });
      } catch (error) {
        console.error('获取自己的共享失败:', error);
      }
    }

    // 2. 加载其他玩家的共享
    for (const player of players) {
      if (player.virtualIp) {
        try {
          const shares = await invoke<SharedFolder[]>('get_remote_shares', { peerIp: player.virtualIp });

          shares.forEach(share => {
            // 过滤掉过期的共享
            if (!share.expire_time || share.expire_time > now) {
              allShares.push({
                share,
                ownerName: player.name,
                ownerIp: player.virtualIp!
              });
            }
          });
        } catch (error) {
          console.error(`获取 ${player.name} 的共享失败:`, error);
        }
      }
    }

    // 检查当前正在浏览的共享是否还存在
    // 只有在正在浏览共享时才检查
    if (selectedShare && activeTab === 'remote') {
      const stillExists = allShares.some(
        s => s.ownerIp === selectedShare.ownerIp && s.share.id === selectedShare.share.id
      );
      if (!stillExists) {
        // 共享已被删除，退出浏览
        setSelectedShare(null);
        setCurrentPath('');
        setFiles([]);
        setSelectedFiles(new Set());
        toast({ title: '该共享文件夹已被删除', status: 'warning', duration: 3000, isClosable: true });
      }
    }

    setRemoteShares(allShares);
  };

  // 组件挂载时加载本地共享
  useEffect(() => {
    loadLocalShares();
  }, []);

  // 【事件驱动】监听文件共享事件
  useEffect(() => {
    console.log('📡 [FileShareManager] 设置文件共享事件监听器');

    // 文件共享添加事件
    const handleFileShareAdded = (event: any) => {
      console.log('📁 [FileShareManager] 收到文件共享添加事件:', event.detail);
      const { shareId, shareName, playerId, playerName, hasPassword } = event.detail;

      // 查找玩家的虚拟IP
      const player = players.find(p => p.id === playerId);
      if (!player || !player.virtualIp) {
        console.warn('⚠️ [FileShareManager] 找不到玩家或虚拟IP:', playerId);
        return;
      }

      // 添加到远程共享列表
      const newShare: SimpleRemoteShare = {
        share: {
          id: shareId,
          name: shareName,
          path: '',
          password: hasPassword ? 'protected' : undefined,
          expire_time: undefined,
          compress_before_send: false,
          owner_id: playerId,
          created_at: Date.now() / 1000,
        },
        ownerName: playerName,
        ownerIp: player.virtualIp,
      };

      setRemoteShares(prev => {
        // 检查是否已存在
        const exists = prev.some(s => s.share.id === shareId && s.ownerIp === player.virtualIp);
        if (exists) {
          console.log('📁 [FileShareManager] 共享已存在，跳过添加');
          return prev;
        }
        console.log('✅ [FileShareManager] 添加新共享到列表');
        return [...prev, newShare];
      });
    };

    // 文件共享删除事件
    const handleFileShareRemoved = (event: any) => {
      console.log('🗑️ [FileShareManager] 收到文件共享删除事件:', event.detail);
      const { shareId, playerId } = event.detail;

      // 查找玩家的虚拟IP
      const player = players.find(p => p.id === playerId);
      if (!player || !player.virtualIp) {
        console.warn('⚠️ [FileShareManager] 找不到玩家或虚拟IP:', playerId);
        return;
      }

      setRemoteShares(prev => {
        const filtered = prev.filter(s => !(s.share.id === shareId && s.ownerIp === player.virtualIp));
        console.log(`✅ [FileShareManager] 从列表移除共享，剩余 ${filtered.length} 个`);
        return filtered;
      });

      // 【修复】如果正在浏览被删除的共享，立即退出浏览
      if (selectedShare && selectedShare.share.id === shareId && selectedShare.ownerIp === player.virtualIp) {
        console.log('⚠️ [FileShareManager] 正在浏览的共享被删除，立即退出浏览');
        setSelectedShare(null);
        setCurrentPath('');
        setFiles([]);
        setSelectedFiles(new Set());
        toast({ title: '该共享文件夹已被删除', status: 'warning', duration: 3000, isClosable: true });
      }
    };

    // 添加事件监听
    window.addEventListener('file-share-added', handleFileShareAdded);
    window.addEventListener('file-share-removed', handleFileShareRemoved);

    console.log('✅ [FileShareManager] 文件共享事件监听器已设置');

    // 清理函数
    return () => {
      console.log('🧹 [FileShareManager] 移除文件共享事件监听器');
      window.removeEventListener('file-share-added', handleFileShareAdded);
      window.removeEventListener('file-share-removed', handleFileShareRemoved);
    };
  }, [players, selectedShare]);

  // 切换到远程共享时加载数据（只加载一次，不轮询）
  useEffect(() => {
    if (activeTab === 'remote') {
      loadRemoteShares();

      // 【修复】添加定时检查过期共享（每秒检查一次）
      const expiryCheckInterval = setInterval(() => {
        const now = Math.floor(Date.now() / 1000);
        setRemoteShares(prev => {
          const filtered = prev.filter(s => !s.share.expire_time || s.share.expire_time > now);

          // 如果有共享被过滤掉，说明有过期的
          if (filtered.length < prev.length) {
            console.log(`⏰ [FileShareManager] 检测到 ${prev.length - filtered.length} 个过期共享，已自动移除`);

            // 如果正在浏览的共享过期了，退出浏览
            if (selectedShare) {
              const stillExists = filtered.some(
                s => s.ownerIp === selectedShare.ownerIp && s.share.id === selectedShare.share.id
              );
              if (!stillExists) {
                console.log('⚠️ [FileShareManager] 正在浏览的共享已过期，自动退出浏览');
                setSelectedShare(null);
                setCurrentPath('');
                setFiles([]);
                setSelectedFiles(new Set());
                toast({ title: '该共享文件夹已过期', status: 'warning', duration: 3000, isClosable: true });
              }
            }
          }

          return filtered;
        });
      }, 1000); // 每秒检查一次

      return () => clearInterval(expiryCheckInterval);
    }
  }, [activeTab, lobby?.virtualIp, players.length, selectedShare]);

  // 切换到传输列表时，默认显示正在下载分页
  useEffect(() => {
    if (activeTab === 'transfers') {
      setTransferSubTab('downloading');
    }
  }, [activeTab]);

  // 删除共享
  const handleDeleteShare = async (shareId: string) => {
    try {
      await invoke('remove_shared_folder', { shareId });

      // 【事件驱动】通过信令服务器广播文件共享删除事件
      try {
        const { webrtcClient } = await import('../../services/webrtc');
        const { currentPlayerId } = useAppStore.getState();
        if (webrtcClient && currentPlayerId) {
          console.log('📡 [FileShareManager] 广播文件共享删除事件');
          webrtcClient.sendWebSocketMessage({
            type: 'file-share-removed',
            from: currentPlayerId,
            shareId: shareId,
          });
        }
      } catch (error) {
        console.error('❌ [FileShareManager] 广播文件共享删除事件失败:', error);
        // 不影响主流程
      }

      toast({ title: '删除共享成功', status: 'success', duration: 3000, isClosable: true });
      loadLocalShares();
    } catch (error) {
      toast({ title: '删除共享失败', status: 'error', duration: 3000, isClosable: true });
    }
  };

  // 浏览共享
  const handleBrowseShare = async (remoteShare: SimpleRemoteShare) => {
    pendingBrowsePathRef.current = '';

    if (remoteShare.share.password) {
      setPendingShare(remoteShare);
      setShowPasswordModal(true);
      return;
    }
    await openShare(remoteShare);
  };

  // 打开共享
  const openShare = async (remoteShare: SimpleRemoteShare, password?: string) => {
    try {
      const targetPath = pendingBrowsePathRef.current || '';
      let verifiedPassword: string | undefined;

      if (remoteShare.share.password) {
        const passwordToVerify = password ?? sharePasswordMap[getShareKey(remoteShare.ownerIp, remoteShare.share.id)] ?? '';
        const valid = await invoke<boolean>('verify_share_password', {
          peerIp: remoteShare.ownerIp,
          shareId: remoteShare.share.id,
          password: passwordToVerify,
        });
        if (!valid) {
          toast({ title: '密码错误', status: 'error', duration: 3000, isClosable: true });
          return;
        }

        verifiedPassword = passwordToVerify;
        setSharePasswordMap(prev => ({
          ...prev,
          [getShareKey(remoteShare.ownerIp, remoteShare.share.id)]: passwordToVerify,
        }));
      }

      setSelectedShare(remoteShare);
      setSelectedFiles(new Set());
      await loadFiles(remoteShare, targetPath, verifiedPassword);
      setShowPasswordModal(false);
      setPasswordInput('');
      setPendingShare(null);
      pendingBrowsePathRef.current = '';
    } catch (error) {
      toast({ title: '打开共享失败', status: 'error', duration: 3000, isClosable: true });
    }
  };

  // 加载文件列表
  const loadFiles = async (remoteShare: SimpleRemoteShare, path: string, passwordOverride?: string) => {
    setLoadingFiles(true);
    try {
      const response = await fetch(
        `http://${remoteShare.ownerIp}:14539/api/shares/${remoteShare.share.id}/files${path ? `?path=${encodeURIComponent(path)}` : ''}`,
        {
          headers: getSharePasswordHeader(remoteShare.ownerIp, remoteShare.share.id, passwordOverride),
        }
      );

      if (!response.ok) {
        if (response.status === 401) {
          const retryPath = path;
          toast({ title: '访问被拒绝，请重新输入密码', status: 'error', duration: 3000, isClosable: true });
          setSharePasswordMap(prev => {
            const next = { ...prev };
            delete next[getShareKey(remoteShare.ownerIp, remoteShare.share.id)];
            return next;
          });

          pendingBrowsePathRef.current = retryPath;
          setPendingShare(remoteShare);
          setShowPasswordModal(true);
          setPasswordInput('');
          return;
        }
        throw new Error(`HTTP ${response.status}`);
      }

      const payload = await response.json();
      const fileList = (payload?.files ?? []) as FileInfo[];
      setFiles(fileList);
      setCurrentPath(path);
      setSelectedFiles(new Set());
    } catch (error) {
      const errorMessage = String(error);
      if (!errorMessage.includes('HTTP 401')) {
        toast({ title: '加载文件列表失败', status: 'error', duration: 3000, isClosable: true });
      }
    } finally {
      setLoadingFiles(false);
    }
  };

  // 下载单个文件
  const handleDownloadFile = async (file: FileInfo) => {
    if (!selectedShare) return;

    try {
      // 选择保存位置
      const savePath = await invoke<string | null>('select_save_location', {
        defaultName: file.name
      });

      if (!savePath) {
        return; // 用户取消
      }

      const downloadUrl = `http://${selectedShare.ownerIp}:14539/api/shares/${selectedShare.share.id}/download/${file.path}`;
      const downloadHeaders = getSharePasswordHeader(selectedShare.ownerIp, selectedShare.share.id);

      // 创建下载任务
      const taskId = `download_${Date.now()}_${Math.random()}`;
      const newTask: DownloadTask = {
        id: taskId,
        fileName: file.name,
        fileSize: file.size,
        downloaded: 0,
        status: 'downloading',
        url: downloadUrl,
        headers: downloadHeaders,
        savePath
      };

      setDownloads(prev => [...prev, newTask]);
      // 不自动跳转到传输列表，让用户继续浏览

      // 开始下载
      startDownload(taskId, downloadUrl, savePath, file.size, downloadHeaders);

      toast({ title: '开始下载文件', status: 'success', duration: 3000, isClosable: true });
    } catch (error) {
      toast({ title: `下载失败: ${error}`, status: 'error', duration: 3000, isClosable: true });
    }
  };

  // 实际执行下载
  const startDownload = async (taskId: string, url: string, savePath: string, fileSize: number, headers?: HeadersInit) => {
      const abortController = new AbortController();
      const startTime = Date.now();
      let lastUpdateTime = startTime;
      let lastDownloaded = 0;

      // 更新任务，添加abortController
      setDownloads(prev => prev.map(task =>
        task.id === taskId ? { ...task, abortController, lastUpdateTime, lastDownloaded } : task
      ));

      try {
        const response = await fetch(url, {
          signal: abortController.signal,
          headers: headers || {},
        });

        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }

        const reader = response.body?.getReader();
        if (!reader) {
          throw new Error('无法读取响应');
        }

        const chunks: Uint8Array[] = [];
        let downloaded = 0;

        while (true) {
          try {
            const { done, value } = await reader.read();

            if (done) break;

            chunks.push(value);
            downloaded += value.length;

            // 计算速度（每500ms更新一次）
            const now = Date.now();
            const timeDiff = now - lastUpdateTime;

            if (timeDiff >= 500) {
              const byteDiff = downloaded - lastDownloaded;
              const speed = (byteDiff / timeDiff) * 1000; // bytes/s

              // 更新进度和速度
              setDownloads(prev => prev.map(task =>
                task.id === taskId ? {
                  ...task,
                  downloaded,
                  speed,
                  lastUpdateTime: now,
                  lastDownloaded: downloaded
                } : task
              ));

              lastUpdateTime = now;
              lastDownloaded = downloaded;
            } else {
              // 【修复】减少状态更新频率，避免过度渲染
              // 只在下载量变化超过1MB时才更新UI
              if (downloaded - (lastDownloaded || 0) > 1024 * 1024) {
                setDownloads(prev => prev.map(task =>
                  task.id === taskId ? { ...task, downloaded } : task
                ));
              }
            }
          } catch (error: any) {
            // 如果是用户主动取消
            if (error.name === 'AbortError') {
              console.log(`❌ [FileShareManager] 下载被取消`);
              return;
            }
            throw error;
          }
        }

        // 合并所有chunks
        const blob = new Blob(chunks as BlobPart[]);
        const arrayBuffer = await blob.arrayBuffer();
        const uint8Array = new Uint8Array(arrayBuffer);

        // 显示"正在保存..."提示
        setSavingMessage('正在保存文件到磁盘...');

        await invoke('save_file', {
          path: savePath,
          data: Array.from(uint8Array)
        });

        setSavingMessage(null);

        // 标记为完成
        setDownloads(prev => prev.map(task =>
          task.id === taskId ? { ...task, status: 'completed' as const, downloaded: fileSize, speed: 0 } : task
        ));

        toast({ title: '下载完成', status: 'success', duration: 3000, isClosable: true });
      } catch (error: any) {
        if (error.name !== 'AbortError') {
          setDownloads(prev => prev.map(task =>
            task.id === taskId ? { ...task, status: 'failed' as const, error: String(error), speed: 0 } : task
          ));
          toast({ title: `下载失败: ${error}`, status: 'error', duration: 3000, isClosable: true });
        }
      }
    }



  // 批量下载选中的文件
  const handleBatchDownload = async () => {
    if (!selectedShare || selectedFiles.size === 0) {
      toast({ title: '请先选择要下载的文件', status: 'warning', duration: 3000, isClosable: true });
      return;
    }

    const selectedFileList = files.filter(f => !f.is_dir && selectedFiles.has(f.path));

    if (selectedFileList.length === 0) {
      toast({ title: '没有选中任何文件', status: 'warning', duration: 3000, isClosable: true });
      return;
    }

    // 选择保存位置
    const saveDir = await invoke<string | null>('select_folder');
    if (!saveDir) {
      return; // 用户取消
    }

    // 检查是否启用了"先压后发"
    if (selectedShare.share.compress_before_send && selectedFileList.length > 1) {
      try {
        // 创建一个下载任务用于显示进度
        const taskId = `batch_download_${Date.now()}`;
        const zipFileName = `batch_download_${Date.now()}.zip`;
        const tempZipPath = `${saveDir}/${zipFileName}`;
        const newTask: DownloadTask = {
          id: taskId,
          fileName: `批量下载 (${selectedFileList.length} 个文件)`,
          fileSize: 0, // 未知大小
          downloaded: 0,
          status: 'downloading',
          url: '',
          savePath: tempZipPath,
          isBatchDownload: true // 标记为批量下载
        };

        setDownloads(prev => [...prev, newTask]);
        toast({ title: `正在打包 ${selectedFileList.length} 个文件，请稍候...`, status: 'info', duration: 3000, isClosable: true });

        // 异步下载，不阻塞UI
        (async () => {
          try {
            // 直接调用HTTP API打包文件
            const url = `http://${selectedShare.ownerIp}:14539/api/shares/${selectedShare.share.id}/batch-download`;
            console.log('📦 [FileShareManager] 请求批量打包:', url);
            console.log('📦 [FileShareManager] 文件列表:', selectedFileList.map(f => f.path));

            const response = await fetch(url, {
              method: 'POST',
              headers: {
                'Content-Type': 'application/json',
                ...getSharePasswordHeader(selectedShare.ownerIp, selectedShare.share.id),
              },
              body: JSON.stringify({
                file_paths: selectedFileList.map(f => f.path)
              })
            });

            if (!response.ok) {
              const errorText = await response.text();
              console.error('❌ [FileShareManager] 批量打包失败:', response.status, errorText);
              throw new Error(`HTTP ${response.status}: ${errorText || '打包失败'}`);
            }

            console.log('✅ [FileShareManager] 开始下载压缩包');

            // 使用流式下载，实时更新进度
            const reader = response.body?.getReader();
            if (!reader) {
              throw new Error('无法读取响应');
            }

            // 获取文件总大小
            const contentLength = response.headers.get('Content-Length');
            const totalSize = contentLength ? parseInt(contentLength, 10) : 0;

            console.log('📦 [FileShareManager] 压缩包总大小:', totalSize, 'bytes');

            // 更新任务的文件大小
            if (totalSize > 0) {
              setDownloads(prev => prev.map(task =>
                task.id === taskId ? { ...task, fileSize: totalSize } : task
              ));
            }

            const chunks: Uint8Array[] = [];
            let downloaded = 0;
            let lastUpdateTime = Date.now();
            let lastDownloaded = 0;

            while (true) {
              const { done, value } = await reader.read();

              if (done) break;

              chunks.push(value);
              downloaded += value.length;

              // 计算速度（每500ms更新一次）
              const now = Date.now();
              const timeDiff = now - lastUpdateTime;

              if (timeDiff >= 500) {
                const byteDiff = downloaded - lastDownloaded;
                const speed = (byteDiff / timeDiff) * 1000; // bytes/s

                // 更新进度和速度
                setDownloads(prev => prev.map(task =>
                  task.id === taskId ? {
                    ...task,
                    downloaded,
                    speed,
                    fileSize: totalSize > 0 ? totalSize : downloaded,
                    lastUpdateTime: now,
                    lastDownloaded: downloaded
                  } : task
                ));

                lastUpdateTime = now;
                lastDownloaded = downloaded;
              }
            }

            // 合并所有chunks
            const blob = new Blob(chunks as BlobPart[]);
            const arrayBuffer = await blob.arrayBuffer();
            const uint8Array = new Uint8Array(arrayBuffer);

            console.log('📦 [FileShareManager] 压缩包下载完成，大小:', uint8Array.length, 'bytes');

            // 显示"正在保存..."提示
            setDownloads(prev => prev.map(task =>
              task.id === taskId ? {
                ...task,
                downloaded: uint8Array.length,
                fileSize: uint8Array.length,
                speed: 0
              } : task
            ));

            setSavingMessage('正在保存压缩包...');

            // 保存临时ZIP文件
            await invoke('save_file', {
              path: tempZipPath,
              data: Array.from(uint8Array)
            });

            setSavingMessage(null);
            console.log('✅ [FileShareManager] 压缩包已保存:', tempZipPath);

            // 【新增】自动解压ZIP文件
            setSavingMessage('正在解压文件...');
            console.log('📦 [FileShareManager] 开始解压ZIP文件到:', saveDir);

            const extractedFiles = await invoke<string[]>('extract_zip', {
              zipPath: tempZipPath,
              extractDir: saveDir
            });

            setSavingMessage(null);
            console.log('✅ [FileShareManager] 文件解压完成，共', extractedFiles.length, '个文件');

            // 【新增】删除临时ZIP文件
            console.log('🗑️ [FileShareManager] 删除临时ZIP文件:', tempZipPath);
            await invoke('delete_file', { path: tempZipPath });
            console.log('✅ [FileShareManager] 临时ZIP文件已删除');

            // 更新任务状态为完成，并更新savePath为实际的解压目录
            setDownloads(prev => prev.map(task =>
              task.id === taskId ? {
                ...task,
                status: 'completed' as const,
                downloaded: uint8Array.length,
                fileSize: uint8Array.length,
                speed: 0,
                fileName: `${selectedFileList.length} 个文件`, // 更新显示名称
                savePath: saveDir // 【修复】更新为实际的解压目录，而不是临时ZIP路径
              } : task
            ));

            toast({ title: `下载完成 (${selectedFileList.length} 个文件)`, status: 'success', duration: 3000, isClosable: true });

            // 清空选中状态
            setSelectedFiles(new Set());
          } catch (error) {
            console.error('❌ [FileShareManager] 批量下载失败:', error);

            // 更新任务状态为失败
            setDownloads(prev => prev.map(task =>
              task.id === taskId ? { ...task, status: 'failed' as const, error: String(error), speed: 0 } : task
            ));
            toast({ title: `下载失败: ${error}`, status: 'error', duration: 3000, isClosable: true });
          }
        })();
      } catch (error) {
        console.error('❌ [FileShareManager] 批量下载失败:', error);
        toast({ title: `批量下载失败: ${error}`, status: 'error', duration: 3000, isClosable: true });
      }
    } else if (!selectedShare.share.compress_before_send && selectedFileList.length > 1) {
      // 【修复】如果没有启用"先压后发"，提示用户
      toast({ title: '该共享未启用"先压后发"功能，将逐个下载文件', status: 'warning', duration: 3000, isClosable: true });

      // 逐个下载
      for (const file of selectedFileList) {
        const savePath = `${saveDir}/${file.name}`;
        const downloadUrl = `http://${selectedShare.ownerIp}:14539/api/shares/${selectedShare.share.id}/download/${file.path}`;
      const downloadHeaders = getSharePasswordHeader(selectedShare.ownerIp, selectedShare.share.id);

        const taskId = `download_${Date.now()}_${Math.random()}`;
        const newTask: DownloadTask = {
          id: taskId,
          fileName: file.name,
          fileSize: file.size,
          downloaded: 0,
          status: 'downloading',
          url: downloadUrl,
          headers: downloadHeaders,
          savePath
        };

        setDownloads(prev => [...prev, newTask]);
        startDownload(taskId, downloadUrl, savePath, file.size, downloadHeaders);
      }

      toast({ title: `开始下载 ${selectedFileList.length} 个文件`, status: 'success', duration: 3000, isClosable: true });

      // 清空选中状态
      setSelectedFiles(new Set());
    } else {
      // 只选中了一个文件，直接下载
      const file = selectedFileList[0];
      const savePath = `${saveDir}/${file.name}`;
      const downloadUrl = `http://${selectedShare.ownerIp}:14539/api/shares/${selectedShare.share.id}/download/${file.path}`;
      const downloadHeaders = getSharePasswordHeader(selectedShare.ownerIp, selectedShare.share.id);

      const taskId = `download_${Date.now()}_${Math.random()}`;
      const newTask: DownloadTask = {
        id: taskId,
        fileName: file.name,
        fileSize: file.size,
        downloaded: 0,
        status: 'downloading',
        url: downloadUrl,
        headers: downloadHeaders,
        savePath
      };

      setDownloads(prev => [...prev, newTask]);
      startDownload(taskId, downloadUrl, savePath, file.size, downloadHeaders);

      toast({ title: '开始下载', status: 'success', duration: 3000, isClosable: true });

      // 清空选中状态
      setSelectedFiles(new Set());
    }
  };

  // 进入文件夹（修复路径拼接问题）
  const handleEnterFolder = async (folder: FileInfo) => {
    if (!selectedShare || !folder.is_dir) return;
    // 修复：folder.name 是文件夹名称，需要拼接到当前路径
    const newPath = currentPath ? `${currentPath}/${folder.name}` : folder.name;
    await loadFiles(selectedShare, newPath);
  };

  // 返回上级
  const handleGoBack = async () => {
    if (!selectedShare || !currentPath) return;
    const parts = currentPath.split('/').filter(p => p);
    parts.pop();
    const newPath = parts.join('/');
    await loadFiles(selectedShare, newPath);
  };

  // 返回根目录
  const handleGoToRoot = async () => {
    if (!selectedShare) return;
    await loadFiles(selectedShare, '');
  };

  const handleExitShareBrowser = () => {
    if (!selectedShare) return;

    const shareKey = getShareKey(selectedShare.ownerIp, selectedShare.share.id);
    setSharePasswordMap(prev => {
      const next = { ...prev };
      delete next[shareKey];
      return next;
    });

    setSelectedShare(null);
    setCurrentPath('');
    setFiles([]);
    setSelectedFiles(new Set());
    pendingBrowsePathRef.current = '';
  };
  const toggleFileSelection = (filePath: string) => {
    setSelectedFiles(prev => {
      const newSet = new Set(prev);
      if (newSet.has(filePath)) {
        newSet.delete(filePath);
      } else {
        newSet.add(filePath);
      }
      return newSet;
    });
  };

  // 全选当前文件夹中的所有文件（不包括文件夹）
  const handleSelectAll = () => {
    const fileOnly = files.filter(f => !f.is_dir);
    if (selectedFiles.size === fileOnly.length) {
      // 已全选，取消全选
      setSelectedFiles(new Set());
    } else {
      // 全选
      setSelectedFiles(new Set(fileOnly.map(f => f.path)));
    }
  };



  // 取消下载
  const handleCancelDownload = async (taskId: string) => {
    const task = downloads.find(t => t.id === taskId);
    if (task?.abortController) {
      console.log('❌ [FileShareManager] 取消下载任务:', taskId);
      task.abortController.abort();
    }

    // 删除已下载的残留文件
    if (task?.savePath) {
      try {
        console.log('🗑️ [FileShareManager] 删除残留文件:', task.savePath);
        await invoke('delete_file', { path: task.savePath });
        console.log('✅ [FileShareManager] 残留文件已删除');
      } catch (error) {
        console.error('❌ [FileShareManager] 删除残留文件失败:', error);
      }

      // 删除临时文件
      try {
        await invoke('delete_file', { path: `${task.savePath}.part` });
        console.log('✅ [FileShareManager] 临时文件已删除');
      } catch (error) {
        // 临时文件可能不存在，忽略错误
      }
    }

    setDownloads(prev => prev.filter(t => t.id !== taskId));
    toast({ title: '已取消下载', status: 'success', duration: 3000, isClosable: true });
  };

  // 打开文件所在文件夹
  const handleOpenFileLocation = async (task: DownloadTask) => {
    try {
      if (task.isBatchDownload) {
        // 批量下载：直接打开文件夹
        await invoke('open_folder', { path: task.savePath });
      } else {
        // 单文件下载：打开文件所在位置并选中文件
        await invoke('open_file_location', { path: task.savePath });
      }
    } catch (error) {
      toast({ title: `打开文件夹失败: ${error}`, status: 'error', duration: 3000, isClosable: true });
    }
  };

  // 格式化大小
  const formatSize = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`;
  };

  // 格式化时间
  const formatTime = (timestamp: number): string => {
    const now = Math.floor(Date.now() / 1000);
    const remaining = timestamp - now;
    if (remaining <= 0) return '已过期';
    const days = Math.floor(remaining / (24 * 60 * 60));
    const hours = Math.floor((remaining % (24 * 60 * 60)) / (60 * 60));
    const minutes = Math.floor((remaining % (60 * 60)) / 60);
    if (days > 0) return `${days}天${hours}时`;
    else if (hours > 0) return `${hours}时${minutes}分`;
    else return `${minutes}分钟`;
  };

  // 格式化速度
  const formatSpeed = (bytesPerSecond: number): string => {
    if (bytesPerSecond === 0) return '0 B/s';
    const k = 1024;
    const sizes = ['B/s', 'KB/s', 'MB/s', 'GB/s'];
    const i = Math.floor(Math.log(bytesPerSecond) / Math.log(k));
    return `${(bytesPerSecond / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`;
  };

  return (
    <Box className="file-share-container" position="relative" bg={bg}>
      {/* 保存中提示 */}
      {savingMessage && (
        <Flex
          position="fixed"
          top="20px"
          left="50%"
          transform="translateX(-50%)"
          zIndex={9999}
          bg={useColorModeValue('white', 'gray.800')}
          color={textColor}
          px={4}
          py={2}
          borderRadius="lg"
          boxShadow="lg"
          alignItems="center"
          gap={2}
          border="1px solid"
          borderColor={borderColor}
        >
          <Spinner size="sm" />
          <Text fontSize="sm">{savingMessage}</Text>
        </Flex>
      )}

      <div className="file-share-content">
        <div className="sidebar-tabs" style={{ background: sidebarBg, borderRight: `1px solid ${sidebarBorder}` }}>
          <Tooltip label="我的共享" placement="right" hasArrow>
            <motion.div
              className={`sidebar-tab ${activeTab === 'local' ? 'active' : ''}`}
              style={{
                background: activeTab === 'local' ? activeColor : tabInactiveBg,
                color: activeTab === 'local' ? contrastText : tabInactiveColor,
              }}
              onClick={() => setActiveTab('local')}
              whileHover={{ x: 4 }}
              whileTap={{ scale: 0.95 }}
            >
              <FolderIcon size={20} />
            </motion.div>
          </Tooltip>
          <Tooltip label="远程共享" placement="right" hasArrow>
            <motion.div
              className={`sidebar-tab ${activeTab === 'remote' ? 'active' : ''}`}
              style={{
                background: activeTab === 'remote' ? activeColor : tabInactiveBg,
                color: activeTab === 'remote' ? contrastText : tabInactiveColor,
              }}
              onClick={() => setActiveTab('remote')}
              whileHover={{ x: 4 }}
              whileTap={{ scale: 0.95 }}
            >
              <ShareIcon size={20} />
            </motion.div>
          </Tooltip>
          <Tooltip label="传输列表" placement="right" hasArrow>
            <motion.div
              className={`sidebar-tab ${activeTab === 'transfers' ? 'active' : ''}`}
              style={{
                background: activeTab === 'transfers' ? activeColor : tabInactiveBg,
                color: activeTab === 'transfers' ? contrastText : tabInactiveColor,
              }}
              onClick={() => setActiveTab('transfers')}
              whileHover={{ x: 4 }}
              whileTap={{ scale: 0.95 }}
            >
              <DownloadIcon size={20} />
              {downloads.filter(t => t.status === 'downloading').length > 0 && (
                <Badge
                  className="transfer-badge"
                  bg={badgeBg}
                  color="white"
                >
                  {downloads.filter(t => t.status === 'downloading').length}
                </Badge>
              )}
            </motion.div>
          </Tooltip>
        </div>
        <div className="content-area" style={{ background: bg }}>
          <AnimatePresence mode="wait">
            {activeTab === 'local' && (
              <motion.div key="local" className="tab-content" initial={{ opacity: 0, y: -20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: 20 }} transition={{ duration: 0.2 }}>
                <div className="share-list">
                  <Button
                    colorScheme="green"
                    leftIcon={<FolderIcon size={16} />}
                    onClick={() => setShowAddShare(true)}
                    mb={4}
                  >
                    添加共享文件夹
                  </Button>
                  <AnimatePresence>
                    {localShares.map((share) => (
                      <motion.div key={share.id} className="share-item" initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -20 }}
                        style={{ background: cardBg, borderColor: borderColor }}
                      >
                        <Box color={activeColor} flexShrink={0}>
                          <FolderIcon size={24} />
                        </Box>
                        <Box flex={1} minW={0}>
                          <Text fontSize="13px" fontWeight={600} color={textColorStrong} mb="2px" isTruncated>{share.name}</Text>
                          <Text fontSize="11px" color={mutedText}>{share.password && '🔒 '}{share.compress_before_send && '📦 '}{share.expire_time && `⏰ ${formatTime(share.expire_time)}`}</Text>
                        </Box>
                        <IconButton
                          aria-label="删除共享"
                          icon={<TrashIcon size={16} />}
                          size="sm"
                          variant="outline"
                          bg={deleteBtnBg}
                          color={deleteBtnColor}
                          borderColor={deleteBtnBorder}
                          _hover={{ bg: useColorModeValue('red.100', 'rgba(239, 68, 68, 0.3)'), borderColor: useColorModeValue('red.300', 'rgba(239, 68, 68, 0.6)') }}
                          onClick={() => handleDeleteShare(share.id)}
                          title="删除共享"
                        />
                      </motion.div>
                    ))}
                  </AnimatePresence>
                  {localShares.length === 0 && (
                    <Flex direction="column" align="center" justify="center" h="100%" color={emptyColor}>
                      <Box opacity={0.3} mb={4}><ShareIcon size={48} /></Box>
                      <Text fontSize="14px">还没有共享文件夹</Text>
                    </Flex>
                  )}
                </div>
              </motion.div>
            )}
            {activeTab === 'remote' && (
              <motion.div key="remote" className="tab-content" initial={{ opacity: 0, y: -20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: 20 }} transition={{ duration: 0.2 }}>
                {!selectedShare ? (
                  <div className="share-list">
                    <AnimatePresence>
                      {remoteShares.map((remoteShare, index) => (
                        <motion.div key={`${remoteShare.ownerIp}_${remoteShare.share.id}_${index}`} className="share-item clickable" initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -20 }} onClick={() => handleBrowseShare(remoteShare)}
                          style={{ background: cardBg, borderColor: borderColor }}
                        >
                          <Box color={activeColor} flexShrink={0}>
                            <FolderIcon size={24} />
                          </Box>
                          <Box flex={1} minW={0}>
                            <Text fontSize="13px" fontWeight={600} color={textColorStrong} mb="2px" isTruncated>{remoteShare.share.name}</Text>
                            <Text fontSize="11px" color={mutedText}>{remoteShare.ownerName}</Text>
                          </Box>
                          {/* 右上角状态图标 */}
                          <div className="share-status-icons">
                            {remoteShare.share.password && (
                              <Tooltip label="需要密码" hasArrow>
                                <div className="status-icon lock-icon" style={{ background: useColorModeValue('rgba(0,0,0,0.08)', 'rgba(0,0,0,0.5)'), borderColor: useColorModeValue('rgba(255,193,7,0.3)', 'rgba(255,193,7,0.3)') }}>🔒</div>
                              </Tooltip>
                            )}
                            {remoteShare.share.compress_before_send && (
                              <Tooltip label="先压后发" hasArrow>
                                <div className="status-icon compress-icon" style={{ background: useColorModeValue('rgba(0,0,0,0.08)', 'rgba(0,0,0,0.5)'), borderColor: useColorModeValue('rgba(76,175,80,0.3)', 'rgba(76,175,80,0.3)') }}>📦</div>
                              </Tooltip>
                            )}
                            {remoteShare.share.expire_time && (
                              <Tooltip label={`有效期至 ${new Date(remoteShare.share.expire_time * 1000).toLocaleString()}`} hasArrow>
                                <div className="status-icon expiry-icon" style={{ background: useColorModeValue('rgba(0,0,0,0.08)', 'rgba(0,0,0,0.5)'), borderColor: useColorModeValue('rgba(33,150,243,0.3)', 'rgba(33,150,243,0.3)') }}>⏰</div>
                              </Tooltip>
                            )}
                          </div>
                        </motion.div>
                      ))}
                    </AnimatePresence>
                    {remoteShares.length === 0 && (
                      <Flex direction="column" align="center" justify="center" h="100%" color={emptyColor}>
                        <Box opacity={0.3} mb={4}><ShareIcon size={48} /></Box>
                        <Text fontSize="14px">暂无可用的共享文件夹</Text>
                      </Flex>
                    )}
                  </div>
                ) : (
                  <div className="file-browser">
                    <div className="browser-header" style={{ background: browserHeaderBg }}>
                      <HStack gap={2} align="center" flex={1}>
                        <Button size="sm" onClick={handleGoBack} isDisabled={!currentPath} leftIcon={<BackIcon size={16} />} title="返回上级">返回</Button>
                        <Button size="sm" onClick={handleGoToRoot} isDisabled={!currentPath} title="返回根目录">根目录</Button>
                        <Button size="sm" onClick={handleSelectAll} title={selectedFiles.size === files.filter(f => !f.is_dir).length ? '取消全选' : '全选文件'}>
                          {selectedFiles.size === files.filter(f => !f.is_dir).length && files.filter(f => !f.is_dir).length > 0 ? '取消全选' : '全选'}
                        </Button>
                      </HStack>
                      <IconButton
                        aria-label="关闭"
                        icon={<CloseIcon size={16} />}
                        size="sm"
                        onClick={handleExitShareBrowser}
                        ml="auto"
                      />
                    </div>
                    <div className="file-list">
                      {loadingFiles ? (
                        <Flex align="center" justify="center" h="100%" color={loadingColor}>
                          <Spinner size="lg" />
                        </Flex>
                      ) : (
                        <AnimatePresence>
                          {files.map((file) => (
                            <motion.div
                              key={file.path}
                              className={`file-item ${file.is_dir ? 'clickable' : ''}`}
                              initial={{ opacity: 0 }}
                              animate={{ opacity: 1 }}
                              exit={{ opacity: 0 }}
                              style={{ display: 'flex', alignItems: 'center', gap: 8, background: fileItemBg }}
                            >
                              {!file.is_dir && (
                                <Checkbox
                                  isChecked={selectedFiles.has(file.path)}
                                  onChange={() => toggleFileSelection(file.path)}
                                  onClick={(e: React.MouseEvent) => e.stopPropagation()}
                                  colorScheme="green"
                                  flexShrink={0}
                                />
                              )}
                              {file.is_dir && <Box w="16px" flexShrink={0} />}
                              <Flex
                                align="center"
                                flex={1}
                                cursor={file.is_dir ? 'pointer' : 'default'}
                                minW={0}
                                gap={2}
                                onClick={() => file.is_dir && handleEnterFolder(file)}
                              >
                                {file.is_dir && <Box color={activeColor}><FolderIcon size={20} /></Box>}
                                <Box flex={1} minW={0}>
                                  <Text fontSize="13px" color={textColorStrong} mb="2px" isTruncated title={file.name}>{file.name}</Text>
                                  <Text fontSize="11px" color={mutedText}>{!file.is_dir && formatSize(file.size)}</Text>
                                </Box>
                              </Flex>
                              {!file.is_dir && (
                                <IconButton
                                  aria-label="下载"
                                  icon={<DownloadIcon size={14} />}
                                  size="sm"
                                  onClick={(e: React.MouseEvent) => { e.stopPropagation(); handleDownloadFile(file); }}
                                  title="下载"
                                  flexShrink={0}
                                />
                              )}
                            </motion.div>
                          ))}
                        </AnimatePresence>
                      )}
                      {!loadingFiles && files.length === 0 && (
                        <Flex direction="column" align="center" justify="center" h="100%" color={emptyColor}>
                          <Box opacity={0.3} mb={4}><FolderIcon size={48} /></Box>
                          <Text fontSize="14px">文件夹为空</Text>
                        </Flex>
                      )}
                    </div>
                    {/* 悬浮批量下载按钮 */}
                    {selectedFiles.size > 0 && (
                      <motion.div
                        initial={{ scale: 0, opacity: 0 }}
                        animate={{ scale: 1, opacity: 1 }}
                        exit={{ scale: 0, opacity: 0 }}
                        style={{
                          position: 'fixed',
                          bottom: 24,
                          right: 24,
                          zIndex: 1000
                        }}
                      >
                        <Box position="relative">
                          <IconButton
                            aria-label={`下载选中 (${selectedFiles.size})`}
                            icon={<DownloadIcon size={18} />}
                            size="lg"
                            colorScheme="green"
                            borderRadius="full"
                            onClick={handleBatchDownload}
                            title={`下载选中 (${selectedFiles.size})`}
                            boxShadow="0 4px 12px rgba(82, 196, 26, 0.4)"
                            w="48px"
                            h="48px"
                          />
                          <Badge
                            position="absolute"
                            top="-8px"
                            right="-8px"
                            bg="red.500"
                            color="white"
                            borderRadius="full"
                            w="20px"
                            h="20px"
                            display="flex"
                            alignItems="center"
                            justifyContent="center"
                            fontSize="11px"
                            fontWeight="bold"
                          >
                            {selectedFiles.size}
                          </Badge>
                        </Box>
                      </motion.div>
                    )}
                  </div>
                )}
              </motion.div>
            )}
            {activeTab === 'transfers' && (
              <motion.div key="transfers" className="tab-content" initial={{ opacity: 0, y: -20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: 20 }} transition={{ duration: 0.2 }}>
                {/* 子标签 */}
                <div className="transfers-subtabs" style={{ background: subtabBg, borderBottom: `1px solid ${subtabBorder}` }}>
                  <div
                    className={`subtab ${transferSubTab === 'downloading' ? 'active' : ''}`}
                    onClick={() => setTransferSubTab('downloading')}
                    style={{
                      background: transferSubTab === 'downloading' ? activeColor : tabInactiveBg,
                      color: transferSubTab === 'downloading' ? contrastText : tabInactiveColor,
                    }}
                  >
                    正在下载
                    {downloads.filter(d => d.status === 'downloading').length > 0 && (
                      <Badge
                        className="subtab-badge"
                        bg={badgeBg}
                        color="white"
                      >
                        {downloads.filter(d => d.status === 'downloading').length}
                      </Badge>
                    )}
                  </div>
                  <div
                    className={`subtab ${transferSubTab === 'completed' ? 'active' : ''}`}
                    onClick={() => setTransferSubTab('completed')}
                    style={{
                      background: transferSubTab === 'completed' ? activeColor : tabInactiveBg,
                      color: transferSubTab === 'completed' ? contrastText : tabInactiveColor,
                    }}
                  >
                    已完成
                    {downloads.filter(d => d.status === 'completed').length > 0 && (
                      <Badge
                        className="subtab-badge"
                        bg={badgeBg}
                        color="white"
                      >
                        {downloads.filter(d => d.status === 'completed').length}
                      </Badge>
                    )}
                  </div>
                </div>

                <div className="transfer-list">
                  {(() => {
                    const filteredDownloads = transferSubTab === 'downloading'
                      ? downloads.filter(d => d.status === 'downloading' || d.status === 'failed')
                      : downloads.filter(d => d.status === 'completed');

                    if (filteredDownloads.length === 0) {
                      return (
                        <Flex direction="column" align="center" justify="center" h="100%" color={emptyColor}>
                          <Box opacity={0.3} mb={4}><DownloadIcon size={48} /></Box>
                          <Text fontSize="14px">{transferSubTab === 'downloading' ? '暂无正在下载的任务' : '暂无已完成的任务'}</Text>
                        </Flex>
                      );
                    }

                    return (
                      <AnimatePresence>
                        {filteredDownloads.map((task) => (
                          <motion.div
                            key={task.id}
                            className={`transfer-item ${task.status === 'completed' ? 'clickable' : ''}`}
                            initial={{ opacity: 0, y: 20 }}
                            animate={{ opacity: 1, y: 0 }}
                            exit={{ opacity: 0, y: -20 }}
                            onClick={() => task.status === 'completed' && handleOpenFileLocation(task)}
                            style={{ position: 'relative', background: cardBg }}
                          >
                            {/* 取消按钮 - 右上角 */}
                            {task.status !== 'completed' && (
                              <IconButton
                                aria-label="取消下载"
                                icon={<CloseIcon size={12} />}
                                size="xs"
                                position="absolute"
                                top="8px"
                                right="8px"
                                bg={cancelBtnBg}
                                color="white"
                                borderColor={cancelBtnBorder}
                                borderRadius="4px"
                                _hover={{ bg: 'red.600' }}
                                onClick={(e: React.MouseEvent) => { e.stopPropagation(); handleCancelDownload(task.id); }}
                                title="取消下载"
                                minW="22px"
                                h="22px"
                                w="22px"
                                p={0}
                              />
                            )}

                            <Box flex={1} minW={0} pr="30px">
                              <Text fontSize="13px" color={textColorStrong} mb={2} isTruncated title={task.fileName}>{task.fileName}</Text>
                              <Progress
                                value={task.fileSize > 0 ? Math.round((task.downloaded / task.fileSize) * 100) : 0}
                                size="sm"
                                colorScheme={task.status === 'failed' ? 'red' : 'green'}
                                hasStripe={task.status === 'downloading'}
                                isAnimated={task.status === 'downloading'}
                              />
                              <Flex justify="space-between" align="center" fontSize="11px" color={mutedText} mt={1}>
                                <Text>{formatSize(task.downloaded)} / {formatSize(task.fileSize)}</Text>
                                <Text>
                                  {task.status === 'downloading' && task.speed && `${formatSpeed(task.speed)}`}
                                  {task.status === 'downloading' && !task.speed && ' - 下载中'}
                                  {task.status === 'completed' && ' - 已完成'}
                                  {task.status === 'failed' && ` - 失败: ${task.error}`}
                                </Text>
                              </Flex>
                            </Box>
                          </motion.div>
                        ))}
                      </AnimatePresence>
                    );
                  })()}
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </div>
      {showAddShare && <AddShareDialog visible={showAddShare} onClose={() => setShowAddShare(false)} onSuccess={() => { setShowAddShare(false); loadLocalShares(); }} />}

      {/* 密码输入弹窗 */}
      <Modal isOpen={showPasswordModal} onClose={() => { setShowPasswordModal(false); setPasswordInput(''); setPendingShare(null); pendingBrowsePathRef.current = ''; }} isCentered>
        <ModalOverlay />
        <ModalContent maxW="400px">
          <ModalHeader>输入密码</ModalHeader>
          <ModalCloseButton />
          <ModalBody>
            <InputGroup mt={4}>
              <Input
                autoFocus
                type={showPassword ? 'text' : 'password'}
                value={passwordInput}
                onChange={(e) => setPasswordInput(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter' && pendingShare) openShare(pendingShare, passwordInput); }}
                placeholder="请输入共享密码"
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
          </ModalBody>
          <ModalFooter>
            <Button variant="ghost" mr={3} onClick={() => { setShowPasswordModal(false); setPasswordInput(''); setPendingShare(null); pendingBrowsePathRef.current = ''; }}>取消</Button>
            <Button colorScheme="green" onClick={() => pendingShare && openShare(pendingShare, passwordInput)}>确定</Button>
          </ModalFooter>
        </ModalContent>
      </Modal>
    </Box>
  );
};

// 添加共享对话框
interface AddShareDialogProps {
  visible: boolean;
  onClose: () => void;
  onSuccess: () => void;
}

const AddShareDialog: React.FC<AddShareDialogProps> = ({ visible, onClose, onSuccess }) => {
  const toast = useToast();
  const { getActiveColor, getHoverColor, getBorderColor } = useThemeColor();
  const textColor = useColorModeValue('gray.800', 'white');
  const mutedText = useColorModeValue('gray.500', 'rgba(255,255,255,0.5)');
  const activeColor = getActiveColor();

  const [folderPath, setFolderPath] = useState('');
  const [folderName, setFolderName] = useState('');
  const [hasPassword, setHasPassword] = useState(false);
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);

  // 从Store获取玩家信息
  const { currentPlayerId, config } = useAppStore();
  const [hasExpiry, setHasExpiry] = useState(false);
  const [expiryDays, setExpiryDays] = useState(0);
  const [expiryHours, setExpiryHours] = useState(0);
  const [expiryMinutes, setExpiryMinutes] = useState(0);
  const [compressBeforeSend, setCompressBeforeSend] = useState(false);
  const [loading, setLoading] = useState(false);

  const handleSelectFolder = async () => {
    try {
      const path = await invoke<string | null>('select_folder');
      if (path) {
        setFolderPath(path);
        const name = await invoke<string>('get_folder_name', { path });
        setFolderName(name || '未命名文件夹');
      }
    } catch (error) {
      toast({ title: `选择文件夹失败: ${error}`, status: 'error', duration: 3000, isClosable: true });
    }
  };

  const handleSubmit = async () => {
    if (!folderPath) {
      toast({ title: '请选择要共享的文件夹', status: 'error', duration: 3000, isClosable: true });
      return;
    }
    if (hasPassword && !password) {
      toast({ title: '请输入密码', status: 'error', duration: 3000, isClosable: true });
      return;
    }
    if (hasExpiry && expiryDays === 0 && expiryHours === 0 && expiryMinutes === 0) {
      toast({ title: '请设置有效期时长', status: 'error', duration: 3000, isClosable: true });
      return;
    }
    try {
      setLoading(true);
      let expiryTimestamp: number | undefined;
      if (hasExpiry) {
        const totalSeconds = expiryDays * 24 * 60 * 60 + expiryHours * 60 * 60 + expiryMinutes * 60;
        expiryTimestamp = Math.floor(Date.now() / 1000) + totalSeconds;
      }
      const share: SharedFolder = {
        id: `share_${Date.now()}`,
        name: folderName,
        path: folderPath,
        password: hasPassword ? password : undefined,
        expire_time: expiryTimestamp,
        compress_before_send: compressBeforeSend,
        owner_id: 'local',
        created_at: Math.floor(Date.now() / 1000),
      };
      await invoke('add_shared_folder', { share });

      // 【事件驱动】通过信令服务器广播文件共享添加事件
      try {
        const { webrtcClient } = await import('../../services/webrtc');
        if (webrtcClient && currentPlayerId) {
          console.log('📡 [FileShareManager] 广播文件共享添加事件');
          webrtcClient.sendWebSocketMessage({
            type: 'file-share-added',
            from: currentPlayerId,
            shareId: share.id,
            shareName: share.name,
            playerName: config.playerName || '未知玩家',
            hasPassword: !!share.password,
          });
        }
      } catch (error) {
        console.error('❌ [FileShareManager] 广播文件共享添加事件失败:', error);
        // 不影响主流程
      }

      toast({ title: '共享文件夹已添加', status: 'success', duration: 3000, isClosable: true });
      onSuccess();
    } catch (error) {
      toast({ title: `添加共享失败: ${error}`, status: 'error', duration: 3000, isClosable: true });
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal isOpen={visible} onClose={onClose} isCentered>
      <ModalOverlay />
      <ModalContent maxW="500px">
        <ModalHeader>添加共享文件夹</ModalHeader>
        <ModalCloseButton />
        <ModalBody>
          <VStack gap={4} py={4}>
            {/* 选择文件夹 */}
            <VStack gap={2} align="stretch" w="100%">
              <Text fontSize="14px" color={textColor}>选择文件夹</Text>
              <HStack gap={2}>
                <Input value={folderPath} placeholder="点击选择文件夹" isReadOnly />
                <Button onClick={handleSelectFolder}>选择</Button>
              </HStack>
            </VStack>

            {/* 密码保护 */}
            <VStack gap={2} align="stretch" w="100%">
              <HStack>
                <Switch isChecked={hasPassword} onChange={(e) => setHasPassword(e.target.checked)} colorScheme="green" />
                <Text fontSize="14px" color={textColor} ml={2}>密码保护</Text>
              </HStack>
              {hasPassword && (
                <InputGroup mt={2}>
                  <Input
                    type={showPassword ? 'text' : 'password'}
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    placeholder="输入密码"
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
              )}
            </VStack>

            {/* 设置有效期 */}
            <VStack gap={2} align="stretch" w="100%">
              <HStack>
                <Switch isChecked={hasExpiry} onChange={(e) => setHasExpiry(e.target.checked)} colorScheme="green" />
                <Text fontSize="14px" color={textColor} ml={2}>设置有效期</Text>
              </HStack>
              {hasExpiry && (
                <HStack gap={2} mt={2} flexWrap="wrap">
                  <HStack gap={1}>
                    <Input type="number" min={0} value={expiryDays} onChange={(e) => setExpiryDays(Math.max(0, parseInt(e.target.value) || 0))} placeholder="0" w="80px" />
                    <Text>天</Text>
                  </HStack>
                  <HStack gap={1}>
                    <Input type="number" min={0} max={23} value={expiryHours} onChange={(e) => setExpiryHours(Math.max(0, Math.min(23, parseInt(e.target.value) || 0)))} placeholder="0" w="80px" />
                    <Text>时</Text>
                  </HStack>
                  <HStack gap={1}>
                    <Input type="number" min={0} max={59} value={expiryMinutes} onChange={(e) => setExpiryMinutes(Math.max(0, Math.min(59, parseInt(e.target.value) || 0)))} placeholder="0" w="80px" />
                    <Text>分</Text>
                  </HStack>
                </HStack>
              )}
            </VStack>

            {/* 先压后发 */}
            <VStack gap={2} align="stretch" w="100%">
              <HStack>
                <Switch isChecked={compressBeforeSend} onChange={(e) => setCompressBeforeSend(e.target.checked)} colorScheme="green" />
                <Text fontSize="14px" color={textColor} ml={2}>先压后发</Text>
              </HStack>
              <Text fontSize="12px" color={mutedText} mt={1}>
                开启后，其他玩家批量下载多个文件时，会先自动打包成ZIP压缩包再下载
              </Text>
            </VStack>
          </VStack>
        </ModalBody>
        <ModalFooter>
          <Button variant="ghost" mr={3} onClick={onClose}>取消</Button>
          <Button colorScheme="green" onClick={handleSubmit} isLoading={loading}>确定</Button>
        </ModalFooter>
      </ModalContent>
    </Modal>
  );
};
