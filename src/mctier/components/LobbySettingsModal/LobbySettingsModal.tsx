import React, { useState, useEffect, useRef } from 'react';
import {
  Modal, ModalOverlay, ModalContent, ModalHeader, ModalBody, ModalFooter,
  Box, VStack, HStack, Text, Button, FormControl, FormLabel,
  Input, Textarea, Spinner, Flex, IconButton, Tooltip,
  useColorModeValue, useToast, Select as ChakraSelect,
} from '@chakra-ui/react';
import { Switch as AntSwitch } from 'antd';
import { invoke } from '@tauri-apps/api/core';
import './LobbySettingsModal.css';

interface LobbySettingsModalProps {
  visible: boolean;
  onClose: () => void;
  currentLobby: {
    name: string;
    password: string;
    virtualIp: string;
  };
  onSettingsSaved: () => void;
}

interface PortForwardRule {
  protocol: string;
  bind_addr: string;
  dst_addr: string;
}

export const LobbySettingsModal: React.FC<LobbySettingsModalProps> = ({
  visible,
  onClose,
  onSettingsSaved,
}) => {
  const toast = useToast();
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [useGlobalConfig, setUseGlobalConfig] = useState(true);
  const [formValues, setFormValues] = useState<Record<string, any>>({});
  const [portForwardRules, setPortForwardRules] = useState<PortForwardRule[]>([]);

  // ref 避免闭包问题
  const useGlobalConfigRef = useRef(useGlobalConfig);
  useEffect(() => { useGlobalConfigRef.current = useGlobalConfig; }, [useGlobalConfig]);

  // ===== 主题感知颜色 =====
  const modalBg = useColorModeValue('white', 'rgba(30,30,40,0.98)');
  const modalBorder = useColorModeValue('gray.200', 'rgba(255,255,255,0.1)');
  const titleColor = useColorModeValue('gray.900', '#fff');
  const subtitleColor = useColorModeValue('gray.500', 'rgba(255,255,255,0.5)');
  const labelColor = useColorModeValue('gray.700', 'rgba(255,255,255,0.85)');
  const inputBg = useColorModeValue('gray.50', 'rgba(255,255,255,0.05)');
  const inputBorder = useColorModeValue('gray.300', 'rgba(255,255,255,0.1)');
  const textColor = useColorModeValue('gray.800', '#fff');
  const mutedText = useColorModeValue('gray.500', 'rgba(255,255,255,0.6)');
  const sectionTitleColor = useColorModeValue('#5a9428', 'rgba(126,211,33,0.95)');
  const sectionBorderColor = useColorModeValue('rgba(90,148,40,0.3)', 'rgba(126,211,33,0.2)');
  const hintBg = useColorModeValue('rgba(90,148,40,0.08)', 'rgba(76,175,80,0.08)');
  const hintBorder = useColorModeValue('rgba(90,148,40,0.25)', 'rgba(76,175,80,0.2)');
  const switchBg = useColorModeValue('#e2e8f0', 'rgba(255,255,255,0.2)');
  const globalSwitchBg = useColorModeValue('rgba(76,175,80,0.1)', 'linear-gradient(135deg, rgba(76,175,80,0.1) 0%, rgba(56,142,60,0.1) 100%)');
  const globalSwitchBorder = useColorModeValue('rgba(76,175,80,0.35)', 'rgba(76,175,80,0.3)');
  const footerBorder = useColorModeValue('gray.200', 'rgba(255,255,255,0.1)');
  const ruleCardBg = useColorModeValue('gray.50', 'rgba(255,255,255,0.03)');
  const ruleCardBorder = useColorModeValue('gray.200', 'rgba(255,255,255,0.1)');
  const addBtnBg = useColorModeValue('rgba(90,148,40,0.1)', 'rgba(126,211,33,0.1)');
  const addBtnBorder = useColorModeValue('rgba(90,148,40,0.3)', 'rgba(126,211,33,0.3)');
  const addBtnColor = useColorModeValue('#5a9428', 'rgba(126,211,33,0.9)');
  const deleteBtnBg = useColorModeValue('rgba(229,62,62,0.1)', 'rgba(255,77,79,0.1)');
  const deleteBtnBorder = useColorModeValue('rgba(229,62,62,0.3)', 'rgba(255,77,79,0.3)');
  const deleteBtnColor = useColorModeValue('#e53e3e', 'rgba(255,77,79,0.9)');
  const selectBg = useColorModeValue('white', 'rgba(30,30,40,0.8)');
  const selectBorder = useColorModeValue('gray.300', 'rgba(126,211,33,0.3)');

  // 加载当前设置
  useEffect(() => {
    if (visible) loadSettings();
  }, [visible]);

  const loadSettings = async () => {
    setLoading(true);
    try {
      const config = await invoke<any>('get_lobby_easytier_advanced_config');
      const useGlobal = config.use_global_config ?? true;
      setUseGlobalConfig(useGlobal);
      setFormValues(config);
      setPortForwardRules(config.port_forward_rules || []);
    } catch (error) {
      console.error('[LobbySettings] 加载设置失败:', error);
      toast({ title: '加载设置失败', status: 'error', duration: 2000 });
    } finally {
      setLoading(false);
    }
  };

  const updateField = (field: string, value: any) => {
    setFormValues(prev => ({ ...prev, [field]: value }));
    if (useGlobalConfig) setUseGlobalConfig(false);
  };

  const handleReset = async () => {
    try {
      setLoading(true);
      await invoke('clear_lobby_easytier_advanced_config');
      await loadSettings();
      toast({ title: '大厅配置已重置为默认值', status: 'success', duration: 2000 });
    } catch (error) {
      console.error('[LobbySettings] 重置配置失败:', error);
      toast({ title: '重置配置失败', status: 'error', duration: 2000 });
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    try {
      let configToSave: any;
      const currentUseGlobalConfig = useGlobalConfigRef.current;

      if (currentUseGlobalConfig) {
        const globalConfig = await invoke<any>('get_global_easytier_advanced_config');
        configToSave = { ...globalConfig, use_global_config: true };
      } else {
        const processArrayField = (field: any): string[] => {
          if (!field) return [];
          if (Array.isArray(field)) return field;
          if (typeof field === 'string') return field.split('\n').map(s => s.trim()).filter(s => s.length > 0);
          return [];
        };

        configToSave = {
          ...formValues,
          use_global_config: false,
          proxy_networks: processArrayField(formValues.proxy_networks),
          exit_nodes: processArrayField(formValues.exit_nodes),
          relay_network_whitelist: processArrayField(formValues.relay_network_whitelist),
          manual_routes: processArrayField(formValues.manual_routes),
          listeners: processArrayField(formValues.listeners),
          mapped_listeners: processArrayField(formValues.mapped_listeners),
          tcp_whitelist: processArrayField(formValues.tcp_whitelist),
          udp_whitelist: processArrayField(formValues.udp_whitelist),
          stun_servers: processArrayField(formValues.stun_servers),
          stun_servers_v6: processArrayField(formValues.stun_servers_v6),
          port_forward_rules: portForwardRules,
        };
      }

      setSaving(true);
      await invoke('save_lobby_easytier_advanced_config', { configJson: configToSave });
      onSettingsSaved();
    } catch (error) {
      console.error('[LobbySettings] 保存设置失败:', error);
      toast({ title: '保存设置失败', status: 'error', duration: 2000 });
    } finally {
      setSaving(false);
    }
  };

  // 渲染表单字段
  const renderInput = (name: string, label: string, placeholder: string) => (
    <FormControl mb={4}>
      <FormLabel fontSize="xs" fontWeight="500" color={labelColor}>{label}</FormLabel>
      <Input
        size="sm"
        value={formValues[name] || ''}
        onChange={(e) => updateField(name, e.target.value)}
        placeholder={placeholder}
        bg={inputBg}
        borderColor={inputBorder}
        color={textColor}
        borderRadius="lg"
        _placeholder={{ color: mutedText }}
        _hover={{ borderColor: sectionTitleColor }}
        _focus={{ borderColor: sectionTitleColor, boxShadow: `0 0 0 2px ${sectionTitleColor}20` }}
      />
    </FormControl>
  );

  const renderTextarea = (name: string, label: string, placeholder: string) => (
    <FormControl mb={4}>
      <FormLabel fontSize="xs" fontWeight="500" color={labelColor}>{label}</FormLabel>
      <Textarea
        size="sm"
        value={formValues[name] || ''}
        onChange={(e) => updateField(name, e.target.value)}
        placeholder={placeholder}
        rows={2}
        bg={inputBg}
        borderColor={inputBorder}
        color={textColor}
        borderRadius="lg"
        _placeholder={{ color: mutedText }}
        _hover={{ borderColor: sectionTitleColor }}
        _focus={{ borderColor: sectionTitleColor, boxShadow: `0 0 0 2px ${sectionTitleColor}20` }}
      />
    </FormControl>
  );

  const renderNumberInput = (name: string, label: string, placeholder: string, min?: number, max?: number) => (
    <FormControl mb={4}>
      <FormLabel fontSize="xs" fontWeight="500" color={labelColor}>{label}</FormLabel>
      <Input
        type="number"
        size="sm"
        value={formValues[name] ?? ''}
        onChange={(e) => updateField(name, e.target.value ? Number(e.target.value) : undefined)}
        placeholder={placeholder}
        bg={inputBg}
        borderColor={inputBorder}
        color={textColor}
        borderRadius="lg"
        min={min}
        max={max}
        _placeholder={{ color: mutedText }}
        _hover={{ borderColor: sectionTitleColor }}
        _focus={{ borderColor: sectionTitleColor, boxShadow: `0 0 0 2px ${sectionTitleColor}20` }}
      />
    </FormControl>
  );

  const renderSwitch = (name: string, label: string) => (
    <HStack justify="space-between" align="center" py={2} px={4} bg={inputBg} borderRadius="lg" mb={4}>
      <FormLabel fontSize="sm" fontWeight="500" color={labelColor} mb={0}>{label}</FormLabel>
      <AntSwitch
        checked={!!formValues[name]}
        onChange={(checked) => updateField(name, checked)}
      />
    </HStack>
  );

  const renderSectionTitle = (title: string) => (
    <Box
      fontSize="md"
      fontWeight="600"
      color={sectionTitleColor}
      mt={6}
      mb={4}
      pb={2}
      borderBottom={`2px solid ${sectionBorderColor}`}
      display="flex"
      alignItems="center"
      gap={2}
    >
      <Box w={1} h={4} bg={sectionTitleColor} rounded="sm" />
      {title}
    </Box>
  );

  return (
    <Modal isOpen={visible} onClose={onClose} isCentered size="lg">
      <ModalOverlay />
      <ModalContent
        bg={modalBg}
        border="1px solid"
        borderColor={modalBorder}
        borderRadius="2xl"
        boxShadow="xl"
        maxW="520px"
        maxH="85vh"
        overflow="hidden"
        className="lobby-settings-modal"
      >
        {/* 标题栏 */}
        <ModalHeader pb={2} pt={6} px={6}>
          <VStack spacing={1} align="center">
            <Text fontSize="xl" fontWeight="600" color={titleColor}>大厅动态设置</Text>
            <Text fontSize="sm" color={subtitleColor}>修改设置后将自动重新加入大厅</Text>
          </VStack>
        </ModalHeader>

        <ModalBody px={6} py={4} overflowY="auto" className="lobby-settings-body">
          {loading ? (
            <Flex justify="center" align="center" py={12}>
              <Spinner size="lg" color={sectionTitleColor} />
            </Flex>
          ) : (
            <>
              {/* 使用全局配置开关 */}
              <HStack
                justify="space-between"
                align="center"
                p={4}
                bg={globalSwitchBg}
                border="1px solid"
                borderColor={globalSwitchBorder}
                borderRadius="xl"
                mb={4}
              >
                <Text fontSize="sm" fontWeight="500" color={titleColor}>使用全局配置</Text>
                <AntSwitch
                  checked={useGlobalConfig}
                  onChange={(checked) => setUseGlobalConfig(checked)}
                />
              </HStack>

              {/* 全局配置提示 */}
              {useGlobalConfig && (
                <HStack
                  gap={3}
                  p={4}
                  bg={hintBg}
                  border="1px solid"
                  borderColor={hintBorder}
                  borderRadius="xl"
                  mb={4}
                >
                  <Box flexShrink={0}>
                    <svg width="20" height="20" viewBox="0 0 24 24" fill="#7ed321">
                      <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/>
                    </svg>
                  </Box>
                  <Text fontSize="sm" color={mutedText} lineHeight="tall">
                    当前使用全局配置，您可以在 MCTier 设置中修改全局配置
                  </Text>
                </HStack>
              )}

              {!useGlobalConfig && (
                <>
                  {/* 网络模式 */}
                  {renderSectionTitle('网络模式')}
                  {renderSwitch('no_tun', '无 TUN 模式')}
                  {renderSwitch('dhcp', '启用 DHCP')}
                  {renderInput('ipv4', '手动指定 IPv4', '10.144.144.1/24')}

                  {/* 代理和转发 */}
                  {renderSectionTitle('代理和转发')}
                  {renderSwitch('enable_socks5', '启用 SOCKS5 代理')}
                  {renderNumberInput('socks5_port', 'SOCKS5 端口', '1080', 1024, 65535)}

                  {/* 端口转发规则 */}
                  <Box mt={4} mb={2}>
                    <HStack justify="space-between" align="center" mb={2}>
                      <Text fontSize="sm" fontWeight="500" color={labelColor}>端口转发规则</Text>
                      <Button
                        size="xs"
                        variant="outline"
                        leftIcon={
                          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
                          </svg>
                        }
                        bg={addBtnBg}
                        borderColor={addBtnBorder}
                        color={addBtnColor}
                        borderRadius="md"
                        onClick={() => setPortForwardRules([...portForwardRules, { protocol: 'tcp', bind_addr: '0.0.0.0:5678', dst_addr: '10.126.126.1:5678' }])}
                        _hover={{ bg: `${addBtnColor}15` }}
                      >
                        添加
                      </Button>
                    </HStack>
                    <Text fontSize="xs" color={mutedText} mb={3}>将本地端口转发到虚拟网络中的远程端口</Text>

                    {portForwardRules.map((rule, idx) => (
                      <Box
                        key={idx}
                        p={3}
                        bg={ruleCardBg}
                        border="1px solid"
                        borderColor={ruleCardBorder}
                        borderRadius="lg"
                        mb={3}
                        position="relative"
                      >
                        <IconButton
                          aria-label="删除规则"
                          position="absolute"
                          top={2}
                          right={2}
                          size="xs"
                          variant="ghost"
                          bg={deleteBtnBg}
                          borderColor={deleteBtnBorder}
                          color={deleteBtnColor}
                          borderRadius="md"
                          fontSize="xs"
                          icon={
                            <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                              <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
                            </svg>
                          }
                          onClick={() => setPortForwardRules(portForwardRules.filter((_, i) => i !== idx))}
                          _hover={{ bg: deleteBtnBg.replace('0.1', '0.2') }}
                        />

                        <FormControl mb={2}>
                          <FormLabel fontSize="xs" color={mutedText}>协议类型</FormLabel>
                          <ChakraSelect
                            value={rule.protocol}
                            onChange={(e) => {
                              const updated = [...portForwardRules];
                              updated[idx].protocol = e.target.value;
                              setPortForwardRules(updated);
                            }}
                            size="sm"
                            bg={selectBg}
                            borderColor={selectBorder}
                            color={textColor}
                            borderRadius="lg"
                            _focus={{ borderColor: sectionTitleColor }}
                          >
                            <option value="tcp">TCP</option>
                            <option value="udp">UDP</option>
                          </ChakraSelect>
                        </FormControl>

                        <FormControl mb={2}>
                          <FormLabel fontSize="xs" color={mutedText}>本地地址</FormLabel>
                          <Input
                            size="xs"
                            value={rule.bind_addr}
                            onChange={(e) => {
                              const updated = [...portForwardRules];
                              updated[idx].bind_addr = e.target.value;
                              setPortForwardRules(updated);
                            }}
                            placeholder="例如：0.0.0.0:5678"
                            bg={inputBg}
                            borderColor={inputBorder}
                            color={textColor}
                            borderRadius="md"
                          />
                        </FormControl>

                        <Text textAlign="center" color={sectionTitleColor} fontSize="lg">↓</Text>

                        <FormControl mt={2}>
                          <FormLabel fontSize="xs" color={mutedText}>目标地址</FormLabel>
                          <Input
                            size="xs"
                            value={rule.dst_addr}
                            onChange={(e) => {
                              const updated = [...portForwardRules];
                              updated[idx].dst_addr = e.target.value;
                              setPortForwardRules(updated);
                            }}
                            placeholder="例如：10.126.126.1:5678"
                            bg={inputBg}
                            borderColor={inputBorder}
                            color={textColor}
                            borderRadius="md"
                          />
                        </FormControl>
                      </Box>
                    ))}
                  </Box>

                  {renderSwitch('proxy_forward_by_system', '系统转发')}
                  {renderTextarea('proxy_networks', '子网代理 CIDR 列表', '192.168.1.0/24')}

                  {/* 出口节点 */}
                  {renderSectionTitle('出口节点')}
                  {renderSwitch('enable_as_exit_node', '作为出口节点')}
                  {renderTextarea('exit_nodes', '出口节点列表', '10.99.0.1')}

                  {/* 性能优化 */}
                  {renderSectionTitle('性能优化')}
                  {renderSwitch('multi_thread', '启用多线程')}
                  {renderNumberInput('multi_thread_count', '线程数量', '2', 2, 16)}
                  {renderSwitch('latency_first', '延迟优先模式')}
                  {renderSwitch('use_smoltcp', '启用 smoltcp')}

                  {/* 协议优化 */}
                  {renderSectionTitle('协议优化')}
                  {renderSwitch('enable_kcp_proxy', '启用 KCP 代理')}
                  {renderSwitch('disable_kcp_input', '禁用 KCP 输入')}
                  {renderSwitch('enable_quic_proxy', '启用 QUIC 代理')}
                  {renderSwitch('disable_quic_input', '禁用 QUIC 输入')}
                  {renderNumberInput('quic_listen_port', 'QUIC 监听端口', '0（随机）', 0, 65535)}

                  {/* 加密和安全 */}
                  {renderSectionTitle('加密和安全')}
                  {renderSwitch('disable_encryption', '禁用加密')}
                  {renderInput('encryption_algorithm', '加密算法', 'aes-gcm')}

                  {/* 网络设备 */}
                  {renderSectionTitle('网络设备')}
                  {renderSwitch('bind_device', '绑定物理设备')}
                  {renderInput('dev_name', 'TUN 设备名称', 'MCTier_Net')}
                  {renderNumberInput('mtu', 'MTU 大小', '1380', 1280, 1500)}

                  {/* P2P 配置 */}
                  {renderSectionTitle('P2P 配置')}
                  {renderSwitch('p2p_only', '仅使用 P2P')}
                  {renderSwitch('disable_p2p', '禁用 P2P')}
                  {renderSwitch('disable_udp_hole_punching', '禁用 UDP 打洞')}
                  {renderSwitch('disable_tcp_hole_punching', '禁用 TCP 打洞')}
                  {renderSwitch('disable_sym_hole_punching', '禁用对称 NAT 打洞')}

                  {/* 中继配置 */}
                  {renderSectionTitle('中继配置')}
                  {renderTextarea('relay_network_whitelist', '中继网络白名单', '*（允许所有）')}
                  {renderSwitch('relay_all_peer_rpc', '转发所有对等节点 RPC')}
                  {renderSwitch('disable_relay_kcp', '禁用中继 KCP')}
                  {renderSwitch('enable_relay_foreign_network_kcp', '启用中继外部网络 KCP')}
                  {renderNumberInput('foreign_relay_bps_limit', '外部网络流量限制（BPS）', '0（无限制）', 0)}

                  {/* 路由配置 */}
                  {renderSectionTitle('路由配置')}
                  {renderTextarea('manual_routes', '手动路由 CIDR', '10.0.0.0/8')}

                  {/* 压缩 */}
                  {renderSectionTitle('压缩')}
                  {renderInput('compression', '压缩算法', 'none')}

                  {/* 监听器配置 */}
                  {renderSectionTitle('监听器配置')}
                  {renderTextarea('listeners', '监听器列表', 'tcp://0.0.0.0:11010')}
                  {renderTextarea('mapped_listeners', '映射的监听器（公网地址）', 'tcp://1.2.3.4:11010')}
                  {renderSwitch('no_listener', '不监听任何端口')}
                  {renderInput('default_protocol', '默认协议', 'tcp')}

                  {/* DNS 配置 */}
                  {renderSectionTitle('DNS 配置')}
                  {renderSwitch('accept_dns', '启用魔法 DNS')}
                  {renderInput('tld_dns_zone', '顶级域名区域', 'et.net')}

                  {/* 端口白名单 */}
                  {renderSectionTitle('端口白名单')}
                  {renderTextarea('tcp_whitelist', 'TCP 端口白名单', '80\n443\n8000-9000')}
                  {renderTextarea('udp_whitelist', 'UDP 端口白名单', '53\n123')}

                  {/* IPv6 */}
                  {renderSectionTitle('IPv6')}
                  {renderSwitch('disable_ipv6', '禁用 IPv6')}
                  {renderInput('ipv6', 'IPv6 地址', 'fe80::1/64')}

                  {/* STUN 服务器 */}
                  {renderSectionTitle('STUN 服务器')}
                  {renderTextarea('stun_servers', 'STUN 服务器列表', 'stun://stun.l.google.com:19302')}
                  {renderTextarea('stun_servers_v6', 'IPv6 STUN 服务器列表', 'stun://[2001:4860:4860::8888]:19302')}

                  {/* 私有模式 */}
                  {renderSectionTitle('私有模式')}
                  {renderSwitch('private_mode', '启用私有模式')}
                </>
              )}
            </>
          )}
        </ModalBody>

        {/* 底部按钮 */}
        <ModalFooter px={6} pb={6} pt={4} borderTop="1px solid" borderColor={footerBorder}>
          <HStack spacing={3} w="full">
            <Button
              flex={1}
              variant="outline"
              onClick={onClose}
              isDisabled={saving || loading}
              borderRadius="lg"
              fontWeight="500"
              bg={useColorModeValue('gray.100', 'rgba(255,255,255,0.05)')}
              color={titleColor}
              borderColor={modalBorder}
              _hover={{ bg: useColorModeValue('gray.200', 'rgba(255,255,255,0.1)') }}
            >
              取消
            </Button>
            <Button
              flex={1}
              bg="#ff4d4f"
              color="#fff"
              _hover={{ bg: '#ff7875' }}
              borderRadius="lg"
              fontWeight="500"
              onClick={handleReset}
              isDisabled={saving || loading}
            >
              {loading ? '重置中...' : '重置'}
            </Button>
            <Button
              flex={1}
              bg="linear-gradient(135deg, #4CAF50 0%, #45a049 100%)"
              color="#fff"
              _hover={{ opacity: 0.9 }}
              borderRadius="lg"
              fontWeight="500"
              onClick={handleSave}
              isLoading={saving}
              isDisabled={saving || loading}
            >
              保存
            </Button>
          </HStack>
        </ModalFooter>
      </ModalContent>
    </Modal>
  );
};
