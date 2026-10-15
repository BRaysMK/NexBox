import { HStack, Text, useColorModeValue } from "@chakra-ui/react";
import { useTranslation } from "react-i18next";
import type { RulesMeta } from "@/lib/gunCodes";

/**
 * 状态栏：底部常驻，显示筛选摘要与词库版本
 *
 * 让「当前看的是哪一批数据」始终可见（尤其筛选后条数变化时）。
 */
export function StatusBar({
  summary,
  meta,
}: {
  summary: { total: number; shown: number; guns: number; cat: string; gunCount: number };
  meta: RulesMeta | null;
}) {
  const { t } = useTranslation();
  const subTextColor = useColorModeValue("gray.500", "gray.400");
  const borderColor = useColorModeValue("gray.200", "#333333");

  const parts: string[] = [];
  parts.push(
    summary.shown === summary.total
      ? t("gunCodes.statusTotal", { n: summary.total })
      : t("gunCodes.statusFiltered", { shown: summary.shown, total: summary.total }),
  );
  parts.push(t("gunCodes.statusGuns", { n: summary.guns }));
  if (summary.cat !== "all") parts.push(t("gunCodes.statusCat", { cat: summary.cat }));
  if (summary.gunCount > 0) parts.push(t("gunCodes.statusGunFilter", { n: summary.gunCount }));
  if (meta) parts.push(t("gunCodes.statusRules", { v: meta.version }));

  return (
    <HStack
      px={4}
      py={1.5}
      borderTop="1px solid"
      borderColor={borderColor}
      spacing={3}
      flexWrap="wrap"
      rowGap={0}
    >
      <Text fontSize="xs" color={subTextColor}>
        {parts.join("  ·  ")}
      </Text>
    </HStack>
  );
}
