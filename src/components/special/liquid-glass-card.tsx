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

  const cardStyles = useMemo(() => {
    if (!liquidGlassEnabled) {
      return {
        bg: defaultBg,
        borderRadius: "xl",
        border: isDashed ? "1px dashed" : "1px solid",
        borderColor: defaultBorder,
        boxShadow: "sm",
      };
    }

    return {
      bg: glassBgColor,
      borderRadius: "xl",
        border: isDashed ? "1px dashed" : "1px solid",
      borderColor: glassBorderColor,
      backdropFilter: "blur(1px)",
      boxShadow: "sm",
      sx: {
        transform: "translateZ(0)",
        WebkitTransform: "translateZ(0)",
        WebkitBackfaceVisibility: "hidden",
        backfaceVisibility: "hidden",
      },
    };
  }, [liquidGlassEnabled, glassBgColor, glassBorderColor, defaultBg, defaultBorder, isDashed]);

  if (!liquidGlassEnabled) {
    return (
      <Box
        className={className}
        {...cardStyles}
        {...props}
      >
        {children}
      </Box>
    );
  }

  return (
    <Box
      className={className}
      {...cardStyles}
      position="relative"
      {...props}
    >
      <Box
        style={getBorderGlowStyle(glowColor)}
      />
      {children}
    </Box>
  );
}
