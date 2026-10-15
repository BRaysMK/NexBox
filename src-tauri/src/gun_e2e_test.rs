//! 端到端验证：用真实文档 TSV 检验三段式解析器
//!
//! 数据来源：`E:\something\AI Agent\xinjinhe\改枪码\` 的两个 xlsx
//! （由 `.dsh/scripts/export_tsv.py` 导出为 TSV，模拟用户从 Excel 粘贴）
//!
//! 运行：
//! ```bash
//! GUN_TSV_DIR="E:/something/AI Agent/xinjinhe/.dsh/tmp/tsv" cargo test --lib -- --ignored gun_e2e
//! ```
//!
//! 由于解析器依赖 `tauri::AppHandle`（用于读取规则库与学习表），
//! 本测试直接调用**纯函数部分**（`is_code_token` / `extract_fingerprint` /
//! `classify_row` / `match_gun_in_row` / `split_cells`）复现三段式流程，
//! 以验证算法在真实数据上的识别率。

#[cfg(test)]
mod gun_e2e {
    use crate::local_gun_codes::*;
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;

    fn tsv_dir() -> Option<PathBuf> {
        std::env::var("GUN_TSV_DIR").ok().map(PathBuf::from)
    }

    /// 用纯函数复现三段式解析（不依赖 AppHandle）
    fn parse_tsv(rules: &GunRules, text: &str) -> Vec<(String, Option<String>, String)> {
        // 构建别名索引
        let mut alias_idx: HashMap<String, String> = HashMap::new();
        for (canon, meta) in &rules.guns {
            alias_idx.entry(normalize(canon)).or_insert_with(|| canon.clone());
            for a in &meta.aliases {
                alias_idx.entry(normalize(a)).or_insert_with(|| canon.clone());
            }
        }
        for fp in rules.fingerprints.values() {
            if !fp.gun.is_empty() {
                alias_idx.entry(normalize(&fp.gun)).or_insert_with(|| fp.gun.clone());
            }
        }

        let mut out = Vec::new();
        let mut carry: Option<String> = None;
        let mut seen: HashSet<String> = HashSet::new();

        for raw in text.lines() {
            let line = raw.trim_end();
            if line.trim().is_empty() {
                continue;
            }
            let mut cells = split_cells(line);

            // 内嵌码拆解
            let mut extra: Vec<String> = Vec::new();
            for c in cells.iter_mut() {
                if is_code_token(c) {
                    continue;
                }
                if let Some(code) = {
                    c.split(|ch: char| !ch.is_ascii_alphanumeric())
                        .find(|t| is_code_token(t))
                        .map(|s| s.to_string())
                } {
                    let prefix = c.replace(&code, " ").trim().to_string();
                    *c = code;
                    if !prefix.is_empty() {
                        extra.push(prefix);
                    }
                }
            }
            cells.extend(extra);

            let code_positions: Vec<usize> = cells
                .iter()
                .enumerate()
                .filter(|(_, c)| is_code_token(c))
                .map(|(i, _)| i)
                .collect();

            // ① 行分类
            match classify_row(&cells, rules, !code_positions.is_empty()) {
                RowKind::Header => {
                    carry = None;
                    continue;
                }
                RowKind::Noise => continue,
                RowKind::Data => {}
            }

            // ② 同行匹配
            let is_bundle = is_bundle_row(&cells, rules);
            let mut row_gun: Option<String> = None;
            let mut best_len = 0usize;
            for c in &cells {
                if c.is_empty() || c.chars().count() > 16 || is_code_token(c) {
                    continue;
                }
                if let Some(g) = alias_idx.get(&normalize(c)) {
                    let l = normalize(g).chars().count();
                    if l > best_len {
                        best_len = l;
                        row_gun = Some(g.clone());
                    }
                }
            }
            if row_gun.is_some() {
                carry = row_gun.clone();
            }
            // ③ 跨行继承（套件码不继承）
            let use_gun = if is_bundle {
                None
            } else {
                row_gun.clone().or_else(|| carry.clone())
            };

            let first_idx = *code_positions.first().unwrap();
            let mut row_config = String::new();
            for j in (0..first_idx).rev() {
                let t = &cells[j];
                if t.is_empty() || is_code_token(t) || alias_idx.contains_key(&normalize(t)) {
                    continue;
                }
                if t.chars().count() <= 24 {
                    row_config = t.clone();
                    break;
                }
            }

            for &ci in &code_positions {
                let code = cells[ci].clone();
                let fp = extract_fingerprint(&code);
                if !seen.insert(fp.clone()) {
                    continue;
                }
                // 规则库命中优先
                let (gun, cfg) = if let Some(f) = rules.fingerprints.get(&fp) {
                    (Some(f.gun.clone()), if f.config.is_empty() { row_config.clone() } else { f.config.clone() })
                } else {
                    (use_gun.clone(), row_config.clone())
                };
                out.push((code, gun, cfg));
            }
        }
        out
    }

    #[test]
    #[ignore = "需要 GUN_TSV_DIR 环境变量指向真实数据"]
    fn gun_e2e_real_documents() {
        let Some(dir) = tsv_dir() else {
            eprintln!("跳过：未设置 GUN_TSV_DIR");
            return;
        };
        let rules: GunRules = serde_json::from_str(
            &std::fs::read_to_string(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("resources/gun-codes/rules.json"),
            )
            .expect("规则库读取失败"),
        )
        .expect("规则库解析失败");

        let mut total = 0usize;
        let mut recognized = 0usize;
        let mut unrecognized_samples: Vec<String> = Vec::new();

        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .expect("目录读取失败")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map(|x| x == "tsv").unwrap_or(false))
            .collect();
        files.sort();

        for f in &files {
            let text = std::fs::read_to_string(f).expect("TSV 读取失败");
            let parsed = parse_tsv(&rules, &text);
            let rec = parsed.iter().filter(|(_, g, _)| g.is_some()).count();
            println!(
                "  {} → 解析 {} 条，识别 {} 条",
                f.file_name().unwrap().to_string_lossy(),
                parsed.len(),
                rec
            );
            total += parsed.len();
            recognized += rec;
            for (code, gun, _) in parsed.iter().filter(|(_, g, _)| g.is_none()) {
                if unrecognized_samples.len() < 10 {
                    unrecognized_samples.push(code.clone());
                }
            }
        }

        println!("\n=== 端到端结果 ===");
        println!("文档数: {}", files.len());
        println!("解析码数: {total}");
        println!("已识别: {recognized}");
        println!(
            "识别率: {:.1}%",
            recognized as f64 * 100.0 / total.max(1) as f64
        );
        if !unrecognized_samples.is_empty() {
            println!("未识别样例: {unrecognized_samples:?}");
        }

        assert!(total > 500, "解析到的码数过少({total})，解析器可能失效");
        let rate = recognized as f64 / total.max(1) as f64;
        assert!(
            rate >= 0.95,
            "识别率 {:.1}% 低于 95% 门槛",
            rate * 100.0
        );

        // 反向校验：纯十六进制图片 ID 不得进入结果
        for f in &files {
            let text = std::fs::read_to_string(f).unwrap();
            let parsed = parse_tsv(&rules, &text);
            for (code, _, _) in &parsed {
                assert!(
                    !code.chars().all(|c| c.is_ascii_digit()
                        || ('A'..='F').contains(&c.to_ascii_uppercase())),
                    "纯十六进制图片 ID 混入结果: {code}"
                );
            }
        }

        // 张冠李戴校验：同一把枪的配置数不应超过该枪在规则库里的指纹数（粗校验）
        println!("反向校验通过：无图片 ID 混入");
    }
}
