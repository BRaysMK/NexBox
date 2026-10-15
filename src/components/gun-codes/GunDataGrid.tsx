import {
  Badge,
  Box,
  Checkbox,
  HStack,
  IconButton,
  Table,
  TableContainer,
  Tbody,
  Td,
  Text,
  Th,
  Thead,
  Tr,
} from "@chakra-ui/react";
import { Copy, Pencil, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { fmtTime, type GunCode } from "@/lib/gunCodes";
import { Highlight } from "./Highlight";

export interface RowEntry {
  item: GunCode;
  gun: string;
  cat: string;
}

/**
 * 表格视图
 *
 * 适合快速浏览与批量操作：一行一条配置，支持表头全选。
 * 窄窗口（`compact`）下隐藏「时间」「分类」两列。
 */
export function GunDataGrid({
  rows,
  query,
  highlightColor,
  selectedIds,
  onToggleSelect,
  onToggleAll,
  onCopy,
  onEdit,
  onDelete,
  compact,
  textColor,
  subTextColor,
  cardBorder,
}: {
  rows: RowEntry[];
  query: string;
  highlightColor: string;
  selectedIds: Set<string>;
  onToggleSelect: (id: string) => void;
  onToggleAll: (on: boolean) => void;
  onCopy: (code: string) => void;
  onEdit: (item: GunCode) => void;
  onDelete: (item: GunCode) => void;
  compact: boolean;
  textColor: string;
  subTextColor: string;
  cardBorder: string;
}) {
  const { t } = useTranslation();
  const allOn = rows.length > 0 && rows.every((r) => selectedIds.has(r.item.id));
  const someOn = rows.some((r) => selectedIds.has(r.item.id));

  return (
    <TableContainer border="1px solid" borderColor={cardBorder} borderRadius="lg">
      <Table size="sm" variant="simple">
        <Thead>
          <Tr>
            <Th w="40px" px={3}>
              <Checkbox
                isChecked={allOn}
                isIndeterminate={!allOn && someOn}
                onChange={(e) => onToggleAll(e.target.checked)}
                aria-label={t("gunCodes.selectAll")}
              />
            </Th>
            <Th>{t("gunCodes.colGun")}</Th>
            {!compact && <Th>{t("gunCodes.colCategory")}</Th>}
            <Th>{t("gunCodes.colNote")}</Th>
            <Th>{t("gunCodes.colCode")}</Th>
            {!compact && <Th>{t("gunCodes.colTime")}</Th>}
            <Th w="120px" textAlign="end">
              {t("gunCodes.colActions")}
            </Th>
          </Tr>
        </Thead>
        <Tbody>
          {rows.map(({ item, gun, cat }) => (
            <Tr key={item.id} _hover={{ bg: "blackAlpha.50" }}>
              <Td px={3}>
                <Checkbox
                  isChecked={selectedIds.has(item.id)}
                  onChange={() => onToggleSelect(item.id)}
                  aria-label={t("gunCodes.selectRow")}
                />
              </Td>
              <Td>
                <Text fontWeight="medium" color={textColor} noOfLines={1}>
                  <Highlight text={gun} query={query} color={highlightColor} />
                </Text>
              </Td>
              {!compact && (
                <Td>
                  {cat && cat !== "其他" ? (
                    <Badge variant="subtle" fontSize="xs">
                      {cat}
                    </Badge>
                  ) : (
                    <Text fontSize="xs" color={subTextColor}>
                      —
                    </Text>
                  )}
                </Td>
              )}
              <Td maxW="200px">
                <Text fontSize="sm" color={subTextColor} noOfLines={1}>
                  {item.note ? (
                    <Highlight text={item.note} query={query} color={highlightColor} />
                  ) : (
                    t("gunCodes.noNote")
                  )}
                </Text>
              </Td>
              <Td>
                <Box
                  as="code"
                  fontSize="xs"
                  cursor="pointer"
                  _hover={{ textDecoration: "underline" }}
                  onClick={() => onCopy(item.code)}
                  title={t("gunCodes.clickToCopy")}
                >
                  <Highlight text={item.code} query={query} color={highlightColor} />
                </Box>
              </Td>
              {!compact && (
                <Td>
                  <Text fontSize="xs" color={subTextColor} whiteSpace="nowrap">
                    {fmtTime(item.created_at)}
                  </Text>
                </Td>
              )}
              <Td textAlign="end">
                <HStack spacing={1} justify="flex-end">
                  <IconButton
                    aria-label={t("gunCodes.copy")}
                    icon={<Copy size={14} />}
                    size="xs"
                    variant="ghost"
                    onClick={() => onCopy(item.code)}
                  />
                  <IconButton
                    aria-label={t("gunCodes.edit")}
                    icon={<Pencil size={14} />}
                    size="xs"
                    variant="ghost"
                    onClick={() => onEdit(item)}
                  />
                  <IconButton
                    aria-label={t("gunCodes.delete")}
                    icon={<Trash2 size={14} />}
                    size="xs"
                    variant="ghost"
                    colorScheme="red"
                    onClick={() => onDelete(item)}
                  />
                </HStack>
              </Td>
            </Tr>
          ))}
        </Tbody>
      </Table>
    </TableContainer>
  );
}
