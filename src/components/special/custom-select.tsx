import { Box, HStack, Text, useColorModeValue } from "@chakra-ui/react";
import { LuChevronDown, LuCheck } from "react-icons/lu";
import { useState, useRef, useEffect, useCallback } from "react";
import { createPortal } from "react-dom";
import { LiquidGlassCard } from "./liquid-glass-card";

interface CustomSelectProps {
  value: string;
  onChange: (value: string) => void;
  options: { value: string; label: string }[];
  width?: string;
  placeholder?: string;
  direction?: "up" | "down";
}

export function CustomSelect({ 
  value, 
  onChange, 
  options, 
  width = "140px",
  placeholder,
  direction = "down"
}: CustomSelectProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [dropdownPos, setDropdownPos] = useState<{ top?: number; bottom?: number; left: number; width: number } | null>(null);
  const selectRef = useRef<HTMLDivElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const scrollTopRef = useRef(0);
  
  const textColor = useColorModeValue("gray.700", "#e0e0e0");
  const iconColor = useColorModeValue("gray.500", "#999999");
  const dropdownBg = useColorModeValue("white", "#111111");
  const dropdownBorder = useColorModeValue("gray.200", "#333333");
  const itemBg = useColorModeValue("white", "#111111");
  const itemBgActive = useColorModeValue("gray.100", "#222222");
  const itemText = useColorModeValue("gray.600", "#cccccc");
  const itemTextActive = useColorModeValue("gray.900", "#ffffff");
  const hoverBg = useColorModeValue("gray.50", "#222222");

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Node;
      const isClickInsideSelect = selectRef.current?.contains(target);
      const isClickInsideDropdown = dropdownRef.current?.contains(target);
      
      if (!isClickInsideSelect && !isClickInsideDropdown) {
        setIsOpen(false);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  // Set initial position when opening
  useEffect(() => {
    if (isOpen && selectRef.current) {
      const rect = selectRef.current.getBoundingClientRect();
      if (direction === "up") {
        setDropdownPos({
          bottom: window.innerHeight - rect.top + 4,
          left: rect.left,
          width: rect.width,
        });
      } else {
        setDropdownPos({
          top: rect.bottom + 4,
          left: rect.left,
          width: rect.width,
        });
      }
    } else {
      setDropdownPos(null);
    }
  }, [isOpen, direction]);

  // Follow scroll container via direct DOM manipulation (no re-render)
  useEffect(() => {
    if (!isOpen || !selectRef.current) return;

    // Find nearest scrollable ancestor
    let scrollContainer: HTMLElement | null = selectRef.current.parentElement;
    while (scrollContainer) {
      const style = window.getComputedStyle(scrollContainer);
      if (style.overflowY === "auto" || style.overflowY === "scroll") {
        break;
      }
      scrollContainer = scrollContainer.parentElement;
    }
    if (!scrollContainer) return;

    scrollTopRef.current = scrollContainer.scrollTop;

    const onScroll = () => {
      const el = dropdownRef.current;
      if (!el || !scrollContainer) return;
      const offset = scrollTopRef.current - scrollContainer.scrollTop;
      el.style.transform = `translateY(${offset}px)`;
    };

    const onResize = () => {
      const el = dropdownRef.current;
      if (!el || !selectRef.current || !scrollContainer) return;
      const rect = selectRef.current.getBoundingClientRect();
      el.style.transform = "";
      scrollTopRef.current = scrollContainer.scrollTop;
      if (direction === "up") {
        el.style.bottom = `${window.innerHeight - rect.top + 4}px`;
        el.style.top = "";
      } else {
        el.style.top = `${rect.bottom + 4}px`;
        el.style.bottom = "";
      }
      el.style.left = `${rect.left}px`;
      el.style.width = `${rect.width}px`;
    };

    scrollContainer.addEventListener("scroll", onScroll);
    window.addEventListener("resize", onResize);
    return () => {
      scrollContainer?.removeEventListener("scroll", onScroll);
      window.removeEventListener("resize", onResize);
    };
  }, [isOpen, direction]);

  const toggleSelect = useCallback(() => {
    setIsOpen((prev) => !prev);
  }, []);

  const selectedOption = options.find((opt) => opt.value === value);
  const displayLabel = selectedOption?.label || placeholder || "";

  return (
    <>
      <Box ref={selectRef} w={width}>
        <LiquidGlassCard
          px={3}
          py={1.5}
          cursor="pointer"
          onClick={toggleSelect}
        >
          <HStack justify="space-between">
            <Text fontSize="sm" color={textColor}>
              {displayLabel}
            </Text>
            <LuChevronDown
              size={14}
              color={iconColor}
              style={{
                transform: isOpen ? "rotate(180deg)" : "rotate(0deg)",
                transition: "transform 0.2s",
              }}
            />
          </HStack>
        </LiquidGlassCard>
      </Box>

      {isOpen && dropdownPos && createPortal(
          <Box
            ref={dropdownRef}
            position="fixed"
            top={dropdownPos.top}
            bottom={dropdownPos.bottom}
            left={dropdownPos.left}
            width={`${dropdownPos.width}px`}
            bg={dropdownBg}
            border="1px solid"
            borderColor={dropdownBorder}
            borderRadius="lg"
            boxShadow="2xl"
            zIndex={99999}
            maxH="280px"
            overflowY="auto"
          >
            {options.map((option) => (
              <Box
                key={option.value}
                px={3}
                py={2}
                cursor="pointer"
                bg={option.value === value ? itemBgActive : itemBg}
                color={option.value === value ? itemTextActive : itemText}
                _hover={{ bg: hoverBg }}
                onClick={() => {
                  onChange(option.value);
                  setIsOpen(false);
                }}
                transition="all 0.15s"
              >
                <HStack justify="space-between">
                  <Text fontSize="sm">{option.label}</Text>
                  {option.value === value && <LuCheck size={14} />}
                </HStack>
              </Box>
            ))}
          </Box>,
          document.body
        )}
    </>
  );
}