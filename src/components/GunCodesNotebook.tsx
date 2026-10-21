import {
  Box,
  Button,
  Drawer,
  DrawerBody,
  DrawerContent,
  DrawerOverlay,
  HStack,
  Heading,
  Spinner,
  Text,
  VStack,
  useColorModeValue,
  useDisclosure,
} from "@chakra-ui/react";
import { ArrowLeft } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useBackground } from "@/contexts/background-context";
import { useDynamicIsland } from "@/components/ui/dynamic-island";
import { useThemeColor } from "@/contexts/theme-color-context";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { store } from "@/lib/store";
import { computePreviewStats, type GunCode } from "@/lib/gunCodes";
import { copyText } from "@/lib/clipboard";
import { useGunCodes } from "@/hooks/useGunCodes";
import { parsePersisted, useGunFilters, type PersistedFilters } from "@/hooks/useGunFilters";
import { useNotebookLayout } from "@/hooks/useMediaQuery";
import { ConfirmDialogs } from "./gun-codes/ConfirmDialogs";
import { GunCodeEditor } from "./gun-codes/GunCodeEditor";
import { GunDataGrid } from "./gun-codes/GunDataGrid";
import { GunFilterPopover } from "./gun-codes/GunFilterPopover";
import { GunTileGrid } from "./gun-codes/GunTileGrid";
import { GunTreeView } from "./gun-codes/GunTreeView";
import { ImportDialog } from "./gun-codes/ImportDialog";
import { NavTilePane } from "./gun-codes/NavTilePane";
import { RulesManagerDialog } from "./gun-codes/RulesManagerDialog";
import { AppContextMenu } from "./gun-codes/AppContextMenu";
import { NotebookToolbar } from "./gun-codes/NotebookToolbar";
import { SelectionBar } from "./gun-codes/SelectionBar";
import { StatusBar } from "./gun-codes/StatusBar";

const FILTER_KEY = "nexbox_gun_codes_filter";

/**
 * 改枪码（容器）
 *
 * 原本是 1463 行单文件；现拆为 hooks（数据/筛选/响应式）+ 组件（视图/对话框）。
 * 本文件只负责编排状态与组合布局。
 */
export default function GunCodesNotebook({ onBack }: { onBack?: () => void }) {
  const { t } = useTranslation();
  const toast = useDynamicIsland("target");
  const { liquidGlassEnabled } = useBackground();
  const { getActiveColor } = useThemeColor();
  const primaryColor = getActiveColor();
  const { isNarrow, isCompact } = useNotebookLayout();

  // ── 配色 ──
  const textColor = useColorModeValue("#000000", "#ffffff");
  const subTextColor = useColorModeValue("gray.500", "gray.400");
  const cardBg = useColorModeValue("white", "#1a1a1a");
  const panelBg = useColorModeValue("gray.50", "#202020");
  const cardBorder = useColorModeValue("gray.200", "#333333");

  // ── 液态玻璃联动 ──
  // 玻璃开启时整块插件区域包在 LiquidGlassCard 里；内部面板/分隔线改用半透明，
  // 否则玻璃表面里会嵌着实色块（原来的做法只是把背景置空，等于没有玻璃）。
  const glassPanelBg = useColorModeValue("rgba(255,255,255,0.35)", "rgba(0,0,0,0.28)");
  const glassDivider = useColorModeValue("rgba(0,0,0,0.08)", "rgba(255,255,255,0.12)");

  const cardBgOut = liquidGlassEnabled ? glassPanelBg : cardBg;
  const panelBgOut = liquidGlassEnabled ? glassPanelBg : panelBg;
  const borderOut = liquidGlassEnabled ? glassDivider : cardBorder;

  const showToast = useCallback(
    (msg: string, type?: "error" | "info") => {
      toast({ description: msg, status: type === "error" ? "error" : "success" });
    },
    [toast],
  );

  // ── 数据层 ──
  const data = useGunCodes((msg) => showToast(t("gunCodes.loadFailed", { err: msg }), "error"));

  // ── 筛选层（持久化到 store） ──
  const persist = useCallback((v: PersistedFilters) => {
    void store.set(FILTER_KEY, v).catch(() => {
      /* 持久化失败不影响使用 */
    });
  }, []);
  const filters = useGunFilters(data.items, persist);

  // 载入已保存的筛选（一次性）
  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const saved = await store.get<Record<string, unknown>>(FILTER_KEY);
        if (alive && saved) filters.hydrate(parsePersisted(saved));
        else if (alive) filters.hydrate({});
      } catch {
        if (alive) filters.hydrate({});
      }
    })();
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ── 本地 UI 状态 ──
  const [editorOpen, setEditorOpen] = useState(false);
  const [editing, setEditing] = useState<GunCode | null>(null);
  const [pendingDelete, setPendingDelete] = useState<GunCode | null>(null);
  const [bulkDeleteCount, setBulkDeleteCount] = useState(0);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  // 导入预览流
  const [importText, setImportText] = useState("");
  const [preview, setPreview] = useState<Awaited<ReturnType<typeof data.preview>> | null>(null);
  const [deselected, setDeselected] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);

  const navDisc = useDisclosure();
  const gunFilterDisc = useDisclosure();
  const importDisc = useDisclosure();
  const rulesDisc = useDisclosure();

  // ── 选中项与当前可见集合求交（筛选变化后自动丢弃不可见的选中项）──
  const visibleIds = useMemo(() => new Set(filters.filtered.map((i) => i.id)), [filters.filtered]);
  const effectiveSelected = useMemo(() => {
    const out = new Set<string>();
    for (const id of selectedIds) if (visibleIds.has(id)) out.add(id);
    return out;
  }, [selectedIds, visibleIds]);
  const selectedItems = useMemo(
    () => filters.filtered.filter((i) => effectiveSelected.has(i.id)),
    [filters.filtered, effectiveSelected],
  );

  // ── 操作：复制 ──
  const handleCopy = useCallback(
    async (code: string) => {
      const ok = await copyText(code);
      showToast(ok ? t("gunCodes.copied") : t("gunCodes.copyFailed"), ok ? undefined : "error");
    },
    [showToast, t],
  );

  const handleCopySelected = useCallback(async () => {
    const text = selectedItems.map((i) => `${i.weapon_name} ${i.code}`).join("\n");
    const ok = await copyText(text);
    showToast(
      ok ? t("gunCodes.bulkCopied", { n: selectedItems.length }) : t("gunCodes.copyFailed"),
      ok ? undefined : "error",
    );
    if (ok) setSelectedIds(new Set());
  }, [selectedItems, showToast, t]);

  // ── 操作：保存 ──
  const handleSave = useCallback(
    async (weaponName: string, code: string, note: string) => {
      try {
        if (editing) {
          await data.update(editing.id, weaponName, code, note);
          showToast(t("gunCodes.updated"));
        } else {
          await data.add(weaponName, code, note);
          showToast(t("gunCodes.saved"));
        }
      } catch (e) {
        showToast(t("gunCodes.saveFailed", { err: String(e) }), "error");
        throw e;
      }
    },
    [data, editing, showToast, t],
  );

  // ── 操作：删除 ──
  const doDelete = useCallback(async () => {
    const item = pendingDelete;
    setPendingDelete(null);
    if (!item) return;
    try {
      await data.remove(item.id);
      showToast(t("gunCodes.deleted"));
    } catch (e) {
      showToast(t("gunCodes.deleteFailed", { err: String(e) }), "error");
    }
  }, [pendingDelete, data, showToast, t]);

  const doBulkDelete = useCallback(async () => {
    const ids = [...effectiveSelected];
    setBulkDeleteCount(0);
    if (ids.length === 0) return;
    try {
      const n = await data.removeMany(ids);
      setSelectedIds(new Set());
      showToast(t("gunCodes.bulkDeleted", { n }));
    } catch (e) {
      showToast(t("gunCodes.deleteFailed", { err: String(e) }), "error");
    }
  }, [effectiveSelected, data, showToast, t]);

  // ── 操作：备份 / 词库 ──
  const handleExport = useCallback(async () => {
    try {
      const n = await data.exportJson();
      if (n !== null) showToast(t("gunCodes.exported", { n }));
    } catch (e) {
      showToast(t("gunCodes.exportFailed", { err: String(e) }), "error");
    }
  }, [data, showToast, t]);

  const handleImportJson = useCallback(async () => {
    try {
      const n = await data.importJson();
      if (n !== null) showToast(t("gunCodes.importedJson", { n }));
    } catch (e) {
      showToast(t("gunCodes.importFailed", { err: String(e) }), "error");
    }
  }, [data, showToast, t]);

  const handleRefreshRules = useCallback(async () => {
    try {
      await data.refreshRules();
      showToast(t("gunCodes.rulesRefreshed"));
    } catch (e) {
      showToast(t("gunCodes.refreshFailed", { err: String(e) }), "error");
    }
  }, [data, showToast, t]);

  // ── 导入预览 ──
  const doPreview = useCallback(async () => {
    if (!importText.trim()) return;
    setBusy(true);
    try {
      setPreview(await data.preview(importText));
      setDeselected(new Set());
    } catch (e) {
      showToast(t("gunCodes.parseFailed", { err: String(e) }), "error");
    } finally {
      setBusy(false);
    }
  }, [importText, data, showToast, t]);

  const doConfirmImport = useCallback(async () => {
    if (!preview) return;
    const rows = preview.rows
      .filter((r) => !r.duplicate && !deselected.has(r.code))
      .map((r) => ({ gun: r.gun, config: r.config, code: r.code }));
    if (rows.length === 0) return;
    setBusy(true);
    try {
      const res = await data.importRows(rows);
      showToast(
        t("gunCodes.importDone", {
          imported: res.imported,
          updated: res.updated,
          skipped: res.skipped,
        }),
      );
      importDisc.onClose();
      setPreview(null);
      setImportText("");
    } catch (e) {
      showToast(t("gunCodes.importFailed", { err: String(e) }), "error");
    } finally {
      setBusy(false);
    }
  }, [preview, deselected, data, showToast, t, importDisc]);

  const previewStats = useMemo(() => computePreviewStats(preview, deselected), [preview, deselected]);

  // ── 选中操作 ──
  const toggleSelect = useCallback((id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const toggleSelectAll = useCallback(
    (on: boolean) => {
      setSelectedIds(on ? new Set(filters.filtered.map((i) => i.id)) : new Set());
    },
    [filters.filtered],
  );

  const navPaneProps = {
    stats: filters.stats,
    meta: data.meta,
    catCounts: filters.catCounts,
    catOrder: filters.catOrder,
    catFilter: filters.catFilter,
    onSelectCat: filters.selectCat,
    onRefreshRules: handleRefreshRules,
    onOpenRulesManager: rulesDisc.onOpen,
    textColor,
    subTextColor,
    cardBorder: borderOut,
  };

  const viewProps = {
    query: filters.search,
    highlightColor: primaryColor,
    onCopy: handleCopy,
    onEdit: (item: GunCode) => {
      setEditing(item);
      setEditorOpen(true);
    },
    onDelete: (item: GunCode) => setPendingDelete(item),
    textColor,
    subTextColor,
    cardBorder: borderOut,
    cardBg: cardBgOut,
  };

  const ready = !data.loading && filters.hydrated;

  // 全部内容（外壳只负责玻璃/实色两种外包装，避免重复一遍 JSX）
  const shell = (
    <>
      {/* 顶部：返回 + 标题 */}
      <HStack
        px={4}
        py={3}
        borderBottom={isNarrow ? "1px solid" : undefined}
        borderColor={borderOut}
        spacing={3}
        flexShrink={0}
      >
        {onBack && (
          <Button
            size="sm"
            variant="ghost"
            leftIcon={<ArrowLeft size={15} />}
            onClick={onBack}
          >
            {t("gunCodes.back")}
          </Button>
        )}
        <Heading size="sm" color={textColor}>
          {t("gunCodes.title")}
        </Heading>
        {isCompact && (
          <Text fontSize="xs" color={subTextColor} noOfLines={1}>
            {filters.stats.count} {t("gunCodes.statRecords")}
          </Text>
        )}
      </HStack>

      {/* 主体：侧边导航 + 内容 */}
      <HStack align="stretch" spacing={0} flex={1} minH={0}>
        {!isNarrow && (
          <Box
            w="220px"
            flexShrink={0}
            px={3}
            py={4}
            borderRight="1px solid"
            borderColor={borderOut}
            overflowY="auto"
          >
            <NavTilePane {...navPaneProps} />
          </Box>
        )}

        <VStack align="stretch" spacing={0} flex={1} minW={0}>
          {/* 工具栏（含枪名筛选浮层，浮层由工具栏按钮控制） */}
          <Box px={4} py={3} borderBottom="1px solid" borderColor={borderOut} flexShrink={0}>
            <NotebookToolbar
              search={filters.search}
              onSearch={filters.setSearch}
              sortKey={filters.sortKey}
              onSort={filters.setSortKey}
              viewMode={filters.viewMode}
              onViewMode={filters.setViewMode}
              onOpenRulesManager={rulesDisc.onOpen}
              gunFilterCount={filters.gunFilter.length}
              gunFilterSlot={
                <GunFilterPopover
                  isOpen={gunFilterDisc.isOpen}
                  onOpen={gunFilterDisc.onOpen}
                  onClose={gunFilterDisc.onClose}
                  candidates={filters.gunCandidates}
                  selected={filters.gunFilter}
                  onToggle={filters.toggleGun}
                  onClear={() => filters.setGuns([])}
                  onBulk={(guns, on) => {
                    const cur = new Set(filters.gunFilter);
                    for (const g of guns) {
                      if (on) cur.add(g);
                      else cur.delete(g);
                    }
                    filters.setGuns([...cur]);
                  }}
                  textColor={textColor}
                  subTextColor={subTextColor}
                  bg={cardBgOut}
                  border={borderOut}
                />
              }
              hasActiveFilter={filters.hasActiveFilter}
              onClearFilters={filters.clearFilters}
              onAdd={() => {
                setEditing(null);
                setEditorOpen(true);
              }}
              onImportDoc={importDisc.onOpen}
              onExport={handleExport}
              onImportJson={handleImportJson}
              onOpenNav={navDisc.onOpen}
              isNarrow={isNarrow}
            />
          </Box>

          {/* 批量操作条 */}
          {effectiveSelected.size > 0 && (
            <Box px={4} pt={3} flexShrink={0}>
              <SelectionBar
                count={effectiveSelected.size}
                onCopyAll={handleCopySelected}
                onDelete={() => setBulkDeleteCount(effectiveSelected.size)}
                onClear={() => setSelectedIds(new Set())}
              />
            </Box>
          )}

          {/* 内容区 */}
          <Box flex={1} minH={0} overflowY="auto" px={4} py={4}>
            {!ready ? (
              <VStack py={16} spacing={3}>
                <Spinner size="md" />
                <Text fontSize="sm" color={subTextColor}>
                  {t("gunCodes.loading")}
                </Text>
              </VStack>
            ) : filters.filtered.length === 0 ? (
              <VStack py={16} spacing={3}>
                <Text fontSize="md" fontWeight="semibold" color={textColor}>
                  {data.items.length === 0 ? t("gunCodes.emptyTitle") : t("gunCodes.noMatchTitle")}
                </Text>
                <Text fontSize="sm" color={subTextColor} textAlign="center" maxW="380px">
                  {data.items.length === 0 ? t("gunCodes.emptyHint") : t("gunCodes.noMatchHint")}
                </Text>
                {data.items.length === 0 ? (
                  <Button
                    size="sm"
                    colorScheme="blue"
                    onClick={() => {
                      setEditing(null);
                      setEditorOpen(true);
                    }}
                  >
                    {t("gunCodes.add")}
                  </Button>
                ) : (
                  <Button size="sm" variant="outline" onClick={filters.clearFilters}>
                    {t("gunCodes.clearFilters")}
                  </Button>
                )}
              </VStack>
            ) : filters.viewMode === "table" ? (
              <GunDataGrid
                rows={filters.rows}
                selectedIds={effectiveSelected}
                onToggleSelect={toggleSelect}
                onToggleAll={toggleSelectAll}
                compact={isCompact}
                {...viewProps}
              />
            ) : filters.viewMode === "tile" ? (
              <GunTileGrid rows={filters.rows} {...viewProps} />
            ) : (
              <GunTreeView
                grouped={filters.grouped}
                collapsed={filters.collapsed}
                onToggleCollapse={filters.toggleCollapse}
                panelBg={panelBgOut}
                {...viewProps}
              />
            )}
          </Box>

          {/* 状态栏 */}
          <StatusBar summary={filters.filterSummary} meta={data.meta} />
        </VStack>
      </HStack>

      {/* 窄屏导航抽屉 */}
      <Drawer isOpen={navDisc.isOpen} placement="left" onClose={navDisc.onClose} size="xs">
        <DrawerOverlay />
        <DrawerContent>
          <DrawerBody py={4}>
            <NavTilePane {...navPaneProps} variant="drawer" />
          </DrawerBody>
        </DrawerContent>
      </Drawer>

      {/* 编辑器抽屉 */}
      <GunCodeEditor
        open={editorOpen}
        editing={editing}
        onClose={() => {
          setEditorOpen(false);
          setEditing(null);
        }}
        onRecognize={data.recognize}
        onSave={handleSave}
      />

      {/* 导入预览流 */}
      <ImportDialog
        isOpen={importDisc.isOpen}
        onClose={() => {
          importDisc.onClose();
          setPreview(null);
        }}
        text={importText}
        setText={setImportText}
        preview={preview}
        deselected={deselected}
        onToggleRow={(code) =>
          setDeselected((prev) => {
            const next = new Set(prev);
            if (next.has(code)) next.delete(code);
            else next.add(code);
            return next;
          })
        }
        onBulkSelect={(codes, on) =>
          setDeselected((prev) => {
            const next = new Set(prev);
            for (const c of codes) {
              if (on) next.delete(c);
              else next.add(c);
            }
            return next;
          })
        }
        onPreview={doPreview}
        onConfirm={doConfirmImport}
        busy={busy}
        stats={previewStats}
        textColor={textColor}
        subTextColor={subTextColor}
        bg={panelBgOut}
        border={borderOut}
        cardBg={cardBgOut}
      />

      {/* 删除确认 */}
      {/* 词库管理面板：在线更新 + 学习表纠错 */}
      <RulesManagerDialog
        isOpen={rulesDisc.isOpen}
        onClose={rulesDisc.onClose}
        meta={data.meta}
        onCheckUpdate={data.checkRulesUpdate}
        onApplyUpdate={data.updateRules}
        onResetRules={data.resetRules}
        onListLearned={data.listLearned}
        onDeleteLearned={data.deleteLearned}
        onClearLearned={data.clearLearned}
        onClearNotes={data.clearLearnedNotes}
        onListRules={data.listRuleFingerprints}
        onDeleteRule={data.deleteRuleFingerprint}
        onUpdateRule={data.updateRuleFingerprint}
        onClearOnlineRules={data.clearOnlineRules}
        onClearRuleOverrides={data.clearRuleOverrides}
        textColor={textColor}
        subTextColor={subTextColor}
        panelBg={panelBg}
        cardBorder={borderOut}
      />

      {/* 应用内右键菜单：替代 WebView2 的「刷新/另存为/打印」 */}
      <AppContextMenu />

      <ConfirmDialogs
        pendingDelete={pendingDelete}
        onCancelDelete={() => setPendingDelete(null)}
        onConfirmDelete={doDelete}
        bulkCount={bulkDeleteCount}
        onCancelBulk={() => setBulkDeleteCount(0)}
        onConfirmBulk={doBulkDelete}
        textColor={textColor}
        subTextColor={subTextColor}
        panelBg={panelBgOut}
        cardBg={cardBgOut}
      />
    </>
  );

  const shellLayout = {
    overflow: "hidden" as const,
    display: "flex",
    flexDirection: "column" as const,
    h: "100%",
    minH: isCompact ? "auto" : "70vh",
  };

  // 玻璃开启：整块插件区域做成液态玻璃表面，与软件其余部分一致
  if (liquidGlassEnabled) {
    return (
      <LiquidGlassCard p={0} {...shellLayout}>
        {shell}
      </LiquidGlassCard>
    );
  }

  return (
    <Box
      bg={cardBg}
      borderRadius="xl"
      border="1px solid"
      borderColor={cardBorder}
      {...shellLayout}
    >
      {shell}
    </Box>
  );
}
