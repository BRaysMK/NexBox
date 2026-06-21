import { motion } from "framer-motion";
import { ReactNode, useEffect, useState } from "react";

interface AnimatedPageProps {
  children: ReactNode;
}

const pageVariants = {
  initial: {
    opacity: 0,
    x: 20,
  },
  enter: {
    opacity: 1,
    x: 0,
    transition: {
      duration: 0.3,
      ease: "easeOut",
    },
  },
  exit: {
    opacity: 0,
    x: -20,
    transition: {
      duration: 0.3,
      ease: "easeIn",
    },
  },
};

export function AnimatedPage({ children }: AnimatedPageProps) {
  const [enabled, setEnabled] = useState(true);

  useEffect(() => {
    const stored = localStorage.getItem("nexbox_page_transition_enabled");
    if (stored !== null) {
      setEnabled(stored === "true");
    }

    const handler = () => {
      const updated = localStorage.getItem("nexbox_page_transition_enabled");
      if (updated !== null) {
        setEnabled(updated === "true");
      }
    };

    window.addEventListener("page-transition-setting-changed", handler);
    return () => window.removeEventListener("page-transition-setting-changed", handler);
  }, []);

  if (!enabled) {
    return <div style={{ width: "100%", height: "100%" }}>{children}</div>;
  }

  return (
    <motion.div
      initial="initial"
      animate="enter"
      exit="exit"
      variants={pageVariants}
      style={{ width: "100%", height: "100%" }}
    >
      {children}
    </motion.div>
  );
}
