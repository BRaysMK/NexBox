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
  const glassBg = getActiveColor();

  return (
    <Button
      className={className}
      bg={glassBg}
      color={getContrastTextColor()}
      border="1px solid"
      borderColor={liquidGlassEnabled ? getHoverColor() : glassBg}
      backdropFilter={liquidGlassEnabled ? "blur(15px)" : "blur(0px)"}
      sx={{
        WebkitBackfaceVisibility: "hidden",
        backfaceVisibility: "hidden",
        transition: "border-color 0.45s cubic-bezier(0.4, 0, 0.2, 1), backdrop-filter 0.45s cubic-bezier(0.4, 0, 0.2, 1)",
      }}
      _hover={{
        bg: getHoverColor(),
      }}
      {...props}
    >
      {children}
    </Button>
  );
}
