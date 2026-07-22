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
  if (p === 'played') return 'Watched history';
  if (p === 'favorites') return 'Favorites';
  if (p === 'finishing') return 'Finishing';
  if (p === 'done') return 'Done';
  if (p === 'cancelled') return 'Cancelled';
  return 'Force sync';
}
/** Collapsed by default: bar + what is happening. Expand for full story text. */
function isForceStoryExpanded() {
  return localStorage.getItem('force-story-expanded') === 'true';
}
function setForceStoryExpanded(show) {
  const body = $('fsStoryExpanded');
  const btn = $('fsStoryToggleBtn');
  if (body) body.style.display = show ? 'block' : 'none';
  // Fixed label width via CSS; keep short so the control stays put.
  if (btn) btn.textContent = show ? 'Hide' : 'Details';
  localStorage.setItem('force-story-expanded', show ? 'true' : 'false');
}
function toggleForceStory() {
  setForceStoryExpanded(!isForceStoryExpanded());
}
function fsFactLine(label, value) {
  const row = document.createElement('div');
  row.className = 'fs-fact';
  const k = document.createElement('strong');
  k.textContent = label;
  row.appendChild(k);
  row.appendChild(document.createTextNode(value));
  return row;
}
function applyForceSyncLiveUi(fs) {
  const live = $('forceSyncLive');
  if (!live || !fs) return;
  const totalPairs = fs.total_pairs || 0;
  const processed = fs.processed || 0;
  const succeeded = fs.succeeded || 0;
  const skipped = fs.skipped || 0;
  const failed = fs.failed || 0;
  const phase = String(fs.phase || '').toLowerCase();
  const preparing = phase === 'preparing';
  const pct = totalPairs > 0 ? Math.min(100, Math.floor(processed / totalPairs * 100)) : 0;
  const startedMs = fs.started_at ? new Date(fs.started_at).getTime() : Date.now();
  const elapsed = Math.max(0, Math.round((Date.now() - startedMs) / 1000));
  const rate = elapsed > 0 ? (processed / elapsed).toFixed(1) : '0';
  const st = forceStateKey(fs.state);
  const done = st === 'completed' || st === 'failed' || !!fs.finished_at;
  live.style.display = 'flex';
  setForceStoryExpanded(isForceStoryExpanded());
  const dry = !!fs.dry_run || (fs.scope && fs.scope.indexOf('dry-run') >= 0);
  const title = $('fsStoryTitle');
  if (title) {
    if (fs.story_headline) title.textContent = fs.story_headline;
    else if (done && st === 'completed') title.textContent = dry ? 'Preview finished' : 'Force finished';
    else if (done && st === 'failed') title.textContent = dry ? 'Preview finished (failures)' : 'Force finished (failures)';
    else title.textContent = (dry ? 'Preview · ' : 'Force · ') + forcePhaseLabel(fs.phase);
  }
  const bar = $('fsProgressBar');
  if (bar) {
    if (done) bar.value = 100;
    else if (preparing) bar.value = Math.min(8, 2 + (elapsed % 6));
    else if (totalPairs > 0) bar.value = Math.max(pct, processed > 0 ? 1 : 0);
    else bar.value = Math.min(95, processed > 0 ? 5 + (processed % 90) : (elapsed % 10));
    bar.max = 100;
  }
  const txt = $('fsProgressText');
  if (txt) {
    if (preparing && !done) {
      txt.textContent = elapsed + 's';
    } else if (totalPairs > 0) {
      txt.textContent = pct + '% · ' + processed + '/' + totalPairs
        + ' · ↑' + succeeded + ' · =' + skipped
        + (failed ? ' · ✕' + failed : '')
        + ' · ' + rate + '/s · ' + elapsed + 's';
    } else {
      txt.textContent = processed + ' · ↑' + succeeded + ' · =' + skipped
        + (failed ? ' · ✕' + failed : '')
        + ' · ' + rate + '/s · ' + elapsed + 's';
    }
  }
  // Always-visible line: who + route only.
  const cu = $('fsCurrentUser');
  if (cu) {
    const bits = [];
    if (fs.current_user) bits.push(fs.current_user);
    if (fs.current_source && fs.current_target) bits.push(fs.current_source + ' → ' + fs.current_target);
    else if (fs.current_source) bits.push(fs.current_source);
    if (fs.pair_total > 0 && fs.pair_index > 0) bits.push(fs.pair_index + '/' + fs.pair_total);
    if (dry) bits.push('preview');
    if (!done) bits.push('live paused');
    cu.textContent = bits.join(' · ');
  }
  // Details: dense facts only (what / who / route / counts / failures).
  const detail = $('fsStoryDetail');
  if (detail) {
    detail.textContent = '';
    const sr = fs.skip_reasons || {};
    const mode = dry ? 'preview (no writes)' : 'write';
    const route = (fs.current_source && fs.current_target)
      ? (fs.current_source + ' → ' + fs.current_target)
      : (fs.current_source || '—');
    detail.appendChild(fsFactLine('Step', forcePhaseLabel(fs.phase) + (fs.story_headline ? (' — ' + fs.story_headline) : '')));
    detail.appendChild(fsFactLine('Person', fs.current_user || '—'));
    detail.appendChild(fsFactLine('Route', route + (fs.pair_total > 0 ? (' · ' + (fs.pair_index || 0) + '/' + fs.pair_total) : '')));
    detail.appendChild(fsFactLine('Mode', mode));
    detail.appendChild(fsFactLine('Match', 'library catalog IDs: IMDb · TMDb · TVDB'));
    detail.appendChild(fsFactLine('Counts',
      'checked ' + processed
      + (totalPairs ? (' / ~' + totalPairs) : '')
      + ' · updated ' + succeeded
      + ' · no change ' + skipped
      + ' · failed ' + failed
      + ' · ' + rate + '/s · ' + elapsed + 's'
    ));
    const nc = [];
    if (sr.already_equal) nc.push(sr.already_equal + ' already same');
    if (sr.no_provider) nc.push(sr.no_provider + ' no catalog ID');
    if (sr.no_match) nc.push(sr.no_match + ' not in other library');
    if (sr.other) nc.push(sr.other + ' other');
    if (nc.length) detail.appendChild(fsFactLine('No change', nc.join(' · ')));
    const bf = fs.by_field || {};
    const pl = bf.played || {};
    const fv = bf.favorite || {};
    if ((pl.ok || pl.skip || pl.fail) || (fv.ok || fv.skip || fv.fail)) {
      detail.appendChild(fsFactLine('By field',
        'watched ↑' + (pl.ok || 0) + ' =' + (pl.skip || 0) + ' ✕' + (pl.fail || 0)
        + ' · favorites ↑' + (fv.ok || 0) + ' =' + (fv.skip || 0) + ' ✕' + (fv.fail || 0)
      ));
    }
    if (fs.scope && fs.scope.length) detail.appendChild(fsFactLine('Scope', fs.scope.join(', ')));
    if (fs.story_detail) detail.appendChild(fsFactLine('Note', fs.story_detail));
  }
  const failBox = $('fsFailureList');
  if (failBox) {
    failBox.textContent = '';
    if (fs.errors && fs.errors.length) {
      failBox.style.display = 'block';
      const head = document.createElement('div');
      head.style.fontWeight = '600';
      head.style.color = 'var(--bright)';
      head.style.marginBottom = '4px';
      head.textContent = 'Failures (' + fs.errors.length + ')';
      failBox.appendChild(head);
      fs.errors.slice(-12).forEach(function (e) {
        const line = document.createElement('div');
        line.className = 'fs-fail-line';
        line.textContent = (e.user || '—') + ' · ' + (e.server || '—') + ' · ' + (e.message || 'error');
        failBox.appendChild(line);
      });
    } else {
      failBox.style.display = 'none';
    }
  }
}
"#;
