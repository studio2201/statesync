# StateSync

Copies **watched**, **resume point**, and **favorites** between Emby and Jellyfin  
(and Emby↔Emby or Jellyfin↔Jellyfin). Same person, same title — both servers agree.

It does **not** move video files, ratings, playlists, or library structure.

---

## What you need

1. One or more Emby and/or Jellyfin servers
2. An **API key** from each server’s admin UI
3. A machine that can reach them over your LAN (Unraid, Docker, etc.)

---

## Install (Unraid)

1. Docker → **Add Container** (import `statesync.xml` from this repo if needed)
2. **Network Type: `br0`** (same custom network Emby/Jellyfin use)
3. Optional: give StateSync its own fixed IP on that network
4. Appdata: `/mnt/user/appdata/statesync`
5. Apply, open `http://STATESYNC-IP:4601`

No login.

### Networking (if “can’t connect”)

If Emby or Jellyfin has its **own LAN IP** on Unraid **br0** (macvlan):

| StateSync network | Can reach Emby on br0? |
|-------------------|------------------------|
| `br0` (same as Emby) | **Yes** |
| `bridge` (docker0) | **Usually no** |
| `host` | **Usually no** (host can’t talk to its own macvlan containers) |

Your PC can open the media server in a browser. That does **not** mean a container on `bridge`/`host` can. Put StateSync on **br0**, then use the media server’s br0 IP.

---

## Install (Docker Compose)

```yaml
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
      - TZ=UTC
      - RUST_LOG=info
```

```bash
mkdir -p config
docker compose up -d
# open http://localhost:4601
```

---

## First setup (web UI)

1. Open the dashboard
2. **Add server** → address + API key (paste a full browser URL if you want)
3. **Test connection** or **Save** — Emby vs Jellyfin is **detected automatically**
4. Add the other server(s)
5. If usernames differ, **Link users**
6. Optional: **Force sync** once to backfill history

### Server address

Use something StateSync can reach **from the container**:

| Good | Bad |
|------|-----|
| `http://10.0.0.5:8096` | `localhost` (the container itself) |
| `http://10.0.0.5:8920` | Hostnames Docker can’t resolve |

Full browser URLs are fine. Only **host + port** is kept. Same IP, different ports get distinct auto-names (`10.0.0.5:8096` vs `10.0.0.5:8920`).

### API key

Create one in Emby or Jellyfin admin settings. It lives in `config.json` — keep it private.

---

## What syncs

| | Live (while watching) | Force sync (backfill) |
|--|----------------------|------------------------|
| **Played** | Yes | Yes — skips if target already watched |
| **Position** | Yes | Yes — in-progress; skips if already equal |
| **Favorites** | Yes | Yes — skips if already favorited |

**Not synced:** ratings, playlists, collections, hidden items, layout, passwords, libraries.

Titles match by **IMDb / TMDb**. People match by **username** or **Link users**.

Dashboard status **Live** means the event stream is open (healthy).

---

## After it works

- **Now playing** — who’s watching, with posters  
- **Force sync** — historical catch-up; shows phases and *why* items were skipped  
- **Settings** — turn live/force fields on or off; threshold for near-duplicate progress  
- **Activity log** — copyable story for support  

Config file:

`/config/config.json` (Unraid: `/mnt/user/appdata/statesync/config.json`)

---

## Common problems

**“Failed to get users list” / can’t connect**  
1. Address is a **LAN IP** reachable *from Docker*, not `localhost`  
2. Correct port  
3. Valid API key  
4. StateSync on the same network path as the media server (often **br0**)

**Users don’t match**  
Same username matches automatically. Different names → **Link users** (or Settings text mappings).

**Nothing while watching**  
Both servers should show **Live**. Wait a few seconds after pause/seek. Force is for history, not a substitute for a live link.

**Force mostly “already matched”**  
Good — target already had that state. Second runs should be fast.

---

## CLI (optional)

```bash
statesync --help
statesync --version
statesync --validate       # config + connection check
statesync --sync-force     # full backfill (played / position / favorites per Settings)
statesync --tui            # terminal dashboard (same story as the web UI)
statesync --dry-run        # mapping / cache check without writing play state
statesync --reload         # ask the running service to reload config
```

Force prints phases and skip reasons (already matched, no provider id, not in other library).

---

## How it works (short)

1. Connects to each server (HTTP + live event stream)  
2. Detects Emby vs Jellyfin  
3. Watches play / played / favorite changes  
4. Matches titles by IMDb/TMDb, people by name or link  
5. Writes only the fields you enabled — skips when already equal  

---

## Optional environment

| Variable | Default | Meaning |
|----------|---------|---------|
| `STATESYNC_BIND` | `0.0.0.0:4601` | Web UI listen address |
| `STATESYNC_SYNC_THRESHOLD_SECONDS` | `5` | Ignore near-duplicate progress |
| `STATESYNC_FORCE_RATE` | `5` | Force items/sec (1–50) |
| `STATESYNC_LOG_RETENTION` | `100` | In-memory activity log lines |
| `STATESYNC_ACCEPT_INVALID_CERTS` | `false` | Self-signed HTTPS only if you must |
| `RUST_LOG` | `info` | Log level |
| `TZ` | `UTC` | Timestamps |

---

## Links

- Image: `ghcr.io/studio2201/statesync:latest` (also `0.28.x` / `v0.28.x` each release)
- Unraid not pulling new image: force-update / remove local image, re-apply  
- Issues: https://github.com/studio2201/statesync/issues  

## License

MIT
