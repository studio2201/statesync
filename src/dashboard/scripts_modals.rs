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
  // Normalize whenever the user leaves the field or pastes a browser URL.
  const urlInput = $('serverUrl');
  if (urlInput && !urlInput._ssBound) {
    urlInput._ssBound = true;
    urlInput.addEventListener('blur', () => {
      const n = normalizeServerUrl(urlInput.value);
      if (n) urlInput.value = n;
    });
    urlInput.addEventListener('paste', () => {
      setTimeout(() => {
        const n = normalizeServerUrl(urlInput.value);
        if (n) urlInput.value = n;
      }, 0);
    });
  }
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

function copyActivityLog() {
  const logs = window._lastSyncLogs || [];
  if (!logs.length) {
    showToast('Nothing to copy yet');
    return;
  }
  const text = logs.map(log => {
    let line = '[' + log.timestamp + '] ' + log.level + ': ' + log.message;
    if (log.source_name || log.target_name) {
      line += ' | ' + (log.source_name || '?') + ' → ' + (log.target_name || '?');
    }
    if (log.detail) line += '\n  ' + log.detail;
    return line;
  }).join('\n');
  const done = () => showToast('Activity log copied (' + logs.length + ' lines)');
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(text).then(done).catch(() => fallbackCopy(text, done));
  } else {
    fallbackCopy(text, done);
  }
}
function fallbackCopy(text, done) {
  const ta = document.createElement('textarea');
  ta.value = text;
  ta.style.cssText = 'position:fixed;left:-9999px;top:0';
  document.body.appendChild(ta);
  ta.focus(); ta.select();
  try { document.execCommand('copy'); done(); }
  catch (e) { showToast('Copy failed — select text in the log manually'); }
  document.body.removeChild(ta);
}

function openMapUsersModal() {
  const st = window._lastStatus;
  const by = (st && st.users_by_server) || [];
  if (by.length < 2) {
    showToast('Add at least two servers and refresh users first');
    return;
  }
  // Use first two servers for the simple picker (most common: Emby + Jellyfin)
  window._mapServerA = by[0];
  window._mapServerB = by[1];
  const labA = $('mapServerALabel');
  const labB = $('mapServerBLabel');
  if (labA) labA.textContent = 'User on ' + (by[0].name || 'server A');
  if (labB) labB.textContent = 'User on ' + (by[1].name || 'server B');
  fillUserSelect($('mapUserA'), by[0].users || []);
  fillUserSelect($('mapUserB'), by[1].users || []);
  renderMapLinksList();
  $('mapUsersModal').style.display = 'flex';
}
function fillUserSelect(sel, names) {
  if (!sel) return;
  sel.textContent = '';
  const opt0 = document.createElement('option');
  opt0.value = '';
  opt0.textContent = names.length ? '— select user —' : '— no users loaded —';
  sel.appendChild(opt0);
  names.slice().sort((a,b) => a.localeCompare(b)).forEach(n => {
    const o = document.createElement('option');
    o.value = n;
    o.textContent = n;
    sel.appendChild(o);
  });
}
function renderMapLinksList() {
  const list = $('mapLinksList');
  if (!list) return;
  list.textContent = '';
  const maps = currentConfig.user_mappings || [];
  if (!maps.length) {
    const empty = document.createElement('div');
    empty.className = 'empty';
    empty.textContent = 'No manual links yet. Exact same usernames still match automatically.';
    list.appendChild(empty);
    return;
  }
  maps.forEach((group, idx) => {
    const row = document.createElement('div');
    row.className = 'map-link-row';
    const label = document.createElement('span');
    label.textContent = group.join('  ↔  ');
    const rm = document.createElement('button');
    rm.className = 'btn btn-danger';
    rm.textContent = 'Remove';
    rm.onclick = () => removeUserMapping(idx);
    row.appendChild(label);
    row.appendChild(rm);
    list.appendChild(row);
  });
}
async function addLinkedUserMapping() {
  const a = ($('mapUserA') && $('mapUserA').value || '').trim();
  const b = ($('mapUserB') && $('mapUserB').value || '').trim();
  if (!a || !b) {
    showToast('Select a user on each server');
    return;
  }
  if (a.toLowerCase() === b.toLowerCase()) {
    showToast('Those names already match — no link needed');
    return;
  }
  if (!currentConfig.user_mappings) currentConfig.user_mappings = [];
  // Merge into existing group if either name already appears
  let merged = false;
  for (let i = 0; i < currentConfig.user_mappings.length; i++) {
    const g = currentConfig.user_mappings[i];
    const lower = g.map(x => x.toLowerCase());
    if (lower.includes(a.toLowerCase()) || lower.includes(b.toLowerCase())) {
      if (!lower.includes(a.toLowerCase())) g.push(a);
      if (!lower.includes(b.toLowerCase())) g.push(b);
      merged = true;
      break;
    }
  }
  if (!merged) currentConfig.user_mappings.push([a, b]);
  // Keep settings textarea in sync
  if ($('cfgUserMappings')) {
    $('cfgUserMappings').value = currentConfig.user_mappings.map(g => g.join(', ')).join('\n');
  }
  showToast('Linked ' + a + ' ↔ ' + b);
  await saveConfig();
  renderMapLinksList();
  setTimeout(loadDashboard, 600);
}
async function removeUserMapping(idx) {
  if (!currentConfig.user_mappings) return;
  currentConfig.user_mappings.splice(idx, 1);
  if ($('cfgUserMappings')) {
    $('cfgUserMappings').value = currentConfig.user_mappings.map(g => g.join(', ')).join('\n');
  }
  await saveConfig();
  renderMapLinksList();
  setTimeout(loadDashboard, 600);
}
function testConnection() {
  let url = normalizeServerUrl($('serverUrl').value);
  $('serverUrl').value = url;
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
  let url = normalizeServerUrl($('serverUrl').value);
  $('serverUrl').value = url;
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
window._forceSyncOptimistic = false;
/** Normalize API state (Running / running) for comparisons. */
function forceStateKey(state) {
  return String(state || '').toLowerCase();
}
function applyForceSyncLiveUi(fs) {
  const live = $('forceSyncLive');
  if (!live || !fs) return;
  const totalPairs = fs.total_pairs || 0;
  const processed = fs.processed || 0;
  const denom = totalPairs > 0 ? totalPairs : Math.max(processed, 1);
  const pct = totalPairs > 0 ? Math.min(100, Math.floor(processed / totalPairs * 100)) : 0;
  const startedMs = fs.started_at ? new Date(fs.started_at).getTime() : Date.now();
  const elapsed = Math.max(0, Math.round((Date.now() - startedMs) / 1000));
  const rate = elapsed > 0 ? (processed / elapsed).toFixed(1) : '0';
  live.style.display = 'flex';
  const title = $('fsStoryTitle');
  if (title) title.textContent = 'Force sync in progress';
  const bar = $('fsProgressBar');
  if (bar) { bar.value = pct; bar.max = 100; }
  const txt = $('fsProgressText');
  if (txt) {
    txt.textContent = totalPairs > 0
      ? (pct + '% · ' + processed + ' / ' + totalPairs + ' · ' + rate + '/s')
      : (processed + ' items · starting…');
  }
  const cu = $('fsCurrentUser');
  if (cu) {
    cu.textContent = fs.current_user
      ? ('Working on user: ' + fs.current_user)
      : (processed === 0 ? 'Building user pairs and loading played history…' : 'Matching titles across servers…');
  }
  const detail = $('fsStoryDetail');
  if (detail) {
    const parts = [];
    parts.push('Live play sync is paused while this runs.');
    parts.push('Pushed ' + (fs.succeeded || 0) + ', already matched ' + (fs.skipped || 0) + ', failed ' + (fs.failed || 0) + '.');
    if (fs.last_error) parts.push('Last error: ' + fs.last_error);
    if (elapsed > 0) parts.push('Elapsed ' + elapsed + 's.');
    detail.textContent = parts.join(' ');
  }
  void denom;
}
async function forceSync() {
  const btn = $('forceSyncBtn');
  if (btn && btn.disabled) return;
  if (btn) btn.disabled = true;
  window._forceSyncOptimistic = true;
  // Show the story immediately — do not wait for the first poll.
  applyForceSyncLiveUi({
    state: 'Running',
    started_at: new Date().toISOString(),
    finished_at: null,
    total_pairs: 0,
    processed: 0,
    succeeded: 0,
    skipped: 0,
    failed: 0,
    current_user: null,
    last_error: null
  });
  const statusHint = $('forceSyncStatus');
  if (statusHint) statusHint.textContent = 'Force sync started — scanning played history on every linked user…';
  showToast('Force sync started — backfilling watched history');
  try {
    const res = await authedFetch('/api/sync/force', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ direction: 'both' })
    });
    if (!res.ok && res.status !== 202) {
      const body = await res.json().catch(() => ({}));
      throw new Error(body.message || ('HTTP ' + res.status));
    }
    pollForceSync();
    loadDashboard();
  } catch (err) {
    window._forceSyncOptimistic = false;
    const live = $('forceSyncLive');
    if (live) live.style.display = 'none';
    showToast('Force sync failed: ' + err.message);
    if (btn) btn.disabled = false;
  }
}
async function cancelForceSync() {
  const btn = $('fsCancelBtn');
  if (btn) btn.disabled = true;
  showToast('Cancel requested — finishing current item…');
  const detail = $('fsStoryDetail');
  if (detail) detail.textContent = 'Cancel requested. Waiting for the current item to finish, then stopping.';
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
    const st = forceStateKey(s.state);
    if (st === 'running' || (s.started_at && !s.finished_at && st !== 'completed' && st !== 'failed' && st !== 'idle')) {
      window._forceSyncOptimistic = false;
      applyForceSyncLiveUi(s);
      _forceSyncTimer = setTimeout(pollForceSync, 1000);
    } else {
      window._forceSyncOptimistic = false;
      _forceSyncTimer = null;
      const live = $('forceSyncLive');
      // Keep banner visible briefly with final numbers, then dashboard refresh owns it
      if (st === 'completed' || st === 'failed') {
        applyForceSyncLiveUi(Object.assign({}, s, { finished_at: s.finished_at || new Date().toISOString() }));
        const title = $('fsStoryTitle');
        if (title) title.textContent = st === 'completed' ? 'Force sync finished' : 'Force sync finished with errors';
        setTimeout(() => { if ($('forceSyncLive')) $('forceSyncLive').style.display = 'none'; }, 4000);
      } else if (live) {
        live.style.display = 'none';
      }
      const btn = $('forceSyncBtn');
      if (btn) btn.disabled = false;
      const cancelBtn = $('fsCancelBtn');
      if (cancelBtn) cancelBtn.disabled = false;
      loadDashboard();
    }
  } catch (err) {
    console.error(err);
    _forceSyncTimer = setTimeout(pollForceSync, 2000);
  }
}
function renderForceSync(s) {
  const div = $('forceSyncStatus');
  if (!div) return;
  const st = forceStateKey(s.state);
  if (st === 'idle' && !s.started_at) {
    div.textContent = 'Force sync has not been run yet.';
    return;
  }
  const elapsed = s.finished_at && s.started_at
    ? Math.max(1, Math.round((new Date(s.finished_at) - new Date(s.started_at)) / 1000))
    : (s.started_at ? Math.round((Date.now() - new Date(s.started_at).getTime()) / 1000) : 0);
  const verb = st === 'running' ? 'Running' : (st === 'completed' ? 'Done' : (st === 'failed' ? 'Failed' : s.state));
  div.textContent = verb + ': looked at ' + s.processed + ' · pushed ' + s.succeeded + ' · skipped ' + s.skipped + ' · failed ' + s.failed + ' (' + elapsed + 's)'
    + (s.last_error ? ' · ' + s.last_error : '');
}
function toggleHowSync() {
  const body = $('howSyncBody');
  const btn = $('toggleHowSyncBtn');
  if (!body || !btn) return;
  const hidden = body.style.display === 'none';
  body.style.display = hidden ? 'block' : 'none';
  btn.textContent = hidden ? 'Collapse' : 'Expand';
  localStorage.setItem('how-sync-expanded', hidden ? 'true' : 'false');
}
function initHowSyncToggle() {
  const expanded = localStorage.getItem('how-sync-expanded');
  // Default expanded so the overview is visible on first visit
  const show = expanded !== 'false';
  const body = $('howSyncBody');
  const btn = $('toggleHowSyncBtn');
  if (body && btn) {
    body.style.display = show ? 'block' : 'none';
    btn.textContent = show ? 'Collapse' : 'Expand';
  }
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
    ['serverModal','settingsModal','mapUsersModal'].forEach(id => {
      const m = $(id); if (m && m.style.display === 'flex') m.style.display = 'none';
    });
  }
});
initLogsToggle();
initHowSyncToggle();
document.addEventListener('DOMContentLoaded', () => {
  loadDashboard();
  setInterval(loadDashboard, 3000);
});
if (document.readyState !== 'loading') {
  loadDashboard();
  setInterval(loadDashboard, 3000);
}
"#;
