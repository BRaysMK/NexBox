import { motion, Transition, Variants } from "framer-motion";
import { ReactNode, useEffect, useState } from "react";

interface AnimatedPageProps {
  children: ReactNode;
}

export type TransitionMode = "slide" | "fade" | "off";

const STORAGE_KEY = "nexbox_page_transition";
const OLD_STORAGE_KEY = "nexbox_page_transition_enabled";
const EVENT_NAME = "page-transition-setting-changed";

export const slideVariants: Variants = {
  initial: { opacity: 0, x: 20 },
  enter: { opacity: 1, x: 0, transition: { duration: 0.3, ease: "easeOut" } },
  exit: { opacity: 0, x: -20, transition: { duration: 0.3, ease: "easeIn" } },
};

export const fadeVariants: Variants = {
  initial: { opacity: 0 },
  enter: { opacity: 1, transition: { duration: 0.3, ease: "easeOut" } },
  exit: { opacity: 0, transition: { duration: 0.2, ease: "easeIn" } },
};

export const slideTransition: Transition = {
  type: "tween",
  ease: "easeOut",
  duration: 0.3,
};

export const fadeTransition: Transition = {
  type: "tween",
  ease: "easeOut",
  duration: 0.3,
};

export function getVariants(mode: TransitionMode): Variants {
  switch (mode) {
    case "fade":
      return fadeVariants;
    case "slide":
    default:
      return slideVariants;
  }
}

export function getTransitionConfig(mode: TransitionMode): Transition {
  switch (mode) {
    case "fade":
      return fadeTransition;
    case "slide":
    default:
      return slideTransition;
  }
}

export function readTransitionMode(): TransitionMode {
  // migrate old key
  const oldVal = localStorage.getItem(OLD_STORAGE_KEY);
  if (oldVal !== null) {
    localStorage.removeItem(OLD_STORAGE_KEY);
    const mode: TransitionMode = oldVal === "true" ? "slide" : "off";
    localStorage.setItem(STORAGE_KEY, mode);
    return mode;
  }
  return (localStorage.getItem(STORAGE_KEY) as TransitionMode) || "fade";
}

export function useTransitionMode(): TransitionMode {
  const [mode, setMode] = useState<TransitionMode>(readTransitionMode);

  useEffect(() => {
    const handler = () => setMode(readTransitionMode());
    window.addEventListener(EVENT_NAME, handler);
    return () => window.removeEventListener(EVENT_NAME, handler);
  }, []);

  return mode;
}

export function AnimatedPage({ children }: AnimatedPageProps) {
  const mode = useTransitionMode();

  if (mode === "off") {
    return <div style={{ width: "100%", height: "100%" }}>{children}</div>;
  }

  return (
    <motion.div
      initial="initial"
      animate="enter"
      exit="exit"
      variants={getVariants(mode)}
      style={{ width: "100%", height: "100%" }}
    >
      {children}
    </motion.div>
  );
}
