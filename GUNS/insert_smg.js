const initSqlJs = require('sql.js');
const fs = require('fs');
const path = require('path');

const DB_PATH = path.join(__dirname, 'guns.db');
const FILE_PATH = path.join(__dirname, '冲锋枪.txt');

// 武器名映射：标题行 → 完整武器名
const WEAPON_MAP = {
  'Vector': 'Vector冲锋枪',
  'MP7': 'MP7冲锋枪',
  'P90': 'P90冲锋枪',
  'QCQ171': 'QCQ171冲锋枪',
  'QCQ1711': 'QCQ171冲锋枪', // typo guard
  'MP5': 'MP5冲锋枪',
  'SMG-45': 'SMG-45冲锋枪',
  '勇士': '勇士冲锋枪',
  'MK4': 'MK4冲锋枪',
};

function parseCost(s) {
  if (!s) return 0;
  // 去掉 "金额：" 或 "金额" 前缀
  const cleaned = s.replace(/^金额[:：]?\s*/, '').trim();
  const num = parseInt(cleaned.replace(/[^\d]/g, ''));
  return isNaN(num) ? 0 : num;
}

async function main() {
  const content = fs.readFileSync(FILE_PATH, 'utf-8');
  const lines = content.split(/\r?\n/);

  const rows = [];
  let currentWeapon = '';

  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line) continue;

    // 标题行（武器名）：以字母开头或中文开头，后面没有连字符
    const titleMatch = line.match(/^([A-Za-z0-9\-]+|[\u4e00-\u9fff]+)$/);
    if (titleMatch) {
      const key = titleMatch[1].trim();
      currentWeapon = WEAPON_MAP[key];
      if (!currentWeapon) {
        console.warn(`未知武器标题: "${key}"，跳过`);
        currentWeapon = '';
      }
      continue;
    }

    // 数据行：格式为 "代码 金额"
    if (!currentWeapon) continue;

    // 尝试从行尾提取金额
    const costMatch = line.match(/^(.*?)\s+(金额[:：]?\s*)?([\d,]+)\s*$/);
    if (costMatch) {
      const code = costMatch[1].trim();
      const cost = parseCost((costMatch[2] || '') + costMatch[3]);
      rows.push([2, currentWeapon, code, cost, '木流', 'approved', 0]);
    } else {
      // 尝试整行作为代码，金额为0
      rows.push([2, currentWeapon, line, 0, '木流', 'approved', 0]);
    }
  }

  if (rows.length === 0) {
    console.log('没有找到任何记录');
    return;
  }

  const SQL = await initSqlJs();
  const buffer = fs.readFileSync(DB_PATH);
  const db = new SQL.Database(buffer);

  const insert = db.prepare(
    'INSERT INTO loadouts (category_id, weapon_name, code, cost, author, status, likes) VALUES (?, ?, ?, ?, ?, ?, ?)'
  );

  for (const r of rows) {
    insert.run(r);
  }
  insert.free();

  const data = db.export();
  fs.writeFileSync(DB_PATH, Buffer.from(data));
  console.log(`已插入 ${rows.length} 条冲锋枪记录`);
  for (const r of rows) {
    console.log(`  ${r[1]} | ${r[2].substring(0, 40)}... | ${r[3]}💰`);
  }
}

main().catch(err => {
  console.error('Error:', err);
  process.exit(1);
});