import {
  AlertDialog,
  AlertDialogBody,
  AlertDialogContent,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogOverlay,
  Badge,
  Box,
  Button,
  Divider,
  HStack,
  IconButton,
  Modal,
  ModalBody,
  ModalCloseButton,
  ModalContent,
  ModalFooter,
  ModalHeader,
  ModalOverlay,
  Spinner,
  Table,
  TableContainer,
  Tbody,
  Td,
  Text,
  Th,
  Thead,
  Tooltip,
  Tr,
  VStack,
  useToast,
} from "@chakra-ui/react";
import { AlertTriangle, CheckCircle2, Download, Eraser, RefreshCw, Trash2, Upload } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { LearnedFingerprint, RuleFingerprintRow, RulesMeta } from "@/lib/gunCodes";
import { RuleLibrarySection } from "./RuleLibrarySection";

/**
 * 词库管理面板（NexBox / Chakra 版）
 *
 * 与 GunCodesAssistant 的 FluentUI 版功能对齐，但用 Chakra 重写以融入 NexBox 视觉。
 *
 * 两个能力：
 *  1. **在线热更新**：GitHub raw → jsDelivr 多源回退，本地优先合并（只补不覆盖）。
 *  2. **学习表管理**：查看/单条删除/清除遗留备注/全部清空。
 *
 * ⚠️ 踩坑记录：清空确认框必须是**兄弟 Modal**，不能嵌套在主 Modal 内部。
 * Chakra 的 Modal 同样用 Portal + 焦点陷阱，嵌套会导致外层 Modal 的
 * onClose 失效（表现为「两个关闭按钮都点了但面板不关」）。
 */
export function RulesManagerDialog({
  isOpen,
  onClose,
  meta,
  onCheckUpdate,
  onApplyUpdate,
  onResetRules,
  onListLearned,
  onDeleteLearned,
  onClearLearned,
  onClearNotes,
  onListRules,
  onDeleteRule,
  onUpdateRule,
  onClearOnlineRules,
  onClearRuleOverrides,
  textColor,
  subTextColor,
  panelBg,
  cardBorder,
}: {
  isOpen: boolean;
  onClose: () => void;
  meta: RulesMeta | null;
  onCheckUpdate: () => Promise<{
    ok: boolean; error?: string; source?: string; remoteVersion?: number;
    wouldAdd?: number; hasUpdate?: boolean; remoteFingerprints?: number;
  }>;
  onApplyUpdate: () => Promise<{ ok: boolean; error?: string; added?: number; after?: number; source?: string }>;
  onResetRules: () => Promise<{ ok: boolean; error?: string; removed?: boolean }>;
  onListLearned: () => Promise<{ ok: boolean; count: number; conflicts: number; items: LearnedFingerprint[] }>;
  onDeleteLearned: (fp: string) => Promise<{ ok: boolean; error?: string }>;
  onClearLearned: () => Promise<{ ok: boolean; cleared: number }>;
  onClearNotes: () => Promise<{ ok: boolean; cleaned: number }>;
  /** v10.4.0：规则库条目管理 */
  onListRules: (query: string, page: number, pageSize: number) => Promise<{
    ok: boolean; total: number; page: number; pageSize: number;
    items: RuleFingerprintRow[]; removedCount: number; patchedCount: number;
  }>;
  onDeleteRule: (fp: string) => Promise<{ ok: boolean; error?: string }>;
  onUpdateRule: (fp: string, gun: string) => Promise<{ ok: boolean; error?: string }>;
  onClearOnlineRules: () => Promise<{ ok: boolean; error?: string; removed?: boolean }>;
  onClearRuleOverrides: () => Promise<{ ok: boolean; error?: string; removed?: boolean }>;
  textColor: string;
  subTextColor: string;
  panelBg: string;
  cardBorder: string;
}) {
  const { t } = useTranslation();
  const toast = useToast();

  const [checking, setChecking] = useState(false);
  const [applying, setApplying] = useState(false);
  const [checkResult, setCheckResult] = useState<{
    ok: boolean; error?: string; source?: string; remoteVersion?: number;
    wouldAdd?: number; hasUpdate?: boolean; remoteFingerprints?: number;
  } | null>(null);

  const [learned, setLearned] = useState<LearnedFingerprint[]>([]);
  const [conflicts, setConflicts] = useState(0);
  const [loadingLearned, setLoadingLearned] = useState(false);
  const [confirmClear, setConfirmClear] = useState(false);
  const cancelRef = useRef<HTMLButtonElement>(null);

  const loadLearned = useCallback(async () => {
    setLoadingLearned(true);
    try {
      const r = await onListLearned();
      setLearned(r.items ?? []);
      setConflicts(r.conflicts ?? 0);
    } catch (e) {
      toast({ title: t("gunCodes.rulesLoadLearnedFailed"), description: String(e), status: "error" });
    } finally {
      setLoadingLearned(false);
    }
  }, [onListLearned, toast, t]);

  useEffect(() => {
    if (isOpen) {
      setCheckResult(null);
      void loadLearned();
    }
  }, [isOpen, loadLearned]);

  const check = async () => {
    setChecking(true);
    setCheckResult(null);
    try {
      const r = await onCheckUpdate();
      setCheckResult(r);
      if (!r.ok) {
        toast({ title: t("gunCodes.rulesCheckFailed"), description: r.error, status: "error" });
      }
    } catch (e) {
      setCheckResult({ ok: false, error: String(e) });
    } finally {
      setChecking(false);
    }
  };

  const apply = async () => {
    setApplying(true);
    try {
      const r = await onApplyUpdate();
      if (r.ok) {
        toast({
          title: r.added
            ? t("gunCodes.rulesUpdatedWith", { count: r.added })
            : t("gunCodes.rulesAlreadyLatest"),
          status: "success",
        });
        await loadLearned();
        setCheckResult(null);
      } else {
        toast({ title: t("gunCodes.rulesUpdateFailed"), description: r.error, status: "error" });
      }
    } catch (e) {
      toast({ title: t("gunCodes.rulesUpdateFailed"), description: String(e), status: "error" });
    } finally {
      setApplying(false);
    }
  };

  const reset = async () => {
    try {
      const r = await onResetRules();
      if (r.ok) {
        toast({
          title: r.removed ? t("gunCodes.rulesResetDone") : t("gunCodes.rulesAlreadyBundled"),
          status: "success",
        });
        await loadLearned();
      } else {
        toast({ title: t("gunCodes.rulesResetFailed"), description: r.error, status: "error" });
      }
    } catch (e) {
      toast({ title: t("gunCodes.rulesResetFailed"), description: String(e), status: "error" });
    }
  };

  const removeOne = async (fp: string) => {
    const r = await onDeleteLearned(fp);
    if (r.ok) {
      toast({ title: t("gunCodes.learnedDeleted", { fp }), status: "success" });
      await loadLearned();
    } else {
      toast({ title: t("gunCodes.learnedDeleteFailed"), description: r.error, status: "error" });
    }
  };

  const clearNotes = async () => {
    try {
      const r = await onClearNotes();
      if (r.ok) {
        toast({
          title: r.cleaned
            ? t("gunCodes.notesCleared", { count: r.cleaned })
            : t("gunCodes.notesNothingToClear"),
          status: "success",
        });
        await loadLearned();
      } else {
        toast({ title: t("gunCodes.notesClearFailed"), status: "error" });
      }
    } catch (e) {
      toast({ title: t("gunCodes.notesClearFailed"), description: String(e), status: "error" });
    }
  };

  const clearAll = async () => {
    setConfirmClear(false);
    const r = await onClearLearned();
    if (r.ok) {
      toast({ title: t("gunCodes.learnedCleared", { count: r.cleared }), status: "success" });
      await loadLearned();
    } else {
      toast({ title: t("gunCodes.learnedClearFailed"), status: "error" });
    }
  };

  const hasNotes = learned.some((x) => x.config);

  const sortedLearned = useMemo(
    () =>
      [...learned].sort((a, b) => {
        // 冲突项排最前，方便用户先处理有问题的
        const ac = a.conflictWith ? 0 : 1;
        const bc = b.conflictWith ? 0 : 1;
        return ac - bc || a.fingerprint.localeCompare(b.fingerprint);
      }),
    [learned],
  );

  return (
    <>
      <Modal isOpen={isOpen} onClose={onClose} size="5xl" scrollBehavior="inside" isCentered>
        <ModalOverlay />
        <ModalContent bg={panelBg}>
          <ModalHeader color={textColor}>{t("gunCodes.rulesManagerTitle")}</ModalHeader>
          <ModalCloseButton aria-label={t("common.close")} />

          <ModalBody>
            <VStack align="stretch" spacing={4}>
              {/* ── 当前词库 ── */}
              <HStack spacing={3} wrap="wrap">
                <Text fontSize="sm" fontWeight="600" color={textColor}>
                  {t("gunCodes.rulesCurrentLabel")}
                </Text>
                {meta ? (
                  <>
                    <Badge colorScheme="blue">v{meta.version}</Badge>
                    <Text fontSize="xs" color={subTextColor}>
                      {t("gunCodes.rulesMetaInline", {
                        guns: meta.gunCount,
                        fps: meta.fingerprintCount,
                        learned: meta.learnedCount,
                      })}
                    </Text>
                  </>
                ) : (
                  <Spinner size="xs" />
                )}
              </HStack>

              <Divider borderColor={cardBorder} />

              {/* ── 在线更新 ── */}
              <VStack align="stretch" spacing={2}>
                <HStack justify="space-between" wrap="wrap" spacing={2}>
                  <Text fontSize="sm" fontWeight="600" color={textColor}>
                    {t("gunCodes.rulesOnlineTitle")}
                  </Text>
                  <HStack spacing={2}>
                    <Button
                      size="xs"
                      variant="outline"
                      leftIcon={<RefreshCw size={13} />}
                      isLoading={checking}
                      isDisabled={applying}
                      onClick={check}
                    >
                      {t("gunCodes.rulesCheckUpdate")}
                    </Button>
                    <Tooltip label={t("gunCodes.rulesResetHint")} hasArrow>
                      <Button size="xs" variant="ghost" isDisabled={applying} onClick={reset}>
                        {t("gunCodes.rulesReset")}
                      </Button>
                    </Tooltip>
                  </HStack>
                </HStack>

                <Text fontSize="xs" color={subTextColor} lineHeight={1.7}>
                  {t("gunCodes.rulesOnlineHint")}
                </Text>

                {checkResult && !checkResult.ok && (
                  <Box p={2} borderRadius="md" borderWidth="1px" borderColor="red.400" bg="red.50" _dark={{ bg: "red.900" }}>
                    <Text fontSize="xs" color="red.600" _dark={{ color: "red.200" }}>
                      {t("gunCodes.rulesCheckFailed")}: {checkResult.error}
                    </Text>
                  </Box>
                )}

                {checkResult?.ok && (
                  <Box
                    p={2}
                    borderRadius="md"
                    borderWidth="1px"
                    borderColor={checkResult.hasUpdate ? "green.400" : "blue.300"}
                  >
                    <HStack spacing={2}>
                      {checkResult.hasUpdate ? <Download size={14} /> : <CheckCircle2 size={14} />}
                      <Text fontSize="xs" color={textColor}>
                        {checkResult.hasUpdate
                          ? t("gunCodes.rulesFoundUpdate", {
                              count: checkResult.wouldAdd,
                              version: checkResult.remoteVersion,
                            })
                          : t("gunCodes.rulesAlreadyLatest")}
                      </Text>
                    </HStack>
                    <Text fontSize="xs" color={subTextColor} mt={1}>
                      {t("gunCodes.rulesSourceInfo", {
                        source: checkResult.source,
                        total: checkResult.remoteFingerprints,
                      })}
                    </Text>
                  </Box>
                )}

                {checkResult?.ok && checkResult.hasUpdate && (
                  <Button
                    size="sm"
                    colorScheme="blue"
                    leftIcon={applying ? <Spinner size="xs" /> : <CheckCircle2 size={14} />}
                    isLoading={applying}
                    onClick={apply}
                  >
                    {t("gunCodes.rulesApplyUpdate", { count: checkResult.wouldAdd })}
                  </Button>
                )}
              </VStack>

              <Divider borderColor={cardBorder} />

              {/* ── 规则库条目（v10.4.0 新增）── */}
              <RuleLibrarySection
                onList={onListRules}
                onDelete={onDeleteRule}
                onUpdate={onUpdateRule}
                onClearOnline={onClearOnlineRules}
                onClearOverrides={onClearRuleOverrides}
                textColor={textColor}
                subTextColor={subTextColor}
                cardBorder={cardBorder}
              />

              {/* ── 学习表 ── */}
              <VStack align="stretch" spacing={2}>
                <HStack justify="space-between" wrap="wrap" spacing={2}>
                  <HStack spacing={2}>
                    <Text fontSize="sm" fontWeight="600" color={textColor}>
                      {t("gunCodes.learnedTitle")}
                    </Text>
                    <Badge>{learned.length}</Badge>
                    {conflicts > 0 && (
                      <Tooltip label={t("gunCodes.learnedConflictHint")} hasArrow>
                        <Badge colorScheme="orange" display="flex" alignItems="center" gap={1}>
                          <AlertTriangle size={11} />
                          {t("gunCodes.learnedConflicts", { count: conflicts })}
                        </Badge>
                      </Tooltip>
                    )}
                  </HStack>
                  <HStack spacing={2}>
                    <Button
                      size="xs"
                      variant="ghost"
                      leftIcon={<RefreshCw size={12} />}
                      isLoading={loadingLearned}
                      onClick={loadLearned}
                    >
                      {t("gunCodes.refresh")}
                    </Button>
                    {hasNotes && (
                      <Tooltip label={t("gunCodes.clearNotesHint")} hasArrow>
                        <Button size="xs" variant="ghost" leftIcon={<Eraser size={12} />} onClick={clearNotes}>
                          {t("gunCodes.clearNotes")}
                        </Button>
                      </Tooltip>
                    )}
                    <Button
                      size="xs"
                      variant="ghost"
                      colorScheme="red"
                      leftIcon={<Trash2 size={12} />}
                      isDisabled={learned.length === 0}
                      onClick={() => setConfirmClear(true)}
                    >
                      {t("gunCodes.learnedClearAll")}
                    </Button>
                  </HStack>
                </HStack>

                <Text fontSize="xs" color={subTextColor} lineHeight={1.7}>
                  {t("gunCodes.learnedHint")}
                </Text>

                {loadingLearned ? (
                  <Spinner size="sm" />
                ) : learned.length === 0 ? (
                  <Box p={3} borderRadius="md" borderWidth="1px" borderColor={cardBorder}>
                    <Text fontSize="xs" color={subTextColor}>
                      {t("gunCodes.learnedEmpty")}
                    </Text>
                  </Box>
                ) : (
                  <TableContainer maxH="300px" overflowY="auto" borderWidth="1px" borderColor={cardBorder} borderRadius="md">
                    <Table size="sm">
                      <Thead>
                        <Tr>
                          <Th>{t("gunCodes.learnedColFp")}</Th>
                          <Th>{t("gunCodes.learnedColGun")}</Th>
                          <Th>{t("gunCodes.learnedColRules")}</Th>
                          <Th w="50px" />
                        </Tr>
                      </Thead>
                      <Tbody>
                        {sortedLearned.map((it) => (
                          <Tr key={it.fingerprint}>
                            <Td>
                              <Text fontFamily="mono" fontSize="xs" color={textColor}>
                                {it.fingerprint}
                              </Text>
                            </Td>
                            <Td>
                              <Text fontSize="xs" color={textColor}>
                                {it.gun || "—"}
                              </Text>
                              {it.config && (
                                <Text fontSize="10px" color={subTextColor}>
                                  · {it.config}
                                </Text>
                              )}
                            </Td>
                            <Td>
                              {it.conflictWith ? (
                                <Tooltip label={t("gunCodes.learnedConflictHint")} hasArrow>
                                  <Text fontSize="xs" color="orange.400">
                                    <AlertTriangle size={10} style={{ display: "inline", marginRight: 3 }} />
                                    {it.conflictWith}
                                  </Text>
                                </Tooltip>
                              ) : it.inLibrary ? (
                                <Text fontSize="xs" color={subTextColor}>
                                  {it.fromRules ?? "—"}
                                </Text>
                              ) : (
                                <Text fontSize="xs" color={subTextColor}>
                                  {t("gunCodes.learnedNotInLibrary")}
                                </Text>
                              )}
                            </Td>
                            <Td>
                              <Tooltip label={t("gunCodes.learnedDelete")} hasArrow>
                                <IconButton
                                  aria-label={t("gunCodes.learnedDelete")}
                                  icon={<Trash2 size={12} />}
                                  size="xs"
                                  variant="ghost"
                                  onClick={() => removeOne(it.fingerprint)}
                                />
                              </Tooltip>
                            </Td>
                          </Tr>
                        ))}
                      </Tbody>
                    </Table>
                  </TableContainer>
                )}
              </VStack>
            </VStack>
          </ModalBody>

          <ModalFooter>
            <Button size="sm" variant="ghost" onClick={onClose}>
              {t("common.close")}
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>

      {/* 清空确认：**兄弟 AlertDialog**，不嵌套（见文件头注释） */}
      <AlertDialog
        isOpen={confirmClear}
        leastDestructiveRef={cancelRef}
        onClose={() => setConfirmClear(false)}
        isCentered
      >
        <AlertDialogOverlay>
          <AlertDialogContent bg={panelBg}>
            <AlertDialogHeader fontSize="md" color={textColor}>
              {t("gunCodes.learnedClearConfirmTitle")}
            </AlertDialogHeader>
            <AlertDialogBody>
              <Text fontSize="sm" color={textColor}>
                {t("gunCodes.learnedClearConfirmBody", { count: learned.length })}
              </Text>
              <Text fontSize="xs" color={subTextColor} mt={2}>
                {t("gunCodes.learnedClearConfirmNote")}
              </Text>
            </AlertDialogBody>
            <AlertDialogFooter>
              <Button ref={cancelRef} size="sm" variant="ghost" onClick={() => setConfirmClear(false)}>
                {t("common.cancel")}
              </Button>
              <Button size="sm" colorScheme="red" ml={3} onClick={clearAll}>
                {t("gunCodes.learnedClearAll")}
              </Button>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialogOverlay>
      </AlertDialog>
    </>
  );
}

/** 未使用占位：保持 import 稳定（Upload 供后续「导入备份」入口复用） */
export const __rulesIcons = { Upload };
