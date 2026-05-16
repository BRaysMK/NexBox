import { motion } from "framer-motion";
import { ReactNode } from "react";

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
