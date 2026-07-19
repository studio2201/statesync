# <img src="graphics/statesync_icon.jpg" width="32" height="32" valign="middle" /> StateSync

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

## Deployment

StateSync runs as a static, zero-dependency Distroless container built on `scratch`.

### 1. Docker Compose

Create a `docker-compose.yml` file:
```yaml
version: '3.8'
services:
  statesync:
    image: ubermetroid/statesync:latest
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
Run the service:
```bash
docker compose up -d
```

### 2. Docker CLI
```bash
docker run -d \
  --name statesync \
  -p 8754:8754 \
  -v /path/to/config:/config \
  -e RUST_LOG=info \
  -e TZ=UTC \
  ubermetroid/statesync:latest
```

---

## Unraid Setup

The template is located in the repository under **[`unraid/unraid-template.xml`](unraid/unraid-template.xml)**.

- **Port**: `8754`
- **Volume**: Map `/config` to save `config.json`.
- **Environment**: Set `TZ` (Timezone) to match your local timezone (e.g., `America/New_York`).

---

## Local Development

```bash
RUST_LOG=info cargo run
```
Open `http://localhost:8754` in your browser.
