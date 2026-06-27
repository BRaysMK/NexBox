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
    [1, 'SR-3M紧凑突击步枪', 'SR-3M紧凑突击步枪-烽火地带-6HHUMQ803EU2NE978I9M2', 350000, '木流', 'approved', 0],
    [1, 'SR-3M紧凑突击步枪', 'SR-3M紧凑突击步枪-烽火地带-6J7KDUC09KG32LJRFUK9I', 290000, '木流', 'approved', 0],
    [1, 'SR-3M紧凑突击步枪', 'SR-3M紧凑突击步枪-烽火地带-6FP78QK0BR2D7T6S3C9FN', 230000, '木流', 'approved', 0],
    [1, 'SR-3M紧凑突击步枪', 'SR-3M紧凑突击步枪-烽火地带-6FFUL8S01P8BEJ3TPCUEH', 600000, '木流', 'approved', 0],
    [1, 'SR-3M紧凑突击步枪', 'SR-3M紧凑突击步枪-烽火地带-6GP0L4C0FI6SBO1DEQQ57', 670000, '木流', 'approved', 0],
    [1, 'SR-3M紧凑突击步枪', 'SR-3M紧凑突击步枪-烽火地带-6FPKC380BR2D7T6S3C9FN', 580000, '木流', 'approved', 0],
  ];

  for (const r of records) {
    insert.run(r);
  }
  insert.free();

  const data = db.export();
  fs.writeFileSync(DB_PATH, Buffer.from(data));
  console.log('已插入 6 条 SR-3M 记录');
}

main().catch(err => {
  console.error('Error:', err);
  process.exit(1);
});