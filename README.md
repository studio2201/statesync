# StateSync

**Watched, resume, and favorites — synced across Emby and Jellyfin.**

```bash
docker run -d --name statesync -p 4601:4601 -v statesync-config:/config \
  ghcr.io/studio2201/statesync:latest
```

Open **http://localhost:4601** → **Add server** (paste address + API key) twice → play something.

StateSync detects Emby vs Jellyfin, matches people and titles, and keeps watch state in sync. No login.

---

## One perfect flow

1. Run the container (command above).  
2. Add your Emby (or Jellyfin) URL + API key → Save (type is auto-detected).  
3. Add the other server.  
4. If usernames differ, click **Link users**.  
5. Optional: **Preview force** then **Force sync** for history.  

That’s it. Live plays sync automatically; force fills in the past.

---

## Install elsewhere

**Unraid:** import `statesync.xml`, network **br0** (same as Emby/Jellyfin if they use macvlan), appdata → `/mnt/user/appdata/statesync`.

**Compose:**

```yaml
services:
  statesync:
    image: ghcr.io/studio2201/statesync:latest
    ports: ["4601:4601"]
    volumes: ["./config:/config"]
    restart: unless-stopped
```

---

## What it syncs

| | Live | Force |
|--|------|--------|
| Played | ✓ | ✓ (skips if already matched) |
| Position | ✓ | ✓ |
| Favorites | ✓ | ✓ |

**Clear watched** is a per-user button (all servers for that person) — not force sync.  
**Not synced:** ratings, playlists, libraries, files.

---

## CLI

```bash
statesync --validate
statesync --sync-force --dry-run   # preview
statesync --sync-force
statesync --tui
```

---

## Docs & ops

- Networking (Unraid br0): if the container can’t reach Emby, put StateSync on **br0** next to it.  
- Config: `/config/config.json`  
- Image tags: `latest`, `0.28.x`, `v0.28.x`  
- Issues: https://github.com/studio2201/statesync/issues  

Architecture and env vars: see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) (if present) or in-app **How sync works**.

## License

MIT
