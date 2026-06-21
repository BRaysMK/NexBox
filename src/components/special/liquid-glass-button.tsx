"use client";

import { Button, useColorModeValue } from "@chakra-ui/react";
import { useBackground } from "@/contexts/background-context";
import { useThemeColor } from "@/contexts/theme-color-context";

interface LiquidGlassButtonProps {
  children: React.ReactNode;
  className?: string;
  [key: string]: any;
}

export function LiquidGlassButton({ children, className, ...props }: LiquidGlassButtonProps) {
  const { liquidGlassEnabled } = useBackground();
  const { getActiveColor, getHoverColor, getContrastTextColor } = useThemeColor();
  
  const glassBorderColor = useColorModeValue("rgba(255,255,255,0.3)", "rgba(255,255,255,0.15)");
  const hoverBgColor = useColorModeValue("rgba(255,255,255,0.4)", "rgba(0,0,0,0.4)");

  if (!liquidGlassEnabled) {
    return (
      <Button
        className={className}
        bg={getActiveColor()}
        color={getContrastTextColor()}
        _hover={{
          bg: getHoverColor(),
        }}
        {...props}
      >
        {children}
      </Button>
    );
  }

  return (
    <Button
      className={className}
      bg={getActiveColor()}
      color={getContrastTextColor()}
      border="1px solid"
      borderColor={getHoverColor()}
      backdropFilter="blur(15px)"
      transform="translateZ(0)"
      sx={{ WebkitBackfaceVisibility: "hidden", backfaceVisibility: "hidden" }}
      _hover={{
        bg: getHoverColor(),
      }}
      {...props}
    >
      {children}
    </Button>
  );
}
