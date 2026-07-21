//! Server modal form handlers

/// Dashboard script fragment.
pub const JS_SERVER_FORM: &str = r#"function openServerModal(idx) {
  editIndex = idx; const isAdd = idx === -1;
  $('modalTitle').innerText = isAdd ? 'Add server' : 'Edit server';
  if (isAdd) {
    $('serverForm').reset();
    $('serverName').value = '';
    setDetectedType(null);
    pickDirection('both');
  } else {
    const srv = currentConfig.servers[idx];
    setDetectedType(!!srv.is_emby, false);
    $('serverName').value = srv.name || '';
    $('serverUrl').value = srv.url;
    $('serverKey').value = srv.api_key;
    pickDirection(srv.sync_direction || 'both');
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
  const hint = $('serverTypeHint');
  if (isEmby === null || isEmby === undefined) {
    $('serverType').value = '';
    if (hint) hint.textContent = 'Emby vs Jellyfin is detected automatically when you test or save.';
    return;
  }
  $('serverType').value = isEmby ? 'emby' : 'jellyfin';
  if (hint) {
    hint.textContent = confirmed
      ? ('Detected: ' + (isEmby ? 'Emby' : 'Jellyfin'))
      : ('Saved as ' + (isEmby ? 'Emby' : 'Jellyfin') + ' (re-detected on test/save)');
    hint.style.color = 'var(--green)';
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
