import { Box as ChakraBox, Flex, IconButton, useColorModeValue, Badge, Image } from "@chakra-ui/react";
import { Home, Wrench, Settings, Cpu, TrendingUp, Heart, Package, Crosshair } from "lucide-react";
import { Link, useLocation } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import deltaForceIcon from "@/assets/deltaforce.png";
import epicGamesIcon from "@/assets/epic-games.png";

interface NavItem {
  path: string;
  icon: React.ComponentType<{ size?: number; strokeWidth?: number }> | null;
  customIcon?: string;
  ariaLabel: string;
  beta?: boolean;
}

function NavButton({ item, isActive, activeBg, hoverBg, iconColor, activeIconColor }: {
  item: NavItem;
  isActive: boolean;
  activeBg: string;
  hoverBg: string;
  iconColor: string;
  activeIconColor: string;
}) {
  const isCustom = !!item.customIcon;

  return (
    <Link key={item.path} to={item.path}>
      <ChakraBox position="relative">
        <IconButton
          aria-label={item.ariaLabel}
          icon={
            isCustom ? (
              <Image
                src={item.customIcon}
                alt={item.ariaLabel}
                w="22px"
                h="22px"
                objectFit="contain"
                filter={isActive ? "none" : "grayscale(30%) opacity(0.7)"}
                transition="filter 0.2s"
              />
            ) : (
              <item.icon size={20} strokeWidth={2.2} />
            )
          }
          variant="ghost"
          borderRadius="xl"
          bg={isActive ? activeBg : "transparent"}
          color={isActive ? activeIconColor : iconColor}
          _hover={{
            bg: isActive ? activeBg : hoverBg,
            transform: "scale(1.1)",
          }}
          _active={{
            transform: "scale(0.95)",
          }}
          transition="all 0.2s cubic-bezier(0.4, 0, 0.2, 1)"
          size="lg"
          w="48px"
          h="48px"
        />
        {item.beta && (
          <Badge
            position="absolute"
            top="-2px"
            right="-2px"
            colorScheme="purple"
            fontSize="8px"
            px={1}
            py={0}
            borderRadius="full"
            textTransform="uppercase"
            fontWeight="bold"
          >
            BETA
          </Badge>
        )}
      </ChakraBox>
    </Link>
  );
}

export function Sidebar() {
  const location = useLocation();
  const { t } = useTranslation();
  const { liquidGlassEnabled } = useBackground();
  const { getActiveColor, getHoverColor, getContrastTextColor } = useThemeColor();
  
  const defaultBgColor = useColorModeValue("rgba(255,255,255,0.9)", "rgba(17,17,17,0.95)");
  const glassBgColor = useColorModeValue("rgba(255,255,255,0.25)", "rgba(0,0,0,0.25)");
  const defaultBorderColor = useColorModeValue("rgba(200,200,200,0.3)", "rgba(51,51,51,0.5)");
  const glassBorderColor = useColorModeValue("rgba(255,255,255,0.2)", "rgba(255,255,255,0.1)");
  
  const iconColor = useColorModeValue("rgba(107,114,128,0.8)", "rgba(156,163,175,0.7)");

  const activeBg = getActiveColor();
  const hoverBg = getHoverColor(true);
  const activeIconColor = getContrastTextColor();

  const navItems: NavItem[] = [
    { path: "/", icon: Home, ariaLabel: t("sidebar.home") },
    { path: "/hardware", icon: Cpu, ariaLabel: t("sidebar.hardware") },
    { path: "/tools", icon: Wrench, ariaLabel: t("sidebar.tools") },
    { path: "/builtin-tools", icon: Package, ariaLabel: t("sidebar.builtinTools") },
    { path: "/tests", icon: Crosshair, ariaLabel: t("sidebar.tests") },
    { path: "/optimization", icon: TrendingUp, ariaLabel: t("sidebar.optimization") },
    { path: "/delta-force", icon: null, customIcon: deltaForceIcon, ariaLabel: t("sidebar.deltaForce") },
    { path: "/epic-free", icon: null, customIcon: epicGamesIcon, ariaLabel: t("sidebar.epicFree") },
    { path: "/mood", icon: Heart, ariaLabel: t("sidebar.mood") },
    { path: "/settings", icon: Settings, ariaLabel: t("sidebar.settings") },
  ];

  const sidebarContent = (
    <Flex direction="column" gap={3}>
      {navItems.map((item) => (
        <NavButton
          key={item.path}
          item={item}
          isActive={location.pathname === item.path}
          activeBg={activeBg}
          hoverBg={hoverBg}
          iconColor={iconColor}
          activeIconColor={activeIconColor}
        />
      ))}
    </Flex>
  );

  if (liquidGlassEnabled) {
    return (
      <ChakraBox
        position="fixed"
        left={6}
        top="50%"
        transform="translateY(-50%) translateZ(0)"
        zIndex={40}
        bg={glassBgColor}
        borderRadius="2xl"
        boxShadow="2xl"
        border="1px solid"
        borderColor={glassBorderColor}
        py={6}
        px={2}
        backdropFilter="blur(20px)"
        sx={{ WebkitBackfaceVisibility: "hidden", backfaceVisibility: "hidden" }}
      >
        {sidebarContent}
      </ChakraBox>
    );
  }

  return (
    <ChakraBox
      position="fixed"
      left={6}
      top="50%"
      transform="translateY(-50%) translateZ(0)"
      zIndex={40}
      bg={defaultBgColor}
      borderRadius="2xl"
      boxShadow="2xl"
      border="1px solid"
      borderColor={defaultBorderColor}
      py={6}
      px={2}
      backdropFilter="blur(12px)"
      sx={{ WebkitBackfaceVisibility: "hidden", backfaceVisibility: "hidden" }}
    >
      {sidebarContent}
    </ChakraBox>
  );
}
