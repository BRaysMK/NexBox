/**
 * 运行时验证：分类表注入前后 catOf() 的行为差异
 *
 * 复现用户报告的现象：某些枪的码被归到「其他」分类。
 * 用 esbuild 打包纯逻辑层后在 Node 里执行（与 app 内同一份代码）。
 */
import { build } from "esbuild";
import { mkdtempSync, rmSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

// 相对本脚本定位，避免硬编码机器路径
const rel = (p) => fileURLToPath(new URL(p, import.meta.url));

const tmp = mkdtempSync(join(tmpdir(), "gun-cat-"));
const outfile = join(tmp, "m.mjs");

try {
  await build({
    entryPoints: [rel("../src/lib/gunCodes.ts")],
    outfile,
    bundle: true,
    format: "esm",
    platform: "node",
    target: "node18",
    logLevel: "warning",
  });

  const m = await import(pathToFileURL(outfile).href);

  const rules = JSON.parse(
    readFileSync(rel("../src-tauri/resources/gun-codes/rules.json"), "utf8"),
  );

  // 用户提到的枪 + 我发现的 14 把缺失枪
  const probe = [
    "汤姆逊", "SR-3M", "SCAR-H", "ASH-12", "AK-12", "AS Val",
    "AKS-74U", "FS-12", "QBZ-95", "93R", "CAR-15", "PTR-32",
    "PSG-1", "SR9", "SV-98",
    // 变体写法
    "AK12", "SR3M", "QJB201", "MCX LT", "复合弓",
    // 未收录
    "某把新枪",
  ];

  console.log("=== 注入前（仅兜底表，模拟 meta 未返回的首帧）===");
  const before = probe.map((g) => [g, m.catOf(g)]);
  for (const [g, c] of before) {
    console.log(`  ${g.padEnd(10)} → ${c}${c === "其他" ? "  ← 归入其他" : ""}`);
  }

  console.log();
  console.log("=== 注入后（后端 guns[] 下发）===");
  m.setGunTable(
    Object.entries(rules.guns).map(([name, meta]) => ({
      name,
      category: meta.category,
      aliases: meta.aliases ?? [],
    })),
  );
  const after = probe.map((g) => [g, m.catOf(g)]);
  for (const [g, c] of after) {
    const b = before.find(([x]) => x === g)[1];
    const diff = b !== c ? `  (原为 ${b})` : "";
    console.log(`  ${g.padEnd(10)} → ${c}${diff}`);
  }

  console.log();
  const beforeOther = before.filter(([, c]) => c === "其他").map(([g]) => g);
  const afterOther = after.filter(([, c]) => c === "其他").map(([g]) => g);
  console.log("=== 结论 ===");
  console.log(`  注入前归入「其他」: ${beforeOther.length} 个 → ${beforeOther.join(", ") || "无"}`);
  console.log(`  注入后归入「其他」: ${afterOther.length} 个 → ${afterOther.join(", ") || "无"}`);
  console.log(
    afterOther.length === 1 && afterOther[0] === "某把新枪"
      ? "  ✅ 仅真正未收录的枪归「其他」，其余全部正确分类"
      : "  ⚠️ 仍有已收录枪归入「其他」",
  );
} finally {
  rmSync(tmp, { recursive: true, force: true });
}
