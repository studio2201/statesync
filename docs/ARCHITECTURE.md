# StateSync architecture (secondary)

StateSync is a LAN sidecar: HTTP API + dashboard + WebSocket clients to Emby/Jellyfin.

## Pipeline

1. **Connect** — REST + live event stream per server (`is_emby` from ProductName / probe).  
2. **Identity** — users by name or `user_mappings`; items by IMDb/TMDb.  
3. **Live** — sessions + `UserDataChanged` → partial UserData writes (played / position / favorite).  
4. **Force** — page played/favorites history → skip-if-equal → write (or dry-run).  
5. **Clear watched** — per-user wipe of played flags on every server (dedicated API).

## Modules (`src/`)

| Area | Role |
|------|------|
| `client/` | MediaClient (HTTP, UserData, lists) |
| `config/` | servers, SyncOptions, allowlist |
| `sync/` | live progress + favorites |
| `sync_force/` | historical backfill |
| `websocket/` | event loops |
| `web` / `web_api/` | dashboard + JSON API |
| `dashboard/` | maud HTML + embedded JS/CSS |
| `cli/` | validate, force, tui |

## Invariants

- No file moves; UserData only.  
- Partial UserData POSTs so favorites don’t clobber progress.  
- Force pauses live sync via `force_sync_in_progress`.  
- Empty `user_allowlist` = all users.

## Env (common)

| Variable | Default |
|----------|---------|
| `STATESYNC_BIND` | `0.0.0.0:4601` |
| `STATESYNC_SYNC_THRESHOLD_SECONDS` | `5` |
| `STATESYNC_FORCE_RATE` | `5` |
| `STATESYNC_LOG_RETENTION` | `100` |
