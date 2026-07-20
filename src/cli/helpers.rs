use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

use statesync::{
    client::MediaClient,
    config::{Config, redacted_url},
    state::{AppState, init_server_cache},
};

pub async fn init_clients_parallel(
    config: &Config,
    app_state: &Arc<Mutex<AppState>>,
) -> Result<(Vec<Arc<MediaClient>>, Vec<statesync::state::ServerCache>)> {
    let mut init_futures = Vec::new();
    for (i, s) in config.servers.iter().enumerate() {
        let name = s.name.clone();
        let url = s.url.clone();
        let api_key = s.api_key.clone();
        let is_emby = s.is_emby;
        let app_state = app_state.clone();
        init_futures.push(tokio::spawn(async move {
            {
                let mut state = app_state.lock().await;
                if i < state.websocket_statuses.len() {
                    state.websocket_statuses[i] = "Validating".to_string();
                }
                state.log_event(
                    "info",
                    &format!("Connecting to server '{}' ({})", name, redacted_url(&url)),
                );
                state.log_event(
                    "info",
                    &format!("Initializing metadata cache for '{}'...", name),
                );
            }
            info!("Connecting to server '{}' ({})", name, redacted_url(&url));
            info!("Initializing metadata cache for '{}'...", name);
            let client = Arc::new(MediaClient::new(url.clone(), api_key.clone(), is_emby));
            {
                let mut state = app_state.lock().await;
                if i < state.websocket_statuses.len() {
                    state.websocket_statuses[i] = "Scanning".to_string();
                }
            }
            match init_server_cache(&name, &client).await {
                Ok(cache) => {
                    info!(
                        "Cache loaded for '{}': {} users, {} matched media items.",
                        name,
                        cache.users.len(),
                        cache.id_to_providers.len()
                    );
                    app_state.lock().await.log_event(
                        "success",
                        &format!(
                            "Cache loaded for '{}': {} users, {} media",
                            name,
                            cache.users.len(),
                            cache.id_to_providers.len()
                        ),
                    );
                    Some((client, cache))
                }
                Err(e) => {
                    warn!(
                        "Failed to initialize cache for server '{}' on startup: {}. Retrying in background...",
                        name, e
                    );
                    let mut state = app_state.lock().await;
                    if i < state.websocket_statuses.len() {
                        state.websocket_statuses[i] = "Error".to_string();
                    }
                    state.log_event(
                        "error",
                        &format!("Failed to connect / init cache for '{}': {}", name, e),
                    );
                    drop(state);
                    Some((
                        client,
                        statesync::state::ServerCache {
                            name: name.clone(),
                            users: std::collections::HashMap::new(),
                            imdb_to_id: std::collections::HashMap::new(),
                            tmdb_to_id: std::collections::HashMap::new(),
                            id_to_providers: std::collections::HashMap::new(),
                        },
                    ))
                }
            }
        }));
    }

    let mut clients = Vec::new();
    let mut caches = Vec::new();
    for fut in init_futures {
        if let Ok(Some((client, cache))) = fut.await {
            clients.push(client);
            caches.push(cache);
        }
    }

    Ok((clients, caches))
}

pub fn print_help() {
    println!("statesync - Emby/Jellyfin Playstate Sync Bridge\n");
    println!("Usage:");
    println!("  statesync [command]\n");
    println!("Commands:");
    println!("  -h, --help       Show this help menu");
    println!("  -v, --version    Print application version");
    println!("  --validate       Validate config.json and test server connections");
    println!("  --reload         Trigger reload of config.json on the running service");
    println!("  --tui            Launch the interactive terminal dashboard");
    println!("  --dry-run        Load config, init caches, run mapping dry-run; exit 0/1");
    println!(
        "  --sync-force     Force a full played-items sync between all servers (see --sync-force --help)"
    );
    println!();
    println!("Environment Variables:");
    println!("  STATESYNC_BIND                 Listen address (default: 127.0.0.1:4601)");
    println!(
        "                                  Refuses non-loopback binds without STATESYNC_WEB_AUTH."
    );
    println!("  STATESYNC_WEB_AUTH             'bearer:<token>' required for non-loopback binds.");
    println!(
        "  STATESYNC_ALLOW_INSECURE_HTTP  Set 'true' to permit http:// URLs to upstream servers."
    );
    println!("  STATESYNC_SERVER_<N>_*         Per-server env-var config (see README).");
    println!("  STATESYNC_SYNC_THRESHOLD_SECONDS   Sync threshold (default 5).");
    println!("  STATESYNC_HTTP_RETRY           'off' to disable retry with backoff.");
    println!("  STATESYNC_LOG_RETENTION        Number of log entries kept in memory (default 30).");
    println!("  STATESYNC_FORCE_RATE           Items/sec during --sync-force, 1..50 (default 5).");
    println!("  RUST_LOG                       tracing log filter (overrides default 'info').");
    println!("  TZ                             Container timezone.");
}

pub const DEFAULT_BIND: &str = "0.0.0.0:4601";

pub fn resolve_bind_addr() -> String {
    std::env::var("STATESYNC_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string())
}

pub fn resolve_web_auth() -> Option<String> {
    std::env::var("STATESYNC_WEB_AUTH").ok().and_then(|v| {
        let v = v.trim().to_string();
        if v.is_empty() || v.eq_ignore_ascii_case("none") {
            None
        } else {
            Some(v)
        }
    })
}

pub fn install_shutdown_handler() -> tokio::sync::oneshot::Receiver<()> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let mut sigterm =
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(s) => s,
                Err(_) => return,
            };
        let mut sigint =
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()) {
                Ok(s) => s,
                Err(_) => return,
            };
        tokio::select! {
            _ = sigterm.recv() => tracing::info!("SIGTERM received."),
            _ = sigint.recv() => tracing::info!("SIGINT received."),
        }
        let _ = tx.send(());
    });
    rx
}
