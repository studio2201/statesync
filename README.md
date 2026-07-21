# StateSync

**Watched, resume, and favorites — synced across Emby and Jellyfin.**

## Install (one line)

```bash
docker run -d --name statesync -p 4601:4601 -v statesync-config:/config ghcr.io/studio2201/statesync:latest
```

No env vars. No login. Open **http://localhost:4601**.

## One perfect example

```text
1. Run the install command above.
2. Dashboard → Add server → paste Emby (or Jellyfin) URL + API key → Save
   (type auto-detects; browser paths like …/web/index.html are stripped to host:port).
3. Add the other server the same way.
4. If usernames differ → Link users.
5. Play something on one server → watch it appear on the other.
   Optional: Preview force → Force sync for older history.
```

That is the whole product path.

---

## Deploy targets

| Target | How |
|--------|-----|
| **Docker / any host** | One-liner above (`ghcr.io/studio2201/statesync`) |
| **Unraid** | Community Apps / import `unraid/unraid-template.xml` — appdata → `/mnt/user/appdata/statesync`, port **4601**, shell **sh** (BusyBox ash). If Emby/Jellyfin use **br0**, put StateSync on **br0** too so it can reach them. |
| **Compose** | `container/docker-compose.yml` (volume + port only) |
| **Binary** | GitHub Release `statesync-linux-x86_64.tar.gz` (static musl) |

Image tags: `latest`, `0.28.x`, `v0.28.x`.

---

## What it syncs

| | Live | Force |
|--|------|--------|
| Played | ✓ | ✓ (skip if already equal) |
| Position | ✓ | ✓ |
| Favorites | ✓ | ✓ |

**Clear watched** is a dedicated per-user action (all servers), not force.  
**Not synced:** ratings, playlists, libraries, media files.

---

## Runtime defaults (zero config)

| | Default |
|--|---------|
| Bind | `0.0.0.0:4601` |
| Config | `/config/config.json` (created on first save) |
| Auth | off |
| Base image | **Alpine Linux** + BusyBox **ash** (Unraid console works) |
| User | PUID/PGID `99:100` when unset (Unraid-friendly) |

Optional knobs only if you need them: see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

```bash
statesync --validate
statesync --sync-force --dry-run
statesync --sync-force
statesync --tui
```

---

## Links

- Issues: https://github.com/studio2201/statesync/issues  
- Packages: https://github.com/studio2201/statesync/pkgs/container/statesync  
- Releases: https://github.com/studio2201/statesync/releases  
- In-app: **How sync works**

## License

MIT
