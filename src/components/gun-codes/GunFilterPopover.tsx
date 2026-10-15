import {
  Button,
  Checkbox,
  Divider,
  HStack,
  Input,
  InputGroup,
  InputLeftElement,
  Popover,
  PopoverBody,
  PopoverContent,
  PopoverTrigger,
  Portal,
  Text,
  VStack,
} from "@chakra-ui/react";
import { ChevronDown, ListFilter, Search } from "lucide-react";
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { norm } from "@/lib/gunCodes";

/**
 * 枪名筛选：可搜索多选浮层
 *
 * 从 `GunCodesNotebook.tsx` 搬迁，**交互逻辑未改**：
 * 与分类筛选**可组合**（不再互斥重置）。
 */
export function GunFilterPopover({
  isOpen,
  onOpen,
  onClose,
  candidates,
  selected,
  onToggle,
  onClear,
  onBulk,
  textColor,
  subTextColor,
  bg,
  border,
}: {
  isOpen: boolean;
  onOpen: () => void;
  onClose: () => void;
  candidates: [string, number][];
  selected: string[];
  onToggle: (g: string) => void;
  onClear: () => void;
  onBulk: (guns: string[], on: boolean) => void;
  textColor: string;
  subTextColor: string;
  bg: string;
  border: string;
}) {
  const { t } = useTranslation();
  const [q, setQ] = useState("");
  const selSet = useMemo(() => new Set(selected.map(norm)), [selected]);
  const shown = useMemo(
    () => candidates.filter(([g]) => !q || norm(g).includes(norm(q))),
    [candidates, q],
  );

  return (
    <Popover
      placement="bottom-start"
      isLazy
      isOpen={isOpen}
      onOpen={onOpen}
      onClose={onClose}
      closeOnBlur
    >
      <PopoverTrigger>
        <Button
          size="sm"
          variant={selected.length ? "solid" : "outline"}
          colorScheme={selected.length ? "blue" : undefined}
          leftIcon={<ListFilter size={13} />}
          rightIcon={<ChevronDown size={12} />}
          flexShrink={0}
        >
          {selected.length
            ? t("gunCodes.gunFilterN", { n: selected.length })
            : t("gunCodes.gunFilter")}
        </Button>
      </PopoverTrigger>
      <Portal>
        <PopoverContent w="280px" bg={bg} borderColor={border}>
          <PopoverBody p={3}>
            <InputGroup size="sm" mb={2}>
              <InputLeftElement pointerEvents="none">
                <Search size={13} color={subTextColor} />
              </InputLeftElement>
              <Input
                value={q}
                onChange={(e) => setQ(e.target.value)}
                placeholder={t("gunCodes.searchGun")}
                bg={bg}
                color={textColor}
                borderColor={border}
              />
            </InputGroup>
            <HStack spacing={2} mb={2}>
              <Button
                size="xs"
                variant="ghost"
                onClick={() => {
                  const allOn = shown.length > 0 && shown.every(([g]) => selSet.has(norm(g)));
                  onBulk(shown.map(([g]) => g), !allOn);
                }}
              >
                {shown.length > 0 && shown.every(([g]) => selSet.has(norm(g)))
                  ? t("gunCodes.deselectAll")
                  : t("gunCodes.selectAll")}
              </Button>
              <Button size="xs" variant="ghost" onClick={onClear}>
                {t("gunCodes.clearSel")}
              </Button>
            </HStack>
            <Divider mb={2} />
            <VStack align="stretch" spacing={0} maxH="240px" overflowY="auto">
              {shown.length === 0 ? (
                <Text fontSize="xs" color={subTextColor} py={2} textAlign="center">
                  {t("gunCodes.noGun")}
                </Text>
              ) : (
                shown.map(([g, n]) => (
                  <HStack
                    key={g}
                    px={1}
                    py={1}
                    borderRadius="sm"
                    cursor="pointer"
                    _hover={{ bg: "blackAlpha.50" }}
                    onClick={() => onToggle(g)}
                    spacing={2}
                  >
                    <Checkbox isChecked={selSet.has(norm(g))} pointerEvents="none" size="sm" />
                    <Text fontSize="xs" color={textColor} flex={1} noOfLines={1}>
                      {g}
                    </Text>
                    <Text fontSize="2xs" color={subTextColor}>
                      {n}
                    </Text>
                  </HStack>
                ))
              )}
            </VStack>
          </PopoverBody>
        </PopoverContent>
      </Portal>
    </Popover>
  );
}
