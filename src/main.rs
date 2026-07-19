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
    if !args.is_empty()
        && (args[0].ends_with("sh") || args[0].ends_with("bash") || args.iter().any(|a| a == "-c"))
    {
        println!("\n==================================================");
        println!("       ⚠️  STATESYNC SECURE SHELL TERMINAL  ⚠️");
        println!("==================================================");
        println!("Welcome! Actually, no. This is a secure container.");
        println!("There is no shell, no utilities, and no backdoor.");
        println!("We will not be helping you compromise this system.");
        println!("==================================================");
        println!("\nPress [ENTER] to close this window...");

        let mut input = String::new();
        match std::io::stdin().read_line(&mut input) {
            Ok(0) | Err(_) => {
                // EOF or closed stdin. Keep window open until closed manually.
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(3600));
                }
            }
            _ => {}
        }
        std::process::exit(0);
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
            let client = Arc::new(MediaClient::new(
                s.url.clone(),
                s.api_key.clone(),
                s.is_emby,
            ));

            info!("Initializing metadata cache for '{}'...", s.name);
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
