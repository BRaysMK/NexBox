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
    [1, 'AS Val突击步枪', 'ASVAL季中赛斜握骨架天蝎（均衡稳定）-6K7DQ8C0EU90O684D8QL5', 0, '霂祏毵', 'approved', 0],
    [1, 'AS Val突击步枪', 'ASVAL季中赛斜握骨架镂空（均衡操控）-6K7DQQS0EU90O684D8QL5', 0, '霂祏毵', 'approved', 0],
    [1, 'AS Val突击步枪', 'ASVAL季中赛共二锚点（65后坐62操控）-6K7OA50049H3TLFDHMKHO', 0, '霂祏毵', 'approved', 0],
    [1, 'AS Val突击步枪', 'ASVAL季中赛共二CT（63操控64操控）-6K7O9J8049H3TLFDHMKHO', 0, '霂祏毵', 'approved', 0],
    [1, 'AS Val突击步枪', 'ASVAL季中赛共二骨架（极限72操控）-6K7OAEC049H3TLFDHMKHO', 0, '霂祏毵', 'approved', 0],
    [1, 'AS Val突击步枪', 'ASVAL季中赛均衡密令/共振版A（更高操控）-6K7P11K0B1RRH96DI8AIT', 0, '霂祏毵', 'approved', 0],
    [1, 'AS Val突击步枪', 'ASVAL季中赛均衡密令/共振版B（8据枪）-6K7P1800B1RRH96DI8AIT', 0, '霂祏毵', 'approved', 0],
  ];

  for (const r of records) {
    insert.run(r);
  }
  insert.free();

  const data = db.export();
  fs.writeFileSync(DB_PATH, Buffer.from(data));
  console.log(`已插入 ${records.length} 条 AS Val 记录，上传者：霂祏毵`);
}

main().catch(err => {
  console.error('Error:', err);
  process.exit(1);
});