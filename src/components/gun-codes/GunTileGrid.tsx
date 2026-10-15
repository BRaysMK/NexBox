import { Box, HStack, IconButton, SimpleGrid, Stack, Text } from "@chakra-ui/react";
import { Copy, Pencil, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { fmtTime, type GunCode } from "@/lib/gunCodes";
import { Highlight } from "./Highlight";

/**
 * 磁贴视图
 *
 * 适合「一屏看很多条」——每条一张小卡，展示枪名/备注/码。
 * 列数随宽度自适应（`minChildWidth`）。
 */
export function GunTileGrid({
  rows,
  query,
  highlightColor,
  onCopy,
  onEdit,
  onDelete,
  textColor,
  subTextColor,
  cardBg,
  cardBorder,
}: {
  rows: { item: GunCode; gun: string; cat: string }[];
  query: string;
  highlightColor: string;
  onCopy: (code: string) => void;
  onEdit: (item: GunCode) => void;
  onDelete: (item: GunCode) => void;
  textColor: string;
  subTextColor: string;
  cardBg: string;
  cardBorder: string;
}) {
  const { t } = useTranslation();

  return (
    <SimpleGrid columns={{ base: 1, md: 2, xl: 3 }} spacing={3} minChildWidth="240px">
      {rows.map(({ item, gun }) => (
        <Box
          key={item.id}
          bg={cardBg}
          border="1px solid"
          borderColor={cardBorder}
          borderRadius="lg"
          p={3}
          transition="border-color 0.15s, box-shadow 0.15s"
          _hover={{ borderColor: "blue.400", shadow: "sm" }}
        >
          <Stack spacing={2}>
            <HStack justify="space-between" align="start">
              <Text fontWeight="bold" fontSize="sm" color={textColor} noOfLines={1}>
                <Highlight text={gun} query={query} color={highlightColor} />
              </Text>
              <Text fontSize="xs" color={subTextColor} whiteSpace="nowrap">
                {fmtTime(item.created_at)}
              </Text>
            </HStack>

            <Text fontSize="xs" color={subTextColor} noOfLines={2} minH="2.4em">
              {item.note ? (
                <Highlight text={item.note} query={query} color={highlightColor} />
              ) : (
                t("gunCodes.noNote")
              )}
            </Text>

            <Box
              as="code"
              fontSize="xs"
              p={2}
              borderRadius="md"
              bg="blackAlpha.100"
              cursor="pointer"
              wordBreak="break-all"
              _hover={{ bg: "blackAlpha.200" }}
              onClick={() => onCopy(item.code)}
              title={t("gunCodes.clickToCopy")}
            >
              <Highlight text={item.code} query={query} color={highlightColor} />
            </Box>

            <HStack spacing={1} justify="flex-end">
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
          </Stack>
        </Box>
      ))}
    </SimpleGrid>
  );
}
