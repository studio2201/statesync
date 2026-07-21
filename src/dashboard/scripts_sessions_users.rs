//! Now-playing and mapped users rendering.

pub const JS_SESSIONS_USERS: &str = r#"const activeDiv = $('activeSessions');
    if (status.active_sessions && status.active_sessions.length > 0) {
      activeDiv.textContent = '';
      status.active_sessions.forEach(sess => {
        const mins = Math.floor(sess.position / 60); const secs = Math.floor(sess.position % 60).toString().padStart(2, '0');
        const durationStr = mins + ':' + secs;
        const row = document.createElement('div'); row.className = 'server-row';
        const left = document.createElement('div'); left.className = 'server-info';
        if (sess.poster_url) {
          const img = document.createElement('img');
          img.alt = sess.item || '';
          img.className = 'poster-thumb';
          img.loading = 'lazy';
          loadPoster(sess.poster_url, img);
          left.appendChild(img);
        } else {
          const ph = document.createElement('div');
          ph.className = 'poster-missing';
          left.appendChild(ph);
        }
        const meta = document.createElement('div'); meta.className = 'server-meta';
        const itemEl = document.createElement('div'); itemEl.className = 'name'; itemEl.textContent = sess.item;
        const userEl = document.createElement('div'); userEl.className = 'url';
        userEl.textContent = sess.is_paused
          ? (sess.user + ' paused on ' + sess.server + ' at ' + durationStr)
          : (sess.user + ' watching on ' + sess.server);
        meta.appendChild(itemEl); meta.appendChild(userEl);
        left.appendChild(meta);
        const right = document.createElement('div'); right.style.cssText = 'display:flex;align-items:center;gap:8px';
        const badge = document.createElement('span'); badge.className = 'badge'; badge.textContent = durationStr;
        right.appendChild(badge);
        if (sess.is_paused) {
          const p = document.createElement('span'); p.className = 'badge'; p.textContent = 'Paused';
          right.appendChild(p);
        }
        row.appendChild(left); row.appendChild(right);
        activeDiv.appendChild(row);
      });
    } else {
      activeDiv.textContent = '';
      const empty = document.createElement('div'); empty.className = 'empty'; empty.textContent = 'No one is playing anything right now.';
      activeDiv.appendChild(empty);
    }
    const usersDiv = $('syncedUsers');
    if (!status.servers || status.servers.length === 0) {
      usersDiv.textContent = '';
      const empty = document.createElement('div'); empty.className = 'empty'; empty.textContent = 'Add two servers to map users.';
      usersDiv.appendChild(empty);
    } else {
      usersDiv.textContent = '';
      const serverCount = status.servers.length;
      const users = (status.users || []).slice().sort((a, b) =>
        a.name.localeCompare(b.name, undefined, { sensitivity: 'base', numeric: true })
      );
      const grid = document.createElement('div');
      grid.style.cssText = 'display:grid;grid-template-columns:repeat(' + serverCount + ', 1fr) auto;gap:6px;align-items:center';
      const headerRow2 = document.createElement('div');
      headerRow2.style.cssText = 'display:grid;grid-template-columns:repeat(' + serverCount + ', 1fr) auto;gap:6px;margin-bottom:6px';
      status.servers.forEach(srv => {
        const h = document.createElement('div');
        h.style.cssText = 'text-align:center;color:var(--muted);font-weight:600;font-size:11px;padding-bottom:6px;border-bottom:1px solid var(--border);text-transform:uppercase';
        h.textContent = srv.name;
        headerRow2.appendChild(h);
      });
      const hAct = document.createElement('div');
      hAct.style.cssText = 'text-align:center;color:var(--muted);font-weight:600;font-size:11px;padding-bottom:6px;border-bottom:1px solid var(--border);text-transform:uppercase';
      hAct.textContent = 'Actions';
      headerRow2.appendChild(hAct);
      usersDiv.appendChild(headerRow2);
      users.forEach(u => {
        const row = document.createElement('div');
        row.style.cssText = 'display:contents';
        for (let i = 0; i < serverCount; i++) {
          const cell = document.createElement('div');
          const filled = u.servers.includes(i);
          cell.className = 'user-cell' + (filled ? ' filled' : ' empty');
          if (filled) {
            cell.textContent = u.name;
            cell.title = u.servers.length > 1
              ? u.name + ' is linked across servers'
              : u.name + ' only on ' + status.servers[i].name + ' — use Link users';
          } else {
            cell.textContent = '·';
            cell.title = 'No linked user on ' + status.servers[i].name;
          }
          row.appendChild(cell);
        }
        const act = document.createElement('div');
        act.style.cssText = 'display:flex;justify-content:flex-end';
        const clr = document.createElement('button');
        clr.className = 'btn btn-danger';
        clr.style.cssText = 'font-size:11px;padding:4px 8px';
        clr.textContent = 'Clear watched';
        clr.title = 'Mark all watched items unwatched for this person on every server';
        clr.addEventListener('click', () => clearWatchedForUser(u.name));
        act.appendChild(clr);
        row.appendChild(act);
        grid.appendChild(row);
      });
      usersDiv.appendChild(grid);
      const mappedCount = users.filter(u => u.servers.length > 1).length;
      const singleCount = users.length - mappedCount;
      const legend = document.createElement('div');
      legend.className = 'form-hint';
      legend.style.cssText = 'margin-top:12px;display:flex;gap:16px;flex-wrap:wrap;align-items:center';
      legend.textContent = users.length + ' rows · ' + mappedCount + ' linked · ' + singleCount + ' need a link';
      if (singleCount > 0) {
        const tip = document.createElement('button');
        tip.className = 'btn';
        tip.style.marginLeft = 'auto';
        tip.textContent = 'Link users';
        tip.onclick = openMapUsersModal;
        legend.appendChild(tip);
      }
      usersDiv.appendChild(legend);
    }
"#;
