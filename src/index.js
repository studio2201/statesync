if ('serviceWorker' in navigator) { navigator.serviceWorker.register('/sw.js').catch(() => {}); }
const $ = id => document.getElementById(id);
let currentConfig = { servers: [], sync_threshold_seconds: 5 }; let editIndex = -1;
function setTheme(n) { document.body.className = n === 'cyberpunk' ? '' : `theme-${n}`; localStorage.setItem('hud-theme', n); }
async function loadDashboard() {
  try {
    const [configRes, statusRes] = await Promise.all([fetch('/api/config'), fetch('/api/status')]);
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
    if (currentConfig.servers.length === 0) { listDiv.innerHTML = '<div style="color: var(--accent)">NO CONFIGURED TRANSCEIVERS</div>'; } else {
      listDiv.innerHTML = '';
      currentConfig.servers.forEach((srv, idx) => {
        const sStatus = status.servers.find(s => s.name === srv.name) || { users_count: 0, media_count: 0, websocket_status: 'Offline' };
        const row = document.createElement('div'); row.className = 'server-row';
        const dirBadge = srv.sync_direction === 'send' ? ' [SEND ONLY]' : (srv.sync_direction === 'receive' ? ' [RCV ONLY]' : '');
        row.innerHTML = `<div class="server-info"><span class="status-${sStatus.websocket_status}">[ ${sStatus.websocket_status.toUpperCase()} ]</span><div><span style="font-weight:600;color:#fff">${srv.name}</span> <span class="badge">${srv.is_emby ? 'EMBY' : 'JELLYFIN'}${dirBadge}</span><div style="font-size:11px;color:var(--text);margin-top:2px">${srv.url}</div></div></div><div class="server-info"><span style="font-size:12px">${sStatus.users_count} USERS | ${sStatus.media_count} CACHED</span><button class="btn" onclick="openServerModal(${idx})">[ EDIT ]</button><button class="btn btn-danger" onclick="deleteServer(${idx})">[ WIPE ]</button></div>`;
        listDiv.appendChild(row);
      });
    }
    const activeDiv = $('activeSessions');
    if (status.active_sessions && status.active_sessions.length > 0) {
      activeDiv.innerHTML = '';
      status.active_sessions.forEach(sess => {
        const mins = Math.floor(sess.position / 60); const secs = Math.floor(sess.position % 60).toString().padStart(2, '0');
        const posterHtml = sess.poster_url ? `<img src="${sess.poster_url}" style="width:30px;height:45px;object-fit:cover;border:1px solid var(--accent);margin-right:12px;flex-shrink:0;">` : '';
        activeDiv.innerHTML += `<div class="server-row" style="border-color:var(--accent);${sess.poster_url ? 'padding:6px 18px;' : ''}"><div class="server-info">${posterHtml}<div><div style="font-weight:600;color:#fff">${sess.item}</div><div style="font-size:11px;color:var(--text)">USER: ${sess.user} | SOURCE: ${sess.server}</div></div></div><div style="display:flex;align-items:center;gap:10px"><span class="badge" style="border-color:var(--accent);color:var(--accent)">${mins}:${secs}</span>${sess.is_paused ? '<span style="font-size:11px;color:var(--accent)">[ PAUSED ]</span>' : ''}</div></div>`;
      });
    } else { activeDiv.innerHTML = '<div style="color:var(--accent)">NO ACTIVE STREAMS DETECTED</div>'; }
    const usersDiv = $('syncedUsers');
    if (!status.servers || status.servers.length === 0) {
      usersDiv.innerHTML = '<div style="color:var(--accent)">NO ACTIVE TRANSCEIVERS</div>';
    } else {
      usersDiv.innerHTML = '';
      const header = document.createElement('div');
      header.style.display = 'flex'; header.style.justifyContent = 'space-between';
      header.style.color = 'var(--border)'; header.style.fontWeight = '600'; header.style.fontSize = '12px';
      header.style.borderBottom = '1px solid rgba(0,240,255,0.3)'; header.style.paddingBottom = '6px'; header.style.marginBottom = '12px';
      status.servers.forEach((srv, idx) => {
        const title = document.createElement('div'); title.innerText = srv.name.toUpperCase(); title.style.width = '120px';
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
        let cells = '';
        group.forEach((username, idx) => {
          const align = idx === 0 ? 'left' : (idx === group.length - 1 ? 'right' : 'center');
          cells += username ? `<div style="color:#fff;font-size:12px;width:120px;text-align:${align}">${username}</div>` : `<div style="color:var(--text);opacity:0.3;font-size:12px;width:120px;text-align:${align}">[ UNMAPPED ]</div>`;
          if (idx < group.length - 1) {
            const isGreen = username !== null && group.slice(idx + 1).some(u => u !== null);
            cells += `<div style="flex:1;border-bottom:1px dotted ${isGreen ? 'var(--green)' : 'var(--accent)'};margin:0 15px;opacity:${isGreen ? '0.7' : '0.2'}"></div>`;
          }
        });
        usersDiv.innerHTML += `<div style="display:flex;align-items:center;padding:6px 0">${cells}</div>`;
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
        let cells = '';
        for (let i = 0; i < status.servers.length; i++) {
          const align = i === 0 ? 'left' : (i === status.servers.length - 1 ? 'right' : 'center');
          cells += `<div style="font-size:12px;width:120px;text-align:${align};color:${i === srvIdx ? 'var(--accent)' : 'var(--text);opacity:0.3'}">${i === srvIdx ? username : '[ UNMAPPED ]'}</div>`;
          if (i < status.servers.length - 1) cells += `<div style="flex:1;border-bottom:1px dotted var(--accent);margin:0 15px;opacity:0.3"></div>`;
        }
        usersDiv.innerHTML += `<div style="display:flex;align-items:center;padding:6px 0">${cells}</div>`;
      });
    }
    const logsDiv = $('syncLogs');
    if (status.sync_logs && status.sync_logs.length > 0) {
      logsDiv.innerHTML = '';
      status.sync_logs.forEach(log => {
        if (log.level === 'success' && log.source_name) {
          const sCol = log.source_is_emby ? 'var(--green)' : '#cc00ff'; const tCol = log.target_is_emby ? 'var(--green)' : '#cc00ff';
          const sBadge = log.source_is_emby ? 'EMBY' : 'JELLYFIN'; const tBadge = log.target_is_emby ? 'EMBY' : 'JELLYFIN';
          logsDiv.innerHTML += `<div class="log-line">&gt; [${log.timestamp}] ${log.message.toUpperCase()} FROM <span style="color:${sCol}">[${sBadge}: ${log.source_name.toUpperCase()}]</span> -&gt; <span style="color:${tCol}">[${tBadge}: ${log.target_name.toUpperCase()}]</span></div>`;
        } else {
          const color = log.level === 'error' ? 'var(--red)' : (log.level === 'warn' ? 'var(--accent)' : 'var(--text)');
          logsDiv.innerHTML += `<div class="log-line">&gt; [${log.timestamp}] <span style="color:${color}">[${log.level.toUpperCase()}] ${log.message.toUpperCase()}</span></div>`;
        }
      });
      logsDiv.scrollTop = logsDiv.scrollHeight;
    } else { logsDiv.innerHTML = '<div style="color:var(--green)">> LISTENING FOR METRIC EVENTS...</div>'; }
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
function closeModal(id) { $(id).style.display = 'none'; }
function testConnection() {
  const type = $('serverType').value, url = $('serverUrl').value, api_key = $('serverKey').value;
  if (!url || !api_key) return showToast('LINK DATA INCOMPLETE');
  showToast('PINGING LINK ADDRESS...');
  fetch('/api/test_connection', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ url, api_key, is_emby: type === 'emby' }) })
    .then(async r => showToast((await r.json()).message.toUpperCase())).catch(() => showToast('LINK RESPONSE FAILED'));
}
$('serverForm').addEventListener('submit', async (e) => {
  e.preventDefault();
  const s = { name: $('serverName').value, url: $('serverUrl').value, api_key: $('serverKey').value, is_emby: $('serverType').value === 'emby', sync_direction: $('serverDirection').value };
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
    const res = await fetch('/api/config', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(currentConfig) });
    showToast((await res.json()).message.toUpperCase()); setTimeout(loadDashboard, 1200);
  } catch (err) { showToast('WRITE CONFIG FAILED'); }
}
function showToast(msg) { const toast = $('toast'); toast.innerText = `> ${msg}`; toast.style.display = 'block'; setTimeout(() => { toast.style.display = 'none'; }, 4000); }
const savedTheme = localStorage.getItem('hud-theme') || 'cyberpunk'; setTheme(savedTheme); $('themeSelector').value = savedTheme; loadDashboard(); setInterval(loadDashboard, 3000);
