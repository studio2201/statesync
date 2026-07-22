//! Settings save + connection test handlers.
pub const JS_CONFIG_SAVE: &str = r#"function testConnection() {
  let url = normalizeServerUrl($('serverUrl').value);
  $('serverUrl').value = url;
  const api_key = $('serverKey').value.trim();
  if (!url || !api_key) return showToast('Enter a server address and API key first');
  showToast('Testing connection…');
  detectServerType(url, api_key, false)
    .then(d => {
      if (d.ok) {
        setDetectedType(d.is_emby, true);
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
  showToast('Detecting server type…');
  let is_emby = $('serverType').value === 'emby';
  try {
    const det = await detectServerType(url, api_key, is_emby);
    if (!det.ok) {
      showToast(det.message || 'Could not reach server — fix address/API key before saving');
      return;
    }
    is_emby = det.is_emby;
    if (det.url) { url = det.url; $('serverUrl').value = url; }
    setDetectedType(is_emby, true);
  } catch (err) {
    showToast('Could not detect server type: ' + (err.message || 'unreachable'));
    return;
  }
  // Name is optional — backend fills from hostname if empty
  let name = ($('serverName').value || '').trim();
  if (!name) name = nameFromUrl(url);
  const s = {
    name,
    url,
    api_key,
    is_emby,
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
  const chk = (id, def) => { const el = $(id); return el ? !!el.checked : def; };
  const allowRaw = ($('cfgUserAllowlist') && $('cfgUserAllowlist').value) || '';
  const user_allowlist = allowRaw.split(/[\n,]+/).map(s => s.trim()).filter(s => s.length > 0);
  const ignRaw = ($('cfgUserIgnorelist') && $('cfgUserIgnorelist').value) || '';
  const user_ignorelist = ignRaw.split(/[\n,]+/).map(s => s.trim()).filter(s => s.length > 0);
  currentConfig.sync = {
    live_played: chk('syncLivePlayed', true),
    live_position: chk('syncLivePosition', true),
    live_favorites: chk('syncLiveFavorites', true),
    force_played: chk('syncForcePlayed', true),
    force_position: chk('syncForcePosition', true),
    force_favorites: chk('syncForceFavorites', true),
    user_allowlist,
    user_ignorelist
  };
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
function forcePhaseLabel(phase) {
  const p = String(phase || '').toLowerCase();
  if (p === 'preparing') return 'Preparing';
  if (p === 'played') return 'Played history';
  if (p === 'favorites') return 'Favorites';
  if (p === 'finishing') return 'Finishing';
  if (p === 'done') return 'Done';
  if (p === 'cancelled') return 'Cancelled';
  return 'Force sync';
}
function applyForceSyncLiveUi(fs) {
  const live = $('forceSyncLive');
  if (!live || !fs) return;
  const totalPairs = fs.total_pairs || 0;
  const processed = fs.processed || 0;
  const pct = totalPairs > 0 ? Math.min(100, Math.floor(processed / totalPairs * 100)) : 0;
  const startedMs = fs.started_at ? new Date(fs.started_at).getTime() : Date.now();
  const elapsed = Math.max(0, Math.round((Date.now() - startedMs) / 1000));
  const rate = elapsed > 0 ? (processed / elapsed).toFixed(1) : '0';
  const st = forceStateKey(fs.state);
  const done = st === 'completed' || st === 'failed' || !!fs.finished_at;
  live.style.display = 'flex';
  const dry = !!fs.dry_run || (fs.scope && fs.scope.indexOf('dry-run') >= 0);
  const title = $('fsStoryTitle');
  if (title) {
    if (done && st === 'completed') title.textContent = dry ? 'Force preview finished (no writes)' : 'Force sync finished';
    else if (done && st === 'failed') title.textContent = dry ? 'Force preview finished with errors' : 'Force sync finished with errors';
    else title.textContent = (dry ? 'Force preview · ' : 'Force sync · ') + forcePhaseLabel(fs.phase);
  }
  const bar = $('fsProgressBar');
  if (bar) { bar.value = done && totalPairs === 0 ? 100 : pct; bar.max = 100; }
  const txt = $('fsProgressText');
  if (txt) {
    txt.textContent = totalPairs > 0
      ? (pct + '% · ' + processed + ' / ' + totalPairs + ' · ' + rate + '/s')
      : (processed + ' items · ' + (done ? 'done' : 'starting…'));
  }
  const cu = $('fsCurrentUser');
  if (cu) {
    const phase = String(fs.phase || '').toLowerCase();
    if (fs.current_user) {
      cu.textContent = (phase === 'favorites' ? 'Favorites for: ' : 'Working on user: ') + fs.current_user;
    } else if (phase === 'preparing' || processed === 0) {
      cu.textContent = 'Building user pairs and loading history…';
    } else if (phase === 'favorites') {
      cu.textContent = 'Copying hearts across servers…';
    } else if (phase === 'played') {
      cu.textContent = 'Matching titles and pushing played state…';
    } else {
      cu.textContent = '';
    }
  }
  const detail = $('fsStoryDetail');
  if (detail) {
    const bf = fs.by_field || {};
    const played = bf.played || {};
    const fav = bf.favorite || {};
    const parts = [];
    if (!done) parts.push(dry ? 'Preview only — no server data is changed.' : 'Live play sync is paused while this runs.');
    if (fs.scope && fs.scope.length) parts.push('Scope: ' + fs.scope.join(', ') + '.');
    parts.push((dry ? 'Would push ' : 'Pushed ') + (fs.succeeded || 0) + ', skipped ' + (fs.skipped || 0) + ', failed ' + (fs.failed || 0) + '.');
    if (played.ok || played.skip || played.fail) {
      parts.push('Played ' + (played.ok || 0) + ' ok / ' + (played.skip || 0) + ' skip / ' + (played.fail || 0) + ' fail.');
    }
    if (fav.ok || fav.skip || fav.fail) {
      parts.push('Favorites ' + (fav.ok || 0) + ' ok / ' + (fav.skip || 0) + ' skip / ' + (fav.fail || 0) + ' fail.');
    }
    const sr = fs.skip_reasons || {};
    const skipBits = [];
    if (sr.already_equal) skipBits.push(sr.already_equal + ' already matched');
    if (sr.no_provider) skipBits.push(sr.no_provider + ' no IMDb/TMDb');
    if (sr.no_match) skipBits.push(sr.no_match + ' not in other library');
    if (sr.other) skipBits.push(sr.other + ' other');
    if (skipBits.length) parts.push('Skips: ' + skipBits.join(', ') + '.');
    if (fs.last_error) parts.push('Last error: ' + fs.last_error);
    if (elapsed > 0) parts.push('Elapsed ' + elapsed + 's.');
    detail.textContent = parts.join(' ');
  }
}
"#;
