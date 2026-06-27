const API_BASE = window.location.origin;
let token = sessionStorage.getItem('admin_token') || '';
let tokenExpiresAt = Number(sessionStorage.getItem('admin_token_expires_at') || 0);
let currentFilter = 'all';
let currentCategoryId = 'all';
let currentWeaponName = '';
let categories = [];
let editingId = null;

// 检查本地 token 是否已过期
if (token && Date.now() > tokenExpiresAt) {
  token = '';
  tokenExpiresAt = 0;
  sessionStorage.removeItem('admin_token');
  sessionStorage.removeItem('admin_token_expires_at');
}

function isTokenExpired() {
  return Date.now() > tokenExpiresAt;
}

// ── 用户名 → 显示名映射 ──
const DISPLAY_NAMES = {
  muliu: '木流',
  lml: 'lml',
  niko: '小羊',
  chujian: '初见',
};

// ── DOM ──
const authScreen = document.getElementById('authScreen');
const adminPanel = document.getElementById('adminPanel');
const loginUser = document.getElementById('loginUser');
const loginPass = document.getElementById('loginPass');
const loginBtn = document.getElementById('loginBtn');
const loginError = document.getElementById('loginError');
const logoutBtn = document.getElementById('logoutBtn');
const adminTbody = document.getElementById('adminTbody');
const statBadge = document.getElementById('statBadge');
const toast = document.getElementById('adminToast');
const currentAdmin = document.getElementById('currentAdmin');

// ── Toast ──
function showToast(msg) {
  toast.textContent = msg;
  toast.classList.add('show');
  clearTimeout(toast._t);
  toast._t = setTimeout(() => toast.classList.remove('show'), 2000);
}

// ── Auth ──
async function handleLogin() {
  loginError.classList.remove('show');
  loginBtn.disabled = true;
  loginBtn.textContent = '登录中...';

  try {
    const uname = loginUser.value.trim();
    const pwd = loginPass.value.trim();
    if (!uname || !pwd) {
      loginError.textContent = '请输入用户名和密码';
      loginError.classList.add('show');
      loginBtn.disabled = false;
      loginBtn.textContent = '登录';
      return;
    }
    loginError.classList.remove('show');

    const res = await fetch(`${API_BASE}/api/admin/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username: uname, password: pwd }),
    });
    const data = await res.json();
    if (!res.ok) throw new Error(data.error || '登录失败');
    token = data.token;
    tokenExpiresAt = data.expiresAt || (Date.now() + 86400000);
    const displayUser = data.username || '未知';
    sessionStorage.setItem('admin_token', token);
    sessionStorage.setItem('admin_token_expires_at', String(tokenExpiresAt));
    sessionStorage.setItem('admin_username', displayUser);
    showAdminPanel();
    showToast(`欢迎回来，${displayUser}`);
  } catch (err) {
    loginError.textContent = err.message;
    loginError.classList.add('show');
    loginBtn.disabled = false;
    loginBtn.textContent = '登录';
  }
}

loginBtn.addEventListener('click', handleLogin);
loginPass.addEventListener('keydown', (e) => { if (e.key === 'Enter') handleLogin(); });

logoutBtn.addEventListener('click', () => {
  token = '';
  tokenExpiresAt = 0;
  sessionStorage.removeItem('admin_token');
  sessionStorage.removeItem('admin_token_expires_at');
  sessionStorage.removeItem('admin_username');
  authScreen.style.display = 'flex';
  adminPanel.style.display = 'none';
});

// ── Admin API ──
async function adminFetch(path, options = {}) {
  // 检查本地 token 是否已过期
  if (token && Date.now() > tokenExpiresAt) {
    sessionStorage.removeItem('admin_token');
    sessionStorage.removeItem('admin_token_expires_at');
    sessionStorage.removeItem('admin_username');
    token = '';
    tokenExpiresAt = 0;
    authScreen.style.display = 'flex';
    adminPanel.style.display = 'none';
    throw new Error('登录已过期，请重新登录');
  }

  const res = await fetch(`${API_BASE}${path}`, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${token}`,
      ...options.headers,
    },
  });

  // 401 → token 失效，跳回登录
  if (res.status === 401) {
    sessionStorage.removeItem('admin_token');
    sessionStorage.removeItem('admin_token_expires_at');
    sessionStorage.removeItem('admin_username');
    token = '';
    tokenExpiresAt = 0;
    authScreen.style.display = 'flex';
    adminPanel.style.display = 'none';
    throw new Error('登录已过期，请重新登录');
  }

  // 尝试解析 JSON，失败则返回文本
  let data;
  const text = await res.text();
  try {
    data = JSON.parse(text);
  } catch (_) {
    throw new Error(`服务器返回格式错误 (${res.status}): ${text.slice(0, 100)}`);
  }

  if (!res.ok) throw new Error(data.error || '请求失败');
  return data;
}

// ── 加载数据 ──
async function loadAdminData() {
  adminTbody.innerHTML = '<tr><td colspan="9" class="loading">加载中...</td></tr>';
  try {
    const params = new URLSearchParams();
    if (currentFilter !== 'all') params.set('status', currentFilter);
    if (currentCategoryId !== 'all') params.set('category_id', currentCategoryId);
    if (currentWeaponName) params.set('weapon_name', currentWeaponName);

    const result = await adminFetch(`/api/admin/loadouts?${params}`);
    renderAdminTable(result.data);

    // 获取全量统计（不受分类/武器筛选影响）
    const all = await adminFetch('/api/admin/loadouts?page=1');

    let badgeText = `共 ${result.total} 条`;
    if (currentCategoryId !== 'all' || currentWeaponName) {
      badgeText += ` (筛选)`;
    }
    badgeText += ` · 全部 ${all.total} 条 · 待审核 ${all.data.filter(d => d.status === 'pending').length} 条`;
    statBadge.textContent = badgeText;
  } catch (err) {
    adminTbody.innerHTML = `<tr><td colspan="8" class="loading">加载失败: ${err.message}</td></tr>`;
  }
}

function renderAdminTable(data) {
  if (data.length === 0) {
    adminTbody.innerHTML = '<tr><td colspan="9" class="loading">暂无数据</td></tr>';
    return;
  }
  adminTbody.innerHTML = data.map(item => {
    const statusClass = item.status === 'approved' ? 'approved' : item.status === 'pending' ? 'pending' : 'rejected';
    const statusLabel = item.status === 'approved' ? '已通过' : item.status === 'pending' ? '待审核' : '已拒绝';
    const costDisplay = item.cost ? Number(item.cost).toLocaleString() + '💰' : '-';
    const isEditing = editingId === item.id;
    const reportedDisplay = item.reported
      ? `<span class="status-badge reported-reported">已报告</span>`
      : `<span class="status-badge reported-clear">正常</span>`;

    let codeCell, costCell, actionCell;

    if (isEditing) {
      codeCell = `<input type="text" class="edit-input edit-code" value="${escHtml(item.code)}" />`;
      costCell = `<input type="number" class="edit-input edit-cost" value="${item.cost || 0}" style="width:80px" />`;
      actionCell = `
        <button class="action-btn approve" onclick="saveEdit(${item.id})">保存</button>
        <button class="action-btn reject" onclick="cancelEdit()">取消</button>
        <button class="action-btn delete" onclick="deleteLoadout(${item.id})">删除</button>
      `;
    } else {
      codeCell = `<span class="code-cell" title="${escHtml(item.code)}">${escHtml(item.code)}</span>`;
      costCell = costDisplay;
      actionCell = `
        ${item.status !== 'approved' ? `<button class="action-btn approve" onclick="updateStatus(${item.id}, 'approved')">通过</button>` : ''}
        ${item.status !== 'rejected' ? `<button class="action-btn reject" onclick="updateStatus(${item.id}, 'rejected')">拒绝</button>` : ''}
        <button class="action-btn edit-btn" onclick="startEdit(${item.id})">编辑</button>
        ${item.reported ? `<button class="action-btn clear-report-btn" onclick="clearReport(${item.id})">清除报告</button>` : ''}
        <button class="action-btn delete" onclick="deleteLoadout(${item.id})">删除</button>
      `;
    }

    return `<tr${isEditing ? ' class="editing-row"' : ''}>
      <td class="weapon-cell">${escHtml(item.weapon_name)}</td>
      <td>${escHtml(item.category_name || '')}</td>
      <td>${codeCell}</td>
      <td>${escHtml(item.author || '匿名')}</td>
      <td>${item.likes}</td>
      <td>${costCell}</td>
      <td>${reportedDisplay}</td>
      <td><span class="status-badge ${statusClass}">${statusLabel}</span></td>
      <td>${actionCell}</td>
    </tr>`;
  }).join('');
}

// ── 行内编辑 ──
function startEdit(id) {
  editingId = id;
  loadAdminData();
}

async function saveEdit(id) {
  const row = adminTbody.querySelector('.editing-row');
  if (!row) return;

  const codeInput = row.querySelector('.edit-code');
  const costInput = row.querySelector('.edit-cost');

  const code = codeInput.value.trim();
  const cost = parseInt(costInput.value) || 0;

  if (!code) {
    showToast('改枪码不能为空');
    return;
  }

  try {
    await adminFetch(`/api/admin/loadouts/${id}`, {
      method: 'PUT',
      body: JSON.stringify({ code, cost }),
    });
    showToast('修改成功');
    editingId = null;
    loadAdminData();
  } catch (err) {
    showToast('修改失败: ' + err.message);
  }
}

function cancelEdit() {
  editingId = null;
  loadAdminData();
}

function escHtml(s) {
  const d = document.createElement('div');
  d.textContent = s;
  return d.innerHTML;
}

// ── 操作 ──
async function updateStatus(id, status) {
  try {
    await adminFetch(`/api/admin/loadouts/${id}`, {
      method: 'PUT',
      body: JSON.stringify({ status }),
    });
    showToast(`操作成功`);
    loadAdminData();
  } catch (err) {
    showToast('操作失败: ' + err.message);
  }
}

async function deleteLoadout(id) {
  if (!confirm('确定要删除这条改枪码吗？')) return;
  try {
    await adminFetch(`/api/admin/loadouts/${id}`, { method: 'DELETE' });
    showToast('删除成功');
    loadAdminData();
  } catch (err) {
    showToast('删除失败: ' + err.message);
  }
}

// ── 清除报告 ──
async function clearReport(id) {
  try {
    await adminFetch(`/api/admin/loadouts/${id}`, {
      method: 'PUT',
      body: JSON.stringify({ reported: 0 }),
    });
    showToast('已清除报告标记');
    loadAdminData();
  } catch (err) {
    showToast('操作失败: ' + err.message);
  }
}

// ── 筛选 ──
document.querySelectorAll('.filter-btn').forEach(btn => {
  btn.addEventListener('click', () => {
    document.querySelectorAll('.filter-btn').forEach(b => b.classList.remove('active'));
    btn.classList.add('active');
    currentFilter = btn.dataset.filter;
    loadAdminData();
  });
});

// ── 显示管理面板 ──
function showAdminPanel() {
  const raw = sessionStorage.getItem('admin_username') || '-';
  currentAdmin.textContent = DISPLAY_NAMES[raw] || raw;
  authScreen.style.display = 'none';
  adminPanel.style.display = 'block';
  loginUser.value = '';
  loginPass.value = '';
  loadAdminData();
  loadAdminCategories();
}

// ── 自动登录检查 ──
if (token) {
  showAdminPanel();
} else {
  authScreen.style.display = 'flex';
  loginPass.focus();
}

// ── 分类 / 武器筛选 ──

async function loadAdminCategories() {
  try {
    categories = await adminFetch('/api/categories');
    renderAdminCategoryFilters();
  } catch (err) {
    console.error('Failed to load categories:', err);
  }
}

function renderAdminCategoryFilters() {
  const bar = document.getElementById('catFilterBar');
  let html = `<button class="cat-btn active" data-cat="all">全部分类</button>`;
  for (const cat of categories) {
    const name = cat.name + (cat.loadout_count != null ? ` (${cat.loadout_count})` : '');
    html += `<button class="cat-btn" data-cat="${cat.id}">${name}</button>`;
  }
  bar.innerHTML = html;
}

document.getElementById('catFilterBar').addEventListener('click', (e) => {
  const btn = e.target.closest('.cat-btn');
  if (!btn) return;
  document.querySelectorAll('.cat-btn').forEach(b => b.classList.remove('active'));
  btn.classList.add('active');
  currentCategoryId = btn.dataset.cat;
  currentWeaponName = '';

  if (currentCategoryId !== 'all') {
    loadAdminWeapons(currentCategoryId);
  } else {
    document.getElementById('weaponFilterBar').style.display = 'none';
    loadAdminData();
  }
});

async function loadAdminWeapons(catId) {
  try {
    const weapons = await adminFetch(`/api/admin/weapons/${catId}`);
    const bar = document.getElementById('weaponFilterBar');
    let html = `<button class="weapon-btn active" data-weapon="">全部武器</button>`;
    for (const w of weapons) {
      html += `<button class="weapon-btn" data-weapon="${escHtml(w.weapon_name).replace(/"/g, '&quot;')}">${escHtml(w.weapon_name)} (${w.count})</button>`;
    }
    bar.innerHTML = html;
    bar.style.display = 'flex';
    loadAdminData();
  } catch (err) {
    console.error('Failed to load weapons:', err);
  }
}

document.getElementById('weaponFilterBar').addEventListener('click', (e) => {
  const btn = e.target.closest('.weapon-btn');
  if (!btn) return;
  document.querySelectorAll('.weapon-btn').forEach(b => b.classList.remove('active'));
  btn.classList.add('active');
  currentWeaponName = btn.dataset.weapon;
  loadAdminData();
});
