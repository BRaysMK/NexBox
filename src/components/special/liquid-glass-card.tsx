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

  // 卡片主体：零滤镜
  const backdropFilter = useMemo(() => {
    if (!liquidGlassEnabled) return "none";
    if (isReal) return "none";
    return `blur(${effectiveBlur}px)`;
  }, [liquidGlassEnabled, isReal, effectiveBlur]);

  // 边缘折射条用的 filter
  const edgeFilter = svgSupported ? "url(#nexbox-liquid-glass-filter)" : "none";

  const cardStyles = useMemo(() => {
    const base: any = {
      bg: isReal ? "transparent" : (liquidGlassEnabled ? glassBgColor : defaultBg),
      borderRadius: "xl" as const,
      border: isDashed ? "1px dashed" : "1px solid",
      borderColor: isReal ? "rgba(255,255,255,0.06)" : (liquidGlassEnabled ? glassBorderColor : defaultBorder),
      backdropFilter,
      WebkitBackdropFilter: backdropFilter,
      transition: "background 0.45s, border-color 0.45s, backdrop-filter 0.45s",
    };
    return base;
  }, [backdropFilter, liquidGlassEnabled, glassBgColor, glassBorderColor, defaultBg, defaultBorder, isDashed, isReal]);

  const borderGlowStyle = useMemo(() => getBorderGlowStyle(glowColor), [glowColor]);

  return (
    <Box
      className={`jelly-bounce-card${isReal ? " real-liquid-glass" : ""}${className ? ` ${className}` : ""}`}
      {...cardStyles}
      position="relative"
      overflow="hidden"
      {...props}
    >
      {/* 边缘折射条 —— 独立叠加层，仅覆盖边缘 10px，卡片内部完全不受影响 */}
      {isReal && svgSupported && (
        <>
          <Box position="absolute" top={0} left={0} w="100%" h="10px" zIndex={0}
            backdropFilter={edgeFilter} sx={{ WebkitBackdropFilter: edgeFilter }}
            bg="transparent" pointerEvents="none" />
          <Box position="absolute" bottom={0} left={0} w="100%" h="10px" zIndex={0}
            backdropFilter={edgeFilter} sx={{ WebkitBackdropFilter: edgeFilter }}
            bg="transparent" pointerEvents="none" />
          <Box position="absolute" left={0} top={0} w="10px" h="100%" zIndex={0}
            backdropFilter={edgeFilter} sx={{ WebkitBackdropFilter: edgeFilter }}
            bg="transparent" pointerEvents="none" />
          <Box position="absolute" right={0} top={0} w="10px" h="100%" zIndex={0}
            backdropFilter={edgeFilter} sx={{ WebkitBackdropFilter: edgeFilter }}
            bg="transparent" pointerEvents="none" />
        </>
      )}

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
