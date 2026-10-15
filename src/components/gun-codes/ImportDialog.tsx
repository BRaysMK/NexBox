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
  Checkbox,
  Divider,
  HStack,
  Progress,
  Text,
  VStack,
  useDisclosure,
} from "@chakra-ui/react";
import { AlertTriangle } from "lucide-react";
import { useRef } from "react";
import { useTranslation } from "react-i18next";
import type { PreviewResult } from "@/lib/gunCodes";

/**
 * 批量导入（预览流）
 *
 * 从 `GunCodesNotebook.tsx` 原样搬迁，**逻辑未改**：
 * ① 粘贴文本 → 解析预览（不写库）→ ② 逐条勾选 → ③ 二次确认后入库。
 */
export function ImportDialog({
  isOpen,
  onClose,
  text,
  setText,
  preview,
  deselected,
  onToggleRow,
  onBulkSelect,
  onPreview,
  onConfirm,
  busy,
  stats,
  textColor,
  subTextColor,
  bg,
  border,
  cardBg,
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
  textColor: string;
  subTextColor: string;
  bg: string;
  border: string;
  cardBg: string;
}) {
  const { t } = useTranslation();
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

      {/* 二次确认 */}
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
