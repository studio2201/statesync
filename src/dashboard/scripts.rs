//! Core JavaScript client logic for the StateSync dashboard.

/// Core dashboard script (init + data rendering).
pub const JS_CORE: &str = r#"if ('serviceWorker' in navigator) { navigator.serviceWorker.register('/sw.js').catch(() => {}); }
const $ = id => document.getElementById(id);
let currentConfig = { servers: [], sync_threshold_seconds: 5 }; let editIndex = -1;
function esc(s) { if (s == null) return ''; return String(s).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'})[c]); }
async function authedFetch(url, opts) {
  opts = opts || {};
  return fetch(url, opts);
}
async function loadPoster(url, img) {
  try {
    const r = await authedFetch(url);
    if (!r.ok) return;
    const blob = await r.blob();
    const obj = URL.createObjectURL(blob);
    img.onload = () => { try { URL.revokeObjectURL(obj); } catch (_) {} };
    img.src = obj;
  } catch (_) {}
}
function setTheme(n) { document.body.className = n === 'cyberpunk' ? '' : `theme-${n}`; localStorage.setItem('hud-theme', n); }
function nameFromUrl(url) {
  try {
    let u = (url || '').trim();
    if (!u) return 'server';
    if (!/^https?:\/\//i.test(u)) u = 'http://' + u;
    const h = new URL(u).hostname;
    return h || 'server';
  } catch (_) {
    return (url || 'server').replace(/^https?:\/\//i, '').split('/')[0].split(':')[0] || 'server';
  }
}
async function loadDashboard() {
  try {
    const [configRes, statusRes] = await Promise.all([
      authedFetch('/api/config'),
      authedFetch('/api/status')
    ]);
    currentConfig = await configRes.json(); const status = await statusRes.json();
    if ($('syncThreshold')) $('syncThreshold').value = currentConfig.sync_threshold_seconds;
    if ($('cfgUserMappings')) {
      $('cfgUserMappings').value = (currentConfig.user_mappings || []).map(group => group.join(', ')).join('\n');
    }
    const listDiv = $('serverList');
    if (!listDiv) return;
    if (currentConfig.servers.length === 0) {
      listDiv.textContent = '';
      const empty = document.createElement('div'); empty.className = 'empty'; empty.textContent = 'No servers yet. Click Add server.';
      listDiv.appendChild(empty);
    } else {
      listDiv.textContent = '';
      currentConfig.servers.forEach((srv, idx) => {
        const sStatus = status.servers.find(s => s.name === srv.name) || { users_count: 0, media_count: 0, websocket_status: 'Offline' };
        const row = document.createElement('div'); row.className = 'server-row';
        const dirBadge = srv.sync_direction === 'send' ? ' · send' : (srv.sync_direction === 'receive' ? ' · receive' : '');
        const urlText = (status.servers.find(s => s.name === srv.name) || {}).url || srv.url;

        const left = document.createElement('div'); left.className = 'server-info';
        const statusSpanEl = document.createElement('span');
        statusSpanEl.className = 'status-' + sStatus.websocket_status;
        statusSpanEl.textContent = sStatus.websocket_status;
        const leftInner = document.createElement('div'); leftInner.className = 'server-meta';
        const nameEl = document.createElement('div'); nameEl.className = 'name';
        nameEl.textContent = (srv.name || nameFromUrl(srv.url)) + ' ';
        const badgeEl = document.createElement('span'); badgeEl.className = 'badge';
        badgeEl.textContent = (srv.is_emby ? 'Emby' : 'Jellyfin') + dirBadge;
        nameEl.appendChild(badgeEl);
        const urlEl = document.createElement('div'); urlEl.className = 'url'; urlEl.textContent = urlText;
        leftInner.appendChild(nameEl); leftInner.appendChild(urlEl);
        left.appendChild(statusSpanEl); left.appendChild(leftInner);

        const right = document.createElement('div'); right.className = 'server-info'; right.style.flex = '0 0 auto';
        const metaSpan = document.createElement('span'); metaSpan.style.fontSize = '12px'; metaSpan.style.color = 'var(--muted)';
        if (['Scanning','Validating','Connecting','Reconnecting'].includes(sStatus.websocket_status)) {
          metaSpan.textContent = sStatus.websocket_status + '…';
        } else {
          metaSpan.textContent = (sStatus.users_count || 0) + ' users';
        }
        const editBtn = document.createElement('button'); editBtn.className = 'btn'; editBtn.textContent = 'Edit';
        editBtn.addEventListener('click', () => openServerModal(idx));
        const removeBtn = document.createElement('button'); removeBtn.className = 'btn btn-danger'; removeBtn.textContent = 'Remove';
        removeBtn.addEventListener('click', () => deleteServer(idx));
        right.appendChild(metaSpan); right.appendChild(editBtn); right.appendChild(removeBtn);

        row.appendChild(left); row.appendChild(right);
        listDiv.appendChild(row);
      });
    }
    const activeDiv = $('activeSessions');
    if (status.active_sessions && status.active_sessions.length > 0) {
      activeDiv.textContent = '';
      status.active_sessions.forEach(sess => {
        const mins = Math.floor(sess.position / 60); const secs = Math.floor(sess.position % 60).toString().padStart(2, '0');
        const durationStr = mins + ':' + secs;
        const row = document.createElement('div'); row.className = 'server-row';
        const left = document.createElement('div'); left.className = 'server-info';
        if (sess.poster_url) {
          const img = document.createElement('img');
          img.alt = '';
          img.style.cssText = 'width:30px;height:45px;object-fit:cover;border-radius:4px;border:1px solid var(--border);flex-shrink:0;';
          loadPoster(sess.poster_url, img);
          left.appendChild(img);
        }
        const meta = document.createElement('div'); meta.className = 'server-meta';
        const itemEl = document.createElement('div'); itemEl.className = 'name'; itemEl.textContent = sess.item;
        const userEl = document.createElement('div'); userEl.className = 'url';
        userEl.textContent = sess.is_paused
          ? (sess.user + ' paused on ' + sess.server + ' at ' + durationStr)
          : (sess.user + ' watching on ' + sess.server);
        meta.appendChild(itemEl); meta.appendChild(userEl);
        left.appendChild(meta);
        const right = document.createElement('div'); right.style.cssText = 'display:flex;align-items:center;gap:8px';
        const badge = document.createElement('span'); badge.className = 'badge'; badge.textContent = durationStr;
        right.appendChild(badge);
        if (sess.is_paused) {
          const p = document.createElement('span'); p.className = 'badge'; p.textContent = 'Paused';
          right.appendChild(p);
        }
        row.appendChild(left); row.appendChild(right);
        activeDiv.appendChild(row);
      });
    } else {
      activeDiv.textContent = '';
      const empty = document.createElement('div'); empty.className = 'empty'; empty.textContent = 'No one is playing anything right now.';
      activeDiv.appendChild(empty);
    }
    const usersDiv = $('syncedUsers');
    if (!status.servers || status.servers.length === 0) {
      usersDiv.textContent = '';
      const empty = document.createElement('div'); empty.className = 'empty'; empty.textContent = 'Add two servers to map users.';
      usersDiv.appendChild(empty);
    } else {
      usersDiv.textContent = '';
      const serverCount = status.servers.length;
      const headerRow = document.createElement('div');
      headerRow.style.cssText = 'display:grid;grid-template-columns:repeat(' + serverCount + ', 1fr);gap:6px;margin-bottom:6px';
      status.servers.forEach(srv => {
        const h = document.createElement('div');
        h.style.cssText = 'text-align:center;color:var(--muted);font-weight:600;font-size:11px;padding-bottom:6px;border-bottom:1px solid var(--border);text-transform:uppercase';
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
          cell.textContent = filled ? u.name : '·';
          cell.title = filled
            ? (u.servers.length > 1
                ? u.name + ' is mapped across servers.'
                : u.name + ' only exists on ' + status.servers[i].name + '.')
            : (status.servers[i] ? status.servers[i].name + ' has no user named ' + u.name : '');
          row.appendChild(cell);
        }
        grid.appendChild(row);
      });
      usersDiv.appendChild(grid);
      const mappedCount = users.filter(u => u.servers.length > 1).length;
      const singleCount = users.length - mappedCount;
      const legend = document.createElement('div');
      legend.className = 'form-hint';
      legend.style.cssText = 'margin-top:12px;display:flex;gap:16px;flex-wrap:wrap';
      legend.textContent = users.length + ' users · ' + mappedCount + ' mapped · ' + singleCount + ' single-server';
      usersDiv.appendChild(legend);
    }
"#;
