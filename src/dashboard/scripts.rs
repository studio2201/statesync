//! Core dashboard load + server list.

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
    "#;
