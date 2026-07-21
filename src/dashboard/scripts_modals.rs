//! Modal handlers and API triggers for the StateSync web dashboard.

/// Modal and event listener dashboard script.
pub const JS_MODALS: &str = r#"function openServerModal(idx) {
  editIndex = idx; const isAdd = idx === -1;
  $('modalTitle').innerText = isAdd ? 'Add server' : 'Edit server';
  if (isAdd) {
    $('serverForm').reset();
    $('serverName').value = '';
    pickType('jellyfin');
    pickDirection('both');
  } else {
    const srv = currentConfig.servers[idx];
    pickType(srv.is_emby ? 'emby' : 'jellyfin');
    $('serverName').value = srv.name || '';
    $('serverUrl').value = srv.url;
    $('serverKey').value = srv.api_key;
    pickDirection(srv.sync_direction || 'both');
  }
  $('serverModal').style.display = 'flex';
  setTimeout(() => { try { $('serverUrl').focus(); } catch(_){} }, 50);
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
function openSettingsModal() { $('settingsModal').style.display = 'flex'; }
function closeModal(id) { $(id).style.display = 'none'; }
function testConnection() {
  let url = $('serverUrl').value.trim();
  const api_key = $('serverKey').value.trim();
  let type = $('serverType').value;
  if (!url || !api_key) return showToast('Enter a server address and API key first');
  showToast('Testing connection…');
  authedFetch('/api/test_connection', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ url, api_key, is_emby: type === 'emby' })
  })
    .then(async r => {
      const d = await r.json();
      if (d.status === 'ok') {
        if (typeof d.is_emby === 'boolean') {
          pickType(d.is_emby ? 'emby' : 'jellyfin');
        }
        if (d.url) $('serverUrl').value = d.url;
        showToast(d.message || 'Connected');
      } else {
        showToast(d.message || 'Connection failed');
      }
    })
    .catch((err) => showToast('Connection failed: ' + (err.message || 'unreachable')));
}
$('serverForm').addEventListener('submit', async (e) => {
  e.preventDefault();
  let url = $('serverUrl').value.trim();
  const api_key = $('serverKey').value.trim();
  if (!url || !api_key) return showToast('Enter a server address and API key first');
  // Name is optional — backend fills from hostname if empty
  let name = ($('serverName').value || '').trim();
  if (!name) name = nameFromUrl(url);
  const s = {
    name,
    url,
    api_key,
    is_emby: $('serverType').value === 'emby',
    sync_direction: $('serverDirection').value || 'both',
    allow_insecure_http: true
  };
  if (editIndex === -1) { currentConfig.servers.push(s); } else { currentConfig.servers[editIndex] = s; }
  closeModal('serverModal'); await saveConfig();
});
async function deleteServer(idx) {
  const srv = currentConfig.servers[idx];
  const label = srv.name || srv.url || 'this server';
  if (!confirm('Remove ' + label + '?')) return;
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
    const res = await authedFetch('/api/config', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(currentConfig)
    });
    const body = await res.json();
    showToast(body.message || (res.ok ? 'Saved' : 'Save failed'));
    setTimeout(loadDashboard, 800);
  } catch (err) { showToast('Save failed'); }
}
function showToast(msg) {
  const toast = $('toast');
  toast.innerText = msg;
  toast.style.display = 'block';
  setTimeout(() => { toast.style.display = 'none'; }, 4500);
}
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
  showToast('Refreshing users…');
  try {
    const res = await authedFetch('/api/users/refresh', { method: 'POST' });
    const data = await res.json();
    showToast('Refreshed ' + ((data.results || []).length) + ' server(s)');
  } catch (err) {
    showToast('Refresh failed: ' + err.message);
  }
  if (btn) btn.disabled = false;
  loadDashboard();
}
let _forceSyncTimer = null;
async function forceSync() {
  const btn = $('forceSyncBtn');
  if (btn && btn.disabled) return;
  if (btn) btn.disabled = true;
  showToast('Force sync started');
  try {
    await authedFetch('/api/sync/force', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ direction: 'both' })
    });
    pollForceSync();
  } catch (err) {
    showToast('Force sync failed: ' + err.message);
    if (btn) btn.disabled = false;
  }
}
async function cancelForceSync() {
  const btn = $('fsCancelBtn');
  if (btn) btn.disabled = true;
  showToast('Cancel requested');
  try {
    await authedFetch('/api/sync/force/cancel', { method: 'POST' });
  } catch (err) {
    showToast('Cancel failed: ' + err.message);
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
  div.textContent = s.state + ': processed ' + s.processed + ' · ok ' + s.succeeded + ' · skip ' + s.skipped + ' · fail ' + s.failed + ' (' + elapsed + 's)'
    + (s.last_error ? ' · ' + s.last_error : '');
}
function toggleLogs() {
  const logsDiv = $('syncLogs');
  const btn = $('toggleLogsBtn');
  if (!logsDiv || !btn) return;
  const collapsed = logsDiv.style.display === 'none';
  if (collapsed) {
    logsDiv.style.display = 'block';
    btn.textContent = 'Collapse';
    logsDiv.scrollTop = logsDiv.scrollHeight;
    localStorage.setItem('logs-expanded', 'true');
  } else {
    logsDiv.style.display = 'none';
    btn.textContent = 'Expand';
    localStorage.setItem('logs-expanded', 'false');
  }
}
function initLogsToggle() {
  const expanded = localStorage.getItem('logs-expanded') === 'true';
  const logsDiv = $('syncLogs');
  const btn = $('toggleLogsBtn');
  if (logsDiv && btn) {
    logsDiv.style.display = expanded ? 'block' : 'none';
    btn.textContent = expanded ? 'Collapse' : 'Expand';
  }
}
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    ['serverModal','settingsModal'].forEach(id => {
      const m = $(id); if (m && m.style.display === 'flex') m.style.display = 'none';
    });
  }
});
initLogsToggle();
document.addEventListener('DOMContentLoaded', () => {
  loadDashboard();
  setInterval(loadDashboard, 3000);
});
if (document.readyState !== 'loading') {
  loadDashboard();
  setInterval(loadDashboard, 3000);
}
"#;
