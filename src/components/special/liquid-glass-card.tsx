"use client";

import { Box, BoxProps, useColorModeValue } from "@chakra-ui/react";
import { useBackground } from "@/contexts/background-context";
import { getBorderGlowStyle } from "@/hooks/use-glow-effect";
import { useMemo } from "react";

interface LiquidGlassCardProps extends BoxProps {
  children: React.ReactNode;
  className?: string;
  isDashed?: boolean;
}

export function LiquidGlassCard({
  children,
  className,
  isDashed = false,
  ...props
}: LiquidGlassCardProps) {
  const { liquidGlassEnabled } = useBackground();
  
  const glassBgColor = useColorModeValue("rgba(255,255,255,0.25)", "rgba(0,0,0,0.25)");
  const glassBorderColor = useColorModeValue("rgba(255,255,255,0.2)", "rgba(255,255,255,0.1)");
  const glowColor = useColorModeValue("rgba(255,255,255,0.8)", "rgba(255,255,255,0.5)");
  const defaultBg = useColorModeValue("white", "#111111");
  const defaultBorder = useColorModeValue("gray.200", "#333333");

  const cardStyles = useMemo(() => ({
    bg: liquidGlassEnabled ? glassBgColor : defaultBg,
    borderRadius: "xl",
    border: isDashed ? "1px dashed" : "1px solid",
    borderColor: liquidGlassEnabled ? glassBorderColor : defaultBorder,
    backdropFilter: liquidGlassEnabled ? "blur(1px)" : "blur(0px)",
    boxShadow: "sm",
    transition: "background 0.45s cubic-bezier(0.4, 0, 0.2, 1), border-color 0.45s cubic-bezier(0.4, 0, 0.2, 1), backdrop-filter 0.45s cubic-bezier(0.4, 0, 0.2, 1)",
    sx: {
      transform: "translateZ(0)",
      WebkitTransform: "translateZ(0)",
      WebkitBackfaceVisibility: "hidden",
      backfaceVisibility: "hidden",
    },
  }), [liquidGlassEnabled, glassBgColor, glassBorderColor, defaultBg, defaultBorder, isDashed]);

  return (
    <Box
      className={className}
      {...cardStyles}
      position="relative"
      {...props}
    >
      <Box
        style={getBorderGlowStyle(glowColor)}
        opacity={liquidGlassEnabled ? 1 : 0}
        transition="opacity 0.45s cubic-bezier(0.4, 0, 0.2, 1)"
      />
      {children}
    </Box>
  );
}
