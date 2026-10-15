import { Button, HStack, Text, useColorModeValue } from "@chakra-ui/react";
import { Copy, Trash2, X } from "lucide-react";
import { useTranslation } from "react-i18next";

/**
 * 多选批量操作条
 *
 * 有选中项时出现在工具栏下方（sticky），显示已选数量与批量动作。
 */
export function SelectionBar({
  count,
  onCopyAll,
  onDelete,
  onClear,
}: {
  count: number;
  onCopyAll: () => void;
  onDelete: () => void;
  onClear: () => void;
}) {
  const { t } = useTranslation();
  const bg = useColorModeValue("blue.50", "#1c2a3a");
  const border = useColorModeValue("blue.200", "#2d4a6b");
  if (count === 0) return null;

  return (
    <HStack
      spacing={3}
      px={4}
      py={2}
      bg={bg}
      border="1px solid"
      borderColor={border}
      borderRadius="lg"
      w="100%"
    >
      <Text fontSize="sm" fontWeight="medium">
        {t("gunCodes.selectedCount", { n: count })}
      </Text>
      <HStack spacing={2} ml="auto">
        <Button size="xs" variant="outline" leftIcon={<Copy size={13} />} onClick={onCopyAll}>
          {t("gunCodes.bulkCopy")}
        </Button>
        <Button size="xs" colorScheme="red" variant="outline" leftIcon={<Trash2 size={13} />} onClick={onDelete}>
          {t("gunCodes.bulkDelete")}
        </Button>
        <Button size="xs" variant="ghost" leftIcon={<X size={13} />} onClick={onClear}>
          {t("gunCodes.clearSelection")}
        </Button>
      </HStack>
    </HStack>
  );
}
