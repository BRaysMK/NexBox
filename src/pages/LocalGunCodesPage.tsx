import {
  Box,
  Text,
  Heading,
  VStack,
  HStack,
  useColorModeValue,
  Badge,
  SimpleGrid,
  Input,
  Button,
  Modal,
  ModalOverlay,
  ModalContent,
  ModalHeader,
  ModalBody,
  ModalFooter,
  ModalCloseButton,
  Textarea,
  FormControl,
  FormLabel,
  IconButton,
  useToast,
} from "@chakra-ui/react";
import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save, open } from "@tauri-apps/plugin-dialog";
import { Link } from "react-router-dom";
import { Search, Copy, Check, Plus, Trash2, Pencil, Save, X, ChevronLeft, Download, Upload } from "lucide-react";
import { LiquidGlassCard } from "@/components/special/liquid-glass-card";

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

function fmtTime(sec: number): string {
  if (!sec) return "";
  const d = new Date(sec * 1000);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

export default function LocalGunCodesPage() {
  const bg = useColorModeValue("rgba(255,255,255,0.06)", "rgba(0,0,0,0.25)");
  const border = useColorModeValue("rgba(0,0,0,0.08)", "rgba(255,255,255,0.08)");
  const muted = useColorModeValue("gray.500", "gray.400");
  const toast = useToast();

  const [items, setItems] = useState<GunCode[]>([]);
  const [search, setSearch] = useState("");
  const [code, setCode] = useState("");
  const [weapon, setWeapon] = useState("");
  const [note, setNote] = useState("");
  const [recog, setRecog] = useState<ImportResult | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editWeapon, setEditWeapon] = useState("");
  const [editNote, setEditNote] = useState("");
  const [showImport, setShowImport] = useState(false);
  const [importText, setImportText] = useState("");
  const [busy, setBusy] = useState(false);
  const [importing, setImporting] = useState(false);
  const [gunFilter, setGunFilter] = useState("all");
  const [catFilter, setCatFilter] = useState("all");
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const recogTimer = useRef<number | null>(null);

  const showToast = useCallback((msg: string, status: "success" | "error" = "success") => {
    toast({ title: msg, status, duration: 1800, isClosable: true, position: "top" });
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
    } catch (e) { showToast("保存失败: " + e, "error"); }
    setBusy(false);
  };

  const handleDelete = async (id: string) => {
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
    } catch (e) { showToast("批量导入失败: " + e, "error"); }
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
    } catch (e) { showToast("导出失败: " + e, "error"); }
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
    } catch (e) { showToast("导入失败: " + e, "error"); }
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

  const catCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const i of items) {
      const c = CATEGORY_MAP[i.weapon_name] || "其他";
      counts.set(c, (counts.get(c) || 0) + 1);
    }
    return counts;
  }, [items]);

  // 按枪分组
  const grouped = useMemo(() => {
    const groups = new Map<string, GunCode[]>();
    for (const item of filtered) {
      const arr = groups.get(item.weapon_name) || [];
      arr.push(item);
      groups.set(item.weapon_name, arr);
    }
    return [...groups.entries()];
  }, [filtered]);

  const stats = useMemo(() => {
    const guns = new Set(items.map((i) => i.weapon_name));
    return { count: items.length, guns: guns.size };
  }, [items]);

  return (
    <VStack spacing={5} align="stretch" w="100%">
      {/* 返回按钮 */}
      <Button
        as={Link}
        to="/delta-force"
        variant="ghost"
        size="sm"
        leftIcon={<ChevronLeft size={16} />}
        alignSelf="flex-start"
        mb={-2}
      >
        返回三角洲行动
      </Button>

      {/* 顶部统计 */}
      <HStack justify="space-between" wrap="wrap" gap={3}>
        <Box>
          <Heading size="md">本地改枪码库</Heading>
          <Text fontSize="sm" color={muted}>三角洲行动 · 粘贴自动识别枪名 + 配置，支持批量导入</Text>
        </Box>
        <HStack gap={3}>
          <Badge colorScheme="blue" px={3} py={1} fontSize="sm">{stats.count} 条记录</Badge>
          <Badge colorScheme="purple" px={3} py={1} fontSize="sm">{stats.guns} 把枪</Badge>
        </HStack>
      </HStack>

      {/* 添加区 */}
      <LiquidGlassCard p={5}>
        <VStack spacing={4} align="stretch">
          <Heading size="sm">添加改枪码</Heading>
          <FormControl>
            <FormLabel fontSize="sm" color={muted}>改枪码（粘贴后自动识别枪名与配置）</FormLabel>
            <Textarea
              value={code}
              onChange={(e) => handleCodeChange(e.target.value)}
              placeholder="例如 6KFCC1K08QMG6NRUK6UQC"
              rows={2}
              bg={bg}
              borderColor={border}
              fontFamily="mono"
              fontSize="sm"
            />
            {recog && (
              <Text fontSize="sm" mt={2} color={recog.recognized ? "green.400" : "yellow.400"}>
                {recog.recognized
                  ? <>已识别：<b>{recog.weapon_name}</b>{recog.note ? <> · <b>{recog.note}</b></> : null}</>
                  : "未识别出枪名，可手动输入后保存，下次同枪代码将自动识别"}
              </Text>
            )}
          </FormControl>
          <HStack gap={3} align="flex-start">
            <FormControl>
              <FormLabel fontSize="sm" color={muted}>枪名</FormLabel>
              <Input value={weapon} onChange={(e) => setWeapon(e.target.value)} placeholder="枪名" bg={bg} borderColor={border} />
            </FormControl>
            <FormControl>
              <FormLabel fontSize="sm" color={muted}>备注（配置）</FormLabel>
              <Input value={note} onChange={(e) => setNote(e.target.value)} placeholder="例如 20W青春版 / 55W满改红点" bg={bg} borderColor={border} />
            </FormControl>
          </HStack>
          <HStack gap={3} wrap="wrap">
            <Button colorScheme="blue" leftIcon={<Plus size={16} />} onClick={handleSave} isLoading={busy}>
              保存
            </Button>
            <Button variant="outline" leftIcon={<Plus size={16} />} onClick={() => setShowImport(true)}>
              批量导入（粘贴文档）
            </Button>
            <Button variant="outline" leftIcon={<Download size={16} />} onClick={handleExport}>
              导出 JSON 备份
            </Button>
            <Button variant="outline" leftIcon={<Upload size={16} />} onClick={handleImportJson}>
              导入 JSON 备份
            </Button>
          </HStack>
        </VStack>
      </LiquidGlassCard>

      {/* 列表区 */}
      <LiquidGlassCard p={5}>
        <VStack spacing={4} align="stretch">
          <HStack justify="space-between" wrap="wrap" gap={3}>
            <Heading size="sm">我的改枪码库</Heading>
            <Input
              w={{ base: "100%", md: "280px" }}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="搜索 枪名 / 代码 / 备注…"
              bg={bg}
              borderColor={border}
              fontSize="sm"
            />
          </HStack>

          {/* 类别筛选 */}
          <HStack spacing={2} wrap="wrap">
            <Button
              size="xs"
              borderRadius="full"
              variant={catFilter === "all" ? "solid" : "outline"}
              colorScheme={catFilter === "all" ? "blue" : "gray"}
              onClick={() => { setCatFilter("all"); setGunFilter("all"); }}
            >
              全部类别（{items.length}）
            </Button>
            {CATEGORY_ORDER.map((cat) => {
              const cnt = catCounts.get(cat) || 0;
              if (cnt === 0) return null;
              return (
                <Button
                  key={cat}
                  size="xs"
                  borderRadius="full"
                  variant={catFilter === cat ? "solid" : "outline"}
                  colorScheme={catFilter === cat ? "blue" : "gray"}
                  onClick={() => { setCatFilter(cat); setGunFilter("all"); }}
                >
                  {cat}（{cnt}）
                </Button>
              );
            })}
          </HStack>

          {/* 枪名筛选 */}
          {gunOptions.length > 0 && (
            <HStack spacing={2} wrap="wrap">
              <Button
                size="xs"
                borderRadius="full"
                variant={gunFilter === "all" ? "solid" : "outline"}
                colorScheme={gunFilter === "all" ? "teal" : "gray"}
                onClick={() => setGunFilter("all")}
              >
                全部枪（{items.length}）
              </Button>
              {gunOptions.map((name) => {
                const cnt = items.filter((i) => i.weapon_name === name).length;
                return (
                  <Button
                    key={name}
                    size="xs"
                    borderRadius="full"
                    variant={gunFilter === name ? "solid" : "outline"}
                    colorScheme={gunFilter === name ? "teal" : "gray"}
                    onClick={() => setGunFilter(name)}
                  >
                    {name}（{cnt}）
                  </Button>
                );
              })}
            </HStack>
          )}

          {filtered.length === 0 ? (
            <Text color={muted} textAlign="center" py={8} fontSize="sm">
              {items.length === 0 ? "还没有记录，粘贴一条改枪码开始吧" : "没有匹配的结果"}
            </Text>
          ) : (
            <VStack spacing={3} align="stretch">
              {grouped.map(([gun, gunItems]) => {
                const isCollapsed = collapsed.has(gun);
                return (
                  <Box
                    key={gun}
                    border="1px solid"
                    borderColor={border}
                    borderRadius="lg"
                    overflow="hidden"
                    bg="blackAlpha.200"
                  >
                    <HStack
                      spacing={3}
                      px={4}
                      py={3}
                      cursor="pointer"
                      _hover={{ bg: "whiteAlpha.50" }}
                      onClick={() => {
                        setCollapsed((prev) => {
                          const next = new Set(prev);
                          if (next.has(gun)) next.delete(gun);
                          else next.add(gun);
                          return next;
                        });
                      }}
                    >
                      <Text fontSize="xs" color={muted}>{isCollapsed ? "▸" : "▾"}</Text>
                      <Text fontWeight="bold" color="blue.300">{gun}</Text>
                      {CATEGORY_MAP[gun] && (
                        <Badge colorScheme="purple" fontSize="xs">{CATEGORY_MAP[gun]}</Badge>
                      )}
                      <Text fontSize="xs" color={muted} ml="auto">{gunItems.length} 条配置</Text>
                    </HStack>
                    {!isCollapsed && (
                      <SimpleGrid columns={{ base: 1, md: 2, xl: 3 }} spacing={3} p={3} pt={1}>
                        {gunItems.map((item) => (
                          <Box
                            key={item.id}
                            p={3}
                            borderRadius="md"
                            bg={bg}
                            border="1px solid"
                            borderColor={border}
                          >
                            {editingId === item.id ? (
                              <VStack spacing={2} align="stretch">
                                <Input value={editWeapon} onChange={(e) => setEditWeapon(e.target.value)} placeholder="枪名" size="xs" bg={bg} borderColor={border} />
                                <Input value={editNote} onChange={(e) => setEditNote(e.target.value)} placeholder="备注" size="xs" bg={bg} borderColor={border} />
                                <HStack gap={2}>
                                  <Button size="xs" colorScheme="blue" leftIcon={<Save size={12} />} onClick={() => handleUpdate(item.id)}>保存</Button>
                                  <Button size="xs" variant="ghost" leftIcon={<X size={12} />} onClick={() => setEditingId(null)}>取消</Button>
                                </HStack>
                              </VStack>
                            ) : (
                              <>
                                <HStack justify="space-between" mb={1}>
                                  <Text fontWeight="semibold" fontSize="sm">{item.note || "无备注"}</Text>
                                  <Text fontSize="xs" color={muted}>{fmtTime(item.created_at)}</Text>
                                </HStack>
                                <Text fontFamily="mono" fontSize="xs" wordBreak="break-all" bg="blackAlpha.300" borderRadius="md" px={2} py={1}>
                                  {item.code}
                                </Text>
                                <HStack gap={1} mt={2}>
                                  <IconButton aria-label="复制" icon={<Copy size={13} />} size="xs" onClick={async () => {
                                    try {
                                      await navigator.clipboard.writeText(item.code);
                                      showToast("已复制 ✓");
                                    } catch { showToast("复制失败", "error"); }
                                  }} />
                                  <IconButton aria-label="编辑" icon={<Pencil size={13} />} size="xs" onClick={() => {
                                    setEditingId(item.id);
                                    setEditWeapon(item.weapon_name);
                                    setEditNote(item.note);
                                  }} />
                                  <IconButton aria-label="删除" icon={<Trash2 size={13} />} size="xs" colorScheme="red" variant="ghost" onClick={() => handleDelete(item.id)} />
                                </HStack>
                              </>
                            )}
                          </Box>
                        ))}
                      </SimpleGrid>
                    )}
                  </Box>
                );
              })}
            </VStack>
          )}
        </VStack>
      </LiquidGlassCard>

      {/* 批量导入弹窗 */}
      <Modal isOpen={showImport} onClose={() => setShowImport(false)} size="lg">
        <ModalOverlay />
        <ModalContent>
          <ModalHeader>批量导入改枪码</ModalHeader>
          <ModalCloseButton />
          <ModalBody>
            <Text fontSize="sm" color={muted} mb={3}>
              从在线文档/群里复制整段内容（枪名 + 配置 + 改枪码），粘贴到下面即可。自动识别每一条并保存。
            </Text>
            <Textarea
              value={importText}
              onChange={(e) => setImportText(e.target.value)}
              placeholder={"例如：\nRM277 20W青春版 6KFCC1K08QMG6NRUK6UQC\nMK47突击步枪-烽火地带-6HN54R808PBNCS3LFEE0A"}
              rows={12}
              fontFamily="mono"
              fontSize="sm"
            />
          </ModalBody>
          <ModalFooter>
            <Button variant="ghost" mr={3} onClick={() => setShowImport(false)}>取消</Button>
            <Button colorScheme="blue" onClick={handleBatchImport} isLoading={importing}>
              开始导入
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>
    </VStack>
  );
}
