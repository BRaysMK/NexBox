import { Box, Button, HStack, IconButton, Stack, Text, Tooltip, VStack } from "@chakra-ui/react";
import { Crosshair, LayoutGrid, Layers, RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { CATEGORY_FALLBACK, type RulesMeta } from "@/lib/gunCodes";

/**
 * 侧边导航：分类导航 + 统计 + 词库信息
 *
 * 宽屏常驻（`variant="pane"`），窄屏放进 Drawer（`variant="drawer"`）。
 */
export function NavTilePane({
  stats,
  meta,
  catCounts,
  catOrder,
  catFilter,
  onSelectCat,
  onRefreshRules,
  textColor,
  subTextColor,
  cardBorder,
  variant = "pane",
}: {
  stats: { count: number; guns: number };
  meta: RulesMeta | null;
  catCounts: Map<string, number>;
  catOrder: string[];
  catFilter: string;
  onSelectCat: (cat: string) => void;
  onRefreshRules: () => void;
  textColor: string;
  subTextColor: string;
  cardBorder: string;
  variant?: "pane" | "drawer";
}) {
  const { t } = useTranslation();

  const item = (key: string, label: string, count: number, isFallback = false) => {
    const active = catFilter === key;
    return (
      <Button
        key={key}
        size="sm"
        variant={active ? "solid" : "ghost"}
        colorScheme={active ? "blue" : undefined}
        justifyContent="space-between"
        w="100%"
        fontWeight={active ? "semibold" : "normal"}
        leftIcon={
          isFallback ? (
            <Box w="10px" h="10px" borderRadius="sm" bg="gray.400" />
          ) : (
            <Box w="10px" h="10px" borderRadius="sm" bg="blue.400" opacity={0.6} />
          )
        }
        onClick={() => onSelectCat(key)}
      >
        <Text flex={1} textAlign="left" fontSize="sm" noOfLines={1}>
          {label}
        </Text>
        <Text fontSize="xs" opacity={0.7}>
          {count}
        </Text>
      </Button>
    );
  };

  return (
    <VStack align="stretch" spacing={4} w="100%">
      {variant === "drawer" && (
        <HStack spacing={2} px={1}>
          <Crosshair size={18} />
          <Text fontWeight="bold" color={textColor}>
            {t("gunCodes.title")}
          </Text>
        </HStack>
      )}

      {/* 统计 */}
      <HStack spacing={2}>
        <Box
          flex={1}
          border="1px solid"
          borderColor={cardBorder}
          borderRadius="md"
          px={3}
          py={2}
          textAlign="center"
        >
          <Text fontSize="lg" fontWeight="bold" color={textColor} lineHeight={1.2}>
            {stats.count}
          </Text>
          <Text fontSize="xs" color={subTextColor}>
            {t("gunCodes.statRecords")}
          </Text>
        </Box>
        <Box
          flex={1}
          border="1px solid"
          borderColor={cardBorder}
          borderRadius="md"
          px={3}
          py={2}
          textAlign="center"
        >
          <Text fontSize="lg" fontWeight="bold" color={textColor} lineHeight={1.2}>
            {stats.guns}
          </Text>
          <Text fontSize="xs" color={subTextColor}>
            {t("gunCodes.statGuns")}
          </Text>
        </Box>
      </HStack>

      {/* 分类导航 */}
      <Box>
        <HStack spacing={1} px={1} mb={2} color={subTextColor}>
          <Layers size={12} />
          <Text fontSize="xs" textTransform="uppercase" letterSpacing="0.06em">
            {t("gunCodes.categoryLabel")}
          </Text>
        </HStack>
        <Stack spacing={0.5}>
          {item("all", t("gunCodes.allCategories"), stats.count)}
          {catOrder.map((cat) =>
            item(
              cat,
              cat === CATEGORY_FALLBACK ? t("gunCodes.categoryOther") : cat,
              catCounts.get(cat) ?? 0,
              cat === CATEGORY_FALLBACK,
            ),
          )}
          {catOrder.length === 0 && (
            <HStack spacing={1} px={2} py={1} color={subTextColor}>
              <LayoutGrid size={12} />
              <Text fontSize="xs">{t("gunCodes.noCategory")}</Text>
            </HStack>
          )}
        </Stack>
      </Box>

      {/* 词库 */}
      <Box mt="auto" pt={3} borderTop="1px solid" borderColor={cardBorder}>
        <HStack justify="space-between" px={1} mb={1}>
          <Text fontSize="xs" color={subTextColor}>
            {t("gunCodes.rulesLabel")}
          </Text>
          <Tooltip label={t("gunCodes.refreshRulesHint")} hasArrow>
            <IconButton
              aria-label={t("gunCodes.refreshRules")}
              icon={<RefreshCw size={12} />}
              size="xs"
              variant="ghost"
              onClick={onRefreshRules}
            />
          </Tooltip>
        </HStack>
        <Text fontSize="xs" color={subTextColor} px={1} lineHeight={1.6}>
          {meta
            ? t("gunCodes.rulesMeta", {
                version: meta.version,
                guns: meta.gunCount,
                fps: meta.fingerprintCount,
                learned: meta.learnedCount,
              })
            : t("gunCodes.rulesLoading")}
        </Text>
      </Box>
    </VStack>
  );
}
