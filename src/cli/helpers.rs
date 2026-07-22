use anyhow::Result;
use statesync::sync_force::ForceSyncStatus;
use std::sync::Arc;
use std::time::Duration;
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
                        statesync::state::ServerCache::empty(name.clone()),
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
    println!("StateSync — watched, resume, and favorites across Emby / Jellyfin\n");
    println!("Usage:");
    println!("  statesync [command]\n");
    println!("Commands:");
    println!("  -h, --help       Show this help");
    println!("  -v, --version    Print version");
    println!("  --validate       Check config.json and test each server connection");
    println!("  --reload         Tell the running service to reload config");
    println!("  --tui            Live terminal dashboard (story + status)");
    println!("  --dry-run        Load caches / user mapping check; no play-state writes");
    println!("  --sync-force     Full backfill (played, position, favorites per Settings)");
    println!();
    println!("  Force: phases + skip reasons. Preview without writing:");
    println!("    statesync --sync-force --dry-run");
    println!();
    println!("Common environment:");
    println!("  STATESYNC_BIND                    Listen address (default 0.0.0.0:4601)");
    println!("  STATESYNC_SYNC_THRESHOLD_SECONDS  Ignore near-duplicate progress (default 5)");
    println!("  STATESYNC_FORCE_RATE              Force items/sec, 1..50 (default 5)");
    println!("  STATESYNC_LOG_RETENTION           Activity log lines in memory (default 100)");
    println!("  STATESYNC_ACCEPT_INVALID_CERTS    true = skip TLS verify (self-signed LAN)");
    println!("  STATESYNC_FUZZY_USER_MATCH        true = soft username match (off by default)");
    println!("  STATESYNC_SERVER_<N>_*            Env-based servers (see README)");
    println!("  RUST_LOG                          Log filter (default info)");
    println!("  TZ                                Timezone");
    println!();
    println!("Dashboard: http://<host>:4601  ·  no login");
}

/// Docker / Unraid-friendly default (all interfaces).
pub const DEFAULT_BIND: &str = "0.0.0.0:4601";

pub fn resolve_bind_addr() -> String {
    std::env::var("STATESYNC_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string())
}

/// Dashboard authentication is disabled — always open, no sign-in.
pub fn resolve_web_auth() -> Option<String> {
    None
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

pub async fn drain_ws_handles(handles: Vec<tokio::task::JoinHandle<()>>, timeout: Duration) {
    let drain = async {
        for h in handles {
            let _ = h.await;
        }
    };
    let _ = tokio::time::timeout(timeout, drain).await;
}

pub(super) fn phase_label(phase: Option<&str>) -> &'static str {
    match phase.unwrap_or("").to_ascii_lowercase().as_str() {
        "preparing" => "Preparing",
        "played" => "Played history",
        "favorites" => "Favorites",
        "finishing" => "Finishing",
        "done" => "Done",
        "cancelled" => "Cancelled",
        _ => "Running",
    }
}

pub(super) fn format_force_skip_story(status: &ForceSyncStatus) -> String {
    let sr = &status.skip_reasons;
    let mut bits = Vec::new();
    if sr.already_equal > 0 {
        bits.push(format!("{} already matched", sr.already_equal));
    }
    if sr.no_provider > 0 {
        bits.push(format!(
            "{} no Imdb/Tmdb/Tvdb in server metadata",
            sr.no_provider
        ));
    }
    if sr.no_match > 0 {
        bits.push(format!("{} not in other library", sr.no_match));
    }
    if sr.other > 0 {
        bits.push(format!("{} other", sr.other));
    }
    if bits.is_empty() {
        String::new()
    } else {
        format!(" · skips: {}", bits.join(", "))
    }
}

pub(super) fn print_force_progress(status: &ForceSyncStatus) {
    let phase = phase_label(status.phase.as_deref());
    let user = status.current_user.as_deref().unwrap_or("…");
    let scope = if status.scope.is_empty() {
        "default".to_string()
    } else {
        status.scope.join("+")
    };
    println!(
        "  [{}] {}/{} · pushed {} · skipped {} · failed {} · user={} · scope={}{}",
        phase,
        status.processed,
        status.total_pairs.max(1),
        status.succeeded,
        status.skipped,
        status.failed,
        user,
        scope,
        format_force_skip_story(status)
    );
}
