import React, { useState, useEffect } from 'react';
import { Collapse, Form, Input, Switch, InputNumber } from 'antd';
import { invoke } from '@tauri-apps/api/core';
import {
  Box, Flex, Spinner, Text, useColorModeValue, useToast,
} from '@chakra-ui/react';
import './GlobalAdvancedConfigPanel.css';

const { Panel } = Collapse;

export const GlobalAdvancedConfigPanel: React.FC = () => {
  const [form] = Form.useForm();
  const [loading, setLoading] = useState(true);
  const toast = useToast();

  // 主题色变量
  const textColor = useColorModeValue('gray.800', 'rgba(255,255,255,0.9)');
  const labelColor = useColorModeValue('gray.700', 'rgba(255,255,255,0.85)');
  const descColor = useColorModeValue('gray.400', 'rgba(255,255,255,0.38)');
  const inputBg = useColorModeValue('gray.100', 'rgba(255,255,255,0.08)');
  const cardBorder = useColorModeValue('gray.200', 'rgba(255,255,255,0.1)');
  const sectionBg = useColorModeValue('gray.100', 'rgba(255,255,255,0.05)');
  const sectionBorder = useColorModeValue('gray.200', 'rgba(255,255,255,0.1)');
  const selectBg = useColorModeValue('white', 'rgba(30, 30, 40, 0.8)');
  const selectText = useColorModeValue('gray.800', '#fff');
  const switchBg = useColorModeValue('gray.300', 'rgba(255,255,255,0.2)');
  const ruleBg = useColorModeValue('gray.50', 'rgba(255,255,255,0.03)');
  const ruleBorder = useColorModeValue('gray.200', 'rgba(255,255,255,0.1)');
  const btnPrimaryBg = 'rgba(126, 211, 33, 0.1)';
  const btnPrimaryBorder = 'rgba(126, 211, 33, 0.3)';
  const btnPrimaryColor = 'rgba(126, 211, 33, 0.9)';
  const btnDangerBg = 'rgba(255, 77, 79, 0.1)';
  const btnDangerBorder = 'rgba(255, 77, 79, 0.3)';
  const btnDangerColor = 'rgba(255, 77, 79, 0.9)';

  const showToast = (title: string, status: 'success' | 'error', duration = 2000) => {
    toast({ title, status, duration, isClosable: true, position: 'top' });
  };

  useEffect(() => {
    loadConfig();
  }, []);

  const loadConfig = async () => {
    setLoading(true);
    try {
      const config = await invoke<any>('get_global_easytier_advanced_config');
      console.log('加载的全局高级配置:', config);
      form.setFieldsValue(config);
    } catch (error) {
      console.error('加载全局高级配置失败:', error);
      showToast('加载配置失败', 'error');
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    try {
      const values = form.getFieldsValue(true);
      console.log('保存全局高级配置:', values);
      await invoke('save_global_easytier_advanced_config', { configJson: values });
      showToast('全局高级配置已保存', 'success');
    } catch (error) {
      console.error('保存全局高级配置失败:', error);
      showToast('保存配置失败', 'error');
    }
  };

  if (loading) {
    return (
      <Flex justify="center" align="center" minH="200px">
        <Spinner size="md" color="rgba(126, 211, 33, 0.8)" thickness="3px" />
      </Flex>
    );
  }

  return (
    <Box px={3.5} pb={3.5}>
      <Form form={form} layout="vertical" onValuesChange={handleSave}>
        <Collapse
          className="advanced-config-collapse"
          expandIconPosition="end"
          style={{
            background: 'transparent',
            border: 'none',
          }}
        >
          {/* 网络模式 */}
          <Panel
            header={<span style={{ color: textColor, fontWeight: 500, fontSize: '14px' }}>网络模式</span>}
            key="network"
            style={{
              background: sectionBg,
              border: `1px solid ${sectionBorder}`,
              borderRadius: '8px',
              marginBottom: '12px',
              overflow: 'hidden',
            }}
          >
            <Form.Item
              name="no_tun"
              label={<span style={{ color: labelColor, fontSize: '13px' }}>无 TUN 模式</span>}
              valuePropName="checked"
              tooltip="不创建虚拟网卡，仅使用代理模式"
            >
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item
              name="dhcp"
              label={<span style={{ color: labelColor, fontSize: '13px' }}>启用 DHCP</span>}
              valuePropName="checked"
              tooltip="自动分配虚拟 IP 地址"
            >
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item
              name="ipv4"
              label={<span style={{ color: labelColor, fontSize: '13px' }}>手动指定 IPv4</span>}
              tooltip="例如：10.144.144.1/24"
            >
              <Input
                placeholder="10.144.144.1/24"
                style={{
                  background: inputBg,
                  border: `1px solid ${cardBorder}`,
                  color: textColor,
                  borderRadius: '6px',
                }}
              />
            </Form.Item>
          </Panel>

          {/* 代理和转发 */}
          <Panel
            header={<span style={{ color: textColor, fontWeight: 500, fontSize: '14px' }}>代理和转发</span>}
            key="proxy"
            style={{
              background: sectionBg,
              border: `1px solid ${sectionBorder}`,
              borderRadius: '8px',
              marginBottom: '12px',
              overflow: 'hidden',
            }}
          >
            <Form.Item
              name="enable_socks5"
              label={<span style={{ color: labelColor, fontSize: '13px' }}>启用 SOCKS5 代理</span>}
              valuePropName="checked"
            >
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item
              name="socks5_port"
              label={<span style={{ color: labelColor, fontSize: '13px' }}>SOCKS5 端口</span>}
            >
              <InputNumber
                min={1024}
                max={65535}
                placeholder="1080"
                style={{
                  width: '100%',
                  background: inputBg,
                  border: `1px solid ${cardBorder}`,
                  color: textColor,
                  borderRadius: '6px',
                }}
              />
            </Form.Item>

            {/* 端口转发 */}
            <Box mt={4} mb={2}>
              <Flex align="center" justify="space-between" mb={2}>
                <Text fontWeight={500} fontSize="sm" color={textColor}>端口转发规则</Text>
                <button
                  type="button"
                  onClick={() => {
                    const currentRules = form.getFieldValue('port_forward_rules') || [];
                    form.setFieldsValue({
                      port_forward_rules: [
                        ...currentRules,
                        { protocol: 'tcp', bind_addr: '0.0.0.0:5678', dst_addr: '10.126.126.1:5678' }
                      ]
                    });
                  }}
                  style={{
                    padding: '4px 12px',
                    background: btnPrimaryBg,
                    border: `1px solid ${btnPrimaryBorder}`,
                    borderRadius: '4px',
                    color: btnPrimaryColor,
                    cursor: 'pointer',
                    fontSize: '12px',
                    display: 'flex',
                    alignItems: 'center',
                    gap: '4px',
                  }}
                >
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <line x1="12" y1="5" x2="12" y2="19"></line>
                    <line x1="5" y1="12" x2="19" y2="12"></line>
                  </svg>
                  添加规则
                </button>
              </Flex>
              <Text fontSize="xs" color={descColor} mb={2}>
                将本地端口转发到虚拟网络中的远程端口。例如：tcp://0.0.0.0:5678 → 10.126.126.1:5678
              </Text>
            </Box>

            <Form.List name="port_forward_rules">
              {(fields, { remove }) => (
                <>
                  {fields.map((field) => (
                    <div key={field.key} style={{
                      marginBottom: '12px',
                      padding: '12px',
                      background: ruleBg,
                      borderRadius: '6px',
                      border: `1px solid ${ruleBorder}`,
                      position: 'relative'
                    }}>
                      <button
                        type="button"
                        onClick={() => remove(field.name)}
                        style={{
                          position: 'absolute',
                          top: '8px',
                          right: '8px',
                          padding: '4px 8px',
                          background: btnDangerBg,
                          border: `1px solid ${btnDangerBorder}`,
                          borderRadius: '4px',
                          color: btnDangerColor,
                          cursor: 'pointer',
                          fontSize: '12px',
                          display: 'flex',
                          alignItems: 'center',
                          gap: '4px'
                        }}
                        title="删除规则"
                      >
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                          <line x1="18" y1="6" x2="6" y2="18"></line>
                          <line x1="6" y1="6" x2="18" y2="18"></line>
                        </svg>
                        删除
                      </button>

                      <div style={{ marginBottom: '8px' }}>
                        <label style={{ display: 'block', marginBottom: '4px', fontSize: '12px', color: labelColor }}>
                          协议类型
                        </label>
                        <Form.Item
                          {...field}
                          name={[field.name, 'protocol']}
                          style={{ marginBottom: 0 }}
                        >
                          <div style={{ position: 'relative' }}>
                            <select style={{
                              width: '100%',
                              padding: '6px 30px 6px 10px',
                              background: selectBg,
                              border: `1px solid ${btnPrimaryBorder}`,
                              borderRadius: '8px',
                              color: selectText,
                              fontSize: '13px',
                              cursor: 'pointer',
                              outline: 'none',
                              transition: 'all 0.2s',
                              appearance: 'none',
                              WebkitAppearance: 'none',
                              MozAppearance: 'none'
                            }}
                            onMouseEnter={(e) => {
                              e.currentTarget.style.borderColor = 'rgba(126, 211, 33, 0.6)';
                            }}
                            onMouseLeave={(e) => {
                              e.currentTarget.style.borderColor = btnPrimaryBorder;
                            }}>
                              <option value="tcp" style={{ background: selectBg, color: selectText, padding: '8px' }}>TCP</option>
                              <option value="udp" style={{ background: selectBg, color: selectText, padding: '8px' }}>UDP</option>
                            </select>
                            <div style={{
                              position: 'absolute',
                              right: '10px',
                              top: '50%',
                              transform: 'translateY(-50%)',
                              pointerEvents: 'none',
                              color: 'rgba(126, 211, 33, 0.6)'
                            }}>
                              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                <polyline points="6 9 12 15 18 9"></polyline>
                              </svg>
                            </div>
                          </div>
                        </Form.Item>
                      </div>

                      <div style={{ marginBottom: '8px' }}>
                        <label style={{ display: 'block', marginBottom: '4px', fontSize: '12px', color: labelColor }}>
                          本地地址
                        </label>
                        <Form.Item
                          {...field}
                          name={[field.name, 'bind_addr']}
                          style={{ marginBottom: 0 }}
                        >
                          <Input placeholder="例如：0.0.0.0:5678" size="small" style={{ background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
                        </Form.Item>
                      </div>

                      <div style={{
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        margin: '4px 0',
                        color: 'rgba(126, 211, 33, 0.6)',
                        fontSize: '16px'
                      }}>
                        ↓
                      </div>

                      <div>
                        <label style={{ display: 'block', marginBottom: '4px', fontSize: '12px', color: labelColor }}>
                          目标地址
                        </label>
                        <Form.Item
                          {...field}
                          name={[field.name, 'dst_addr']}
                          style={{ marginBottom: 0 }}
                        >
                          <Input placeholder="例如：10.126.126.1:5678" size="small" style={{ background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
                        </Form.Item>
                      </div>
                    </div>
                  ))}
                </>
              )}
            </Form.List>

            <Form.Item
              name="proxy_forward_by_system"
              label={<span style={{ color: labelColor, fontSize: '13px' }}>系统转发</span>}
              valuePropName="checked"
              tooltip="通过系统内核转发子网代理数据包"
            >
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item
              name="proxy_networks"
              label={<span style={{ color: labelColor, fontSize: '13px' }}>子网代理 CIDR 列表</span>}
              tooltip="每行一个 CIDR，例如：192.168.1.0/24"
            >
              <Input.TextArea placeholder="192.168.1.0/24" rows={3} style={{ background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
            </Form.Item>
          </Panel>

          {/* 出口节点 */}
          <Panel header={<span style={{ color: textColor, fontWeight: 500, fontSize: '14px' }}>出口节点</span>} key="exit"
            style={{ background: sectionBg, border: `1px solid ${sectionBorder}`, borderRadius: '8px', marginBottom: '12px', overflow: 'hidden' }}
          >
            <Form.Item name="enable_as_exit_node" label={<span style={{ color: labelColor, fontSize: '13px' }}>作为出口节点</span>} valuePropName="checked" tooltip="允许其他节点通过本机访问网络">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item name="exit_nodes" label={<span style={{ color: labelColor, fontSize: '13px' }}>出口节点列表</span>} tooltip="每行一个虚拟 IP，例如：10.99.0.1">
              <Input.TextArea placeholder="10.99.0.1" rows={3} style={{ background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
            </Form.Item>
          </Panel>

          {/* 性能优化 */}
          <Panel header={<span style={{ color: textColor, fontWeight: 500, fontSize: '14px' }}>性能优化</span>} key="performance"
            style={{ background: sectionBg, border: `1px solid ${sectionBorder}`, borderRadius: '8px', marginBottom: '12px', overflow: 'hidden' }}
          >
            <Form.Item name="multi_thread" label={<span style={{ color: labelColor, fontSize: '13px' }}>启用多线程</span>} valuePropName="checked">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item name="multi_thread_count" label={<span style={{ color: labelColor, fontSize: '13px' }}>线程数量</span>}>
              <InputNumber min={2} max={16} placeholder="2" style={{ width: '100%', background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
            </Form.Item>
            <Form.Item name="latency_first" label={<span style={{ color: labelColor, fontSize: '13px' }}>延迟优先模式</span>} valuePropName="checked" tooltip="优先使用低延迟路径">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item name="use_smoltcp" label={<span style={{ color: labelColor, fontSize: '13px' }}>启用 smoltcp</span>} valuePropName="checked" tooltip="使用 smoltcp 网络栈">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
          </Panel>

          {/* 协议优化 */}
          <Panel header={<span style={{ color: textColor, fontWeight: 500, fontSize: '14px' }}>协议优化</span>} key="protocol"
            style={{ background: sectionBg, border: `1px solid ${sectionBorder}`, borderRadius: '8px', marginBottom: '12px', overflow: 'hidden' }}
          >
            <Form.Item name="enable_kcp_proxy" label={<span style={{ color: labelColor, fontSize: '13px' }}>启用 KCP 代理</span>} valuePropName="checked" tooltip="使用 KCP 协议提升 UDP 性能">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item name="disable_kcp_input" label={<span style={{ color: labelColor, fontSize: '13px' }}>禁用 KCP 输入</span>} valuePropName="checked">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item name="enable_quic_proxy" label={<span style={{ color: labelColor, fontSize: '13px' }}>启用 QUIC 代理</span>} valuePropName="checked" tooltip="使用 QUIC 协议提升性能">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item name="disable_quic_input" label={<span style={{ color: labelColor, fontSize: '13px' }}>禁用 QUIC 输入</span>} valuePropName="checked">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item name="quic_listen_port" label={<span style={{ color: labelColor, fontSize: '13px' }}>QUIC 监听端口</span>}>
              <InputNumber min={0} max={65535} placeholder="0（随机）" style={{ width: '100%', background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
            </Form.Item>
          </Panel>

          {/* 加密和安全 */}
          <Panel header={<span style={{ color: textColor, fontWeight: 500, fontSize: '14px' }}>加密和安全</span>} key="security"
            style={{ background: sectionBg, border: `1px solid ${sectionBorder}`, borderRadius: '8px', marginBottom: '12px', overflow: 'hidden' }}
          >
            <Form.Item name="disable_encryption" label={<span style={{ color: labelColor, fontSize: '13px' }}>禁用加密</span>} valuePropName="checked" tooltip="警告：禁用加密会降低安全性">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item name="encryption_algorithm" label={<span style={{ color: labelColor, fontSize: '13px' }}>加密算法</span>} tooltip="支持：aes-gcm, aes-256-gcm, xor, chacha20">
              <Input placeholder="aes-gcm" style={{ background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
            </Form.Item>
          </Panel>

          {/* 网络设备 */}
          <Panel header={<span style={{ color: textColor, fontWeight: 500, fontSize: '14px' }}>网络设备</span>} key="device"
            style={{ background: sectionBg, border: `1px solid ${sectionBorder}`, borderRadius: '8px', marginBottom: '12px', overflow: 'hidden' }}
          >
            <Form.Item name="bind_device" label={<span style={{ color: labelColor, fontSize: '13px' }}>绑定物理设备</span>} valuePropName="checked" tooltip="绑定到物理网卡，避免路由问题">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item name="dev_name" label={<span style={{ color: labelColor, fontSize: '13px' }}>TUN 设备名称</span>}>
              <Input placeholder="MCTier_Net" style={{ background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
            </Form.Item>
            <Form.Item name="mtu" label={<span style={{ color: labelColor, fontSize: '13px' }}>MTU 大小</span>}>
              <InputNumber min={1280} max={1500} placeholder="1380" style={{ width: '100%', background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
            </Form.Item>
          </Panel>

          {/* P2P 配置 */}
          <Panel header={<span style={{ color: textColor, fontWeight: 500, fontSize: '14px' }}>P2P 配置</span>} key="p2p"
            style={{ background: sectionBg, border: `1px solid ${sectionBorder}`, borderRadius: '8px', marginBottom: '12px', overflow: 'hidden' }}
          >
            <Form.Item name="p2p_only" label={<span style={{ color: labelColor, fontSize: '13px' }}>仅使用 P2P</span>} valuePropName="checked" tooltip="只与已建立 P2P 连接的节点通信">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item name="disable_p2p" label={<span style={{ color: labelColor, fontSize: '13px' }}>禁用 P2P</span>} valuePropName="checked" tooltip="只通过中继节点转发数据">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item name="disable_udp_hole_punching" label={<span style={{ color: labelColor, fontSize: '13px' }}>禁用 UDP 打洞</span>} valuePropName="checked">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item name="disable_tcp_hole_punching" label={<span style={{ color: labelColor, fontSize: '13px' }}>禁用 TCP 打洞</span>} valuePropName="checked">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item name="disable_sym_hole_punching" label={<span style={{ color: labelColor, fontSize: '13px' }}>禁用对称 NAT 打洞</span>} valuePropName="checked">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
          </Panel>

          {/* 中继配置 */}
          <Panel header={<span style={{ color: textColor, fontWeight: 500, fontSize: '14px' }}>中继配置</span>} key="relay"
            style={{ background: sectionBg, border: `1px solid ${sectionBorder}`, borderRadius: '8px', marginBottom: '12px', overflow: 'hidden' }}
          >
            <Form.Item name="relay_network_whitelist" label={<span style={{ color: labelColor, fontSize: '13px' }}>中继网络白名单</span>} tooltip="每行一个网络名称，支持通配符">
              <Input.TextArea placeholder="*（允许所有）" rows={3} style={{ background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
            </Form.Item>
            <Form.Item name="relay_all_peer_rpc" label={<span style={{ color: labelColor, fontSize: '13px' }}>转发所有对等节点 RPC</span>} valuePropName="checked">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item name="disable_relay_kcp" label={<span style={{ color: labelColor, fontSize: '13px' }}>禁用中继 KCP</span>} valuePropName="checked">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item name="enable_relay_foreign_network_kcp" label={<span style={{ color: labelColor, fontSize: '13px' }}>启用中继外部网络 KCP</span>} valuePropName="checked">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item name="foreign_relay_bps_limit" label={<span style={{ color: labelColor, fontSize: '13px' }}>外部网络流量限制（BPS）</span>}>
              <InputNumber min={0} placeholder="0（无限制）" style={{ width: '100%', background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
            </Form.Item>
          </Panel>

          {/* 路由配置 */}
          <Panel header={<span style={{ color: textColor, fontWeight: 500, fontSize: '14px' }}>路由配置</span>} key="route"
            style={{ background: sectionBg, border: `1px solid ${sectionBorder}`, borderRadius: '8px', marginBottom: '12px', overflow: 'hidden' }}
          >
            <Form.Item name="manual_routes" label={<span style={{ color: labelColor, fontSize: '13px' }}>手动路由 CIDR</span>} tooltip="每行一个 CIDR">
              <Input.TextArea placeholder="10.0.0.0/8" rows={3} style={{ background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
            </Form.Item>
          </Panel>

          {/* 压缩 */}
          <Panel header={<span style={{ color: textColor, fontWeight: 500, fontSize: '14px' }}>压缩</span>} key="compression"
            style={{ background: sectionBg, border: `1px solid ${sectionBorder}`, borderRadius: '8px', marginBottom: '12px', overflow: 'hidden' }}
          >
            <Form.Item name="compression" label={<span style={{ color: labelColor, fontSize: '13px' }}>压缩算法</span>} tooltip="支持：none, zstd">
              <Input placeholder="none" style={{ background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
            </Form.Item>
          </Panel>

          {/* 监听器配置 */}
          <Panel header={<span style={{ color: textColor, fontWeight: 500, fontSize: '14px' }}>监听器配置</span>} key="listener"
            style={{ background: sectionBg, border: `1px solid ${sectionBorder}`, borderRadius: '8px', marginBottom: '12px', overflow: 'hidden' }}
          >
            <Form.Item name="listeners" label={<span style={{ color: labelColor, fontSize: '13px' }}>监听器列表</span>} tooltip="每行一个监听地址，例如：tcp://0.0.0.0:11010">
              <Input.TextArea placeholder="tcp://0.0.0.0:11010" rows={3} style={{ background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
            </Form.Item>
            <Form.Item name="mapped_listeners" label={<span style={{ color: labelColor, fontSize: '13px' }}>映射的监听器（公网地址）</span>} tooltip="每行一个公网地址">
              <Input.TextArea placeholder="tcp://1.2.3.4:11010" rows={3} style={{ background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
            </Form.Item>
            <Form.Item name="no_listener" label={<span style={{ color: labelColor, fontSize: '13px' }}>不监听任何端口</span>} valuePropName="checked">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item name="default_protocol" label={<span style={{ color: labelColor, fontSize: '13px' }}>默认协议</span>} tooltip="tcp, udp, wg, ws, wss">
              <Input placeholder="tcp" style={{ background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
            </Form.Item>
          </Panel>

          {/* DNS 配置 */}
          <Panel header={<span style={{ color: textColor, fontWeight: 500, fontSize: '14px' }}>DNS 配置</span>} key="dns"
            style={{ background: sectionBg, border: `1px solid ${sectionBorder}`, borderRadius: '8px', marginBottom: '12px', overflow: 'hidden' }}
          >
            <Form.Item name="accept_dns" label={<span style={{ color: labelColor, fontSize: '13px' }}>启用魔法 DNS</span>} valuePropName="checked" tooltip="使用域名访问其他节点">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item name="tld_dns_zone" label={<span style={{ color: labelColor, fontSize: '13px' }}>顶级域名区域</span>}>
              <Input placeholder="et.net" style={{ background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
            </Form.Item>
          </Panel>

          {/* 端口白名单 */}
          <Panel header={<span style={{ color: textColor, fontWeight: 500, fontSize: '14px' }}>端口白名单</span>} key="whitelist"
            style={{ background: sectionBg, border: `1px solid ${sectionBorder}`, borderRadius: '8px', marginBottom: '12px', overflow: 'hidden' }}
          >
            <Form.Item name="tcp_whitelist" label={<span style={{ color: labelColor, fontSize: '13px' }}>TCP 端口白名单</span>} tooltip="每行一个端口或端口范围，例如：80 或 8000-9000">
              <Input.TextArea placeholder="80&#10;443&#10;8000-9000" rows={3} style={{ background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
            </Form.Item>
            <Form.Item name="udp_whitelist" label={<span style={{ color: labelColor, fontSize: '13px' }}>UDP 端口白名单</span>} tooltip="每行一个端口或端口范围">
              <Input.TextArea placeholder="53&#10;123" rows={3} style={{ background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
            </Form.Item>
          </Panel>

          {/* IPv6 */}
          <Panel header={<span style={{ color: textColor, fontWeight: 500, fontSize: '14px' }}>IPv6</span>} key="ipv6"
            style={{ background: sectionBg, border: `1px solid ${sectionBorder}`, borderRadius: '8px', marginBottom: '12px', overflow: 'hidden' }}
          >
            <Form.Item name="disable_ipv6" label={<span style={{ color: labelColor, fontSize: '13px' }}>禁用 IPv6</span>} valuePropName="checked">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
            <Form.Item name="ipv6" label={<span style={{ color: labelColor, fontSize: '13px' }}>IPv6 地址</span>}>
              <Input placeholder="fe80::1/64" style={{ background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
            </Form.Item>
          </Panel>

          {/* STUN 服务器 */}
          <Panel header={<span style={{ color: textColor, fontWeight: 500, fontSize: '14px' }}>STUN 服务器</span>} key="stun"
            style={{ background: sectionBg, border: `1px solid ${sectionBorder}`, borderRadius: '8px', marginBottom: '12px', overflow: 'hidden' }}
          >
            <Form.Item name="stun_servers" label={<span style={{ color: labelColor, fontSize: '13px' }}>STUN 服务器列表</span>} tooltip="每行一个 STUN 服务器地址">
              <Input.TextArea placeholder="stun://stun.l.google.com:19302" rows={3} style={{ background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
            </Form.Item>
            <Form.Item name="stun_servers_v6" label={<span style={{ color: labelColor, fontSize: '13px' }}>IPv6 STUN 服务器列表</span>} tooltip="每行一个 IPv6 STUN 服务器地址">
              <Input.TextArea placeholder="stun://[2001:4860:4860::8888]:19302" rows={3} style={{ background: inputBg, border: `1px solid ${cardBorder}`, color: textColor, borderRadius: '6px' }} />
            </Form.Item>
          </Panel>

          {/* 私有模式 */}
          <Panel header={<span style={{ color: textColor, fontWeight: 500, fontSize: '14px' }}>私有模式</span>} key="private"
            style={{ background: sectionBg, border: `1px solid ${sectionBorder}`, borderRadius: '8px', marginBottom: '12px', overflow: 'hidden' }}
          >
            <Form.Item name="private_mode" label={<span style={{ color: labelColor, fontSize: '13px' }}>启用私有模式</span>} valuePropName="checked" tooltip="不允许其他网络的节点通过本节点中转">
              <Switch style={{ background: switchBg }} />
            </Form.Item>
          </Panel>
        </Collapse>
      </Form>
    </Box>
  );
};