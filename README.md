# StateSync

Syncs **watch progress** between Emby and Jellyfin.

Pause, resume, or finish something on one server → the same position shows up on the other.

---

## What you need

1. An Emby server and/or a Jellyfin server (one of each is the usual case)
2. An **API key** from each server’s admin UI
3. A machine that can reach both over your LAN (Unraid, Docker, etc.)

StateSync does **not** move video files. It only copies *where you left off* and *played* status.

---

## Install (Unraid)

1. Docker → **Add Container** (or add this repo as a template source)
2. Use **`statesync.xml`** from this repo (or `unraid/unraid-template.xml`)
3. Check:
   - **Network:** bridge  
   - **Port:** `4601`  
   - **Appdata:** `/mnt/user/appdata/statesync`
4. Apply, then open: `http://YOUR-UNRAID-IP:4601`

No login.

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
2. Click **Add server**
3. Pick **Emby** or **Jellyfin**
4. Enter the server address and API key
5. **Test connection**, then **Save**
6. Repeat for the other server

### Server address

Use something StateSync can reach from the **container**:

| Good | Bad |
|------|-----|
| `http://10.0.0.5:8096` | `localhost` (that’s the container itself) |
| `http://emby.lan:8096` | Hostnames only your Unraid box knows, if Docker can’t resolve them |

You can paste a full browser URL (for example the API keys page). StateSync keeps only **host + port** and drops paths like `/web/index.html#!/…`.

### API key

Create one in Emby or Jellyfin admin settings, then paste it into StateSync. Keep it private; it lives in `config.json`.

---

## After it works

- **Live sync** — while something is playing, progress is mirrored in near real time
- **Mapped users** — same person on both servers should share a name, or map names under **Settings**
- **Force sync** — one-time catch-up of older “played” history (optional; use after first install if you want history filled in)

Config file (if you prefer editing by hand):

`/config/config.json` inside the container  
(Unraid: `/mnt/user/appdata/statesync/config.json`)

---

## Common problems

**“Failed to get users list”**  
StateSync can’t talk to that server. Check:

1. Address is a **LAN IP** (or a hostname that works *from Docker*), not `localhost`
2. Port is correct (often `8096`)
3. API key is valid
4. Type is Emby vs Jellyfin correctly

**Users don’t match**  
If Alice is `alice` on one box and `Alice Home` on the other, add a mapping in **Settings** (one line, names separated by commas).

**Nothing happens while watching**  
Both servers should be online (status on the dashboard). Give it a few seconds after pause/seek. Force sync is for history, not a substitute for live WebSocket connection.

---

## Optional settings (advanced)

Most people leave these alone.

| Variable | Default | Meaning |
|----------|---------|---------|
| `STATESYNC_BIND` | `0.0.0.0:4601` | Where the web UI listens |
| `STATESYNC_ALLOW_INSECURE_HTTP` | `true` | Allow `http://` to media servers on the LAN |
| `STATESYNC_ACCEPT_INVALID_CERTS` | `false` | Only if you use broken/self-signed HTTPS on purpose |
| `STATESYNC_SYNC_THRESHOLD_SECONDS` | `5` | Ignore tiny duplicate progress updates |
| `PUID` / `PGID` / `UMASK` | `99` / `100` / `022` | File ownership (Unraid “nobody” style) |
| `RUST_LOG` | `info` | Log noise (`debug` for troubleshooting) |
| `TZ` | `UTC` | Log timestamps |

---

## CLI (optional)

```bash
statesync --help
statesync --version
statesync --validate      # check config + connections
statesync --sync-force    # full historical played-item push
statesync --dry-run       # see user/item mapping without writing
```

---

## How it works (short)

1. Connects to each media server (HTTP API + WebSocket)
2. Watches for playback / “played” changes
3. Matches titles by **IMDb / TMDb** IDs (not folder names)
4. Matches people by **username** (or your mappings)
5. Writes the same progress to the other server

It does not rewrite your libraries. It only updates user watch state.

---

## Links

- Image: `ghcr.io/studio2201/statesync:latest` (also tagged `v0.28.x` each release)
- If Unraid does not pull a new image: force-update / remove the local image, then re-apply
- Package must be **public** under GitHub → Packages → statesync → Package settings → Change visibility
- Issues: https://github.com/studio2201/statesync/issues

## License

MIT
