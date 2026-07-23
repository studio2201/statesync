<h1 align="center">
  <img src="assets/icon.png?v=1.0.31" width="48" height="48" valign="middle"> StateSync
</h1>

<p align="center">
  <b>Bi-directional watch state and position synchronization engine for Emby and Jellyfin written in Rust.</b>
</p>

---

### Instant One-Line Install (Docker Container)

Run the official zero-dependency container on port 8096:

```bash
docker run -d --name statesync -p 8096:8096 -v /mnt/user/appdata/statesync:/config ghcr.io/studio2201/statesync:latest
```

Open your browser to `http://localhost:8096` to access the real-time management dashboard.

---

### One-Line Install (Native Package Manager)

On Debian, Ubuntu, Fedora, or RHEL:

```bash
curl -fsSL https://studio2201.github.io/packages/install.sh | sudo bash
```

---

### Unraid NAS Deployment

Deploy via the official Unraid Template:

1. Copy [`statesync.xml`](statesync.xml) to your Unraid flash drive under `/boot/config/plugins/dockerMan/templates-user/`.
2. Open **Docker** -> **Add Container** -> Select **statesync** from the template dropdown.
3. Click **Apply**.

---

### Environment Configuration

The backend service can be customized using the following environment variables:

| Variable | Description | Default |
| :--- | :--- | :---: |
| `PORT` | Network port the web server binds to | `8096` |
| `STATESYNC_PIN` | Security PIN required for application access | *(Disabled)* |
| `STATESYNC_DATA_DIR` | Directory path for persistent data and configuration | `/config` |
| `STATESYNC_ALLOWED_ORIGINS` | CORS allowed origins list (comma-separated) | `*` |
| `TRUST_PROXY` | Honor reverse proxy headers (`X-Forwarded-For`) | `false` |
| `TRUSTED_PROXY_IPS` | Comma-separated CIDR list of trusted reverse proxies | *(None)* |
| `LOG_LEVEL` | Tracing filter (`error`, `warn`, `info`, `debug`) | `info` |

---

### Administration CLI & TUI Dashboard

Every container and package includes a built-in administration utility (`statesync`).

Launch interactive TUI dashboard:
```bash
docker exec -it statesync statesync tui
```

System diagnostics and self-healing check:
```bash
docker exec -it statesync statesync doctor
```

CLI Command Reference:
- `statesync tui` — Interactive terminal user interface.
- `statesync doctor` — Diagnoses storage permissions, ports, and database health.
- `statesync status` — Displays network configuration and security parameters.
- `statesync data stats` — Shows storage utilization and sync metrics.

---

### Architecture & Security

- **Axum Web Backend**: High-concurrency async HTTP runtime built on Tokio.
- **WebSocket Synchronization Protocol**: Low-latency bi-directional event bus for instant playback progress mapping.
- **Strict Input & Path Sanitization**: Path canonicalization guards preventing directory traversal escapes.
- **Fail-Closed Security PIN Authentication**: Rate-limited brute force protection with automatic lockout timers.

---

### License

Distributed under the Apache 2.0 License. See [LICENSE](LICENSE) for details.

---

### Project Banner Showcase

Official **StateSync** project banner displaying real-time media server state synchronization multi-device architecture.

<p align="center">
  <a href="https://github.com/studio2201/statesync">
    <img src="assets/statesync-header.jpg" alt="studio2201 banner" width="100%">
  </a>
</p>
