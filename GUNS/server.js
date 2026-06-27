const express = require('express');
const cors = require('cors');
const path = require('path');
const crypto = require('crypto');
const fs = require('fs');
const bcrypt = require('bcryptjs');
const initSqlJs = require('sql.js');

const app = express();
const PORT = process.env.PORT || 3002;

// 中间件
app.use(cors());
app.use(express.json());
app.use(express.static(__dirname));

// ── 安全配置 ──
const BCRYPT_ROUNDS = 10;
const TOKEN_EXPIRY_MS = 24 * 60 * 60 * 1000;    // 24 小时
const RATE_LIMIT_MAX = 5;                          // 同一 IP 最大失败次数
const RATE_LIMIT_WINDOW_MS = 5 * 60 * 1000;        // 封锁窗口 5 分钟
const adminTokens = new Map();                     // token -> { username, expiresAt }
const loginAttempts = new Map();                   // ip -> [{ time, success }]

// ── 数据库 ──
const DB_PATH = path.join(__dirname, 'guns.db');
let db;

function saveDb() {
  const data = db.export();
  const buffer = Buffer.from(data);
  fs.writeFileSync(DB_PATH, buffer);
}

function initDb() {
  // 直接使用同步 API
  db.run(`CREATE TABLE IF NOT EXISTS categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    icon TEXT DEFAULT '',
    sort_order INTEGER DEFAULT 0
  )`);

  db.run(`CREATE TABLE IF NOT EXISTS loadouts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    category_id INTEGER NOT NULL,
    weapon_name TEXT NOT NULL,
    code TEXT NOT NULL,
    description TEXT DEFAULT '',
    cost INTEGER DEFAULT 0,
    author TEXT DEFAULT '匿名',
    likes INTEGER DEFAULT 0,
    status TEXT DEFAULT 'pending',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (category_id) REFERENCES categories(id)
  )`);

  db.run(`CREATE TABLE IF NOT EXISTS admins (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL
  )`);

  // 检查是否有种子数据
  const row = db.exec('SELECT COUNT(*) as count FROM categories');
  if (row.length > 0 && row[0].values[0][0] === 0) {
    seedData();
  }

  // 迁移: 为已有数据库添加 cost 列
  try {
    db.run('ALTER TABLE loadouts ADD COLUMN cost INTEGER DEFAULT 0');
  } catch (_) { /* 列已存在则忽略 */ }

  // 迁移: 添加 reported 列（用户报告无法使用）
  try {
    db.run('ALTER TABLE loadouts ADD COLUMN reported INTEGER DEFAULT 0');
  } catch (_) { /* 列已存在则忽略 */ }
}

function seedData() {
  const insertCat = db.prepare('INSERT INTO categories (name, icon, sort_order) VALUES (?, ?, ?)');
  const categories = [
    ['步枪', '', 1],
    ['冲锋枪', '', 2],
    ['狙击步枪', '', 3],
    ['霰弹枪', '', 4],
    ['射手步枪', '', 5],
    ['轻机枪', '', 6],
    ['手枪', '', 7],
    ['特殊武器', '', 8],
  ];

  for (const cat of categories) {
    insertCat.run(cat);
  }
  insertCat.free();

  const insertLoadout = db.prepare(
    'INSERT INTO loadouts (category_id, weapon_name, code, description, cost, author, status, likes) VALUES (?, ?, ?, ?, ?, ?, ?, ?)'
  );

  const loadouts = [
    // 步枪
    [1, 'M7战斗步枪',     'M7-稳定控枪-A1B2C3D4',      '适合中距离扫射，后坐力控制优秀',       38000, '枪械大师', 'approved', 42],
    [1, 'K437突击步枪',   'K437-均衡配置-E5F6G7H8',     '全能型突击步枪，适应多种场景',         42000, '战术专家', 'approved', 35],
    [1, 'ASH-12战斗步枪','ASH12-高伤-I9J0K1L2',        '大口径重击，近距离爆发力强',           45000, '重装玩家', 'approved', 28],
    [1, 'K416突击步枪',   'K416-精准-M3N4O5P6',         '高精度配置，点射利器',                 48000, '三角洲老兵','approved', 38],
    [1, 'KC17突击步枪',   'KC17-轻量-Q7R8S9T0',         '轻量化设计，机动性出色',               35000, '速攻玩家', 'approved', 22],
    [1, 'ASVal突击步枪',  'ASVal-消音-U1V2W3X4',         '内置消音器，隐蔽作战首选',             52000, '潜行专家', 'approved', 31],
    [1, 'M4A1突击步枪',   'M4A1-经典-Y5Z6A7B8',          '经典突击步枪，可靠耐用',               36000, '老玩家',   'approved', 45],
    [1, 'AUG突击步枪',    'AUG-模块-C9D0E1F2',           '无托设计，紧凑灵活',                   40000, '室内战玩家','approved', 25],
    [1, 'AK-12突击步枪',  'AK12-火力-G3H4I5J6',          'AK系列现代改型，火力凶猛',             38000, 'AK爱好者', 'approved', 47],
    [1, 'SCAR-H战斗步枪', 'SCAR-H-重型-K7L8M9N0',        '7.62mm高伤害配置',                     55000, '战术专家', 'approved', 27],
    [1, 'AKM突击步枪',    'AKM-经典-O1P2Q3R4',           '经典突击步枪，简单粗暴',               32000, '怀旧玩家', 'approved', 33],
    [1, '腾龙突击步枪',   '腾龙-S5T6U7V8',               '特殊改装型号，独特手感',               50000, '收藏家',   'approved', 15],
    [1, 'SG552突击步枪',  'SG552-精确-W9X0Y1Z2',          '瑞士工艺，精度与射速兼顾',             43000, '精准控',   'approved', 21],
    [1, 'G3战斗步枪',     'G3-全能-A3B4C5D6',             '综合性能优异，新手友好',               37000, '新人推荐', 'approved', 18],
    [1, 'QBZ95-1突击步枪','QBZ951-国产-E7F8G9H0',        '国产突击步枪，三发点射稳定',           35000, '国货支持', 'approved', 29],
    [1, 'PTR-32突击步枪', 'PTR32-中距-I1J2K3L4',          '中距离作战优化，后坐力低',             34000, '控枪党',   'approved', 24],
    [1, 'CAR-15突击步枪', 'CAR15-经典-M5N6O7P8',          '经典卡宾枪，短小精悍',                 30000, '复古风',   'approved', 20],
    [1, 'M16A4突击步枪',  'M16A4-三点-Q9R0S1T2',          '三发点射模式，中远距离压制',           36000, '点射手',   'approved', 26],
    [1, 'AKS-74U突击步枪','AKS74U-折叠-U3V4W5X6',         '折叠枪托设计，便携性提升',             31000, '机动兵',   'approved', 30],
    [1, 'MK47突击步枪',   'MK47-重击-Y7Z8A9B0',           'MK系列突击变体，威力强劲',             46000, '火力控',   'approved', 36],
    [1, 'AR57突击步枪',   'AR57-轻快-C1D2E3F4',           '超轻量突击步枪，跑打神器',             39000, '跑打流',   'approved', 23],
    [1, 'MCXLT突击步枪',  'MCXLT-模块-G5H6I7J8',          '模块化设计，可自定义程度高',           44000, 'DIY玩家',  'approved', 17],
    [1, 'SR-3M紧凑突击步枪','SR3M-紧凑-Z9A0B1C2',         '紧凑型设计，近战爆发力强',             42000, '特战玩家',  'approved', 29],
    // 冲锋枪
    [2, 'Vector冲锋枪',   'Vector-超高速-B8C7D6E5',       '理论射速最高配置，近距离压制力极强',     28000, '速射玩家', 'approved', 44],
    [2, 'MP7冲锋枪',      'MP7-轻便-F4G5H6I7',            '轻量化冲锋枪，机动性出色',             32000, '跑打流',   'approved', 31],
    [2, 'P90冲锋枪',      'P90-大弹匣-J8K9L0M1',          '50发弹匣，持久火力输出',               35000, '弹药大师', 'approved', 19],
    [2, 'QCQ17冲锋枪',    'QCQ17-近战-N2O3P4Q5',          '国产新型冲锋枪，CQB表现出色',           25000, '国货支持', 'approved', 27],
    [2, 'MP5冲锋枪',      'MP5-经典-R6S7T8U9',            '经典冲锋枪，稳定可靠',                  30000, '老玩家',   'approved', 35],
    [2, 'SMG-45冲锋枪',   'SMG45-火力-V0W1X2Y3',          '.45口径大威力，近距离压制',             34000, '重装玩家', 'approved', 23],
    [2, '勇士冲锋枪',      '勇士-均衡-Z4A5B6C7',            '综合性能优异，适合多种场景',           26000, '全能型',   'approved', 18],
    [2, '野牛冲锋枪',      '野牛-大容量-D8E9F0G1',          '大弹鼓供弹，持续压制能力突出',         27000, '火力覆盖', 'approved', 26],
    [2, 'UZI冲锋枪',       'UZI-经典-H2I3J4K5',            '经典冲锋枪，射速快体积小',             22000, '怀旧玩家', 'approved', 21],
    [2, 'MK4冲锋枪',       'MK4-模块-L6M7N8O9',            '模块化设计，可自定义配置',             29000, 'DIY玩家',  'approved', 16],
    // 狙击步枪
    [3, 'AWM狙击步枪',    'AWM-远距狙杀-K9L8M7N6',      '一发致命，极致远距离精度',              68000, '狙击之神', 'approved', 56],
    [3, 'M700狙击步枪',   'M700-快速拉栓-P3Q4R5S6',     '拉栓速度优化配置，快速连狙',           55000, '快枪手',   'approved', 31],
    [3, 'R93狙击步枪',    'R93-精准-T2U3V4W5',          '德系精密狙击步枪，中远距离稳定',        60000, '精准控',   'approved', 28],
    [3, 'SV-98狙击步枪',  'SV98-大口径-X6Y7Z8A9',       '俄系狙击步枪，威力强劲',               52000, '战术大师', 'approved', 24],
    [3, 'M82狙击步枪',    'M82-反器材-B1C2D3E4',         '.50口径反器材步枪，穿透力极强',         75000, '重火力',   'approved', 43],
    // 霰弹枪
    [4, '725双管霰弹枪',  '725-双管-B2C3D4E5',           '经典双管霰弹枪，近距离一发入魂',         28000, '喷子王',   'approved', 34],
    [4, 'S12K霰弹枪',     'S12K-半自动-F6G7H8I9',        '半自动霰弹枪，射速快火力猛',            35000, '火力控',   'approved', 27],
    [4, 'M1014霰弹枪',    'M1014-战术-J0K1L2M3',         '战术霰弹枪，适应多种作战场景',           42000, '全能战士', 'approved', 31],
    [4, 'M870霰弹枪',     'M870-经典-N4O5P6Q7',          '经典泵动霰弹枪，可靠耐用',              32000, '老猎人',   'approved', 22],
    [4, 'FS-12霰弹枪',    'FS12-重型-R8S9T0U1',          '重型霰弹枪，大威力压制',                38000, '重装玩家', 'approved', 18],
    // 射手步枪
    [5, 'SR-25射手步枪',  'SR25-精确-V2W3X4Y5',          '高精度半自动射手步枪',                  48000, '精确射手', 'approved', 29],
    [5, 'M14射手步枪',    'M14-经典-Z6A7B8C9',           '经典战斗步枪改射手配置',                42000, '老兵',     'approved', 35],
    [5, 'SVD狙击步枪',    'SVD-俄系-D0E1F2G3',           '俄系半自动精确射击步枪',                50000, '战术大师', 'approved', 41],
    [5, 'PSG-1射手步枪',  'PSG1-精密-H4I5J6K7',          '德系高精度射手步枪',                    58000, '精准控',   'approved', 26],
    [5, 'VSS射手步枪',    'VSS-消音-L8M9N0O1',           '内置消音器，隐蔽精确打击',              46000, '潜行专家', 'approved', 33],
    [5, 'Mini-14射手步枪','Mini14-轻便-P2Q3R4S5',         '轻量化射手步枪，机动性好',              36000, '游骑兵',   'approved', 20],
    [5, 'SKS射手步枪',    'SKS-半自动-T6U7V8W9',         '经典半自动步枪，中距离精确',             38000, '怀旧玩家', 'approved', 24],
    [5, 'SR9射手步枪',    'SR9-精准-X0Y1Z2A3',           '现代半自动射手步枪',                    40000, '新锐射手', 'approved', 17],
    [5, 'Marlin杠杆步枪', 'Marlin-杠杆-B4C5D6E7',        '经典杠杆式步枪，独特射击手感',           32000, '西部枪手', 'approved', 14],
    // 轻机枪
    [6, 'M250通用机枪',   'M250-通用-F8G9H0I1',          '现代通用机枪，火力持续',                42000, '机枪手',   'approved', 28],
    [6, 'M249轻机枪',     'M249-火力-J2K3L4M5',          '班用轻机枪，压制火力强劲',              45000, '火力支援', 'approved', 38],
    [6, 'PKM通用机枪',    'PKM-俄制-N6O7P8Q9',           '俄系通用机枪，经典型号',                38000, '老兵',     'approved', 33],
    [6, 'QJB201轻机枪',   'QJB201-国产-R0S1T2U3',        '国产新型轻机枪，性能稳定',              40000, '国货支持', 'approved', 21],
    // 手枪
    [7, 'G18',            'G18-全自动-V4W5X6Y7',         '全自动手枪，近距爆发力强',              12000, '手枪狂人', 'approved', 23],
    [7, '沙漠之鹰',       '沙漠之鹰-大口径-Z8A9B0C1',     '.50口径大威力手枪',                     15000, '重火力',   'approved', 29],
    [7, 'M1911',          'M1911-经典-D2E3F4G5',         '经典手枪，.45ACP大威力',                8000, '怀旧玩家', 'approved', 18],
    [7, '93R',            '93R-三连发-H6I7J8K9',         '三发点射手枪，精准控制',                10000, '点射手',   'approved', 14],
    [7, '.357左轮',       '357左轮-L0M1N2O3',            '经典左轮手枪，威力强劲',                13000, '西部枪手', 'approved', 21],
    [7, 'G17',            'G17-均衡-P4Q5R6S7',           '通用型手枪，综合性能优异',              9000, '全能型',   'approved', 16],
    [7, 'QSZ92G',         'QSZ92G-国产-T8U9V0W1',        '国产手枪，可靠耐用',                    7000, '国货支持', 'approved', 12],
    // 特殊武器
    [8, '复合弓',         '复合弓-静默-X2Y3Z4A5',        '无声远程武器，隐蔽猎杀',                25000, '潜行大师', 'approved', 11],
  ];

  for (const l of loadouts) {
    insertLoadout.run(l);
  }
  insertLoadout.free();

  // 管理用户 (bcrypt 加盐哈希)
  const users = [
    ['muliu', 'muliunb666'],
    ['lml', '78512611324'],
    ['niko', 'Mm6580439'],
    ['chujian', 'qqwwee112233'],
  ];
  for (const [u, p] of users) {
    const existing = queryOne('SELECT id FROM admins WHERE username = ?', [u]);
    if (!existing) {
      const hash = bcrypt.hashSync(p, BCRYPT_ROUNDS);
      db.run('INSERT INTO admins (username, password_hash) VALUES (?, ?)', [u, hash]);
    }
  }

  saveDb();
}

function query(sql, params = []) {
  const stmt = db.prepare(sql);
  if (sql.trim().toUpperCase().startsWith('SELECT') || sql.trim().toUpperCase().startsWith('WITH')) {
    stmt.bind(params);
    const rows = [];
    while (stmt.step()) {
      rows.push(stmt.getAsObject());
    }
    stmt.free();
    return rows;
  }
  const result = stmt.run(params);
  stmt.free();
  saveDb();
  return result;
}

function queryOne(sql, params = []) {
  const rows = query(sql, params);
  return rows.length > 0 ? rows[0] : null;
}

// ── 初始化 ──
async function start() {
  const SQL = await initSqlJs();

  if (fs.existsSync(DB_PATH)) {
    const buffer = fs.readFileSync(DB_PATH);
    db = new SQL.Database(buffer);
  } else {
    db = new SQL.Database();
  }

  initDb();
  startServer();
}

// ── 启动服务器 ──
function startServer() {
  // ── 公开 API ──

  // 获取分类列表
  app.get('/api/categories', (req, res) => {
    try {
      const rows = query(
        'SELECT c.*, (SELECT COUNT(*) FROM loadouts l WHERE l.category_id = c.id AND l.status = ?) as loadout_count FROM categories c ORDER BY c.sort_order',
        ['approved']
      );
      res.json(rows);
    } catch (err) {
      res.status(500).json({ error: err.message });
    }
  });

  // 获取分类下的所有武器
  app.get('/api/weapons/:categoryId', (req, res) => {
    try {
      const { categoryId } = req.params;
      const rows = query(
        'SELECT weapon_name, COUNT(*) as count FROM loadouts WHERE category_id = ? AND status = ? GROUP BY weapon_name ORDER BY count DESC',
        [categoryId, 'approved']
      );
      res.json(rows);
    } catch (err) {
      res.status(500).json({ error: err.message });
    }
  });

  // 获取改枪码列表
  app.get('/api/loadouts', (req, res) => {
    try {
      const { category_id, search, weapon_name, page = 1 } = req.query;
      const limit = 50;
      const offset = (Math.max(1, parseInt(page)) - 1) * limit;

      let where = 'WHERE l.status = ?';
      const params = ['approved'];

      if (category_id && category_id !== 'all') {
        where += ' AND l.category_id = ?';
        params.push(category_id);
      }

      if (weapon_name) {
        where += ' AND l.weapon_name = ?';
        params.push(weapon_name);
      }

      if (search) {
        where += ' AND (l.weapon_name LIKE ? OR l.code LIKE ? OR l.description LIKE ?)';
        const q = `%${search}%`;
        params.push(q, q, q);
      }

      const countRow = queryOne(`SELECT COUNT(*) as total FROM loadouts l ${where}`, params);
      const total = countRow.total;

      const rows = query(
        `SELECT l.*, c.name as category_name FROM loadouts l JOIN categories c ON l.category_id = c.id ${where} ORDER BY l.likes DESC, l.created_at DESC LIMIT ? OFFSET ?`,
        [...params, limit, offset]
      );

      res.json({ data: rows, total, page: parseInt(page), totalPages: Math.ceil(total / limit) });
    } catch (err) {
      res.status(500).json({ error: err.message });
    }
  });

  // 获取单个改枪码详情
  app.get('/api/loadouts/:id', (req, res) => {
    try {
      const row = queryOne(
        'SELECT l.*, c.name as category_name FROM loadouts l JOIN categories c ON l.category_id = c.id WHERE l.id = ?',
        [req.params.id]
      );
      if (!row) return res.status(404).json({ error: '改枪码不存在' });
      res.json(row);
    } catch (err) {
      res.status(500).json({ error: err.message });
    }
  });

  // 用户上传改枪码
  app.post('/api/loadouts', (req, res) => {
    try {
      const { category_id, weapon_name, code, cost, description, author } = req.body;

      if (!category_id || !weapon_name || !code) {
        return res.status(400).json({ error: '请填写必填字段（分类、武器名称、改枪码）' });
      }

      const stmt = db.prepare(
        'INSERT INTO loadouts (category_id, weapon_name, code, cost, description, author) VALUES (?, ?, ?, ?, ?, ?)'
      );
      const result = stmt.run([
        Number(category_id),
        String(weapon_name || '').trim(),
        String(code || '').trim(),
        Math.max(0, parseInt(cost) || 0),
        String(description || '').trim(),
        String(author || '匿名').trim()
      ]);
      stmt.free();
      saveDb();

      const row = queryOne('SELECT * FROM loadouts WHERE id = ?', [Number(result.lastInsertRowid)]);

      res.status(201).json(row || {
        id: Number(result.lastInsertRowid),
        category_id: Number(category_id),
        weapon_name: String(weapon_name || '').trim(),
        code: String(code || '').trim(),
        description: String(description || '').trim(),
        author: String(author || '匿名').trim(),
        status: 'pending',
        likes: 0
      });
    } catch (err) {
      console.error('Upload error:', err);
      res.status(500).json({ error: err.message || String(err) });
    }
  });

  // 点赞
  app.post('/api/loadouts/:id/like', (req, res) => {
    try {
      const row = queryOne('SELECT * FROM loadouts WHERE id = ?', [req.params.id]);
      if (!row) return res.status(404).json({ error: '改枪码不存在' });

      const updStmt = db.prepare('UPDATE loadouts SET likes = likes + 1 WHERE id = ?');
      updStmt.run([Number(req.params.id)]);
      updStmt.free();
      saveDb();

      const updated = queryOne('SELECT likes FROM loadouts WHERE id = ?', [req.params.id]);
      res.json({ id: req.params.id, likes: updated.likes });
    } catch (err) {
      res.status(500).json({ error: err.message });
    }
  });

  // 用户报告改枪码无法使用
  app.post('/api/loadouts/:id/report', (req, res) => {
    try {
      const row = queryOne('SELECT * FROM loadouts WHERE id = ?', [req.params.id]);
      if (!row) return res.status(404).json({ error: '改枪码不存在' });

      const updStmt = db.prepare('UPDATE loadouts SET reported = 1 WHERE id = ?');
      updStmt.run([Number(req.params.id)]);
      updStmt.free();
      saveDb();

      res.json({ message: '已报告' });
    } catch (err) {
      res.status(500).json({ error: err.message });
    }
  });

  // ── 管理后台 API ──

  // 登录限流检查
  function checkRateLimit(ip) {
    const now = Date.now();
    const attempts = (loginAttempts.get(ip) || []).filter(a => now - a.time < RATE_LIMIT_WINDOW_MS);
    const failures = attempts.filter(a => !a.success).length;
    if (failures >= RATE_LIMIT_MAX) {
      const oldest = attempts[0].time;
      const waitMs = RATE_LIMIT_WINDOW_MS - (now - oldest);
      return { blocked: true, waitSec: Math.ceil(waitMs / 1000) };
    }
    return { blocked: false };
  }

  function recordLoginAttempt(ip, success) {
    const now = Date.now();
    const attempts = (loginAttempts.get(ip) || []).filter(a => now - a.time < RATE_LIMIT_WINDOW_MS);
    attempts.push({ time: now, success });
    loginAttempts.set(ip, attempts);
  }

  // 清理过期 token
  function cleanExpiredTokens() {
    const now = Date.now();
    for (const [token, data] of adminTokens) {
      if (now > data.expiresAt) adminTokens.delete(token);
    }
  }

  // 管理员登录
  app.post('/api/admin/login', (req, res) => {
    try {
      const ip = req.ip || req.connection.remoteAddress || 'unknown';

      // 限流检查
      const limit = checkRateLimit(ip);
      if (limit.blocked) {
        return res.status(429).json({
          error: `登录失败次数过多，请在 ${limit.waitSec} 秒后重试`
        });
      }

      const { username, password } = req.body;
      const admin = queryOne('SELECT * FROM admins WHERE username = ?', [username]);

      if (!admin || !bcrypt.compareSync(password || '', admin.password_hash)) {
        recordLoginAttempt(ip, false);
        return res.status(401).json({ error: '用户名或密码错误' });
      }

      recordLoginAttempt(ip, true);
      cleanExpiredTokens();

      const token = crypto.randomBytes(32).toString('hex');
      adminTokens.set(token, {
        username: admin.username,
        expiresAt: Date.now() + TOKEN_EXPIRY_MS
      });

      res.json({ token, username: admin.username, expiresAt: Date.now() + TOKEN_EXPIRY_MS });
    } catch (err) {
      res.status(500).json({ error: err.message });
    }
  });

  // 认证中间件
  function requireAdmin(req, res, next) {
    const token = req.headers.authorization?.replace('Bearer ', '');
    const session = adminTokens.get(token);

    if (!token || !session) {
      return res.status(401).json({ error: '未授权，请重新登录' });
    }

    if (Date.now() > session.expiresAt) {
      adminTokens.delete(token);
      return res.status(401).json({ error: '登录已过期，请重新登录' });
    }

    next();
  }

  // 获取所有改枪码（含待审核）
  app.get('/api/admin/loadouts', requireAdmin, (req, res) => {
    try {
      const { status, category_id, weapon_name, page = 1 } = req.query;
      const limit = 100;
      const offset = (Math.max(1, parseInt(page)) - 1) * limit;

      let where = 'WHERE 1=1';
      const params = [];

      if (status && status !== 'all') {
        where += ' AND l.status = ?';
        params.push(status);
      }

      if (category_id && category_id !== 'all') {
        where += ' AND l.category_id = ?';
        params.push(category_id);
      }

      if (weapon_name) {
        where += ' AND l.weapon_name = ?';
        params.push(weapon_name);
      }

      const countRow = queryOne(`SELECT COUNT(*) as total FROM loadouts l ${where}`, params);

      const rows = query(
        `SELECT l.*, c.name as category_name FROM loadouts l JOIN categories c ON l.category_id = c.id ${where} ORDER BY l.created_at DESC LIMIT ? OFFSET ?`,
        [...params, limit, offset]
      );

      res.json({ data: rows, total: countRow.total, page: parseInt(page), totalPages: Math.ceil(countRow.total / limit) });
    } catch (err) {
      res.status(500).json({ error: err.message });
    }
  });

  // Admin: 获取分类下的武器列表（全部状态）
  app.get('/api/admin/weapons/:categoryId', requireAdmin, (req, res) => {
    try {
      const { categoryId } = req.params;
      const rows = query(
        'SELECT weapon_name, COUNT(*) as count FROM loadouts WHERE category_id = ? GROUP BY weapon_name ORDER BY count DESC',
        [categoryId]
      );
      res.json(rows);
    } catch (err) {
      res.status(500).json({ error: err.message });
    }
  });

  // 审核/编辑改枪码
  app.put('/api/admin/loadouts/:id', requireAdmin, (req, res) => {
    try {
      const { status, weapon_name, code, cost, description, reported } = req.body;
      const loadout = queryOne('SELECT * FROM loadouts WHERE id = ?', [req.params.id]);
      if (!loadout) return res.status(404).json({ error: '改枪码不存在' });

      const updates = [];
      const params = [];

      if (status) { updates.push('status = ?'); params.push(status); }
      if (weapon_name) { updates.push('weapon_name = ?'); params.push(weapon_name); }
      if (code) { updates.push('code = ?'); params.push(code); }
      if (cost !== undefined) { updates.push('cost = ?'); params.push(Math.max(0, parseInt(cost) || 0)); }
      if (description !== undefined) { updates.push('description = ?'); params.push(description); }
      if (reported !== undefined) { updates.push('reported = ?'); params.push(reported ? 1 : 0); }

      if (updates.length === 0) {
        return res.status(400).json({ error: '没有需要更新的字段' });
      }

      params.push(Number(req.params.id));
      const updStmt2 = db.prepare(`UPDATE loadouts SET ${updates.join(', ')} WHERE id = ?`);
      updStmt2.run(params);
      updStmt2.free();
      saveDb();

      const updated = queryOne(
        'SELECT l.*, c.name as category_name FROM loadouts l JOIN categories c ON l.category_id = c.id WHERE l.id = ?',
        [req.params.id]
      );
      res.json(updated);
    } catch (err) {
      res.status(500).json({ error: err.message });
    }
  });

  // 删除改枪码
  app.delete('/api/admin/loadouts/:id', requireAdmin, (req, res) => {
    try {
      const loadout = queryOne('SELECT * FROM loadouts WHERE id = ?', [req.params.id]);
      if (!loadout) return res.status(404).json({ error: '改枪码不存在' });

      const delStmt = db.prepare('DELETE FROM loadouts WHERE id = ?');
      delStmt.run([Number(req.params.id)]);
      delStmt.free();
      saveDb();
      res.json({ message: '删除成功' });
    } catch (err) {
      res.status(500).json({ error: err.message });
    }
  });

  app.listen(PORT, () => {
    console.log(`三角洲改枪码平台运行在 http://localhost:${PORT}`);
    console.log(`API: http://localhost:${PORT}/api/loadouts`);
    console.log(`管理后台: http://localhost:${PORT}/admin.html`);
  });
}

start().catch(err => {
  console.error('启动失败:', err);
  process.exit(1);
});
