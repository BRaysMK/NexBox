const initSqlJs = require('sql.js');
const fs = require('fs');
const path = require('path');

const DB_PATH = path.join(__dirname, 'guns.db');

async function main() {
  const SQL = await initSqlJs();
  const buffer = fs.readFileSync(DB_PATH);
  const db = new SQL.Database(buffer);

  const insert = db.prepare(
    'INSERT INTO loadouts (category_id, weapon_name, code, cost, author, status, likes) VALUES (?, ?, ?, ?, ?, ?, ?)'
  );

  const records = [
    [7, 'G18', 'G18-烽火地带-6H36OGO0EIJINPT6QF6HV', 114000, '木流', 'approved', 0],
    [7, '沙漠之鹰', '沙漠之鹰-烽火地带-6JR6AEC06ROD14M849JRD', 96000, '木流', 'approved', 0],
    [7, '93R', '93R-烽火地带-6G7ER6G053SUNNT5TTPUV', 77000, '木流', 'approved', 0],
    [7, 'G17', 'G17-烽火地带-6IDAUA001VJ5299V440CV', 130000, '木流', 'approved', 0],
  ];

  for (const r of records) {
    insert.run(r);
  }
  insert.free();

  const data = db.export();
  fs.writeFileSync(DB_PATH, Buffer.from(data));
  console.log(`已插入 ${records.length} 条手枪记录，上传者：木流`);
  for (const r of records) {
    console.log(`  ${r[1]} | ${r[3]}💰`);
  }
}

main().catch(err => {
  console.error('Error:', err);
  process.exit(1);
});