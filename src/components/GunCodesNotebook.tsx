import { useState, useEffect, useRef, useCallback, useMemo } from "react";
import {
  Box,
  Text,
  Heading,
  VStack,
  HStack,
  useColorModeValue,
  Input,
  InputGroup,
  Button,
  Textarea,
  Badge,
  Divider,
  IconButton,
} from "@chakra-ui/react";
import { invoke } from "@tauri-apps/api/core";
import { save, open } from "@tauri-apps/plugin-dialog";
import { ArrowLeft, Search, Copy, Pencil, Trash2, Plus, Download, Upload } from "lucide-react";
import { useDynamicIsland } from "@/components/ui/dynamic-island";
import { useBackground } from "@/contexts/background-context";

// ── 类型（与后端 local_gun_codes.rs 对齐） ──
interface GunCode {
  id: string;
  weapon_name: string;
  code: string;
  note: string;
  created_at: number;
}

interface ImportResult {
  weapon_name: string;
  note: string;
  recognized: boolean;
}

// ── 枪械类别映射 ──
const CATEGORY_MAP: Record<string, string> = {
  "RM277": "步枪", "AR57": "步枪", "MCX": "步枪", "MK47": "步枪", "KC17": "步枪",
  "K437": "步枪", "腾龙": "步枪", "AS Val": "步枪", "CAR-15": "步枪", "PTR-32": "步枪",
  "G3": "步枪", "SCAR-H": "步枪", "AK-12": "步枪", "SG552": "步枪", "M7": "步枪",
  "AUG": "步枪", "M16A4": "步枪", "K416": "步枪", "ASH-12": "步枪", "AKS-74U": "步枪",
  "QBZ-95": "步枪", "AKM": "步枪", "M4A1": "步枪",
  "QCQ171": "冲锋枪", "MP7": "冲锋枪", "勇士": "冲锋枪", "SR-3M": "冲锋枪", "SMG45": "冲锋枪",
  "野牛": "冲锋枪", "UZI": "冲锋枪", "Vector": "冲锋枪", "P90": "冲锋枪", "MP5": "冲锋枪", "MK4": "冲锋枪",
  "SVCH": "射手步枪", "M14": "射手步枪", "M700": "射手步枪", "PSG-1": "射手步枪", "SVD": "射手步枪",
  "MINI14": "射手步枪", "VSS": "射手步枪", "SR25": "射手步枪", "R93": "射手步枪", "SV-98": "射手步枪",
  "AWM": "射手步枪", "SKS": "射手步枪", "杠杆步枪": "射手步枪", "巴雷特": "射手步枪",
  "M250": "机枪", "PKM": "机枪", "QJB": "机枪", "M249": "机枪",
  "S12K": "霰弹枪", "M1014": "霰弹枪", "725": "霰弹枪", "M870": "霰弹枪", "FS-12": "霰弹枪",
  "93R": "手枪", "G18": "手枪", "沙鹰": "手枪", "左轮": "手枪", "M1911": "手枪",
  "弓": "弓",
};
const CATEGORY_ORDER = ["步枪", "冲锋枪", "射手步枪", "机枪", "霰弹枪", "手枪", "弓"];

// ── 小工具 ──
function fmtTime(sec: number): string {
  if (!sec) return "";
  const d = new Date(sec * 1000);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

async function copyText(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch (_) { /* fallthrough */ }
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
  } catch (_) {
    return false;
  }
}

interface Props {
  /** 返回回调：记事本内点击返回键时触发 */
  onBack?: () => void;
}

// ── 主组件：本地改枪码记事本 ──
export default function GunCodesNotebook({ onBack }: Props) {
  const toast = useDynamicIsland("target");
  const { liquidGlassEnabled } = useBackground();
  const textColor = useColorModeValue("#000000", "#ffffff");
  const subTextColor = useColorModeValue("#666666", "#aaaaaa");
  const cardBg = useColorModeValue("white", "#161616");
  const cardBorder = useColorModeValue("gray.200", "#333333");
  const panelBg = useColorModeValue("gray.50", "#1a1a1a");
  const pillActive = useColorModeValue("#00000011", "#ffffff22");

  const [items, setItems] = useState<GunCode[]>([]);
  const [search, setSearch] = useState("");
  const [gunFilter, setGunFilter] = useState("all");
  const [catFilter, setCatFilter] = useState("all");
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [code, setCode] = useState("");
  const [weapon, setWeapon] = useState("");
  const [note, setNote] = useState("");
  const [recog, setRecog] = useState<ImportResult | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editWeapon, setEditWeapon] = useState("");
  const [editNote, setEditNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [showImport, setShowImport] = useState(false);
  const [importText, setImportText] = useState("");
  const [importing, setImporting] = useState(false);
  const recogTimer = useRef<number | null>(null);

  const showToast = useCallback((msg: string, type?: "error") => {
    toast({ description: msg, status: type === "error" ? "error" : "success" });
  }, [toast]);

  const refresh = useCallback(async () => {
    try {
      const list = await invoke<GunCode[]>("get_local_gun_codes");
      setItems(list);
    } catch (e) {
      showToast("加载失败: " + e, "error");
    }
  }, [showToast]);

  useEffect(() => { refresh(); }, [refresh]);

  // 粘贴/输入代码时自动识别枪名
  const handleCodeChange = useCallback((v: string) => {
    setCode(v);
    const clean = v.replace(/[^a-zA-Z0-9]/g, "");
    if (clean.length >= 6) {
      if (recogTimer.current) window.clearTimeout(recogTimer.current);
      recogTimer.current = window.setTimeout(async () => {
        try {
          const r = await invoke<ImportResult>("recognize_gun_name", { code: v });
          setRecog(r);
          if (r.recognized) {
            setWeapon(r.weapon_name);
            setNote(r.note || "");
          } else {
            setNote("");
          }
        } catch (_) { /* ignore */ }
      }, 300);
    } else {
      setRecog(null);
      setWeapon("");
      setNote("");
    }
  }, []);

  const handleSave = async () => {
    if (!code.trim()) { showToast("请先粘贴改枪码", "error"); return; }
    setBusy(true);
    try {
      await invoke("add_local_gun_code", { weaponName: weapon, code, note });
      showToast("已保存 ✓");
      setCode(""); setWeapon(""); setNote(""); setRecog(null);
      await refresh();
    } catch (e) {
      showToast("保存失败: " + e, "error");
    }
    setBusy(false);
  };

  const handleDelete = async (id: string) => {
    if (!window.confirm("确定删除这条改枪码？")) return;
    try {
      await invoke("delete_local_gun_code", { id });
      showToast("已删除");
      await refresh();
    } catch (e) { showToast("删除失败: " + e, "error"); }
  };

  const handleUpdate = async (id: string) => {
    const item = items.find((i) => i.id === id);
    if (!item) return;
    try {
      await invoke("update_local_gun_code", {
        id, weaponName: editWeapon || item.weapon_name, code: item.code, note: editNote,
      });
      setEditingId(null);
      showToast("已更新 ✓");
      await refresh();
    } catch (e) { showToast("更新失败: " + e, "error"); }
  };

  const handleBatchImport = async () => {
    if (!importText.trim()) { showToast("请先粘贴文档内容", "error"); return; }
    setImporting(true);
    try {
      const res = await invoke<{ imported: number; updated: number; skipped: number }>("import_gun_codes_batch", { text: importText });
      showToast(`批量导入完成：新增 ${res.imported} 条${res.updated ? `，补上备注 ${res.updated} 条` : ""}${res.skipped ? `，跳过 ${res.skipped} 条` : ""}`);
      setShowImport(false);
      setImportText("");
      await refresh();
    } catch (e) {
      showToast("批量导入失败: " + e, "error");
    }
    setImporting(false);
  };

  const handleExport = async () => {
    try {
      const path = await save({
        title: "导出改枪码备份",
        defaultPath: `改枪码备份_${new Date().toISOString().slice(0, 10)}.json`,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!path) return;
      const n = await invoke<number>("export_gun_codes", { path });
      showToast(`已导出 ${n} 条记录 ✓`);
    } catch (e) {
      showToast("导出失败: " + e, "error");
    }
  };

  const handleImportJson = async () => {
    try {
      const path = await open({
        title: "导入改枪码备份",
        multiple: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!path) return;
      const n = await invoke<number>("import_gun_codes", { path: String(path) });
      showToast(`导入完成：新增 ${n} 条${n === 0 ? "（无新记录）" : ""}`);
      await refresh();
    } catch (e) {
      showToast("导入失败: " + e, "error");
    }
  };

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    return items.filter((i) => {
      if (catFilter !== "all" && CATEGORY_MAP[i.weapon_name] !== catFilter) return false;
      if (gunFilter !== "all" && i.weapon_name !== gunFilter) return false;
      if (!q) return true;
      return (
        i.weapon_name.toLowerCase().includes(q) ||
        i.code.toLowerCase().includes(q) ||
        i.note.toLowerCase().includes(q)
      );
    });
  }, [items, search, gunFilter, catFilter]);

  const gunOptions = useMemo(() => {
    const counts = new Map<string, number>();
    for (const i of items) {
      if (catFilter !== "all" && CATEGORY_MAP[i.weapon_name] !== catFilter) continue;
      counts.set(i.weapon_name, (counts.get(i.weapon_name) || 0) + 1);
    }
    return [...counts.entries()].sort((a, b) => b[1] - a[1]).map(([name]) => name);
  }, [items, catFilter]);

  const grouped = useMemo(() => {
    const groups = new Map<string, GunCode[]>();
    for (const item of filtered) {
      const arr = groups.get(item.weapon_name) || [];
      arr.push(item);
      groups.set(item.weapon_name, arr);
    }
    return [...groups.entries()];
  }, [filtered]);

  const catCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const i of items) {
      const c = CATEGORY_MAP[i.weapon_name] || "其他";
      counts.set(c, (counts.get(c) || 0) + 1);
    }
    return counts;
  }, [items]);

  const stats = useMemo(() => {
    const guns = new Set(items.map((i) => i.weapon_name));
    return { count: items.length, guns: guns.size };
  }, [items]);

  const wrapperBg = liquidGlassEnabled ? undefined : cardBg;

  return (
    <Box>
      {/* 标题栏 + 返回键 */}
      <HStack justify="space-between" align="center" mb={4}>
        <HStack spacing={3} align="center">
          {onBack && (
            <IconButton
              aria-label="返回"
              icon={<ArrowLeft size={18} />}
              size="sm"
              variant="ghost"
              onClick={onBack}
            />
          )}
          <Box>
            <Heading size="md" color={textColor}>三角洲本地改枪码记事本</Heading>
            <Text fontSize="xs" color={subTextColor}>本地改枪码库 · 粘贴自动识别</Text>
          </Box>
        </HStack>
        <HStack spacing={3}>
          <Text fontSize="sm" color={subTextColor}><b style={{ color: textColor }}>{stats.count}</b> 条记录</Text>
          <Text fontSize="sm" color={subTextColor}><b style={{ color: textColor }}>{stats.guns}</b> 把枪</Text>
        </HStack>
      </HStack>

      {/* 输入区 */}
      <Box bg={wrapperBg} border={liquidGlassEnabled ? undefined : "1px solid"} borderColor={cardBorder} borderRadius="xl" p={5} mb={5}>
        <Heading size="sm" mb={3} color={textColor}>添加改枪码</Heading>
        <VStack spacing={3} align="stretch">
          <Box>
            <Text fontSize="xs" color={subTextColor} mb={1}>改枪码（粘贴后自动识别枪名）</Text>
            <Textarea
              value={code}
              onChange={(e) => handleCodeChange(e.target.value)}
              placeholder="例如 6KFCC1K08QMG6NRUK6UQC"
              rows={2}
              bg={cardBg}
              color={textColor}
              borderColor={cardBorder}
            />
            {recog && (
              <Text fontSize="xs" mt={1} color={recog.recognized ? "green.400" : "orange.300"}>
                {recog.recognized
                  ? <>已识别：<b>{recog.weapon_name}</b></>
                  : "未识别出枪名，可手动输入后保存，下次同枪代码将自动识别"}
              </Text>
            )}
          </Box>
          <HStack spacing={3} align="flex-start">
            <Box flex={1}>
              <Text fontSize="xs" color={subTextColor} mb={1}>枪名</Text>
              <Input value={weapon} onChange={(e) => setWeapon(e.target.value)} placeholder="枪名" bg={cardBg} color={textColor} borderColor={cardBorder} />
            </Box>
            <Box flex={1.5}>
              <Text fontSize="xs" color={subTextColor} mb={1}>备注</Text>
              <Input value={note} onChange={(e) => setNote(e.target.value)} placeholder="例如 20W青春版 / 55W满改红点" bg={cardBg} color={textColor} borderColor={cardBorder} />
            </Box>
          </HStack>
          <HStack spacing={3} wrap="wrap">
            <Button size="sm" colorScheme="blue" onClick={handleSave} isLoading={busy}>保存</Button>
            <Button size="sm" variant="ghost" onClick={() => setShowImport(true)} leftIcon={<Plus size={14} />}>批量导入</Button>
            <Button size="sm" variant="ghost" onClick={handleExport} leftIcon={<Download size={14} />}>导出 JSON</Button>
            <Button size="sm" variant="ghost" onClick={handleImportJson} leftIcon={<Upload size={14} />}>导入 JSON</Button>
          </HStack>
        </VStack>
      </Box>

      {/* 列表区 */}
      <Box bg={wrapperBg} border={liquidGlassEnabled ? undefined : "1px solid"} borderColor={cardBorder} borderRadius="xl" p={5}>
        <HStack justify="space-between" mb={3}>
          <Heading size="sm" color={textColor}>我的改枪码库</Heading>
          <InputGroup size="sm" width="220px">
            <Input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="搜索 枪名 / 代码 / 备注…"
              bg={cardBg}
              color={textColor}
              borderColor={cardBorder}
            />
          </InputGroup>
        </HStack>

        {/* 类别筛选 */}
        <HStack spacing={2} wrap="wrap" mb={2}>
          <Button
            size="xs"
            variant={catFilter === "all" ? "solid" : "ghost"}
            colorScheme="blue"
            onClick={() => { setCatFilter("all"); setGunFilter("all"); }}
          >
            全部（{items.length}）
          </Button>
          {CATEGORY_ORDER.map((cat) => {
            const cnt = catCounts.get(cat) || 0;
            if (cnt === 0) return null;
            return (
              <Button
                key={cat}
                size="xs"
                variant={catFilter === cat ? "solid" : "ghost"}
                colorScheme="blue"
                onClick={() => { setCatFilter(cat); setGunFilter("all"); }}
              >
                {cat}（{cnt}）
              </Button>
            );
          })}
        </HStack>

        {/* 枪筛选 */}
        {gunOptions.length > 0 && (
          <HStack spacing={2} wrap="wrap" mb={3}>
            <Button
              size="xs"
              variant={gunFilter === "all" ? "solid" : "ghost"}
              colorScheme="teal"
              onClick={() => setGunFilter("all")}
            >
              全部枪
            </Button>
            {gunOptions.map((name) => (
              <Button
                key={name}
                size="xs"
                variant={gunFilter === name ? "solid" : "ghost"}
                colorScheme="teal"
                onClick={() => setGunFilter(name)}
              >
                {name}
              </Button>
            ))}
          </HStack>
        )}

        <Divider mb={3} />

        {filtered.length === 0 ? (
          <Text fontSize="sm" color={subTextColor} textAlign="center" py={8}>
            {items.length === 0 ? "还没有记录，粘贴一条改枪码开始吧" : "没有匹配的结果"}
          </Text>
        ) : (
          <VStack spacing={3} align="stretch">
            {grouped.map(([gun, gunItems]) => {
              const isCollapsed = collapsed.has(gun);
              return (
                <Box key={gun} bg={panelBg} borderRadius="lg" border="1px solid" borderColor={cardBorder} overflow="hidden">
                  <HStack
                    justify="space-between"
                    px={4}
                    py={2}
                    cursor="pointer"
                    onClick={() => {
                      setCollapsed((prev) => {
                        const next = new Set(prev);
                        if (next.has(gun)) next.delete(gun);
                        else next.add(gun);
                        return next;
                      });
                    }}
                    _hover={{ bg: pillActive }}
                  >
                    <HStack spacing={2}>
                      <Text color={textColor}>{isCollapsed ? "▸" : "▾"}</Text>
                      <Text fontWeight="semibold" color={textColor}>{gun}</Text>
                      {CATEGORY_MAP[gun] && <Badge size="sm" colorScheme="blue">{CATEGORY_MAP[gun]}</Badge>}
                    </HStack>
                    <Text fontSize="xs" color={subTextColor}>{gunItems.length} 条配置</Text>
                  </HStack>
                  {!isCollapsed && (
                    <Box px={4} pb={3}>
                      <VStack spacing={2} align="stretch">
                        {gunItems.map((item) => (
                          <Box key={item.id} bg={cardBg} borderRadius="md" border="1px solid" borderColor={cardBorder} p={3}>
                            {editingId === item.id ? (
                              <VStack spacing={2} align="stretch">
                                <Input size="sm" value={editWeapon} onChange={(e) => setEditWeapon(e.target.value)} placeholder="枪名" bg={panelBg} color={textColor} borderColor={cardBorder} />
                                <Input size="sm" value={editNote} onChange={(e) => setEditNote(e.target.value)} placeholder="备注" bg={panelBg} color={textColor} borderColor={cardBorder} />
                                <HStack spacing={2}>
                                  <Button size="xs" colorScheme="blue" onClick={() => handleUpdate(item.id)}>保存修改</Button>
                                  <Button size="xs" variant="ghost" onClick={() => setEditingId(null)}>取消</Button>
                                </HStack>
                              </VStack>
                            ) : (
                              <>
                                <HStack justify="space-between" mb={1}>
                                  <Text fontSize="xs" color={subTextColor}>{item.note || "无备注"}</Text>
                                  <Text fontSize="xs" color={subTextColor}>{fmtTime(item.created_at)}</Text>
                                </HStack>
                                <Text fontFamily="monospace" fontSize="sm" color={textColor} wordBreak="break-all" mb={2}>{item.code}</Text>
                                <HStack spacing={2}>
                                  <Button size="xs" leftIcon={<Copy size={12} />} onClick={async () => {
                                    if (await copyText(item.code)) showToast("已复制 ✓");
                                    else showToast("复制失败", "error");
                                  }}>复制</Button>
                                  <Button size="xs" variant="ghost" leftIcon={<Pencil size={12} />} onClick={() => {
                                    setEditingId(item.id);
                                    setEditWeapon(item.weapon_name);
                                    setEditNote(item.note);
                                  }}>编辑</Button>
                                  <Button size="xs" variant="ghost" colorScheme="red" leftIcon={<Trash2 size={12} />} onClick={() => handleDelete(item.id)}>删除</Button>
                                </HStack>
                              </>
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
      </Box>

      {/* 批量导入弹窗 */}
      {showImport && (
        <Box
          position="fixed"
          top={0} left={0} right={0} bottom={0}
          bg="blackAlpha.600"
          zIndex={1400}
          display="flex"
          alignItems="center"
          justifyContent="center"
          onClick={() => setShowImport(false)}
        >
          <Box bg={cardBg} borderRadius="xl" p={5} maxW="560px" w="90%" onClick={(e) => e.stopPropagation()}>
            <Heading size="sm" mb={2} color={textColor}>批量导入改枪码</Heading>
            <Text fontSize="xs" color={subTextColor} mb={3}>
              从在线文档/群里复制整段内容（枪名 + 配置 + 改枪码），粘贴到下面即可。自动识别每一条并保存。
            </Text>
            <Textarea
              value={importText}
              onChange={(e) => setImportText(e.target.value)}
              placeholder={"例如：\nRM277 20W青春版 6KFCC1K08QMG6NRUK6UQC\nMK47突击步枪-烽火地带-6HN54R808PBNCS3LFEE0A"}
              rows={10}
              bg={panelBg}
              color={textColor}
              borderColor={cardBorder}
            />
            <HStack mt={3} spacing={3}>
              <Button size="sm" colorScheme="blue" onClick={handleBatchImport} isLoading={importing}>
                {importing ? "导入中…" : "开始导入"}
              </Button>
              <Button size="sm" variant="ghost" onClick={() => setShowImport(false)}>取消</Button>
            </HStack>
          </Box>
        </Box>
      )}
    </Box>
  );
}
