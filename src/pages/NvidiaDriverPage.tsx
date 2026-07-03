import { useState, useEffect } from "react";
import {
  Box,
  Text,
  Heading,
  VStack,
  HStack,
  useColorModeValue,
  Button,
  Select,
  Switch,
  Slider,
  SliderTrack,
  SliderFilledTrack,
  SliderThumb,
  Spinner,
  Badge,
  useToast,
  Alert,
  AlertIcon,
  AlertTitle,
  AlertDescription,
  Flex,
} from "@chakra-ui/react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { ArrowLeft, Cpu, Monitor, Settings2, AlertTriangle, RefreshCw } from "lucide-react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import { hexToRgba } from "@/lib/color-utils";

interface SettingOption {
  value: number;
  label: string;
}

interface NvApiStatus {
  available: boolean;
  error: string | null;
}

interface CandidateDllInfo {
  path: string;
  exists: boolean;
  size: number;
  loaded: boolean;
  has_initialize: boolean;
  export_count: number;
  company: string | null;
  version: string | null;
}

interface NvApiDiagnostic {
  nvapi64_exists: boolean;
  nvapi64_size: number;
  nvapi32_exists: boolean;
  nvapi32_size: number;
  dll_loaded: boolean;
  loaded_module_path: string | null;
  has_initialize: boolean;
  has_unload: boolean;
  has_drs_create: boolean;
  last_error: number;
  conclusion: string;
  suggestion: string;
  file_company: string | null;
  file_product: string | null;
  file_version: string | null;
  exports: string[];
  export_count: number;
  candidate_dlls: CandidateDllInfo[];
}

interface NvidiaSetting {
  id: number;
  name: string;
  description: string;
  current_value: number;
  default_value: number;
  options: SettingOption[];
}

const SETTING_IDS: Record<string, number> = {
  VSYNCMODE: 0x00a879cf,
  QUALITY_ENHANCEMENTS: 0x00ce2691,
  ANISO_MODE_LEVEL: 0x101e61a9,
  AA_MODE_METHOD: 0x10d773d2,
  FRL_FPS: 0x10835002,
  PREFERRED_PSTATE: 0x1057eb71,
  FXAA_ENABLE: 0x1074c972,
};

export default function NvidiaDriverPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const toast = useToast();
  const { liquidGlassEnabled } = useBackground();
  const { getActiveColor, getContrastTextColor } = useThemeColor();

  const [isLoading, setIsLoading] = useState(true);
  const [isNvidiaAvailable, setIsNvidiaAvailable] = useState<boolean | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [diagnostic, setDiagnostic] = useState<NvApiDiagnostic | null>(null);
  const [driverVersion, setDriverVersion] = useState<string>("");
  const [driverBranch, setDriverBranch] = useState<string>("");
  const [settings, setSettings] = useState<NvidiaSetting[]>([]);
  const [modifiedSettings, setModifiedSettings] = useState<Record<number, number>>({});
  const [isSaving, setIsSaving] = useState(false);

  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const textColor = useColorModeValue("gray.700", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const cardBg = useColorModeValue("white", "#1a1a1a");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const sectionBg = useColorModeValue("gray.50", "#161616");
  const nvidiaGreen = "#76B900";

  useEffect(() => {
    loadData();
  }, []);

  async function loadData() {
    setIsLoading(true);
    setLoadError(null);
    try {
      // 1. 确保硬件缓存已填充，然后检测是否有 NVIDIA 显卡
      try { await invoke("get_hardware"); } catch (_) {}
      const available = await invoke<boolean>("is_nvidia_gpu");
      setIsNvidiaAvailable(available);
      if (!available) {
        setIsLoading(false);
        return;
      }

      // 2. 检查 NVAPI 是否真正可用（硬件检测到 NVIDIA ≠ NVAPI 库可用）
      const nvapiStatus = await invoke<NvApiStatus>("get_nvapi_status");
      if (!nvapiStatus.available) {
        const reason = nvapiStatus.error || "未知原因";
        // 运行诊断以获取详细信息
        try {
          const diag = await invoke<NvApiDiagnostic>("diagnose_nvapi");
          setDiagnostic(diag);
          setLoadError(diag.conclusion || reason);
        } catch (_) {
          setLoadError(reason);
        }
        setIsLoading(false);
        return;
      }

      // 3. 获取驱动版本
      try {
        const [ver, branch] = await invoke<[number, string]>("get_nvidia_driver_version");
        setDriverVersion(ver.toString());
        setDriverBranch(branch);
      } catch (_) {
        // 老版本 NVAPI 可能不支持
      }

      // 4. 读取所有设置
      const allSettings = await invoke<NvidiaSetting[]>("list_nvidia_settings");
      setSettings(allSettings);
    } catch (e) {
      console.error("加载 NVIDIA 设置失败:", e);
      setLoadError(`加载 NVIDIA 设置时出错: ${String(e)}`);
    } finally {
      setIsLoading(false);
    }
  }

  function getModifiedValue(settingId: number): number {
    if (settingId in modifiedSettings) {
      return modifiedSettings[settingId];
    }
    const setting = settings.find((s) => s.id === settingId);
    return setting?.current_value ?? 0;
  }

  function handleChange(settingId: number, value: number) {
    setModifiedSettings((prev) => ({ ...prev, [settingId]: value }));
  }

  function hasChanges(): boolean {
    if (Object.keys(modifiedSettings).length === 0) return false;
    for (const [id, value] of Object.entries(modifiedSettings)) {
      const original = settings.find((s) => s.id === Number(id));
      if (!original || original.current_value !== value) return true;
    }
    return false;
  }

  async function handleApply() {
    setIsSaving(true);
    try {
      let changed = 0;
      for (const [idStr, value] of Object.entries(modifiedSettings)) {
        const id = Number(idStr);
        const original = settings.find((s) => s.id === id);
        if (original && original.current_value !== value) {
          await invoke("set_nvidia_setting", { settingId: id, value });
          changed++;
        }
      }

      if (changed > 0) {
        toast({
          title: "设置已应用",
          description: `已修改 ${changed} 个 NVIDIA 驱动设置`,
          status: "success",
          duration: 2000,
          isClosable: true,
        });
        // 重新加载
        setModifiedSettings({});
        await loadData();
      } else {
        toast({
          title: "未检测到更改",
          status: "info",
          duration: 2000,
          isClosable: true,
        });
      }
    } catch (e) {
      toast({
        title: "应用失败",
        description: String(e),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    } finally {
      setIsSaving(false);
    }
  }

  async function handleReset() {
    setIsSaving(true);
    try {
      await invoke("reset_nvidia_settings");
      toast({
        title: "已恢复默认",
        description: "所有 NVIDIA 图形设置已恢复为默认值",
        status: "success",
        duration: 2000,
        isClosable: true,
      });
      setModifiedSettings({});
      await loadData();
    } catch (e) {
      toast({
        title: "恢复失败",
        description: String(e),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    } finally {
      setIsSaving(false);
    }
  }

  // === 渲染单个设置项 ===
  function renderSettingControl(setting: NvidiaSetting) {
    const currentVal = getModifiedValue(setting.id);

    // 帧率限制 → 特殊处理：用 Slider
    if (setting.id === SETTING_IDS.FRL_FPS) {
      return (
        <Box>
          <HStack justify="space-between" mb={2}>
            <Text fontSize="sm" color={subTextColor}>
              {currentVal === 0 ? "关闭" : `${currentVal} FPS`}
            </Text>
            <Badge colorScheme={currentVal > 0 ? "green" : "gray"}>
              {currentVal > 0 ? "已启用" : "已关闭"}
            </Badge>
          </HStack>
          <Slider
            min={0}
            max={240}
            step={1}
            value={currentVal}
            onChange={(v) => handleChange(setting.id, v)}
          >
            <SliderTrack bg={useColorModeValue("gray.200", "gray.600")}>
              <SliderFilledTrack bg={nvidiaGreen} />
            </SliderTrack>
            <SliderThumb boxSize={5} bg={nvidiaGreen} />
          </Slider>
          <HStack justify="space-between" mt={1}>
            <Text fontSize="xs" color={subTextColor}>关闭</Text>
            <Text fontSize="xs" color={subTextColor}>240 FPS</Text>
          </HStack>
        </Box>
      );
    }

    // 开关（FXAA）
    if (setting.options.length === 2 &&
        setting.options[0].value === 0 &&
        setting.options[1].value === 1) {
      return (
        <HStack justify="space-between">
          <Text fontSize="sm" color={subTextColor}>
            {setting.options.find((o) => o.value === currentVal)?.label ?? String(currentVal)}
          </Text>
          <Switch
            isChecked={currentVal === 1}
            onChange={(e) => handleChange(setting.id, e.target.checked ? 1 : 0)}
            colorScheme="green"
          />
        </HStack>
      );
    }

    // Select 下拉菜单（其他设置）
    return (
      <Select
        value={currentVal}
        onChange={(e) => handleChange(setting.id, Number(e.target.value))}
        bg={useColorModeValue("white", "#1f1f1f")}
        borderColor={borderColor}
        color={textColor}
        _hover={{ borderColor: nvidiaGreen }}
        focusBorderColor={nvidiaGreen}
        borderRadius="lg"
        size="sm"
      >
        {setting.options.map((opt) => (
          <option key={opt.value} value={opt.value}
            style={{ background: useColorModeValue("#fff", "#1f1f1f"), color: useColorModeValue("#000", "#fff") }}>
            {opt.label}
          </option>
        ))}
      </Select>
    );
  }

  // === 渲染设置分组 ===
  function renderSettingGroup(title: string, icon: React.ReactNode, settingIds: number[]) {
    const groupSettings = settings.filter((s) => settingIds.includes(s.id));
    if (groupSettings.length === 0) return null;

    return (
      <LiquidGlassCard
        p={5}
        bg={cardBg}
        borderColor={borderColor}
        borderRadius="xl"
        w="full"
      >
        <HStack mb={4} spacing={2}>
          {icon}
          <Text fontWeight="semibold" color={headingColor} fontSize="md">
            {title}
          </Text>
        </HStack>
        <VStack spacing={4} align="stretch">
          {groupSettings.map((setting) => (
            <Box key={setting.id}>
              <HStack justify="space-between" mb={1}>
                <Text fontWeight="medium" fontSize="sm" color={textColor}>
                  {setting.name}
                </Text>
                <Text fontSize="xs" color={subTextColor}>
                  {setting.description}
                </Text>
              </HStack>
              {renderSettingControl(setting)}
            </Box>
          ))}
        </VStack>
      </LiquidGlassCard>
    );
  }

  // === 加载中 ===
  if (isLoading) {
    return (
      <Box pt={8}>
        <Flex justify="center" align="center" minH="200px">
          <Spinner size="xl" color={nvidiaGreen} />
        </Flex>
      </Box>
    );
  }

  // === 无 NVIDIA 显卡 ===
  if (isNvidiaAvailable === false) {
    return (
      <Box pt={8}>
        <VStack spacing={6} align="start">
          <HStack>
            <Box as="button" onClick={() => navigate(-1)} p={1} borderRadius="md"
              _hover={{ bg: useColorModeValue("gray.100", "gray.700") }}>
              <ArrowLeft size={20} />
            </Box>
            <Heading size="lg" color={headingColor}>
              显卡设置（仅 NVIDIA）
            </Heading>
          </HStack>

          <Alert status="warning" borderRadius="xl" bg={useColorModeValue("orange.50", "rgba(255, 152, 0, 0.1)")}>
            <AlertIcon />
            <Box>
              <AlertTitle>未检测到 NVIDIA 显卡</AlertTitle>
              <AlertDescription>
                此功能仅适用于搭载 NVIDIA 独立显卡的设备。
                请确认已正确安装 NVIDIA 显卡驱动，且当前使用的是 NVIDIA GPU。
              </AlertDescription>
            </Box>
          </Alert>
        </VStack>
      </Box>
    );
  }

  // === NVAPI 不可用（检测到 NVIDIA 显卡但 NVAPI 库无法加载） ===
  if (loadError) {
    return (
      <Box pt={8}>
        <VStack spacing={6} align="start">
          <HStack>
            <Box as="button" onClick={() => navigate(-1)} p={1} borderRadius="md"
              _hover={{ bg: useColorModeValue("gray.100", "gray.700") }}>
              <ArrowLeft size={20} />
            </Box>
            <Heading size="lg" color={headingColor}>
              显卡设置（仅 NVIDIA）
            </Heading>
          </HStack>

          <LiquidGlassCard
            p={6}
            bg={cardBg}
            borderColor={borderColor}
            borderRadius="xl"
            w="full"
          >
            <VStack spacing={4} align="start">
              <HStack spacing={3}>
                <Box color={useColorModeValue("orange.500", "orange.300")}>
                  <AlertTriangle size={32} />
                </Box>
                <VStack align="start" spacing={0}>
                  <Text fontWeight="bold" color={headingColor} fontSize="md">
                    NVAPI 加载失败
                  </Text>
                  <Text fontSize="xs" color={subTextColor}>
                    已检测到 NVIDIA 显卡，但无法加载 NVAPI 接口
                  </Text>
                </VStack>
              </HStack>

              {/* 诊断结论 */}
              <Alert status="error" borderRadius="lg" bg={useColorModeValue("red.50", "rgba(239, 68, 68, 0.1)")}>
                <AlertIcon />
                <Box>
                  <AlertTitle fontSize="sm">诊断结果</AlertTitle>
                  <AlertDescription fontSize="xs" whiteSpace="pre-wrap">
                    {loadError}
                  </AlertDescription>
                </Box>
              </Alert>

              {/* 诊断详细信息 */}
              {diagnostic && (
                <Box
                  w="full"
                  p={4}
                  borderRadius="lg"
                  bg={useColorModeValue("gray.50", "#111111")}
                  borderWidth="1px"
                  borderColor={borderColor}
                >
                  <Text fontWeight="semibold" fontSize="xs" color={subTextColor} mb={2}>
                    系统诊断详情
                  </Text>
                  <VStack spacing={1} align="start" w="full">
                    <HStack justify="space-between" w="full">
                      <Text fontSize="xs" color={subTextColor}>nvapi64.dll 存在</Text>
                      <Badge colorScheme={diagnostic.nvapi64_exists ? "green" : "red"} fontSize="2xs">
                        {diagnostic.nvapi64_exists ? "是" : "否"}
                      </Badge>
                    </HStack>
                    {diagnostic.nvapi64_exists && (
                      <HStack justify="space-between" w="full">
                        <Text fontSize="xs" color={subTextColor}>nvapi64.dll 大小</Text>
                        <Text fontSize="xs" color={textColor} fontFamily="mono">
                          {(diagnostic.nvapi64_size / 1024).toFixed(1)} KB
                        </Text>
                      </HStack>
                    )}
                    {diagnostic.file_company && (
                      <HStack justify="space-between" w="full">
                        <Text fontSize="xs" color={subTextColor}>文件公司</Text>
                        <Text fontSize="xs" color={textColor} fontFamily="mono">
                          {diagnostic.file_company}
                        </Text>
                      </HStack>
                    )}
                    {diagnostic.file_product && (
                      <HStack justify="space-between" w="full">
                        <Text fontSize="xs" color={subTextColor}>产品名称</Text>
                        <Text fontSize="xs" color={textColor} fontFamily="mono">
                          {diagnostic.file_product}
                        </Text>
                      </HStack>
                    )}
                    {diagnostic.file_version && (
                      <HStack justify="space-between" w="full">
                        <Text fontSize="xs" color={subTextColor}>文件版本</Text>
                        <Text fontSize="xs" color={textColor} fontFamily="mono">
                          {diagnostic.file_version}
                        </Text>
                      </HStack>
                    )}
                    <HStack justify="space-between" w="full">
                      <Text fontSize="xs" color={subTextColor}>DLL 加载成功</Text>
                      <Badge colorScheme={diagnostic.dll_loaded ? "green" : "red"} fontSize="2xs">
                        {diagnostic.dll_loaded ? "是" : "否"}
                      </Badge>
                    </HStack>
                    {diagnostic.loaded_module_path && (
                      <HStack justify="space-between" w="full">
                        <Text fontSize="xs" color={subTextColor}>加载路径</Text>
                        <Text fontSize="xs" color={textColor} fontFamily="mono" wordBreak="break-all" maxW="60%">
                          {diagnostic.loaded_module_path}
                        </Text>
                      </HStack>
                    )}
                    <HStack justify="space-between" w="full">
                      <Text fontSize="xs" color={subTextColor}>NvAPI_Initialize 导出</Text>
                      <Badge colorScheme={diagnostic.has_initialize ? "green" : "red"} fontSize="2xs">
                        {diagnostic.has_initialize ? "存在" : "缺失"}
                      </Badge>
                    </HStack>
                    <HStack justify="space-between" w="full">
                      <Text fontSize="xs" color={subTextColor}>导出函数总数</Text>
                      <Text fontSize="xs" color={textColor} fontFamily="mono">
                        {diagnostic.export_count}
                      </Text>
                    </HStack>
                    {diagnostic.last_error > 0 && (
                      <HStack justify="space-between" w="full">
                        <Text fontSize="xs" color={subTextColor}>Windows 错误码</Text>
                        <Text fontSize="xs" color={textColor} fontFamily="mono">
                          {diagnostic.last_error}
                        </Text>
                      </HStack>
                    )}
                  </VStack>

                  {/* 导出函数列表 */}
                  {diagnostic.exports.length > 0 && (
                    <Box mt={3} w="full">
                      <Text fontWeight="semibold" fontSize="xs" color={subTextColor} mb={1}>
                        DLL 导出函数（前 {diagnostic.exports.length} 个{diagnostic.export_count > 50 ? `，共 ${diagnostic.export_count} 个` : ""}）
                      </Text>
                      <Box
                        maxH="120px"
                        overflowY="auto"
                        p={2}
                        borderRadius="md"
                        bg={useColorModeValue("white", "#0a0a0a")}
                        borderWidth="1px"
                        borderColor={borderColor}
                      >
                        <VStack spacing={0} align="start" fontSize="2xs" fontFamily="mono">
                          {diagnostic.exports.map((exp, i) => (
                            <Text
                              key={i}
                              color={exp.includes("NvAPI_Initialize") ? "green.500" : subTextColor}
                              fontWeight={exp.includes("NvAPI_Initialize") ? "bold" : "normal"}
                            >
                              {exp}
                            </Text>
                          ))}
                        </VStack>
                      </Box>
                    </Box>
                  )}
                  {diagnostic.export_count === 0 && diagnostic.nvapi64_exists && (
                    <Text fontSize="xs" color="red.500" mt={2}>
                      ⚠ 该 DLL 没有导出任何函数，可能不是有效的动态链接库
                    </Text>
                  )}

                  {/* 候选 DLL 列表 */}
                  {diagnostic.candidate_dlls && diagnostic.candidate_dlls.length > 0 && (
                    <Box mt={3} w="full">
                      <Text fontWeight="semibold" fontSize="xs" color={subTextColor} mb={2}>
                        系统中找到的 nvapi64.dll 候选文件（{diagnostic.candidate_dlls.length} 个）
                      </Text>
                      <VStack spacing={2} align="start" w="full">
                        {diagnostic.candidate_dlls.map((dll, i) => (
                          <Box
                            key={i}
                            w="full"
                            p={2}
                            borderRadius="md"
                            bg={useColorModeValue("white", "#0a0a0a")}
                            borderWidth="1px"
                            borderColor={dll.has_initialize ? "green.300" : borderColor}
                          >
                            <HStack justify="space-between" mb={1}>
                              <Text fontSize="2xs" color={textColor} fontFamily="mono" wordBreak="break-all" maxW="70%">
                                {dll.path}
                              </Text>
                              {dll.has_initialize ? (
                                <Badge colorScheme="green" fontSize="2xs">✓ 可用</Badge>
                              ) : (
                                <Badge colorScheme="red" fontSize="2xs">✗ 不可用</Badge>
                              )}
                            </HStack>
                            <HStack spacing={3} fontSize="2xs" color={subTextColor}>
                              <Text>{(dll.size / 1024).toFixed(1)} KB</Text>
                              <Text>导出: {dll.export_count}</Text>
                              {dll.company && <Text>来源: {dll.company}</Text>}
                              {dll.version && <Text>版本: {dll.version}</Text>}
                            </HStack>
                          </Box>
                        ))}
                      </VStack>
                    </Box>
                  )}
                </Box>
              )}

              {/* 建议操作 */}
              <VStack spacing={2} align="start" w="full" mt={2}>
                <Text fontWeight="semibold" fontSize="sm" color={textColor}>
                  建议操作：
                </Text>
                <Text fontSize="xs" color={subTextColor}>
                  {diagnostic?.suggestion || "请从 NVIDIA 官网下载并安装最新的显卡驱动程序。"}
                </Text>
                <VStack spacing={1} align="start" pl={4} mt={1}>
                  <Text fontSize="xs" color={subTextColor}>
                    1. 前往 NVIDIA 官网（nvidia.com/drivers）下载对应显卡型号的最新驱动
                  </Text>
                  <Text fontSize="xs" color={subTextColor}>
                    2. 安装时选择「自定义安装」并勾选「执行清洁安装」
                  </Text>
                  <Text fontSize="xs" color={subTextColor}>
                    3. 如果是笔记本，请确认是否使用了 OEM 定制驱动（如联想/戴尔/华硕版），建议改用 NVIDIA 官方驱动
                  </Text>
                  <Text fontSize="xs" color={subTextColor}>
                    4. 安装完成后重启电脑，再返回此页面重试
                  </Text>
                </VStack>
              </VStack>

              <HStack spacing={3} w="full" pt={2}>
                <Button
                  leftIcon={<RefreshCw size={16} />}
                  colorScheme="green"
                  bg={nvidiaGreen}
                  variant="solid"
                  size="md"
                  borderRadius="lg"
                  onClick={() => loadData()}
                  _hover={{ bg: "#6aa800" }}
                >
                  重新检测
                </Button>
                <Button
                  variant="outline"
                  borderColor={borderColor}
                  size="md"
                  borderRadius="lg"
                  color={textColor}
                  onClick={() => navigate(-1)}
                >
                  返回
                </Button>
              </HStack>
            </VStack>
          </LiquidGlassCard>
        </VStack>
      </Box>
    );
  }

  // === 主页面 ===
  const hasAnyChanges = hasChanges();

  return (
    <Box pt={8}>
      <VStack spacing={6} align="start">
        {/* 标题栏 */}
        <HStack w="full" justify="space-between">
          <HStack>
            <Box as="button" onClick={() => navigate(-1)} p={1} borderRadius="md"
              _hover={{ bg: useColorModeValue("gray.100", "gray.700") }}>
              <ArrowLeft size={20} />
            </Box>
            <Heading size="lg" color={headingColor}>
              显卡设置（仅 NVIDIA）
            </Heading>
            <Badge colorScheme="green" variant="solid" fontSize="xs" px={2}>
              BETA
            </Badge>
          </HStack>
        </HStack>

        {/* 驱动信息卡片 */}
        <LiquidGlassCard
          p={4}
          bg={cardBg}
          borderColor={borderColor}
          borderRadius="xl"
          w="full"
          borderLeft="4px solid"
          borderLeftColor={nvidiaGreen}
        >
          <HStack spacing={4}>
            <Box color={nvidiaGreen}>
              <Monitor size={28} />
            </Box>
            <Box>
              <Text fontWeight="bold" color={textColor} fontSize="sm">
                NVIDIA GeForce 显卡
              </Text>
              <Text fontSize="xs" color={subTextColor}>
                驱动版本: {driverVersion || "未知"} {driverBranch ? `(${driverBranch})` : ""}
              </Text>
            </Box>
            <Box ml="auto">
              <Badge colorScheme="green" variant="subtle" px={2}>
                NVAPI 已就绪
              </Badge>
            </Box>
          </HStack>
        </LiquidGlassCard>

        {/* 同步与显示 */}
        {renderSettingGroup(
          "同步与显示",
          <Monitor size={18} color={nvidiaGreen} />,
          [SETTING_IDS.VSYNCMODE, SETTING_IDS.FRL_FPS]
        )}

        {/* 画质与纹理 */}
        {renderSettingGroup(
          "画质与纹理",
          <Settings2 size={18} color={nvidiaGreen} />,
          [SETTING_IDS.QUALITY_ENHANCEMENTS, SETTING_IDS.ANISO_MODE_LEVEL, SETTING_IDS.AA_MODE_METHOD, SETTING_IDS.FXAA_ENABLE]
        )}

        {/* 电源与性能 */}
        {renderSettingGroup(
          "电源与性能",
          <Cpu size={18} color={nvidiaGreen} />,
          [SETTING_IDS.PREFERRED_PSTATE]
        )}

        {/* 操作按钮 */}
        <HStack w="full" spacing={4} pt={2} pb={8}>
          <Button
            colorScheme="green"
            bg={nvidiaGreen}
            isDisabled={!hasAnyChanges}
            isLoading={isSaving}
            loadingText="应用设置..."
            onClick={handleApply}
            flex={1}
            size="md"
            borderRadius="lg"
            _hover={{ bg: "#6aa800" }}
          >
            应用更改
          </Button>
          <Button
            variant="outline"
            borderColor={borderColor}
            isDisabled={isSaving}
            onClick={handleReset}
            flex={1}
            size="md"
            borderRadius="lg"
            color={textColor}
          >
            恢复默认
          </Button>
        </HStack>

        {/* 提示 */}
        <Box w="full" pb={4}>
          <Text fontSize="xs" color={subTextColor} textAlign="center">
            修改将在下次启动游戏或应用程序时生效。一些设置可能需要重启才能完全应用。
          </Text>
        </Box>
      </VStack>
    </Box>
  );
}
