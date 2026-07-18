# Emby Sync Play Daemon

A lightweight, high-performance Rust daemon designed to synchronize playback across multiple Emby Media Server client sessions. Perfect for watch parties, home setups with multiple TVs, or synchronized multi-room playback.

## Features

- **Multi-Leader Sync**: Play, pause, seek, or change media on *any* of the configured TVs, and all other TVs will instantly synchronize.
- **WebSocket Driven**: Subscribes to real-time session events on Emby, reducing network overhead and providing sub-second reaction times.
- **Smart Lag/Buffering Correction**: If a TV is buffering or lagging behind, it is commanded to seek forward to catch up *without* dragging the other TVs backward.
- **Automatic Cooldowns**: Prevents feedback loops where one TV's state update triggers commands that echo back.
- **Dynamic Reconnection**: Automatically reconnects to the Emby WebSocket in case of connection dropouts or server restarts.
- **Startup Sessions Inspector**: Queries and lists all active client sessions and their Device IDs at startup to simplify configuration.

## How it Works

The daemon runs an asynchronous event loop that:
1. Listens for session updates on Emby via WebSockets.
2. Identifies user actions by comparing a TV's current state with its *own previous state* (detecting new items, play/pause toggles, and position jumps).
3. Propagates those actions to the rest of the sync group.
4. Corrects any client that falls out of sync with the collective state due to starting up late, lag, or buffering.

---

## Configuration

The daemon is configured via a `config.json` file in its current working directory.

### Example `config.json`

```json
{
  "emby_url": "http://192.168.1.100:8096",
  "api_key": "YOUR_EMBY_API_KEY",
  "sync_devices": [
    "device_id_of_tv_1",
    "device_id_of_tv_2",
    "device_id_of_tv_3"
  ],
  "sync_threshold_seconds": 3,
  "cooldown_seconds": 5
}
```

### Configuration Fields

- `emby_url`: The URL of your Emby Media Server (e.g., `http://192.168.1.100:8096` or `https://emby.yourdomain.com`).
- `api_key`: A valid Emby API key. Generate one from the Emby dashboard under **Settings** -> **API Keys**.
- `sync_devices`: A list of unique `DeviceId`s for the Emby clients that you want to keep synchronized.
- `sync_threshold_seconds`: The maximum difference (in seconds) allowed between client positions before a seek command is triggered. Default: `3`.
- `cooldown_seconds`: Cooldown duration (in seconds) applied to a device after commanding it, ignoring its transient status reports to prevent race conditions. Default: `5`.

---

## How to Get Device IDs

To find the `DeviceId` of your TVs:
1. Open the Emby client app on each TV.
2. Start the daemon with dummy Device IDs.
3. At startup, the daemon will connect to your Emby server, discover all active client sessions, and print their details to the log:
   ```text
   [INFO] Successfully connected to Emby server. Active sessions found:
   [INFO]   - Device: 'Living Room TV', Client: 'Emby for Android TV', User: 'Jeryd', DeviceId: 'a1b2c3d4-e5f6...'
   ```
4. Copy the `DeviceId` of your TVs and paste them into the `sync_devices` array in `config.json`.

---

## Running the Daemon

Set the log filter environment variable to see output, and run:

```bash
RUST_LOG=info cargo run
```

Or build a release version:

```bash
cargo build --release
./target/release/emby-syncplay
```
