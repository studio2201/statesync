# Emby Sync Play Daemon

A lightweight, high-performance Rust daemon designed to synchronize playback across multiple Emby Media Server client sessions. Perfect for watch parties, home setups with multiple TVs, or synchronized multi-room playback.

## Features

- **Multi-Leader Sync**: Play, pause, seek, or change media on *any* of the configured TVs, and all other TVs will instantly synchronize.
- **Choose to Join (Dynamic Playlists)**: No automatic forcing. When someone starts watching a movie (e.g. Kyle is watching *Bob's Burgers*), a temporary Emby playlist named `Join - Bob's Burgers` is dynamically created. If you want to join, select that playlist from your TV home screen and click Play. If you don't want to join, just ignore it and watch your own content normally.
- **Multi-User Visibility**: Playlists are dynamically created for **every user account** on the server, ensuring that whichever user is logged in on any client browser or TV will see the watch-party playlist card instantly under their native "Playlists" section.
- **Zero-Friction Setup**: No custom media libraries, dummy video files, or folders needed. Everything is handled dynamically via Emby's native Playlists API.
- **Auto-Cleanup**: The daemon automatically deletes the watch-party playlists across all user accounts as soon as the TV sessions stop playing or change media.
- **Smart Lag/Buffering Correction**: If a TV is buffering or lagging behind, it is commanded to seek forward to catch up *without* dragging the other TVs backward.
- **Automatic Cooldowns**: Prevents feedback loops where one TV's state update triggers commands that echo back.
- **Dynamic Reconnection**: Automatically reconnects to the Emby WebSocket in case of connection dropouts or server restarts.

## How it Works

The daemon runs an asynchronous event loop that:
1. Listens for session updates on Emby via WebSockets.
2. Creates and removes temporary `Join - {Movie}` playlists on your Emby server dynamically for all user accounts.
3. Intercepts when a TV client plays one of these dynamic playlists, stops the playlist playback, and instantly redirects your TV client to join the target room's movie at its current playback position.

---

## Configuration

The daemon is configured via a `config.json` file in its current working directory.

### Example `config.json`

```json
{
  "emby_url": "http://192.168.1.100:8096",
  "api_key": "YOUR_EMBY_API_KEY",
  "sync_threshold_seconds": 3,
  "cooldown_seconds": 5
}
```

### Configuration Fields

- `emby_url`: The URL of your Emby Media Server (e.g., `http://192.168.1.100:8096`).
- `api_key`: A valid Emby API key. Generate one from the Emby dashboard under **Settings** -> **API Keys**.
- `sync_threshold_seconds`: The maximum difference (in seconds) allowed between client positions before a seek command is triggered. Default: `3`.
- `cooldown_seconds`: Cooldown duration (in seconds) applied to a device after commanding it, ignoring its transient status reports. Default: `5`.

---

## How to Run

1. **Configure Server Info**: Open `config.json` and fill in your Emby server URL and API key.
2. **Run the Daemon**:
   ```bash
   RUST_LOG=info cargo run
   ```
   Or build a release version:
   ```bash
   cargo build --release
   ./target/release/emby-syncplay
   ```
