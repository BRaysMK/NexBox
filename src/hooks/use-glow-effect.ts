"use client";

import { useState, useCallback } from "react";

interface GlowEffectResult {
  mouseX: number;
  mouseY: number;
  isHovering: boolean;
  handleMouseMove: (e: React.MouseEvent<HTMLElement>) => void;
  handleMouseLeave: () => void;
  handleMouseEnter: () => void;
}

export function useGlowEffect(): GlowEffectResult {
  const [mouseX, setMouseX] = useState(50);
  const [mouseY, setMouseY] = useState(50);
  const [isHovering, setIsHovering] = useState(false);

  const handleMouseMove = useCallback((e: React.MouseEvent<HTMLElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const x = ((e.clientX - rect.left) / rect.width) * 100;
    const y = ((e.clientY - rect.top) / rect.height) * 100;
    setMouseX(x);
    setMouseY(y);
  }, []);

  const handleMouseLeave = useCallback(() => {
    setIsHovering(false);
  }, []);

  const handleMouseEnter = useCallback(() => {
    setIsHovering(true);
  }, []);

  return {
    mouseX,
    mouseY,
    isHovering,
    handleMouseMove,
    handleMouseLeave,
    handleMouseEnter,
  };
}

export function getBorderGlowStyle(
  mouseX: number,
  mouseY: number,
  isHovering: boolean,
  glowColor?: string,
  borderWidth: number = 1
): React.CSSProperties {
  const color = glowColor || "rgba(255, 255, 255, 0.6)";
  
  return {
    position: "absolute" as const,
    inset: `-${borderWidth}px`,
    borderRadius: "inherit",
    padding: `${borderWidth}px`,
    background: isHovering
      ? `radial-gradient(circle at ${mouseX}% ${mouseY}%, ${color} 0%, transparent 50%)`
      : "transparent",
    WebkitMask: "linear-gradient(#fff 0 0) content-box, linear-gradient(#fff 0 0)",
    WebkitMaskComposite: "xor",
    maskComposite: "exclude",
    pointerEvents: "none" as const,
    transition: "opacity 0.2s ease",
    opacity: isHovering ? 1 : 0,
    willChange: "opacity",
  };
}
