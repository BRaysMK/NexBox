import { useState, useEffect } from "react";
import {
  Modal,
  ModalOverlay,
  ModalContent,
  ModalHeader,
  ModalBody,
  ModalFooter,
  Button,
  useColorModeValue,
  Text,
  VStack,
  Badge,
  HStack,
} from "@chakra-ui/react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { LiquidGlassButton } from "@/components/special/liquid-glass-button";

interface Announcement {
  title: string;
  content: string;
  important: boolean;
  create_time: string;
}

interface AnnouncementResponse {
  version: number;
  announce_list: Announcement[];
}

function getAnnouncementKey(announcement: Announcement): string {
  return `${announcement.title}_${announcement.create_time}`;
}

export function ImportantAnnouncementModal() {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const [currentAnnouncement, setCurrentAnnouncement] = useState<Announcement | null>(null);

  const labelColor = useColorModeValue("gray.700", "#e0e0e0");
  const subLabelColor = useColorModeValue("gray.500", "#888888");
  const modalBg = useColorModeValue("white", "#1a1a1a");
  const modalBorderColor = useColorModeValue("gray.200", "#333333");

  useEffect(() => {
    const checkImportantAnnouncement = async () => {
      try {
        const response = await invoke<AnnouncementResponse>("get_announcements");
        const importantAnnouncements = response.announce_list.filter(a => a.important);

        if (importantAnnouncements.length === 0) return;

        const confirmedKeys = JSON.parse(
          localStorage.getItem("nexbox_confirmed_announcements") || "[]"
        ) as string[];

        const unconfirmed = importantAnnouncements.find(
          a => !confirmedKeys.includes(getAnnouncementKey(a))
        );

        if (unconfirmed) {
          setCurrentAnnouncement(unconfirmed);
          setIsOpen(true);
        }
      } catch (e) {
        console.error("Failed to check important announcements:", e);
      }
    };

    const timer = setTimeout(checkImportantAnnouncement, 1500);
    return () => clearTimeout(timer);
  }, []);

  const handleConfirm = () => {
    if (!currentAnnouncement) return;

    const confirmedKeys = JSON.parse(
      localStorage.getItem("nexbox_confirmed_announcements") || "[]"
    ) as string[];

    confirmedKeys.push(getAnnouncementKey(currentAnnouncement));
    localStorage.setItem("nexbox_confirmed_announcements", JSON.stringify(confirmedKeys));

    setIsOpen(false);
    setCurrentAnnouncement(null);
  };

  if (!currentAnnouncement) return null;

  return (
    <Modal
      isOpen={isOpen}
      onClose={() => {}}
      isCentered
      closeOnOverlayClick={false}
      closeOnEsc={false}
    >
      <ModalOverlay />
      <ModalContent bg={modalBg} borderColor={modalBorderColor} borderRadius="xl">
        <ModalHeader color={labelColor}>
          <VStack align="start" spacing={1}>
            <HStack>
              <Text>{t("home.importantAnnouncement")}</Text>
              <Badge colorScheme="red">{t("home.important")}</Badge>
            </HStack>
            <Text fontSize="sm" color={subLabelColor} fontWeight="normal">
              {currentAnnouncement.create_time}
            </Text>
          </VStack>
        </ModalHeader>
        <ModalBody>
          <VStack align="start" spacing={3}>
            <Text color={labelColor} fontWeight="bold" fontSize="lg">
              {currentAnnouncement.title}
            </Text>
            <Text color={subLabelColor} whiteSpace="pre-wrap">
              {currentAnnouncement.content}
            </Text>
          </VStack>
        </ModalBody>
        <ModalFooter>
          <LiquidGlassButton colorScheme="purple" onClick={handleConfirm}>
            {t("home.confirmAnnouncement")}
          </LiquidGlassButton>
        </ModalFooter>
      </ModalContent>
    </Modal>
  );
}
