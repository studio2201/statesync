#![allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::collapsible_if,
    clippy::single_match
)]

use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, broadcast, mpsc};
use tracing::{error, info, warn};

use statesync::{
    state::AppState,
    web::{WebServerState, create_router},
    websocket::{handle_websocket_loop, make_ws_url},
};

mod cli;

use cli::{resolve_bind_addr, resolve_web_auth, install_shutdown_handler};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        let cmd = &args[1];
        match cmd.as_str() {
            "--version" | "-v" => {
                println!("statesync version {}", VERSION);
                return Ok(());
            }
            "--help" | "-h" => {
                cli::print_help();
                return Ok(());
            }
            "--validate" => {
                return cli::validate_config().await;
            }
            "--reload" => {
                return cli::trigger_reload().await;
            }
            "--tui" => {
                let bind_addr = resolve_bind_addr();
                let web_auth = resolve_web_auth();
                return cli::run_tui(&bind_addr, web_auth.as_deref()).await;
            }
            "--dry-run" => {
                init_logging();
                return cli::dry_run().await;
            }
            "--sync-force" => {
                return cli::run_sync_force_cli(&args).await;
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

    init_logging();
    info!("Starting statesync v{} sidecar...", VERSION);
    let bind_addr = resolve_bind_addr();
    let web_auth = resolve_web_auth();

    if !statesync::config::is_loopback_bind(&bind_addr) && web_auth.is_none() {
        eprintln!(
            "FATAL: Refusing to bind to non-loopback address '{}' without STATESYNC_WEB_AUTH configuration. \
             To bind to public interfaces, you MUST set STATESYNC_WEB_AUTH=bearer:<token>.",
            bind_addr
        );
        std::process::exit(1);
    }

    if web_auth.is_some() {
        eprintln!("STATESYNC_WEB_AUTH is set; bearer token required for /api/* endpoints.");
    } else {
        eprintln!(
            "STATESYNC_WEB_AUTH not set; /api/* endpoints are open on {}. \
             Set STATESYNC_WEB_AUTH=bearer:<token> to require authentication.",
            bind_addr
        );
    }

    let app_state = Arc::new(Mutex::new(AppState::new(vec![])));
    let (reload_tx, mut reload_rx) = mpsc::channel::<()>(5);
    let started_at = chrono::Utc::now().to_rfc3339();
    let started_instant = std::time::Instant::now();

    let web_state = Arc::new(WebServerState {
        app_state: app_state.clone(),
        reload_tx: reload_tx.clone(),
        bind_addr: bind_addr.clone(),
        web_auth: web_auth.clone(),
        version: VERSION.to_string(),
        started_at: started_at.clone(),
        started_instant,
    });
    let app = create_router(web_state);

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("Failed to bind web UI server to {}", bind_addr))?;

    info!("Web UI listening on http://{}", bind_addr);

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            error!("Web server error: {}", e);
        }
    });

    let mut shutdown_signal = install_shutdown_handler();

    loop {
        info!("Loading configuration...");
        let config_res = statesync::config::load_or_create_default();

        let config = match config_res {
            Ok(cfg) => cfg,
            Err(e) => {
                warn!(
                    "Configuration load warning: {}. Web UI is active. Waiting for settings updates...",
                    e
                );
                tokio::select! {
                    _ = reload_rx.recv() => continue,
                    _ = &mut shutdown_signal => {
                        info!("Shutdown signal received, exiting.");
                        return Ok(());
                    }
                }
            }
        };

        if config.servers.is_empty() {
            info!(
                "No servers configured. Add one via the web UI at http://{}/ or by editing {}",
                bind_addr,
                statesync::config::get_config_path()
            );
        }

        for s in &config.servers {
            if s.url.starts_with("http://") && !s.allow_insecure_http {
                warn!(
                    "Server '{}' uses plaintext HTTP; consider HTTPS to protect API keys in transit.",
                    s.name
                );
            }
        }

        {
            let mut state = app_state.lock().await;
            state.websocket_statuses = vec!["Offline".to_string(); config.servers.len()];
        }

        let (clients, caches) = match cli::init_clients_parallel(&config, &app_state).await {
            Ok(v) => v,
            Err(e) => {
                warn!(
                    "Failed to initialize clients/caches: {}. Retrying on reload.",
                    e
                );
                tokio::select! {
                    _ = reload_rx.recv() => continue,
                    _ = &mut shutdown_signal => return Ok(()),
                }
            }
        };

        {
            let mut state = app_state.lock().await;
            state.caches = caches;
        }

        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        for (i, s) in config.servers.iter().enumerate() {
            let ws_url = make_ws_url(&s.url, &s.api_key, s.is_emby);
            if ws_url.starts_with("ws://") {
                warn!(
                    "Server '{}' WebSocket URL is plaintext (ws://); consider HTTPS.",
                    s.name
                );
            }
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

        tokio::select! {
            _ = reload_rx.recv() => {
                info!("Reload signal received. Shutting down active synchronization loops...");
                let _ = shutdown_tx.send(());
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            _ = &mut shutdown_signal => {
                info!("Shutdown signal received, exiting.");
                let _ = shutdown_tx.send(());
                tokio::time::sleep(Duration::from_millis(500)).await;
                return Ok(());
            }
        }
    }
}

fn init_logging() {
    use tracing_subscriber::{EnvFilter, fmt};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(filter).try_init();
}
