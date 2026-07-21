# ![StateSync](graphics/statesync_icon.jpg) StateSync

Real-time, bi-directional watch-state sync between Emby and Jellyfin.

When a user pauses, resumes, or finishes a show on one server, the same position is written to the other.

![Dashboard](graphics/bgL4h.jpg)

## Install — Unraid

1. **Docker tab** → **Template Repositories** → add `https://github.com/studio2201/statesync` (or import `statesync.xml` / `unraid/unraid-template.xml` from this repo)
2. **Add Container** → pick **statesync**
3. Confirm **Network Type = bridge**, **Web UI Port = 4601**, **Config** = `/mnt/user/appdata/statesync`, then **Apply**
4. Open `http://<your-unraid-ip>:4601` — no login required
5. **Add server** → use Emby/Jellyfin **LAN IPs** (not `localhost`) + API key → **Save**

Config persists at `/mnt/user/appdata/statesync/config.json`. Defaults `PUID=99` / `PGID=100` so appdata shows as `nobody` in the Unraid file manager.

**Note:** host networking binds `0.0.0.0:4601`. The daemon **requires** dashboard auth on non-loopback binds (open LAN UI without a token is refused). Use a published image that includes this behavior, or build/load a local image from this tree before testing.

## Install — Docker Compose

```yaml
# compose.yaml
services:
  statesync:
    image: ghcr.io/studio2201/statesync:latest
    container_name: statesync
    restart: unless-stopped
    ports:
      - "4601:4601"
    volumes:
      - ./config:/config
    environment:
      - RUST_LOG=info
      - TZ=UTC
      # Optional PUID/PGID/UMASK if you want a non-default user:
      # - PUID=99
      # - PGID=100
      # - UMASK=022
```

```bash
mkdir -p config && docker compose up -d
# open http://localhost:4601
```

The first run creates a default `config.json` if none exists — open the web UI and add servers.

## Config

`config.json` lives at `/config/config.json` inside the container (your bind-mount target).

```json
{
  "servers": [
    {
      "name": "emby",
      "url": "https://emby.example.com:8096",
      "api_key": "your-emby-api-key",
      "is_emby": true,
      "sync_direction": "both",
      "allow_insecure_http": true
    },
    {
      "name": "jellyfin",
      "url": "https://jellyfin.example.com:8096",
      "api_key": "your-jellyfin-api-key",
      "is_emby": false,
      "sync_direction": "both",
      "allow_insecure_http": true
    }
  ],
  "sync_threshold_seconds": 5,
  "user_mappings": [
    ["john doe", "john"],
    ["jane", "jane_doe"]
  ]
}
```

| Field | What |
|---|---|
| `name` | Friendly label shown in the dashboard |
| `url` | Server base URL (no trailing slash). Plain `http://` works on most home LANs. |
| `api_key` | API key from the server's admin UI |
| `is_emby` | `true` for Emby, `false` for Jellyfin |
| `sync_direction` | `both` (default), `send` (emit only), or `receive` (accept only) |
| `allow_insecure_http` | Default `true`; set `false` to require `https://` upstream |
| `sync_threshold_seconds` | Skip redundant updates within this window (default 5) |
| `user_mappings` | Map user names across servers, one group per line in the UI |

You can also configure everything in the web UI — changes save to this file.

## Environment variables

| Variable | Default | What |
|---|---|---|
| `STATESYNC_BIND` | `0.0.0.0:4601` | Listen address |
| `STATESYNC_SYNC_THRESHOLD_SECONDS` | `5` | Skip redundant updates (progress and played) within this window |
| `STATESYNC_ALLOW_INSECURE_HTTP` | `true` | Permits plain `http://` URLs to upstream Emby/Jellyfin servers (LAN-friendly default). Plain HTTP means the API key travels unencrypted between containers — fine on a home LAN, not fine if your media servers are exposed beyond it (e.g. behind a reverse proxy with TLS). Set `false` to require `https://`. |
| `STATESYNC_ACCEPT_INVALID_CERTS` | `false` | Set `true` only for intentionally trusted self-signed HTTPS upstreams (disables TLS cert verification). |
| `STATESYNC_FUZZY_USER_MATCH` | `false` | Set `true` to allow substring username matching. Prefer explicit `user_mappings` instead. |
| `STATESYNC_HTTP_RETRY` | `on` | Set `off` to disable HTTP retry on transient errors |
| `STATESYNC_LOG_RETENTION` | `30` | Number of log entries kept in memory |
| `STATESYNC_FORCE_RATE` | `5` | Items/sec during force-sync, `1..50` |
| `PUID` | `99` | Process uid inside the container (Unraid community-app convention; matches the `nobody` user on Unraid hosts) |
| `PGID` | `100` | Process gid |
| `UMASK` | `022` | File-creation umask inside the container |
| `RUST_LOG` | `info` | tracing-subscriber filter |
| `TZ` | `UTC` | Container timezone |

## CLI

```bash
statesync --validate       # load config, test connections, exit 0/1
statesync --reload         # POST /api/reload to the running daemon
statesync --tui            # interactive terminal dashboard (1s poll)
statesync --dry-run        # init caches, report mapping coverage
statesync --version
statesync --help
```

The TUI shows live server status, active streams, and recent sync events. Same data as the web UI, in your terminal.

## Dashboard

The header has four buttons: **[REFRESH USERS]** | **[FORCE SYNC]** | **[SETTINGS]** | **[+ ADD MODULE]**

- **REFRESH USERS** — re-fetches `/Users` on each configured server and merges the result into the in-memory cache (existing entries preserved; transient hiccups don't drop users).
- **FORCE SYNC** — runs `POST /api/sync/force`: iterates every user × every server × every played item, resolves target by IMDb/TMDb, pushes the source state to target (source-wins merge). Rate-limited to 5 items/sec by default. Live WebSocket sync is paused for the duration. Same as the CLI flag.
- **SETTINGS** — opens a modal with the global `sync_threshold_seconds` and `user_mappings` settings.
- **+ ADD MODULE** — opens the add/edit modal. Form has:
  - Module type: JELLYFIN (purple) or EMBY (green) — click to toggle, the other is greyed
  - SERVER ADDRESS + **↻ AUTO** button: fetches `/System/Info/Public` on the target and pre-fills DISPLAY NAME
  - DISPLAY NAME
  - ACCESS KEY (API) — masked password input
  - SYNC DIRECTION: three buttons (BIDIRECTIONAL / SEND ONLY / RECEIVE ONLY)

The MAPPED USERS card shows every user from every server as a grid:

```
                [EMBY]      [JELLYFIN]
                @alice  ────  @alice
                  @bob
                                @carol
                 @dave  ─────  @dave

       4 users total · 2 mapped across servers · 2 single-server
```

- One row per user (alphabetical).
- `@username` is a user; `·` is "this server has no user here". Tooltip says "user: alice" or "server: emby (no user here)" explicitly so a user named "green" doesn't get confused with a server named "green".
- The legend makes it obvious which users need a manual mapping (Settings → user_mappings).

## Health endpoint

```
GET /healthz   → 200 OK | 503 Service Unavailable
```

Unauthenticated. Returns JSON with version, uptime, server count, and connected count. Use this for container health checks, uptime monitoring, etc.

## Container user

The container is set up with the standard Unraid `PUID=99` / `PGID=100` / `UMASK=022` variables honored. The entrypoint:

1. Reads `PUID` / `PGID` / `UMASK` (defaults to 99 / 100 / 022)
2. Applies the umask
3. Chowns `/config` and `/app` to `${PUID}:${PGID}`
4. Execs the daemon as `${PUID}:${PGID}` via `su-exec`

This means the appdata dir shows as `nobody` in the Unraid file manager, matching the convention from `binhex-syncthing`, `glances`, `ollama`, etc. If you see files owned by `65534` instead of `nobody` in some other view, that's the same uid, just shown numerically — the view is consulting the host's `/etc/passwd`.

## Force sync

The dashboard has a **FORCE SYNC** button (next to the MAPPED USERS header) and a CLI:

```bash
statesync --sync-force [--direction=emby-to-jellyfin|jellyfin-to-emby|both]
```

Iterates every user on every source server, reads their played items (paginated), resolves the target on the other server, and pushes the source state (source-wins merge). Rate-limited to 5 items/sec by default (`STATESYNC_FORCE_RATE` env var, `1..50`). Live WebSocket sync is paused for the duration to avoid two-writer races on `last_syncs`. Hard cap 100k items per run.

Useful for initial reconciliation after the daemon has been running a while and you want to push all historical played state across.

## Security

- **API keys**: stored in `config.json` only. Returned masked by `GET /api/config` (first 4 + last 4 chars).
- **Upstream HTTPS**: by default StateSync talks plain `http://` to your Emby/Jellyfin (LAN convention). Set `STATESYNC_ALLOW_INSECURE_HTTP=false` if your media servers are exposed beyond the LAN (e.g. behind a reverse proxy with TLS).
- **Dashboard**: open on the LAN with no sign-in. For internet exposure, put it behind a reverse proxy (Caddy / Traefik / nginx) with its own auth/TLS.

## How it works

For each server pair, the daemon opens a WebSocket to the source server and listens for `Sessions` and `UserDataChanged` events. When a user's playback position or `Played` flag changes, it resolves the matching item on the target server (by IMDb / TMDb ID), maps the user, and POSTs the update. Items never synced are resolved lazily; a small per-(user, item) throttle skips redundant updates within the threshold window.

Forward-only by default — historical watch state isn't backfilled. Use the dashboard's **FORCE SYNC** button to push all historical played state across after install.

## License

MIT
