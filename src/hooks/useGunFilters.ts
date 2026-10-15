import { useCallback, useEffect, useMemo, useState } from "react";
import {
  buildCatOrder,
  buildGunCandidates,
  canonicalOf,
  catOf,
  countByCategory,
  healCatFilter,
  healGunFilter,
  matchesQuery,
  norm,
  type GunCode,
  type SortKey,
  type ViewMode,
} from "@/lib/gunCodes";

export interface PersistedFilters {
  search?: string;
  catFilter?: string;
  gunFilter?: string[];
  sortKey?: SortKey;
  viewMode?: ViewMode;
}

/** 从已保存对象解析出筛选状态（供 store 载入后调用） */
export function parsePersisted(v: unknown): PersistedFilters {
  const o = (v ?? {}) as Record<string, unknown>;
  const sortKey =
    o.sortKey === "name" || o.sortKey === "recent" || o.sortKey === "count"
      ? (o.sortKey as SortKey)
      : "count";
  const viewMode =
    o.viewMode === "tree" || o.viewMode === "table" || o.viewMode === "tile"
      ? (o.viewMode as ViewMode)
      : "tree";
  return {
    search: typeof o.search === "string" ? o.search : "",
    catFilter: typeof o.catFilter === "string" ? o.catFilter : "all",
    gunFilter: Array.isArray(o.gunFilter) ? o.gunFilter.filter((x) => typeof x === "string") : [],
    sortKey,
    viewMode,
  };
}

/**
 * 筛选 / 排序 / 分组 / 自愈
 *
 * 从 `GunCodesNotebook.tsx` 提取（**只搬迁不重写**），行为保持一致：
 *  - 分类与枪名都按归一化比较（`AK-12` ≡ `AK12`）
 *  - 分类兜底「其他」始终可见
 *  - 切换分类会清空枪名筛选
 *  - 失效枪名 / 空分类自动回滚
 *
 * 新增：`viewMode`（树/表/磁贴）与 `rows`（表格视图用的扁平行）。
 */
export function useGunFilters(items: GunCode[], onPersist: (v: PersistedFilters) => void) {
  const [search, setSearch] = useState("");
  const [catFilter, setCatFilter] = useState("all");
  const [gunFilter, setGunFilter] = useState<string[]>([]);
  const [sortKey, setSortKey] = useState<SortKey>("count");
  const [viewMode, setViewMode] = useState<ViewMode>("tree");
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [hydrated, setHydrated] = useState(false);

  /** 载入已保存的筛选（由容器在 store 读完后调用） */
  const hydrate = useCallback((v: PersistedFilters) => {
    setSearch(v.search ?? "");
    setCatFilter(v.catFilter ?? "all");
    setGunFilter(v.gunFilter ?? []);
    setSortKey(v.sortKey ?? "count");
    setViewMode(v.viewMode ?? "tree");
    setHydrated(true);
  }, []);

  // 持久化（hydrate 之前不回写，避免把空状态覆盖掉已存的值）
  useEffect(() => {
    if (!hydrated) return;
    onPersist({ search, catFilter, gunFilter, sortKey, viewMode });
  }, [hydrated, search, catFilter, gunFilter, sortKey, viewMode, onPersist]);

  // ── 失效筛选自愈 ──
  // ① 枪名：剔除库中已不存在的
  useEffect(() => {
    const kept = healGunFilter(items, gunFilter);
    if (kept.length !== gunFilter.length) setGunFilter(kept);
  }, [items, gunFilter]);

  // ② 分类：已无条目时回退 all
  useEffect(() => {
    const healed = healCatFilter(items, catFilter);
    if (healed !== catFilter) setCatFilter(healed);
  }, [items, catFilter]);

  // ── 过滤 ──
  const filtered = useMemo(() => {
    const gunSet = new Set(gunFilter.map(norm));
    return items.filter((i) => {
      if (catFilter !== "all" && catOf(i.weapon_name) !== catFilter) return false;
      if (gunSet.size > 0 && !gunSet.has(norm(canonicalOf(i.weapon_name)))) return false;
      return matchesQuery(i, search);
    });
  }, [items, search, gunFilter, catFilter]);

  // ── 分组 + 排序 ──
  const grouped = useMemo(() => {
    const g = new Map<string, GunCode[]>();
    for (const it of filtered) {
      const key = canonicalOf(it.weapon_name);
      const arr = g.get(key) ?? [];
      arr.push(it);
      g.set(key, arr);
    }
    const entries = [...g.entries()];
    switch (sortKey) {
      case "name":
        entries.sort((a, b) => a[0].localeCompare(b[0], "zh-Hans-CN"));
        break;
      case "recent":
        entries.sort(
          (a, b) =>
            Math.max(...b[1].map((x) => x.created_at)) -
            Math.max(...a[1].map((x) => x.created_at)),
        );
        break;
      default:
        entries.sort((a, b) => b[1].length - a[1].length);
    }
    return entries;
  }, [filtered, sortKey]);

  /** 表格视图用：按分组顺序摊平（保持与树视图一致的排序） */
  const rows = useMemo(() => {
    const out: { item: GunCode; gun: string; cat: string }[] = [];
    for (const [gun, list] of grouped) {
      const cat = catOf(gun);
      for (const item of list) out.push({ item, gun, cat });
    }
    return out;
  }, [grouped]);

  const catCounts = useMemo(() => countByCategory(items), [items]);
  const catOrder = useMemo(() => buildCatOrder(catCounts), [catCounts]);
  const gunCandidates = useMemo(() => buildGunCandidates(items, catFilter), [items, catFilter]);

  const stats = useMemo(
    () => ({
      count: items.length,
      guns: new Set(items.map((i) => canonicalOf(i.weapon_name))).size,
    }),
    [items],
  );

  /** 状态栏摘要文案用的数据 */
  const filterSummary = useMemo(
    () => ({
      total: items.length,
      shown: filtered.length,
      guns: grouped.length,
      cat: catFilter,
      gunCount: gunFilter.length,
    }),
    [items.length, filtered.length, grouped.length, catFilter, gunFilter.length],
  );

  // ── 操作 ──
  const selectCat = useCallback((cat: string) => {
    // 切分类必须清空枪名筛选，否则若原选中的枪不属于新分类会得到空列表
    setCatFilter(cat);
    setGunFilter([]);
    setCollapsed(new Set());
  }, []);

  const toggleGun = useCallback((gun: string) => {
    setGunFilter((prev) =>
      prev.includes(gun) ? prev.filter((g) => g !== gun) : [...prev, gun],
    );
    setCollapsed(new Set());
  }, []);

  const setGuns = useCallback((guns: string[]) => {
    setGunFilter(guns);
    setCollapsed(new Set());
  }, []);

  const clearFilters = useCallback(() => {
    setSearch("");
    setCatFilter("all");
    setGunFilter([]);
    setCollapsed(new Set());
  }, []);

  const toggleCollapse = useCallback((gun: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(gun)) next.delete(gun);
      else next.add(gun);
      return next;
    });
  }, []);

  /** 全部展开 / 收起（工具栏用） */
  const setAllCollapsed = useCallback((collapseAll: boolean) => {
    setCollapsed(collapseAll ? new Set(grouped.map(([g]) => g)) : new Set());
  }, [grouped]);

  const hasActiveFilter = search !== "" || catFilter !== "all" || gunFilter.length > 0;

  return {
    // 状态
    search, catFilter, gunFilter, sortKey, viewMode, collapsed,
    // 派生
    filtered, grouped, rows, catCounts, catOrder, gunCandidates, stats,
    filterSummary, hasActiveFilter, hydrated,
    // 操作
    setSearch, setSortKey, setViewMode, selectCat, toggleGun, setGuns,
    clearFilters, toggleCollapse, setAllCollapsed, hydrate,
  };
}
