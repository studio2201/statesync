mod config;
mod client;
mod state;
mod websocket;
mod web;
mod sync;

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc, broadcast};
use anyhow::{Result, Context};
use tracing::{info, warn, error};
use tracing_subscriber;

use crate::config::Config;
use crate::client::MediaClient;
use crate::state::{AppState, init_server_cache};
use crate::websocket::{make_ws_url, handle_websocket_loop};
use crate::web::{WebServerState, create_router};

#[tokio::main]
async fn main() -> Result<()> {
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
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8754").await
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
                warn!("Configuration load warning: {}. Web UI is active. Waiting for settings updates...", e);
                // Wait for a reload signal from the Web UI before trying again
                let _ = reload_rx.recv().await;
                continue;
            }
        };

        let mut clients = Vec::new();
        let mut caches = Vec::new();

        // Initialize all clients and cache metadata
        let mut init_success = true;
        for s in &config.servers {
            {
                let mut state = app_state.lock().await;
                state.log_event("info", &format!("Connecting to server '{}' ({})", s.name, s.url));
                state.log_event("info", &format!("Initializing metadata cache for '{}'...", s.name));
            }
            info!("Connecting to server '{}' ({})", s.name, s.url);
            let client = Arc::new(MediaClient::new(s.url.clone(), s.api_key.clone(), s.is_emby));
            
            info!("Initializing metadata cache for '{}'...", s.name);
            match init_server_cache(&s.name, &client).await {
                Ok(cache) => {
                    info!("Cache loaded for '{}': {} users, {} matched media items.", s.name, cache.users.len(), cache.id_to_providers.len());
                    {
                        let mut state = app_state.lock().await;
                        state.log_event("success", &format!("Cache loaded for '{}': {} users, {} media", s.name, cache.users.len(), cache.id_to_providers.len()));
                    }
                    clients.push(client);
                    caches.push(cache);
                }
                Err(e) => {
                    error!("Failed to initialize cache for server '{}': {}. Re-trying on config change...", s.name, e);
                    {
                        let mut state = app_state.lock().await;
                        state.log_event("error", &format!("Failed to initialize cache for server '{}': {}", s.name, e));
                    }
                    init_success = false;
                    break;
                }
            }
        }

        if !init_success {
            // Wait for next config change
            let _ = reload_rx.recv().await;
            continue;
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
                ).await;
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
