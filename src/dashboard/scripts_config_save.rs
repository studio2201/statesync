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
  if (btn) btn.textContent = show ? 'Hide details' : 'Details';
  localStorage.setItem('force-story-expanded', show ? 'true' : 'false');
}
function toggleForceStory() {
  setForceStoryExpanded(!isForceStoryExpanded());
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
  // Keep expand preference; default collapsed (no long story until Details).
  setForceStoryExpanded(isForceStoryExpanded());
  const dry = !!fs.dry_run || (fs.scope && fs.scope.indexOf('dry-run') >= 0);
  const title = $('fsStoryTitle');
  if (title) {
    if (fs.story_headline) title.textContent = fs.story_headline;
    else if (done && st === 'completed') title.textContent = dry ? 'Preview finished (no writes)' : 'Force sync finished';
    else if (done && st === 'failed') title.textContent = dry ? 'Preview finished with errors' : 'Force sync finished with errors';
    else title.textContent = (dry ? 'Force preview · ' : 'Force sync · ') + forcePhaseLabel(fs.phase);
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
      txt.textContent = 'elapsed ' + elapsed + 's';
    } else if (totalPairs > 0) {
      txt.textContent = pct + '% · checked ' + processed + ' of ~' + totalPairs
        + ' · updated ' + succeeded + ' · no change ' + skipped
        + (failed ? ' · failed ' + failed : '')
        + ' · ' + rate + '/s · ' + elapsed + 's';
    } else {
      txt.textContent = 'checked ' + processed + ' · updated ' + succeeded + ' · no change ' + skipped
        + (failed ? ' · failed ' + failed : '')
        + ' · ' + rate + '/s · ' + elapsed + 's';
    }
  }
  // Compact “what is happening now” — always visible when the card is shown.
  const cu = $('fsCurrentUser');
  if (cu) {
    const bits = [];
    if (fs.current_user) bits.push('Person: ' + fs.current_user);
    if (fs.current_source && fs.current_target) {
      bits.push('Route: ' + fs.current_source + ' → ' + fs.current_target);
    } else if (fs.current_source) {
      bits.push('Server: ' + fs.current_source);
    }
    if (fs.pair_total > 0 && fs.pair_index > 0) {
      bits.push('Direction ' + fs.pair_index + ' of ' + fs.pair_total);
    }
    if (bits.length) {
      cu.textContent = bits.join(' · ');
    } else if (!done) {
      cu.textContent = dry
        ? 'Preview in progress — no writes will be made.'
        : 'Working — live play sync is paused until this finishes.';
    } else {
      cu.textContent = '';
    }
  }
  // Long story text only when expanded.
  const detail = $('fsStoryDetail');
  if (detail) {
    const parts = [];
    if (fs.story_detail) parts.push(fs.story_detail);
    const sr = fs.skip_reasons || {};
    const skipBits = [];
    if (sr.already_equal) skipBits.push(sr.already_equal + ' already the same in both libraries (good)');
    if (sr.no_provider) skipBits.push(sr.no_provider + ' could not pair — source library title has no shared catalog ID (IMDb/TMDb/TVDB)');
    if (sr.no_match) skipBits.push(sr.no_match + ' could not pair — title not in the other app’s library');
    if (sr.other) skipBits.push(sr.other + ' other no-change');
    if (skipBits.length) {
      parts.push('Why no change so far: ' + skipBits.join('; ') + '.');
    }
    if (!done) {
      parts.push(dry
        ? 'This is a preview: counts only.'
        : 'Live play sync stays paused until this run ends.');
    }
    if (fs.last_error) parts.push('Last error: ' + fs.last_error);
    detail.textContent = parts.join(' ');
  }
}
"#;
