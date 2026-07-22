//! Force sync + clear watched UI

/// Dashboard script fragment.
pub const JS_FORCE_UI: &str = r#"async function clearWatchedForUser(name) {
  if (!name) return;
  const ok = confirm(
    'Clear ALL watched history for "' + name + '" on every connected server?\n\n' +
    'Every played item will be marked unwatched. This cannot be undone.\n\n' +
    'Favorites and libraries are not removed — only watched flags.'
  );
  if (!ok) return;
  showToast('Clearing watched for ' + name + '…');
  try {
    const res = await authedFetch('/api/users/clear_watched', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name })
    });
    const body = await res.json().catch(() => ({}));
    if (!res.ok) throw new Error(body.message || ('HTTP ' + res.status));
    showToast(body.message || ('Clearing watched for ' + name));
    setTimeout(loadDashboard, 1500);
  } catch (err) {
    showToast('Clear watched failed: ' + err.message);
  }
}
async function forceSync(dryRun, onlyUser) {
  dryRun = !!dryRun;
  onlyUser = (onlyUser && String(onlyUser).trim()) || null;
  const btn = dryRun ? $('previewForceBtn') : $('forceSyncBtn');
  const other = dryRun ? $('forceSyncBtn') : $('previewForceBtn');
  if (!onlyUser) {
    if (btn && btn.disabled) return;
    if (btn) btn.disabled = true;
    if (other) other.disabled = true;
  }
  window._forceSyncOptimistic = true;
  const scope = dryRun ? ['dry-run'] : [];
  if (onlyUser) scope.push('user=' + onlyUser);
  applyForceSyncLiveUi({
    state: 'Running',
    started_at: new Date().toISOString(),
    finished_at: null,
    total_pairs: 0,
    processed: 0,
    succeeded: 0,
    skipped: 0,
    failed: 0,
    current_user: onlyUser,
    last_error: null,
    dry_run: dryRun,
    scope: scope,
    story_headline: dryRun
      ? ('Preview started' + (onlyUser ? (' for "' + onlyUser + '"') : ' for every linked person'))
      : ('Force sync started' + (onlyUser ? (' for "' + onlyUser + '"') : ' for every linked person')),
    story_detail: dryRun
      ? 'Preview counts what would change. No data is written. Next: count watched titles on each server. Matching uses Imdb/Tmdb from each item’s server metadata — never folder or file names. Not a Movies-then-TV walk.'
      : 'Catch-up for past watched history, resume, and favorites. Live play sync is paused. Next: count watched titles on each server. Matching uses Imdb/Tmdb from Emby/Jellyfin item metadata (not disk paths). Order is each person and each server direction — not one library at a time.'
  });
  const statusHint = $('forceSyncStatus');
  if (statusHint) {
    statusHint.textContent = onlyUser
      ? ((dryRun ? 'Preview for person "' : 'Force sync for person "') + onlyUser + '"…')
      : (dryRun
        ? 'Preview started — counting watched titles (no writes)…'
        : 'Force sync started — counting watched titles on each linked server…');
  }
  showToast(onlyUser
    ? ((dryRun ? 'Preview force for ' : 'Force sync for ') + onlyUser)
    : (dryRun ? 'Force preview started (no writes)' : 'Force sync started'));
  try {
    const payload = { direction: 'Both', dry_run: dryRun };
    if (onlyUser) payload.user = onlyUser;
    const res = await authedFetch('/api/sync/force', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (!res.ok && res.status !== 202) {
      let msg = 'HTTP ' + res.status;
      try {
        const body = await res.json();
        if (body.message) msg = body.message;
      } catch (_) {
        try { msg = (await res.text()) || msg; } catch (__) {}
      }
      throw new Error(msg);
    }
    pollForceSync();
    loadDashboard();
  } catch (err) {
    window._forceSyncOptimistic = false;
    const live = $('forceSyncLive');
    if (live) live.style.display = 'none';
    showToast((dryRun ? 'Preview' : 'Force sync') + ' failed: ' + err.message);
    if (btn) btn.disabled = false;
    if (other) other.disabled = false;
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
      _forceSyncTimer = setTimeout(pollForceSync, 500);
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
function setHowSyncExpanded(show) {
  const body = $('howSyncBody');
  const btn = $('toggleHowSyncBtn');
  const hint = $('howSyncCollapsedHint');
  if (!body || !btn) return;
  body.style.display = show ? 'block' : 'none';
  btn.textContent = show ? 'Collapse' : 'Expand';
  if (hint) hint.style.display = show ? 'none' : 'block';
  localStorage.setItem('how-sync-expanded', show ? 'true' : 'false');
}
function toggleHowSync() {
  const body = $('howSyncBody');
  if (!body) return;
  setHowSyncExpanded(body.style.display === 'none');
}
function initHowSyncToggle() {
  // Default expanded so the overview is visible on first visit
  const show = localStorage.getItem('how-sync-expanded') !== 'false';
  setHowSyncExpanded(show);
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
