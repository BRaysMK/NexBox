import { Box, Image, useColorModeValue } from "@chakra-ui/react";
import { useState, useEffect } from "react";

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

  const handleClick = async () => {
    const { open } = await import("@tauri-apps/plugin-shell");
    open("https://nexbox.top/feedback");
  };

  return (
    <Box
      borderRadius="xl"
      overflow="hidden"
      border="1px solid"
      borderColor={borderColor}
      bg={cardBg}
      cursor="pointer"
      onClick={handleClick}
      maxW="360px"
      _hover={{ borderColor: "purple.500" }}
      transition="border-color 0.2s"
    >
      <Image
        src="/feedback-banner.png"
        alt="反馈"
        w="100%"
        objectFit="contain"
        display="block"
      />
    </Box>
  );
}
