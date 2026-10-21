import {
  Button,
  HStack,
  IconButton,
  Input,
  InputGroup,
  InputLeftElement,
  Menu,
  MenuButton,
  MenuItem,
  MenuList,
  Tooltip,
  useColorModeValue,
} from "@chakra-ui/react";
import {
  ArrowUpDown,
  ChevronDown,
  Download,
  LayoutGrid,
  List,
  ListFilter,
  MoreHorizontal,
  Plus,
  RefreshCw,
  Rows3,
  Search,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import type { SortKey, ViewMode } from "@/lib/gunCodes";

const SORT_KEYS: SortKey[] = ["count", "name", "recent"];
const VIEW_KEYS: ViewMode[] = ["tree", "table", "tile"];

const VIEW_ICONS: Record<ViewMode, typeof List> = {
  tree: List,
  table: Rows3,
  tile: LayoutGrid,
};

/**
 * 工具栏：搜索 / 枪名多选 / 排序 / 视图切换 / 导入导出 / 添加
 *
 * 响应式：窄窗口下整体换行（由容器的 `flexWrap` 处理）。
 */
export function NotebookToolbar({
  search,
  onSearch,
  sortKey,
  onSort,
  viewMode,
  onViewMode,
  gunFilterCount,
  gunFilterSlot,
  hasActiveFilter,
  onClearFilters,
  onAdd,
  onImportDoc,
  onExport,
  onImportJson,
  onOpenRulesManager,
  onDeleteFiltered,
  onClearAll,
  filteredCount,
  hasFilter,
  onOpenNav,
  isNarrow,
}: {
  search: string;
  onSearch: (v: string) => void;
  sortKey: SortKey;
  onSort: (k: SortKey) => void;
  viewMode: ViewMode;
  onViewMode: (v: ViewMode) => void;
  gunFilterCount: number;
  /** 枪名筛选浮层（由父级构造，便于复用同一份候选/选中状态） */
  gunFilterSlot: React.ReactNode;
  hasActiveFilter: boolean;
  onClearFilters: () => void;
  onAdd: () => void;
  onImportDoc: () => void;
  onExport: () => void;
  onImportJson: () => void;
  /** 打开词库管理面板（常驻入口，不得因宽度隐藏） */
  onOpenRulesManager: () => void;
  /** 删除当前筛选结果（无筛选时禁用） */
  onDeleteFiltered: () => void;
  /** 清空全部改枪码 */
  onClearAll: () => void;
  /** 当前筛选出的条数 */
  filteredCount: number;
  /** 是否处于筛选态 */
  hasFilter: boolean;
  onOpenNav: () => void;
  isNarrow: boolean;
}) {
  const { t } = useTranslation();
  const borderColor = useColorModeValue("gray.200", "#333333");

  return (
    <HStack spacing={2} flexWrap="wrap" w="100%" rowGap={2}>
      {isNarrow && (
        <Button size="sm" variant="outline" onClick={onOpenNav} flexShrink={0}>
          <ListFilter size={14} />
        </Button>
      )}

      <InputGroup size="sm" maxW={{ base: "100%", md: "320px" }} flex={{ base: "1 0 100%", md: "0 1 320px" }}>
        <InputLeftElement pointerEvents="none">
          <Search size={14} />
        </InputLeftElement>
        <Input
          value={search}
          onChange={(e) => onSearch(e.target.value)}
          placeholder={t("gunCodes.searchPlaceholder")}
          aria-label={t("gunCodes.search")}
        />
      </InputGroup>

      {/* 枪名多选（浮层由父级注入，按钮与内容共享同一份状态） */}
      {gunFilterSlot}

      {/* 排序 */}
      <Menu>
        <MenuButton
          as={Button}
          size="sm"
          variant="outline"
          leftIcon={<ArrowUpDown size={13} />}
          rightIcon={<ChevronDown size={12} />}
          flexShrink={0}
        >
          {t(`gunCodes.sort_${sortKey}`)}
        </MenuButton>
        <MenuList minW="140px">
          {SORT_KEYS.map((k) => (
            <MenuItem key={k} fontSize="sm" onClick={() => onSort(k)}>
              {t(`gunCodes.sort_${k}`)}
            </MenuItem>
          ))}
        </MenuList>
      </Menu>

      {/* 视图切换 */}
      <HStack spacing={0} border="1px solid" borderColor={borderColor} borderRadius="md" p="2px" flexShrink={0}>
        {VIEW_KEYS.map((v) => {
          const Icon = VIEW_ICONS[v];
          return (
            <Tooltip key={v} label={t(`gunCodes.view_${v}`)} hasArrow>
              <IconButton
                aria-label={t(`gunCodes.view_${v}`)}
                icon={<Icon size={14} />}
                size="xs"
                variant={viewMode === v ? "solid" : "ghost"}
                colorScheme={viewMode === v ? "blue" : undefined}
                onClick={() => onViewMode(v)}
              />
            </Tooltip>
          );
        })}
      </HStack>

      {hasActiveFilter && (
        <Button size="sm" variant="ghost" leftIcon={<X size={13} />} onClick={onClearFilters} flexShrink={0}>
          {t("gunCodes.clearFilters")}
        </Button>
      )}

      <HStack spacing={2} ml="auto" flexShrink={0}>
        <Button size="sm" variant="outline" leftIcon={<Upload size={14} />} onClick={onImportDoc}>
          {t("gunCodes.importDoc")}
        </Button>
        <Tooltip label={t("gunCodes.exportJson")} hasArrow>
          <IconButton
            aria-label={t("gunCodes.exportJson")}
            icon={<Download size={14} />}
            size="sm"
            variant="outline"
            onClick={onExport}
          />
        </Tooltip>
        <Tooltip label={t("gunCodes.importJson")} hasArrow>
          <IconButton
            aria-label={t("gunCodes.importJson")}
            icon={<Upload size={14} />}
            size="sm"
            variant="outline"
            onClick={onImportJson}
          />
        </Tooltip>
        <Tooltip label={t("gunCodes.rulesManagerHint")} hasArrow>
          <IconButton
            aria-label={t("gunCodes.rulesManager")}
            icon={<RefreshCw size={14} />}
            size="sm"
            variant="outline"
            onClick={onOpenRulesManager}
          />
        </Tooltip>
        {/* 危险操作收进菜单，避免误触常驻按钮 */}
        <Menu placement="bottom-end">
          <MenuButton
            as={IconButton}
            aria-label={t("gunCodes.moreActions")}
            icon={<MoreHorizontal size={14} />}
            size="sm"
            variant="outline"
          />
          <MenuList>
            <MenuItem
              icon={<Trash2 size={13} />}
              isDisabled={!hasFilter || filteredCount === 0}
              onClick={onDeleteFiltered}
            >
              {hasFilter
                ? t("gunCodes.deleteFiltered", { n: filteredCount })
                : t("gunCodes.deleteFilteredNoFilter")}
            </MenuItem>
            <MenuItem icon={<Trash2 size={13} />} color="red.500" onClick={onClearAll}>
              {t("gunCodes.clearAllCodes")}
            </MenuItem>
          </MenuList>
        </Menu>
        <Button size="sm" colorScheme="blue" leftIcon={<Plus size={15} />} onClick={onAdd}>
          {t("gunCodes.add")}
        </Button>
      </HStack>
    </HStack>
  );
}
