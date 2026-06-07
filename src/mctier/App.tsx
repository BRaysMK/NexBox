import { useEffect } from 'react';
import { ErrorBoundary, MainWindow, MiniWindow } from './components';
import { useAppStore } from './stores';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { hotkeyManager, webrtcClient, audioService, fileShareService } from './services';
import { screenShareService } from './services/screenShare/ScreenShareService';
import { p2pChatService } from './services/chat/P2PChatService';
import type { UserConfig } from './types';
import './App.css';

export function MCTierApp() {
  const appState = useAppStore((state) => state.appState);
  const lobby = useAppStore((state) => state.lobby);
  const setMicEnabled = useAppStore((state) => state.setMicEnabled);
  const addPlayer = useAppStore((state) => state.addPlayer);
  const removePlayer = useAppStore((state) => state.removePlayer);
  const updatePlayerStatus = useAppStore((state) => state.updatePlayerStatus);
  const setCurrentPlayerId = useAppStore((state) => state.setCurrentPlayerId);
  const currentPlayerId = useAppStore((state) => state.currentPlayerId);
  const addChatMessage = useAppStore((state) => state.addChatMessage);

  // 初始化应用
  useEffect(() => {
    let isCleaningUp = false;

    const init = async () => {
      try {
        // 生成玩家ID
        const timestamp = Date.now();
        const randomSuffix = Math.random().toString(36).substring(2, 11);
        const playerId = `player-${timestamp}-${randomSuffix}`;
        setCurrentPlayerId(playerId);
        console.log('生成玩家ID:', playerId);

        // 清理函数（页面卸载时）
        const cleanup = () => {
          if (isCleaningUp) return;
          isCleaningUp = true;
          console.log('MCTier 页面卸载，清理资源...');
          hotkeyManager.cleanup();
          webrtcClient.cleanup();
        };

        // 从后端加载用户配置
        try {
          const userConfig = await invoke<UserConfig>('get_config');
          console.log('已加载用户配置:', userConfig);
          const { updateConfig } = useAppStore.getState();
          updateConfig(userConfig);
        } catch (error) {
          console.warn('加载用户配置失败，使用默认配置:', error);
        }

        // 初始化快捷键管理器
        await hotkeyManager.initialize();

        // 监听后端全局快捷键触发的麦克风状态变化事件
        const unlistenMicToggled = await listen<boolean>('mic-toggled', (event) => {
          const newState = event.payload;
          setMicEnabled(newState);
          webrtcClient.setMicEnabled(newState);
          console.log('麦克风状态已更新:', newState);
        });

        // 监听后端全局快捷键触发的全局静音状态变化事件
        const unlistenGlobalMuteToggled = await listen<boolean>('global-mute-toggled', (event) => {
          const newState = event.payload;
          const { toggleGlobalMute, globalMuted } = useAppStore.getState();
          if (globalMuted !== newState) {
            toggleGlobalMute();
          }
          console.log('全局听筒状态已更新:', newState ? '静音' : '开启');
        });

        console.log('MCTier 初始化完成');

        return () => {
          unlistenMicToggled();
          unlistenGlobalMuteToggled();
        };
      } catch (error) {
        console.error('MCTier 初始化失败:', error);
        return undefined;
      }
    };

    let cleanup: (() => void) | undefined;
    init().then((cleanupFn) => {
      cleanup = cleanupFn;
    });

    return () => {
      if (cleanup) {
        cleanup();
      }
      hotkeyManager.cleanup();
      webrtcClient.cleanup();
    };
  }, [setMicEnabled]);

  // 当进入大厅时初始化WebRTC
  useEffect(() => {
    if (appState === 'in-lobby' && lobby) {
      const initWebRTC = async () => {
        try {
          const { currentPlayerId: playerId } = useAppStore.getState();

          if (!playerId) {
            console.error('玩家ID不存在，无法初始化WebRTC');
            return;
          }

          console.log('使用已存在的玩家ID初始化WebRTC:', playerId);

          const playerName = useAppStore.getState().config.playerName || '未知玩家';
          console.log('使用玩家名称:', playerName);

          addPlayer({
            id: playerId,
            name: playerName,
            micEnabled: false,
            isMuted: false,
            joinedAt: new Date().toISOString(),
          });

          webrtcClient.onVersionError((currentVersion, minimumVersion, downloadUrl) => {
            console.log(`WebRTC: 版本错误 - 当前版本: ${currentVersion}, 最低要求: ${minimumVersion}`);
            const { setVersionError } = useAppStore.getState();
            setVersionError({ currentVersion, minimumVersion, downloadUrl });
          });

          const signalingServer = lobby.signalingServer || 'wss://mctier.pmhs.top/signaling';
          console.log('使用信令服务器:', signalingServer);

          await webrtcClient.initialize(playerId, playerName, lobby.name, lobby.password || '', lobby.virtualDomain, lobby.useDomain, signalingServer);

          const ws = (webrtcClient as any).websocket;
          if (ws) {
            screenShareService.initialize(playerId, playerName, ws);
            console.log('屏幕共享服务已初始化');
          }

          webrtcClient.onPlayerJoined((playerId, playerName, virtualIp, virtualDomain, useDomain) => {
            console.log(`WebRTC: 玩家加入 - ${playerName} (${playerId})`);
            addPlayer({
              id: playerId,
              name: playerName,
              virtualIp: virtualIp,
              virtualDomain: virtualDomain,
              useDomain: useDomain,
              micEnabled: false,
              isMuted: false,
              joinedAt: new Date().toISOString(),
            });
          });

          webrtcClient.onPlayerLeft((playerId) => {
            console.log(`WebRTC: 玩家离开 - ${playerId}`);
            removePlayer(playerId);
          });

          webrtcClient.onStatusUpdate((playerId, micEnabled) => {
            updatePlayerStatus(playerId, { micEnabled });
          });

          webrtcClient.onRemoteStream((playerId, _stream) => {
            console.log(`WebRTC: 接收到远程音频流 - ${playerId}`);
          });

          webrtcClient.onChatMessage((playerId, playerName, content, timestamp) => {
            addChatMessage({
              id: `${playerId}-${timestamp}`,
              playerId,
              playerName,
              content,
              timestamp,
            });

            if (playerId !== currentPlayerId) {
              const isInChatRoom = (window as any).__isInChatRoom__ || false;
              if (!isInChatRoom) {
                audioService.play('newMessage').catch(err => {
                  console.error('播放新消息音效失败:', err);
                });
              }
            }
          });

          console.log('WebRTC 初始化完成，玩家ID:', playerId);

          try {
            await fileShareService.startServer(lobby.virtualIp);
            console.log('HTTP文件服务器启动成功');
          } catch (error) {
            console.error('启动HTTP文件服务器失败:', error);
          }
        } catch (error) {
          console.error('WebRTC 初始化失败:', error);
        }
      };

      initWebRTC();
    }
  }, [appState, lobby, addPlayer, removePlayer, updatePlayerStatus, setCurrentPlayerId, addChatMessage]);

  // 监听来自后端的退出请求事件（托盘退出）
  useEffect(() => {
    let isHandlingExit = false;

    const unlisten = listen('lobby-exit-requested', async () => {
      if (isHandlingExit) return;
      isHandlingExit = true;

      const state = useAppStore.getState();
      const { appState, lobby } = state;

      // 先显示并聚焦窗口
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const appWindow = getCurrentWindow();
        await appWindow.show();
        await appWindow.setFocus();
        await appWindow.unminimize();
        console.log('窗口已显示并聚焦');
      } catch (error) {
        console.error('显示窗口失败:', error);
      }

      // 如果不在大厅中，直接退出
      if (appState !== 'in-lobby' || !lobby) {
        console.log('不在大厅中，直接退出应用');
        await invoke('force_exit_app');
        return;
      }

      // 在大厅中：标记退出后关闭软件，触发 MiniWindow 的 handleLeaveLobby
      console.log('托盘退出：在大厅中，触发退出大厅后关闭软件...');
      state.setShouldExitAfterLeave(true);
      state.setPendingLeaveLobby(true);
    });

    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  // 拦截窗口关闭按钮 (X) — 在大厅中时返回主界面，否则直接退出
  useEffect(() => {
    let unlistenFn: (() => void) | undefined;

    import('@tauri-apps/api/window').then(({ getCurrentWindow }) => {
      const appWindow = getCurrentWindow();
      appWindow.onCloseRequested(async (event) => {
        event.preventDefault();

        const state = useAppStore.getState();
        const { appState, lobby } = state;

        // 如果不在大厅中，直接退出
        if (appState !== 'in-lobby' || !lobby) {
          console.log('X关闭：不在大厅中，直接退出应用');
          await invoke('force_exit_app');
          return;
        }

        // 在大厅中：标记退出后关闭软件，触发 MiniWindow 的 handleLeaveLobby
        console.log('X关闭：在大厅中，触发退出大厅后关闭软件...');
        state.setShouldExitAfterLeave(true);
        state.setPendingLeaveLobby(true);
      });
    });

    return () => {
      unlistenFn?.();
    };
  }, []);

  return (
    <ErrorBoundary>
      <div className="app-container" style={{ height: '100%' }}>
        {appState === 'in-lobby' && lobby ? <MiniWindow /> : <MainWindow />}
      </div>
    </ErrorBoundary>
  );
}

export default MCTierApp;
