import { Box as ChakraBox, Flex, IconButton, Text, useColorModeValue, Badge, Image } from "@chakra-ui/react";
import { Home, Wrench, Settings, Cpu, TrendingUp, Heart, Package, Crosshair, Music } from "lucide-react";
import { Link, useLocation } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import { getBorderGlowStyle } from "@/hooks/use-glow-effect";
import deltaForceIcon from "@/assets/deltaforce.png";
import epicGamesIcon from "@/assets/epic-games.png";
import { useState, useEffect } from "react";

interface NavItem {
  path: string;
  icon: React.ComponentType<{ size?: number; strokeWidth?: number }> | null;
  customIcon?: string;
  ariaLabel: string;
  beta?: boolean;
}

function NavButton({ item, isActive, activeBg, hoverBg, iconColor, activeIconColor, showLabel }: {
  item: NavItem;
  isActive: boolean;
  activeBg: string;
  hoverBg: string;
  iconColor: string;
  activeIconColor: string;
  showLabel: boolean;
}) {
  const isCustom = !!item.customIcon;

  const iconElement = isCustom ? (
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
  );

  return (
    <Link key={item.path} to={item.path} style={{ transition: "all 0.4s cubic-bezier(0.4, 0, 0.2, 1)", flexShrink: 0, display: "flex" }}>
      <ChakraBox position="relative">
        {showLabel ? (
          <Flex
            direction="column"
            align="center"
            justify="center"
            gap={0.5}
            aria-label={item.ariaLabel}
            w="48px"
            h="48px"
            borderRadius="xl"
            cursor="pointer"
            bg={isActive ? activeBg : "transparent"}
            color={isActive ? activeIconColor : iconColor}
            _hover={{ bg: isActive ? activeBg : hoverBg }}
            transition="all 0.2s cubic-bezier(0.4, 0, 0.2, 1)"
            as="span"
            role="button"
            tabIndex={0}
          >
            <ChakraBox display="flex" alignItems="center" justifyContent="center" lineHeight={0}>
              {iconElement}
            </ChakraBox>
            <Text fontSize="2xs" fontWeight="medium" noOfLines={1} textAlign="center" lineHeight="1.1">
              {item.ariaLabel}
            </Text>
          </Flex>
        ) : (
          <IconButton
            aria-label={item.ariaLabel}
            icon={iconElement}
            variant="ghost"
            borderRadius="xl"
            bg={isActive ? activeBg : "transparent"}
            color={isActive ? activeIconColor : iconColor}
            _hover={{ bg: isActive ? activeBg : hoverBg }}
            _active={{ transform: "scale(0.95)" }}
            transition="all 0.2s cubic-bezier(0.4, 0, 0.2, 1)"
            size="lg"
            w="48px"
            h="48px"
          />
        )}
        {!showLabel && item.beta && (
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
  const [showLabel, setShowLabel] = useState(() => {
    return localStorage.getItem("nexbox_sidebar_show_label") === "true";
  });
  const [navPosition, setNavPosition] = useState<"left" | "top">(() => {
    const saved = localStorage.getItem("nexbox_nav_position");
    return saved === "top" ? "top" : "left";
  });
  
  useEffect(() => {
    const handler = (e: CustomEvent) => {
      setShowLabel(e.detail === true);
    };
    window.addEventListener("sidebar-show-label-changed", handler as EventListener);
    return () => {
      window.removeEventListener("sidebar-show-label-changed", handler as EventListener);
    };
  }, []);

  useEffect(() => {
    const handler = (e: CustomEvent) => {
      const val = e.detail;
      setNavPosition(val === "top" ? "top" : "left");
    };
    window.addEventListener("nav-position-changed", handler as EventListener);
    return () => {
      window.removeEventListener("nav-position-changed", handler as EventListener);
    };
  }, []);

  const isTop = navPosition === "top";
  
  const defaultBgColor = useColorModeValue("rgba(255,255,255,0.9)", "rgba(17,17,17,0.95)");
  const glassBgColor = useColorModeValue("rgba(255,255,255,0.25)", "rgba(0,0,0,0.25)");
  const defaultBorderColor = useColorModeValue("rgba(200,200,200,0.3)", "rgba(51,51,51,0.5)");
  const glassBorderColor = useColorModeValue("rgba(255,255,255,0.2)", "rgba(255,255,255,0.1)");
  const glowColor = useColorModeValue("rgba(255,255,255,0.8)", "rgba(255,255,255,0.5)");
  
  const iconColor = useColorModeValue("rgba(0,0,0,0.75)", "rgba(255,255,255,0.8)");

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
    { path: "/music", icon: Music, ariaLabel: "音乐" },
    { path: "/settings", icon: Settings, ariaLabel: t("sidebar.settings") },
  ];

  const sidebarContent = (
    <Flex direction="row" wrap="wrap" gap={3} align="center" justify="center" transition="gap 0.4s cubic-bezier(0.4, 0, 0.2, 1)">
      {navItems.map((item) => (
        <NavButton
          key={item.path}
          item={item}
          isActive={location.pathname === item.path}
          activeBg={activeBg}
          hoverBg={hoverBg}
          iconColor={iconColor}
          activeIconColor={activeIconColor}
          showLabel={showLabel}
        />
      ))}
    </Flex>
  );

  const containerTransition = "max-width 0.25s cubic-bezier(0.95, 0, 1, 1), left 0.4s cubic-bezier(0.4, 0, 0.2, 1), top 0.4s cubic-bezier(0.4, 0, 0.2, 1), transform 0.4s cubic-bezier(0.4, 0, 0.2, 1), padding 0.4s cubic-bezier(0.4, 0, 0.2, 1)";

  const leftContainerStyles = {
    position: "fixed" as const,
    left: 6,
    top: "50%",
    transform: "translateY(-50%) translateZ(0)",
    zIndex: 40,
    borderRadius: "2xl",
    boxShadow: "2xl",
    py: 6,
    px: 2,
    maxWidth: "64px",
    transition: containerTransition,
    sx: { WebkitBackfaceVisibility: "hidden" as const, backfaceVisibility: "hidden" as const },
  };

  const topContainerStyles = {
    position: "fixed" as const,
    top: "54px",
    left: "50%",
    transform: "translateX(-50%) translateZ(0)",
    zIndex: 40,
    borderRadius: "2xl",
    boxShadow: "2xl",
    py: 2,
    px: 3,
    maxWidth: "2000px",
    width: "max-content",
    transition: containerTransition,
    sx: { WebkitBackfaceVisibility: "hidden" as const, backfaceVisibility: "hidden" as const },
  };

  const containerStyles = isTop ? topContainerStyles : leftContainerStyles;

  return (
    <ChakraBox
      id="main-sidebar"
      {...containerStyles}
      bg={liquidGlassEnabled ? glassBgColor : defaultBgColor}
      border="1px solid"
      borderColor={glassBorderColor}
      backdropFilter={liquidGlassEnabled ? "blur(1px)" : "blur(0px)"}
      transition="max-width 0.25s cubic-bezier(0.95, 0, 1, 1), left 0.4s cubic-bezier(0.4, 0, 0.2, 1), top 0.4s cubic-bezier(0.4, 0, 0.2, 1), transform 0.4s cubic-bezier(0.4, 0, 0.2, 1), padding 0.4s cubic-bezier(0.4, 0, 0.2, 1), background 0.45s cubic-bezier(0.4, 0, 0.2, 1), backdrop-filter 0.45s cubic-bezier(0.4, 0, 0.2, 1)"
    >
      <ChakraBox
        style={getBorderGlowStyle(glowColor)}
        opacity={liquidGlassEnabled ? 1 : 0}
        transition="opacity 0.45s cubic-bezier(0.4, 0, 0.2, 1)"
      />
      {sidebarContent}
    </ChakraBox>
  );
}
