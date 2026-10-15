import { Box, HStack, IconButton, Stack, Text, VStack } from "@chakra-ui/react";
import { ChevronDown, ChevronRight, Copy, Pencil, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { fmtTime, type GunCode } from "@/lib/gunCodes";
import { Highlight } from "./Highlight";

/**
 * 树视图（按枪分组，可折叠）
 *
 * 从 `GunCodesNotebook.tsx` 原样搬迁，**行为不变**：
 * 分组头显示枪名 / 分类 / 条数，点击折叠；组内每条一张卡。
 */
export function GunTreeView({
  grouped,
  collapsed,
  onToggleCollapse,
  query,
  highlightColor,
  onCopy,
  onEdit,
  onDelete,
  textColor,
  subTextColor,
  panelBg,
  cardBg,
  cardBorder,
}: {
  grouped: [string, GunCode[]][];
  collapsed: Set<string>;
  onToggleCollapse: (gun: string) => void;
  query: string;
  highlightColor: string;
  onCopy: (code: string) => void;
  onEdit: (item: GunCode) => void;
  onDelete: (item: GunCode) => void;
  textColor: string;
  subTextColor: string;
  panelBg: string;
  cardBg: string;
  cardBorder: string;
}) {
  const { t } = useTranslation();

  return (
    <VStack spacing={3} align="stretch">
      {grouped.map(([gun, gunItems]) => {
        const isCollapsed = collapsed.has(gun);
        return (
          <Box
            key={gun}
            bg={panelBg}
            borderRadius="lg"
            border="1px solid"
            borderColor={cardBorder}
            overflow="hidden"
          >
            <HStack
              justify="space-between"
              px={4}
              py={2}
              cursor="pointer"
              _hover={{ bg: "blackAlpha.50" }}
              onClick={() => onToggleCollapse(gun)}
            >
              <HStack spacing={2}>
                {isCollapsed ? <ChevronRight size={16} /> : <ChevronDown size={16} />}
                <Text fontWeight="bold" color={textColor}>
                  <Highlight text={gun} query={query} color={highlightColor} />
                </Text>
              </HStack>
              <Text fontSize="sm" color={subTextColor}>
                {t("gunCodes.configCount", { n: gunItems.length })}
              </Text>
            </HStack>

            {!isCollapsed && (
              <Stack spacing={2} px={4} pb={4}>
                {gunItems.map((item) => (
                  <Box
                    key={item.id}
                    bg={cardBg}
                    borderRadius="md"
                    border="1px solid"
                    borderColor={cardBorder}
                    p={3}
                  >
                    <HStack justify="space-between" mb={1}>
                      <Text fontSize="sm" color={subTextColor} noOfLines={1}>
                        {item.note ? (
                          <Highlight text={item.note} query={query} color={highlightColor} />
                        ) : (
                          t("gunCodes.noNote")
                        )}
                      </Text>
                      <Text fontSize="xs" color={subTextColor} whiteSpace="nowrap">
                        {fmtTime(item.created_at)}
                      </Text>
                    </HStack>

                    <Box
                      as="code"
                      fontSize="sm"
                      cursor="pointer"
                      _hover={{ textDecoration: "underline" }}
                      onClick={() => onCopy(item.code)}
                      title={t("gunCodes.clickToCopy")}
                      wordBreak="break-all"
                    >
                      <Highlight text={item.code} query={query} color={highlightColor} />
                    </Box>

                    <HStack spacing={1} justify="flex-end" mt={1}>
                      <IconButton
                        aria-label={t("gunCodes.copy")}
                        icon={<Copy size={13} />}
                        size="xs"
                        variant="ghost"
                        onClick={() => onCopy(item.code)}
                      />
                      <IconButton
                        aria-label={t("gunCodes.edit")}
                        icon={<Pencil size={13} />}
                        size="xs"
                        variant="ghost"
                        onClick={() => onEdit(item)}
                      />
                      <IconButton
                        aria-label={t("gunCodes.delete")}
                        icon={<Trash2 size={13} />}
                        size="xs"
                        variant="ghost"
                        colorScheme="red"
                        onClick={() => onDelete(item)}
                      />
                    </HStack>
                  </Box>
                ))}
              </Stack>
            )}
          </Box>
        );
      })}
    </VStack>
  );
}
