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
/** First-principles labels for media-server link state (what is actually true). */
function serverStatusLabel(raw) {
  const s = String(raw || 'Offline');
  const map = {
    'Synchronizing': 'Live',
    'Connected': 'Live',
    'Validating': 'Checking access',
    'Scanning': 'Loading data',
    'Connecting': 'Connecting',
    'Reconnecting': 'Reconnecting',
    'Offline': 'Offline',
    'Error': 'Failed'
  };
  return map[s] || s;
}
function serverStatusClass(raw) {
  const s = String(raw || 'Offline');
  if (s === 'Synchronizing' || s === 'Connected') return 'status-live';
  if (s === 'Error') return 'status-failed';
  return 'status-pending';
}
/** Load a now-playing poster. Prefer direct same-origin src (CSP-safe). */
async function loadPoster(url, img) {
  if (!url || !img) return;
  // Direct src works without blob: CSP and avoids silent failures on re-render.
  img.onerror = () => {
    img.style.display = 'none';
    if (img.parentNode) {
      const ph = document.createElement('div');
      ph.className = 'poster-missing';
      ph.title = 'Poster unavailable';
      img.parentNode.insertBefore(ph, img);
    }
  };
  img.src = url;
  img.className = (img.className ? img.className + ' ' : '') + 'poster-thumb';
}
/** Keep only scheme://host:port — strip /web/…, #!, query strings. */
function normalizeServerUrl(url) {
  let u = (url || '').trim();
  if (!u) return '';
  u = u.split('#')[0].split('?')[0].trim();
  if (!/^https?:\/\//i.test(u)) u = 'http://' + u;
  try {
    const parsed = new URL(u);
    const port = parsed.port ? (':' + parsed.port) : '';
    return parsed.protocol + '//' + parsed.hostname + port;
  } catch (_) {
    // Fallback: scheme://host[:port] before any path
    const m = u.match(/^(https?:\/\/[^\/]+)/i);
    return m ? m[1].replace(/\/$/, '') : u.replace(/\/$/, '');
  }
}
/** Auto name = host:port so same-IP different-port servers stay distinct. */
function nameFromUrl(url) {
  try {
    const u = normalizeServerUrl(url);
    if (!u) return 'server';
    const parsed = new URL(u);
    const host = parsed.hostname || '';
    if (!host) return 'server';
    // Include port when the URL has one (or when non-default is explicit in href).
    if (parsed.port) return host + ':' + parsed.port;
    return host;
  } catch (_) {
    const bare = (url || 'server').replace(/^https?:\/\//i, '').split('/')[0] || 'server';
    return bare || 'server';
  }
}
async function loadDashboard() {
  try {
    const [configRes, statusRes] = await Promise.all([
      authedFetch('/api/config'),
      authedFetch('/api/status')
    ]);
    currentConfig = await configRes.json(); const status = await statusRes.json();
    if (!currentConfig.sync) currentConfig.sync = {
      live_played: true, live_position: true, live_favorites: true,
      force_played: true, force_position: true, force_favorites: true,
      user_allowlist: []
    };
    if ($('syncThreshold')) $('syncThreshold').value = currentConfig.sync_threshold_seconds;
    const s = currentConfig.sync;
    const setChk = (id, v) => { const el = $(id); if (el) el.checked = !!v; };
    setChk('syncLivePlayed', s.live_played !== false);
    setChk('syncLivePosition', s.live_position !== false);
    setChk('syncLiveFavorites', s.live_favorites !== false);
    setChk('syncForcePlayed', s.force_played !== false);
    setChk('syncForcePosition', s.force_position !== false);
    setChk('syncForceFavorites', s.force_favorites !== false);
    if ($('cfgUserAllowlist')) {
      $('cfgUserAllowlist').value = (s.user_allowlist || []).join('\n');
    }
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
        const rawWs = sStatus.websocket_status || 'Offline';
        statusSpanEl.className = serverStatusClass(rawWs);
        statusSpanEl.textContent = serverStatusLabel(rawWs);
        statusSpanEl.title = 'Raw: ' + rawWs;
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
        if (['Scanning','Validating','Connecting','Reconnecting'].includes(rawWs)) {
          metaSpan.textContent = serverStatusLabel(rawWs) + '…';
        } else if (rawWs === 'Error') {
          metaSpan.textContent = 'see activity log';
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
          img.alt = sess.item || '';
          img.className = 'poster-thumb';
          img.loading = 'lazy';
          loadPoster(sess.poster_url, img);
          left.appendChild(img);
        } else {
          const ph = document.createElement('div');
          ph.className = 'poster-missing';
          left.appendChild(ph);
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
      const users = (status.users || []).slice().sort((a, b) =>
        a.name.localeCompare(b.name, undefined, { sensitivity: 'base', numeric: true })
      );
      const grid = document.createElement('div');
      grid.style.cssText = 'display:grid;grid-template-columns:repeat(' + serverCount + ', 1fr) auto;gap:6px;align-items:center';
      const headerRow2 = document.createElement('div');
      headerRow2.style.cssText = 'display:grid;grid-template-columns:repeat(' + serverCount + ', 1fr) auto;gap:6px;margin-bottom:6px';
      status.servers.forEach(srv => {
        const h = document.createElement('div');
        h.style.cssText = 'text-align:center;color:var(--muted);font-weight:600;font-size:11px;padding-bottom:6px;border-bottom:1px solid var(--border);text-transform:uppercase';
        h.textContent = srv.name;
        headerRow2.appendChild(h);
      });
      const hAct = document.createElement('div');
      hAct.style.cssText = 'text-align:center;color:var(--muted);font-weight:600;font-size:11px;padding-bottom:6px;border-bottom:1px solid var(--border);text-transform:uppercase';
      hAct.textContent = 'Actions';
      headerRow2.appendChild(hAct);
      usersDiv.appendChild(headerRow2);
      users.forEach(u => {
        const row = document.createElement('div');
        row.style.cssText = 'display:contents';
        for (let i = 0; i < serverCount; i++) {
          const cell = document.createElement('div');
          const filled = u.servers.includes(i);
          cell.className = 'user-cell' + (filled ? ' filled' : ' empty');
          if (filled) {
            cell.textContent = u.name;
            cell.title = u.servers.length > 1
              ? u.name + ' is linked across servers'
              : u.name + ' only on ' + status.servers[i].name + ' — use Link users';
          } else {
            cell.textContent = '·';
            cell.title = 'No linked user on ' + status.servers[i].name;
          }
          row.appendChild(cell);
        }
        const act = document.createElement('div');
        act.style.cssText = 'display:flex;justify-content:flex-end';
        const clr = document.createElement('button');
        clr.className = 'btn btn-danger';
        clr.style.cssText = 'font-size:11px;padding:4px 8px';
        clr.textContent = 'Clear watched';
        clr.title = 'Mark all watched items unwatched for this person on every server';
        clr.addEventListener('click', () => clearWatchedForUser(u.name));
        act.appendChild(clr);
        row.appendChild(act);
        grid.appendChild(row);
      });
      usersDiv.appendChild(grid);
      const mappedCount = users.filter(u => u.servers.length > 1).length;
      const singleCount = users.length - mappedCount;
      const legend = document.createElement('div');
      legend.className = 'form-hint';
      legend.style.cssText = 'margin-top:12px;display:flex;gap:16px;flex-wrap:wrap;align-items:center';
      legend.textContent = users.length + ' rows · ' + mappedCount + ' linked · ' + singleCount + ' need a link';
      if (singleCount > 0) {
        const tip = document.createElement('button');
        tip.className = 'btn';
        tip.style.marginLeft = 'auto';
        tip.textContent = 'Link users';
        tip.onclick = openMapUsersModal;
        legend.appendChild(tip);
      }
      usersDiv.appendChild(legend);
    }
"#;
