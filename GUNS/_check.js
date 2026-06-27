const initSqlJs = require('sql.js');
const bcrypt = require('bcryptjs');
const fs = require('fs');
(async () => {
  const SQL = await initSqlJs();
  const db = new SQL.Database(fs.readFileSync('guns.db'));
  const r = db.exec('SELECT username, password_hash FROM admins WHERE username = "chujian"');
  if (r.length > 0 && r[0].values.length > 0) {
    const hash = r[0].values[0][1];
    const match = bcrypt.compareSync('qqwwee112233', hash);
    console.log('chujian exists:', true);
    console.log('password match:', match);
  } else {
    console.log('chujian not found in DB');
  }
})();
