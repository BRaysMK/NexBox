import {
  Button,
  Drawer,
  DrawerBody,
  DrawerCloseButton,
  DrawerContent,
  DrawerFooter,
  DrawerHeader,
  DrawerOverlay,
  FormControl,
  FormLabel,
  HStack,
  Input,
  Text,
  Textarea,
  VStack,
  useColorModeValue,
} from "@chakra-ui/react";
import { AlertCircle, CheckCircle2, Sparkles } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { GunCode, RecognizeResult } from "@/lib/gunCodes";

/**
 * 添加 / 编辑改枪码（抽屉）
 *
 * 交互与旧版一致：粘贴 → 250ms 防抖识别 → 可手动修正 → 保存后学习指纹。
 * 由内联表单改为 Chakra `Drawer`，不再占用列表版面。
 */
export function GunCodeEditor({
  open,
  editing,
  onClose,
  onRecognize,
  onSave,
}: {
  open: boolean;
  editing: GunCode | null;
  onClose: () => void;
  onRecognize: (code: string) => Promise<RecognizeResult>;
  onSave: (weaponName: string, code: string, note: string) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [code, setCode] = useState("");
  const [weapon, setWeapon] = useState("");
  const [note, setNote] = useState("");
  const [recog, setRecog] = useState<RecognizeResult | null>(null);
  const [saving, setSaving] = useState(false);
  const timer = useRef<number | null>(null);
  const okColor = useColorModeValue("green.600", "green.300");
  const warnColor = useColorModeValue("orange.600", "orange.300");

  // 打开时按「编辑对象」初始化；关闭时清空
  useEffect(() => {
    if (!open) {
      setCode("");
      setWeapon("");
      setNote("");
      setRecog(null);
      if (timer.current) window.clearTimeout(timer.current);
      return;
    }
    if (editing) {
      setCode(editing.code);
      setWeapon(editing.weapon_name);
      setNote(editing.note);
      setRecog(null);
    } else {
      setCode("");
      setWeapon("");
      setNote("");
      setRecog(null);
    }
  }, [open, editing]);

  const handleCode = (v: string) => {
    setCode(v);
    // 编辑已有条目时不重新识别（用户已确认过枪名）
    if (editing) return;
    const clean = v.replace(/[^a-zA-Z0-9]/g, "");
    if (clean.length < 6) {
      setRecog(null);
      setWeapon("");
      setNote("");
      return;
    }
    if (timer.current) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(async () => {
      try {
        const r = await onRecognize(v);
        setRecog(r);
        if (r.recognized) {
          setWeapon(r.weapon_name);
          setNote(r.note || "");
        } else {
          setNote("");
        }
      } catch {
        /* 忽略识别失败 */
      }
    }, 250);
  };

  const save = async () => {
    if (!code.trim()) return;
    setSaving(true);
    try {
      await onSave(weapon, code, note);
      onClose();
    } finally {
      setSaving(false);
    }
  };

  return (
    <Drawer isOpen={open} placement="right" onClose={onClose} size="md">
      <DrawerOverlay />
      <DrawerContent>
        <DrawerCloseButton />
        <DrawerHeader fontSize="md">
          {editing ? t("gunCodes.editTitle") : t("gunCodes.addTitle")}
        </DrawerHeader>

        <DrawerBody>
          <VStack spacing={4} align="stretch">
            <FormControl>
              <FormLabel fontSize="sm">{t("gunCodes.codeLabel")}</FormLabel>
              <Textarea
                value={code}
                onChange={(e) => handleCode(e.target.value)}
                placeholder={t("gunCodes.codePlaceholder")}
                rows={3}
                fontFamily="mono"
                autoFocus
              />
              {recog && (
                <HStack
                  mt={2}
                  px={3}
                  py={2}
                  borderRadius="md"
                  spacing={2}
                  align="start"
                  bg={recog.recognized ? "green.50" : "orange.50"}
                  color={recog.recognized ? okColor : warnColor}
                  border="1px solid"
                  borderColor={recog.recognized ? "green.200" : "orange.200"}
                  _dark={{
                    bg: recog.recognized ? "rgba(62,207,142,0.10)" : "rgba(240,194,90,0.10)",
                    borderColor: recog.recognized
                      ? "rgba(62,207,142,0.25)"
                      : "rgba(240,194,90,0.25)",
                  }}
                >
                  {recog.recognized ? <Sparkles size={14} /> : <AlertCircle size={14} />}
                  <VStack align="start" spacing={0} flex={1}>
                    {recog.recognized ? (
                      <>
                        <Text fontSize="sm">
                          {t("gunCodes.recognized")}
                          <b>{recog.weapon_name}</b>
                        </Text>
                        {recog.confidence > 0 && (
                          <Text fontSize="xs" opacity={0.8}>
                            {t("gunCodes.confidence", {
                              p: Math.round(recog.confidence * 100),
                            })}
                            {recog.reason ? ` · ${recog.reason}` : ""}
                          </Text>
                        )}
                      </>
                    ) : (
                      <Text fontSize="sm">{t("gunCodes.notRecognized")}</Text>
                    )}
                  </VStack>
                </HStack>
              )}
            </FormControl>

            <FormControl>
              <FormLabel fontSize="sm">{t("gunCodes.gunNameLabel")}</FormLabel>
              <Input
                value={weapon}
                onChange={(e) => setWeapon(e.target.value)}
                placeholder={t("gunCodes.gunNamePlaceholder")}
              />
            </FormControl>

            <FormControl>
              <FormLabel fontSize="sm">{t("gunCodes.noteLabel")}</FormLabel>
              <Input
                value={note}
                onChange={(e) => setNote(e.target.value)}
                placeholder={t("gunCodes.notePlaceholder")}
              />
            </FormControl>
          </VStack>
        </DrawerBody>

        <DrawerFooter>
          <Button variant="ghost" mr={3} onClick={onClose}>
            {t("gunCodes.cancel")}
          </Button>
          <Button
            colorScheme="blue"
            leftIcon={<CheckCircle2 size={15} />}
            isDisabled={!code.trim()}
            isLoading={saving}
            onClick={save}
          >
            {t("gunCodes.save")}
          </Button>
        </DrawerFooter>
      </DrawerContent>
    </Drawer>
  );
}
