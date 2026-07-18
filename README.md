# Emby-Jellyfin Playstate Sync Sidecar

A lightweight, high-performance Rust daemon designed to synchronize playback progress, watch states, and resume points bi-directionally between an Emby Media Server and a Jellyfin Media Server in real-time.

## Features

- **Bi-directional Real-Time Sync**: Syncs playback positions, play states, and paused/resumed statuses between Emby and Jellyfin instantly.
- **IMDb & TMDb Matching**: Uses global identifiers (IMDb ID and TMDb ID) from the metadata of your media files to link items. Works perfectly even if database IDs, filenames, or library structures differ between your servers.
- **LDAP-Friendly User Mapping**: Matches users across servers automatically by matching their usernames (case-insensitive). Perfect for setups synced via LDAP or Active Directory.
- **Intelligent Feedback Loop Prevention**: Caches and tracks the last synchronized positions per user/movie to prevent endless "ping-pong" update loops between servers.
- **Robust Connection Recovery**: Connects to the WebSockets of both servers concurrently and automatically reconnects in case of connection dropouts or server restarts.
- **Zero Server Modification**: Requires no plugins, DLLs, or restarts on either Emby or Jellyfin. Connects purely via standard REST APIs and WebSockets.

---

## Configuration

The sidecar is configured via a `config.json` file in its current working directory.

### Example `config.json`

```json
{
  "emby": {
    "url": "http://192.168.3.3:8096",
    "api_key": "YOUR_EMBY_API_KEY"
  },
  "jellyfin": {
    "url": "http://192.168.3.10:8096",
    "api_key": "YOUR_JELLYFIN_API_KEY"
  },
  "sync_threshold_seconds": 5
}
```

### Configuration Fields

- `emby.url`: The URL of your Emby Media Server.
- `emby.api_key`: A valid Emby API key.
- `jellyfin.url`: The URL of your Jellyfin Media Server.
- `jellyfin.api_key`: A valid Jellyfin API key.
- `sync_threshold_seconds`: The maximum difference (in seconds) allowed between client positions before a seek command is triggered. Default: `5`.

---

## How to Run

1. **Configure Endpoints**: Open `config.json` and fill in your Emby and Jellyfin server URLs and API keys.
2. **Run the Daemon**:
   ```bash
   RUST_LOG=info cargo run
   ```
   Or build a release version:
   ```bash
   cargo build --release
   ./target/release/emby-syncplay
   ```
