import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { setGunTable, type GunCode, type PreviewResult, type RecognizeResult, type RulesMeta } from "@/lib/gunCodes";
import type {
  LearnedList,
  RuleFingerprintList,
  RulesResetResult,
  RulesUpdateCheck,
  RulesUpdateResult,
} from "@/lib/gunCodes";

export interface ImportSummary {
  imported: number;
  updated: number;
  skipped: number;
  unrecognized: number;
}

/**
 * 改枪码数据层
 *
 * 所有 Tauri 命令调用集中于此，组件只消费状态与回调。
 *
 * 从 `GunCodesNotebook.tsx` 原样提取（**只搬迁不重写**），
 * 唯一新增是 `loadMeta()` 里注入后端下发的枪械分类表（见 `lib/gunCodes.ts`）。
 */
export function useGunCodes(onError: (msg: string) => void) {
  const [items, setItems] = useState<GunCode[]>([]);
  const [loading, setLoading] = useState(true);
  const [meta, setMeta] = useState<RulesMeta | null>(null);

  // 用 ref 存 onError，避免把它放进依赖导致 refresh 频繁重建
  const errRef = useRef(onError);
  useEffect(() => {
    errRef.current = onError;
  }, [onError]);

  const refresh = useCallback(async () => {
    try {
      const list = await invoke<GunCode[]>("get_local_gun_codes");
      setItems(list);
    } catch (e) {
      errRef.current(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  /**
   * 加载规则库元信息并注入分类表
   *
   * 注入后 `catOf()` 才能给出正确分类；「刷新词库」也走此路径，
   * 因此规则库热更后分类表自动同步。
   */
  const loadMeta = useCallback(async () => {
    try {
      const m = await invoke<RulesMeta>("get_gun_rules_meta");
      setMeta(m);
      if (m.guns && m.guns.length > 0) setGunTable(m.guns);
    } catch {
      /* 元信息失败不阻塞主流程（分类退回兜底表） */
    }
  }, []);

  useEffect(() => {
    void refresh();
    void loadMeta();
  }, [refresh, loadMeta]);

  // ── 识别（粘贴时实时调用）──
  const recognize = useCallback(
    (code: string) => invoke<RecognizeResult>("recognize_gun_name", { code }),
    [],
  );

  // ── 增删改 ──
  const add = useCallback(
    async (weaponName: string, code: string, note: string) => {
      await invoke("add_local_gun_code", { weaponName, code, note });
      await refresh();
      void loadMeta();
    },
    [refresh, loadMeta],
  );

  const update = useCallback(
    async (id: string, weaponName: string, code: string, note: string) => {
      await invoke("update_local_gun_code", { id, weaponName, code, note });
      await refresh();
      void loadMeta();
    },
    [refresh, loadMeta],
  );

  const remove = useCallback(
    async (id: string) => {
      await invoke("delete_local_gun_code", { id });
      await refresh();
    },
    [refresh],
  );

  /** 批量删除（逐条调用后端，失败不中断，返回成功数） */
  const removeMany = useCallback(
    async (ids: string[]) => {
      let okCount = 0;
      for (const id of ids) {
        try {
          await invoke("delete_local_gun_code", { id });
          okCount++;
        } catch {
          /* 单条失败不中断整批 */
        }
      }
      await refresh();
      return okCount;
    },
    [refresh],
  );

  // ── 导入 ──
  /** 解析预览（**不写库**） */
  const preview = useCallback(
    (text: string) => invoke<PreviewResult>("preview_import_gun_codes", { text }),
    [],
  );

  /** 提交导入（把勾选项拼成 TSV 复用同一解析路径） */
  const importRows = useCallback(
    async (rows: { gun: string; config: string; code: string }[]) => {
      const text = rows.map((r) => [r.gun, r.config, r.code].join("\t")).join("\n");
      const res = await invoke<ImportSummary>("import_gun_codes_batch", { text });
      await refresh();
      void loadMeta();
      return res;
    },
    [refresh, loadMeta],
  );

  // ── 备份 ──
  const exportJson = useCallback(async () => {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const path = await save({
      defaultPath: `改枪码备份_${new Date().toISOString().slice(0, 10)}.json`,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) return null;
    return invoke<number>("export_gun_codes", { path });
  }, []);

  const importJson = useCallback(async () => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const path = await open({
      filters: [{ name: "JSON", extensions: ["json"] }],
      multiple: false,
    });
    if (!path) return null;
    const n = await invoke<number>("import_gun_codes", { path: String(path) });
    await refresh();
    void loadMeta();
    return n;
  }, [refresh, loadMeta]);

  // ── 词库 ──
  const refreshRules = useCallback(async () => {
    await invoke("refresh_gun_rules");
    await loadMeta();
  }, [loadMeta]);

  // ── 词库在线热更新（v10.3.0）──

  /** 检查是否有更新（只比对，不落盘） */
  const checkRulesUpdate = useCallback(
    () => invoke<RulesUpdateCheck>("check_gun_rules_update"),
    [],
  );

  /** 下载并应用远程词库（本地优先合并） */
  const updateRules = useCallback(async () => {
    const res = await invoke<RulesUpdateResult>("update_gun_rules");
    await loadMeta();
    return res;
  }, [loadMeta]);

  /** 清除合并结果，回到随包分发的词库 */
  const resetRules = useCallback(async () => {
    const res = await invoke<RulesResetResult>("reset_gun_rules");
    await loadMeta();
    return res;
  }, [loadMeta]);

  // ── 学习表管理（v10.3.0）──

  const listLearned = useCallback(
    () => invoke<LearnedList>("list_learned_fingerprints"),
    [],
  );

  const deleteLearned = useCallback(
    (fingerprint: string) =>
      invoke<{ ok: boolean; removed?: number; remaining?: number; error?: string }>(
        "delete_learned_fingerprint",
        { fingerprint },
      ),
    [],
  );

  const clearLearned = useCallback(
    () => invoke<{ ok: boolean; cleared: number }>("clear_learned_fingerprints"),
    [],
  );

  /** 只清除学习表里遗留的备注，保留枪名 */
  const clearLearnedNotes = useCallback(
    () => invoke<{ ok: boolean; cleaned: number; total?: number }>("clear_learned_notes"),
    [],
  );

  // ── 规则库条目管理（v10.4.0）──

  const listRuleFingerprints = useCallback(
    (query = "", page = 1, pageSize = 50) =>
      invoke<RuleFingerprintList>("list_rule_fingerprints", { query, page, pageSize }),
    [],
  );

  const deleteRuleFingerprint = useCallback(
    async (fingerprint: string) => {
      const r = await invoke<{ ok: boolean; error?: string }>("delete_rule_fingerprint", { fingerprint });
      await loadMeta();
      return r;
    },
    [loadMeta],
  );

  const updateRuleFingerprint = useCallback(
    async (fingerprint: string, gun: string) => {
      const r = await invoke<{ ok: boolean; error?: string }>("update_rule_fingerprint", { fingerprint, gun });
      await loadMeta();
      return r;
    },
    [loadMeta],
  );

  const clearOnlineRules = useCallback(async () => {
    const r = await invoke<{ ok: boolean; error?: string; removed?: boolean }>("clear_online_rules_cmd");
    await loadMeta();
    return r;
  }, [loadMeta]);

  const clearRuleOverrides = useCallback(async () => {
    const r = await invoke<{ ok: boolean; error?: string; removed?: boolean }>("clear_rule_overrides");
    await loadMeta();
    return r;
  }, [loadMeta]);

  return {
    items, loading, meta,
    refresh, loadMeta,
    recognize,
    add, update, remove, removeMany,
    preview, importRows,
    exportJson, importJson,
    refreshRules,
    checkRulesUpdate, updateRules, resetRules,
    listLearned, deleteLearned, clearLearned, clearLearnedNotes,
    listRuleFingerprints, deleteRuleFingerprint, updateRuleFingerprint,
    clearOnlineRules, clearRuleOverrides,
  };
}
