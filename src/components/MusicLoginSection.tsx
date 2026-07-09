import { useState } from "react";
import {
  Box,
  VStack,
  HStack,
  Input,
  Button,
  Text,
  IconButton,
  Tooltip,
  Avatar,
  useColorModeValue,
} from "@chakra-ui/react";
import {
  ExternalLink,
  LogOut,
  Cookie,
} from "lucide-react";
import { useMusicStore, coverProxyUrl } from "@/stores/music-store";
import { useThemeColor } from "@/contexts/theme-color-context";

export function MusicLoginSection() {
  const store = useMusicStore();
  const [cookieInput, setCookieInput] = useState("");
  const [showCookieInput, setShowCookieInput] = useState(false);
  const { getActiveColor, getHoverColor, getContrastTextColor } = useThemeColor();

  const activeColor = getActiveColor();
  const contrastText = getContrastTextColor();
  const hoverBg = getHoverColor(false);

  const borderColor = useColorModeValue("gray.200", "#333333");
  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const subTextColor = useColorModeValue("gray.500", "#888888");

  // 已登录状态
  if (store.loginInfo?.logged_in) {
    return (
      <HStack spacing={3} justify="space-between">
        <HStack spacing={3}>
          <Avatar size="sm" src={coverProxyUrl(store.loginInfo.avatar, store.proxyPort)} />
          <VStack spacing={0} align="start">
            <Text color={textColor} fontSize="sm" fontWeight="medium">
              {store.loginInfo.nickname}
            </Text>
            <Text color={subTextColor} fontSize="xs">
              {store.loginInfo.is_svip ? "SVIP" : store.loginInfo.is_vip ? "VIP" : "普通用户"}
            </Text>
          </VStack>
        </HStack>
        <Tooltip label="退出登录">
          <IconButton
            aria-label="Logout"
            icon={<LogOut size={16} />}
            size="sm"
            variant="ghost"
            onClick={() => store.logout()}
          />
        </Tooltip>
      </HStack>
    );
  }

  // 未登录状态
  return (
    <VStack spacing={3} align="stretch" w="100%">
      <HStack wrap="wrap" spacing={2}>
        <Button
          size="sm"
          leftIcon={<ExternalLink size={14} />}
          onClick={() => store.openLoginWindow()}
          sx={{
            bg: activeColor,
            color: contrastText,
            _hover: { bg: activeColor, filter: "brightness(0.9)" },
            _active: { bg: activeColor, filter: "brightness(0.8)" },
          }}
        >
          网页登录
        </Button>
        <Button
          size="sm"
          variant="ghost"
          leftIcon={<Cookie size={14} />}
          onClick={() => setShowCookieInput(!showCookieInput)}
          sx={{ _hover: { bg: hoverBg } }}
        >
          {showCookieInput ? "收起" : "Cookie"}
        </Button>
      </HStack>

      {showCookieInput && (
        <VStack spacing={2} align="stretch">
          <Input
            placeholder="粘贴网易云 Cookie (MUSIC_U=...)"
            value={cookieInput}
            onChange={(e) => setCookieInput(e.target.value)}
            size="sm"
            borderColor={borderColor}
            _focus={{ borderColor: activeColor, boxShadow: `0 0 0 1px ${activeColor}` }}
          />
          <Button
            size="sm"
            onClick={async () => {
              if (await store.loginWithCookie(cookieInput)) {
                setCookieInput("");
                setShowCookieInput(false);
              }
            }}
            sx={{
              bg: activeColor,
              color: contrastText,
              _hover: { bg: activeColor, filter: "brightness(0.9)" },
              _active: { bg: activeColor, filter: "brightness(0.8)" },
            }}
          >
            登录
          </Button>
        </VStack>
      )}

      {!showCookieInput && (
        <Text color={subTextColor} fontSize="sm">未登录</Text>
      )}
    </VStack>
  );
}
