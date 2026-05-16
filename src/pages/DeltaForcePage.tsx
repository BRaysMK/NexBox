import {
  Box,
  Text,
  Heading,
  VStack,
  HStack,
  useColorModeValue,
  Badge,
  Spinner,
  Image,
  useToast,
  SimpleGrid,
} from "@chakra-ui/react";
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { Check, ExternalLink, ChevronRight, MapPin, Crosshair, ArrowLeft, Music } from "lucide-react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useBackground } from "@/contexts/background-context";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { motion, AnimatePresence } from "framer-motion";
import deltaForceLogo from "@/assets/deltaforce.png";
import { MusicPlayer } from "@/components/MusicPlayer";

interface DeltaPasswordItem {
  name: string;
  password: string;
}

const getWeaponPlatforms = (t: (key: string) => string) => [
  { id: "atguns", name: "ATGUNS", url: "https://guns.anxu.cc/", color: "#FF6B35" },
  { id: "maqt", name: t("deltaForce.weaponCodePlatformNames.maqt"), url: "https://maqt.top/", color: "#4ECDC4" },
  { id: "anchor-codes", name: t("deltaForce.weaponCodePlatformNames.anchor-codes"), url: "https://g.aitags.cn/live", color: "#FF6B35" },
  { id: "xiaotao-check", name: t("deltaForce.weaponCodePlatformNames.xiaotao-check"), url: "https://orzice.com/v/gun_fw", color: "#4ECDC4" },
  { id: "delta-workshop", name: t("deltaForce.weaponCodePlatformNames.delta-workshop"), url: "https://gamefun66.com/gunChangePlan", color: "#9B59B6" },
];

function SectionCard({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  const { liquidGlassEnabled } = useBackground();
  const cardBg = useColorModeValue("white", "#111111");
  const borderColor = useColorModeValue("gray.200", "#333333");

  if (liquidGlassEnabled) {
    return (
      <LiquidGlassCard p={5}>
        <VStack align="stretch" spacing={4}>
          <Text fontWeight="semibold" fontSize="md" color="white">{title}</Text>
          {children}
        </VStack>
      </LiquidGlassCard>
    );
  }

  return (
    <Box bg={cardBg} borderRadius="xl" p={5} border="1px solid" borderColor={borderColor}>
      <VStack align="stretch" spacing={4}>
        <Text fontWeight="semibold" fontSize="md" color="white">{title}</Text>
        {children}
      </VStack>
    </Box>
  );
}

function PasswordCard() {
  const { t } = useTranslation();
  const toast = useToast();
  const [passwords, setPasswords] = useState<DeltaPasswordItem[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);

  const subTextColor = useColorModeValue("gray.500", "#888888");
  const cardItemBg = useColorModeValue("gray.50", "#1a1a1a");
  const cardItemHoverBg = useColorModeValue("gray.100", "#222222");

  useEffect(() => {
    loadPasswords();
    const interval = setInterval(loadPasswords, 60000);
    return () => clearInterval(interval);
  }, []);

  const loadPasswords = async () => {
    try {
      const data = await invoke<DeltaPasswordItem[]>("get_delta_passwords");
      setPasswords(data);
    } catch (error) {
      console.error("Failed to load passwords:", error);
    } finally {
      setIsLoading(false);
    }
  };

  const copyToClipboard = async (text: string, index: number) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedIndex(index);
      toast({
        title: t("deltaForce.copySuccess"),
        status: "success",
        duration: 2000,
        isClosable: true,
      });
      setTimeout(() => setCopiedIndex(null), 2000);
    } catch (error) {
      toast({
        title: t("deltaForce.copyFailed"),
        status: "error",
        duration: 2000,
        isClosable: true,
      });
    }
  };

  return (
    <SectionCard title={t("deltaForce.dailyPassword")}>
      {isLoading ? (
        <HStack justify="center" py={4}>
          <Spinner size="sm" color="teal.400" />
          <Text color={subTextColor} fontSize="sm">{t("deltaForce.loading")}</Text>
        </HStack>
      ) : passwords.length === 0 ? (
        <Text color={subTextColor} fontSize="sm">{t("deltaForce.noPassword")}</Text>
      ) : (
        <HStack spacing={3} align="stretch" wrap="wrap">
          {passwords.map((item, index) => (
            <Box
              key={index}
              flex="1"
              minW="140px"
              p={4}
              borderRadius="xl"
              bg={cardItemBg}
              cursor="pointer"
              onClick={() => copyToClipboard(item.password, index)}
              _hover={{ bg: cardItemHoverBg }}
              transition="background-color 0.2s"
            >
              <VStack spacing={2} align="center">
                <MapPin size={18} color="#98DDD0" />
                <Text color={subTextColor} fontSize="sm" fontWeight="medium">{item.name}</Text>
                <Text color="teal.400" fontWeight="bold" fontSize="xl" letterSpacing="wider">
                  {item.password}
                </Text>
                {copiedIndex === index && (
                  <Badge colorScheme="green" fontSize="xs">
                    <Check size={10} style={{ display: "inline" }} /> {t("deltaForce.copied")}
                  </Badge>
                )}
              </VStack>
            </Box>
          ))}
        </HStack>
      )}
    </SectionCard>
  );
}

function QuickEntryCards({ onEnterWeaponCodes }: { onEnterWeaponCodes: () => void }) {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const cardBg = useColorModeValue("gray.50", "#1a1a1a");
  const cardHoverBg = useColorModeValue("gray.100", "#222222");

  const openMapWebview = async () => {
    try {
      const label = "delta-official-map";
      const existing = WebviewWindow.getByLabel(label);
      if (existing) {
        const win = await existing;
        if (win) {
          await win.setFocus();
          return;
        }
      }
      const webview = new WebviewWindow(label, {
        url: "https://df.qq.com/cp/a20240729directory/",
        title: t("deltaForce.officialMapToolTitle"),
        width: 1280,
        height: 800,
        center: true,
        resizable: true,
      });
      webview.once("tauri://error", (e) => {
        console.error("Webview error:", e);
      });
    } catch (error) {
      console.error("Failed to open map webview:", error);
    }
  };

  const openWallpaperWebview = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-shell");
      await open("https://df.qq.com/cp/community1031/wallpaper.html#/");
    } catch (error) {
      console.error("Failed to open wallpaper page:", error);
    }
  };

  const EntryCard = ({ children, onClick }: { children: React.ReactNode; onClick: () => void }) => {
    if (liquidGlassEnabled) {
      return (
        <LiquidGlassCard
          flex="1"
          p={6}
          cursor="pointer"
          onClick={onClick}
        >
          {children}
        </LiquidGlassCard>
      );
    }
    
    return (
      <Box
        flex="1"
        p={6}
        borderRadius="xl"
        bg={cardBg}
        cursor="pointer"
        onClick={onClick}
        _hover={{ bg: cardHoverBg }}
        transition="background-color 0.2s"
      >
        {children}
      </Box>
    );
  };

  return (
    <SimpleGrid columns={{ base: 1, md: 3 }} spacing={4}>
      <EntryCard onClick={onEnterWeaponCodes}>
        <HStack justify="space-between" align="center">
          <HStack spacing={4}>
            <Box p={3} borderRadius="xl" bg="orange.500">
              <Crosshair size={24} color="white" />
            </Box>
            <VStack align="start" spacing={0}>
              <Text color={textColor} fontWeight="bold" fontSize="lg">
                {t("deltaForce.weaponCodes")}
              </Text>
              <Text color={subTextColor} fontSize="xs">
                {t("deltaForce.weaponCodeDesc")}
              </Text>
            </VStack>
          </HStack>
          <ChevronRight size={22} color="#98DDD0" />
        </HStack>
      </EntryCard>

      <EntryCard onClick={openMapWebview}>
        <HStack justify="space-between" align="center">
          <HStack spacing={4}>
            <Image src={deltaForceLogo} alt="Delta Force" w="40px" h="40px" objectFit="contain" borderRadius="lg" />
            <VStack align="start" spacing={0}>
              <Text color={textColor} fontWeight="bold" fontSize="lg">
                {t("deltaForce.officialMaps")}
              </Text>
              <Text color={subTextColor} fontSize="xs">
                {t("deltaForce.mapToolDesc")}
              </Text>
            </VStack>
          </HStack>
          <ChevronRight size={22} color="#98DDD0" />
        </HStack>
      </EntryCard>

      <EntryCard onClick={openWallpaperWebview}>
        <HStack justify="space-between" align="center">
          <HStack spacing={4}>
            <Image src={deltaForceLogo} alt="Delta Force" w="40px" h="40px" objectFit="contain" borderRadius="lg" />
            <VStack align="start" spacing={0}>
              <Text color={textColor} fontWeight="bold" fontSize="lg">
                {t("deltaForce.officialWallpaper")}
              </Text>
              <Text color={subTextColor} fontSize="xs">
                {t("deltaForce.wallpaperDesc")}
              </Text>
            </VStack>
          </HStack>
          <ChevronRight size={22} color="#98DDD0" />
        </HStack>
      </EntryCard>
    </SimpleGrid>
  );
}

function WeaponCodeDetail({ onBack }: { onBack: () => void }) {
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");
  const cardBg = useColorModeValue("gray.50", "#1a1a1a");
  const cardHoverBg = useColorModeValue("gray.100", "#222222");
  const weaponPlatforms = getWeaponPlatforms(t);

  const openWebview = async (platform: ReturnType<typeof getWeaponPlatforms>[0]) => {
    try {
      const label = `delta-weapon-${platform.id}`;
      const existing = WebviewWindow.getByLabel(label);
      if (existing) {
        const win = await existing;
        if (win) {
          await win.setFocus();
          return;
        }
      }
      const webview = new WebviewWindow(label, {
        url: platform.url,
        title: platform.name,
        width: 1280,
        height: 800,
        center: true,
        resizable: true,
      });
      webview.once("tauri://error", (e) => {
        console.error("Webview error:", e);
      });
    } catch (error) {
      console.error("Failed to open webview:", error);
    }
  };

  const PlatformCard = ({ children, onClick }: { children: React.ReactNode; onClick: () => void }) => {
    if (liquidGlassEnabled) {
      return (
        <LiquidGlassCard
          p={5}
          cursor="pointer"
          onClick={onClick}
        >
          {children}
        </LiquidGlassCard>
      );
    }
    
    return (
      <Box
        p={5}
        borderRadius="xl"
        bg={cardBg}
        cursor="pointer"
        onClick={onClick}
        _hover={{ bg: cardHoverBg }}
        transition="background-color 0.2s"
      >
        {children}
      </Box>
    );
  };

  return (
    <VStack align="stretch" spacing={5}>
      <HStack spacing={3} cursor="pointer" onClick={onBack} _hover={{ opacity: 0.7 }}>
        <ArrowLeft size={18} color="#98DDD0" />
        <Text color="teal.400" fontWeight="medium" fontSize="sm">{t("deltaForce.back")}</Text>
      </HStack>

      <SectionCard title={t("deltaForce.weaponCodePlatforms")}>
        <VStack spacing={3} align="stretch">
          {weaponPlatforms.map(platform => (
            <PlatformCard key={platform.id} onClick={() => openWebview(platform)}>
              <HStack justify="space-between">
                <HStack spacing={3}>
                  <Box p={2} borderRadius="lg" bg={platform.color}>
                    <Crosshair size={18} color="white" />
                  </Box>
                  <Text color={textColor} fontWeight="bold" fontSize="md">{platform.name}</Text>
                </HStack>
                <HStack spacing={2}>
                  <Text color="teal.400" fontSize="sm">{t("deltaForce.openPlatform")}</Text>
                  <ExternalLink size={16} color="#98DDD0" />
                </HStack>
              </HStack>
            </PlatformCard>
          ))}
        </VStack>
      </SectionCard>
    </VStack>
  );
}

export default function DeltaForcePage() {
  const { t } = useTranslation();
  const headingColor = useColorModeValue("gray.900", "#ffffff");
  const [showWeaponDetail, setShowWeaponDetail] = useState(false);

  const pageVariants = {
    initial: { opacity: 0, x: 20 },
    in: { opacity: 1, x: 0 },
    out: { opacity: 0, x: -20 },
  };

  const pageTransition = {
    type: "tween",
    ease: "easeOut",
    duration: 0.3,
  };

  return (
    <Box pt={8} pb={8}>
      <AnimatePresence mode="wait">
        {showWeaponDetail ? (
          <motion.div
            key="detail"
            initial="initial"
            animate="in"
            exit="out"
            variants={pageVariants}
            transition={pageTransition}
          >
            <Heading size="lg" color={headingColor} mb={6}>
              {t("deltaForce.weaponCodes")}
            </Heading>
            <WeaponCodeDetail onBack={() => setShowWeaponDetail(false)} />
          </motion.div>
        ) : (
          <motion.div
            key="main"
            initial="initial"
            animate="in"
            exit="out"
            variants={pageVariants}
            transition={pageTransition}
          >
            <Heading size="lg" color={headingColor} mb={6}>
              {t("deltaForce.title")}
            </Heading>

            <VStack align="stretch" spacing={5}>
              <PasswordCard />
              <QuickEntryCards onEnterWeaponCodes={() => setShowWeaponDetail(true)} />
            </VStack>
          </motion.div>
        )}
      </AnimatePresence>

      {/* <Box mt={6}>
        <HStack spacing={2} mb={3}>
          <Music size={18} color="#98DDD0" />
          <Text color={headingColor} fontWeight="semibold" fontSize="md">
            {t("musicPlayer.title")}
          </Text>
        </HStack>
        <MusicPlayer />
      </Box> */}
    </Box>
  );
}
