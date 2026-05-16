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

const STORAGE_KEY = "nexbox_custom_html";

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
        <img src="data:image/png;base64,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" alt="新境盒" />
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
  const [tempHtml, setTempHtml] = useState("");
  const [iframeKey, setIframeKey] = useState(0);
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const { isOpen, onOpen, onClose } = useDisclosure();
  const { t } = useTranslation();

  useEffect(() => {
    setIframeKey((k) => k + 1);
  }, [html]);

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
          srcDoc={html}
          style={{
            width: "100%",
            height: "100%",
            border: "none",
            display: "block",
          }}
          sandbox="allow-scripts"
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
                    sandbox="allow-scripts"
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
