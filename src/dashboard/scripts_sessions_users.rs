//! Now-playing and mapped users rendering.
//! Session rows are updated in place so posters are not torn down every poll.

pub const JS_SESSIONS_USERS: &str = r#"const activeDiv = $('activeSessions');
    const sessions = (status.active_sessions && status.active_sessions.length)
      ? status.active_sessions : [];
    if (sessions.length === 0) {
      if (activeDiv.dataset.mode !== 'empty') {
        activeDiv.textContent = '';
        const empty = document.createElement('div'); empty.className = 'empty';
        empty.textContent = 'No one is playing anything right now.';
        activeDiv.appendChild(empty);
        activeDiv.dataset.mode = 'empty';
      }
    } else {
      activeDiv.dataset.mode = 'list';
      // Drop empty-state placeholder if present.
      if (activeDiv.querySelector('.empty') && !activeDiv.querySelector('[data-sk]')) {
        activeDiv.textContent = '';
      }
      const seen = {};
      sessions.forEach(sess => {
        const key = String(sess.server || '') + '|' + String(sess.user || '');
        seen[key] = true;
        const mins = Math.floor(sess.position / 60);
        const secs = Math.floor(sess.position % 60).toString().padStart(2, '0');
        const durationStr = mins + ':' + secs;
        let row = activeDiv.querySelector('[data-sk="' + CSS.escape(key) + '"]');
        if (!row) {
          row = document.createElement('div');
          row.className = 'server-row';
          row.setAttribute('data-sk', key);
          const left = document.createElement('div'); left.className = 'server-info';
          left.setAttribute('data-left', '1');
          if (sess.poster_url) {
            const img = document.createElement('img');
            img.alt = sess.item || '';
            img.className = 'poster-thumb';
            img.loading = 'lazy';
            img.setAttribute('data-poster', '1');
            loadPoster(sess.poster_url, img);
            left.appendChild(img);
          } else {
            const ph = document.createElement('div');
            ph.className = 'poster-missing';
            left.appendChild(ph);
          }
          const meta = document.createElement('div'); meta.className = 'server-meta';
          const itemEl = document.createElement('div'); itemEl.className = 'name';
          itemEl.setAttribute('data-item', '1');
          itemEl.textContent = sess.item;
          const userEl = document.createElement('div'); userEl.className = 'url';
          userEl.setAttribute('data-userline', '1');
          meta.appendChild(itemEl); meta.appendChild(userEl);
          left.appendChild(meta);
          const right = document.createElement('div');
          right.style.cssText = 'display:flex;align-items:center;gap:8px';
          right.setAttribute('data-right', '1');
          const badge = document.createElement('span'); badge.className = 'badge';
          badge.setAttribute('data-pos', '1');
          right.appendChild(badge);
          row.appendChild(left); row.appendChild(right);
          activeDiv.appendChild(row);
        }
        // Update text only — never rebuild the <img> if poster URL is unchanged.
        const itemEl = row.querySelector('[data-item]');
        if (itemEl && itemEl.textContent !== sess.item) itemEl.textContent = sess.item;
        const userEl = row.querySelector('[data-userline]');
        if (userEl) {
          const line = sess.is_paused
            ? (sess.user + ' paused on ' + sess.server + ' at ' + durationStr)
            : (sess.user + ' watching on ' + sess.server);
          if (userEl.textContent !== line) userEl.textContent = line;
        }
        const badge = row.querySelector('[data-pos]');
        if (badge && badge.textContent !== durationStr) badge.textContent = durationStr;
        let pausedBadge = row.querySelector('[data-paused]');
        if (sess.is_paused) {
          if (!pausedBadge) {
            pausedBadge = document.createElement('span');
            pausedBadge.className = 'badge';
            pausedBadge.setAttribute('data-paused', '1');
            pausedBadge.textContent = 'Paused';
            const right = row.querySelector('[data-right]');
            if (right) right.appendChild(pausedBadge);
          }
        } else if (pausedBadge) {
          pausedBadge.remove();
        }
        const img = row.querySelector('img[data-poster]');
        if (img && sess.poster_url && img.dataset.posterUrl !== sess.poster_url
            && !_posterObjectUrls[sess.poster_url]) {
          // Title changed for this user — load the new Primary art once.
          loadPoster(sess.poster_url, img);
        } else if (img && sess.poster_url && _posterObjectUrls[sess.poster_url]
            && img.src !== _posterObjectUrls[sess.poster_url]) {
          img.src = _posterObjectUrls[sess.poster_url];
          img.dataset.posterUrl = sess.poster_url;
        } else if (img && sess.poster_url && img.dataset.posterUrl !== sess.poster_url) {
          loadPoster(sess.poster_url, img);
        }
        if (img && sess.item) img.alt = sess.item;
      });
      // Remove rows for users who stopped playing.
      Array.from(activeDiv.querySelectorAll('[data-sk]')).forEach(row => {
        const k = row.getAttribute('data-sk');
        if (k && !seen[k]) row.remove();
      });
    }
    const usersDiv = $('syncedUsers');
    if (!status.servers || status.servers.length === 0) {
      usersDiv.textContent = '';
      window._mappedUsersList = [];
      const empty = document.createElement('div'); empty.className = 'empty'; empty.textContent = 'Add two servers to map users.';
      usersDiv.appendChild(empty);
    } else {
      usersDiv.textContent = '';
      const serverCount = status.servers.length;
      const users = (status.users || []).slice().sort((a, b) =>
        a.name.localeCompare(b.name, undefined, { sensitivity: 'base', numeric: true })
      );
      const ignList = ((currentConfig.sync && currentConfig.sync.user_ignorelist) || [])
        .map(n => String(n).trim().toLowerCase());
      const isUserIgnored = (name) => {
        const key = String(name).trim().toLowerCase();
        if (ignList.indexOf(key) >= 0) return true;
        if (currentConfig.user_mappings) {
          for (let gi = 0; gi < currentConfig.user_mappings.length; gi++) {
            const members = (currentConfig.user_mappings[gi] || []).map(n => String(n).trim().toLowerCase()).filter(Boolean);
            if (members.indexOf(key) >= 0 && members.some(m => ignList.indexOf(m) >= 0)) return true;
          }
        }
        return false;
      };
      window._mappedUsersList = users.map(u => ({ name: u.name, ignored: isUserIgnored(u.name) }));
      // Server columns only — actions live under header Actions modal.
      const grid = document.createElement('div');
      grid.style.cssText = 'display:grid;grid-template-columns:repeat(' + serverCount + ', 1fr);gap:6px;align-items:center';
      const headerRow2 = document.createElement('div');
      headerRow2.style.cssText = 'display:contents';
      status.servers.forEach((srv, si) => {
        const h = document.createElement('div');
        h.style.cssText = 'text-align:center;color:var(--muted);font-weight:600;font-size:11px;padding-bottom:6px;border-bottom:1px solid var(--border);line-height:1.35';
        const cfg = (currentConfig.servers || [])[si] || {};
        const hostPort = srv.host_port || nameFromUrl(cfg.url || srv.url || '') || '';
        const niceName = String(srv.name || cfg.name || '').trim();
        // Prefer distinct address (host:port). Show config name only when it adds info.
        // Do not label Emby/Jellyfin here — type is auto-detected for APIs and often wrong in UI.
        let title = hostPort || niceName || ('Server ' + (si + 1));
        if (niceName && hostPort && niceName !== hostPort && niceName.toLowerCase() !== hostPort.toLowerCase()) {
          title = niceName + '\n' + hostPort;
        }
        h.textContent = title;
        h.title = srv.display_label || title.replace(/\n/g, ' · ');
        headerRow2.appendChild(h);
      });
      grid.appendChild(headerRow2);
      users.forEach(u => {
        const ignored = isUserIgnored(u.name);
        const selected = window._selectedMappedUser === u.name;
        for (let i = 0; i < serverCount; i++) {
          const cell = document.createElement('div');
          const filled = u.servers.includes(i);
          cell.className = 'user-cell' + (filled ? ' filled' : ' empty') + (selected ? ' selected' : '');
          cell.style.cursor = 'pointer';
          const srv = status.servers[i] || {};
          const srvLabel = srv.display_label || srv.host_port || srv.name || ('server ' + (i + 1));
          if (filled) {
            cell.textContent = ignored ? (u.name + ' · ignored') : u.name;
            cell.title = (ignored ? 'Ignored · ' : '') + srvLabel + ' · Click to select · Actions in header';
          } else {
            cell.textContent = '·';
            cell.title = 'No linked user on ' + srvLabel + ' — use Link users';
          }
          cell.addEventListener('click', () => {
            window._selectedMappedUser = u.name;
            loadDashboard();
          });
          grid.appendChild(cell);
        }
      });
      usersDiv.appendChild(grid);
      const mappedCount = users.filter(u => u.servers.length > 1).length;
      const singleCount = users.length - mappedCount;
      const legend = document.createElement('div');
      legend.className = 'form-hint';
      legend.style.cssText = 'margin-top:12px';
      legend.textContent = users.length + ' rows · ' + mappedCount + ' linked · ' + singleCount + ' need a link'
        + (window._selectedMappedUser ? (' · selected: ' + window._selectedMappedUser) : ' · click a name, then Actions');
      usersDiv.appendChild(legend);
    }
"#;
