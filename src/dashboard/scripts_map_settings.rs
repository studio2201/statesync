//! Link-users modal handlers.
pub const JS_MAP_SETTINGS: &str = r#"function openMapUsersModal() {
  const st = window._lastStatus;
  const by = (st && st.users_by_server) || [];
  if (by.length < 2) {
    showToast('Add at least two servers and refresh users first');
    return;
  }
  // Use first two servers for the simple picker (most common: Emby + Jellyfin)
  window._mapServerA = by[0];
  window._mapServerB = by[1];
  const labA = $('mapServerALabel');
  const labB = $('mapServerBLabel');
  if (labA) labA.textContent = 'User on ' + (by[0].name || 'server A');
  if (labB) labB.textContent = 'User on ' + (by[1].name || 'server B');
  fillUserSelect($('mapUserA'), by[0].users || []);
  fillUserSelect($('mapUserB'), by[1].users || []);
  renderMapLinksList();
  $('mapUsersModal').style.display = 'flex';
}
function fillUserSelect(sel, names) {
  if (!sel) return;
  sel.textContent = '';
  const opt0 = document.createElement('option');
  opt0.value = '';
  opt0.textContent = names.length ? '— select user —' : '— no users loaded —';
  sel.appendChild(opt0);
  names.slice().sort((a,b) => a.localeCompare(b)).forEach(n => {
    const o = document.createElement('option');
    o.value = n;
    o.textContent = n;
    sel.appendChild(o);
  });
}
function renderMapLinksList() {
  const list = $('mapLinksList');
  if (!list) return;
  list.textContent = '';
  const maps = currentConfig.user_mappings || [];
  if (!maps.length) {
    const empty = document.createElement('div');
    empty.className = 'empty';
    empty.textContent = 'No manual links yet. Exact same usernames still match automatically.';
    list.appendChild(empty);
    return;
  }
  maps.forEach((group, idx) => {
    const row = document.createElement('div');
    row.className = 'map-link-row';
    const label = document.createElement('span');
    label.textContent = group.join('  ↔  ');
    const rm = document.createElement('button');
    rm.className = 'btn btn-danger';
    rm.textContent = 'Remove';
    rm.onclick = () => removeUserMapping(idx);
    row.appendChild(label);
    row.appendChild(rm);
    list.appendChild(row);
  });
}
async function addLinkedUserMapping() {
  const a = ($('mapUserA') && $('mapUserA').value || '').trim();
  const b = ($('mapUserB') && $('mapUserB').value || '').trim();
  if (!a || !b) {
    showToast('Select a user on each server');
    return;
  }
  if (a.toLowerCase() === b.toLowerCase()) {
    showToast('Those names already match — no link needed');
    return;
  }
  if (!currentConfig.user_mappings) currentConfig.user_mappings = [];
  // Merge into existing group if either name already appears
  let merged = false;
  for (let i = 0; i < currentConfig.user_mappings.length; i++) {
    const g = currentConfig.user_mappings[i];
    const lower = g.map(x => x.toLowerCase());
    if (lower.includes(a.toLowerCase()) || lower.includes(b.toLowerCase())) {
      if (!lower.includes(a.toLowerCase())) g.push(a);
      if (!lower.includes(b.toLowerCase())) g.push(b);
      merged = true;
      break;
    }
  }
  if (!merged) currentConfig.user_mappings.push([a, b]);
  // Keep settings textarea in sync
  if ($('cfgUserMappings')) {
    $('cfgUserMappings').value = currentConfig.user_mappings.map(g => g.join(', ')).join('\n');
  }
  showToast('Linked ' + a + ' ↔ ' + b);
  await saveConfig();
  renderMapLinksList();
  setTimeout(loadDashboard, 600);
}
async function removeUserMapping(idx) {
  if (!currentConfig.user_mappings) return;
  currentConfig.user_mappings.splice(idx, 1);
  if ($('cfgUserMappings')) {
    $('cfgUserMappings').value = currentConfig.user_mappings.map(g => g.join(', ')).join('\n');
  }
  await saveConfig();
  renderMapLinksList();
  setTimeout(loadDashboard, 600);
}
/** Detect Emby vs Jellyfin via test_connection (tries both). Returns {ok, is_emby, url, message}. */
async function detectServerType(url, api_key, preferEmby) {
  const res = await authedFetch('/api/test_connection', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      url,
      api_key,
      // Hint only — server tries both orderings.
      is_emby: !!preferEmby
    })
  });
  const d = await res.json().catch(() => ({}));
  return {
    ok: d.status === 'ok',
    is_emby: !!d.is_emby,
    url: d.url || url,
    message: d.message || (res.ok ? 'Connected' : 'Connection failed')
  };
}
"#;
