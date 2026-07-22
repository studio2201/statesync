//! Server modal form handlers

/// Dashboard script fragment.
pub const JS_SERVER_FORM: &str = r#"function openServerModal(idx) {
  editIndex = idx; const isAdd = idx === -1;
  $('modalTitle').innerText = isAdd ? 'Add server' : 'Edit server';
  const keyInput = $('serverKey');
  const liveHint = $('serverLiveHint');
  if (isAdd) {
    $('serverForm').reset();
    $('serverName').value = '';
    setDetectedType(null);
    pickDirection('both');
    if (keyInput) {
      keyInput.value = '';
      keyInput.placeholder = 'API key from Emby/Jellyfin';
      keyInput.required = true;
    }
    if (liveHint) { liveHint.style.display = 'none'; liveHint.textContent = ''; }
  } else {
    const srv = currentConfig.servers[idx];
    setDetectedType(!!srv.is_emby, false);
    $('serverName').value = srv.name || '';
    $('serverUrl').value = srv.url;
    // Never put the masked •••• key into the field — Test would send bullets and fail
    // while Live still works with the real saved key.
    if (keyInput) {
      keyInput.value = '';
      keyInput.placeholder = 'Leave blank to keep saved key';
      keyInput.required = false;
    }
    pickDirection(srv.sync_direction || 'both');
    if (liveHint) {
      const st = (window._lastStatus && window._lastStatus.servers && window._lastStatus.servers[idx]) || {};
      const raw = st.websocket_status || '';
      const live = (raw === 'Connected' || raw === 'Synchronizing');
      liveHint.style.display = 'block';
      liveHint.style.color = live ? 'var(--green)' : 'var(--muted)';
      liveHint.textContent = live
        ? 'Live right now with the saved settings. Test uses this address; leave API key blank to reuse the saved key.'
        : 'Not Live yet with saved settings. Test checks address + key (blank key = saved key).';
    }
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
/** isEmby: true/false/null. confirmed=true after Test connection or save detect. */
function setDetectedType(isEmby, confirmed) {
  // Type is kept for API path hints only — not shown as a product label.
  const hint = $('serverTypeHint');
  if (isEmby === null || isEmby === undefined) {
    $('serverType').value = '';
    if (hint) {
      hint.textContent = 'Works with Emby or Jellyfin. Type is handled automatically.';
      hint.style.color = '';
    }
    return;
  }
  $('serverType').value = isEmby ? 'emby' : 'jellyfin';
  if (hint) {
    hint.textContent = confirmed
      ? 'Connection OK. Ready to save.'
      : 'Works with Emby or Jellyfin. Type is handled automatically.';
    hint.style.color = confirmed ? 'var(--green)' : '';
  }
}
function pickType(t) {
  // Kept for any leftover callers; type is auto-detected.
  setDetectedType(t === 'emby', false);
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

"#;
