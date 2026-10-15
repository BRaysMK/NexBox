import { Box } from "@chakra-ui/react";
import { norm } from "@/lib/gunCodes";

/**
 * 搜索命中高亮
 *
 * 归一化会去掉 `-`/空格，导致归一化索引与原串索引错位，
 * 因此用「逐字符归一 + 记录原串下标映射」的方式定位，避免错位高亮。
 */
export function Highlight({
  text,
  query,
  color,
}: {
  text: string;
  query: string;
  color: string;
}) {
  const q = query.trim();
  if (!q) return <>{text}</>;

  const normChar = (c: string) => (/[\s\-_]/.test(c) ? "" : c.toUpperCase());
  const normQ = [...q].map(normChar).join("");
  if (!normQ) return <>{text}</>;

  const chars = [...text];
  const map: number[] = []; // 归一化下标 → 原串下标
  for (let i = 0; i < chars.length; i++) {
    if (normChar(chars[i]) !== "") map.push(i);
  }
  const normText = chars.map(normChar).join("");

  const idx = normText.indexOf(normQ);
  if (idx < 0) return <>{text}</>;

  const start = map[idx];
  const end = map[Math.min(idx + normQ.length - 1, map.length - 1)] + 1;

  return (
    <>
      {text.slice(0, start)}
      <Box as="mark" bg="transparent" color={color} fontWeight="bold">
        {text.slice(start, end)}
      </Box>
      {text.slice(end)}
    </>
  );
}

/** 归一化匹配判断（供不支持 <mark> 的场景，如 Chakra Table 的排序 key） */
export { norm };
