# Emby Sync Play Daemon

A lightweight, high-performance Rust daemon designed to synchronize playback across multiple Emby Media Server client sessions. Perfect for watch parties, home setups with multiple TVs, or synchronized multi-room playback.

## Features

- **Multi-Leader Sync**: Play, pause, seek, or change media on *any* of the configured TVs, and all other TVs will instantly synchronize.
- **Choose to Join (Dynamic Playlists)**: No automatic forcing. When someone starts watching a movie (e.g. Kyle is watching *Bob's Burgers* in the Living Room), a temporary Emby playlist named `Join Living Room - Bob's Burgers` is dynamically created. If you want to join, select that playlist from your TV home screen and click Play. If you don't want to join, just ignore it and watch your own content normally.
- **Zero-Friction Setup**: No custom media libraries, dummy video files, or folders needed. Everything is handled dynamically via Emby's native Playlists API.
- **Auto-Cleanup**: The daemon automatically deletes the watch-party playlists as soon as the TV sessions stop playing or change media.
- **Smart Lag/Buffering Correction**: If a TV is buffering or lagging behind, it is commanded to seek forward to catch up *without* dragging the other TVs backward.
- **Automatic Cooldowns**: Prevents feedback loops where one TV's state update triggers commands that echo back.
- **Dynamic Reconnection**: Automatically reconnects to the Emby WebSocket in case of connection dropouts or server restarts.

## How it Works

The daemon runs an asynchronous event loop that:
1. Listens for session updates on Emby via WebSockets.
2. Creates and removes temporary `Join {Room} - {Movie}` playlists on your Emby server dynamically.
3. Intercepts when a TV client plays one of these dynamic playlists, stops the playlist playback, and instantly redirects your TV client to join the target room's movie at its current playback position.

---

## Configuration

The daemon is configured via a `config.json` file in its current working directory.

### Example `config.json`

```json
{
  "emby_url": "http://192.168.1.100:8096",
  "api_key": "YOUR_EMBY_API_KEY",
  "sync_devices": [
    { "id": "device_id_of_tv_1", "name": "Living Room" },
    { "id": "device_id_of_tv_2", "name": "Bedroom" },
    { "id": "device_id_of_tv_3", "name": "Kitchen" }
  ],
  "sync_threshold_seconds": 3,
  "cooldown_seconds": 5
}
```

### Configuration Fields

- `emby_url`: The URL of your Emby Media Server (e.g., `http://192.168.1.100:8096`).
- `api_key`: A valid Emby API key. Generate one from the Emby dashboard under **Settings** -> **API Keys**.
- `sync_devices`: A list of target devices to sync, specified as objects:
  - `id`: The unique `DeviceId` of the client.
  - `name`: A friendly name (e.g. `Living Room`). This name will be used to generate the playlist `Join {name} - {movie}`.
- `sync_threshold_seconds`: The maximum difference (in seconds) allowed between client positions before a seek command is triggered. Default: `3`.
- `cooldown_seconds`: Cooldown duration (in seconds) applied to a device after commanding it, ignoring its transient status reports. Default: `5`.

---

## How to Run

1. **Find TV Device IDs**: Start the daemon with dummy Device IDs. At startup, the daemon will print all active client sessions and their details:
   ```text
   [INFO] Successfully connected to Emby server. Active sessions found:
   [INFO]   - Device: 'LG OLED', Client: 'Emby for LG Smart TV', User: 'Jeryd', DeviceId: 'a1b2c3d4-e5f6...'
   ```
2. **Configure your Devices**: Copy the `DeviceId`s and update the `sync_devices` array in `config.json` with their IDs and friendly names.
3. **Run the Daemon**:
   ```bash
   RUST_LOG=info cargo run
   ```
   Or build a release version:
   ```bash
   cargo build --release
   ./target/release/emby-syncplay
   ```
