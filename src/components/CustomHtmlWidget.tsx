import {
  Box,
  Text,
  Flex,
  IconButton,
  Textarea,
  Tabs,
  TabList,
  Tab,
  TabPanels,
  TabPanel,
  useDisclosure,
  Modal,
  ModalOverlay,
  ModalContent,
  ModalHeader,
  ModalBody,
  ModalCloseButton,
  Button,
  HStack,
  useColorModeValue,
} from "@chakra-ui/react";
import { useState, useRef, useEffect } from "react";
import { Code, RotateCcw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";

const STORAGE_KEY = "nexbox_custom_html";

/** 根据扩展名推断 MIME 类型 */
function getMimeFromPath(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase() || "png";
  const mimeMap: Record<string, string> = {
    png: "image/png",
    jpg: "image/jpeg",
    jpeg: "image/jpeg",
    gif: "image/gif",
    webp: "image/webp",
    svg: "image/svg+xml",
    bmp: "image/bmp",
    ico: "image/x-icon",
  };
  return mimeMap[ext] || "image/png";
}

/** 字节数组转 base64 data URL */
function bytesToDataUrl(bytes: number[], mimeType: string): string {
  const uint8 = new Uint8Array(bytes);
  let binary = "";
  const chunkSize = 8192;
  for (let i = 0; i < uint8.length; i += chunkSize) {
    const chunk = uint8.subarray(i, i + chunkSize);
    binary += String.fromCharCode(...chunk);
  }
  const base64 = btoa(binary);
  return `data:${mimeType};base64,${base64}`;
}

/**
 * 读取本地文件并转为 base64 data URL
 */
async function pathToDataUrl(path: string): Promise<string> {
  try {
    const bytes = await invoke<number[]>("read_file_bytes", { path });
    return bytesToDataUrl(bytes, getMimeFromPath(path));
  } catch (e) {
    console.error("读取文件失败:", path, e);
    return path; // 读取失败时保留原路径
  }
}

/**
 * 将 HTML 中的本地文件路径转换为 base64 data URL
 * 支持 Windows 绝对路径 (C:\...、D:\...) 和 Unix 绝对路径 (/Users/...)
 */
async function convertLocalPathsToDataUrls(html: string): Promise<string> {
  const imgRegex =
    /(<img\s+[^>]*src\s*=\s*["'])([A-Za-z]:\\[^"']+|(?:\/[^\/\s"']+)+\.(?:png|jpe?g|gif|webp|svg|bmp|ico))(["'])/gi;

  const matches: { index: number; prefix: string; path: string; suffix: string }[] = [];
  let match;
  while ((match = imgRegex.exec(html)) !== null) {
    matches.push({ index: match.index, prefix: match[1], path: match[2], suffix: match[3] });
  }

  if (matches.length === 0) return html;

  // 并行读取所有文件并转换
  const dataUrls = await Promise.all(
    matches.map((m) => pathToDataUrl(m.path))
  );

  // 从后往前替换，避免索引偏移
  let result = html;
  for (let i = matches.length - 1; i >= 0; i--) {
    const { prefix, suffix } = matches[i];
    const dataUrl = dataUrls[i];
    const original = prefix + matches[i].path + suffix;
    // 只有成功转换（dataUrl 以 data: 开头）才替换
    if (dataUrl.startsWith("data:")) {
      result = result.slice(0, matches[i].index) + prefix + dataUrl + suffix + result.slice(matches[i].index + original.length);
    }
  }

  return result;
}

const DEFAULT_HTML = `<!DOCTYPE html>
<html>
<head>
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    
    body {
      width: 600px;
      height: 360px;
      background: #0a0a0a;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", sans-serif;
      overflow: hidden;
      position: relative;
    }
    
    .grid-bg {
      position: absolute;
      inset: 0;
      background-image: 
        linear-gradient(rgba(0, 217, 166, 0.03) 1px, transparent 1px),
        linear-gradient(90deg, rgba(0, 217, 166, 0.03) 1px, transparent 1px);
      background-size: 40px 40px;
      animation: gridMove 20s linear infinite;
    }
    
    @keyframes gridMove {
      0% { transform: translate(0, 0); }
      100% { transform: translate(40px, 40px); }
    }
    
    .glow-orb {
      position: absolute;
      width: 300px;
      height: 300px;
      border-radius: 50%;
      filter: blur(80px);
      opacity: 0.4;
      animation: orbFloat 8s ease-in-out infinite;
    }
    
    .orb-1 {
      background: radial-gradient(circle, #00d9a6 0%, transparent 70%);
      top: -100px;
      right: -50px;
      animation-delay: 0s;
    }
    
    .orb-2 {
      background: radial-gradient(circle, #0066ff 0%, transparent 70%);
      bottom: -100px;
      left: -50px;
      animation-delay: -4s;
    }
    
    @keyframes orbFloat {
      0%, 100% { transform: translate(0, 0) scale(1); }
      50% { transform: translate(20px, 20px) scale(1.1); }
    }
    
    .content {
      position: relative;
      z-index: 10;
      height: 100%;
      display: flex;
      flex-direction: column;
      justify-content: center;
      padding: 40px 50px;
    }
    
    .logo-container {
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 24px;
      opacity: 0;
      animation: fadeSlideIn 0.8s ease-out 0.2s forwards;
    }
    
    .logo-icon {
      width: 42px;
      height: 42px;
      border-radius: 10px;
      overflow: hidden;
      box-shadow: 0 4px 20px rgba(0, 217, 166, 0.3);
    }
    
    .logo-icon img {
      width: 100%;
      height: 100%;
      object-fit: cover;
    }
    
    .logo-text {
      font-size: 28px;
      font-weight: 700;
      color: #ffffff;
      letter-spacing: -0.5px;
    }
    
    .tagline {
      font-size: 15px;
      color: #888;
      font-weight: 300;
      margin-bottom: 32px;
      opacity: 0;
      animation: fadeSlideIn 0.8s ease-out 0.4s forwards;
    }
    
    .features {
      display: flex;
      gap: 24px;
    }
    
    .feature {
      display: flex;
      align-items: center;
      gap: 10px;
      opacity: 0;
      animation: fadeSlideIn 0.8s ease-out forwards;
    }
    
    .feature:nth-child(1) { animation-delay: 0.6s; }
    .feature:nth-child(2) { animation-delay: 0.75s; }
    .feature:nth-child(3) { animation-delay: 0.9s; }
    
    .feature-dot {
      width: 8px;
      height: 8px;
      background: #00d9a6;
      border-radius: 50%;
      box-shadow: 0 0 10px rgba(0, 217, 166, 0.5);
    }
    
    .feature-text {
      font-size: 13px;
      color: #aaa;
      font-weight: 500;
    }
    
    @keyframes fadeSlideIn {
      from {
        opacity: 0;
        transform: translateY(15px);
      }
      to {
        opacity: 1;
        transform: translateY(0);
      }
    }
    
    .corner-accent {
      position: absolute;
      width: 60px;
      height: 60px;
      border: 1px solid rgba(0, 217, 166, 0.2);
    }
    
    .corner-tl {
      top: 20px;
      left: 20px;
      border-right: none;
      border-bottom: none;
    }
    
    .corner-br {
      bottom: 20px;
      right: 20px;
      border-left: none;
      border-top: none;
    }
    
    .scan-line {
      position: absolute;
      top: 0;
      left: 0;
      right: 0;
      height: 2px;
      background: linear-gradient(90deg, transparent, #00d9a6, transparent);
      opacity: 0.3;
      animation: scan 4s linear infinite;
    }
    
    @keyframes scan {
      0% { top: 0; }
      100% { top: 100%; }
    }
  </style>
</head>
<body>
  <div class="grid-bg"></div>
  <div class="glow-orb orb-1"></div>
  <div class="glow-orb orb-2"></div>
  <div class="scan-line"></div>
  <div class="corner-accent corner-tl"></div>
  <div class="corner-accent corner-br"></div>
  
  <div class="content">
    <div class="logo-container">
      <div class="logo-icon">
        <img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACwAAAAsCAYAAAAehFoBAAAJcElEQVR42s1ZaYwUxxV+1TN7hhtlNwiBEAYSIGB7F3HE4pI45QX+xBjEKYER5jA3ASQk4AdeICAgRhBj8QMQYJM/Nr8WELcQBmGMWa7FIFAWgjeAOHdhdmcq73vd1fT0dM/OrqwkLdWop7q76quv3vveq6oo2VeES5zL77ks4zJcKVXIxaL/xaV1IqH1L3xXxqWUy02DUXnAzuTyNy5R+j+5lAJ2quPbuVx2ACvAaoDlZ9u5gNE6510UUuZjU5SneOuCnqmQ4v/O24/pmHEpm0gQWMIFjF9QjhmU802UkSfIBi0N6KBRm1kL+O9/5v9ON45ogwlE/hEM/5VLkbIrIuRjtDHTyLbvlFSwKmQwafpSDlgw3QR/HvBPG51MUqNYAchIxKK6urjtyWxxiUSC7VCnnbEM+oB5KA2slmXFuVHLMfC0w9Rp/kejUQZaJ/dZWVkCPhaLpTxLZ1719c1tJqKOoyV9lcmozXPLAqtRqq2tlf/r1q2j+/fvU011Nb33/vs0e/ZsAZuVFaV4PCGMm/Z1GhPRARWQWUunAVNfXZRBsGAK2F69etHZs2dp6dKl1KRJE8rJzaVZs2bR5cuXadCgQfxOnYAF+5QhGUGXlTI/aZzD/ciy7GlmEDk5ObRx40a6cOEC9evXT57H43EBh6tnz550/Phx2rlzJzVv3lwGZ7FtRyIRtyPVgBFYYcPSIU4FhgAG0zyqpIRu3LhBCxcuFJA1NTVOoNKuQ6AO706fPp0qblbQ+PHj5V0Um22V0pdKQ5qVjlKv3IARAAZDhYWFtH//fvru0CHq0KEDxd7EHJD2u7nMenZ2dtLI4YAFhQW0b98+OuR8Z+wes5WOLB0IOMTwHe90p3nGJ5/QTWZq3Lhx0qF4v7I7zcvPo1OnTtHuPXvom4MH6ciRI1JnAOHd2lgtlWBmrtszg4GiXnQ7TD1UPYCVLwjAZouLi+no0aP09y+/ZFtsJozhGToEm8+ePZPBDBw4kO7evUsPHjygYcOG0ZQpU+jx48c247any7c5ubbtw1E/+OBP0gc5wUarcIpVhIUiDqkJ0GE0AlZz2eMrKiqoXbt29LrmNTuNPU4z7QcOHKB58+ZRVVWV1Bn9hUO+efOGWrVqRZs2bRLwxjzEOTnAYAYwoE7vvENPedCMh+KOwwY5+1tZ06kOBrBNmzalkg8/pO7du0mnuXm5AgoFTI4aNUocCWDz8vIETLNmzahly5YCFoN98uQJTZ06VRjHwM33ALtjxw5q3749DWTpwzc2eeGShUinRYJ8am2mGyCqfqmiyz9dprFjx1JBQQHt3r2bTpw4QYsWLRR9BajXr1/Ld8OHDxcQYPfTT2fSt99+J/VoB4qBdjds2ECjR4+myZMn05UrP9E3Xx+kIUOGsFP+lk3rOSHsJnRqJAPDLmC/SbwFnEvl5VepY8eOUr982TLaunUrVTsSZsJu69atafPmzTRx4sQkcv7Bzjf3s8/o4cOH8i7n5hLxcE2YMIG+2vmVzBqiY7du3ej58+du38aPyBEgMQmvYQdnTay90SzX9j4vLaVSDr/GhgF2/vz5dPv2bQGL/6ZARf780Ud069YtWrFihYAA2K5du9KxY8do7969AlaSI6WCnV/Xo8PBYdmuNdErPz/f/Y+O2rZtK1HMWycsOe8jVLdo0UKkccmSJWwGV2jw4MFCAOwc7+K5KIWPNJ2JrIVrs3JDr9dsAALTefLkSWEdQSZiRSib7fjcue/FqbZs2cL352j9+vXyTJRC20qCaw9r99OnT4MTLx0CWDd4rajdcH39+nVJcmbMmEEvXrwQk5g2bRrnF31FRSorK6lPnz4SFWO1MRlUdk423bt3T5Rm5syZSSSEBY5oYN7ZQNCwVQDAPZIc2GfMkbTy8nKWxO5u/uHV723btrHSLBKz6Ny5swSbV69eJYFOG5p1A1YW3lEqx0zQCUDCAfv07UsV7GwAC0AmtQSr165dk9mYM2eOPFuzZo0M0gxENTRb0xmw6gLXntkRFbBZ7N27t5utQc5MPlH6eakMAvaOdPTixYu0cuVKBpvDM1AbuMBVYSZBDWQYAKNsCnXMrp+V6lfVdpbHDgbvxwyMGDFC8hGYD1YmMAfTTgJt+PZtgqKwpXxTGyhqDqMm6YYTLViwwNZbk9eq5FxLOS1DEjFABASARXKEHBpgE6zJcEKxWRt5cET2ypr2jUilsGm5yxqTZkJXkVcg03rv3XfF6YRtiWQ6UArx7decJCGkd+rUSWxXSIhG3IipLKvhKw6//SJHePTokR0UdMKZvoToKGzw0o8/0tq1a90QnbJmU3YwQWAZ+/HHMvVYWmVnZUt7RtOfPXvKfdXUa45WOsfCihidISrt2rVLwABYLTtH1Fkp4/ny5cvF84cOHeoukyxfqDXyhxEgPcU92oIyID3t378/sx4Te/fPUkYrDjvMamkA6SGCADIqpIcmOpnWYIfQ0cOHD0smJ0upgH0I+EA8wfLH7ULeKv9ZSWPGjBGfQHJk8m8K2Q6znVipVWZnJpxprJIj9PPPt2n79u0CGIwAgASNKJb7cVn+FBUXUTXvSbx8+ZJGjhwpU25Yg8mAUfzf9sUXDHY0lV+96kqe2bMIW2bK6keHvKCSmE5I3ouGAWAZp5jFRUV0/vx5YQrbU9KZpZJCtrmwsrDYeTFQRD4Mds7cuZKiYoMFA0mErDKUT4dDI11Q8DCLRdjyD5cuSW6wePFicUCTaqZ8wwPNys6SnletWkU9evSgM2fOSBtgGkSEbVtpT2lUtuZ1HpgIwGMhify2rKwsKe003p//m3w6ffq0AF29erUrYcZhM4muSme4L5F2/9PZoQRTd+7ckSg2adIkqWvKOg2txoXsbcCAAXSVbdVsEgbNRMZ9e5dIpJM9M92mtZcZEwHBapcuXahNmzbyH6th2KysxZygU98uaJqjA7OmU7xQ1VZKQyHbryogMmrn/SgvpcxujrnAKhh1Eybfd2GpbchWWUI2tLm0cfCpRk2TpxPLG14R5eKJX+VcyenmX5ZztETOSVLohpzKYHtfOWFYO8ULNmyDT6WRU88Vd6StTA5l+KecnEMZ5SREGdlaI05agtr1RzXfc/dQRjuHMo+5/FvZR0vmACSUSJVBCd6je5vCqszNIO45KJoNhiNOxQXnHGwEmFaNtOVf9VDRicQOgbPNwaLyHd3+gctfuAzjt39nH91q+m/jZ0XhnSr9kG8P+49u/wOkMr2toDNV6QAAAABJRU5ErkJggg==" alt="新境盒" />
      </div>
      <span class="logo-text">新境盒</span>
    </div>
    
    <p class="tagline">简洁高效的系统工具箱 · 让优化变得简单</p>
    
    <div class="features">
      <div class="feature">
        <div class="feature-dot"></div>
        <span class="feature-text">系统优化</span>
      </div>
      <div class="feature">
        <div class="feature-dot"></div>
        <span class="feature-text">硬件检测</span>
      </div>
      <div class="feature">
        <div class="feature-dot"></div>
        <span class="feature-text">工具集成</span>
      </div>
    </div>
  </div>
</body>
</html>`;

function loadSavedHtml(): string {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved && saved.trim().length > 0) {
      return saved;
    }
  } catch (e) {
    console.error("Failed to load saved HTML:", e);
  }
  return DEFAULT_HTML;
}

export default function CustomHtmlWidget() {
  const [html, setHtml] = useState<string>(() => loadSavedHtml());
  const [processedHtml, setProcessedHtml] = useState<string>("");
  const [tempHtml, setTempHtml] = useState("");
  const [iframeKey, setIframeKey] = useState(0);
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const { isOpen, onOpen, onClose } = useDisclosure();
  const { t } = useTranslation();

  // 异步将本地路径转为 base64
  useEffect(() => {
    let cancelled = false;
    convertLocalPathsToDataUrls(html).then((result) => {
      if (!cancelled) {
        setProcessedHtml(result);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [html]);

  useEffect(() => {
    setIframeKey((k) => k + 1);
  }, [processedHtml]);

  const cardBg = useColorModeValue("gray.50", "#111111");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const headerColor = useColorModeValue("gray.800", "#ffffff");
  const textareaBg = useColorModeValue("white", "#0a0a0a");
  const textareaColor = useColorModeValue("gray.800", "#e0e0e0");

  const handleSave = () => {
    const contentToSave = tempHtml.trim() || DEFAULT_HTML;
    setHtml(contentToSave);
    try {
      localStorage.setItem(STORAGE_KEY, contentToSave);
    } catch (e) {
      console.error("Failed to save HTML:", e);
    }
    onClose();
  };

  const handleOpenEditor = () => {
    setTempHtml(html);
    onOpen();
  };

  const handleReset = () => {
    setHtml(DEFAULT_HTML);
    try {
      localStorage.setItem(STORAGE_KEY, DEFAULT_HTML);
    } catch (e) {
      console.error("Failed to reset HTML:", e);
    }
  };

  return (
    <Box w="600px">
      <Flex justify="space-between" align="center" mb={2} px={1} userSelect="none">
        <Text fontSize="sm" fontWeight="semibold" color={headerColor}>
          {t("home.customHtml")}
        </Text>
        <HStack spacing={1}>
          <IconButton
            aria-label={t("home.edit")}
            icon={<Code size={14} />}
            size="xs"
            variant="ghost"
            onClick={handleOpenEditor}
          />
          <IconButton
            aria-label={t("home.reset")}
            icon={<RotateCcw size={14} />}
            size="xs"
            variant="ghost"
            onClick={handleReset}
          />
        </HStack>
      </Flex>

      <Box
        bg={cardBg}
        borderRadius="lg"
        border="1px solid"
        borderColor={borderColor}
        overflow="hidden"
        h="360px"
        w="100%"
      >
        <iframe
          key={iframeKey}
          ref={iframeRef}
          srcDoc={processedHtml}
          style={{
            width: "100%",
            height: "100%",
            border: "none",
            display: "block",
          }}
          title="Custom HTML Preview"
        />
      </Box>

      <Modal isOpen={isOpen} onClose={onClose} size="4xl">
        <ModalOverlay />
        <ModalContent maxW="90vw" h="80vh">
          <ModalHeader>{t("home.editHtml")}</ModalHeader>
          <ModalCloseButton />
          <ModalBody p={0} flex={1} overflow="hidden">
            <Tabs h="100%" display="flex" flexDirection="column">
              <TabList px={4} pt={2}>
                <Tab>{t("home.code")}</Tab>
                <Tab>{t("home.preview")}</Tab>
              </TabList>

              <TabPanels flex={1} overflow="hidden">
                <TabPanel p={4} h="100%" overflow="hidden">
                  <Textarea
                    ref={textareaRef}
                    value={tempHtml}
                    onChange={(e) => setTempHtml(e.target.value)}
                    placeholder={t("home.htmlPlaceholder")}
                    h="calc(100% - 60px)"
                    bg={textareaBg}
                    color={textareaColor}
                    fontFamily="monospace"
                    fontSize="sm"
                    resize="none"
                  />
                  <HStack justify="flex-end" mt={3}>
                    <Button size="sm" variant="ghost" onClick={() => setTempHtml(DEFAULT_HTML)}>
                      {t("home.resetToDefault")}
                    </Button>
                    <Button size="sm" colorScheme="teal" onClick={handleSave}>
                      {t("home.save")}
                    </Button>
                  </HStack>
                </TabPanel>

                <TabPanel p={4} h="100%" overflow="auto">
                  <iframe
                    srcDoc={tempHtml}
                    style={{
                      width: "100%",
                      height: "400px",
                      border: "none",
                      borderRadius: "8px",
                    }}
                    title="Preview"
                  />
                </TabPanel>
              </TabPanels>
            </Tabs>
          </ModalBody>
        </ModalContent>
      </Modal>
    </Box>
  );
}
