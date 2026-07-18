# statesync

A lightweight, high-performance Rust daemon designed to synchronize playback progress, watch states, and resume points bi-directionally between an arbitrary number of Emby and Jellyfin Media Servers in real-time.

## Features

- **Bi-directional Real-Time Sync**: Syncs playback positions, play states, and paused/resumed statuses between all configured servers instantly.
- **Support for N-Servers**: Syncs across 2, 3, or more servers seamlessly.
- **IMDb & TMDb Matching**: Uses global identifiers (IMDb ID and TMDb ID) from the metadata of your media files to link items. Works perfectly even if database IDs, filenames, or library structures differ between your servers.
- **LDAP-Friendly User Mapping**: Matches users across servers automatically by matching their usernames (case-insensitive). Perfect for setups synced via LDAP or Active Directory.
- **Intelligent Feedback Loop Prevention**: Caches and tracks the last synchronized positions per user/movie to prevent endless "ping-pong" update loops between servers.
- **Robust Connection Recovery**: Connects to the WebSockets of all servers concurrently and automatically reconnects in case of connection dropouts or server restarts.
- **Zero Server Modification**: Requires no plugins, DLLs, or restarts on your servers. Connects purely via standard REST APIs and WebSockets.

---

## Unraid Deployment (No Config Files)

`statesync` is fully compatible with Unraid Community Applications and includes a native **[unraid-template.xml](unraid-template.xml)** file. 

This allows you to add, edit, and configure your Emby and Jellyfin servers directly in the **Unraid Web GUI form fields** without ever touching configuration files:

1. **Add Template**: Copy the raw URL of `unraid-template.xml` from your repository and add it to your Unraid templates path.
2. **Fill in the Fields**: Enter the URL and API key for each of your servers (supports up to 4 servers out-of-the-box).
3. **Click Apply**: Unraid builds the container and runs it with flat environment variables automatically!

---

## Alternative Configuration Options

For non-Unraid deployments, you can choose from these options:

### Option A: Flat Environment Variables (Recommended for CLI)

Set these environment variables directly on the container (supports up to 20 servers):

- `STATESYNC_SERVER_0_NAME`: Friendly name for Server 0.
- `STATESYNC_SERVER_0_URL`: URL of Server 0.
- `STATESYNC_SERVER_0_API_KEY`: API Key for Server 0.
- `STATESYNC_SERVER_0_TYPE`: Type of Server 0 (`emby` or `jellyfin`).
- `STATESYNC_SERVER_1_NAME`: Friendly name for Server 1.
- `STATESYNC_SERVER_1_URL`: URL of Server 1.
- `STATESYNC_SERVER_1_API_KEY`: API Key for Server 1.
- `STATESYNC_SERVER_1_TYPE`: Type of Server 1 (`emby` or `jellyfin`).
- *(repeat for `SERVER_2`, `SERVER_3`, etc.)*
- `STATESYNC_SYNC_THRESHOLD_SECONDS`: Optional. Sync threshold in seconds. Default: `5`.

### Option B: `config.json` File Volume Mount

Create a `config.json` file and mount it to `/etc/statesync/config.json`:

```json
{
  "servers": [
    {
      "name": "Emby Home",
      "url": "http://192.168.3.3:8096",
      "api_key": "YOUR_EMBY_API_KEY",
      "is_emby": true
    },
    {
      "name": "Jellyfin Primary",
      "url": "http://192.168.3.10:8096",
      "api_key": "YOUR_JELLYFIN_API_KEY",
      "is_emby": false
    }
  ],
  "sync_threshold_seconds": 5
}
```

---

## Container Deployment (Docker Compose)

We package `statesync` as a lightweight container using **RedHat UBI-minimal (`ubi9/ubi-minimal`)** as the secure base runtime image.

1. Create a `docker-compose.yml` file:
   ```yaml
   version: '3.8'
   services:
     statesync:
       build: .
       container_name: statesync
       restart: unless-stopped
       environment:
         - STATESYNC_SERVER_0_NAME=Emby
         - STATESYNC_SERVER_0_URL=http://192.168.3.3:8096
         - STATESYNC_SERVER_0_API_KEY=YOUR_EMBY_API_KEY
         - STATESYNC_SERVER_0_TYPE=emby
         - STATESYNC_SERVER_1_NAME=Jellyfin
         - STATESYNC_SERVER_1_URL=http://192.168.3.10:8096
         - STATESYNC_SERVER_1_API_KEY=YOUR_JELLYFIN_API_KEY
         - STATESYNC_SERVER_1_TYPE=jellyfin
         - RUST_LOG=info
   ```
2. Build and start the container:
   ```bash
   docker compose up -d --build
   ```

---

## Local Development (Without Containers)

1. Install Cargo and Rust.
2. Build and run locally:
   ```bash
   RUST_LOG=info cargo run
   ```
