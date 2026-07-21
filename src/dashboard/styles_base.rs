//! Base dashboard theme and layout CSS.
pub const CSS_BASE: &str = r#":root {
  --bg: #0b0f14;
  --card: #121820;
  --border: #2a3544;
  --text: #9aa8b8;
  --bright: #e8eef5;
  --accent: #3b9eff;
  --green: #3dd68c;
  --red: #f07178;
  --muted: #5a6a7a;
}
* { box-sizing: border-box; margin: 0; padding: 0; }
body {
  background: var(--bg);
  color: var(--text);
  color-scheme: dark;
  font-family: ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial, sans-serif;
  font-size: 14px;
  line-height: 1.45;
  padding: 24px;
}
.container { max-width: 1100px; margin: 0 auto; }
.header {
  display: flex;
  flex-wrap: wrap;
  gap: 12px 16px;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 20px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--border);
}
.brand { display: flex; align-items: center; gap: 10px; color: var(--bright); font-weight: 600; font-size: 18px; }
.brand img { width: 32px; height: 32px; border-radius: 6px; }
.actions { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
.card {
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 16px 18px;
  margin-bottom: 16px;
}
.card h2 {
  color: var(--bright);
  font-size: 13px;
  font-weight: 600;
  letter-spacing: 0.03em;
  text-transform: uppercase;
  margin-bottom: 12px;
}
.row-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
  align-items: start;
}
.stack { display: flex; flex-direction: column; gap: 16px; }
.server-row {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  align-items: center;
  gap: 10px;
  padding: 12px 14px;
  background: rgba(0,0,0,0.25);
  border: 1px solid var(--border);
  border-radius: 6px;
  margin-bottom: 8px;
}
.server-info { display: flex; gap: 12px; align-items: center; min-width: 0; flex: 1; }
.server-meta { min-width: 0; }
.server-meta .name { color: var(--bright); font-weight: 600; word-break: break-word; }
.server-meta .url { font-size: 12px; color: var(--muted); margin-top: 2px; word-break: break-all; }
.badge {
  display: inline-block;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.04em;
  padding: 2px 7px;
  border-radius: 999px;
  border: 1px solid var(--border);
  color: var(--text);
  background: rgba(255,255,255,0.03);
  white-space: nowrap;
}
.btn {
  background: transparent;
  border: 1px solid var(--border);
  color: var(--bright);
  padding: 7px 12px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 12px;
  font-weight: 500;
}
.btn:hover { border-color: var(--accent); color: var(--accent); }
.btn:disabled { opacity: 0.45; cursor: not-allowed; }
.btn-primary { background: var(--accent); border-color: var(--accent); color: #041018; }
.btn-primary:hover { filter: brightness(1.08); color: #041018; }
.btn-danger { border-color: var(--red); color: var(--red); }
.btn-danger:hover { background: var(--red); color: #fff; }
.btn-group { display: flex; flex-wrap: wrap; gap: 6px; }
.btn-radio {
  background: transparent;
  border: 1px solid var(--border);
  color: var(--text);
  padding: 6px 10px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 12px;
}
.btn-radio.active { background: var(--accent); border-color: var(--accent); color: #041018; font-weight: 600; }
.user-cell {
  padding: 8px 10px;
  font-size: 12px;
  text-align: center;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: rgba(0,0,0,0.2);
}
.user-cell.filled { color: var(--bright); border-color: #3a4a5c; }
.user-cell.empty { color: var(--muted); opacity: 0.7; }
.log-feed {
  background: #070a0e;
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 10px 12px;
  font-size: 12px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  height: 240px;
  overflow-y: auto;
  color: var(--text);
  user-select: text;
  -webkit-user-select: text;
  cursor: text;
}
.log-line { margin-bottom: 8px; word-break: break-word; white-space: pre-wrap; }
.log-line .log-detail {
  display: block;
  margin-top: 2px;
  margin-left: 0;
  font-size: 11px;
  color: var(--muted);
  user-select: text;
}
.map-links { display: flex; flex-direction: column; gap: 6px; max-height: 200px; overflow-y: auto; }
.map-link-row {
  display: flex; justify-content: space-between; align-items: center; gap: 8px;
  padding: 8px 10px; border: 1px solid var(--border); border-radius: 6px; background: rgba(0,0,0,0.25);
  font-size: 12px; color: var(--bright);
}
select {
  width: 100%; background: #070a0e; border: 1px solid var(--border);
  color: var(--bright); padding: 9px 10px; border-radius: 6px; font-size: 13px;
}
/* Machine status codes (backend) + first-principles display classes */
.status-Connected, .status-Synchronizing, .status-live { color: var(--green); font-weight: 600; font-size: 12px; }
.status-Error, .status-failed { color: var(--red); font-weight: 600; font-size: 12px; }
.status-Offline, .status-Reconnecting, .status-Validating, .status-Scanning, .status-Connecting,
.status-pending {
  color: var(--muted); font-weight: 600; font-size: 12px;
}
.poster-thumb {
  width: 30px; height: 45px; object-fit: cover; border-radius: 4px;
  border: 1px solid var(--border); flex-shrink: 0; background: rgba(0,0,0,0.35);
}
.poster-missing {
  width: 30px; height: 45px; border-radius: 4px; border: 1px dashed var(--border);
  flex-shrink: 0; background: rgba(0,0,0,0.2);
}
.banner-live {
  border-color: var(--accent) !important;
  background: rgba(59, 158, 255, 0.08) !important;
}
"#;
