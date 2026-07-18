mod config;
mod client;
mod state;
mod websocket;

use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::{Result, Context};
use tracing::info;
use tracing_subscriber;

use crate::config::Config;
use crate::client::MediaClient;
use crate::state::{AppState, init_server_cache};
use crate::websocket::{make_ws_url, handle_websocket_loop};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting Emby-Jellyfin Playstate Sync Sidecar...");

    let config_path = "config.json";
    let config_data = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read configuration file: {}", config_path))?;
    
    let config: Config = serde_json::from_str(&config_data)
        .context("Failed to parse configuration file")?;

    info!("Connecting to Emby: {}", config.emby.url);
    let emby_client = Arc::new(MediaClient::new(config.emby.url.clone(), config.emby.api_key.clone(), true));
    
    info!("Connecting to Jellyfin: {}", config.jellyfin.url);
    let jellyfin_client = Arc::new(MediaClient::new(config.jellyfin.url.clone(), config.jellyfin.api_key.clone(), false));

    // Initialize Caches
    info!("Initializing Emby metadata cache...");
    let emby_cache = init_server_cache(&emby_client).await
        .context("Failed to initialize Emby metadata cache")?;
    info!("Emby cache loaded: {} users, {} matched media items.", emby_cache.users.len(), emby_cache.id_to_providers.len());

    info!("Initializing Jellyfin metadata cache...");
    let jellyfin_cache = init_server_cache(&jellyfin_client).await
        .context("Failed to initialize Jellyfin metadata cache")?;
    info!("Jellyfin cache loaded: {} users, {} matched media items.", jellyfin_cache.users.len(), jellyfin_cache.id_to_providers.len());

    let app_state = Arc::new(Mutex::new(AppState::new(emby_cache, jellyfin_cache)));

    let emby_ws_url = make_ws_url(&config.emby.url, &config.emby.api_key, true);
    let jellyfin_ws_url = make_ws_url(&config.jellyfin.url, &config.jellyfin.api_key, false);

    // Launch Emby WS listener
    let emby_ws_url_clone = emby_ws_url.clone();
    let jellyfin_client_clone = jellyfin_client.clone();
    let state_clone = app_state.clone();
    let config_clone = config.clone();
    tokio::spawn(async move {
        handle_websocket_loop(
            &emby_ws_url_clone,
            true, // source is emby
            jellyfin_client_clone,
            state_clone,
            config_clone,
        ).await;
    });

    // Launch Jellyfin WS listener
    let jellyfin_ws_url_clone = jellyfin_ws_url.clone();
    let emby_client_clone = emby_client.clone();
    let state_clone2 = app_state.clone();
    let config_clone2 = config.clone();
    tokio::spawn(async move {
        handle_websocket_loop(
            &jellyfin_ws_url_clone,
            false, // source is jellyfin
            emby_client_clone,
            state_clone2,
            config_clone2,
        ).await;
    });

    info!("Playstate Sync Sidecar started. Press Ctrl+C to stop.");
    tokio::signal::ctrl_c().await?;
    info!("Stopping Playstate Sync Sidecar.");
    Ok(())
}
