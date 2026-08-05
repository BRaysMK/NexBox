"use client";

import { Box, HStack, Text, useColorModeValue } from "@chakra-ui/react";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import { useGlowEffect, getBorderGlowStyle } from "@/hooks/use-glow-effect";
import { useLiquidGlassRefraction } from "@/components/special/liquid-glass-svg-filter";
import { useState, useEffect } from "react";

interface LiquidGlassMenuItemProps {
  children: React.ReactNode;
  isActive?: boolean;
  onClick?: () => void;
  icon?: React.ElementType;
}

export function LiquidGlassMenuItem({
  children,
  isActive = false,
  onClick,
  icon: Icon
}: LiquidGlassMenuItemProps) {
  const { liquidGlassEnabled, liquidGlassBlur, liquidGlassMode } = useBackground();
  const { getActiveColor, getBorderColor, getContrastTextColor } = useThemeColor();
  const { mouseX, mouseY, isHovering, handleMouseMove, handleMouseLeave, handleMouseEnter } = useGlowEffect();
  const { svgSupported } = useLiquidGlassRefraction(liquidGlassEnabled && liquidGlassMode === "real");

  const activeBg = getActiveColor();
  const activeTextFinal = getContrastTextColor();
  const inactiveText = useColorModeValue("gray.500", "#ffffff");
  const glassInactiveText = useColorModeValue("gray.900", "#ffffff");
  const defaultInactiveBg = useColorModeValue("white", "#111111");
  const hoverBg = useColorModeValue("gray.200", "#333333");

  const glassInactiveBg = useColorModeValue("rgba(255,255,255,0.25)", "rgba(0,0,0,0.25)");
  const glassHoverBg = useColorModeValue("rgba(255,255,255,0.35)", "rgba(0,0,0,0.35)");
  const glassBorderColor = useColorModeValue("rgba(255,255,255,0.25)", "rgba(255,255,255,0.12)");
  const glassActiveBorder = getBorderColor();
  const outlineColor = getActiveColor();
  const glowColor = useColorModeValue("rgba(255,255,255,0.8)", "rgba(255,255,255,0.5)");

  const [showBlur, setShowBlur] = useState(false);

  useEffect(() => {
    if (liquidGlassEnabled) {
      const timer = setTimeout(() => setShowBlur(true), 250);
      return () => clearTimeout(timer);
    } else {
      setShowBlur(false);
    }
  }, [liquidGlassEnabled]);

  const effectiveBlur = showBlur ? liquidGlassBlur : 0;
  const transition = "background 0.45s cubic-bezier(0.4, 0, 0.2, 1), border-color 0.45s cubic-bezier(0.4, 0, 0.2, 1), backdrop-filter 0.45s cubic-bezier(0.4, 0, 0.2, 1)";
  const isReal = liquidGlassEnabled && liquidGlassMode === "real";

  const backdropFilter = isReal
    ? (svgSupported
        ? `url(#nexbox-liquid-glass-filter) saturate(1.4)`
        : `saturate(1.4) brightness(1.05)`)
    : `blur(${effectiveBlur}px)`;

  return (
    <Box
      className={`jelly-bounce-menu-item${isReal ? " real-liquid-glass" : ""}`}
      onClick={onClick}
      cursor="pointer"
      borderRadius="lg"
      px={3}
      py={2.5}
      bg={isActive ? activeBg : (liquidGlassEnabled ? (isHovering ? glassHoverBg : glassInactiveBg) : (isHovering ? hoverBg : defaultInactiveBg))}
      border="1px solid"
      borderColor={isActive ? (liquidGlassEnabled ? glassActiveBorder : activeBg) : (liquidGlassEnabled ? glassBorderColor : "transparent")}
      backdropFilter={backdropFilter}
      position="relative"
      color={isActive ? activeTextFinal : (liquidGlassEnabled ? glassInactiveText : inactiveText)}
      onMouseMove={handleMouseMove}
      onMouseLeave={handleMouseLeave}
      onMouseEnter={handleMouseEnter}
      sx={{
        WebkitBackdropFilter: backdropFilter,
        transform: "translateZ(0)",
        WebkitTransform: "translateZ(0)",
        WebkitBackfaceVisibility: "hidden",
        backfaceVisibility: "hidden",
        willChange: "auto",
        transition,
      }}
      _focusVisible={{
        outline: "2px solid",
        outlineColor: outlineColor,
        outlineOffset: "2px"
      }}
    >
      {!isReal && (
        <Box
          style={getBorderGlowStyle(glowColor)}
          opacity={liquidGlassEnabled ? 1 : 0}
          transition="opacity 0.45s cubic-bezier(0.4, 0, 0.2, 1)"
        />
      )}
      <HStack position="relative" zIndex={1}>
        {Icon && <Icon size={18} />}
        <Text fontSize="sm" fontWeight={isActive ? "semibold" : "normal"}>
          {children}
        </Text>
      </HStack>
    </Box>
  );
}
