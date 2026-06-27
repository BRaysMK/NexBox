const initSqlJs = require('sql.js');
const fs = require('fs');
const path = require('path');

const DB_PATH = path.join(__dirname, 'guns.db');
const FILE_PATH = path.join(__dirname, '轻机枪.txt');

const WEAPON_MAP = {
  'M250': 'M250通用机枪',
  'M249': 'M249轻机枪',
  'PKM': 'PKM通用机枪',
  'QJB': 'QJB201轻机枪',
};

function parseCost(s) {
  if (!s) return 0;
  const cleaned = s.replace(/^金额[:：]?\s*/i, '').trim();
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

    const titleMatch = line.match(/^([A-Za-z0-9\-]+|[\u4e00-\u9fff]+)$/);
    if (titleMatch) {
      const key = titleMatch[1].trim();
      currentWeapon = WEAPON_MAP[key];
      if (!currentWeapon) {
        console.warn(`未知武器标题: "${key}"`);
        currentWeapon = '';
      }
      continue;
    }

    if (!currentWeapon) continue;

    const costMatch = line.match(/^(.*?)\s+(金额[:：]?\s*)?([\d,]+)\s*$/);
    if (costMatch) {
      const code = costMatch[1].trim();
      const cost = parseCost((costMatch[2] || '') + costMatch[3]);
      rows.push([6, currentWeapon, code, cost, '木流', 'approved', 0]);
    } else {
      rows.push([6, currentWeapon, line, 0, '木流', 'approved', 0]);
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
  console.log(`已插入 ${rows.length} 条轻机枪记录，上传者：木流`);
  for (const r of rows) {
    console.log(`  ${r[1]} | ${r[3]}💰`);
  }
}

main().catch(err => {
  console.error('Error:', err);
  process.exit(1);
});