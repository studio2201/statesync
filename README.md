# <img src="graphics/statesync_icon.jpg" width="48" height="48" valign="middle" /> StateSync

<p align="center">
  <img src="graphics/statesync_header.jpg" width="100%" height="180" style="object-fit: cover; border-radius: 8px;" alt="StateSync Header" />
</p>

A lightweight, high-performance Rust sidecar daemon designed to synchronize playback progress, watch states, and resume points bi-directionally between an arbitrary number of Emby and Jellyfin Media Servers in real-time.

It features a simple, beautiful **Web UI Dashboard** running on port `8754` so you can manage your servers directly from your web browser with zero configuration files to edit!

---

## Dashboard Interface Screenshot

![StateSync Dashboard](graphics/bgL4h.jpg)

---

## Features

- **Web UI Dashboard**: Add, remove, and monitor your Emby and Jellyfin servers directly in your browser on port `8754`.
- **Bi-directional Real-Time Sync**: Syncs playback positions, play states, and paused/resumed statuses between all configured servers instantly.
- **Support for N-Servers**: Syncs across 2, 3, or more servers seamlessly.
- **IMDb & TMDb Matching**: Uses global identifiers (IMDb ID and TMDb ID) from the metadata of your media files to link items. Works perfectly even if database IDs, filenames, or library structures differ between your servers.
- **LDAP-Friendly User Mapping**: Matches users across servers automatically by matching their usernames (case-insensitive) or via manual configuration groups.
- **Intelligent Feedback Loop Prevention**: Caches and tracks the last synchronized positions per user/movie to prevent endless "ping-pong" update loops between servers.
- **Robust Connection Recovery**: Connects to the WebSockets of all servers concurrently and automatically reconnects in case of connection dropouts or server restarts.
- **Zero Server Modification**: Requires no plugins, DLLs, or restarts on your servers. Connects purely via standard REST APIs and WebSockets.

---

## Container Deployment

StateSync is packaged as a secure, zero-attack-surface **Distroless Scratch** container built statically using Rust's Musl target compilation. It has no OS libraries, package managers, or shells, making it highly secure.

### 1. Run with Docker Compose (Recommended)

1. Create a `docker-compose.yml` file using our layout under the `container/` directory:
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
2. Start the container:
   ```bash
   docker compose up -d
   ```

### 2. Run with Docker CLI
```bash
docker run -d \
  --name statesync \
  -p 8754:8754 \
  -v /path/to/config:/config \
  -e RUST_LOG=info \
  -e TZ=UTC \
  ubermetroid/statesync:latest
```

Once the container starts, open **`http://<your-ip>:8754`** in your browser to configure your servers!

---

## Unraid Setup

We maintain a dedicated Unraid application XML template under the **`unraid/`** directory:

1. **Relocated Template**: The template is located at [unraid/unraid-template.xml](unraid/unraid-template.xml).
2. **Timezone Support**: Out of the box, the container supports custom timezone configurations via the `TZ` environment variable. You can specify your local timezone directly in Unraid's deployment interface.
3. **Persistent Volume**: Map the container `/config` directory to your preferred path on disk (e.g. `/mnt/user/appdata/statesync`).

---

## Local Development (Without Containers)

1. Install Cargo and Rust.
2. Build and run locally:
   ```bash
   RUST_LOG=info cargo run
   ```
   Open `http://localhost:8754` to access the dashboard.
