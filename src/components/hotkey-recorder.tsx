"use client";

import { Box, Text, useColorModeValue } from "@chakra-ui/react";
import { useState, useCallback, useRef } from "react";

export function keyToHotkeyFormat(key: string): string | null {
  if (key.startsWith("F") && key.length >= 2 && key.length <= 3) {
    const num = parseInt(key.slice(1));
    if (num >= 1 && num <= 24) return key;
  }
  if (key === "ArrowUp") return "Up";
  if (key === "ArrowDown") return "Down";
  if (key === "ArrowLeft") return "Left";
  if (key === "ArrowRight") return "Right";
  if (key === " ") return "Space";
  if (key === "Escape") return "Escape";
  if (key === "Tab") return "Tab";
  if (key === "Enter") return "Enter";
  if (key === "Backspace") return "Backspace";
  if (key === "Delete") return "Delete";
  if (key === "Home") return "Home";
  if (key === "End") return "End";
  if (key === "PageUp") return "PageUp";
  if (key === "PageDown") return "PageDown";
  if (key === "Insert") return "Insert";
  if (key === "Pause") return "Pause";
  if (key === "ScrollLock") return "ScrollLock";
  if (key === "CapsLock") return "CapsLock";
  if (key === "NumLock") return "NumLock";
  if (key === "PrintScreen") return "PrintScreen";
  if (/^[0-9]$/.test(key)) return key;
  if (/^[a-zA-Z]$/.test(key)) return key.toUpperCase();
  return null;
}

function buildComboFromEvent(e: React.KeyboardEvent): string[] {
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.shiftKey) parts.push("Shift");
  if (e.altKey) parts.push("Alt");
  if (e.metaKey) parts.push("Command");
  const nonModifier = e.key;
  if (!["Control", "Shift", "Alt", "Meta"].includes(nonModifier)) {
    const mapped = keyToHotkeyFormat(nonModifier);
    if (mapped) {
      parts.push(mapped);
    }
  }
  return parts;
}

const MODIFIER_LABELS = ["Ctrl", "Shift", "Alt", "Command"];

export function HotkeyRecorder({
  value,
  onChange,
}: {
  value: string;
  onChange: (val: string) => void;
}) {
  const [isRecording, setIsRecording] = useState(false);
  const [displayText, setDisplayText] = useState("");
  const pendingRef = useRef<string[]>([]);
  const textColor = useColorModeValue("gray.800", "#e0e0e0");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const recordBg = useColorModeValue("teal.50", "rgba(0,150,136,0.1)");
  const recordBorder = useColorModeValue("teal.400", "teal.300");

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (!isRecording) return;
      e.preventDefault();
      e.stopPropagation();
      const parts = buildComboFromEvent(e);
      pendingRef.current = parts;
      setDisplayText(parts.join("+"));
    },
    [isRecording]
  );

  const handleKeyUp = useCallback(
    (e: React.KeyboardEvent) => {
      if (!isRecording) return;
      e.preventDefault();
      e.stopPropagation();

      if (e.key === "Escape") {
        setIsRecording(false);
        setDisplayText("");
        pendingRef.current = [];
        return;
      }

      const combo = pendingRef.current;
      if (combo.length > 0) {
        const lastPart = combo[combo.length - 1];
        const hasMainKey = !MODIFIER_LABELS.includes(lastPart);
        if (hasMainKey) {
          onChange(combo.join("+"));
          setIsRecording(false);
          setDisplayText("");
          pendingRef.current = [];
          return;
        }
      }

      const remainingParts = buildComboFromEvent(e);
      pendingRef.current = remainingParts;
      setDisplayText(remainingParts.join("+"));
    },
    [isRecording, onChange]
  );

  const startRecording = useCallback(() => {
    setIsRecording(true);
    setDisplayText("");
    pendingRef.current = [];
  }, []);

  const stopRecording = useCallback(() => {
    if (isRecording) {
      setIsRecording(false);
      setDisplayText("");
      pendingRef.current = [];
    }
  }, [isRecording]);

  return (
    <Box
      tabIndex={0}
      role="button"
      cursor="pointer"
      onKeyDown={handleKeyDown}
      onKeyUp={handleKeyUp}
      onClick={startRecording}
      onBlur={stopRecording}
      px={3}
      py={2}
      borderRadius="lg"
      border="2px solid"
      borderColor={isRecording ? recordBorder : borderColor}
      bg={isRecording ? recordBg : "transparent"}
      transition="all 0.2s"
      _hover={{ borderColor: recordBorder }}
      outline="none"
      minW="180px"
      textAlign="center"
      userSelect="none"
    >
      {isRecording ? (
        <Text color="teal.400" fontSize="sm" fontWeight="medium">
          {displayText || "按下快捷键..."}
        </Text>
      ) : (
        <Text color={textColor} fontSize="sm" fontWeight="medium">
          {value || "无"}
        </Text>
      )}
    </Box>
  );
}
