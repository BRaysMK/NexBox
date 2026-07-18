import { Box, Image, useColorModeValue, Flex } from "@chakra-ui/react";
import { useState, useEffect, useCallback, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";

interface CarouselImage {
  src: string;
  alt: string;
  url: string;
}

const carouselImages: CarouselImage[] = [
  {
    src: "/kuaikuai.png",
    alt: "Kuaikuai",
    url: "http://www.kkidc.com/i/U84px5s6v",
  },
];

export function useFeedbackEnabled() {
  const [enabled, setEnabled] = useState(true);

  useEffect(() => {
    const saved = localStorage.getItem("nexbox_feedback_enabled");
    if (saved !== null) {
      setEnabled(saved === "true");
    }
  }, []);

  useEffect(() => {
    const handler = (e: CustomEvent) => setEnabled(e.detail);
    window.addEventListener("feedback-setting-changed", handler as EventListener);
    return () => window.removeEventListener("feedback-setting-changed", handler as EventListener);
  }, []);

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
    }, 4000);
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
    const { open } = await import("@tauri-apps/plugin-shell");
    open(images[currentIndex].url);
  };

  const goTo = (index: number) => {
    setDirection(index > currentIndex ? 1 : -1);
    setCurrentIndex(index);
    if (intervalRef.current) clearInterval(intervalRef.current);
    if (!isPaused && images.length > 1) {
      intervalRef.current = setInterval(() => {
        setDirection(1);
        setCurrentIndex((prev) => (prev + 1) % images.length);
      }, 4000);
    }
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
      maxW="360px"
      _hover={{ borderColor: "purple.500" }}
      transition="border-color 0.2s"
      onMouseEnter={() => setIsPaused(true)}
      onMouseLeave={() => setIsPaused(false)}
    >
      <Box position="relative" onClick={handleClick}>
        <AnimatePresence mode="wait" custom={direction}>
          <motion.div
            key={currentIndex}
            custom={direction}
            variants={variants}
            initial="enter"
            animate="center"
            exit="exit"
            transition={{ duration: 0.3, ease: "easeInOut" }}
          >
            <Image
              src={images[currentIndex].src}
              alt={images[currentIndex].alt}
              w="100%"
              objectFit="contain"
              display="block"
            />
          </motion.div>
        </AnimatePresence>
      </Box>

      {images.length > 1 && (
        <Flex justify="center" gap={2} py={2}>
          {images.map((_, index) => (
            <Box
              key={index}
              w={2}
              h={2}
              borderRadius="full"
              bg={index === currentIndex ? "purple.500" : "gray.500"}
              opacity={index === currentIndex ? 1 : 0.5}
              cursor="pointer"
              onClick={(e) => {
                e.stopPropagation();
                goTo(index);
              }}
              transition="all 0.2s"
            />
          ))}
        </Flex>
      )}
    </Box>
  );
}
