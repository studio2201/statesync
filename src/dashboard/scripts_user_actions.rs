//! Mapped-users Actions modal (Force / Ignore / Clear watched).

pub const JS_USER_ACTIONS: &str = r#"async function forceSyncForUser(name) {
  if (!name) return;
  if (!confirm('Force sync watched / resume / favorites for "' + name + '" only across all servers?')) return;
  await forceSync(false, name);
}
async function toggleIgnoreUser(name, ignore) {
  if (!name) return;
  const verb = ignore ? 'Ignore' : 'Un-ignore';
  if (ignore && !confirm(verb + ' "' + name + '"?\n\nLive sync and force will leave this person out (linked aliases too).')) return;
  try {
    const res = await authedFetch('/api/users/ignore', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name: name, ignore: !!ignore })
    });
    const body = await res.json().catch(() => ({}));
    if (!res.ok) throw new Error(body.message || ('HTTP ' + res.status));
    if (currentConfig.sync) {
      currentConfig.sync.user_ignorelist = body.user_ignorelist || currentConfig.sync.user_ignorelist || [];
    }
    showToast(body.message || (verb + ' saved'));
    setTimeout(loadDashboard, 400);
  } catch (err) {
    showToast(verb + ' failed: ' + err.message);
  }
}
function openUserActionsModal() {
  const list = window._mappedUsersList || [];
  if (!list.length) {
    showToast('No users loaded yet — refresh after servers connect');
    return;
  }
  const sel = $('userActionsSelect');
  if (!sel) return;
  sel.textContent = '';
  list.forEach(u => {
    const o = document.createElement('option');
    o.value = u.name;
    o.textContent = u.ignored ? (u.name + ' (ignored)') : u.name;
    sel.appendChild(o);
  });
  if (window._selectedMappedUser) {
    const hit = list.find(u => u.name === window._selectedMappedUser);
    if (hit) sel.value = hit.name;
  }
  refreshUserActionsIgnoreBtn();
  $('userActionsModal').style.display = 'flex';
}
function userActionsSelectedName() {
  const sel = $('userActionsSelect');
  return sel && sel.value ? sel.value : '';
}
function refreshUserActionsIgnoreBtn() {
  const name = userActionsSelectedName();
  const btn = $('userActionsIgnoreBtn');
  if (!btn) return;
  const u = (window._mappedUsersList || []).find(x => x.name === name);
  btn.textContent = (u && u.ignored) ? 'Un-ignore' : 'Ignore';
}
async function userActionsForce() {
  const name = userActionsSelectedName();
  if (!name) return showToast('Pick a user');
  window._selectedMappedUser = name;
  closeModal('userActionsModal');
  await forceSyncForUser(name);
}
async function userActionsToggleIgnore() {
  const name = userActionsSelectedName();
  if (!name) return showToast('Pick a user');
  const u = (window._mappedUsersList || []).find(x => x.name === name);
  const ignored = u ? !!u.ignored : false;
  window._selectedMappedUser = name;
  closeModal('userActionsModal');
  await toggleIgnoreUser(name, !ignored);
}
async function userActionsClearWatched() {
  const name = userActionsSelectedName();
  if (!name) return showToast('Pick a user');
  window._selectedMappedUser = name;
  closeModal('userActionsModal');
  await clearWatchedForUser(name);
}
"#;
