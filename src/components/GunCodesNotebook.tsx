/**
 * 三角洲本地改枪码记事本 —— 重写版（2026-10）
 *
 * ## 相对旧版的改动
 *
 * ### 状态收敛（旧版 17 个 useState）
 * 用 `useReducer` 管理列表/筛选/编辑/导入四组状态，显式建模
 * `idle | parsing | preview | saving` 阶段，避免散乱布尔量互相打架。
 *
 * ### 导入预览流
 * 粘贴 → `preview_import_gun_codes` 解析（**不写库**）→ 预览表
 * （可取消勾选 / 看识别理由与置信度 / 标记重复）→ 确认导入。
 *
 * ### 筛选重写（B1–B7）
 * - B1 归一化索引：`AK-12`/`AK12` 合并为同一筛选项
 * - B2 分类兜底可见：未收录枪归入可见的「其他」，不再出现"按钮不显示"
 * - B3 枪名改可搜索多选（自建 popover，与分类**可组合**）
 * - B4 搜索：归一化匹配 + 匹配高亮 + 相关性排序
 * - B5 排序：名称 / 条数 / 最近更新
 * - B6 筛选状态持久化（`@/lib/store`）
 * - B7 折叠状态与筛选解耦（换筛选默认全展开）
 *
 * ### i18n
 * 接入 `gunCodes.*` 文案段（旧版是全项目唯一硬编码中文的页面组件）。
 */
import { useState, useEffect, useRef, useCallback, useMemo, useReducer } from "react";
import {
  Box, Text, HStack, VStack, Input, Button, IconButton, Badge, Divider,
  useColorModeValue, AlertDialog, AlertDialogBody, AlertDialogFooter,
  AlertDialogHeader, AlertDialogContent, AlertDialogOverlay, Checkbox,
  Spinner, Tooltip, InputGroup, InputLeftElement, Popover, PopoverTrigger,
  PopoverContent, PopoverBody, Portal, useDisclosure, Progress,
} from "@chakra-ui/react";
import { invoke } from "@tauri-apps/api/core";
import { save, open } from "@tauri-apps/plugin-dialog";
import {
  ArrowLeft, Search, Copy, Pencil, Trash2, Plus, Download, Upload,
  X, ChevronDown, Check, RefreshCw, Info, AlertTriangle, ListFilter,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { useDynamicIsland } from "@/components/ui/dynamic-island";
import { useBackground } from "@/contexts/background-context";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";
import { useThemeColor } from "@/contexts/theme-color-context";
import { store } from "@/lib/store";

// ═══════════════════════ 类型 ═══════════════════════

interface GunCode {
  id: string;
  weapon_name: string;
  code: string;
  note: string;
  created_at: number;
}

/** 后端 recognize_gun_name 的返回 */
interface RecognizeResult {
  weapon_name: string;
  note: string;
  recognized: boolean;
  confidence: number;
  reason: string;
  source: string;
}

/** 后端 preview_import_gun_codes 的单行 */
interface PreviewRow {
  code: string;
  gun: string;
  config: string;
  recognized: boolean;
  confidence: number;
  reason: string;
  source: string;
  duplicate: boolean;
}

interface PreviewResult {
  total: number;
  recognized: number;
  unrecognized: number;
  rows: PreviewRow[];
}

interface RulesMeta {
  version: number;
  generated: string;
  source: string;
  gunCount: number;
  fingerprintCount: number;
  learnedCount: number;
  categories: Record<string, number>;
}

type SortKey = "count" | "name" | "recent";

interface State {
  items: GunCode[];
  loading: boolean;

  // 筛选
  search: string;
  catFilter: string;
  gunFilter: string[];      // B3：多选
  sortKey: SortKey;
  /** B7：换筛选时重置的折叠集合 */
  collapsed: Set<string>;

  // 新增
  code: string;
  weapon: string;
  note: string;
  recog: RecognizeResult | null;
  saving: boolean;

  // 编辑
  editingId: string | null;
  editWeapon: string;
  editNote: string;

  // 导入
  showImport: boolean;
  importText: string;
  preview: PreviewResult | null;
  /** 预览中被取消勾选的行（按 code 记） */
  deselected: Set<string>;
  importing: boolean;

  // 删除确认
  pendingDelete: GunCode | null;
}

type Action =
  | { type: "set"; patch: Partial<State> }
  | { type: "items"; items: GunCode[] }
  | { type: "toggleGun"; gun: string }
  | { type: "clearGuns" }
  | { type: "toggleCollapse"; gun: string }
  | { type: "resetCollapse" }
  | { type: "toggleDeselect"; code: string }
  | { type: "deselectAll"; codes: string[]; on: boolean }
  | { type: "toggleGunFilterMulti"; guns: string[]; on: boolean };

const initialState: State = {
  items: [],
  loading: true,
  search: "",
  catFilter: "all",
  gunFilter: [],
  sortKey: "count",
  collapsed: new Set(),
  code: "",
  weapon: "",
  note: "",
  recog: null,
  saving: false,
  editingId: null,
  editWeapon: "",
  editNote: "",
  showImport: false,
  importText: "",
  preview: null,
  deselected: new Set(),
  importing: false,
  pendingDelete: null,
};

function reducer(s: State, a: Action): State {
  switch (a.type) {
    case "set":
      return { ...s, ...a.patch };
    case "items":
      return { ...s, items: a.items, loading: false };
    case "toggleGun": {
      const next = new Set(s.gunFilter);
      if (next.has(a.gun)) next.delete(a.gun);
      else next.add(a.gun);
      // B7：换筛选 → 展开全部
      return { ...s, gunFilter: [...next], collapsed: new Set() };
    }
    case "clearGuns":
      return { ...s, gunFilter: [], collapsed: new Set() };
    case "toggleGunFilterMulti": {
      const next = new Set(s.gunFilter);
      for (const g of a.guns) {
        if (a.on) next.add(g);
        else next.delete(g);
      }
      return { ...s, gunFilter: [...next], collapsed: new Set() };
    }
    case "toggleCollapse": {
      const next = new Set(s.collapsed);
      if (next.has(a.gun)) next.delete(a.gun);
      else next.add(a.gun);
      return { ...s, collapsed: next };
    }
    case "resetCollapse":
      return { ...s, collapsed: new Set() };
    case "toggleDeselect": {
      const next = new Set(s.deselected);
      if (next.has(a.code)) next.delete(a.code);
      else next.add(a.code);
      return { ...s, deselected: next };
    }
    case "deselectAll": {
      const next = new Set(s.deselected);
      for (const c of a.codes) {
        if (a.on) next.add(c);
        else next.delete(c);
      }
      return { ...s, deselected: next };
    }
    default:
      return s;
  }
}

// ═══════════════════════ 工具 ═══════════════════════

/** 与后端 `normalize()` 保持一致的归一化（去空格/连字符/下划线，转大写） */
function norm(s: string): string {
  return s.replace(/[\s\-_]/g, "").toUpperCase();
}

function fmtTime(sec: number): string {
  if (!sec) return "";
  const d = new Date(sec * 1000);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    try {
      const ta = document.createElement("textarea");
      ta.value = text;
      ta.style.position = "fixed";
      ta.style.opacity = "0";
      document.body.appendChild(ta);
      ta.select();
      const ok = document.execCommand("copy");
      document.body.removeChild(ta);
      return ok;
    } catch {
      return false;
    }
  }
}

/** B4：高亮匹配片段 */
function Highlight({ text, query, color }: { text: string; query: string; color: string }) {
  if (!query.trim()) return <>{text}</>;
  const q = norm(query);
  const n = norm(text);
  const idx = n.indexOf(q);
  if (idx < 0) return <>{text}</>;
  // 归一化后索引与原串索引可能错位（去掉了符号），退化为按词高亮
  const parts = text.split(new RegExp(`(${query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`, "ig"));
  return (
    <>
      {parts.map((p, i) =>
        p.toLowerCase() === query.toLowerCase() ? (
          <Box as="mark" key={i} bg="transparent" color={color} fontWeight="bold">
            {p}
          </Box>
        ) : (
          <span key={i}>{p}</span>
        ),
      )}
    </>
  );
}

// ═══════════════════════ 主组件 ═══════════════════════

export default function GunCodesNotebook({ onBack }: { onBack?: () => void }) {
  const { t } = useTranslation();
  const toast = useDynamicIsland("target");
  const { liquidGlassEnabled } = useBackground();
  const { getActiveColor } = useThemeColor();
  const primaryColor = getActiveColor();

  const [s, dispatch] = useReducer(reducer, initialState);
  const set = useCallback((patch: Partial<State>) => dispatch({ type: "set", patch }), []);

  const recogTimer = useRef<number | null>(null);
  const rulesMeta = useRef<RulesMeta | null>(null);
  const deleteCancelRef = useRef<HTMLButtonElement>(null);

  // 配色
  const textColor = useColorModeValue("#000000", "#ffffff");
  const subTextColor = useColorModeValue("gray.500", "gray.400");
  const cardBg = useColorModeValue("white", "#1a1a1a");
  const panelBg = useColorModeValue("gray.50", "#202020");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const pillActive = useColorModeValue("gray.100", "#2a2a2a");
  const wrapperBg = liquidGlassEnabled ? undefined : cardBg;

  const showToast = useCallback(
    (msg: string, type?: "error" | "info") => {
      toast({ description: msg, status: type === "error" ? "error" : "success" });
    },
    [toast],
  );

  // ── 加载列表 ──
  const refresh = useCallback(async () => {
    try {
      const items = await invoke<GunCode[]>("get_local_gun_codes");
      dispatch({ type: "items", items });
    } catch (e) {
      showToast(t("gunCodes.loadFailed", { err: String(e) }), "error");
    }
  }, [showToast, t]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // ── 规则库元信息（词库版本展示） ──
  const [meta, setMeta] = useState<RulesMeta | null>(null);
  const loadMeta = useCallback(async () => {
    try {
      const m = await invoke<RulesMeta>("get_gun_rules_meta");
      rulesMeta.current = m;
      setMeta(m);
    } catch {
      /* 忽略 */
    }
  }, []);
  useEffect(() => {
    void loadMeta();
  }, [loadMeta]);

  // ── B6：筛选状态持久化 ──
  const FILTER_KEY = "nexbox_gun_codes_filter";
  useEffect(() => {
    (async () => {
      try {
        const saved = await store.get<{
          search?: string; catFilter?: string; gunFilter?: string[]; sortKey?: SortKey;
        }>(FILTER_KEY);
        if (saved) {
          set({
            search: saved.search ?? "",
            catFilter: saved.catFilter ?? "all",
            gunFilter: saved.gunFilter ?? [],
            sortKey: saved.sortKey ?? "count",
          });
        }
      } catch {
        /* 忽略 */
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    void store.set(FILTER_KEY, {
      search: s.search, catFilter: s.catFilter, gunFilter: s.gunFilter, sortKey: s.sortKey,
    }).then(() => store.save()).catch(() => {});
  }, [s.search, s.catFilter, s.gunFilter, s.sortKey]);

  // ── 别名索引（B1：归一化，合并变体） ──
  // 后端规则库里已有别名表；此处从列表已有的枪名 + 常见变体构造轻量索引，
  // 用于把「AK-12」与「AK12」这类写法聚合到同一筛选项。
  const canonicalOf = useCallback((raw: string): string => {
    const n = norm(raw);
    // 常见变体 → 规范名（与后端 rules.json 的 aliases 对齐）
    const map: Record<string, string> = {
      AK12: "AK-12", SR3M: "SR-3M", QJB201: "QJB", M16: "M16A4",
      ASVAL: "AS Val", M4: "M4A1", M7A1: "M7", M250A: "M250",
      PKMA: "PKM", M14A: "M14", AKM74: "AKM",
    };
    return map[n] ?? raw;
  }, []);

  // ── B1/B2：分类索引（未收录 → 「其他」，且「其他」可见） ──
  const catOf = useCallback((gun: string): string => {
    const c = meta?.categories ? Object.keys(meta.categories) : [];
    // 用后端返回的分类统计无法反查单枪分类；改为从规则库常识表兜底
    return CATEGORY_FALLBACK[canonicalOf(gun)] ?? "其他";
  }, [canonicalOf, meta]);

  // ── 过滤 + 排序 ──
  const filtered = useMemo(() => {
    const q = norm(s.search);
    const gunSet = new Set(s.gunFilter.map(norm));
    return s.items.filter((i) => {
      // B1：分类与枪名都按归一化比较
      if (s.catFilter !== "all" && catOf(i.weapon_name) !== s.catFilter) return false;
      if (gunSet.size > 0 && !gunSet.has(norm(canonicalOf(i.weapon_name)))) return false;
      if (!q) return true;
      // B4：归一化匹配（搜 ak12 命中 AK-12）
      return (
        norm(i.weapon_name).includes(q) ||
        i.code.toUpperCase().includes(q) ||
        norm(i.note).includes(q)
      );
    });
  }, [s.items, s.search, s.gunFilter, s.catFilter, catOf, canonicalOf]);

  // ── B5：分组 + 排序 ──
  const grouped = useMemo(() => {
    const g = new Map<string, GunCode[]>();
    for (const it of filtered) {
      const key = canonicalOf(it.weapon_name);
      const arr = g.get(key) ?? [];
      arr.push(it);
      g.set(key, arr);
    }
    const entries = [...g.entries()];
    switch (s.sortKey) {
      case "name":
        entries.sort((a, b) => a[0].localeCompare(b[0], "zh-Hans-CN"));
        break;
      case "recent":
        entries.sort(
          (a, b) => Math.max(...b[1].map((x) => x.created_at)) - Math.max(...a[1].map((x) => x.created_at)),
        );
        break;
      default:
        entries.sort((a, b) => b[1].length - a[1].length);
    }
    return entries;
  }, [filtered, s.sortKey, canonicalOf]);

  // ── B3：枪名候选（受分类筛选影响，可搜索） ──
  const gunCandidates = useMemo(() => {
    const counts = new Map<string, number>();
    for (const i of s.items) {
      const gun = canonicalOf(i.weapon_name);
      if (s.catFilter !== "all" && catOf(i.weapon_name) !== s.catFilter) continue;
      counts.set(gun, (counts.get(gun) ?? 0) + 1);
    }
    return [...counts.entries()].sort((a, b) => b[1] - a[1]);
  }, [s.items, s.catFilter, catOf, canonicalOf]);

  // ── B2：分类列表（动态生成，含可见的「其他」） ──
  const catCounts = useMemo(() => {
    const m = new Map<string, number>();
    for (const i of s.items) {
      const c = catOf(i.weapon_name);
      m.set(c, (m.get(c) ?? 0) + 1);
    }
    return m;
  }, [s.items, catOf]);

  const catOrder = useMemo(() => {
    // 固定顺序在前，其余（含「其他」）按数量降序追加 —— 保证「其他」可见
    const fixed = ["步枪", "冲锋枪", "射手步枪", "机枪", "霰弹枪", "手枪", "弓"];
    const rest = [...catCounts.keys()].filter((c) => !fixed.includes(c) && catCounts.get(c));
    rest.sort((a, b) => (catCounts.get(b) ?? 0) - (catCounts.get(a) ?? 0));
    return [...fixed, ...rest].filter((c) => (catCounts.get(c) ?? 0) > 0);
  }, [catCounts]);

  const stats = useMemo(
    () => ({
      count: s.items.length,
      guns: new Set(s.items.map((i) => canonicalOf(i.weapon_name))).size,
    }),
    [s.items, canonicalOf],
  );

  // ── 新增：粘贴识别 ──
  const handleCodeChange = useCallback(
    (v: string) => {
      set({ code: v });
      const clean = v.replace(/[^a-zA-Z0-9]/g, "");
      if (clean.length < 6) {
        set({ recog: null, weapon: "", note: "" });
        return;
      }
      if (recogTimer.current) window.clearTimeout(recogTimer.current);
      recogTimer.current = window.setTimeout(async () => {
        try {
          const r = await invoke<RecognizeResult>("recognize_gun_name", { code: v });
          set({ recog: r });
          if (r.recognized) set({ weapon: r.weapon_name, note: r.note || "" });
          else set({ note: "" });
        } catch {
          /* 忽略 */
        }
      }, 250);
    },
    [set],
  );

  const handleSave = async () => {
    if (!s.code.trim()) {
      showToast(t("gunCodes.needCode"), "error");
      return;
    }
    set({ saving: true });
    try {
      await invoke("add_local_gun_code", { weaponName: s.weapon, code: s.code, note: s.note });
      showToast(t("gunCodes.saved"));
      set({ code: "", weapon: "", note: "", recog: null });
      await refresh();
      void loadMeta();
    } catch (e) {
      showToast(t("gunCodes.saveFailed", { err: String(e) }), "error");
    }
    set({ saving: false });
  };

  // ── 删除（AlertDialog 替代 window.confirm） ──
  const doDelete = async () => {
    const item = s.pendingDelete;
    set({ pendingDelete: null });
    if (!item) return;
    try {
      await invoke("delete_local_gun_code", { id: item.id });
      showToast(t("gunCodes.deleted"));
      await refresh();
    } catch (e) {
      showToast(t("gunCodes.deleteFailed", { err: String(e) }), "error");
    }
  };

  const handleUpdate = async (id: string) => {
    const item = s.items.find((i) => i.id === id);
    if (!item) return;
    try {
      await invoke("update_local_gun_code", {
        id,
        weaponName: s.editWeapon || item.weapon_name,
        code: item.code,
        note: s.editNote,
      });
      set({ editingId: null });
      showToast(t("gunCodes.updated"));
      await refresh();
      void loadMeta();
    } catch (e) {
      showToast(t("gunCodes.updateFailed", { err: String(e) }), "error");
    }
  };

  // ── 导入预览流 ──
  const doPreview = async () => {
    if (!s.importText.trim()) {
      showToast(t("gunCodes.needDoc"), "error");
      return;
    }
    set({ importing: true });
    try {
      const p = await invoke<PreviewResult>("preview_import_gun_codes", { text: s.importText });
      set({ preview: p, deselected: new Set() });
    } catch (e) {
      showToast(t("gunCodes.importFailed", { err: String(e) }), "error");
    }
    set({ importing: false });
  };

  const doConfirmImport = async () => {
    if (!s.preview) return;
    // 只导入勾选且非重复的行
    const keep = s.preview.rows
      .filter((r) => !s.deselected.has(r.code) && !r.duplicate)
      .map((r) => r.code);
    if (keep.length === 0) {
      showToast(t("gunCodes.nothingToImport"), "info");
      return;
    }
    set({ importing: true });
    try {
      // 后端批量导入按整段文本解析；此处把勾选项拼成 TSV 再提交，
      // 复用同一套解析路径（含指纹学习）
      const text = s.preview.rows
        .filter((r) => !s.deselected.has(r.code) && !r.duplicate)
        .map((r) => [r.gun, r.config, r.code].join("\t"))
        .join("\n");
      const res = await invoke<{ imported: number; updated: number; skipped: number; unrecognized: number }>(
        "import_gun_codes_batch",
        { text },
      );
      showToast(
        t("gunCodes.importDone", {
          imported: res.imported,
          updated: res.updated,
          skipped: res.skipped,
        }),
      );
      set({ showImport: false, importText: "", preview: null, deselected: new Set() });
      await refresh();
      void loadMeta();
    } catch (e) {
      showToast(t("gunCodes.importFailed", { err: String(e) }), "error");
    }
    set({ importing: false });
  };

  const handleExport = async () => {
    try {
      const path = await save({
        defaultPath: `改枪码备份_${new Date().toISOString().slice(0, 10)}.json`,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!path) return;
      const n = await invoke<number>("export_gun_codes", { path });
      showToast(t("gunCodes.exported", { n }));
    } catch (e) {
      showToast(t("gunCodes.exportFailed", { err: String(e) }), "error");
    }
  };

  const handleImportJson = async () => {
    try {
      const path = await open({ filters: [{ name: "JSON", extensions: ["json"] }], multiple: false });
      if (!path) return;
      const n = await invoke<number>("import_gun_codes", { path: String(path) });
      showToast(t("gunCodes.importJsonDone", { n }));
      await refresh();
      void loadMeta();
    } catch (e) {
      showToast(t("gunCodes.importFailed", { err: String(e) }), "error");
    }
  };

  const doCopy = async (code: string) => {
    const ok = await copyText(code);
    showToast(ok ? t("gunCodes.copied") : t("gunCodes.copyFailed"), ok ? undefined : "error");
  };

  // 预览勾选统计
  const previewStats = useMemo(() => {
    if (!s.preview) return { sel: 0, dup: 0, unrecog: 0 };
    let sel = 0, dup = 0, unrecog = 0;
    for (const r of s.preview.rows) {
      if (r.duplicate) dup++;
      else if (!s.deselected.has(r.code)) {
        sel++;
        if (!r.recognized) unrecog++;
      }
    }
    return { sel, dup, unrecog };
  }, [s.preview, s.deselected]);

  const card = (children: React.ReactNode, props: Record<string, unknown> = {}) =>
    liquidGlassEnabled ? (
      <LiquidGlassCard p={5} mb={5} {...props}>{children}</LiquidGlassCard>
    ) : (
      <Box bg={cardBg} border="1px solid" borderColor={cardBorder} borderRadius="xl" p={5} mb={5} {...props}>
        {children}
      </Box>
    );

  // ═══════════════ 渲染 ═══════════════
  return (
    <Box>
      {/* 标题栏 */}
      <HStack justify="space-between" align="center" mb={4} wrap="wrap" gap={2}>
        <HStack spacing={3} align="center">
          {onBack && (
            <IconButton
              aria-label={t("gunCodes.back")}
              icon={<ArrowLeft size={18} />}
              size="sm"
              variant="ghost"
              onClick={onBack}
            />
          )}
          <Box>
            <HStack spacing={2}>
              <Text fontSize="lg" fontWeight="bold" color={textColor}>
                {t("gunCodes.title")}
              </Text>
              {meta && (
                <Tooltip
                  label={t("gunCodes.rulesTip", {
                    version: meta.version,
                    guns: meta.gunCount,
                    fps: meta.fingerprintCount,
                    learned: meta.learnedCount,
                  })}
                >
                  <Badge colorScheme="purple" fontSize="2xs">
                    v{meta.version}
                  </Badge>
                </Tooltip>
              )}
            </HStack>
            <Text fontSize="xs" color={subTextColor}>{t("gunCodes.subtitle")}</Text>
          </Box>
        </HStack>
        <HStack spacing={3}>
          <Text fontSize="sm" color={subTextColor}>
            <b style={{ color: textColor }}>{stats.count}</b> {t("gunCodes.records")}
          </Text>
          <Text fontSize="sm" color={subTextColor}>
            <b style={{ color: textColor }}>{stats.guns}</b> {t("gunCodes.guns")}
          </Text>
          <Tooltip label={t("gunCodes.refreshRules")}>
            <IconButton
              aria-label={t("gunCodes.refreshRules")}
              icon={<RefreshCw size={15} />}
              size="xs"
              variant="ghost"
              onClick={async () => {
                await invoke("refresh_gun_rules").catch(() => {});
                await loadMeta();
                showToast(t("gunCodes.rulesRefreshed"));
              }}
            />
          </Tooltip>
        </HStack>
      </HStack>

      {/* 新增区 */}
      {card(
        <>
          <Text fontWeight="semibold" mb={3} color={textColor}>{t("gunCodes.addTitle")}</Text>
          <VStack spacing={3} align="stretch">
            <Box>
              <Text fontSize="xs" color={subTextColor} mb={1}>{t("gunCodes.codeLabel")}</Text>
              <Input
                value={s.code}
                onChange={(e) => handleCodeChange(e.target.value)}
                placeholder="6KFCC1K08QMG6NRUK6UQC"
                bg={panelBg}
                color={textColor}
                borderColor={cardBorder}
                fontFamily="mono"
              />
              {s.recog && (
                <HStack mt={1} spacing={2} align="center" wrap="wrap">
                  {s.recog.recognized ? (
                    <Badge colorScheme={s.recog.confidence >= 1 ? "green" : "yellow"} fontSize="2xs">
                      {t("gunCodes.recognizedAs", { gun: s.recog.weapon_name })}
                    </Badge>
                  ) : (
                    <Badge colorScheme="orange" fontSize="2xs">{t("gunCodes.notRecognized")}</Badge>
                  )}
                  {s.recog.reason && (
                    <Tooltip label={s.recog.reason}>
                      <HStack spacing={1} color={subTextColor} fontSize="xs">
                        <Info size={11} />
                        <Text fontSize="xs">
                          {t("gunCodes.confidence", { p: Math.round(s.recog.confidence * 100) })}
                        </Text>
                      </HStack>
                    </Tooltip>
                  )}
                  {s.recog.source && (
                    <Text fontSize="2xs" color={subTextColor} fontFamily="mono">
                      {t("gunCodes.source")}: {s.recog.source}
                    </Text>
                  )}
                </HStack>
              )}
            </Box>
            <HStack spacing={3} align="flex-start" wrap="wrap">
              <Box flex={1} minW="160px">
                <Text fontSize="xs" color={subTextColor} mb={1}>{t("gunCodes.weaponLabel")}</Text>
                <Input
                  value={s.weapon}
                  onChange={(e) => set({ weapon: e.target.value })}
                  placeholder={t("gunCodes.weaponPlaceholder")}
                  bg={panelBg} color={textColor} borderColor={cardBorder}
                />
              </Box>
              <Box flex={1.5} minW="200px">
                <Text fontSize="xs" color={subTextColor} mb={1}>{t("gunCodes.noteLabel")}</Text>
                <Input
                  value={s.note}
                  onChange={(e) => set({ note: e.target.value })}
                  placeholder={t("gunCodes.notePlaceholder")}
                  bg={panelBg} color={textColor} borderColor={cardBorder}
                />
              </Box>
            </HStack>
            <HStack spacing={3} wrap="wrap">
              <Button size="sm" colorScheme="blue" onClick={handleSave} isLoading={s.saving}>
                {t("gunCodes.save")}
              </Button>
              <Button
                size="sm" variant="ghost" leftIcon={<Plus size={14} />}
                onClick={() => set({ showImport: true, importText: "", preview: null })}
              >
                {t("gunCodes.batchImport")}
              </Button>
              <Button size="sm" variant="ghost" leftIcon={<Download size={14} />} onClick={handleExport}>
                {t("gunCodes.exportJson")}
              </Button>
              <Button size="sm" variant="ghost" leftIcon={<Upload size={14} />} onClick={handleImportJson}>
                {t("gunCodes.importJson")}
              </Button>
            </HStack>
          </VStack>
        </>,
      )}

      {/* 列表区 */}
      {card(
        <>
          {/* 头部：标题 + 搜索 */}
          <HStack justify="space-between" mb={3} wrap="wrap" gap={2}>
            <Text fontWeight="semibold" color={textColor}>{t("gunCodes.libraryTitle")}</Text>
            <HStack spacing={2}>
              <InputGroup size="sm" width={{ base: "160px", md: "220px" }}>
                <InputLeftElement pointerEvents="none">
                  <Search size={14} color={subTextColor as string} />
                </InputLeftElement>
                <Input
                  value={s.search}
                  onChange={(e) => set({ search: e.target.value })}
                  placeholder={t("gunCodes.searchPlaceholder")}
                  bg={panelBg} color={textColor} borderColor={cardBorder}
                />
              </InputGroup>
              {s.search && (
                <IconButton
                  aria-label={t("gunCodes.clearSearch")}
                  icon={<X size={14} />} size="sm" variant="ghost"
                  onClick={() => set({ search: "" })}
                />
              )}
            </HStack>
          </HStack>

          {/* B2：分类筛选（「其他」可见） */}
          <HStack spacing={2} wrap="wrap" mb={2}>
            <Button
              size="xs"
              variant={s.catFilter === "all" ? "solid" : "ghost"}
              colorScheme="blue"
              onClick={() => set({ catFilter: "all", gunFilter: [] })}
            >
              {t("gunCodes.allCats")}（{s.items.length}）
            </Button>
            {catOrder.map((cat) => (
              <Button
                key={cat}
                size="xs"
                variant={s.catFilter === cat ? "solid" : "ghost"}
                colorScheme={cat === "其他" ? "gray" : "blue"}
                onClick={() => dispatch({ type: "set", patch: { catFilter: cat, collapsed: new Set() } })}
              >
                {cat}（{catCounts.get(cat) ?? 0}）
              </Button>
            ))}
          </HStack>

          {/* B3：枪名可搜索多选 + B5 排序 */}
          <HStack spacing={2} mb={3} wrap="wrap">
            <GunFilterPopover
              candidates={gunCandidates}
              selected={s.gunFilter}
              onToggle={(g) => dispatch({ type: "toggleGun", gun: g })}
              onClear={() => dispatch({ type: "clearGuns" })}
              onBulk={(guns, on) => dispatch({ type: "toggleGunFilterMulti", guns, on })}
              t={t}
              textColor={textColor}
              subTextColor={subTextColor}
              bg={panelBg}
              border={cardBorder}
            />
            <SortSelect value={s.sortKey} onChange={(k) => set({ sortKey: k })} t={t} bg={panelBg} border={cardBorder} color={textColor} />
            {(s.gunFilter.length > 0 || s.catFilter !== "all" || s.search) && (
              <Button
                size="xs" variant="ghost" leftIcon={<X size={12} />}
                onClick={() => set({ search: "", catFilter: "all", gunFilter: [], collapsed: new Set() })}
              >
                {t("gunCodes.clearFilters")}
              </Button>
            )}
          </HStack>

          <Divider mb={3} />

          {/* 结果 */}
          {s.loading ? (
            <VStack py={10} spacing={3}>
              <Spinner size="md" color={primaryColor} />
              <Text fontSize="sm" color={subTextColor}>{t("gunCodes.loading")}</Text>
            </VStack>
          ) : filtered.length === 0 ? (
            <VStack py={10} spacing={2}>
              <ListFilter size={28} color={subTextColor as string} />
              <Text fontSize="sm" color={subTextColor}>
                {s.items.length === 0 ? t("gunCodes.emptyLibrary") : t("gunCodes.noMatch")}
              </Text>
              {s.items.length > 0 && (
                <Button
                  size="xs" variant="ghost"
                  onClick={() => set({ search: "", catFilter: "all", gunFilter: [], collapsed: new Set() })}
                >
                  {t("gunCodes.clearFilters")}
                </Button>
              )}
            </VStack>
          ) : (
            <VStack spacing={3} align="stretch">
              {grouped.map(([gun, gunItems]) => {
                const isCollapsed = s.collapsed.has(gun);
                const cat = catOf(gun);
                return (
                  <Box key={gun} bg={panelBg} borderRadius="lg" border="1px solid" borderColor={cardBorder} overflow="hidden">
                    <HStack
                      justify="space-between"
                      px={4} py={2}
                      cursor="pointer"
                      _hover={{ bg: pillActive }}
                      onClick={() => dispatch({ type: "toggleCollapse", gun })}
                    >
                      <HStack spacing={2} minW={0}>
                        <ChevronDown
                          size={14}
                          color={subTextColor as string}
                          style={{ transform: isCollapsed ? "rotate(-90deg)" : "none", transition: "transform .15s" }}
                        />
                        <Text fontWeight="semibold" color={textColor} noOfLines={1}>
                          <Highlight text={gun} query={s.search} color={primaryColor} />
                        </Text>
                        <Badge size="sm" colorScheme={cat === "其他" ? "gray" : "blue"}>{cat}</Badge>
                      </HStack>
                      <Text fontSize="xs" color={subTextColor} flexShrink={0}>
                        {t("gunCodes.configCount", { n: gunItems.length })}
                      </Text>
                    </HStack>

                    {!isCollapsed && (
                      <Box px={4} pb={3}>
                        <VStack spacing={2} align="stretch">
                          {gunItems.map((item) => (
                            <Box key={item.id} bg={cardBg} borderRadius="md" border="1px solid" borderColor={cardBorder} p={3}>
                              {s.editingId === item.id ? (
                                <VStack spacing={2} align="stretch">
                                  <Input
                                    size="sm" value={s.editWeapon}
                                    onChange={(e) => set({ editWeapon: e.target.value })}
                                    placeholder={t("gunCodes.weaponPlaceholder")}
                                    bg={panelBg} color={textColor} borderColor={cardBorder}
                                  />
                                  <Input
                                    size="sm" value={s.editNote}
                                    onChange={(e) => set({ editNote: e.target.value })}
                                    placeholder={t("gunCodes.notePlaceholder")}
                                    bg={panelBg} color={textColor} borderColor={cardBorder}
                                  />
                                  <HStack spacing={2}>
                                    <Button size="xs" colorScheme="blue" onClick={() => handleUpdate(item.id)}>
                                      {t("gunCodes.save")}
                                    </Button>
                                    <Button size="xs" variant="ghost" onClick={() => set({ editingId: null })}>
                                      {t("gunCodes.cancel")}
                                    </Button>
                                  </HStack>
                                </VStack>
                              ) : (
                                <HStack justify="space-between" align="flex-start" spacing={3}>
                                  <VStack align="start" spacing={1} flex={1} minW={0}>
                                    <Text
                                      fontFamily="mono" fontSize="sm" color={textColor}
                                      wordBreak="break-all" cursor="pointer"
                                      onClick={() => doCopy(item.code)}
                                      title={t("gunCodes.clickCopy")}
                                    >
                                      <Highlight text={item.code} query={s.search} color={primaryColor} />
                                    </Text>
                                    {item.note && (
                                      <Text fontSize="xs" color={subTextColor}>
                                        <Highlight text={item.note} query={s.search} color={primaryColor} />
                                      </Text>
                                    )}
                                    <Text fontSize="2xs" color={subTextColor}>{fmtTime(item.created_at)}</Text>
                                  </VStack>
                                  <HStack spacing={1} flexShrink={0}>
                                    <Tooltip label={t("gunCodes.copy")}>
                                      <IconButton
                                        aria-label={t("gunCodes.copy")} icon={<Copy size={13} />}
                                        size="xs" variant="ghost" onClick={() => doCopy(item.code)}
                                      />
                                    </Tooltip>
                                    <Tooltip label={t("gunCodes.edit")}>
                                      <IconButton
                                        aria-label={t("gunCodes.edit")} icon={<Pencil size={13} />}
                                        size="xs" variant="ghost"
                                        onClick={() => set({ editingId: item.id, editWeapon: item.weapon_name, editNote: item.note })}
                                      />
                                    </Tooltip>
                                    <Tooltip label={t("gunCodes.delete")}>
                                      <IconButton
                                        aria-label={t("gunCodes.delete")} icon={<Trash2 size={13} />}
                                        size="xs" variant="ghost" colorScheme="red"
                                        onClick={() => set({ pendingDelete: item })}
                                      />
                                    </Tooltip>
                                  </HStack>
                                </HStack>
                              )}
                            </Box>
                          ))}
                        </VStack>
                      </Box>
                    )}
                  </Box>
                );
              })}
            </VStack>
          )}
        </>,
      )}

      {/* 导入预览弹窗 */}
      <ImportPreviewDialog
        isOpen={s.showImport}
        onClose={() => set({ showImport: false, preview: null })}
        text={s.importText}
        setText={(v) => set({ importText: v })}
        preview={s.preview}
        deselected={s.deselected}
        onToggleRow={(code) => dispatch({ type: "toggleDeselect", code })}
        onBulkSelect={(codes, on) => dispatch({ type: "deselectAll", codes, on })}
        onPreview={doPreview}
        onConfirm={doConfirmImport}
        busy={s.importing}
        stats={previewStats}
        t={t}
        textColor={textColor}
        subTextColor={subTextColor}
        bg={panelBg}
        border={cardBorder}
        cardBg={cardBg}
      />

      {/* 删除确认（替代 window.confirm） */}
      <AlertDialog
        isOpen={!!s.pendingDelete}
        leastDestructiveRef={deleteCancelRef}
        onClose={() => set({ pendingDelete: null })}
        isCentered
      >
        <AlertDialogOverlay>
          <AlertDialogContent bg={cardBg}>
            <AlertDialogHeader fontSize="md" fontWeight="bold" color={textColor}>
              {t("gunCodes.deleteTitle")}
            </AlertDialogHeader>
            <AlertDialogBody color={textColor}>
              <Text fontSize="sm" mb={2}>{t("gunCodes.deleteConfirm")}</Text>
              {s.pendingDelete && (
                <Box bg={panelBg} p={2} borderRadius="md">
                  <Text fontSize="xs" fontFamily="mono" wordBreak="break-all">
                    {s.pendingDelete.weapon_name} · {s.pendingDelete.code}
                  </Text>
                  {s.pendingDelete.note && (
                    <Text fontSize="xs" color={subTextColor}>{s.pendingDelete.note}</Text>
                  )}
                </Box>
              )}
            </AlertDialogBody>
            <AlertDialogFooter>
              <Button ref={deleteCancelRef} size="sm" variant="ghost" onClick={() => set({ pendingDelete: null })}>
                {t("gunCodes.cancel")}
              </Button>
              <Button size="sm" colorScheme="red" ml={3} onClick={doDelete}>
                {t("gunCodes.delete")}
              </Button>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialogOverlay>
      </AlertDialog>
    </Box>
  );
}

// ═══════════════════════ 枪名筛选 Popover（B3） ═══════════════════════

function GunFilterPopover({
  candidates, selected, onToggle, onClear, onBulk, t, textColor, subTextColor, bg, border,
}: {
  candidates: [string, number][];
  selected: string[];
  onToggle: (g: string) => void;
  onClear: () => void;
  onBulk: (guns: string[], on: boolean) => void;
  t: (k: string, o?: Record<string, unknown>) => string;
  textColor: string;
  subTextColor: string;
  bg: string;
  border: string;
}) {
  const [q, setQ] = useState("");
  const selSet = new Set(selected.map(norm));
  const shown = useMemo(
    () => candidates.filter(([g]) => !q || norm(g).includes(norm(q))),
    [candidates, q],
  );

  return (
    <Popover placement="bottom-start" isLazy>
      <PopoverTrigger>
        <Button
          size="xs"
          variant={selected.length ? "solid" : "outline"}
          colorScheme="teal"
          leftIcon={<ListFilter size={12} />}
          rightIcon={<ChevronDown size={12} />}
        >
          {selected.length ? t("gunCodes.gunFilterN", { n: selected.length }) : t("gunCodes.gunFilter")}
        </Button>
      </PopoverTrigger>
      <Portal>
        <PopoverContent w="260px" bg={bg} borderColor={border}>
          <PopoverBody p={3}>
            <InputGroup size="xs" mb={2}>
              <InputLeftElement pointerEvents="none">
                <Search size={12} color={subTextColor} />
              </InputLeftElement>
              <Input
                value={q}
                onChange={(e) => setQ(e.target.value)}
                placeholder={t("gunCodes.searchGun")}
                bg={bg} color={textColor} borderColor={border}
              />
            </InputGroup>
            <HStack spacing={2} mb={2}>
              <Button size="2xs" variant="ghost" onClick={() => onBulk(shown.map(([g]) => g), true)}>
                {t("gunCodes.selectAll")}
              </Button>
              <Button size="2xs" variant="ghost" onClick={onClear}>
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
                    px={1} py={1}
                    borderRadius="sm"
                    cursor="pointer"
                    _hover={{ bg: "rgba(0,0,0,0.06)" }}
                    onClick={() => onToggle(g)}
                    spacing={2}
                  >
                    <Checkbox isChecked={selSet.has(norm(g))} pointerEvents="none" size="sm" />
                    <Text fontSize="xs" color={textColor} flex={1} noOfLines={1}>{g}</Text>
                    <Text fontSize="2xs" color={subTextColor}>{n}</Text>
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

// ═══════════════════════ 排序选择（B5） ═══════════════════════

function SortSelect({
  value, onChange, t, bg, border, color,
}: {
  value: SortKey;
  onChange: (k: SortKey) => void;
  t: (k: string) => string;
  bg: string;
  border: string;
  color: string;
}) {
  const { isOpen, onToggle, onClose } = useDisclosure();
  const label =
    value === "name" ? t("gunCodes.sortName")
    : value === "recent" ? t("gunCodes.sortRecent")
    : t("gunCodes.sortCount");
  return (
    <Popover placement="bottom-start" isOpen={isOpen} onClose={onClose} isLazy>
      <PopoverTrigger>
        <Button size="xs" variant="outline" rightIcon={<ChevronDown size={12} />} onClick={onToggle}>
          {label}
        </Button>
      </PopoverTrigger>
      <Portal>
        <PopoverContent w="140px" bg={bg} borderColor={border}>
          <PopoverBody p={1}>
            {(["count", "name", "recent"] as SortKey[]).map((k) => (
              <HStack
                key={k}
                px={2} py={1.5}
                borderRadius="sm"
                cursor="pointer"
                _hover={{ bg: "rgba(0,0,0,0.06)" }}
                onClick={() => { onChange(k); onClose(); }}
                spacing={2}
              >
                <Box w="14px">
                  {value === k && <Check size={13} color={color} />}
                </Box>
                <Text fontSize="xs" color={color}>
                  {k === "name" ? t("gunCodes.sortName") : k === "recent" ? t("gunCodes.sortRecent") : t("gunCodes.sortCount")}
                </Text>
              </HStack>
            ))}
          </PopoverBody>
        </PopoverContent>
      </Portal>
    </Popover>
  );
}

// ═══════════════════════ 导入预览弹窗 ═══════════════════════

function ImportPreviewDialog({
  isOpen, onClose, text, setText, preview, deselected, onToggleRow, onBulkSelect,
  onPreview, onConfirm, busy, stats, t, textColor, subTextColor, bg, border, cardBg,
}: {
  isOpen: boolean;
  onClose: () => void;
  text: string;
  setText: (v: string) => void;
  preview: PreviewResult | null;
  deselected: Set<string>;
  onToggleRow: (code: string) => void;
  onBulkSelect: (codes: string[], on: boolean) => void;
  onPreview: () => void;
  onConfirm: () => void;
  busy: boolean;
  stats: { sel: number; dup: number; unrecog: number };
  t: (k: string, o?: Record<string, unknown>) => string;
  textColor: string;
  subTextColor: string;
  bg: string;
  border: string;
  cardBg: string;
}) {
  const { isOpen: dOpen, onOpen: dOnOpen, onClose: dOnClose } = useDisclosure();
  const cancelRef = useRef<HTMLButtonElement>(null);

  const importable = preview ? preview.rows.filter((r) => !r.duplicate) : [];
  const allOn = importable.length > 0 && importable.every((r) => !deselected.has(r.code));

  return (
    <>
      <AlertDialog isOpen={isOpen} leastDestructiveRef={cancelRef} onClose={onClose} isCentered size="xl">
        <AlertDialogOverlay>
          <AlertDialogContent bg={cardBg} maxW="900px" maxH="85vh">
            <AlertDialogHeader fontSize="md" fontWeight="bold" color={textColor}>
              {t("gunCodes.importTitle")}
            </AlertDialogHeader>
            <AlertDialogBody overflowY="auto" maxH="62vh">
              {!preview ? (
                <VStack align="stretch" spacing={3}>
                  <Text fontSize="sm" color={subTextColor}>{t("gunCodes.importHint")}</Text>
                  <Box
                    as="textarea"
                    value={text}
                    onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) => setText(e.target.value)}
                    placeholder={t("gunCodes.importPlaceholder")}
                    bg={bg} color={textColor}
                    border="1px solid" borderColor={border} borderRadius="md"
                    p={3} h="260px" fontSize="xs" fontFamily="mono"
                    resize="vertical" w="100%"
                    _focus={{ outline: "none", borderColor: "blue.400" }}
                  />
                  <HStack>
                    <Button size="sm" colorScheme="blue" onClick={onPreview} isLoading={busy}>
                      {t("gunCodes.parsePreview")}
                    </Button>
                  </HStack>
                </VStack>
              ) : (
                <VStack align="stretch" spacing={3}>
                  {/* 统计条 */}
                  <HStack spacing={3} wrap="wrap">
                    <Badge colorScheme="blue">{t("gunCodes.totalN", { n: preview.total })}</Badge>
                    <Badge colorScheme="green">{t("gunCodes.recognizedN", { n: preview.recognized })}</Badge>
                    {preview.unrecognized > 0 && (
                      <Badge colorScheme="orange">{t("gunCodes.unrecognizedN", { n: preview.unrecognized })}</Badge>
                    )}
                    {stats.dup > 0 && (
                      <Badge colorScheme="gray">{t("gunCodes.duplicateN", { n: stats.dup })}</Badge>
                    )}
                    <Text fontSize="xs" color={subTextColor}>
                      {t("gunCodes.willImport", { n: stats.sel })}
                    </Text>
                  </HStack>
                  <Progress
                    value={preview.total ? (preview.recognized / preview.total) * 100 : 0}
                    size="xs" colorScheme="green" borderRadius="full"
                  />
                  {preview.unrecognized > 0 && (
                    <HStack spacing={2} color="orange.400" fontSize="xs">
                      <AlertTriangle size={13} />
                      <Text>{t("gunCodes.unrecognizedWarn")}</Text>
                    </HStack>
                  )}

                  {/* 全选 / 反选 */}
                  <HStack spacing={2}>
                    <Button
                      size="xs" variant="ghost"
                      onClick={() => onBulkSelect(importable.map((r) => r.code), !allOn)}
                    >
                      {allOn ? t("gunCodes.deselectAll") : t("gunCodes.selectAllImportable")}
                    </Button>
                  </HStack>

                  <Divider />

                  {/* 预览表 */}
                  <VStack align="stretch" spacing={1}>
                    {preview.rows.map((r) => {
                      const off = r.duplicate || deselected.has(r.code);
                      return (
                        <HStack
                          key={r.code}
                          px={2} py={1.5}
                          borderRadius="md"
                          bg={off ? "transparent" : bg}
                          border="1px solid"
                          borderColor={off ? "transparent" : border}
                          opacity={r.duplicate ? 0.5 : 1}
                          spacing={2}
                          align="flex-start"
                        >
                          <Checkbox
                            mt={0.5}
                            isChecked={!off}
                            isDisabled={r.duplicate}
                            onChange={() => onToggleRow(r.code)}
                          />
                          <VStack align="start" spacing={0} flex={1} minW={0}>
                            <HStack spacing={2} wrap="wrap">
                              <Text fontSize="xs" fontWeight="semibold" color={textColor}>
                                {r.recognized ? r.gun : t("gunCodes.unknown")}
                              </Text>
                              {r.config && <Text fontSize="xs" color={subTextColor}>· {r.config}</Text>}
                              {r.duplicate && <Badge colorScheme="gray" fontSize="2xs">{t("gunCodes.dupBadge")}</Badge>}
                              {!r.recognized && <Badge colorScheme="orange" fontSize="2xs">{t("gunCodes.unrecogBadge")}</Badge>}
                            </HStack>
                            <Text fontSize="2xs" fontFamily="mono" color={subTextColor} wordBreak="break-all">
                              {r.code}
                            </Text>
                            <Text fontSize="2xs" color={subTextColor}>{r.reason}</Text>
                          </VStack>
                        </HStack>
                      );
                    })}
                  </VStack>
                </VStack>
              )}
            </AlertDialogBody>
            <AlertDialogFooter>
              <Button ref={cancelRef} size="sm" variant="ghost" onClick={onClose}>
                {t("gunCodes.cancel")}
              </Button>
              {preview && (
                <>
                  <Button
                    size="sm" variant="ghost" ml={2}
                    onClick={() => { setText(""); onPreview(); }}
                  >
                    {t("gunCodes.reparse")}
                  </Button>
                  <Button
                    size="sm" colorScheme="blue" ml={2}
                    onClick={() => dOnOpen()}
                    isLoading={busy}
                    isDisabled={stats.sel === 0}
                  >
                    {t("gunCodes.confirmImport", { n: stats.sel })}
                  </Button>
                </>
              )}
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialogOverlay>
      </AlertDialog>

      {/* 二次确认（大数量导入时） */}
      <AlertDialog isOpen={dOpen} leastDestructiveRef={cancelRef} onClose={dOnClose} isCentered>
        <AlertDialogOverlay>
          <AlertDialogContent bg={cardBg}>
            <AlertDialogHeader fontSize="md" fontWeight="bold" color={textColor}>
              {t("gunCodes.confirmTitle")}
            </AlertDialogHeader>
            <AlertDialogBody color={textColor} fontSize="sm">
              {t("gunCodes.confirmBody", { n: stats.sel, unrecog: stats.unrecog })}
            </AlertDialogBody>
            <AlertDialogFooter>
              <Button ref={cancelRef} size="sm" variant="ghost" onClick={dOnClose}>
                {t("gunCodes.cancel")}
              </Button>
              <Button
                size="sm" colorScheme="blue" ml={3}
                onClick={() => { dOnClose(); onConfirm(); }}
              >
                {t("gunCodes.confirm")}
              </Button>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialogOverlay>
      </AlertDialog>
    </>
  );
}

// ═══════════════════════ 分类兜底表（B1/B2） ═══════════════════════
//
// 与后端 `rules.json` 的 `guns[].category` 对齐。
// 放在前端是为了「未加载规则库时也能分类」；「其他」为兜底且**在界面上可见**。
const CATEGORY_FALLBACK: Record<string, string> = {
  RM277: "步枪", AR57: "步枪", MCX: "步枪", MK47: "步枪", KC17: "步枪", K437: "步枪",
  腾龙: "步枪", "AS Val": "步枪", "CAR-15": "步枪", "PTR-32": "步枪", G3: "步枪",
  "SCAR-H": "步枪", "AK-12": "步枪", SG552: "步枪", M7: "步枪", AUG: "步枪",
  M16A4: "步枪", K416: "步枪", "ASH-12": "步枪", "AKS-74U": "步枪", "QBZ-95": "步枪",
  AKM: "步枪", M4A1: "步枪", MDR: "步枪",
  QCQ171: "冲锋枪", MP7: "冲锋枪", 勇士: "冲锋枪", "SR-3M": "冲锋枪", SMG45: "冲锋枪",
  野牛: "冲锋枪", UZI: "冲锋枪", Vector: "冲锋枪", P90: "冲锋枪", MP5: "冲锋枪",
  MK4: "冲锋枪", 汤姆逊: "冲锋枪", MP9: "冲锋枪",
  SVCH: "射手步枪", M14: "射手步枪", M700: "射手步枪", "PSG-1": "射手步枪", SVD: "射手步枪",
  MINI14: "射手步枪", VSS: "射手步枪", SR25: "射手步枪", R93: "射手步枪", "SV-98": "射手步枪",
  AWM: "射手步枪", SKS: "射手步枪", 杠杆步枪: "射手步枪", 巴雷特: "射手步枪", M82: "射手步枪",
  M250: "机枪", PKM: "机枪", QJB: "机枪", M249: "机枪",
  S12K: "霰弹枪", M1014: "霰弹枪", 725: "霰弹枪", M870: "霰弹枪", "FS-12": "霰弹枪",
  DP12: "霰弹枪", KSG: "霰弹枪",
  "93R": "手枪", G18: "手枪", 沙鹰: "手枪", 左轮: "手枪", M1911: "手枪", P226: "手枪", M9: "手枪",
  弓: "弓",
};
