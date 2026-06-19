import {
  Box,
  HStack,
  Text,
  VStack,
  Input,
  Portal,
  useColorModeValue,
  useOutsideClick,
} from "@chakra-ui/react";
import { useState, useRef, useEffect, useCallback } from "react";
import { isValidHexColor, hexToHsv, hsvToHex } from "@/lib/color-utils";

const SV_PANEL_W = 200;
const SV_PANEL_H = 150;

export function CustomColorPicker({ color, onChange, compact }: { color: string; onChange: (c: string) => void; compact?: boolean }) {
  const [isOpen, setIsOpen] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLDivElement>(null);
  const [popoverPos, setPopoverPos] = useState({ top: 0, left: 0 });
  const svRef = useRef<HTMLDivElement>(null);
  const hueRef = useRef<HTMLDivElement>(null);
  const isDragging = useRef<"sv" | "hue" | null>(null);

  const hsv = hexToHsv(color);
  const hueValueRef = useRef(hsv.h);
  hueValueRef.current = hsv.h;
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const panelBg = useColorModeValue("white", "#1a1a1a");
  const panelShadow = useColorModeValue("0 4px 20px rgba(0,0,0,0.15)", "0 4px 20px rgba(0,0,0,0.4)");
  const [hexInput, setHexInput] = useState(color);

  useEffect(() => { setHexInput(color); }, [color]);

  useOutsideClick({ ref: panelRef, handler: () => setIsOpen(false) });

  const openPicker = () => {
    if (triggerRef.current) {
      const rect = triggerRef.current.getBoundingClientRect();
      setPopoverPos({ top: rect.bottom + 8, left: rect.left });
    }
    setIsOpen(true);
  };

  // Follow trigger position on scroll/resize when open
  useEffect(() => {
    if (!isOpen || !triggerRef.current) return;
    const updatePos = () => {
      if (triggerRef.current) {
        const rect = triggerRef.current.getBoundingClientRect();
        setPopoverPos({ top: rect.bottom + 8, left: rect.left });
      }
    };
    window.addEventListener("scroll", updatePos, true);
    window.addEventListener("resize", updatePos);
    return () => {
      window.removeEventListener("scroll", updatePos, true);
      window.removeEventListener("resize", updatePos);
    };
  }, [isOpen]);

  // SV panel: 2D saturation-value picker
  const handleSvMouseDown = useCallback((e: React.MouseEvent) => {
    isDragging.current = "sv";
    updateSvFromMouse(e);
    const onMove = (ev: MouseEvent) => updateSvFromMouse(ev);
    const onUp = () => { isDragging.current = null; document.removeEventListener("mousemove", onMove); document.removeEventListener("mouseup", onUp); };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  }, [hsv.h]);

  const updateSvFromMouse = (e: { clientX: number; clientY: number }) => {
    const rect = svRef.current?.getBoundingClientRect();
    if (!rect) return;
    let x = (e.clientX - rect.left) / rect.width;
    let y = (e.clientY - rect.top) / rect.height;
    x = Math.max(0, Math.min(1, x));
    y = Math.max(0, Math.min(1, y));
    const s = x * 100;
    const v = (1 - y) * 100;
    const newHex = hsvToHex(hueValueRef.current, s, v);
    onChange(newHex);
  };

  // Hue slider
  const handleHueMouseDown = useCallback((e: React.MouseEvent) => {
    isDragging.current = "hue";
    updateHueFromMouse(e);
    const onMove = (ev: MouseEvent) => updateHueFromMouse(ev);
    const onUp = () => { isDragging.current = null; document.removeEventListener("mousemove", onMove); document.removeEventListener("mouseup", onUp); };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  }, []);

  const updateHueFromMouse = (e: { clientX: number }) => {
    const rect = hueRef.current?.getBoundingClientRect();
    if (!rect) return;
    let x = (e.clientX - rect.left) / rect.width;
    x = Math.max(0, Math.min(1, x));
    const newHex = hsvToHex(x * 360, hsv.s, hsv.v);
    onChange(newHex);
  };

  const handleHexInputChange = (value: string) => {
    setHexInput(value);
    if (isValidHexColor(value)) {
      onChange(value);
    }
  };

  const cursorX = hsv.s / 100 * SV_PANEL_W;
  const cursorY = (1 - hsv.v / 100) * SV_PANEL_H;
  const huePos = hsv.h / 360 * SV_PANEL_W;

  return (
    <Box position="relative" ref={panelRef}>
      {/* Trigger swatch */}
      <HStack spacing={compact ? 0 : 3}>
        <Box
          ref={triggerRef}
          w={compact ? "32px" : "40px"} h={compact ? "32px" : "40px"}
          borderRadius="md"
          bg={color}
          border="1px solid"
          borderColor={cardBorder}
          cursor="pointer"
          onClick={openPicker}
          transition="box-shadow 0.2s"
          _hover={{ boxShadow: `0 0 0 2px ${color}40` }}
        />
        {!compact && (
          <Input
            value={hexInput}
            onChange={(e) => handleHexInputChange(e.target.value)}
            placeholder="#98DDD0"
            size="sm"
            width="120px"
            borderRadius="lg"
          />
        )}
      </HStack>

      {/* Picker panel via Portal (fixed positioning to avoid card clipping) */}
      {isOpen && (
        <Portal>
          <Box
            position="fixed"
            top={`${popoverPos.top}px`}
            left={`${popoverPos.left}px`}
            zIndex={999}
            bg={panelBg}
            borderRadius="xl"
            boxShadow={panelShadow}
            border="1px solid"
            borderColor={cardBorder}
            p={3}
            w={`${SV_PANEL_W + 24}px`}
          >
            <VStack spacing={3} align="stretch">
              {/* SV square */}
              <Box
                ref={svRef}
                w={`${SV_PANEL_W}px`}
                h={`${SV_PANEL_H}px`}
                borderRadius="md"
                position="relative"
                cursor="crosshair"
                onMouseDown={handleSvMouseDown}
                sx={{
                  background: `linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, hsl(${hsv.h}, 100%, 50%))`,
                }}
              >
                {/* Cursor */}
                <Box
                  position="absolute"
                  left={`${cursorX - 6}px`}
                  top={`${cursorY - 6}px`}
                  w="12px" h="12px"
                  borderRadius="full"
                  border="2px solid white"
                  boxShadow="0 0 3px rgba(0,0,0,0.5)"
                  pointerEvents="none"
                />
              </Box>

              {/* Hue slider */}
              <Box
                ref={hueRef}
                w={`${SV_PANEL_W}px`}
                h="14px"
                borderRadius="full"
                position="relative"
                cursor="pointer"
                onMouseDown={handleHueMouseDown}
                sx={{
                  background: "linear-gradient(to right, #f00 0%, #ff0 17%, #0f0 33%, #0ff 50%, #00f 67%, #f0f 83%, #f00 100%)",
                }}
              >
                <Box
                  position="absolute"
                  left={`${huePos - 5}px`}
                  top="-3px"
                  w="10px" h="20px"
                  borderRadius="sm"
                  border="2px solid white"
                  boxShadow="0 0 3px rgba(0,0,0,0.5)"
                  pointerEvents="none"
                />
              </Box>

              {/* Preview + hex */}
              <HStack spacing={2}>
                <Box w="24px" h="24px" borderRadius="md" bg={color} flexShrink={0} />
                <Text fontSize="xs" fontWeight="semibold" letterSpacing="0.03em" color={useColorModeValue("gray.700", "#e0e0e0")}>
                  {color.toUpperCase()}
                </Text>
              </HStack>
            </VStack>
          </Box>
        </Portal>
      )}
    </Box>
  );
}
