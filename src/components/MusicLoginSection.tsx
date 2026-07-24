import {
  HStack,
  VStack,
  Button,
  Text,
  IconButton,
  Tooltip,
  Avatar,
  Box,
  useColorModeValue,
} from "@chakra-ui/react";
import {
  ExternalLink,
  LogOut,
  Crown,
} from "lucide-react";
import { useShallow } from "zustand/react/shallow";
import { useMusicStore, coverProxyUrl } from "@/stores/music-store";
import { useThemeColor } from "@/contexts/theme-color-context";

export function MusicLoginSection() {
  const { loginInfo, proxyPort, logout, openLoginWindow } = useMusicStore(
    useShallow((s) => ({
      loginInfo: s.loginInfo,
      proxyPort: s.proxyPort,
      logout: s.logout,
      openLoginWindow: s.openLoginWindow,
    }))
  );
  const { getActiveColor, getContrastTextColor } = useThemeColor();

  const activeColor = getActiveColor();
  const contrastText = getContrastTextColor();

  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");

  // 已登录状态
  if (loginInfo?.logged_in) {
    return (
      <HStack spacing={3} justify="space-between">
        <HStack spacing={3}>
          <Avatar size="sm" src={coverProxyUrl(loginInfo.avatar, proxyPort)} />
          <VStack spacing={0} align="start">
            <Text color={textColor} fontSize="sm" fontWeight="medium">
              {loginInfo.nickname}
            </Text>
            {loginInfo.is_svip ? (
              <Box
                as="span"
                display="inline-flex"
                alignItems="center"
                gap={1}
                px={1.5}
                py={0.5}
                borderRadius="sm"
                fontSize="10px"
                fontWeight="bold"
                letterSpacing="0.5px"
                position="relative"
                overflow="hidden"
                sx={{
                  background: "linear-gradient(135deg, #f6d365 0%, #fda085 50%, #f6d365 100%)",
                  backgroundSize: "200% 100%",
                  animation: "svipGoldShine 2.5s ease-in-out infinite",
                  color: "#5a3000",
                  textShadow: "0 1px 2px rgba(255,215,0,0.3)",
                  boxShadow: "0 1px 4px rgba(218,165,32,0.4), inset 0 1px 2px rgba(255,255,255,0.3)",
                  "@keyframes svipGoldShine": {
                    "0%": { backgroundPosition: "0% 50%" },
                    "50%": { backgroundPosition: "100% 50%" },
                    "100%": { backgroundPosition: "0% 50%" },
                  },
                  "&::after": {
                    content: '""',
                    position: "absolute",
                    top: 0,
                    left: "-100%",
                    width: "60%",
                    height: "100%",
                    background: "linear-gradient(90deg, transparent, rgba(255,255,255,0.25), transparent)",
                    animation: "svipSweep 2.5s ease-in-out infinite",
                  },
                  "@keyframes svipSweep": {
                    "0%": { left: "-100%" },
                    "50%": { left: "200%" },
                    "100%": { left: "200%" },
                  },
                }}
              >
                <Crown size={10} strokeWidth={2.5} style={{ filter: "drop-shadow(0 1px 1px rgba(218,165,32,0.5))" }} />
                SVIP
              </Box>
            ) : loginInfo.is_vip ? (
              <Text color={subTextColor} fontSize="xs">
                VIP
              </Text>
            ) : (
              <Text color={subTextColor} fontSize="xs">
                普通用户
              </Text>
            )}
          </VStack>
        </HStack>
        <Tooltip label="退出登录">
          <IconButton
            aria-label="Logout"
            icon={<LogOut size={16} />}
            size="sm"
            variant="ghost"
            onClick={() => logout()}
          />
        </Tooltip>
      </HStack>
    );
  }

  // 未登录状态：右上角一个明显按钮
  return (
    <Button
      size="sm"
      leftIcon={<ExternalLink size={14} />}
      onClick={() => openLoginWindow()}
      sx={{
        bg: activeColor,
        color: contrastText,
        _hover: { bg: activeColor, filter: "brightness(0.9)" },
        _active: { bg: activeColor, filter: "brightness(0.8)" },
      }}
    >
      登录
    </Button>
  );
}
