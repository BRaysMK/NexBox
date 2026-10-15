import {
  AlertDialog,
  AlertDialogBody,
  AlertDialogContent,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogOverlay,
  Box,
  Button,
  Text,
} from "@chakra-ui/react";
import { useRef } from "react";
import { useTranslation } from "react-i18next";
import type { GunCode } from "@/lib/gunCodes";

/**
 * 删除确认（单条 + 批量）
 *
 * 应用内弹窗，替代 `window.confirm`。
 * 从 `GunCodesNotebook.tsx` 搬迁，新增批量删除确认。
 */
export function ConfirmDialogs({
  pendingDelete,
  onCancelDelete,
  onConfirmDelete,
  bulkCount,
  onCancelBulk,
  onConfirmBulk,
  textColor,
  subTextColor,
  panelBg,
  cardBg,
}: {
  pendingDelete: GunCode | null;
  onCancelDelete: () => void;
  onConfirmDelete: () => void;
  bulkCount: number;
  onCancelBulk: () => void;
  onConfirmBulk: () => void;
  textColor: string;
  subTextColor: string;
  panelBg: string;
  cardBg: string;
}) {
  const { t } = useTranslation();
  const singleRef = useRef<HTMLButtonElement>(null);
  const bulkRef = useRef<HTMLButtonElement>(null);

  return (
    <>
      {/* 单条删除 */}
      <AlertDialog
        isOpen={!!pendingDelete}
        leastDestructiveRef={singleRef}
        onClose={onCancelDelete}
        isCentered
      >
        <AlertDialogOverlay>
          <AlertDialogContent bg={cardBg}>
            <AlertDialogHeader fontSize="md" fontWeight="bold" color={textColor}>
              {t("gunCodes.deleteTitle")}
            </AlertDialogHeader>
            <AlertDialogBody color={textColor}>
              <Text fontSize="sm" mb={2}>{t("gunCodes.deleteConfirm")}</Text>
              {pendingDelete && (
                <Box bg={panelBg} p={2} borderRadius="md">
                  <Text fontSize="xs" fontFamily="mono" wordBreak="break-all">
                    {pendingDelete.weapon_name} · {pendingDelete.code}
                  </Text>
                  {pendingDelete.note && (
                    <Text fontSize="xs" color={subTextColor}>{pendingDelete.note}</Text>
                  )}
                </Box>
              )}
            </AlertDialogBody>
            <AlertDialogFooter>
              <Button ref={singleRef} size="sm" variant="ghost" onClick={onCancelDelete}>
                {t("gunCodes.cancel")}
              </Button>
              <Button size="sm" colorScheme="red" ml={3} onClick={onConfirmDelete}>
                {t("gunCodes.delete")}
              </Button>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialogOverlay>
      </AlertDialog>

      {/* 批量删除 */}
      <AlertDialog
        isOpen={bulkCount > 0}
        leastDestructiveRef={bulkRef}
        onClose={onCancelBulk}
        isCentered
      >
        <AlertDialogOverlay>
          <AlertDialogContent bg={cardBg}>
            <AlertDialogHeader fontSize="md" fontWeight="bold" color={textColor}>
              {t("gunCodes.bulkDeleteTitle")}
            </AlertDialogHeader>
            <AlertDialogBody color={textColor} fontSize="sm">
              {t("gunCodes.bulkDeleteConfirm", { n: bulkCount })}
            </AlertDialogBody>
            <AlertDialogFooter>
              <Button ref={bulkRef} size="sm" variant="ghost" onClick={onCancelBulk}>
                {t("gunCodes.cancel")}
              </Button>
              <Button size="sm" colorScheme="red" ml={3} onClick={onConfirmBulk}>
                {t("gunCodes.delete")}
              </Button>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialogOverlay>
      </AlertDialog>
    </>
  );
}
