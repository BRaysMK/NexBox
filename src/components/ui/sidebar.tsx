import { Box as ChakraBox, Flex, IconButton, Text, useColorModeValue, Badge, Image } from "@chakra-ui/react";
import { Home, Wrench, Settings, Cpu, TrendingUp, Heart, Package, Music, LayoutGrid } from "lucide-react";
import { Link, useLocation } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import { getBorderGlowStyle } from "@/hooks/use-glow-effect";
import { useLiquidGlassRefraction } from "@/components/special/liquid-glass-svg-filter";
import deltaForceIcon from "@/assets/deltaforce.png";
import epicGamesIcon from "@/assets/epic-games.png";
import { useState, useEffect, useRef, useCallback, useMemo } from "react";

interface NavItem {
  path: string;
  icon: React.ComponentType<{ size?: number; strokeWidth?: number }> | null;
  customIcon?: string;
  ariaLabel: string;
  beta?: boolean;
}

const NAV_ORDER_KEY = "nexbox_nav_order";

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
  const { getActiveColor } = useThemeColor();
  const activeColor = getActiveColor();
  const [bursts, setBursts] = useState<Array<{id: number, x: number, y: number}>>([]);
  const nextId = useRef(0);

  const handleBurst = useCallback((e: React.MouseEvent) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const id = nextId.current++;
    setBursts(prev => [...prev, { id, x, y }]);
    setTimeout(() => {
      setBursts(prev => prev.filter(b => b.id !== id));
    }, 450);
  }, []);

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
    <Link key={item.path} to={item.path} className="jelly-bounce-nav-button" style={{ transition: "all 0.4s cubic-bezier(0.4, 0, 0.2, 1)", flexShrink: 0, display: "flex" }}>
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
            position="relative"
            overflow="hidden"
            onClick={handleBurst}
          >
            <ChakraBox display="flex" alignItems="center" justifyContent="center" lineHeight={0} position="relative" zIndex={1}>
              {iconElement}
            </ChakraBox>
            <Text fontSize="2xs" fontWeight="medium" noOfLines={1} textAlign="center" lineHeight="1.1" position="relative" zIndex={1}>
              {item.ariaLabel}
            </Text>
            {bursts.map(b => (
              <ChakraBox
                key={b.id}
                position="absolute"
                left={0}
                top={0}
                w="48px"
                h="48px"
                borderRadius="xl"
                pointerEvents="none"
                zIndex={3}
                bg="red.400"
                opacity={0.7}
                sx={{
                  animation: "navBurstGrow 0.4s ease-out forwards",
                }}
              />
            ))}
          </Flex>
        ) : (
          <ChakraBox position="relative" overflow="hidden" borderRadius="xl" onClick={handleBurst}>
            <ChakraBox position="relative" zIndex={1}>
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
            </ChakraBox>
            {bursts.map(b => (
              <ChakraBox
                key={b.id}
                position="absolute"
                left={0}
                top={0}
                w="48px"
                h="48px"
                borderRadius="xl"
                pointerEvents="none"
                zIndex={3}
                bg="red.400"
                opacity={0.7}
                sx={{
                  animation: "navBurstGrow 0.4s ease-out forwards",
                }}
              />
            ))}
          </ChakraBox>
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
  const { liquidGlassEnabled, liquidGlassBlur, liquidGlassMode, jellyBounceEnabled } = useBackground();
  const { getActiveColor, getHoverColor, getContrastTextColor } = useThemeColor();
  const { svgSupported } = useLiquidGlassRefraction(liquidGlassEnabled && liquidGlassMode === "real");
  const [showLabel, setShowLabel] = useState(() => {
    return localStorage.getItem("nexbox_sidebar_show_label") === "true";
  });
  const [navPosition, setNavPosition] = useState<"left" | "top">(() => {
    const saved = localStorage.getItem("nexbox_nav_position");
    return saved === "top" ? "top" : "left";
  });

  // 果冻弹跳效果：当导航栏状态切换（位置/标签/可见性）时触发
  const sidebarContentRef = useRef<HTMLDivElement>(null);
  const sidebarContainerRef = useRef<HTMLDivElement>(null);
  const isFirstRender = useRef(true);

  const triggerJellyBounce = useCallback(() => {
    if (!jellyBounceEnabled) return;
    // 触发内容区弹跳
    const contentEl = sidebarContentRef.current;
    if (contentEl) {
      contentEl.classList.remove("jelly-bouncing");
      void contentEl.offsetWidth;
      contentEl.classList.add("jelly-bouncing");
    }
    // 触发外层容器弹跳
    const containerEl = sidebarContainerRef.current;
    if (containerEl) {
      containerEl.classList.remove("jelly-bouncing");
      void containerEl.offsetWidth;
      containerEl.classList.add("jelly-bouncing");
    }
  }, [jellyBounceEnabled]);
  
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
  
  const hideableNavPaths = ["/hardware", "/tools", "/builtin-tools", "/optimization", "/music", "/delta-force", "/epic-free", "/mood", "/custom"];
  const getNavStorageKey = (path: string) => `nexbox_nav_visible_${path.replace(/\//g, "").replace(/-/g, "_")}`;

  const [navVisibility, setNavVisibility] = useState<Record<string, boolean>>(() => {
    const initial: Record<string, boolean> = {};
    for (const path of hideableNavPaths) {
      if (path === "/custom") {
        // 自定义页面默认关闭
        initial[path] = localStorage.getItem(getNavStorageKey(path)) === "true";
      } else {
        initial[path] = localStorage.getItem(getNavStorageKey(path)) !== "false";
      }
    }
    return initial;
  });

  // 读取导航栏排序（拖拽在设置页面进行）
  const [navOrder, setNavOrder] = useState<string[] | null>(() => {
    try {
      const saved = localStorage.getItem(NAV_ORDER_KEY);
      return saved ? JSON.parse(saved) : null;
    } catch { return null; }
  });

  // 监听排序变化
  useEffect(() => {
    const handler = () => {
      try {
        const saved = localStorage.getItem(NAV_ORDER_KEY);
        setNavOrder(saved ? JSON.parse(saved) : null);
      } catch { setNavOrder(null); }
    };
    window.addEventListener("nav-order-changed", handler as EventListener);
    return () => { window.removeEventListener("nav-order-changed", handler as EventListener); };
  }, []);

  useEffect(() => {
    const handler = (e: CustomEvent) => {
      const { path, visible } = e.detail || {};
      if (path) {
        setNavVisibility(prev => ({ ...prev, [path]: visible }));
      }
    };
    window.addEventListener("nav-visibility-changed", handler as EventListener);
    return () => { window.removeEventListener("nav-visibility-changed", handler as EventListener); };
  }, []);

  // 监听导航栏状态变化/页面切换，触发果冻弹跳动画（跳过初次渲染）
  const navVisibilityKey = JSON.stringify(navVisibility);
  useEffect(() => {
    if (isFirstRender.current) {
      return;
    }
    triggerJellyBounce();
  }, [navPosition, showLabel, navVisibilityKey, location.pathname, triggerJellyBounce]);

  useEffect(() => {
    isFirstRender.current = false;
  }, []);

  const [showBlur, setShowBlur] = useState(false);

  useEffect(() => {
    if (typeof document === "undefined") return;
    const styleId = "nav-burst-keyframes";
    if (document.getElementById(styleId)) return;
    const style = document.createElement("style");
    style.id = styleId;
    style.textContent = `@keyframes navBurstGrow{from{transform:scale(0);opacity:0.8}to{transform:scale(3.5);opacity:0}}`;
    document.head.appendChild(style);
  }, []);

  useEffect(() => {
    if (liquidGlassEnabled) {
      const timer = setTimeout(() => setShowBlur(true), 250);
      return () => clearTimeout(timer);
    } else {
      setShowBlur(false);
    }
  }, [liquidGlassEnabled]);

  const effectiveBlur = showBlur ? liquidGlassBlur : 0;
  const isReal = liquidGlassEnabled && liquidGlassMode === "real";
  const backdropFilter = isReal
    ? (svgSupported
        ? `url(#nexbox-liquid-glass-filter) saturate(1.4)`
        : `saturate(1.4) brightness(1.05)`)
    : `blur(${effectiveBlur}px)`;

  const defaultBgColor = useColorModeValue("rgba(255,255,255,1)", "rgba(17,17,17,1)");
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
    { path: "/optimization", icon: TrendingUp, ariaLabel: t("sidebar.optimization") },
    { path: "/music", icon: Music, ariaLabel: t("sidebar.music") },
    { path: "/delta-force", icon: null, customIcon: deltaForceIcon, ariaLabel: t("sidebar.deltaForce") },
    { path: "/epic-free", icon: null, customIcon: epicGamesIcon, ariaLabel: t("sidebar.epicFree") },
    { path: "/mood", icon: Heart, ariaLabel: t("sidebar.mood") },
    { path: "/custom", icon: LayoutGrid, ariaLabel: t("sidebar.custom") },
    { path: "/settings", icon: Settings, ariaLabel: t("sidebar.settings") },
  ];

  // 根据保存的顺序排序 navItems（首页和设置固定首尾）
  const sortedNavItems = useMemo(() => {
    if (!navOrder) return navItems;
    const map = new Map(navItems.map(i => [i.path, i]));
    const ordered: NavItem[] = [];
    // 首页始终在最前
    const home = map.get("/");
    if (home) ordered.push(home);
    // 按 navOrder 排序中间项
    for (const p of navOrder) {
      if (p === "/" || p === "/settings") continue;
      const item = map.get(p);
      if (item) ordered.push(item);
    }
    // 追加未在 order 中的新项
    navItems.forEach(item => {
      if (item.path === "/" || item.path === "/settings") return;
      if (!ordered.find(i => i.path === item.path)) ordered.push(item);
    });
    // 设置始终在最后
    const settings = map.get("/settings");
    if (settings) ordered.push(settings);
    return ordered;
  }, [navItems, navOrder]);

  const visibleNavItems = sortedNavItems.filter(item => {
    if (item.path === "/" || item.path === "/settings") return true;
    return navVisibility[item.path] !== false;
  });

  const sidebarContent = (
    <Flex
      ref={sidebarContentRef}
      className="jelly-bounce-sidebar-content"
      direction="row" wrap="wrap" gap={3} align="center" justify="center" transition="gap 0.4s cubic-bezier(0.4, 0, 0.2, 1)"
      onAnimationEnd={(e: React.AnimationEvent) => {
        if (e.animationName === "jellyBounce") {
          sidebarContentRef.current?.classList.remove("jelly-bouncing");
        }
      }}
    >
      {visibleNavItems.map((item) => (
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
      ref={sidebarContainerRef}
      id="main-sidebar"
      className={`${isTop ? "jelly-bounce-sidebar-top" : "jelly-bounce-sidebar-left"}${isReal ? " real-liquid-glass" : ""}`}
      {...containerStyles}
      bg={liquidGlassEnabled ? glassBgColor : defaultBgColor}
      border="1px solid"
      borderColor={liquidGlassEnabled ? glassBorderColor : defaultBorderColor}
      backdropFilter={backdropFilter}
      transition="max-width 0.25s cubic-bezier(0.95, 0, 1, 1), left 0.4s cubic-bezier(0.4, 0, 0.2, 1), top 0.4s cubic-bezier(0.4, 0, 0.2, 1), transform 0.4s cubic-bezier(0.4, 0, 0.2, 1), padding 0.4s cubic-bezier(0.4, 0, 0.2, 1), background 0.45s cubic-bezier(0.4, 0, 0.2, 1), backdrop-filter 0.45s cubic-bezier(0.4, 0, 0.2, 1)"
      sx={{
        ...containerStyles.sx,
        willChange: "auto",
      }}
      onAnimationEnd={(e: React.AnimationEvent) => {
        const name = e.animationName;
        if (name === "jellyBounceSidebarLeft" || name === "jellyBounceSidebarTop") {
          sidebarContainerRef.current?.classList.remove("jelly-bouncing");
        }
      }}
    >
      {!isReal && (
        <ChakraBox
          style={getBorderGlowStyle(glowColor)}
          opacity={liquidGlassEnabled ? 1 : 0}
          transition="opacity 0.45s cubic-bezier(0.4, 0, 0.2, 1)"
        />
      )}
      {sidebarContent}
    </ChakraBox>
  );
}
