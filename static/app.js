// ── Constants ──
const DAY_MS = 86400000;
const HOURSPERDAY = 7.5;
const COLORS = {
  review: '#4361ee', coding: '#2ec4b6', testing: '#ff9f1c',
  deploy: '#e63946', tracking: '#9b5de5', other: '#6c757d',
  idle: '#d0d5dd'
};
const LABELS = {
  review: '评审', coding: '编码', testing: '跟测',
  deploy: '上线', tracking: '跟踪', other: '其它', idle: '空闲'
};

// ── State ──
let currentUser = null;
let users = [];
let tasks = [];
let leaves = [];
let overtimes = [];
let holidays = [];
let recentIterations = [];
let dayWidth = 24;
let ganttView = 'task';

// Virtual scroll date range (epoch day indices)
let rangeStart = 0;  // epoch day of chartStartDate
let rangeEnd = 0;

// Date base: epoch = 1970-01-01
const EPOCH = new Date(1970, 0, 1);
function dayIndex(dateStr) {
  const d = parseDate(dateStr);
  return Math.floor((d - EPOCH) / DAY_MS);
}
function dayIndexFromDate(d) {
  return Math.floor((d.getTime() - EPOCH.getTime()) / DAY_MS);
}
function dateFromDayIndex(idx) {
  return new Date(EPOCH.getTime() + idx * DAY_MS);
}
function dateStrFromIdx(idx) {
  return fmtDate(dateFromDayIndex(idx));
}

// ── Drag state ──
let dragState = null;

// ── Inertia scroll ──
let scrollVelocity = 0;
let scrollRAF = null;
let isRendering = false;

// ── API Helper ──
async function api(method, path, body) {
  const opts = { method, headers: { 'Content-Type': 'application/json' }, credentials: 'same-origin' };
  if (body) opts.body = JSON.stringify(body);
  const res = await fetch('/api' + path, opts);
  const data = await res.json();
  if (!res.ok) throw new Error(data.error || '请求失败');
  return data;
}

// ── Auth ──
async function doLogin() {
  const u = document.getElementById('login-username').value.trim();
  const p = document.getElementById('login-password').value;
  if (!u || !p) { showLoginError('请输入用户名和密码'); return; }
  try {
    const data = await api('POST', '/login', { username: u, password: p });
    currentUser = data;
    showApp();
  } catch (e) { showLoginError(e.message); }
}

async function doRegister() {
  const u = document.getElementById('login-username').value.trim();
  const p = document.getElementById('login-password').value;
  if (!u || !p) { showLoginError('请输入用户名和密码'); return; }
  try {
    await api('POST', '/register', { username: u, name: u, password: p });
    const data = await api('POST', '/login', { username: u, password: p });
    currentUser = data;
    showApp();
  } catch (e) { showLoginError(e.message); }
}

async function doLogout() {
  await api('POST', '/logout');
  currentUser = null;
  document.getElementById('app-page').style.display = 'none';
  document.getElementById('login-page').style.display = 'flex';
}

function showLoginError(msg) {
  const el = document.getElementById('login-error');
  el.textContent = msg;
  setTimeout(() => el.textContent = '', 3000);
}

// ── Init ──
async function showApp() {
  document.getElementById('login-page').style.display = 'none';
  document.getElementById('app-page').style.display = 'flex';
  document.getElementById('user-display').innerHTML = `👤 <b>${currentUser.name}</b>`;
  await refreshAll();
}

async function refreshAll() {
  await Promise.all([loadUsers(), loadReport(), loadTasks(), loadLeave(), loadOvertime(), loadHolidays(), loadRecentIterations()]);
  initDateRange();
  renderGantt();
}

// ── Users ──
async function loadUsers() {
  try { users = await api('GET', '/users'); fillUserSelects(); } catch (e) { console.error(e); }
}

function fillUserSelects() {
  ['task-user', 'leave-user', 'overtime-user'].forEach(id => {
    document.getElementById(id).innerHTML = users.map(u => `<option value="${u.id}">${u.name}</option>`).join('');
  });
}

// ── Report ──
async function loadReport() {
  try {
    const r = await api('GET', '/report');
    document.getElementById('r-members').textContent = `${r.members_m}/${r.members_n}`;
    document.getElementById('r-tasks').textContent = `${r.tasks_m}/${r.tasks_n}`;
    document.getElementById('r-overtime').textContent = `${r.overtime_m.toFixed(1)}/${r.overtime_n.toFixed(1)}`;
    document.getElementById('r-leave').textContent = `${r.leave_m.toFixed(1)}/${r.leave_n.toFixed(1)}`;
  } catch (e) { console.error(e); }
}

// ── Tasks ──
async function loadTasks() {
  try { tasks = await api('GET', '/tasks'); } catch (e) { console.error(e); }
}

async function addTask() {
  const body = {
    user_id: parseInt(document.getElementById('task-user').value),
    iteration_name: document.getElementById('task-iteration').value.trim(),
    start_date: document.getElementById('task-start').value,
    end_date: document.getElementById('task-end').value,
    hours_review: parseFloat(document.getElementById('task-h-review').value) || 0,
    hours_coding: parseFloat(document.getElementById('task-h-coding').value) || 0,
    hours_testing: parseFloat(document.getElementById('task-h-testing').value) || 0,
    hours_deploy: parseFloat(document.getElementById('task-h-deploy').value) || 0,
    hours_tracking: parseFloat(document.getElementById('task-h-tracking').value) || 0,
    hours_other: parseFloat(document.getElementById('task-h-other').value) || 0,
  };
  if (!body.iteration_name || !body.start_date || !body.end_date) { alert('请填写完整'); return; }
  try {
    await api('POST', '/tasks', body);
    await refreshAll();
  } catch (e) { alert(e.message); }
}

// ── Leave ──
async function loadLeave() {
  try { leaves = await api('GET', '/leave'); } catch (e) { console.error(e); }
}

async function addLeave() {
  const body = {
    user_id: parseInt(document.getElementById('leave-user').value),
    start_date: document.getElementById('leave-date').value,
    hours: parseFloat(document.getElementById('leave-hours').value) || 0,
  };
  if (!body.start_date || !body.hours) { alert('请填写完整'); return; }
  try {
    await api('POST', '/leave', body);
    await refreshAll();
  } catch (e) { alert(e.message); }
}

// ── Overtime ──
async function loadOvertime() {
  try { overtimes = await api('GET', '/overtime'); } catch (e) { console.error(e); }
}

async function addOvertime() {
  const body = {
    user_id: parseInt(document.getElementById('overtime-user').value),
    start_date: document.getElementById('overtime-date').value,
    hours: parseFloat(document.getElementById('overtime-hours').value) || 0,
  };
  if (!body.start_date || !body.hours) { alert('请填写完整'); return; }
  try {
    await api('POST', '/overtime', body);
    await refreshAll();
  } catch (e) { alert(e.message); }
}

// ── Holidays ──
async function loadHolidays() {
  try { holidays = await api('GET', '/holidays'); } catch (e) { console.error(e); }
}

// ── Recent Iterations ──
async function loadRecentIterations() {
  try { recentIterations = await api('GET', '/iterations/recent'); } catch (e) { console.error(e); }
}

(function setupAutocomplete() {
  const input = () => document.getElementById('task-iteration');
  const list = () => document.getElementById('task-iteration-list');
  document.addEventListener('click', (e) => {
    if (!e.target.closest('.autocomplete-wrap')) list().classList.remove('show');
  });
  document.addEventListener('input', (e) => {
    if (e.target.id !== 'task-iteration') return;
    const val = e.target.value.toLowerCase();
    const filtered = val ? recentIterations.filter(n => n.toLowerCase().includes(val)) : recentIterations;
    const el = list();
    if (!filtered.length) { el.classList.remove('show'); return; }
    el.innerHTML = filtered.map(n => `<div class="autocomplete-item">${n}</div>`).join('');
    el.classList.add('show');
  });
  document.addEventListener('click', (e) => {
    if (e.target.classList.contains('autocomplete-item')) {
      input().value = e.target.textContent;
      list().classList.remove('show');
    }
  });
})();

// ── Forms Toggle ──
function toggleForms() {
  const content = document.getElementById('forms-content');
  const arrow = document.getElementById('forms-arrow');
  content.classList.toggle('collapsed');
  arrow.classList.toggle('collapsed');
  if (!content.classList.contains('collapsed')) {
    content.style.maxHeight = content.scrollHeight + 'px';
  }
}

// ── Gantt View Toggle ──
function setGanttView(view) {
  ganttView = view;
  document.getElementById('view-task').classList.toggle('active', view === 'task');
  document.getElementById('view-person').classList.toggle('active', view === 'person');
  renderGantt();
}

// ── Date Helpers ──
function parseDate(s) { const [y,m,d] = s.split('-').map(Number); return new Date(y, m-1, d); }
function fmtDate(d) { return d.getFullYear() + '-' + String(d.getMonth()+1).padStart(2,'0') + '-' + String(d.getDate()).padStart(2,'0'); }
function isWeekend(d) { const dow = d.getDay(); return dow === 0 || dow === 6; }
function holidayMap() {
  // Returns { "2025-01-01": { type: "rest", note: "元旦" }, ... }
  const m = {};
  holidays.forEach(h => { m[h.date] = { type: h.htype, note: h.note || '' }; });
  return m;
}

// ── Date Range: dynamic, centered around today ──
function initDateRange() {
  const todayIdx = dayIndexFromDate(new Date());
  rangeStart = todayIdx - 90;   // 3 months back
  rangeEnd = todayIdx + 180;    // 6 months forward
  // Extend to include all task dates
  tasks.forEach(t => {
    const si = dayIndex(t.start_date);
    const ei = dayIndex(t.end_date);
    if (si < rangeStart) rangeStart = si - 30;
    if (ei > rangeEnd) rangeEnd = ei + 30;
  });
}

function totalCols() { return rangeEnd - rangeStart + 1; }

// ── Infinite Scroll: extend range on edge approach ──
function checkInfiniteScroll() {
  const chart = document.getElementById('gantt-chart');
  if (!chart) return;
  const scrollLeft = chart.scrollLeft;
  const clientWidth = chart.clientWidth;
  const totalW = totalCols() * dayWidth;
  let changed = false;

  if (scrollLeft < dayWidth * 30) {
    // Near left edge: extend 90 days backward
    rangeStart -= 90;
    changed = true;
    // Adjust scroll to keep visual position
    chart.scrollLeft += 90 * dayWidth;
  }
  if (scrollLeft + clientWidth > totalW - dayWidth * 30) {
    // Near right edge: extend 90 days forward
    rangeEnd += 90;
    changed = true;
  }

  if (changed) renderGantt();
}

// ── Idle time calculation ──
function calcIdleDays(task) {
  const totalHours = task.hours_review + task.hours_coding + task.hours_testing +
                     task.hours_deploy + task.hours_tracking + task.hours_other;
  const workDays = totalHours / HOURSPERDAY;
  const startDate = parseDate(task.start_date);
  const endDate = parseDate(task.end_date);
  const calDays = (endDate - startDate) / DAY_MS + 1;
  return calDays - workDays;  // can be negative (overbooked)
}

// ── Build rows data ──
function buildRows() {
  // Each row: { label, subRows: [ [task, task, ...], [task, ...] ] }
  const rows = [];
  if (ganttView === 'task') {
    const groups = {};
    tasks.forEach(t => {
      if (!groups[t.iteration_name]) groups[t.iteration_name] = [];
      groups[t.iteration_name].push(t);
    });
    Object.keys(groups).sort().forEach(name => {
      rows.push({ label: name, subRows: [groups[name]] });
    });
  } else {
    // Person view: pack overlapping tasks into lanes (sub-rows)
    const groups = {};
    tasks.forEach(t => {
      if (!groups[t.user_id]) groups[t.user_id] = { name: t.user_name, tasks: [] };
      groups[t.user_id].tasks.push(t);
    });
    users.forEach(u => {
      const g = groups[u.id];
      if (!g) return;
      const sorted = g.tasks.slice().sort((a, b) => a.start_date.localeCompare(b.start_date));
      const lanes = []; // each lane: end_date of last task
      const laneTasks = [];
      sorted.forEach(t => {
        let placed = false;
        for (let i = 0; i < lanes.length; i++) {
          if (t.start_date > lanes[i]) {
            lanes[i] = t.end_date;
            laneTasks[i].push(t);
            placed = true;
            break;
          }
        }
        if (!placed) {
          lanes.push(t.end_date);
          laneTasks.push([t]);
        }
      });
      rows.push({ label: u.name, subRows: laneTasks });
    });
  }
  return rows;
}

// ── Gantt Render ──
function renderGantt() {
  if (isRendering) return;
  isRendering = true;

  const days = totalCols();
  const hmap = holidayMap();
  const rows = buildRows();
  const totalWidth = days * dayWidth;

  // Save scroll position
  const chart = document.getElementById('gantt-chart');
  const savedScrollLeft = chart ? chart.scrollLeft : 0;
  const savedScrollTop = chart ? chart.scrollTop : 0;

  // ── Sidebar ──
  document.getElementById('sidebar-header').textContent = ganttView === 'task' ? '迭代' : '人员';
  // Sidebar: one row per sub-row, label only on first sub-row of each group
  let sidebarHTML = '';
  rows.forEach(r => {
    r.subRows.forEach((_, si) => {
      const label = si === 0 ? r.label : '';
      sidebarHTML += `<div class="gantt-sidebar-row" title="${r.label}">${label}</div>`;
    });
  });
  document.getElementById('sidebar-rows').innerHTML = sidebarHTML;

  // ── Header: absolute positioning for perfect alignment ──
  const headerEl = document.getElementById('gantt-header');
  let hCells = '';

  // Month cells: detect month boundaries, emit span per month
  let curYM = -1, monthStartCol = 0;
  for (let i = 0; i <= days; i++) {
    const d = dateFromDayIndex(rangeStart + i);
    const ym = d.getFullYear() * 100 + d.getMonth(); // unique month id
    if (ym !== curYM || i === days) {
      if (curYM >= 0) {
        const spanCols = i - monthStartCol;
        const left = monthStartCol * dayWidth;
        const w = spanCols * dayWidth;
        const dt = dateFromDayIndex(rangeStart + monthStartCol);
        const label = dt.getFullYear() + '-' + String(dt.getMonth()+1).padStart(2,'0');
        hCells += `<div class="gantt-header-cell month" style="position:absolute;left:${left}px;width:${w}px;top:0;height:24px;">${label}</div>`;
      }
      curYM = ym;
      monthStartCol = i;
    }
  }

  // Day cells: each positioned absolutely
  for (let i = 0; i < days; i++) {
    const d = dateFromDayIndex(rangeStart + i);
    const dayNum = d.getDate();
    const left = i * dayWidth;
    const ds = fmtDate(d);
    let bg = '';
    const hm = hmap[ds];
    if (hm && hm.type === 'rest') {
      bg = 'background:#fff0b3;';
    } else if (isWeekend(d) && !(hm && hm.type === 'overtime')) {
      bg = 'background:#fce4e4;';
    } else if (hm && hm.type === 'overtime') {
      bg = 'background:#c8e6c9;';
    }
    const title = (hm && hm.note) ? ` title="${hm.note}"` : '';
    hCells += `<div class="gantt-header-cell" style="position:absolute;left:${left}px;width:${dayWidth}px;top:24px;height:24px;${bg}"${title}>${dayNum}</div>`;
  }

  headerEl.innerHTML = `<div style="position:relative;width:${totalWidth}px;height:48px;">${hCells}</div>`;
  headerEl.style.width = totalWidth + 'px';

  // ── Body ──
  const bodyEl = document.getElementById('gantt-body');

  // Pre-compute background strips (weekend/holiday/today) — shared across rows
  let bgStrips = '';
  for (let i = 0; i < days; i++) {
    const d = dateFromDayIndex(rangeStart + i);
    const ds = fmtDate(d);
    const left = i * dayWidth;
    const hm = hmap[ds];
    if (hm && hm.type === 'rest') {
      bgStrips += `<div class="gantt-holiday" style="left:${left}px;width:${dayWidth}px;" title="${hm.note || '法定假日'}"></div>`;
    } else if (isWeekend(d) && !(hm && hm.type === 'overtime')) {
      bgStrips += `<div class="gantt-weekend" style="left:${left}px;width:${dayWidth}px;"></div>`;
    }
  }
  // Today marker
  const todayIdx = dayIndexFromDate(new Date());
  if (todayIdx >= rangeStart && todayIdx <= rangeEnd) {
    const todayLeft = (todayIdx - rangeStart) * dayWidth + Math.floor(dayWidth / 2);
    bgStrips += `<div class="gantt-today" style="left:${todayLeft}px;"></div>`;
  }

  // Render rows with bars
  let bodyHTML = '';
  rows.forEach(row => {
    row.subRows.forEach(subTasks => {
      let rowHTML = `<div class="gantt-row" style="width:${totalWidth}px;">${bgStrips}`;

      subTasks.forEach(t => {
      const si = dayIndex(t.start_date);
      const ei = dayIndex(t.end_date);
      const barLeft = (si - rangeStart) * dayWidth;
      const barWidth = (ei - si + 1) * dayWidth;

      // Calculate idle days
      const idleDays = calcIdleDays(t);

      // Build segments
      const segments = [
        { key: 'review', hours: t.hours_review },
        { key: 'coding', hours: t.hours_coding },
        { key: 'testing', hours: t.hours_testing },
        { key: 'deploy', hours: t.hours_deploy },
        { key: 'tracking', hours: t.hours_tracking },
        { key: 'other', hours: t.hours_other },
      ];
      const totalH = segments.reduce((s, seg) => s + seg.hours, 0);

      // Segment colors
      let segHTML = '';
      let barExtraStyle = '';
      if (idleDays >= 0) {
        // Normal: work segments + idle segment
        const workDayFraction = totalH > 0 ? (totalH / HOURSPERDAY) / (ei - si + 1) : 0;
        const idleFraction = idleDays / (ei - si + 1);
        const activePct = Math.min(1, workDayFraction) * 100;
        segments.forEach(seg => {
          if (seg.hours <= 0) return;
          const pct = totalH > 0 ? (seg.hours / totalH) * activePct : 0;
          const lbl = dayWidth > 14 ? `${LABELS[seg.key]}${seg.hours}h` : '';
          segHTML += `<div class="segment" style="width:${pct}%;background:${COLORS[seg.key]}">${lbl}</div>`;
        });
        if (idleDays > 0) {
          const idlePct = idleFraction * 100;
          const idleLabel = dayWidth > 14 ? `空闲${idleDays.toFixed(2)}d` : '';
          segHTML += `<div class="segment" style="width:${idlePct}%;background:${COLORS.idle};color:#888;">${idleLabel}</div>`;
        }
      } else {
        // Overbooked: work segments fill 100%, red 2px border
        barExtraStyle = 'border-right:2px solid #e63946;';
        segments.forEach(seg => {
          if (seg.hours <= 0) return;
          const pct = totalH > 0 ? (seg.hours / totalH) * 100 : 0;
          const lbl = dayWidth > 14 ? `${LABELS[seg.key]}${seg.hours}h` : '';
          segHTML += `<div class="segment" style="width:${pct}%;background:${COLORS[seg.key]}">${lbl}</div>`;
        });
        // Show overbooked indicator
        const overLabel = dayWidth > 14 ? `<div style="position:absolute;right:4px;top:0;line-height:22px;font-size:9px;color:#e63946;font-weight:700;">${idleDays.toFixed(2)}d</div>` : '';
        segHTML += overLabel;
      }

      const taskData = JSON.stringify(t).replace(/'/g, '&#39;').replace(/"/g, '&quot;');
      rowHTML += `<div class="gantt-bar" data-task='${taskData}'
        style="left:${barLeft}px;width:${barWidth}px;${barExtraStyle}"
        onmousedown="startDrag(event, ${t.id})"
        ondblclick="openEditModal(${t.id})"
        onmouseenter="showTooltip(event, this)"
        onmouseleave="hideTooltip()">
        ${segHTML}
        <div class="resize-handle left" onmousedown="startResize(event, ${t.id}, 'left')"></div>
        <div class="resize-handle right" onmousedown="startResize(event, ${t.id}, 'right')"></div>
      </div>`;
    });

    rowHTML += '</div>';
    bodyHTML += rowHTML;
    }); // end subRows.forEach
  }); // end rows.forEach

  bodyEl.innerHTML = bodyHTML;
  bodyEl.style.width = totalWidth + 'px';

  // Restore scroll
  if (chart) {
    chart.scrollLeft = savedScrollLeft;
    chart.scrollTop = savedScrollTop;
    // Sync sidebar vertical scroll
    const sidebarRows = document.getElementById('sidebar-rows');
    chart.onscroll = () => {
      sidebarRows.scrollTop = chart.scrollTop;
    };
  }

  isRendering = false;
}

// ── Scroll & Zoom (infinite scroll + inertia + zoom) ──
(function setupScrollZoom() {
  const chart = () => document.getElementById('gantt-chart');

  document.addEventListener('wheel', (e) => {
    if (!chart() || !document.getElementById('app-page').style.display.includes('flex')) return;
    if (!e.target.closest('#gantt-chart') && !e.target.closest('.gantt-sidebar') && !e.target.closest('.gantt-header')) return;

    if (e.shiftKey) {
      // Horizontal scroll
      e.preventDefault();
      chart().scrollLeft += e.deltaY;
      scrollVelocity = e.deltaY * 0.5;
      startInertia();
      checkInfiniteScroll();
    } else if (e.ctrlKey || e.metaKey) {
      // Zoom
      e.preventDefault();
      const oldDW = dayWidth;
      dayWidth = Math.max(8, Math.min(80, dayWidth + (e.deltaY > 0 ? -2 : 2)));
      if (dayWidth !== oldDW) {
        // Preserve date under cursor
        const rect = chart().getBoundingClientRect();
        const cursorX = e.clientX - rect.left + chart().scrollLeft;
        const cursorCol = cursorX / oldDW;
        renderGantt();
        chart().scrollLeft = cursorCol * dayWidth - (e.clientX - rect.left);
        checkInfiniteScroll();
      }
    }
  }, { passive: false });

  // Also check infinite scroll on normal scroll
  document.addEventListener('scroll', (e) => {
    if (e.target.id === 'gantt-chart') checkInfiniteScroll();
  }, true);
})();

function startInertia() {
  if (scrollRAF) cancelAnimationFrame(scrollRAF);
  const chart = document.getElementById('gantt-chart');
  function tick() {
    if (Math.abs(scrollVelocity) < 0.5) { scrollVelocity = 0; return; }
    chart.scrollLeft += scrollVelocity;
    scrollVelocity *= 0.92;
    checkInfiniteScroll();
    scrollRAF = requestAnimationFrame(tick);
  }
  scrollRAF = requestAnimationFrame(tick);
}

// ── Drag ──
function startDrag(e, taskId) {
  if (e.target.classList.contains('resize-handle')) return;
  e.preventDefault();
  const t = tasks.find(x => x.id === taskId);
  if (!t) return;
  dragState = { taskId, mode: 'move', origStart: t.start_date, origEnd: t.end_date, startX: e.clientX };
  document.addEventListener('mousemove', onDragMove);
  document.addEventListener('mouseup', onDragEnd);
}

function startResize(e, taskId, side) {
  e.preventDefault();
  e.stopPropagation();
  const t = tasks.find(x => x.id === taskId);
  if (!t) return;
  dragState = { taskId, mode: side === 'left' ? 'resize-left' : 'resize-right', origStart: t.start_date, origEnd: t.end_date, startX: e.clientX };
  document.addEventListener('mousemove', onDragMove);
  document.addEventListener('mouseup', onDragEnd);
}

function onDragMove(e) {
  if (!dragState) return;
  const dx = e.clientX - dragState.startX;
  const daysDelta = Math.round(dx / dayWidth);
  const t = tasks.find(x => x.id === dragState.taskId);
  if (!t) return;

  if (dragState.mode === 'move') {
    const origStart = parseDate(dragState.origStart);
    const origEnd = parseDate(dragState.origEnd);
    t.start_date = fmtDate(new Date(origStart.getTime() + daysDelta * DAY_MS));
    t.end_date = fmtDate(new Date(origEnd.getTime() + daysDelta * DAY_MS));
  } else if (dragState.mode === 'resize-left') {
    const origStart = parseDate(dragState.origStart);
    const newStart = new Date(origStart.getTime() + daysDelta * DAY_MS);
    if (newStart <= parseDate(t.end_date)) t.start_date = fmtDate(newStart);
  } else if (dragState.mode === 'resize-right') {
    const origEnd = parseDate(dragState.origEnd);
    const newEnd = new Date(origEnd.getTime() + daysDelta * DAY_MS);
    if (newEnd >= parseDate(t.start_date)) t.end_date = fmtDate(newEnd);
  }
  renderGantt();
}

async function onDragEnd(e) {
  document.removeEventListener('mousemove', onDragMove);
  document.removeEventListener('mouseup', onDragEnd);
  if (!dragState) return;
  const t = tasks.find(x => x.id === dragState.taskId);
  if (t && (t.start_date !== dragState.origStart || t.end_date !== dragState.origEnd)) {
    try {
      await api('PUT', `/tasks/${t.id}`, {
        user_id: t.user_id, iteration_name: t.iteration_name,
        start_date: t.start_date, end_date: t.end_date,
        hours_review: t.hours_review, hours_coding: t.hours_coding,
        hours_testing: t.hours_testing, hours_deploy: t.hours_deploy,
        hours_tracking: t.hours_tracking, hours_other: t.hours_other,
      });
    } catch (err) {
      t.start_date = dragState.origStart;
      t.end_date = dragState.origEnd;
      renderGantt();
      alert(err.message);
    }
  }
  dragState = null;
}

// ── Tooltip ──
function showTooltip(e, el) {
  const task = JSON.parse(el.dataset.task);
  const total = task.hours_review + task.hours_coding + task.hours_testing + task.hours_deploy + task.hours_tracking + task.hours_other;
  const idleDays = calcIdleDays(task);
  const tt = document.getElementById('gantt-tooltip');

  let barHTML = '';
  [task.hours_review, task.hours_coding, task.hours_testing, task.hours_deploy, task.hours_tracking, task.hours_other].forEach((h, i) => {
    if (h > 0) {
      const keys = ['review','coding','testing','deploy','tracking','other'];
      barHTML += `<div style="width:${h/Math.max(total, HOURSPERDAY)*100}%;background:${COLORS[keys[i]]}"></div>`;
    }
  });
  if (idleDays > 0) {
    barHTML += `<div style="width:${idleDays*HOURSPERDAY/Math.max(total, HOURSPERDAY)*100}%;background:${COLORS.idle}"></div>`;
  }

  tt.innerHTML = `
    <div class="tt-title">${task.iteration_name} - ${task.user_name}</div>
    <div class="tt-row"><span class="tt-label">日期:</span>${task.start_date} ~ ${task.end_date}</div>
    <div class="tt-row"><span class="tt-label">评审:</span>${task.hours_review}h <span class="tt-label">编码:</span>${task.hours_coding}h <span class="tt-label">跟测:</span>${task.hours_testing}h</div>
    <div class="tt-row"><span class="tt-label">上线:</span>${task.hours_deploy}h <span class="tt-label">跟踪:</span>${task.hours_tracking}h <span class="tt-label">其它:</span>${task.hours_other}h</div>
    <div class="tt-row"><span class="tt-label">总计:</span>${total.toFixed(1)}h (${(total/HOURSPERDAY).toFixed(1)}人天) <span class="tt-label">空闲:</span>${idleDays.toFixed(2)}天</div>
    <div class="tt-bar">${barHTML}</div>
  `;
  tt.style.display = 'block';
  tt.style.left = Math.min(e.clientX + 12, window.innerWidth - 300) + 'px';
  tt.style.top = (e.clientY + 12) + 'px';
}

function hideTooltip() {
  document.getElementById('gantt-tooltip').style.display = 'none';
}

document.addEventListener('mousemove', (e) => {
  const tt = document.getElementById('gantt-tooltip');
  if (tt.style.display === 'block') {
    tt.style.left = Math.min(e.clientX + 12, window.innerWidth - 300) + 'px';
    tt.style.top = (e.clientY + 12) + 'px';
  }
});

// ── Edit Modal ──
let editingTask = null;

function openEditModal(taskId) {
  const t = tasks.find(x => x.id === taskId);
  if (!t) return;
  editingTask = t;
  const modal = document.getElementById('edit-modal');
  const body = document.getElementById('modal-body');
  document.getElementById('modal-title').textContent = '编辑任务';
  body.innerHTML = `
    <div class="form-row"><label>人员</label><select id="edit-user">${users.map(u => `<option value="${u.id}" ${u.id===t.user_id?'selected':''}>${u.name}</option>`).join('')}</select></div>
    <div class="form-row"><label>迭代</label><input type="text" id="edit-iteration" value="${t.iteration_name}"></div>
    <div class="form-row"><label>开始</label><input type="date" id="edit-start" value="${t.start_date}"></div>
    <div class="form-row"><label>结束</label><input type="date" id="edit-end" value="${t.end_date}"></div>
    <div class="hours-grid">
      <div class="form-row"><label>评审</label><input type="number" id="edit-h-review" min="0" step="0.1" value="${t.hours_review}"></div>
      <div class="form-row"><label>编码</label><input type="number" id="edit-h-coding" min="0" step="0.1" value="${t.hours_coding}"></div>
      <div class="form-row"><label>测跟</label><input type="number" id="edit-h-testing" min="0" step="0.1" value="${t.hours_testing}"></div>
      <div class="form-row"><label>上线</label><input type="number" id="edit-h-deploy" min="0" step="0.1" value="${t.hours_deploy}"></div>
      <div class="form-row"><label>跟踪</label><input type="number" id="edit-h-tracking" min="0" step="0.1" value="${t.hours_tracking}"></div>
      <div class="form-row"><label>其它</label><input type="number" id="edit-h-other" min="0" step="0.1" value="${t.hours_other}"></div>
    </div>
  `;
  modal.style.display = 'flex';
}

function closeModal() {
  document.getElementById('edit-modal').style.display = 'none';
  editingTask = null;
}

async function saveModal() {
  if (!editingTask) return;
  const body = {
    user_id: parseInt(document.getElementById('edit-user').value),
    iteration_name: document.getElementById('edit-iteration').value.trim(),
    start_date: document.getElementById('edit-start').value,
    end_date: document.getElementById('edit-end').value,
    hours_review: parseFloat(document.getElementById('edit-h-review').value) || 0,
    hours_coding: parseFloat(document.getElementById('edit-h-coding').value) || 0,
    hours_testing: parseFloat(document.getElementById('edit-h-testing').value) || 0,
    hours_deploy: parseFloat(document.getElementById('edit-h-deploy').value) || 0,
    hours_tracking: parseFloat(document.getElementById('edit-h-tracking').value) || 0,
    hours_other: parseFloat(document.getElementById('edit-h-other').value) || 0,
  };
  try {
    await api('PUT', `/tasks/${editingTask.id}`, body);
    closeModal();
    await refreshAll();
  } catch (e) { alert(e.message); }
}

async function deleteCurrent() {
  if (!editingTask) return;
  if (!confirm('确定删除?')) return;
  try {
    await api('DELETE', `/tasks/${editingTask.id}`);
    closeModal();
    await refreshAll();
  } catch (e) { alert(e.message); }
}

document.addEventListener('click', (e) => {
  if (e.target.id === 'edit-modal') closeModal();
});

document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') closeModal();
  if (e.key === 'Enter' && document.getElementById('login-page').style.display !== 'none') doLogin();
});

// ── Auto-login check ──
(async function() {
  try {
    currentUser = await api('GET', '/me');
    showApp();
  } catch (e) {
    document.getElementById('login-page').style.display = 'flex';
  }
})();
