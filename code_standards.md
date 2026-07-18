# Coding Standards and Architecture Guidelines

This document outlines the design decisions, coding standards, and architectural guidelines established for the `statesync` daemon.

---

## 1. Core Stack & Dependencies

- **Runtime**: Asynchronous I/O is powered by **Tokio** (Edition 2024). All networking, file, and signal handling must be non-blocking.
- **REST Client**: **Reqwest** is used for REST HTTP client requests.
- **WebSockets**: **Tokio-Tungstenite** handles WebSocket streaming connections to both media servers.
- **Serialization**: **Serde** and **Serde JSON** handle DTO parsing. Field mappings must use `#[serde(alias = "...")]` to support capitalization differences between Emby and Jellyfin payloads.
- **Logging**: **Tracing** and **Tracing-Subscriber** handle structured, levels-based diagnostics.

---

## 2. Concurrency & State Management

- **Shared State**: All shared runtime states (metadata caches, synchronization history) are wrapped in `Arc<Mutex<AppState>>`.
- **Lock Contention / Deadlocks**:
  - **Rule**: Never hold a `MutexGuard` across an await point that performs network I/O (`reqwest` requests or WebSocket sends).
  - **Pattern**: Always clone the required parameters from the state, drop the `MutexGuard` explicitly, perform the async network request, and then re-acquire the lock to record the results:
    ```rust
    // Drop lock before async request
    drop(state);
    
    // Perform async I/O
    let result = client.update_progress(...).await;
    
    // Re-acquire lock to write back
    let mut state = state_lock.lock().await;
    state.last_syncs.insert(...);
    ```

---

## 3. Data Integrity & Loop Prevention

- **Bidirectional Sync Protection**: To prevent infinite ping-pong loops between Emby and Jellyfin updates:
  - Cache the last sent position per user and movie key: `(username, provider_id)`.
  - Discard incoming WebSocket updates if the new position is within `sync_threshold_seconds` of the last synced position and occurred within the last 5 seconds.
- **Media Matching**: Always use global identifiers (`IMDb ID` or `TMDb ID`) to match media. Do not rely on server-specific library folder structures or internal database IDs.

---

## 4. Error Handling & Logging

- **Recoverability**: WebSocket stream dropouts must be caught and automatically reconnected in an backoff reconnect loop without crashing the daemon.
- **Error Propagation**: Use `anyhow::Result` with `.context()` to attach readable context to errors during network handshakes or configuration parses.
- **Logging Guidelines**:
  - `INFO`: Connection handshakes, cache load summaries, and synchronization triggers.
  - `WARN`: Recoverable network disconnects, retry intervals.
  - `ERROR`: Failures in API progress updates or configuration parsing.

---

## 5. Code Style & Warnings

- The codebase must compile cleanly with `cargo check` and `cargo build`.
- Avoid unused imports, unused variables, or dead code. Silence intentional dead code with `#[allow(dead_code)]` explicitly.
