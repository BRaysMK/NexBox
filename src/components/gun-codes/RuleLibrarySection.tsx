import {
  Badge,
  Box,
  Button,
  Divider,
  HStack,
  IconButton,
  Input,
  InputGroup,
  InputLeftElement,
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
  useDisclosure,
  useToast,
} from "@chakra-ui/react";
import { Edit, RefreshCw, Search, Trash2 } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { RuleFingerprintRow } from "@/lib/gunCodes";

/**
 * 规则库条目管理（NexBox / Chakra 版，v10.4.0）
 *
 * 支持：搜索 / 分页浏览 / 改单条枪名 / 删单条 / 清空线上库 / 清空我的改动。
 *
 * 删除的进黑名单、修改的进覆盖层，与随包库和线上库**分开存储**。
 */
export function RuleLibrarySection({
  onList,
  onDelete,
  onUpdate,
  onClearOnline,
  onClearOverrides,
  textColor,
  subTextColor,
  cardBorder,
  pageSize = 50,
}: {
  onList: (
    query: string,
    page: number,
    pageSize: number,
  ) => Promise<{
    ok: boolean;
    total: number;
    page: number;
    pageSize: number;
    items: RuleFingerprintRow[];
    removedCount: number;
    patchedCount: number;
  }>;
  onDelete: (fp: string) => Promise<{ ok: boolean; error?: string }>;
  onUpdate: (fp: string, gun: string) => Promise<{ ok: boolean; error?: string }>;
  onClearOnline: () => Promise<{ ok: boolean; error?: string; removed?: boolean }>;
  onClearOverrides: () => Promise<{ ok: boolean; error?: string; removed?: boolean }>;
  textColor: string;
  subTextColor: string;
  cardBorder: string;
  pageSize?: number;
}) {
  const { t } = useTranslation();
  const toast = useToast();

  const [query, setQuery] = useState("");
  const [page, setPage] = useState(1);
  const [rows, setRows] = useState<RuleFingerprintRow[]>([]);
  const [total, setTotal] = useState(0);
  const [removedCount, setRemovedCount] = useState(0);
  const [patchedCount, setPatchedCount] = useState(0);
  const [loading, setLoading] = useState(false);

  const [editing, setEditing] = useState<RuleFingerprintRow | null>(null);
  const [editGun, setEditGun] = useState("");
  const [saving, setSaving] = useState(false);
  const editDisc = useDisclosure();
  const inputRef = useRef<HTMLInputElement>(null);

  const load = useCallback(
    async (q: string, p: number) => {
      setLoading(true);
      try {
        const r = await onList(q, p, pageSize);
        setRows(r.items ?? []);
        setTotal(r.total ?? 0);
        setRemovedCount(r.removedCount ?? 0);
        setPatchedCount(r.patchedCount ?? 0);
      } catch (e) {
        toast({ title: t("gunCodes.ruleListFailed"), description: String(e), status: "error" });
      } finally {
        setLoading(false);
      }
    },
    [onList, toast, t, pageSize],
  );

  // 换页时加载
  useEffect(() => {
    void load(query, page);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [page]);

  // 搜索防抖
  useEffect(() => {
    const timer = window.setTimeout(() => {
      setPage(1);
      void load(query, 1);
    }, 300);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query]);

  const totalPages = Math.max(1, Math.ceil(total / pageSize));

  const doDelete = async (fp: string) => {
    const r = await onDelete(fp);
    if (r.ok) {
      toast({ title: t("gunCodes.ruleDeleted", { fp }), status: "success" });
      await load(query, page);
    } else {
      toast({ title: t("gunCodes.ruleDeleteFailed"), description: r.error, status: "error" });
    }
  };

  const openEdit = (row: RuleFingerprintRow) => {
    setEditing(row);
    setEditGun(row.gun);
    editDisc.onOpen();
  };

  const saveEdit = async () => {
    if (!editing) return;
    const g = editGun.trim();
    if (!g) return;
    setSaving(true);
    try {
      const r = await onUpdate(editing.fingerprint, g);
      if (r.ok) {
        toast({ title: t("gunCodes.ruleUpdated", { fp: editing.fingerprint, gun: g }), status: "success" });
        editDisc.onClose();
        await load(query, page);
      } else {
        toast({ title: t("gunCodes.ruleUpdateFailed"), description: r.error, status: "error" });
      }
    } finally {
      setSaving(false);
    }
  };

  return (
    <VStack align="stretch" spacing={2}>
      <HStack justify="space-between" wrap="wrap" spacing={2}>
        <HStack spacing={2} wrap="wrap">
          <Text fontSize="sm" fontWeight="600" color={textColor}>
            {t("gunCodes.ruleLibraryTitle")}
          </Text>
          <Badge>{total}</Badge>
          {removedCount > 0 && (
            <Tooltip label={t("gunCodes.ruleRemovedHint")} hasArrow>
              <Badge colorScheme="red">
                {t("gunCodes.ruleRemovedCount", { count: removedCount })}
              </Badge>
            </Tooltip>
          )}
          {patchedCount > 0 && (
            <Tooltip label={t("gunCodes.rulePatchedHint")} hasArrow>
              <Badge colorScheme="orange">
                {t("gunCodes.rulePatchedCount", { count: patchedCount })}
              </Badge>
            </Tooltip>
          )}
        </HStack>
        <HStack spacing={2}>
          <InputGroup size="xs" w="190px">
            <InputLeftElement pointerEvents="none">
              <Search size={12} />
            </InputLeftElement>
            <Input
              ref={inputRef}
              placeholder={t("gunCodes.ruleSearchPlaceholder")}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </InputGroup>
          <Tooltip label={t("gunCodes.refresh")} hasArrow>
            <IconButton
              aria-label={t("gunCodes.refresh")}
              icon={<RefreshCw size={12} />}
              size="xs"
              variant="ghost"
              isLoading={loading}
              onClick={() => load(query, page)}
            />
          </Tooltip>
        </HStack>
      </HStack>

      <Text fontSize="xs" color={subTextColor} lineHeight={1.7}>
        {t("gunCodes.ruleLibraryHint")}
      </Text>

      {(removedCount > 0 || patchedCount > 0) && (
        <Button
          size="xs"
          variant="ghost"
          alignSelf="flex-start"
          onClick={async () => {
            const r = await onClearOverrides();
            if (r.ok) {
              toast({ title: t("gunCodes.ruleOverridesCleared"), status: "success" });
              await load(query, page);
            } else {
              toast({ title: t("gunCodes.ruleClearFailed"), description: r.error, status: "error" });
            }
          }}
        >
          {t("gunCodes.ruleClearOverrides")}
        </Button>
      )}

      {loading ? (
        <Spinner size="sm" />
      ) : rows.length === 0 ? (
        <Box p={3} borderRadius="md" borderWidth="1px" borderColor={cardBorder}>
          <Text fontSize="xs" color={subTextColor}>
            {t("gunCodes.ruleNoMatch")}
          </Text>
        </Box>
      ) : (
        <TableContainer
          maxH="240px"
          overflowY="auto"
          borderWidth="1px"
          borderColor={cardBorder}
          borderRadius="md"
        >
          <Table size="sm">
            <Thead>
              <Tr>
                <Th>{t("gunCodes.ruleColFp")}</Th>
                <Th>{t("gunCodes.ruleColGun")}</Th>
                <Th>{t("gunCodes.ruleColSource")}</Th>
                <Th w="70px" />
              </Tr>
            </Thead>
            <Tbody>
              {rows.map((row) => (
                <Tr key={row.fingerprint}>
                  <Td>
                    <Text fontFamily="mono" fontSize="xs" color={textColor}>
                      {row.fingerprint}
                    </Text>
                  </Td>
                  <Td>
                    <Text fontSize="xs" color={textColor}>
                      {row.gun || "—"}
                    </Text>
                  </Td>
                  <Td>
                    {row.removed ? (
                      <Badge colorScheme="red" fontSize="10px">
                        {t("gunCodes.ruleBadgeRemoved")}
                      </Badge>
                    ) : row.patched ? (
                      <Badge colorScheme="orange" fontSize="10px">
                        {t("gunCodes.ruleBadgePatched")}
                      </Badge>
                    ) : (
                      <Text fontSize="xs" color={subTextColor}>
                        {t("gunCodes.ruleBadgeLibrary")}
                      </Text>
                    )}
                  </Td>
                  <Td>
                    <HStack spacing={0}>
                      <Tooltip label={t("gunCodes.ruleEdit")} hasArrow>
                        <IconButton
                          aria-label={t("gunCodes.ruleEdit")}
                          icon={<Edit size={12} />}
                          size="xs"
                          variant="ghost"
                          onClick={() => openEdit(row)}
                        />
                      </Tooltip>
                      <Tooltip label={t("gunCodes.ruleDelete")} hasArrow>
                        <IconButton
                          aria-label={t("gunCodes.ruleDelete")}
                          icon={<Trash2 size={12} />}
                          size="xs"
                          variant="ghost"
                          colorScheme="red"
                          onClick={() => doDelete(row.fingerprint)}
                        />
                      </Tooltip>
                    </HStack>
                  </Td>
                </Tr>
              ))}
            </Tbody>
          </Table>
        </TableContainer>
      )}

      <HStack justify="space-between" wrap="wrap" spacing={2}>
        <Text fontSize="xs" color={subTextColor}>
          {t("gunCodes.rulePager", { page, total: totalPages, count: total })}
        </Text>
        <HStack spacing={2}>
          <Button size="xs" variant="ghost" isDisabled={page <= 1} onClick={() => setPage((p) => Math.max(1, p - 1))}>
            {t("gunCodes.rulePrev")}
          </Button>
          <Button
            size="xs"
            variant="ghost"
            isDisabled={page >= totalPages}
            onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
          >
            {t("gunCodes.ruleNext")}
          </Button>
          <Tooltip label={t("gunCodes.ruleClearOnlineHint")} hasArrow>
            <Button
              size="xs"
              variant="ghost"
              onClick={async () => {
                const r = await onClearOnline();
                if (r.ok) {
                  toast({
                    title: r.removed ? t("gunCodes.ruleOnlineCleared") : t("gunCodes.ruleNoOnline"),
                    status: "success",
                  });
                  await load(query, page);
                } else {
                  toast({ title: t("gunCodes.ruleClearFailed"), description: r.error, status: "error" });
                }
              }}
            >
              {t("gunCodes.ruleClearOnline")}
            </Button>
          </Tooltip>
        </HStack>
      </HStack>

      {/* 修改枪名对话框 */}
      <Modal isOpen={editDisc.isOpen} onClose={editDisc.onClose} isCentered size="sm">
        <ModalOverlay />
        <ModalContent>
          <ModalHeader fontSize="md">{t("gunCodes.ruleEditTitle")}</ModalHeader>
          <ModalCloseButton />
          <ModalBody>
            <VStack align="stretch" spacing={3}>
              <Box>
                <Text fontSize="xs" color={subTextColor} mb={1}>
                  {t("gunCodes.ruleColFp")}
                </Text>
                <Text fontFamily="mono" fontSize="sm">
                  {editing?.fingerprint}
                </Text>
              </Box>
              <Box>
                <Text fontSize="xs" color={subTextColor} mb={1}>
                  {t("gunCodes.ruleColGun")}
                </Text>
                <Input
                  size="sm"
                  value={editGun}
                  onChange={(e) => setEditGun(e.target.value)}
                  autoFocus
                />
              </Box>
              <Text fontSize="xs" color={subTextColor}>
                {t("gunCodes.ruleEditHint")}
              </Text>
            </VStack>
          </ModalBody>
          <ModalFooter>
            <Button size="sm" variant="ghost" mr={2} onClick={editDisc.onClose}>
              {t("common.cancel")}
            </Button>
            <Button size="sm" colorScheme="blue" isLoading={saving} isDisabled={!editGun.trim()} onClick={saveEdit}>
              {t("common.save")}
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>

      <Divider borderColor={cardBorder} />
    </VStack>
  );
}
