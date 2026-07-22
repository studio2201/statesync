//! Action handlers and log feed rendering for the StateSync web dashboard.

/// Log feed and banner update script string slice (Part 2).
pub const JS_ACTIONS: &str = r#"    const logsDiv = $('syncLogs');
    if (logsDiv) {
      if (status.sync_logs && status.sync_logs.length > 0) {
        logsDiv.textContent = '';
        window._lastSyncLogs = status.sync_logs;
        status.sync_logs.forEach(log => {
          const line = document.createElement('div'); line.className = 'log-line';
          const color = log.level === 'error' ? 'var(--red)' : (log.level === 'warn' ? 'var(--accent)' : 'var(--text)');
          let main = '[' + log.timestamp + '] ' + log.level + ': ' + log.message;
          if (log.source_name && log.target_name) {
            main += '  (' + log.source_name + ' → ' + log.target_name + ')';
          }
          const mainEl = document.createElement('span');
          mainEl.style.color = color;
          mainEl.textContent = main;
          line.appendChild(mainEl);
          if (log.detail) {
            const d = document.createElement('span');
            d.className = 'log-detail';
            d.textContent = log.detail;
            line.appendChild(d);
          }
          logsDiv.appendChild(line);
        });
        // Keep scroll at bottom only if user was already near bottom
        logsDiv.scrollTop = logsDiv.scrollHeight;
      } else if (!window._lastSyncLogs) {
        logsDiv.textContent = '';
        const empty = document.createElement('div');
        empty.className = 'empty';
        empty.textContent = 'No activity yet. Play something or use Force sync.';
        logsDiv.appendChild(empty);
      }
    }
    const banner = $('lastFullSyncBanner');
    if (banner && status.last_full_sync) {
      const fs = status.last_full_sync;
      const st = forceStateKey(fs.state);
      banner.textContent = '';
      const left = document.createElement('span');
      if (fs.finished_at && (st === 'completed' || st === 'failed')) {
        const age = Date.now() - new Date(fs.finished_at).getTime();
        const ago = formatAgo(age);
        const statusColor = st === 'completed' ? 'var(--green)' : 'var(--red)';
        const label = st === 'completed' ? 'finished cleanly' : 'finished with errors';
        let story = 'Last force sync <span style="color:' + statusColor + '">' + label + '</span> ' + ago + '. ';
        story += 'Checked ' + (fs.processed || 0) + ' library titles, updated ' + (fs.succeeded || 0);
        if (fs.skipped > 0) story += ', no change needed on ' + fs.skipped;
        if (fs.failed > 0) story += ', failed ' + fs.failed;
        const bf = fs.by_field || {};
        if (bf.favorite && (bf.favorite.ok || bf.favorite.skip || bf.favorite.fail)) {
          story += ' · favorites updated ' + (bf.favorite.ok || 0);
        }
        const sr = fs.skip_reasons || {};
        if (sr.already_equal) story += ' · ' + sr.already_equal + ' already same (good)';
        if (sr.no_provider) story += ' · ' + sr.no_provider + ' could not pair (no catalog ID)';
        if (sr.no_match) story += ' · ' + sr.no_match + ' could not pair (not in other library)';
        if (fs.scope && fs.scope.length) story += ' · scope ' + fs.scope.join('/');
        story += '.';
        left.innerHTML = story;
      } else if (st === 'running' || (fs.started_at && !fs.finished_at)) {
        left.textContent = 'Force sync is running right now · ' + (fs.processed || 0) + ' items so far. Live play sync is paused until it finishes.';
      } else {
        left.textContent = 'No force sync yet. Use Force sync once to push historical watched state; live plays sync automatically after that.';
      }
      banner.appendChild(left);
    }
    const live = $('forceSyncLive');
    if (live) {
      const fs = status.last_full_sync;
      const st = fs ? forceStateKey(fs.state) : '';
      if (fs && (st === 'running' || (fs.started_at && !fs.finished_at))) {
        applyForceSyncLiveUi(fs);
      } else if (!window._forceSyncOptimistic) {
        live.style.display = 'none';
      }
    }
    const forceBtn = $('forceSyncBtn');
    const previewBtn = $('previewForceBtn');
    {
      const noServers = !currentConfig.servers || currentConfig.servers.length === 0;
      const fs = status.last_full_sync;
      const st = fs ? forceStateKey(fs.state) : '';
      const inProgress = !!(fs && (st === 'running' || (fs.started_at && !fs.finished_at))) || !!window._forceSyncOptimistic;
      if (forceBtn) forceBtn.disabled = noServers || inProgress;
      if (previewBtn) previewBtn.disabled = noServers || inProgress;
    }
    const footer = $('versionFooter');
    if (footer && status.version) {
      footer.textContent = '';
      const link = document.createElement('a');
      link.href = 'https://github.com/studio2201/statesync/releases/tag/v' + status.version;
      link.target = '_blank';
      link.rel = 'noopener noreferrer';
      link.textContent = 'v' + status.version;
      link.style.cssText = 'color: var(--accent); text-decoration: none; border-bottom: 1px dotted var(--accent);';
      footer.appendChild(link);
      footer.appendChild(document.createTextNode(' | uptime ' + Math.floor(status.uptime_seconds / 60) + 'm'));
    }
    window._lastStatus = status;
  } catch (err) { console.error(err); }
}
"#;
