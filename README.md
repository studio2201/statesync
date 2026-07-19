# ![StateSync](graphics/statesync_icon.jpg) StateSync

StateSync synchronizes watch states and playback progress bi-directionally in real-time between Emby and Jellyfin servers.

---

## Screenshot

![StateSync Dashboard](graphics/bgL4h.jpg)

---

## Features

- **Real-Time Sync**: Bi-directional playback position, play state, and session tracking.
- **Media Matching**: Matches items using metadata identifiers (IMDb and TMDb IDs).
- **User Mapping**: Automatically pairs users by case-insensitive name, with override groups in settings.
- **Loop Prevention**: Tracks and caches synced timestamps to block infinite updates.
- **Resilient Sockets**: Concurrently streams WebSocket events and auto-reconnects on network drops.
- **Zero-Dependency Agent**: Integrates using standard HTTP and WebSockets; no server plugins required.

---

## Security

StateSync is a security-first update starting with v0.18.

### Defaults (safe by default)

- **Web UI binds to `127.0.0.1:8754` (loopback only).** No external access without explicit configuration.
- **HTTP (`http://`) URLs to upstream Emby/Jellyfin servers are rejected.** Set `allow_insecure_http: true` per-server or `STATESYNC_ALLOW_INSECURE_HTTP=true` env var to override (e.g. for testing on a trusted LAN). HTTPS is strongly recommended in production.
- **API keys are never logged** and are masked when returned by `GET /api/config`.
- **`config.json` is gitignored** so secrets never end up in version control. Use `config.example.json` as a template.
- **External exposure requires authentication.** If you set `STATESYNC_BIND=0.0.0.0:8754` you MUST also set `STATESYNC_WEB_AUTH=bearer:<token>` or the daemon will refuse to start.

### Exposing the UI beyond loopback

1. Generate a token:
   ```bash
   openssl rand -hex 32
   ```
2. Set `STATESYNC_BIND=0.0.0.0:8754` and `STATESYNC_WEB_AUTH=bearer:<that-token>`.
3. Open `http://your-host:8754/` and paste the token when prompted. The browser stores it in `localStorage`.

For internet-facing deployments, put StateSync behind a reverse proxy (Caddy, nginx, Traefik) that terminates TLS.

### Rotate exposed keys

If you previously committed a `config.json` containing API keys, **rotate the keys in each server's admin UI immediately** — assume they are public. Then scrub local history with `git-filter-repo --invert-paths --path config.json`.

---

## Configuration

StateSync reads its configuration from any of (in priority order):

1. Per-server environment variables: `STATESYNC_SERVER_0_URL`, `STATESYNC_SERVER_0_NAME`, `STATESYNC_SERVER_0_API_KEY`, `STATESYNC_SERVER_0_TYPE` (`emby`|`jellyfin`), `STATESYNC_SERVER_0_DIRECTION` (`both`|`send`|`receive`), `STATESYNC_SERVER_0_INSECURE` (`true` to permit http://). Indices 0..19 are checked.
2. Legacy two-server environment variables: `STATESYNC_EMBY_*`, `STATESYNC_JELLYFIN_*`.
3. `config.json` — searched in `/config/config.json`, `/etc/statesync/config.json`, `/app/config.json`, then `./config.json`.

See `config.example.json` for the full schema. Validate a config without running the daemon with `statesync --validate`.

### Environment variables

| Variable | Default | Purpose |
|---|---|---|
| `STATESYNC_BIND` | `127.0.0.1:8754` | Listen address. Loopback-only unless `STATESYNC_WEB_AUTH` is also set. |
| `STATESYNC_WEB_AUTH` | _(unset)_ | `bearer:<token>` required for non-loopback binds. |
| `STATESYNC_ALLOW_INSECURE_HTTP` | _(unset)_ | `true` to permit `http://` URLs to upstream servers. |
| `RUST_LOG` | `info` | tracing-subscriber filter. |
| `TZ` | `UTC` | Container timezone for log timestamps. |

---

## Deployment

StateSync runs as a static, zero-dependency container built on Alpine 3.20.

### 1. Docker Compose

Create a `docker-compose.yml` file:

```yaml
version: '3.8'
services:
  statesync:
    image: ghcr.io/ubermetroid/statesync:latest
    container_name: statesync
    restart: unless-stopped
    # Loopback-only by default. See "Security" above to expose beyond loopback.
    ports:
      - "127.0.0.1:8754:8754"
    volumes:
      - ./config:/config
    environment:
      - RUST_LOG=info
      - TZ=UTC
```

Place your real `config.json` in `./config/` (copy from `config.example.json` and fill in API keys). It is **not** committed.

Run the service:

```bash
docker compose up -d
```

### 2. Docker CLI

```bash
docker run -d \
  --name statesync \
  -p 127.0.0.1:8754:8754 \
  -v /path/to/config:/config \
  -e RUST_LOG=info \
  -e TZ=UTC \
  ghcr.io/ubermetroid/statesync:latest
```

---

## Unraid Setup

To install StateSync on Unraid:

### 1. Add the Template Repository

1. Navigate to the **Docker** tab in the Unraid WebUI.
2. Scroll to the bottom of the page and locate the **Template Repositories** field.
3. Paste the following URL:

   ```
   https://github.com/UberMetroid/statesync
   ```
4. Click **Save**.

### 2. Configure and Install Container

1. Click **Add Container** on the Docker page.
2. In the **Template** dropdown, select **statesync**.
3. Verify or configure the default parameters:
   - **Name**: `statesync`
   - **Repository**: `ghcr.io/ubermetroid/statesync:latest`
   - **WebUI Port**: `8754` (mapped to host loopback by default; do **not** expose publicly without setting `STATESYNC_WEB_AUTH`).
    - **Config Volume**: Map `/config` to `/mnt/user/appdata/statesync`.
    - **Bind Address**: leave `127.0.0.1:8754` unless you also configure **Web UI Bearer Token**.
    - **Web UI Bearer Token**: only needed if you change the bind address. Generate with `openssl rand -hex 32` and paste it in.
    - **Timezone (TZ)**: Change from `UTC` to your local timezone.
4. Click **Apply** to download and start the container.

---

## Backfilling history

StateSync's WebSocket path is **forward-only** — it only syncs state when Emby or Jellyfin emits a `Sessions` or `UserDataChanged` event. The **backfill** command reconciles existing watch history between servers.

### Trigger from CLI

```bash
statesync --backfill [--force] \
  [--direction=emby-to-jellyfin|jellyfin-to-emby|both] \
  [--merge=max|source-wins|newest] \
  [--scope=played|resumable|all] \
  [--rate=5]
```

Run with `--backfill --help` for inline help. CLI exits 0 on success, 1 on failures. Progress logged every 2s.

### Trigger from dashboard

Click `BACKFILL` in the header. Choose direction, merge policy, scope, rate, optionally force. Progress polls `/api/backfill/status` every 1s.

### Trigger via HTTP

```bash
curl -X POST http://127.0.0.1:8754/api/backfill \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"direction":"both","merge":"max","scope":"all","rate":5,"force":false}'
```

Returns `202 Accepted` with initial status, or `409 Conflict` if already running.

### Force flag

`--force` / `force: true` bypasses the `last_syncs` dedup cache and re-pushes every item. Always uses `source-wins` merge.

Use for:
- Reconciling two servers that have drifted
- Applying a new merge policy retroactively
- Recovering from suspected cache poisoning

### Merge policies

| Policy | Behavior |
|---|---|
| `max` (default) | `max(source_position, target_position)`; mark played if either side is played. Never reduces progress. |
| `source-wins` | Always source. Overwrites target. |
| `newest` | Pick side with newer `LastPlayedDate`. Falls back: source if source missing, target if target missing. |

### Limits

- Rate cap: 1–50 items/sec (default 5)
- Hard cap: 100,000 items per run
- One backfill at a time (mutex-guarded)
- HTTP/WS timeouts: 60s per page, 30s per `update_progress`
- Cancelling: graceful — completes current item and stops

### Auto-start

Set `STATESYNC_BACKFILL_ON_START=true` to auto-run on daemon start. Defaults come from `STATESYNC_BACKFILL_DIRECTION`, `STATESYNC_BACKFILL_MERGE`, `STATESYNC_BACKFILL_SCOPE`, `STATESYNC_BACKFILL_RATE` env vars.

---

## About

StateSync synchronizes watch states and playback progress bi-directionally in real-time between Emby and Jellyfin servers.
