# ![StateSync](graphics/statesync_icon.jpg) StateSync

Real-time, bi-directional watch-state sync between Emby and Jellyfin.

When a user pauses, resumes, or finishes a show on one server, the same position is written to the other.

![Dashboard](graphics/bgL4h.jpg)

## Install — Unraid

1. **Docker tab** → **Template Repositories** → add `https://github.com/UberMetroid/statesync`
2. **Add Container** → pick **statesync** → click **Apply**
3. Open `http://<your-unraid-ip>:8754` in a browser
4. Click **+ ADD MODULE** and configure your Emby / Jellyfin servers (URL + API key)

Config persists at `/mnt/user/appdata/statesync/config.json` (or whatever path you mapped).

## Install — Docker Compose

```yaml
# compose.yaml
services:
  statesync:
    image: ghcr.io/ubermetroid/statesync:latest
    container_name: statesync
    restart: unless-stopped
    ports:
      - "8754:8754"
    volumes:
      - ./config:/config
    environment:
      - RUST_LOG=info
      - TZ=UTC
```

```bash
mkdir -p config && docker compose up -d
# open http://localhost:8754
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
      "sync_direction": "both"
    },
    {
      "name": "jellyfin",
      "url": "https://jellyfin.example.com:8096",
      "api_key": "your-jellyfin-api-key",
      "is_emby": false,
      "sync_direction": "both"
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
| `url` | Server base URL (no trailing slash) |
| `api_key` | API key from the server's admin UI |
| `is_emby` | `true` for Emby, `false` for Jellyfin |
| `sync_direction` | `both` (default), `send` (emit only), or `receive` (accept only) |
| `sync_threshold_seconds` | Skip redundant updates within this window (default 5) |
| `user_mappings` | Map user names across servers, one group per line in the UI |

You can also configure everything in the web UI — changes save to this file.

## Environment variables

| Variable | Default | What |
|---|---|---|
| `STATESYNC_BIND` | `0.0.0.0:8754` | Listen address |
| `STATESYNC_WEB_AUTH` | _(unset)_ | Optional. `bearer:<token>` to require auth on `/api/*`. Generate with `openssl rand -hex 32` |
| `STATESYNC_SERVER_<N>_URL` | — | Per-server env-var config (alternative to config.json) |
| `STATESYNC_SERVER_<N>_NAME` | — | |
| `STATESYNC_SERVER_<N>_API_KEY` | — | |
| `STATESYNC_SERVER_<N>_TYPE` | — | `emby` or `jellyfin` |
| `STATESYNC_SERVER_<N>_DIRECTION` | — | `both`, `send`, or `receive` |
| `STATESYNC_SYNC_THRESHOLD_SECONDS` | `5` | |
| `STATESYNC_ALLOW_INSECURE_HTTP` | `true` | Permits plain `http://` URLs to upstream Emby/Jellyfin servers (LAN-friendly default). StateSync calls your media servers to read user lists and push play-state updates. Plain HTTP means the API key travels unencrypted between containers — fine on a home LAN, not fine if your media servers are exposed beyond it (e.g. behind a reverse proxy with TLS). Set `false` to require `https://`. |
| `STATESYNC_HTTP_RETRY` | `on` | Set `off` to disable HTTP retry on transient errors |
| `STATESYNC_LOG_RETENTION` | `30` | Number of log entries kept in memory |
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

The TUI shows live server status, active sessions, and recent sync events. Same data as the web UI, in your terminal.

## Health endpoint

```
GET /healthz   → 200 OK | 503 Service Unavailable
```

Unauthenticated. Returns JSON with version, uptime, server count, and connected count. Use this for container health checks, uptime monitoring, etc.

## Container user

The daemon runs as the system `nobody` user (uid 65534), which is Unraid's appdata convention. The entrypoint chowns `/config` to `nobody:nogroup` on every start so the daemon can write to it without permission errors.

If you see files owned by `65534` instead of `nobody` in some view (e.g. Unraid's file manager or via SSH on the host), that's because that view is consulting the host's `/etc/passwd` — same uid, same user, just shown numerically. The container's `/etc/passwd` has `nobody:x:65534:65534:nobody:/:/sbin/nologin`, so `ls -l` inside the container shows `nobody`.

## Force sync

The dashboard has a **FORCE SYNC** button (next to the MAPPED USERS header) and a CLI:

```bash
statesync --sync-force [--direction=emby-to-jellyfin|jellyfin-to-emby|both]
```

Iterates every user on every source server, reads their played items, resolves the target on the other server, and pushes the source state (source-wins merge). Rate-limited to 5 items/sec by default (`STATESYNC_FORCE_RATE` env var, 1..50). Live WebSocket sync is paused for the duration to avoid two-writer races on `last_syncs`.

Useful for initial reconciliation after the daemon has been running a while and you want to push all historical played state across.

## Security

- **API keys**: stored in `config.json` only. Returned masked by `GET /api/config` (first 4 + last 4 chars).
- **HTTPS upstream**: required by default. Set `STATESYNC_ALLOW_INSECURE_HTTP=true` for LAN testing only.
- **Loopback / non-loopback**: by default the daemon listens on `0.0.0.0:8754` so the web UI is reachable on the LAN. Set `STATESYNC_WEB_AUTH=bearer:<token>` to require a token. For internet exposure, put the daemon behind a reverse proxy (Caddy / Traefik / nginx) that handles TLS.

## How it works

For each server pair, the daemon opens a WebSocket to the source server and listens for `Sessions` and `UserDataChanged` events. When a user's playback position or `Played` flag changes, it resolves the matching item on the target server (by IMDb / TMDb ID), maps the user, and POSTs the update. Items never synced are resolved lazily; a small per-(user, item) throttle skips redundant updates within the threshold window.

Forward-only — historical watch state isn't backfilled. If you have a big library and want one-time reconciliation, run your own offline import script (this project doesn't ship one).

## License

MIT