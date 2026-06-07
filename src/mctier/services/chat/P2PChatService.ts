/**
 * P2P聊天服务
 * 基于HTTP over WireGuard的点对点聊天
 * 使用SSE(Server-Sent Events)实现事件驱动的消息推送
 * 不依赖中心服务器，直接在虚拟局域网中传输
 */

import { invoke } from '@tauri-apps/api/core';
import type { ChatMessage } from '../../types';

interface BackendChatMessage {
  id: string;
  player_id: string;
  player_name: string;
  content: string;
  message_type: 'text' | 'image';
  timestamp: number;
  image_data?: number[]; // Uint8Array转换为number[]
}

class P2PChatService {
  private eventSources: Map<string, EventSource> = new Map(); // 每个玩家一个EventSource
  private onMessageCallback?: (message: ChatMessage) => void;
  private peerIps: string[] = [];
  private currentPlayerId: string = '';
  private myVirtualIp: string = ''; // 自己的虚拟IP，用于过滤
  private currentPlayerName: string = ''; // 当前玩家名称，用于发送消息
  private lastMessageIdByPlayer: Map<string, string> = new Map(); // 记录每个玩家最近一次发送的消息ID
  private receivedMessageFingerprints: Set<string> = new Set(); // 去重指纹集 (playerId:timestamp:contentHash)
  private pollTimer: number | null = null; // 轮询定时器
  private lastPollTimestamp: number = 0; // 上次轮询的时间戳

  /**
   * 初始化服务
   */
  initialize(peerIps: string[], currentPlayerId: string, myVirtualIp: string, playerName: string): void {
    // 【修复】先清理旧的连接，避免重复连接
    console.log('🔄 [P2PChatService] 清理旧连接...');
    this.stopListening();

    // 更新玩家IPs和ID
    this.peerIps = peerIps;
    this.currentPlayerId = currentPlayerId;
    this.myVirtualIp = myVirtualIp;
    this.currentPlayerName = playerName;
    
    console.log('✅ [P2PChatService] 初始化完成');
    console.log('  - 当前玩家ID:', currentPlayerId);
    console.log('  - 自己的虚拟IP:', myVirtualIp);
    console.log('  - 其他玩家IPs:', peerIps);
  }
  
  /**
   * 重置服务状态（退出大厅时调用）
   */
  reset(): void {
    this.stopPolling();
    this.peerIps = [];
    this.currentPlayerId = '';
    this.myVirtualIp = '';
    this.currentPlayerName = '';
    this.onMessageCallback = undefined;
    this.lastMessageIdByPlayer.clear(); // 清理玩家消息ID记录
    this.receivedMessageFingerprints.clear(); // 清理消息指纹去重集
    this.lastPollTimestamp = 0;
    console.log('🔄 [P2PChatService] 服务已重置');
  }

  /**
   * 设置消息接收回调
   */
  onMessage(callback: (message: ChatMessage) => void): void {
    this.onMessageCallback = callback;
  }

  /**
   * 开始监听消息（使用SSE + 轮询后备）
   */
  startPolling(): void {
    console.log('✅ [P2PChatService] 开始监听消息（SSE事件驱动 + HTTP轮询后备）');
    console.log('📊 [P2PChatService] 当前已有连接数:', this.eventSources.size);
    
    // 【修复】先完全清理所有旧连接
    if (this.eventSources.size > 0) {
      console.log('⚠️ [P2PChatService] 检测到旧连接，先清理所有连接');
      this.stopListening();
    }
    
    // 为每个玩家创建SSE连接
    for (const peerIp of this.peerIps) {
      // 【修复】不再跳过自己的IP，也连接自己的SSE流以接收其他玩家POST的消息
      // 通过消息去重机制避免重复显示
      
      // 【双重检查】确保没有重复连接
      if (this.eventSources.has(peerIp)) {
        console.error(`❌ [P2PChatService] 严重错误：清理后仍存在连接: ${peerIp}`);
        const oldEventSource = this.eventSources.get(peerIp);
        if (oldEventSource) {
          oldEventSource.close();
        }
        this.eventSources.delete(peerIp);
      }
      
      this.connectToPlayer(peerIp);
    }
    
    console.log('📊 [P2PChatService] 连接建立完成，当前连接数:', this.eventSources.size);
    
    // 初始化轮询时间戳为当前时间，避免拉取历史消息
    this.lastPollTimestamp = Math.floor(Date.now() / 1000);
    console.log('⏱️ [P2PChatService] 初始化轮询时间戳:', this.lastPollTimestamp, '(忽略此前的历史消息)');

    // ====== 添加HTTP轮询作为SSE的后备 ======
    // SSE在Tauri WebView中可能不可靠，轮询确保消息不丢失
    this.startPollTimer();
  }

  /**
   * 启动HTTP轮询后备（每2秒轮询一次）
   */
  private startPollTimer(): void {
    if (this.pollTimer !== null) {
      clearInterval(this.pollTimer);
    }
    
    console.log('⏱️ [P2PChatService] 启动HTTP轮询后备（间隔2秒）');
    
    this.pollTimer = window.setInterval(async () => {
      if (this.peerIps.length === 0) return;
      
      try {
        const messages = await invoke<BackendChatMessage[]>('get_p2p_chat_messages', {
          peerIps: this.peerIps,
          since: this.lastPollTimestamp > 0 ? this.lastPollTimestamp : null,
        });
        
        if (messages && messages.length > 0) {
          // 更新最新时间戳
          const maxTimestamp = Math.max(...messages.map(m => m.timestamp));
          if (maxTimestamp > this.lastPollTimestamp) {
            this.lastPollTimestamp = maxTimestamp;
          }
          
          for (const msg of messages) {
            this.handleMessage(msg);
          }
        }
      } catch (error) {
        // 轮询失败静默处理，下次继续
        console.debug('⏱️ [P2PChatService] 轮询消息失败（下次重试）:', error);
      }
    }, 2000);
  }

  /**
   * 连接到指定玩家的SSE流
   */
  private connectToPlayer(peerIp: string): void {
    const url = `http://${peerIp}:14540/api/chat/stream`;
    console.log(`📡 [P2PChatService] 连接到玩家: ${url}`);
    
    try {
      const eventSource = new EventSource(url);
      
      eventSource.onopen = () => {
        console.log(`✅ [P2PChatService] SSE连接已建立: ${peerIp}`);
      };
      
      eventSource.onmessage = (event) => {
        // 跳过keep-alive消息
        if (event.data === 'keep-alive') {
          return;
        }
        
        try {
          const message: BackendChatMessage = JSON.parse(event.data);
          this.handleMessage(message);
        } catch (error) {
          console.error('❌ [P2PChatService] 解析消息失败:', error);
        }
      };
      
      eventSource.onerror = (error) => {
        console.warn(`⚠️ [P2PChatService] SSE连接错误: ${peerIp}`, error);
        // 连接断开，移除EventSource
        this.eventSources.delete(peerIp);
        eventSource.close();
        
        // 5秒后重连
        setTimeout(() => {
          if (this.peerIps.includes(peerIp)) {
            console.log(`🔄 [P2PChatService] 重新连接: ${peerIp}`);
            this.connectToPlayer(peerIp);
          }
        }, 5000);
      };
      
      this.eventSources.set(peerIp, eventSource);
    } catch (error) {
      console.error(`❌ [P2PChatService] 创建SSE连接失败: ${peerIp}`, error);
    }
  }

  /**
   * 处理接收到的消息
   */
  private handleMessage(msg: BackendChatMessage): void {
    // 跳过自己发送的消息
    if (msg.player_id === this.currentPlayerId) {
      console.log('🚫 [P2PChatService] 跳过自己发送的消息:', msg.id);
      return;
    }

    // 【修复】更可靠的消息去重：基于 playerId + timestamp(秒) + contentHash 指纹
    // 当消息同时从发送方SSE和本地SSE流入时需要去重
    const contentPreview = msg.content.substring(0, 50);
    const fingerprint = `${msg.player_id}:${msg.timestamp}:${contentPreview}`;
    if (this.receivedMessageFingerprints.has(fingerprint)) {
      console.log('🚫 [P2PChatService] 跳过重复消息（指纹匹配）:', msg.id, fingerprint);
      return;
    }
    this.receivedMessageFingerprints.add(fingerprint);

    // 消息去重：基于消息ID，避免相同内容消息被误过滤
    const lastMessageId = this.lastMessageIdByPlayer.get(msg.player_id);
    if (lastMessageId === msg.id) {
      console.log('🚫 [P2PChatService] 跳过重复消息（ID相同）:', msg.id);
      return;
    }

    console.log('✅ [P2PChatService] 接收新消息:', `${msg.player_name}: ${msg.message_type === 'text' ? msg.content.substring(0, 20) + '...' : '[图片]'}`);

    // 更新该玩家最近一次发送的消息ID
    this.lastMessageIdByPlayer.set(msg.player_id, msg.id);

    // 转换为前端消息格式
    const chatMessage: ChatMessage = {
      id: msg.id,
      playerId: msg.player_id,
      playerName: msg.player_name,
      content: msg.content,
      timestamp: msg.timestamp * 1000, // 转换为毫秒
      type: msg.message_type,
      imageData: msg.image_data ? this.arrayToBase64(msg.image_data) : undefined,
    };

    // 回调通知新消息
    if (this.onMessageCallback) {
      this.onMessageCallback(chatMessage);
    }

    // 只有在不在聊天室界面时才播放音效
    const isInChatRoom = (window as any).__isInChatRoom__;
    if (!isInChatRoom) {
      this.playNewMessageSound();
    } else {
      console.log('🔕 [P2PChatService] 在聊天室中，跳过播放音效');
    }
  }

  /**
   * 播放新消息音效
   */
  private async playNewMessageSound(): Promise<void> {
    try {
      const { audioService } = await import('../audio/AudioService');
      await audioService.play('newMessage');
      console.log('🔔 [P2PChatService] 播放新消息音效');
    } catch (error) {
      console.error('❌ [P2PChatService] 播放新消息音效失败:', error);
    }
  }

  /**
   * 停止监听消息
   */
  stopPolling(): void {
    this.stopPollTimer();
    this.stopListening();
  }

  /**
   * 停止轮询定时器
   */
  private stopPollTimer(): void {
    if (this.pollTimer !== null) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
      console.log('⏱️ [P2PChatService] 已停止HTTP轮询');
    }
  }

  /**
   * 停止所有SSE连接
   */
  private stopListening(): void {
    for (const [peerIp, eventSource] of this.eventSources.entries()) {
      eventSource.close();
      console.log(`🛑 [P2PChatService] 关闭SSE连接: ${peerIp}`);
    }
    this.eventSources.clear();
  }

  /**
   * 发送文本消息
   */
  async sendTextMessage(content: string): Promise<void> {
    if (!this.currentPlayerId) {
      throw new Error('未初始化：缺少玩家ID');
    }

    try {
      await invoke('send_p2p_chat_message', {
        playerId: this.currentPlayerId,
        playerName: this.currentPlayerName,
        content,
        messageType: 'text',
        imageData: null,
        peerIps: this.peerIps,
      });
      console.log('✅ [P2PChatService] 文本消息已发送');
      // 注意：消息已由 ChatRoom 通过乐观更新(optimisticMessage)添加到 store，无需 echoSentMessage
    } catch (error) {
      console.error('❌ [P2PChatService] 发送文本消息失败:', error);
      // 失败时消息也已通过乐观更新显示在 UI 中
    }
  }

  /**
   * 发送图片消息（Base64格式）
   * 【优化】使用更高效的数据转换方式
   */
  async sendImageMessage(imageDataUrl: string): Promise<void> {
    if (!this.currentPlayerId) {
      throw new Error('未初始化：缺少玩家ID');
    }

    try {
      // 从Data URL中提取Base64数据
      const base64Data = imageDataUrl.split(',')[1];
      
      // 【优化】使用Uint8Array直接转换，避免中间字符串
      const binaryString = atob(base64Data);
      const bytes = new Uint8Array(binaryString.length);
      
      // 分块处理，提高性能
      const chunkSize = 8192;
      for (let i = 0; i < binaryString.length; i += chunkSize) {
        const end = Math.min(i + chunkSize, binaryString.length);
        for (let j = i; j < end; j++) {
          bytes[j] = binaryString.charCodeAt(j);
        }
      }

      const startTime = performance.now();
      
      await invoke('send_p2p_chat_message', {
        playerId: this.currentPlayerId,
        playerName: this.currentPlayerName,
        content: '[图片]',
        messageType: 'image',
        imageData: Array.from(bytes),
        peerIps: this.peerIps,
      });
      
      const elapsed = performance.now() - startTime;
      console.log(`✅ [P2PChatService] 图片消息已发送 (耗时: ${elapsed.toFixed(2)}ms, 大小: ${(bytes.length / 1024).toFixed(2)}KB)`);
      // 注意：消息已由 ChatRoom 通过乐观更新(optimisticMessage)添加到 store，无需 echoSentMessage
    } catch (error) {
      console.error('❌ [P2PChatService] 发送图片消息失败:', error);
      // 失败时消息也已通过乐观更新显示在 UI 中
    }
  }

  /**
   * 清空本地消息
   */
  async clearMessages(): Promise<void> {
    try {
      await invoke('clear_p2p_chat_messages');
      console.log('✅ [P2PChatService] 本地消息已清空');
    } catch (error) {
      console.error('❌ [P2PChatService] 清空消息失败:', error);
      throw error;
    }
  }

  /**
   * 将number数组转换为Base64 Data URL
   * 【优化】直接使用JPEG格式，因为前端已经统一转换为JPEG
   */
  private arrayToBase64(data: number[]): string {
    const bytes = new Uint8Array(data);
    let binary = '';
    const chunkSize = 8192; // 分块处理，提高性能
    
    for (let i = 0; i < bytes.length; i += chunkSize) {
      const chunk = bytes.subarray(i, Math.min(i + chunkSize, bytes.length));
      binary += String.fromCharCode.apply(null, Array.from(chunk));
    }
    
    const base64 = btoa(binary);
    // 前端已统一转换为JPEG格式
    return `data:image/jpeg;base64,${base64}`;
  }
}

export const p2pChatService = new P2PChatService();
