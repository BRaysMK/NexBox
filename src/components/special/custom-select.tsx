import { Box, HStack, Text, Portal, useColorModeValue } from "@chakra-ui/react";
import { LuChevronDown, LuCheck } from "react-icons/lu";
import { useState, useRef, useEffect } from "react";

interface CustomSelectProps {
  value: string;
  onChange: (value: string) => void;
  options: { value: string; label: string }[];
  width?: string;
  placeholder?: string;
}

export function CustomSelect({ 
  value, 
  onChange, 
  options, 
  width = "140px",
  placeholder 
}: CustomSelectProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [dropdownPosition, setDropdownPosition] = useState({ top: 0, left: 0, width: 0 });
  const selectRef = useRef<HTMLDivElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  
  const bgColor = useColorModeValue("gray.50", "#111111");
  const borderColor = useColorModeValue("gray.300", "#333333");
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

  const updatePosition = () => {
    if (selectRef.current) {
      const rect = selectRef.current.getBoundingClientRect();
      setDropdownPosition({
        top: rect.bottom + 4,
        left: rect.left,
        width: rect.width,
      });
    }
  };

  const toggleSelect = () => {
    if (!isOpen) {
      updatePosition();
    }
    setIsOpen(!isOpen);
  };

  const selectedOption = options.find((opt) => opt.value === value);
  const displayLabel = selectedOption?.label || placeholder || "";

  return (
    <>
      <Box ref={selectRef} position="relative" w={width}>
        <Box
          bg={bgColor}
          border="1px solid"
          borderColor={borderColor}
          borderRadius="lg"
          px={3}
          py={1.5}
          cursor="pointer"
          onClick={toggleSelect}
          _hover={{ borderColor: "blue.400" }}
          transition="all 0.2s"
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
        </Box>
      </Box>

      {isOpen && (
        <Portal>
          <Box
            ref={dropdownRef}
            position="fixed"
            top={dropdownPosition.top}
            left={dropdownPosition.left}
            width={dropdownPosition.width}
            bg={dropdownBg}
            border="1px solid"
            borderColor={dropdownBorder}
            borderRadius="lg"
            boxShadow="2xl"
            zIndex={99999}
            overflow="hidden"
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
          </Box>
        </Portal>
      )}
    </>
  );
}
