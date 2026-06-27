const bcrypt = require('bcryptjs');
const initSqlJs = require('sql.js');
const fs = require('fs');
(async () => {
  const SQL = await initSqlJs();
  const db = new SQL.Database(fs.readFileSync('guns.db'));
  const existing = db.exec('SELECT id FROM admins WHERE username = "chujian"');
  if (existing.length > 0 && existing[0].values.length > 0) {
    console.log('chujian already exists, skipping');
  } else {
    const hash = bcrypt.hashSync('qqwwee112233', 10);
    db.run('INSERT INTO admins (username, password_hash) VALUES (?, ?)', ['chujian', hash]);
    fs.writeFileSync('guns.db', db.export());
    console.log('chujian added to database');
  }
  // Show all users
  const users = db.exec('SELECT username FROM admins');
  console.log('Current users:', JSON.stringify(users[0].values.flat()));
})();
