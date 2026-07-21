//! Modal handlers, event listeners, and API triggers for the StateSync web dashboard.

/// Modal and event listener dashboard script string slice (Part 3).
pub const JS_MODALS: &str = r#"function openServerModal(idx) {
  editIndex = idx; const isAdd = idx === -1;
  $('modalTitle').innerText = isAdd ? '[ ADD MEDIA SERVER ]' : '[ EDIT MEDIA SERVER ]';
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
  const type = $('serverType').value;
  const url = $('serverUrl').value.trim();
  const api_key = $('serverKey').value.trim();
  if (!url || !api_key) return showToast('LINK DATA INCOMPLETE (URL & API KEY REQUIRED)');
  showToast('PINGING LINK ADDRESS...');
  authedFetch('/api/test_connection', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ url, api_key, is_emby: type === 'emby' }) })
    .then(async r => {
      const d = await r.json();
      showToast((d.message || d.status || 'UNKNOWN').toUpperCase());
    })
    .catch((err) => showToast('LINK RESPONSE FAILED: ' + (err.message || 'UNREACHABLE').toUpperCase()));
}
$('serverForm').addEventListener('submit', async (e) => {
  e.preventDefault();
  const s = { name: $('serverName').value, url: $('serverUrl').value, api_key: $('serverKey').value, is_emby: $('serverType').value === 'emby', sync_direction: $('serverDirection').value, allow_insecure_http: $('serverUrl').value.startsWith('http://') };
  if (editIndex === -1) { currentConfig.servers.push(s); } else { currentConfig.servers[editIndex] = s; }
  closeModal('serverModal'); await saveConfig();
});
async function deleteServer(idx) {
  const srv = currentConfig.servers[idx];
  if (!confirm(`Are you sure you want to remove the server "${srv.name}"?`)) {
    return;
  }
  currentConfig.servers.splice(idx, 1);
  await saveConfig();
}
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
function formatAgo(ms) {
  if (ms < 0) return 'just now';
  const s = Math.floor(ms / 1000);
  if (s < 60) return s + 's ago';
  const m = Math.floor(s / 60);
  if (m < 60) return m + ' min ago';
  const h = Math.floor(m / 60);
  if (h < 24) return h + ' hr ago';
  const d = Math.floor(h / 24);
  return d + ' day' + (d === 1 ? '' : 's') + ' ago';
}
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
  if (btn && btn.disabled) return;
  if (btn) btn.disabled = true;
  showToast('FORCE SYNC STARTED');
  try {
    const res = await authedFetch('/api/sync/force', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ direction: 'both' }) });
    pollForceSync();
  } catch (err) {
    showToast('FORCE SYNC FAILED: ' + err.message);
    if (btn) btn.disabled = false;
  }
}
async function cancelForceSync() {
  const btn = $('fsCancelBtn');
  if (btn) btn.disabled = true;
  showToast('CANCEL REQUESTED (stops after current item)');
  try {
    await authedFetch('/api/sync/force/cancel', { method: 'POST' });
  } catch (err) {
    showToast('CANCEL FAILED: ' + err.message);
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
function toggleLogs() {
  const logsDiv = $('syncLogs');
  const btn = $('toggleLogsBtn');
  if (!logsDiv || !btn) return;
  const collapsed = logsDiv.style.display === 'none';
  if (collapsed) {
    logsDiv.style.display = 'block';
    btn.textContent = '[ COLLAPSE ]';
    logsDiv.scrollTop = logsDiv.scrollHeight;
    localStorage.setItem('logs-expanded', 'true');
  } else {
    logsDiv.style.display = 'none';
    btn.textContent = '[ EXPAND ]';
    localStorage.setItem('logs-expanded', 'false');
  }
}
function initLogsToggle() {
  const expanded = localStorage.getItem('logs-expanded') === 'true';
  const logsDiv = $('syncLogs');
  const btn = $('toggleLogsBtn');
  if (logsDiv && btn) {
    logsDiv.style.display = expanded ? 'block' : 'none';
    btn.textContent = expanded ? '[ COLLAPSE ]' : '[ EXPAND ]';
  }
}
document.addEventListener('keydown', (e) => { if (e.key === 'Escape') { ['serverModal','settingsModal','authModal'].forEach(id => { const m=$(id); if (m && m.style.display === 'flex') m.style.display='none'; }); } });
const savedTheme = localStorage.getItem('hud-theme') || 'cyberpunk'; setTheme(savedTheme); $('themeSelector').value = savedTheme;
initLogsToggle();
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
"#;
