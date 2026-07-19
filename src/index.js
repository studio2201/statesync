if ('serviceWorker' in navigator) { navigator.serviceWorker.register('/sw.js').catch(() => {}); }
const $ = id => document.getElementById(id);
let currentConfig = { servers: [], sync_threshold_seconds: 5 }; let editIndex = -1;
const AUTH_TOKEN_KEY = 'statesync-auth-token';
function esc(s) { if (s == null) return ''; return String(s).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'})[c]); }
function getAuthHeaders() {
  const t = localStorage.getItem(AUTH_TOKEN_KEY);
  return t ? { 'Authorization': 'Bearer ' + t } : {};
}
async function authedFetch(url, opts) {
  opts = opts || {};
  opts.headers = Object.assign({}, opts.headers || {}, getAuthHeaders());
  const r = await fetch(url, opts);
  if (r.status === 401) { showAuthModal(); throw new Error('unauthorized'); }
  return r;
}
function showAuthModal() {
  const m = $('authModal'); if (m) m.style.display = 'flex';
}
function hideAuthModal() {
  const m = $('authModal'); if (m) m.style.display = 'none';
}
function submitAuth() {
  const t = $('authToken').value.trim();
  if (!t) return;
  localStorage.setItem(AUTH_TOKEN_KEY, t);
  hideAuthModal();
  loadDashboard();
}
function setTheme(n) { document.body.className = n === 'cyberpunk' ? '' : `theme-${n}`; localStorage.setItem('hud-theme', n); }
async function loadDashboard() {
  try {
    const [configRes, statusRes] = await Promise.all([
      authedFetch('/api/config'),
      authedFetch('/api/status')
    ]);
    currentConfig = await configRes.json(); const status = await statusRes.json();
    $('syncThreshold').value = currentConfig.sync_threshold_seconds;
    $('cfgUserMappings').value = (currentConfig.user_mappings || []).map(group => group.join(', ')).join('\n');
    const listDiv = $('serverList');
    if (currentConfig.servers.length === 0) {
      listDiv.textContent = '';
      const empty = document.createElement('div'); empty.style.color = 'var(--accent)'; empty.textContent = 'NO CONFIGURED TRANSCEIVERS';
      listDiv.appendChild(empty);
    } else {
      listDiv.textContent = '';
      currentConfig.servers.forEach((srv, idx) => {
        const sStatus = status.servers.find(s => s.name === srv.name) || { users_count: 0, media_count: 0, websocket_status: 'Offline' };
        const row = document.createElement('div'); row.className = 'server-row';
        const dirBadge = srv.sync_direction === 'send' ? ' [SEND ONLY]' : (srv.sync_direction === 'receive' ? ' [RCV ONLY]' : '');
        const urlText = (status.servers.find(s => s.name === srv.name) || {}).url || srv.url;

        const left = document.createElement('div'); left.className = 'server-info';
        const statusSpanEl = document.createElement('span'); statusSpanEl.className = 'status-' + sStatus.websocket_status;
        statusSpanEl.textContent = '[ ' + sStatus.websocket_status.toUpperCase() + ' ]';
        const leftInner = document.createElement('div');
        const nameEl = document.createElement('span'); nameEl.style.cssText = 'font-weight:600;color:#fff'; nameEl.textContent = srv.name;
        const badgeEl = document.createElement('span'); badgeEl.className = 'badge'; badgeEl.textContent = (srv.is_emby ? 'EMBY' : 'JELLYFIN') + dirBadge;
        const urlEl = document.createElement('div'); urlEl.style.cssText = 'font-size:11px;color:var(--text);margin-top:2px'; urlEl.textContent = urlText;
        leftInner.appendChild(nameEl); leftInner.appendChild(document.createTextNode(' ')); leftInner.appendChild(badgeEl); leftInner.appendChild(urlEl);
        left.appendChild(statusSpanEl); left.appendChild(leftInner);

        const right = document.createElement('div'); right.className = 'server-info';
        const metaSpan = document.createElement('span'); metaSpan.style.fontSize = '12px';
        metaSpan.textContent = sStatus.users_count + ' USERS | ' + sStatus.media_count + ' CACHED';
        const editBtn = document.createElement('button'); editBtn.className = 'btn'; editBtn.textContent = '[ EDIT ]';
        editBtn.addEventListener('click', () => openServerModal(idx));
        const wipeBtn = document.createElement('button'); wipeBtn.className = 'btn btn-danger'; wipeBtn.textContent = '[ WIPE ]';
        wipeBtn.addEventListener('click', () => deleteServer(idx));
        right.appendChild(metaSpan); right.appendChild(editBtn); right.appendChild(wipeBtn);

        row.appendChild(left); row.appendChild(right);
        listDiv.appendChild(row);
      });
    }
    const activeDiv = $('activeSessions');
    if (status.active_sessions && status.active_sessions.length > 0) {
      activeDiv.textContent = '';
      status.active_sessions.forEach(sess => {
        const mins = Math.floor(sess.position / 60); const secs = Math.floor(sess.position % 60).toString().padStart(2, '0');
        const row = document.createElement('div'); row.className = 'server-row';
        if (sess.poster_url) { row.style.borderColor = 'var(--accent)'; row.style.padding = '6px 18px'; }
        const left = document.createElement('div'); left.className = 'server-info';
        if (sess.poster_url) {
          const img = document.createElement('img');
          img.src = sess.poster_url;
          img.alt = '';
          img.style.cssText = 'width:30px;height:45px;object-fit:cover;border:1px solid var(--accent);margin-right:12px;flex-shrink:0;';
          left.appendChild(img);
        }
        const meta = document.createElement('div');
        const itemEl = document.createElement('div'); itemEl.style.cssText = 'font-weight:600;color:#fff'; itemEl.textContent = sess.item;
        const userEl = document.createElement('div'); userEl.style.cssText = 'font-size:11px;color:var(--text)'; userEl.textContent = 'USER: ' + sess.user + ' | SOURCE: ' + sess.server;
        meta.appendChild(itemEl); meta.appendChild(userEl);
        left.appendChild(meta);
        const right = document.createElement('div'); right.style.cssText = 'display:flex;align-items:center;gap:10px';
        const badge = document.createElement('span'); badge.className = 'badge'; badge.style.cssText = 'border-color:var(--accent);color:var(--accent)';
        badge.textContent = mins + ':' + secs;
        right.appendChild(badge);
        if (sess.is_paused) {
          const p = document.createElement('span'); p.style.cssText = 'font-size:11px;color:var(--accent)'; p.textContent = '[ PAUSED ]';
          right.appendChild(p);
        }
        row.appendChild(left); row.appendChild(right);
        activeDiv.appendChild(row);
      });
    } else {
      activeDiv.textContent = '';
      const empty = document.createElement('div'); empty.style.color = 'var(--accent)'; empty.textContent = 'NO ACTIVE STREAMS DETECTED';
      activeDiv.appendChild(empty);
    }
    const usersDiv = $('syncedUsers');
    if (!status.servers || status.servers.length === 0) {
      usersDiv.textContent = '';
      const empty = document.createElement('div'); empty.style.color = 'var(--accent)'; empty.textContent = 'NO ACTIVE TRANSCEIVERS';
      usersDiv.appendChild(empty);
    } else {
      usersDiv.textContent = '';
      const header = document.createElement('div');
      header.style.cssText = 'display:flex;justify-content:space-between;color:var(--border);font-weight:600;font-size:12px;border-bottom:1px solid rgba(0,240,255,0.3);padding-bottom:6px;margin-bottom:12px';
      status.servers.forEach((srv, idx) => {
        const title = document.createElement('div'); title.textContent = srv.name.toUpperCase(); title.style.width = '120px';
        title.style.textAlign = idx === 0 ? 'left' : (idx === status.servers.length - 1 ? 'right' : 'center');
        header.appendChild(title);
        if (idx < status.servers.length - 1) {
          const placeholder = document.createElement('div'); placeholder.style.flex = '1'; header.appendChild(placeholder);
        }
      });
      usersDiv.appendChild(header);
      const serverCount = status.servers.length;
      const headerRow = document.createElement('div');
      headerRow.style.cssText = 'display:grid;grid-template-columns:repeat(' + serverCount + ', 1fr);gap:6px;margin-bottom:6px';
      status.servers.forEach(srv => {
        const h = document.createElement('div');
        h.style.cssText = 'text-align:center;color:var(--border);font-weight:600;font-size:12px;padding-bottom:6px;border-bottom:1px solid rgba(0,240,255,0.3);text-transform:uppercase';
        h.textContent = srv.name;
        headerRow.appendChild(h);
      });
      usersDiv.appendChild(headerRow);
      const users = (status.users || []).slice().sort((a, b) =>
        a.name.localeCompare(b.name, undefined, { sensitivity: 'base', numeric: true })
      );
      const grid = document.createElement('div');
      grid.style.cssText = 'display:grid;grid-template-columns:repeat(' + serverCount + ', 1fr);gap:6px';
      users.forEach(u => {
        const row = document.createElement('div');
        row.style.cssText = 'display:contents';
        for (let i = 0; i < serverCount; i++) {
          const cell = document.createElement('div');
          const filled = u.servers.includes(i);
          cell.className = 'user-cell' + (filled ? ' filled' : ' empty');
          cell.textContent = filled ? '@' + u.name : '·';
          cell.title = filled
            ? 'user: ' + u.name + (u.servers.length > 1 ? ' (mapped across ' + u.servers.length + ' servers)' : '')
            : (status.servers[i] ? 'server: ' + status.servers[i].name + ' (no user here)' : '');
          row.appendChild(cell);
        }
        grid.appendChild(row);
      });
      usersDiv.appendChild(grid);
      const mappedCount = users.filter(u => u.servers.length > 1).length;
      const singleCount = users.length - mappedCount;
      const legend = document.createElement('div');
      legend.style.cssText = 'margin-top:12px;font-size:11px;color:var(--text);opacity:0.7;display:flex;gap:16px;flex-wrap:wrap';
      legend.innerHTML = '<span>' + users.length + ' users total</span>' +
        '<span style="color:var(--border)">' + mappedCount + ' mapped across servers</span>' +
        '<span style="color:var(--accent)">' + singleCount + ' single-server (need a manual mapping)</span>';
      usersDiv.appendChild(legend);
    }
    const logsDiv = $('syncLogs');
    if (status.sync_logs && status.sync_logs.length > 0) {
      logsDiv.textContent = '';
      status.sync_logs.forEach(log => {
        const line = document.createElement('div'); line.className = 'log-line';
        const prefix = document.createTextNode('> [' + log.timestamp + '] ');
        line.appendChild(prefix);
        if (log.level === 'success' && log.source_name) {
          const sCol = log.source_is_emby ? 'var(--green)' : '#cc00ff';
          const tCol = log.target_is_emby ? 'var(--green)' : '#cc00ff';
          const sBadge = log.source_is_emby ? 'EMBY' : 'JELLYFIN';
          const tBadge = log.target_is_emby ? 'EMBY' : 'JELLYFIN';
          line.appendChild(document.createTextNode(log.message.toUpperCase() + ' FROM '));
          const fromSpan = document.createElement('span'); fromSpan.style.color = sCol;
          fromSpan.textContent = '[' + sBadge + ': ' + log.source_name.toUpperCase() + ']';
          line.appendChild(fromSpan);
          line.appendChild(document.createTextNode(' -> '));
          const toSpan = document.createElement('span'); toSpan.style.color = tCol;
          toSpan.textContent = '[' + tBadge + ': ' + log.target_name.toUpperCase() + ']';
          line.appendChild(toSpan);
        } else {
          const color = log.level === 'error' ? 'var(--red)' : (log.level === 'warn' ? 'var(--accent)' : 'var(--text)');
          const inner = document.createElement('span'); inner.style.color = color;
          inner.textContent = '[' + log.level.toUpperCase() + '] ' + log.message.toUpperCase();
          line.appendChild(inner);
        }
        logsDiv.appendChild(line);
      });
      logsDiv.scrollTop = logsDiv.scrollHeight;
    } else {
      logsDiv.textContent = '';
      const empty = document.createElement('div'); empty.style.color = 'var(--green)'; empty.textContent = '> LISTENING FOR METRIC EVENTS...';
      logsDiv.appendChild(empty);
    }
    const footer = $('versionFooter');
    if (footer && status.version) {
      footer.textContent = '';
      const link = document.createElement('a');
      link.href = 'https://github.com/UberMetroid/statesync/releases/tag/v' + status.version;
      link.target = '_blank';
      link.rel = 'noopener noreferrer';
      link.textContent = 'v' + status.version;
      link.style.cssText = 'color: var(--accent); text-decoration: none; border-bottom: 1px dotted var(--accent);';
      footer.appendChild(link);
      footer.appendChild(document.createTextNode(' | uptime ' + Math.floor(status.uptime_seconds / 60) + 'm'));
    }
  } catch (err) { console.error(err); }
}
function openServerModal(idx) {
  editIndex = idx; const isAdd = idx === -1;
  $('modalTitle').innerText = isAdd ? '[ ADD TRANSCEIVER MODULE ]' : '[ EDIT TRANSCEIVER MODULE ]';
  if (isAdd) {
    $('serverForm').reset();
    pickType('jellyfin');
    pickDirection('both');
  } else {
    const srv = currentConfig.servers[idx];
    pickType(srv.is_emby ? 'emby' : 'jellyfin');
    $('serverName').value = srv.name;
    $('serverUrl').value = srv.url;
    $('serverKey').value = srv.api_key;
    pickDirection(srv.sync_direction || 'both');
  }
  $('serverModal').style.display = 'flex';
}
function pickType(t) {
  $('serverType').value = t;
  $('btnJellyfin').classList.toggle('active', t === 'jellyfin');
  $('btnEmby').classList.toggle('active', t === 'emby');
}
function pickDirection(d) {
  $('serverDirection').value = d;
  document.querySelectorAll('#serverForm .btn-radio[data-dir]').forEach(b => {
    b.classList.toggle('active', b.getAttribute('data-dir') === d);
  });
}
async function autoFetchServerName() {
  const btn = $('autoNameBtn');
  if (btn) { btn.disabled = true; btn.textContent = '...'; }
  const url = $('serverUrl').value.trim();
  const api_key = $('serverKey').value;
  const is_emby = $('serverType').value === 'emby';
  if (!url) {
    showToast('ENTER SERVER ADDRESS FIRST');
    if (btn) { btn.disabled = false; btn.textContent = '↻ AUTO'; }
    return;
  }
  try {
    const params = new URLSearchParams({ url, is_emby: is_emby ? 'true' : 'false' });
    if (api_key) params.set('api_key', api_key);
    const res = await authedFetch('/api/server-info?' + params.toString());
    if (!res.ok) {
      const data = await res.json().catch(() => ({}));
      showToast('AUTO FILL FAILED: ' + (data.error || res.status));
      if (btn) { btn.disabled = false; btn.textContent = '↻ AUTO'; }
      return;
    }
    const data = await res.json();
    if (data.name) {
      $('serverName').value = data.name;
      showToast('AUTO FILLED: ' + data.name);
    } else {
      showToast('SERVER DID NOT RETURN A NAME');
    }
  } catch (err) {
    showToast('AUTO FILL FAILED: ' + err.message);
  }
  if (btn) { btn.disabled = false; btn.textContent = '↻ AUTO'; }
}
function openSettingsModal() { $('settingsModal').style.display = 'flex'; }
function closeModal(id) { $(id).style.display = 'none'; }
function testConnection() {
  const type = $('serverType').value, url = $('serverUrl').value, api_key = $('serverKey').value;
  if (!url || !api_key) return showToast('LINK DATA INCOMPLETE');
  showToast('PINGING LINK ADDRESS...');
  authedFetch('/api/test_connection', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ url, api_key, is_emby: type === 'emby' }) })
    .then(async r => showToast((await r.json()).message.toUpperCase())).catch(() => showToast('LINK RESPONSE FAILED'));
}
$('serverForm').addEventListener('submit', async (e) => {
  e.preventDefault();
  const s = { name: $('serverName').value, url: $('serverUrl').value, api_key: $('serverKey').value, is_emby: $('serverType').value === 'emby', sync_direction: $('serverDirection').value, allow_insecure_http: $('serverUrl').value.startsWith('http://') };
  if (editIndex === -1) { currentConfig.servers.push(s); } else { currentConfig.servers[editIndex] = s; }
  closeModal('serverModal'); await saveConfig();
});
async function deleteServer(idx) { currentConfig.servers.splice(idx, 1); await saveConfig(); }
async function saveSettings() {
  currentConfig.sync_threshold_seconds = parseInt($('syncThreshold').value);
  const mappingsLines = $('cfgUserMappings').value.split('\n');
  const user_mappings = [];
  mappingsLines.forEach(line => {
    const parts = line.split(',').map(p => p.trim()).filter(p => p.length > 0);
    if (parts.length > 0) user_mappings.push(parts);
  });
  currentConfig.user_mappings = user_mappings;
  closeModal('settingsModal');
  await saveConfig();
}
async function saveConfig() {
  try {
    const res = await authedFetch('/api/config', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(currentConfig) });
    showToast((await res.json()).message.toUpperCase()); setTimeout(loadDashboard, 1200);
  } catch (err) { showToast('WRITE CONFIG FAILED'); }
}
function showToast(msg) { const toast = $('toast'); toast.innerText = `> ${msg}`; toast.style.display = 'block'; setTimeout(() => { toast.style.display = 'none'; }, 4000); }
async function refreshUsers() {
  const btn = $('refreshUsersBtn');
  if (btn) btn.disabled = true;
  showToast('REFRESHING USER LISTS...');
  try {
    const res = await authedFetch('/api/users/refresh', { method: 'POST' });
    const data = await res.json();
    showToast(`REFRESHED: ${(data.results || []).length} SERVERS`);
  } catch (err) {
    showToast('REFRESH FAILED: ' + err.message);
  }
  if (btn) btn.disabled = false;
  loadDashboard();
}
let _forceSyncTimer = null;
async function forceSync() {
  const btn = $('forceSyncBtn');
  const status = $('forceSyncStatus');
  if (btn) btn.disabled = true;
  if (status) status.textContent = 'STARTING...';
  showToast('FORCE SYNC STARTED');
  try {
    const res = await authedFetch('/api/sync/force', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ direction: 'both' }) });
    pollForceSync();
  } catch (err) {
    showToast('FORCE SYNC FAILED: ' + err.message);
    if (btn) btn.disabled = false;
  }
}
async function pollForceSync() {
  if (_forceSyncTimer) clearTimeout(_forceSyncTimer);
  try {
    const res = await authedFetch('/api/sync/force/status');
    const s = await res.json();
    renderForceSync(s);
    if (s.state === 'running') {
      _forceSyncTimer = setTimeout(pollForceSync, 1000);
    } else {
      _forceSyncTimer = null;
      const btn = $('forceSyncBtn');
      if (btn) btn.disabled = false;
    }
  } catch (err) {
    console.error(err);
  }
}
function renderForceSync(s) {
  const div = $('forceSyncStatus');
  if (!div) return;
  if (s.state === 'idle' && !s.started_at) {
    div.textContent = 'Force sync has not been run yet.';
    return;
  }
  const elapsed = s.finished_at && s.started_at
    ? Math.max(1, Math.round((new Date(s.finished_at) - new Date(s.started_at)) / 1000))
    : (s.started_at ? Math.round((Date.now() - new Date(s.started_at).getTime()) / 1000) : 0);
  const base = `[${s.state.toUpperCase()}] processed=${s.processed} ok=${s.succeeded} skip=${s.skipped} fail=${s.failed} (${elapsed}s)`;
  div.textContent = base + (s.last_error ? ` | last: ${s.last_error}` : '');
}
document.addEventListener('keydown', (e) => { if (e.key === 'Escape') { ['serverModal','settingsModal','authModal'].forEach(id => { const m=$(id); if (m && m.style.display === 'flex') m.style.display='none'; }); } });
const savedTheme = localStorage.getItem('hud-theme') || 'cyberpunk'; setTheme(savedTheme); $('themeSelector').value = savedTheme;
document.addEventListener('DOMContentLoaded', () => {
  const b = $('authSubmitBtn'); if (b) b.addEventListener('click', submitAuth);
  const t = $('authToken'); if (t) t.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitAuth(); });
  loadDashboard();
  setInterval(loadDashboard, 3000);
});
if (document.readyState !== 'loading') {
  const b = $('authSubmitBtn'); if (b) b.addEventListener('click', submitAuth);
  const t = $('authToken'); if (t) t.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitAuth(); });
  loadDashboard();
  setInterval(loadDashboard, 3000);
}
