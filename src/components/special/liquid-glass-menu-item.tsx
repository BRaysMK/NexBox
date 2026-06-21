"use client";

import { Box, HStack, Text, useColorModeValue } from "@chakra-ui/react";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";
import { useGlowEffect, getBorderGlowStyle } from "@/hooks/use-glow-effect";

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
  const { liquidGlassEnabled } = useBackground();
  const { getActiveColor, getBorderColor, getContrastTextColor } = useThemeColor();
  const { mouseX, mouseY, isHovering, handleMouseMove, handleMouseLeave, handleMouseEnter } = useGlowEffect();
  
  const activeBg = getActiveColor();
  const activeTextFinal = getContrastTextColor();
  const inactiveText = useColorModeValue("gray.500", "#d0d0d0");
  const glassInactiveText = useColorModeValue("gray.900", "#d0d0d0");
  const defaultInactiveBg = useColorModeValue("gray.100", "#222222");
  const hoverBg = useColorModeValue("gray.200", "#4a4a4a");
  
  const glassInactiveBg = useColorModeValue("rgba(255,255,255,0.2)", "rgba(30,30,30,0.6)");
  const glassHoverBg = useColorModeValue("rgba(255,255,255,0.3)", "rgba(50,50,50,0.7)");
  const glassBorderColor = useColorModeValue("rgba(255,255,255,0.25)", "rgba(255,255,255,0.12)");
  const glassActiveBorder = getBorderColor();
  const outlineColor = getActiveColor();
  const glowColor = useColorModeValue("rgba(255,255,255,0.8)", "rgba(255,255,255,0.5)");

  if (liquidGlassEnabled) {
    return (
      <Box
        onClick={onClick}
        cursor="pointer"
        borderRadius="lg"
        px={3}
        py={2.5}
        bg={isActive ? activeBg : (isHovering ? glassHoverBg : glassInactiveBg)}
        border="1px solid"
        borderColor={isActive ? glassActiveBorder : glassBorderColor}
        backdropFilter="blur(12px)"
        color={isActive ? activeTextFinal : glassInactiveText}
        position="relative"
        onMouseMove={handleMouseMove}
        onMouseLeave={handleMouseLeave}
        onMouseEnter={handleMouseEnter}
        sx={{
          transform: "translateZ(0)",
          WebkitTransform: "translateZ(0)",
          WebkitBackfaceVisibility: "hidden",
          backfaceVisibility: "hidden",
        }}
        _focusVisible={{
          outline: "2px solid",
          outlineColor: outlineColor,
          outlineOffset: "2px"
        }}
      >
        <Box
          style={getBorderGlowStyle(glowColor)}
        />
        <HStack position="relative" zIndex={1}>
          {Icon && <Icon size={18} />}
          <Text fontSize="sm" fontWeight={isActive ? "semibold" : "normal"}>
            {children}
          </Text>
        </HStack>
      </Box>
    );
  }

  return (
    <Box
      onClick={onClick}
      cursor="pointer"
      borderRadius="lg"
      px={3}
      py={2.5}
      bg={isActive ? activeBg : defaultInactiveBg}
      border="1px solid"
      borderColor="transparent"
      color={isActive ? activeTextFinal : inactiveText}
      _hover={{ bg: isActive ? activeBg : hoverBg }}
      _focusVisible={{
        outline: "2px solid",
        outlineColor: outlineColor,
        outlineOffset: "2px"
      }}
    >
      <HStack>
        {Icon && <Icon size={18} />}
        <Text fontSize="sm" fontWeight={isActive ? "semibold" : "normal"}>
          {children}
        </Text>
      </HStack>
    </Box>
  );
}
