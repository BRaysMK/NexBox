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
    // S12K
    [4, 'S12K霰弹枪', 'S12K霰弹枪-烽火地带-6GT4NF80FUH1E5MQ63E4O', 200000, '木流', 'approved', 0],
    [4, 'S12K霰弹枪', 'S12K霰弹枪-烽火地带-6GQTM54095HC4609S9G3Q', 158000, '木流', 'approved', 0],
    [4, 'S12K霰弹枪', 'S12K霰弹枪-烽火地带-6FPJOK00FUH1E5MQ63E4O', 260000, '木流', 'approved', 0],
    [4, 'S12K霰弹枪', 'S12K霰弹枪-烽火地带-6HBCUF40CREMAGRTEVQUG', 200000, '木流', 'approved', 0],
    [4, 'S12K霰弹枪', 'S12K霰弹枪-烽火地带-6GQTMDC095HC4609S9G3Q', 250000, '木流', 'approved', 0],
    // M1014
    [4, 'M1014霰弹枪', 'M1014霰弹枪-烽火地带-6GQTQL4095HC4609S9G3Q', 70000, '木流', 'approved', 0],
    [4, 'M1014霰弹枪', 'M1014霰弹枪-烽火地带-6GQTRE4095HC4609S9G3Q', 160000, '木流', 'approved', 0],
    [4, 'M1014霰弹枪', 'M1014霰弹枪-烽火地带-6GQTR10095HC4609S9G3Q', 290000, '木流', 'approved', 0],
    [4, 'M1014霰弹枪', 'M1014霰弹枪-烽火地带-6HBCULC0CREMAGRTEVQUG', 124000, '木流', 'approved', 0],
  ];

  for (const r of records) {
    insert.run(r);
  }
  insert.free();

  const data = db.export();
  fs.writeFileSync(DB_PATH, Buffer.from(data));
  console.log(`已插入 ${records.length} 条霰弹枪记录，上传者：木流`);
  for (const r of records) {
    console.log(`  ${r[1]} | ${r[3]}💰`);
  }
}

main().catch(err => {
  console.error('Error:', err);
  process.exit(1);
});