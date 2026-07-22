"use client";

import { Box, BoxProps, useColorModeValue } from "@chakra-ui/react";
import { useBackground } from "@/contexts/background-context";
import { getBorderGlowStyle } from "@/hooks/use-glow-effect";
import { useLiquidGlassRefraction } from "@/components/special/liquid-glass-svg-filter";
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
  const { liquidGlassEnabled, liquidGlassBlur, liquidGlassMode } = useBackground();
  const { svgSupported } = useLiquidGlassRefraction(liquidGlassEnabled && liquidGlassMode === "real");

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
  const isReal = liquidGlassEnabled && liquidGlassMode === "real" && !isDashed;

  const backdropFilter = useMemo(() => {
    if (!liquidGlassEnabled) return "none";
    if (isReal && svgSupported) return `url(#nexbox-liquid-glass-filter) saturate(1.4)`;
    if (isReal) return `saturate(1.4) brightness(1.05)`;
    return `blur(${effectiveBlur}px)`;
  }, [liquidGlassEnabled, isReal, svgSupported, effectiveBlur]);

  const cardStyles = useMemo(() => ({
    bg: liquidGlassEnabled ? glassBgColor : defaultBg,
    borderRadius: "xl" as const,
    border: isDashed ? "1px dashed" : "1px solid",
    borderColor: liquidGlassEnabled ? glassBorderColor : defaultBorder,
    backdropFilter,
    WebkitBackdropFilter: backdropFilter,
    transition: "background 0.45s cubic-bezier(0.4, 0, 0.2, 1), border-color 0.45s cubic-bezier(0.4, 0, 0.2, 1), backdrop-filter 0.45s cubic-bezier(0.4, 0, 0.2, 1)",
    sx: {
      transform: "translateZ(0)",
      WebkitTransform: "translateZ(0)",
      WebkitBackfaceVisibility: "hidden",
      backfaceVisibility: "hidden",
      ...(liquidGlassEnabled ? { willChange: "backdrop-filter" } : {}),
    },
  }), [backdropFilter, liquidGlassEnabled, glassBgColor, glassBorderColor, defaultBg, defaultBorder, isDashed]);

  const borderGlowStyle = useMemo(() => getBorderGlowStyle(glowColor), [glowColor]);

  return (
    <Box
      className={`jelly-bounce-card${isReal ? " real-liquid-glass" : ""}${className ? ` ${className}` : ""}`}
      {...cardStyles}
      position="relative"
      {...props}
    >
      {!isReal && (
        <Box
          style={borderGlowStyle}
          opacity={liquidGlassEnabled ? 1 : 0}
          transition="opacity 0.45s cubic-bezier(0.4, 0, 0.2, 1)"
        />
      )}
      {children}
    </Box>
  );
}
