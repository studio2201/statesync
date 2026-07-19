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
    image: ghcr.io/ubermetroid/statesync:latest
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
  ghcr.io/ubermetroid/statesync:latest
```

---

## Unraid Setup

To install StateSync on Unraid:

### 1. Add the Template Repository
1. Navigate to the **Docker** tab in the Unraid WebUI.
2. Scroll to the bottom of the page and locate the **Template Repositories** field.
3. Paste the following URL:
   ```text
   https://github.com/UberMetroid/statesync
   ```
4. Click **Save**.

### 2. Configure and Install Container
1. Click **Add Container** on the Docker page.
2. In the **Template** dropdown, select **statesync**.
3. Verify or configure the default parameters:
   - **Name**: `statesync`
   - **Repository**: `ghcr.io/ubermetroid/statesync:latest`
   - **WebUI Port**: `8754` (mapped to port 8754 on host).
   - **Config Volume**: Map `/config` to `/mnt/user/appdata/statesync` (to persist `config.json`).
   - **Timezone (TZ)**: Change from `UTC` to your local timezone (e.g., `America/New_York`) to align log times.
4. Click **Apply** to download and start the container.
