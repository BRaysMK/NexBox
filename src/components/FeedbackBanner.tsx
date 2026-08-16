import { Box, Image, useColorModeValue, Flex } from "@chakra-ui/react";
import { useState, useEffect, useCallback, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { store } from "@/lib/store";
import { initVisibility, subscribeVisibility } from "@/lib/ui-visibility";

interface CarouselImage {
  src: string;
  alt: string;
  url: string;
}

const carouselImages: CarouselImage[] = [
  {
    src: "/qq.png",
    alt: "QQ Group",
    url: "https://qm.qq.com/q/atlGEA2tQk",
  },
  {
    src: "/feedback-banner.png",
    alt: "Feedback",
    url: "https://nexbox.top/feedback",
  },
];

export function useFeedbackEnabled() {
  const [enabled, setEnabled] = useState(() => initVisibility("nexbox_feedback_enabled"));

  useEffect(() =>
    subscribeVisibility("nexbox_feedback_enabled", "feedback-setting-changed", setEnabled, store),
  []);

  return enabled;
}

export function FeedbackBanner() {
  const borderColor = useColorModeValue("gray.200", "#333333");
  const cardBg = useColorModeValue("white", "#111111");
  const [currentIndex, setCurrentIndex] = useState(0);
  const [isPaused, setIsPaused] = useState(false);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const [direction, setDirection] = useState(1);

  const images = carouselImages;

  const startAutoPlay = useCallback(() => {
    if (intervalRef.current) clearInterval(intervalRef.current);
    intervalRef.current = setInterval(() => {
      setDirection(1);
      setCurrentIndex((prev) => (prev + 1) % images.length);
    }, 2000);
  }, [images.length]);

  useEffect(() => {
    if (!isPaused && images.length > 1) {
      startAutoPlay();
    }
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [isPaused, images.length, startAutoPlay]);

  const handleClick = async () => {
    const url = images[currentIndex].url;
    if (!url) return;
    const { open } = await import("@tauri-apps/plugin-shell");
    open(url);
  };

  const variants = {
    enter: (dir: number) => ({ x: dir > 0 ? 300 : -300, opacity: 0 }),
    center: { x: 0, opacity: 1 },
    exit: (dir: number) => ({ x: dir > 0 ? -300 : 300, opacity: 0 }),
  };

  return (
    <Box
      borderRadius="xl"
      overflow="hidden"
      border="1px solid"
      borderColor={borderColor}
      bg={cardBg}
      cursor="pointer"
      w="360px"
      h="200px"
      _hover={{ borderColor: "purple.500" }}
      transition="border-color 0.2s"
      onMouseEnter={() => setIsPaused(true)}
      onMouseLeave={() => setIsPaused(false)}
    >
      <Flex
        w="full"
        h="full"
        align="center"
        justify="center"
        position="relative"
        onClick={handleClick}
      >
        <AnimatePresence mode="wait" custom={direction}>
          <motion.div
            key={currentIndex}
            custom={direction}
            variants={variants}
            initial="enter"
            animate="center"
            exit="exit"
            transition={{ duration: 0.3, ease: "easeInOut" }}
            style={{ width: "100%", height: "100%", display: "flex", alignItems: "center", justifyContent: "center", backgroundColor: cardBg }}
          >
            <Image
              src={images[currentIndex].src}
              alt={images[currentIndex].alt}
              maxW="100%"
              maxH="100%"
              objectFit="contain"
              display="block"
            />
          </motion.div>
        </AnimatePresence>
      </Flex>
    </Box>
  );
}
