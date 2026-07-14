"use client";

import { Box, BoxProps, useColorModeValue } from "@chakra-ui/react";
import { useBackground } from "@/contexts/background-context";
import { getBorderGlowStyle } from "@/hooks/use-glow-effect";
import { useMemo, useState, useEffect } from "react";

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
  const { liquidGlassEnabled, liquidGlassBlur } = useBackground();
  
  const glassBgColor = useColorModeValue("rgba(255,255,255,0.25)", "rgba(0,0,0,0.25)");
  const glassBorderColor = useColorModeValue("rgba(255,255,255,0.2)", "rgba(255,255,255,0.1)");
  const glowColor = useColorModeValue("rgba(255,255,255,0.8)", "rgba(255,255,255,0.5)");
  const defaultBg = useColorModeValue("white", "#111111");
  const defaultBorder = useColorModeValue("gray.200", "#333333");

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

  const cardStyles = useMemo(() => ({
    bg: liquidGlassEnabled ? glassBgColor : defaultBg,
    borderRadius: "xl",
    border: isDashed ? "1px dashed" : "1px solid",
    borderColor: liquidGlassEnabled ? glassBorderColor : defaultBorder,
    backdropFilter: `blur(${effectiveBlur}px)`,
    boxShadow: "sm",
    transition: "background 0.45s cubic-bezier(0.4, 0, 0.2, 1), border-color 0.45s cubic-bezier(0.4, 0, 0.2, 1), backdrop-filter 0.45s cubic-bezier(0.4, 0, 0.2, 1)",
    sx: {
      transform: "translateZ(0)",
      WebkitTransform: "translateZ(0)",
      WebkitBackfaceVisibility: "hidden",
      backfaceVisibility: "hidden",
      // willChange 仅在启用液态玻璃时设置，避免大量静态卡片累积 GPU 图层
      ...(liquidGlassEnabled ? { willChange: "backdrop-filter" } : {}),
    },
  }), [effectiveBlur, liquidGlassEnabled, glassBgColor, glassBorderColor, defaultBg, defaultBorder, isDashed]);

  const borderGlowStyle = useMemo(() => getBorderGlowStyle(glowColor), [glowColor]);

  return (
    <Box
      className={className}
      {...cardStyles}
      position="relative"
      {...props}
    >
      <Box
        style={borderGlowStyle}
        opacity={liquidGlassEnabled ? 1 : 0}
        transition="opacity 0.45s cubic-bezier(0.4, 0, 0.2, 1)"
      />
      {children}
    </Box>
  );
}
