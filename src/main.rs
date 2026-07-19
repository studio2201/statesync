#![allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::collapsible_if,
    clippy::single_match
)]

mod client;
mod config;
mod dashboard;
mod state;
mod sync;
mod web;
mod web_api;
mod websocket;

use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, broadcast, mpsc};
use tracing::{error, info, warn};

use crate::client::MediaClient;
use crate::config::Config;
use crate::state::{AppState, init_server_cache};
use crate::web::{WebServerState, create_router};
use crate::websocket::{handle_websocket_loop, make_ws_url};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        let cmd = &args[1];
        match cmd.as_str() {
            "--version" | "-v" => {
                println!("statesync version {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            "--validate" => {
                return validate_config().await;
            }
            "--reload" => {
                return trigger_reload().await;
            }
            "--tui" => {
                return run_tui().await;
            }
            _ => {
                eprintln!(
                    "Unknown argument: {}. Use --help to see available commands.",
                    cmd
                );
                std::process::exit(1);
            }
        }
    }

    tracing_subscriber::fmt::init();
    info!("Starting statesync Sidecar...");

    // Shared thread-safe state container. Starts empty.
    let app_state = Arc::new(Mutex::new(AppState::new(vec![])));

    // Reload channel to notify the sync loop to rebuild caches & restart websocket threads
    let (reload_tx, mut reload_rx) = mpsc::channel::<()>(5);

    // Build Axum web router
    let web_state = Arc::new(WebServerState {
        app_state: app_state.clone(),
        reload_tx: reload_tx.clone(),
    });
    let app = create_router(web_state);

    // Spawn the HTTP server on port 8754
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8754")
        .await
        .context("Failed to bind web UI server to port 8754")?;

    info!("Web UI Dashboard listening on http://0.0.0.0:8754");

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            error!("Web server error: {}", e);
        }
    });

    // Orchestrator loop
    loop {
        info!("Loading configuration...");
        let config_res = Config::load();

        let config = match config_res {
            Ok(cfg) => cfg,
            Err(e) => {
                warn!(
                    "Configuration load warning: {}. Web UI is active. Waiting for settings updates...",
                    e
                );
                // Wait for a reload signal from the Web UI before trying again
                let _ = reload_rx.recv().await;
                continue;
            }
        };

        let mut clients = Vec::new();
        let mut caches = Vec::new();

        // Initialize all clients and cache metadata
        for s in &config.servers {
            {
                let mut state = app_state.lock().await;
                state.log_event(
                    "info",
                    &format!("Connecting to server '{}' ({})", s.name, s.url),
                );
                state.log_event(
                    "info",
                    &format!("Initializing metadata cache for '{}'...", s.name),
                );
            }
            info!("Connecting to server '{}' ({})", s.name, s.url);
            info!("Initializing metadata cache for '{}'...", s.name);
            let client = Arc::new(MediaClient::new(
                s.url.clone(),
                s.api_key.clone(),
                s.is_emby,
            ));

            match init_server_cache(&s.name, &client).await {
                Ok(cache) => {
                    info!(
                        "Cache loaded for '{}': {} users, {} matched media items.",
                        s.name,
                        cache.users.len(),
                        cache.id_to_providers.len()
                    );
                    app_state.lock().await.log_event(
                        "success",
                        &format!(
                            "Cache loaded for '{}': {} users, {} media",
                            s.name,
                            cache.users.len(),
                            cache.id_to_providers.len()
                        ),
                    );
                    clients.push(client);
                    caches.push(cache);
                }
                Err(e) => {
                    warn!(
                        "Failed to initialize cache for server '{}' on startup: {}. Retrying in background...",
                        s.name, e
                    );
                    app_state.lock().await.log_event(
                        "warn",
                        &format!(
                            "Offline server '{}' on startup. Retrying in background...",
                            s.name
                        ),
                    );
                    clients.push(client);
                    caches.push(crate::state::ServerCache {
                        name: s.name.clone(),
                        users: std::collections::HashMap::new(),
                        imdb_to_id: std::collections::HashMap::new(),
                        tmdb_to_id: std::collections::HashMap::new(),
                        id_to_providers: std::collections::HashMap::new(),
                    });
                }
            }
        }

        // Update shared AppState for the Web UI status report
        {
            let mut state = app_state.lock().await;
            let count = caches.len();
            state.caches = caches;
            state.websocket_statuses = vec!["Offline".to_string(); count];
        }

        // Create broadcast shutdown channel to terminate the current websocket connection threads
        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        // Spawn websocket connection loops
        for (i, s) in config.servers.iter().enumerate() {
            let ws_url = make_ws_url(&s.url, &s.api_key, s.is_emby);
            let state_clone = app_state.clone();
            let config_clone = config.clone();
            let shutdown_rx = shutdown_tx.subscribe();

            let mut target_clients = Vec::new();
            for (j, client) in clients.iter().enumerate() {
                if j != i {
                    target_clients.push((j, client.clone()));
                }
            }

            let source_client = clients[i].clone();
            tokio::spawn(async move {
                handle_websocket_loop(
                    i,
                    &ws_url,
                    source_client,
                    target_clients,
                    state_clone,
                    config_clone,
                    shutdown_rx,
                )
                .await;
            });
        }

        info!("All synchronization loops started.");

        // Block here until a reload signal is sent from the Web UI
        let _ = reload_rx.recv().await;
        info!("Reload signal received. Shutting down active synchronization loops...");

        // Terminate all current websocket tasks
        let _ = shutdown_tx.send(());

        // Wait brief moment for threads to wind down
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

fn print_help() {
    println!("statesync - Emby/Jellyfin Playstate Sync Bridge\n");
    println!("Usage:");
    println!("  statesync [command]\n");
    println!("Commands:");
    println!("  -h, --help       Show this help menu");
    println!("  -v, --version    Print application version");
    println!("  --validate       Validate config.json and test server connections");
    println!("  --reload         Trigger reload of config.json on the running service");
    println!("  --tui            Launch the interactive terminal dashboard");
}

async fn trigger_reload() -> Result<()> {
    println!("Sending reload signal to active statesync service...");
    let client = reqwest::Client::new();
    match client.post("http://127.0.0.1:8754/api/reload").send().await {
        Ok(resp) => {
            if resp.status() == reqwest::StatusCode::OK {
                println!("✓ Reload signal successfully sent. Active service is reloading config.");
                Ok(())
            } else {
                let err_text = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                println!("✗ Active service returned error: {}", err_text);
                std::process::exit(1);
            }
        }
        Err(e) => {
            println!(
                "✗ Failed to connect to active statesync service on port 8754: {}",
                e
            );
            println!("Make sure the statesync background container/service is running.");
            std::process::exit(1);
        }
    }
}

async fn validate_config() -> Result<()> {
    println!("=== CONFIGURATION VALIDATION ===");
    let config = match Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            println!("✗ Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };
    println!("✓ Config file parsed successfully.");
    println!("Found {} configured server(s).", config.servers.len());
    println!(
        "Sync threshold: {} seconds.\n",
        config.sync_threshold_seconds
    );

    let mut all_ok = true;
    for s in &config.servers {
        println!("Checking connection to '{}' ({})...", s.name, s.url);
        let client = MediaClient::new(s.url.clone(), s.api_key.clone(), s.is_emby);
        match init_server_cache(&s.name, &client).await {
            Ok(cache) => {
                println!(
                    "  ✓ Connected successfully! Loaded {} users, {} media items.",
                    cache.users.len(),
                    cache.id_to_providers.len()
                );
            }
            Err(e) => {
                println!("  ✗ Connection failed: {}", e);
                all_ok = false;
            }
        }
    }

    if all_ok {
        println!("\n✓ All checks passed! Configuration is valid.");
        Ok(())
    } else {
        println!("\n✗ Some checks failed. Please check your network and API keys.");
        std::process::exit(1);
    }
}

async fn run_tui() -> Result<()> {
    let client = reqwest::Client::new();
    let url = "http://127.0.0.1:8754/api/status";

    loop {
        match client.get(url).send().await {
            Ok(resp) => {
                if resp.status() == reqwest::StatusCode::OK {
                    if let Ok(status) = resp.json::<serde_json::Value>().await {
                        draw_tui_from_json(&status);
                    }
                }
            }
            Err(e) => {
                print!("\x1B[2J\x1B[H");
                println!(
                    "✗ Cannot connect to statesync background service on port 8754: {}",
                    e
                );
                println!("Make sure the statesync background container is running.");
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

fn draw_tui_from_json(status: &serde_json::Value) {
    // Clear screen and move cursor to top-left
    print!("\x1B[2J\x1B[H");

    println!(
        "\x1B[1m\x1B[36m┌──────────────────────────────────────────────────────────────────────────────┐\x1B[0m"
    );
    println!(
        "\x1B[1m\x1B[36m│                       STATESYNC TERMINAL DASHBOARD                           │\x1B[0m"
    );
    println!(
        "\x1B[1m\x1B[36m│                       Version: v{:<44} │\x1B[0m",
        env!("CARGO_PKG_VERSION")
    );
    println!(
        "\x1B[1m\x1B[36m└──────────────────────────────────────────────────────────────────────────────┘\x1B[0m"
    );

    println!("\x1B[1m\x1B[33m[ SERVERS AND STATUS ]\x1B[0m");
    if let Some(servers) = status.get("servers").and_then(|v| v.as_array()) {
        if servers.is_empty() {
            println!("  No servers configured or loading caches...");
        } else {
            for s in servers {
                let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
                let ws_status = s
                    .get("websocket_status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Offline");
                let users_count = s.get("users_count").and_then(|v| v.as_u64()).unwrap_or(0);
                let media_count = s.get("media_count").and_then(|v| v.as_u64()).unwrap_or(0);

                let status_color = if ws_status == "Connected" {
                    "\x1B[32m"
                } else {
                    "\x1B[31m"
                };
                println!(
                    "  • \x1B[1m{:<12}\x1B[0m: {}{:<10}\x1B[0m ({} Users | {} Cached Media Items)",
                    name, status_color, ws_status, users_count, media_count
                );
            }
        }
    } else {
        println!("  Loading server status details...");
    }
    println!();

    println!("\x1B[1m\x1B[33m[ ACTIVE SESSIONS ]\x1B[0m");
    if let Some(sessions) = status.get("active_sessions").and_then(|v| v.as_array()) {
        if sessions.is_empty() {
            println!("  No active playback streams detected.");
        } else {
            for sess in sessions {
                let server = sess
                    .get("server")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let user = sess
                    .get("user")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let item = sess
                    .get("item")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let position = sess.get("position").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let is_paused = sess
                    .get("is_paused")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                let play_icon = if !is_paused {
                    "\x1B[32m▶ Playing\x1B[0m"
                } else {
                    "\x1B[33m⏸ Paused\x1B[0m"
                };
                println!(
                    "  • \x1B[1m{:<8}\x1B[0m - User \x1B[1m{:<12}\x1B[0m: {} - progress: {:.1}s ({})",
                    server, user, item, position, play_icon
                );
            }
        }
    } else {
        println!("  Reading active sessions...");
    }
    println!();

    println!("\x1B[1m\x1B[33m[ RECENT ACTIVITY LOGS ]\x1B[0m");
    if let Some(logs) = status.get("sync_logs").and_then(|v| v.as_array()) {
        if logs.is_empty() {
            println!("  No logs recorded yet.");
        } else {
            for entry in logs.iter().take(12) {
                let timestamp = entry
                    .get("timestamp")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let level = entry
                    .get("level")
                    .and_then(|v| v.as_str())
                    .unwrap_or("info");
                let message = entry.get("message").and_then(|v| v.as_str()).unwrap_or("");

                let color = match level {
                    "success" => "\x1B[32m", // Green
                    "warn" => "\x1B[33m",    // Yellow
                    "error" => "\x1B[31m",   // Red
                    _ => "\x1B[37m",         // White
                };
                println!("  [{}] {}{}\x1B[0m", timestamp, color, message);
            }
        }
    } else {
        println!("  Reading activity logs...");
    }
    println!("\n\x1B[90m(Press Ctrl+C to close and exit dashboard)\x1B[0m");

    // Flush stdout to make sure the terminal updates immediately
    use std::io::Write;
    let _ = std::io::stdout().flush();
}
