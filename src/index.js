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
    const statusSpan = $('systemStatusText');
    if (currentConfig.servers.length === 0) {
      statusSpan.innerText = '[ SYNC ENGINE: IDLE ]'; statusSpan.style.color = 'var(--border)';
    } else {
      const statuses = status.servers.map(s => s.websocket_status);
      if (statuses.length === 0 || statuses.every(s => s === 'Offline')) {
        statusSpan.innerText = '[ SYNC ENGINE: OFFLINE ]'; statusSpan.style.color = 'var(--red)';
      } else if (statuses.some(s => s === 'Offline' || s === 'Reconnecting')) {
        statusSpan.innerText = '[ SYNC ENGINE: DEGRADED ]'; statusSpan.style.color = 'var(--accent)';
      } else {
        statusSpan.innerText = '[ SYNC ENGINE: ONLINE ]'; statusSpan.style.color = 'var(--green)';
      }
    }
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
      const mapped = (status.mapped_users || []).slice();
      mapped.sort((a, b) => {
        const nameA = a.find(u => u !== null) || '';
        const nameB = b.find(u => u !== null) || '';
        return nameA.localeCompare(nameB, undefined, { sensitivity: 'base', numeric: true });
      });
      mapped.forEach(group => {
        const row = document.createElement('div'); row.style.cssText = 'display:flex;align-items:center;padding:6px 0';
        group.forEach((username, idx) => {
          const align = idx === 0 ? 'left' : (idx === group.length - 1 ? 'right' : 'center');
          const cell = document.createElement('div');
          cell.style.cssText = username ? 'color:#fff;font-size:12px;width:120px;text-align:' + align : 'color:var(--text);opacity:0.3;font-size:12px;width:120px;text-align:' + align;
          cell.textContent = username || '[ UNMAPPED ]';
          row.appendChild(cell);
          if (idx < group.length - 1) {
            const isGreen = username !== null && group.slice(idx + 1).some(u => u !== null);
            const sep = document.createElement('div');
            sep.style.cssText = 'flex:1;border-bottom:1px dotted ' + (isGreen ? 'var(--green)' : 'var(--accent)') + ';margin:0 15px;opacity:' + (isGreen ? '0.7' : '0.2');
            row.appendChild(sep);
          }
        });
        usersDiv.appendChild(row);
      });
      const unmapped = [];
      status.servers.forEach((srv, srvIdx) => {
        if (srv.users) srv.users.forEach(username => {
          if (!mapped.some(group => group.some(u => u === username))) {
            unmapped.push({ username, srvIdx });
          }
        });
      });
      unmapped.sort((a, b) => a.username.localeCompare(b.username, undefined, { sensitivity: 'base', numeric: true }));
      unmapped.forEach(({ username, srvIdx }) => {
        const row = document.createElement('div'); row.style.cssText = 'display:flex;align-items:center;padding:6px 0';
        for (let i = 0; i < status.servers.length; i++) {
          const align = i === 0 ? 'left' : (i === status.servers.length - 1 ? 'right' : 'center');
          const cell = document.createElement('div');
          cell.style.cssText = 'font-size:12px;width:120px;text-align:' + align + ';color:' + (i === srvIdx ? 'var(--accent)' : 'var(--text);opacity:0.3');
          cell.textContent = i === srvIdx ? username : '[ UNMAPPED ]';
          row.appendChild(cell);
          if (i < status.servers.length - 1) {
            const sep = document.createElement('div'); sep.style.cssText = 'flex:1;border-bottom:1px dotted var(--accent);margin:0 15px;opacity:0.3';
            row.appendChild(sep);
          }
        }
        usersDiv.appendChild(row);
      });
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
    if (footer && status.version) footer.textContent = 'v' + status.version + ' | uptime ' + Math.floor(status.uptime_seconds / 60) + 'm';
  } catch (err) { console.error(err); }
}
function openServerModal(idx) {
  editIndex = idx; const isAdd = idx === -1;
  $('modalTitle').innerText = isAdd ? '[ ADD TRANSCEIVER MODULE ]' : '[ EDIT TRANSCEIVER MODULE ]';
  if (isAdd) { $('serverForm').reset(); $('serverDirection').value = 'both'; } else {
    const srv = currentConfig.servers[idx]; $('serverType').value = srv.is_emby ? 'emby' : 'jellyfin';
    $('serverName').value = srv.name; $('serverUrl').value = srv.url;
    $('serverKey').value = srv.api_key; $('serverDirection').value = srv.sync_direction || 'both';
  }
  $('serverModal').style.display = 'flex';
}
function openSettingsModal() { $('settingsModal').style.display = 'flex'; }
function openBackfillModal() {
  $('backfillModal').style.display = 'flex';
  pollBackfillStatus();
}
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
async function startBackfill() {
  const opts = {
    direction: $('bfDirection').value,
    merge: $('bfMerge').value,
    scope: $('bfScope').value,
    rate: Math.max(1, Math.min(50, parseInt($('bfRate').value) || 5)),
    force: $('bfForce').checked,
  };
  $('bfStartBtn').disabled = true;
  $('bfCancelBtn').disabled = false;
  try {
    const res = await authedFetch('/api/backfill', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(opts) });
    showToast('BACKFILL STARTED');
    pollBackfillStatus();
  } catch (err) {
    showToast('BACKFILL FAILED: ' + err.message);
    $('bfStartBtn').disabled = false;
    $('bfCancelBtn').disabled = true;
  }
}
let _bfPollTimer = null;
async function pollBackfillStatus() {
  if (_bfPollTimer) clearTimeout(_bfPollTimer);
  try {
    const res = await authedFetch('/api/backfill/status');
    const s = await res.json();
    renderBackfillProgress(s);
    if (s.state === 'running' || s.state === 'idle' && s.total_pairs > 0) {
      _bfPollTimer = setTimeout(pollBackfillStatus, 1000);
    } else {
      _bfPollTimer = null;
      $('bfStartBtn').disabled = false;
      $('bfCancelBtn').disabled = true;
    }
  } catch (err) {
    console.error(err);
  }
}
function renderBackfillProgress(s) {
  const pct = s.total_pairs > 0 ? Math.floor((s.processed / s.total_pairs) * 100) : 0;
  const txt = s.state.toUpperCase() + ' | processed=' + s.processed + '/' + s.total_pairs +
    ' | ok=' + s.succeeded + ' skip=' + s.skipped + ' fail=' + s.failed +
    ' (' + pct + '%)' +
    (s.current_pair ? ' | pair=' + s.current_pair : '');
  const div = $('bfProgress');
  if (!div) return;
  div.textContent = txt;
}
async function cancelBackfill() {
  $('bfCancelBtn').disabled = true;
  showToast('CANCEL REQUESTED (will stop after current item)');
}
document.addEventListener('keydown', (e) => { if (e.key === 'Escape') { ['serverModal','settingsModal','authModal','backfillModal'].forEach(id => { const m=$(id); if (m && m.style.display === 'flex') m.style.display='none'; }); } });
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
