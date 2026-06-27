const API_BASE = window.location.origin;

// ── 移动端侧栏切换 ──
const sidebar = document.getElementById('sidebar');
const sidebarOverlay = document.getElementById('sidebarOverlay');
const menuToggle = document.getElementById('menuToggle');

if (menuToggle && sidebar && sidebarOverlay) {
  function toggleSidebar(open) {
    sidebar.classList.toggle('open', open);
    sidebarOverlay.classList.toggle('open', open);
    document.body.style.overflow = open ? 'hidden' : '';
  }

  menuToggle.addEventListener('click', () => toggleSidebar(!sidebar.classList.contains('open')));
  sidebarOverlay.addEventListener('click', () => toggleSidebar(false));

  // 移动端：选中武器或上传后关闭侧栏（展开/折叠分类时不关）
  document.addEventListener('click', (e) => {
    if (window.innerWidth > 768) return;
    const trigger = e.target.closest('.nav-sub-item, .upload-btn');
    if (trigger) toggleSidebar(false);
  });
}

// ── 状态 ──
let categories = [];
let weaponsMap = {}; // { categoryId: [ {weapon_name, count}, ... ] }
let currentCategoryId = 'all';
let currentWeaponName = ''; // '' means all weapons in category
let currentSort = 'likes';

// ── DOM ──
const $ = (s) => document.querySelector(s);
const $$ = (s) => document.querySelectorAll(s);

const categoryNav = $('#categoryNav');
const loadoutList = $('#loadoutList');
const currentCategory = $('#currentCategory');
const loadoutCount = $('#loadoutCount');
const searchInput = $('#searchInput');
const sortBtns = $$('.sort-btn');
const uploadBtn = $('#uploadBtn');
const uploadModal = $('#uploadModal');
const modalClose = $('#modalClose');
const modalCancel = $('#modalCancel');
const formCategory = $('#formCategory');
const formWeapon = $('#formWeapon');
const formCode = $('#formCode');
const formDesc = $('#formDesc');
const formCost = $('#formCost');
const formAuthor = $('#formAuthor');
const formError = $('#formError');
const formSuccess = $('#formSuccess');
const formSubmit = $('#formSubmit');
const toast = $('#toast');

let searchTimer = null;

// ── API 调用 ──
async function apiGet(path) {
  const res = await fetch(`${API_BASE}${path}`);
  const text = await res.text();
  let data;
  try { data = JSON.parse(text); } catch (_) { throw new Error(`响应格式错误: ${text.slice(0, 100)}`); }
  if (!res.ok) throw new Error(data.error || String(res.status));
  return data;
}

async function apiPost(path, body) {
  const res = await fetch(`${API_BASE}${path}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  const text = await res.text();
  let data;
  try { data = JSON.parse(text); } catch (_) { throw new Error(`响应格式错误: ${text.slice(0, 100)}`); }
  if (!res.ok) throw new Error(data.error || String(res.status));
  return data;
}

// ── 工具 ──
function showToast(msg) {
  toast.textContent = msg;
  toast.classList.add('show');
  clearTimeout(toast._t);
  toast._t = setTimeout(() => toast.classList.remove('show'), 2000);
}

function escHtml(s) {
  const d = document.createElement('div');
  d.textContent = s;
  return d.innerHTML;
}

// ── 加载分类 & 武器 ──
async function loadCategories() {
  categories = await apiGet('/api/categories');
  // 并行加载每个分类下的武器列表
  const weaponsPromises = categories.map(cat =>
    apiGet(`/api/weapons/${cat.id}`).then(weapons => {
      weaponsMap[cat.id] = weapons;
    })
  );
  await Promise.all(weaponsPromises);
  renderCategories();
  populateFormCategories();
}

function renderCategories() {
  const totalCount = categories.reduce((sum, c) => sum + (c.loadout_count || 0), 0);

  let html = `<div class="nav-item nav-item-all ${currentCategoryId === 'all' ? 'active' : ''}" onclick="selectAll()">
    <span class="nav-item-label">全部</span>
    <span class="nav-count" id="countAll">${totalCount}</span>
  </div>`;

  for (const cat of categories) {
    const weapons = weaponsMap[cat.id] || [];
    const isExpanded = currentCategoryId == cat.id;
    const chevronIcon = weapons.length > 0
      ? `<svg class="nav-chevron ${isExpanded ? 'expanded' : ''}" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"/></svg>`
      : '';

    html += `<div class="nav-group">
      <div class="nav-item nav-item-cat ${currentCategoryId == cat.id && !currentWeaponName ? 'active' : ''}" data-id="${cat.id}" onclick="toggleCategory(${cat.id}, this)">
        ${chevronIcon}
        <span class="nav-item-label">${cat.name}</span>
        <span class="nav-count" id="count${cat.id}">${cat.loadout_count || 0}</span>
      </div>
      <div class="nav-sub-list ${isExpanded ? 'expanded' : ''}" id="subList${cat.id}">`;

    for (const w of weapons) {
      const isWeaponActive = currentCategoryId == cat.id && currentWeaponName === w.weapon_name;
      html += `<div class="nav-sub-item ${isWeaponActive ? 'active' : ''}" onclick="selectWeapon(${cat.id}, '${escHtml(w.weapon_name).replace(/'/g, "\\'")}', this)">
        <span class="nav-sub-label">${escHtml(w.weapon_name)}</span>
      </div>`;
    }

    html += `</div></div>`;
  }

  const label = categoryNav.querySelector('.nav-section-label');
  categoryNav.innerHTML = label.outerHTML + html;
}

function populateFormCategories() {
  formCategory.innerHTML = categories.map(c =>
    `<option value="${c.id}">${c.name}</option>`
  ).join('');
  // 默认选中第一个分类并加载对应武器
  if (categories.length > 0) {
    formCategory.value = categories[0].id;
    populateFormWeapons(categories[0].id);
  }
}

// 分类切换 → 更新武器下拉
function onCategoryChange() {
  const catId = parseInt(formCategory.value);
  if (catId) {
    populateFormWeapons(catId);
  } else {
    formWeapon.innerHTML = '<option value="">请先选择分类</option>';
  }
}

function populateFormWeapons(catId) {
  const weapons = weaponsMap[catId] || [];
  formWeapon.innerHTML = '<option value="">请选择武器</option>' +
    weapons.map(w => `<option value="${escHtml(w.weapon_name)}">${escHtml(w.weapon_name)}</option>`).join('');
}

// ── 导航操作 ──
function selectAll() {
  currentCategoryId = 'all';
  currentWeaponName = '';
  currentCategory.textContent = '全部分类';
  // 清除 active
  $$('.nav-item').forEach(el => el.classList.remove('active'));
  $$('.nav-sub-item').forEach(el => el.classList.remove('active'));
  document.querySelector('.nav-item-all')?.classList.add('active');
  loadLoadouts();
}

function toggleCategory(id, btn) {
  const subList = document.getElementById('subList' + id);
  if (!subList) return;

  const wasExpanded = subList.classList.contains('expanded');

  // 如果展开过，收起；没展开过则展开并选中分类
  if (wasExpanded) {
    subList.classList.remove('expanded');
    btn.querySelector('.nav-chevron')?.classList.remove('expanded');
    // 如果当前是在这个分类下且没选武器，不收起，而是选中「全部」分类
    return;
  }

  // 展开
  subList.classList.add('expanded');
  btn.querySelector('.nav-chevron')?.classList.add('expanded');

  // 选中该分类
  currentCategoryId = id;
  currentWeaponName = '';
  const cat = categories.find(c => c.id == id);
  currentCategory.textContent = cat ? cat.name : '全部分类';
  $$('.nav-item').forEach(el => el.classList.remove('active'));
  $$('.nav-sub-item').forEach(el => el.classList.remove('active'));
  btn.classList.add('active');
  loadLoadouts();
}

function selectWeapon(catId, weaponName, el) {
  currentCategoryId = catId;
  currentWeaponName = weaponName;
  const cat = categories.find(c => c.id == catId);
  currentCategory.textContent = cat ? `${cat.name} · ${weaponName}` : weaponName;

  // 更新 active 状态
  $$('.nav-item').forEach(el => el.classList.remove('active'));
  $$('.nav-sub-item').forEach(el => el.classList.remove('active'));
  el.classList.add('active');

  // 展开该分类
  const subList = document.getElementById('subList' + catId);
  if (subList) subList.classList.add('expanded');

  loadLoadouts();
}

// ── 加载改枪码 ──
async function loadLoadouts() {
  const params = new URLSearchParams();
  if (currentCategoryId !== 'all') params.set('category_id', currentCategoryId);
  if (currentWeaponName) params.set('weapon_name', currentWeaponName);
  if (searchInput.value.trim()) params.set('search', searchInput.value.trim());

  loadoutList.innerHTML = '<div class="loading-spinner"></div>';

  try {
    const result = await apiGet(`/api/loadouts?${params}`);
    renderLoadouts(result.data, result.total);
  } catch (err) {
    loadoutList.innerHTML = `<div class="empty-state">加载失败: ${err.message}</div>`;
  }
}

function renderLoadouts(data, total) {
  loadoutCount.textContent = `${total} 条改枪码`;

  if (data.length === 0) {
    const weaponHint = currentWeaponName ? `「${escHtml(currentWeaponName)}」` : '';
    loadoutList.innerHTML = `<div class="empty-state">${weaponHint}还没有改枪码<br>点击左侧「上传改枪码」来分享你的配置</div>`;
    return;
  }

  // 按排序
  if (currentSort === 'latest') {
    data.sort((a, b) => new Date(b.created_at) - new Date(a.created_at));
  } else {
    data.sort((a, b) => b.likes - a.likes);
  }

  loadoutList.innerHTML = data.map((item, i) => `
    <div class="loadout-card" style="animation-delay:${i * 0.04}s">
      <div class="card-top">
        <div class="card-weapon">
          <span class="card-weapon-name">${escHtml(item.weapon_name)}</span>
          <span class="card-category-badge">${escHtml(item.category_name)}</span>
        </div>
        <div class="card-meta">
          ${item.cost ? `<span class="card-cost">金额 ${Number(item.cost).toLocaleString()}</span>` : ''}
        </div>
      </div>
      <div class="card-code-wrap">
        <div class="card-code">${escHtml(item.code)}</div>
        <button class="copy-btn" data-code="${escHtml(item.code).replace(/'/g, '&#39;')}" onclick="copyCode(this)">复制</button>
      </div>
      ${item.description ? `<div class="card-desc">${escHtml(item.description)}</div>` : ''}
      <div class="card-bottom">
        <span class="card-author">${escHtml(item.author || '匿名')}</span>
        <div class="card-actions">
          <button class="report-btn" onclick="reportLoadout(${item.id}, this)">无法使用？</button>
          <button class="like-btn" onclick="likeLoadout(${item.id}, this)">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"/>
            </svg>
            ${item.likes}
          </button>
        </div>
      </div>
    </div>
  `).join('');
}

// ── 复制改枪码 ──
function copyCode(btn) {
  const code = btn.dataset.code;
  navigator.clipboard.writeText(code).then(() => {
    btn.textContent = '已复制 ✓';
    btn.classList.add('copied');
    setTimeout(() => {
      btn.textContent = '复制';
      btn.classList.remove('copied');
    }, 1500);
  }).catch(() => {
    showToast('复制失败，请手动复制');
  });
}

// ── 点赞 ──
async function likeLoadout(id, btn) {
  if (btn.classList.contains('liked')) return;
  try {
    const result = await apiPost(`/api/loadouts/${id}/like`);
    btn.classList.add('liked');
    btn.innerHTML = `<svg viewBox="0 0 24 24" fill="#EF4444" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"/></svg> ${result.likes}`;
    showToast('已点赞 👍');
  } catch (err) {
    showToast('点赞失败');
  }
}

// ── 报告无法使用 ──
async function reportLoadout(id, btn) {
  if (btn.classList.contains('reported')) return;
  try {
    await apiPost(`/api/loadouts/${id}/report`);
    btn.classList.add('reported');
    btn.textContent = '已报告管理员 ✓';
    showToast('已报告管理员，将尽快处理');
  } catch (err) {
    showToast('报告失败');
  }
}

// ── 搜索 ──
searchInput.addEventListener('input', () => {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(loadLoadouts, 300);
});

// ── 排序 ──
sortBtns.forEach(btn => {
  btn.addEventListener('click', () => {
    sortBtns.forEach(b => b.classList.remove('active'));
    btn.classList.add('active');
    currentSort = btn.dataset.sort;
    loadLoadouts();
  });
});

// ── 上传弹窗 ──
uploadBtn.addEventListener('click', () => {
  formError.classList.remove('show');
  formSuccess.classList.remove('show');
  // 重置分类到第一个
  if (categories.length > 0) {
    formCategory.value = categories[0].id;
    onCategoryChange();
  }
  formWeapon.value = '';
  formCode.value = '';
  formDesc.value = '';
  formCost.value = '';
  formAuthor.value = '';
  formSubmit.disabled = false;
  formSubmit.textContent = '提交改枪码';
  uploadModal.classList.add('open');
});

function closeModal() {
  uploadModal.classList.remove('open');
}

modalClose.addEventListener('click', closeModal);
modalCancel.addEventListener('click', closeModal);
uploadModal.addEventListener('click', (e) => {
  if (e.target === uploadModal) closeModal();
});

// ── 提交表单 ──
formSubmit.addEventListener('click', async () => {
  const category_id = formCategory.value;
  const weapon_name = formWeapon.value.trim();
  const code = formCode.value.trim();
  const cost = parseInt(formCost.value);
  const description = formDesc.value.trim();
  const author = formAuthor.value.trim() || '匿名';

  if (!weapon_name) {
    formError.textContent = '请选择武器';
    formError.classList.add('show');
    return;
  }
  if (!code) {
    formError.textContent = '请输入改枪码';
    formError.classList.add('show');
    return;
  }
  if (!cost || cost <= 0) {
    formError.textContent = '请输入改枪费用';
    formError.classList.add('show');
    return;
  }

  formError.classList.remove('show');
  formSubmit.disabled = true;
  formSubmit.textContent = '提交中...';

  try {
    await apiPost('/api/loadouts', { category_id, weapon_name, code, cost, description, author });
    formSuccess.classList.add('show');
    formSubmit.textContent = '提交成功 ✓';
    setTimeout(() => {
      closeModal();
      loadLoadouts();
    }, 1500);
  } catch (err) {
    formError.textContent = '提交失败: ' + err.message;
    formError.classList.add('show');
    formSubmit.disabled = false;
    formSubmit.textContent = '提交改枪码';
  }
});

// ── 初始化 ──
loadCategories().then(loadLoadouts);