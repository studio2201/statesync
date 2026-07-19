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
    backfill::{self, BackfillContext, BackfillOptions, Direction, MergePolicy, Scope},
    client::MediaClient,
    config::{Config, is_loopback_bind, redacted_url},
    state::{AppState, init_server_cache},
    web::{WebServerState, create_router},
    websocket::{handle_websocket_loop, make_ws_url},
};

const DEFAULT_BIND: &str = "127.0.0.1:8754";

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn resolve_bind_addr() -> String {
    std::env::var("STATESYNC_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string())
}

fn resolve_web_auth() -> Option<String> {
    std::env::var("STATESYNC_WEB_AUTH").ok().and_then(|v| {
        let v = v.trim().to_string();
        if v.is_empty() || v.eq_ignore_ascii_case("none") {
            None
        } else {
            Some(v)
        }
    })
}

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
            "--dry-run" => {
                return dry_run().await;
            }
            "--backfill" => {
                return run_backfill_cli(&args).await;
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

    if !is_loopback_bind(&bind_addr) && web_auth.is_none() {
        eprintln!(
            "Refusing to start: bind address '{}' is not loopback and no STATESYNC_WEB_AUTH is configured.",
            bind_addr
        );
        eprintln!("Set STATESYNC_WEB_AUTH=bearer:<token> or bind to 127.0.0.1 (the default).");
        std::process::exit(2);
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

    if web_auth.is_some() {
        info!("Web UI listening on http://{} (auth required)", bind_addr);
    } else {
        info!("Web UI listening on http://{} (loopback only)", bind_addr);
    }

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

        let (clients, caches) = match init_clients_parallel(&config, &app_state).await {
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
            let count = caches.len();
            state.caches = caches;
            state.websocket_statuses = vec!["Offline".to_string(); count];
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

async fn init_clients_parallel(
    config: &Config,
    app_state: &Arc<Mutex<AppState>>,
) -> Result<(Vec<Arc<MediaClient>>, Vec<statesync::state::ServerCache>)> {
    let mut init_futures = Vec::new();
    for s in &config.servers {
        let name = s.name.clone();
        let url = s.url.clone();
        let api_key = s.api_key.clone();
        let is_emby = s.is_emby;
        let app_state = app_state.clone();
        init_futures.push(tokio::spawn(async move {
            {
                let mut state = app_state.lock().await;
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
                    app_state.lock().await.log_event(
                        "warn",
                        &format!("Offline server '{}' on startup. Retrying in background...", name),
                    );
                    Some((
                        client,
                        statesync::state::ServerCache {
                            name: name.clone(),
                            users: std::collections::HashMap::new(),
                            imdb_to_id: std::collections::HashMap::new(),
                            tmdb_to_id: std::collections::HashMap::new(),
                            id_to_providers: std::collections::HashMap::new(),
                            last_negative_cache: None,
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

fn init_logging() {
    use tracing_subscriber::{EnvFilter, fmt};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(filter).try_init();
}

fn install_shutdown_handler() -> tokio::sync::oneshot::Receiver<()> {
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
            _ = sigterm.recv() => info!("SIGTERM received."),
            _ = sigint.recv() => info!("SIGINT received."),
        }
        let _ = tx.send(());
    });
    rx
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
    println!("  --dry-run        Load config, init caches, run mapping dry-run; exit 0/1");
    println!("  --backfill       Run historical watch-state backfill (see --backfill --help)");
    println!();
    println!("Environment Variables:");
    println!("  STATESYNC_BIND                 Listen address (default: 127.0.0.1:8754)");
    println!(
        "                                  Refuses non-loopback binds without STATESYNC_WEB_AUTH."
    );
    println!("  STATESYNC_WEB_AUTH             'bearer:<token>' required for non-loopback binds.");
    println!(
        "  STATESYNC_ALLOW_INSECURE_HTTP  Set 'true' to permit http:// URLs to upstream servers."
    );
    println!("  STATESYNC_SERVER_<N>_*         Per-server env-var config (see README).");
    println!("  STATESYNC_SYNC_THRESHOLD_SECONDS   Sync threshold (default 5).");
    println!("  STATESYNC_LOG_FORMAT           'text' (default) or 'json'.");
    println!("  STATESYNC_HTTP_RETRY           'off' to disable retry with backoff.");
    println!("  STATESYNC_MAX_SYNC_SPAWNS      Max concurrent sync tasks per source (default 8).");
    println!("  STATESYNC_LOG_RETENTION        Number of log entries kept in memory (default 30).");
    println!(
        "  STATESYNC_BACKFILL_DIRECTION   emby-to-jellyfin | jellyfin-to-emby | both (default)."
    );
    println!("  STATESYNC_BACKFILL_MERGE       max (default) | source-wins | newest.");
    println!("  STATESYNC_BACKFILL_SCOPE       played | resumable | all (default).");
    println!("  STATESYNC_BACKFILL_RATE        Items/sec, 1..50 (default 5).");
    println!(
        "  STATESYNC_BACKFILL_ON_START    'true' to run backfill automatically on daemon start."
    );
    println!("  RUST_LOG                       tracing log filter (overrides default 'info').");
    println!("  TZ                             Container timezone.");
}

fn parse_backfill_args(args: &[String]) -> BackfillOptions {
    let mut opts = BackfillOptions::from_env_or_default();
    let mut i = 2;
    while i < args.len() {
        let a = &args[i];
        if a == "--force" {
            opts.force = true;
        } else if let Some(v) = a.strip_prefix("--direction=") {
            opts.direction = match v {
                "emby-to-jellyfin" => Direction::EmbyToJellyfin,
                "jellyfin-to-emby" => Direction::JellyfinToEmby,
                _ => Direction::Both,
            };
        } else if let Some(v) = a.strip_prefix("--merge=") {
            opts.merge = match v {
                "max" => MergePolicy::Max,
                "source-wins" => MergePolicy::SourceWins,
                "newest" => MergePolicy::Newest,
                _ => opts.merge,
            };
        } else if let Some(v) = a.strip_prefix("--scope=") {
            opts.scope = match v {
                "played" => Scope::Played,
                "resumable" => Scope::Resumable,
                _ => Scope::All,
            };
        } else if let Some(v) = a.strip_prefix("--rate=") {
            if let Ok(n) = v.parse::<u32>() {
                opts.rate = n.clamp(1, 50);
            }
        } else if a == "--help" || a == "-h" {
            println!(
                "Usage: statesync --backfill [--force] [--direction=...] [--merge=...] [--scope=...] [--rate=N]"
            );
            println!();
            println!("Options:");
            println!(
                "  --force                    Ignore last_syncs cache; re-push all items (uses source-wins)."
            );
            println!(
                "  --direction=<d>            emby-to-jellyfin | jellyfin-to-emby | both (default both)."
            );
            println!("  --merge=<m>                max | source-wins | newest (default max).");
            println!("  --scope=<s>                played | resumable | all (default all).");
            println!("  --rate=<n>                 Items/sec, 1..50 (default 5).");
            std::process::exit(0);
        } else {
            eprintln!("Unknown --backfill argument: {}", a);
            std::process::exit(2);
        }
        i += 1;
    }
    opts
}

async fn run_backfill_cli(args: &[String]) -> Result<()> {
    init_logging();
    let opts = parse_backfill_args(args);
    let config = Config::load()?;
    if config.servers.is_empty() {
        eprintln!("No servers configured.");
        std::process::exit(1);
    }
    let server_names: Vec<String> = config.servers.iter().map(|s| s.name.clone()).collect();
    let mut clients = Vec::new();
    let mut caches = Vec::new();
    for s in &config.servers {
        let client = Arc::new(MediaClient::new(
            s.url.clone(),
            s.api_key.clone(),
            s.is_emby,
        ));
        match init_server_cache(&s.name, &client).await {
            Ok(c) => {
                clients.push(client);
                caches.push(c);
            }
            Err(e) => {
                eprintln!("Failed to init cache for '{}': {}", s.name, e);
                std::process::exit(1);
            }
        }
    }
    let state = Arc::new(Mutex::new(AppState::new(caches)));
    let tracker = state.lock().await.backfill.clone();

    println!(
        "Starting backfill: direction={:?}, merge={:?}, scope={:?}, force={}, rate={}/s",
        opts.direction, opts.merge, opts.scope, opts.force, opts.rate
    );
    let ctx = BackfillContext {
        config,
        clients,
        state: state.clone(),
        tracker: tracker.clone(),
        options: opts,
        server_names,
    };
    let last_print = Arc::new(Mutex::new(std::time::Instant::now()));
    let printer = {
        let tracker = tracker.clone();
        let last_print = last_print.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
                let status = tracker.status.lock().await.clone();
                if status.state != backfill::BackfillState::Running {
                    if status.state != backfill::BackfillState::Idle {
                        println!(
                            "[{:?}] done: processed={} succeeded={} skipped={} failed={}",
                            status.state,
                            status.processed,
                            status.succeeded,
                            status.skipped,
                            status.failed
                        );
                    }
                    break;
                }
                let now = std::time::Instant::now();
                let mut last = last_print.lock().await;
                if now.duration_since(*last).as_secs() >= 2 {
                    println!(
                        "[running] {}/{} processed (succeeded={} skipped={} failed={}) pair={}",
                        status.processed,
                        status.total_pairs,
                        status.succeeded,
                        status.skipped,
                        status.failed,
                        status.current_pair.as_deref().unwrap_or("?"),
                    );
                    *last = now;
                }
            }
        })
    };

    let status = backfill::run_backfill(ctx).await;
    let _ = printer.await;

    println!(
        "Backfill {:?}: processed={} succeeded={} skipped={} failed={}",
        status.state, status.processed, status.succeeded, status.skipped, status.failed
    );
    if !status.errors.is_empty() {
        println!("Last error: {}", status.last_error.as_deref().unwrap_or(""));
    }
    if status.state == backfill::BackfillState::Failed {
        std::process::exit(1);
    }
    Ok(())
}

async fn trigger_reload() -> Result<()> {
    println!("Sending reload signal to active statesync service...");
    let url = std::env::var("STATESYNC_RELOAD_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8754/api/reload".to_string());
    let token = std::env::var("STATESYNC_WEB_AUTH").ok();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let mut req = client.post(&url);
    if let Some(t) = token {
        if let Some(b) = t.strip_prefix("bearer:") {
            req = req.bearer_auth(b);
        }
    }
    match req.send().await {
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
            println!("✗ Failed to connect to active statesync service: {}", e);
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

async fn dry_run() -> Result<()> {
    use std::collections::HashSet;
    println!("=== DRY RUN ===");
    init_logging();
    let config = match Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            println!("✗ Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };
    println!("Loaded {} server(s).", config.servers.len());
    let mut caches = Vec::new();
    for s in &config.servers {
        println!("Initializing cache for '{}'...", s.name);
        let client = MediaClient::new(s.url.clone(), s.api_key.clone(), s.is_emby);
        match init_server_cache(&s.name, &client).await {
            Ok(c) => caches.push(c),
            Err(e) => {
                println!("  ✗ '{}' failed: {}", s.name, e);
                std::process::exit(1);
            }
        }
    }
    let mut seen: HashSet<(usize, String)> = HashSet::new();
    let mut ambiguous = 0u32;
    for (idx, cache) in caches.iter().enumerate() {
        for username in cache.users.keys() {
            let key = (idx, username.clone());
            if seen.contains(&key) {
                continue;
            }
            for (other_idx, other_cache) in caches.iter().enumerate() {
                if other_idx == idx {
                    continue;
                }
                let matched = statesync::state::find_mapped_user_id(
                    username,
                    &other_cache.users,
                    &config.user_mappings,
                );
                if let Some(_id) = matched {
                    seen.insert(key.clone());
                    seen.insert((other_idx, _id));
                }
            }
        }
    }
    for c in &caches {
        if c.users.is_empty() {
            println!("  ! '{}' has no users", c.name);
            ambiguous += 1;
        }
    }
    if ambiguous > 0 {
        println!("\n✗ {} problem(s) detected.", ambiguous);
        std::process::exit(1);
    }
    println!("\n✓ Dry run complete; no problems detected.");
    Ok(())
}

async fn run_tui() -> Result<()> {
    let bind_addr = resolve_bind_addr();
    let web_auth = resolve_web_auth();
    let url = std::env::var("STATESYNC_TUI_URL")
        .unwrap_or_else(|_| format!("http://{}/api/status", bind_addr));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    loop {
        let mut req = client.get(&url);
        if let Some(spec) = web_auth.as_deref() {
            if let Some(token) = spec.strip_prefix("bearer:") {
                req = req.bearer_auth(token);
            }
        }
        match req.send().await {
            Ok(resp) => {
                if resp.status() == reqwest::StatusCode::OK {
                    if let Ok(status) = resp.json::<serde_json::Value>().await {
                        draw_tui_from_json(&status);
                    }
                } else {
                    print!("\x1B[2J\x1B[H");
                    println!(
                        "✗ TUI got HTTP {} from {}. Check STATESYNC_WEB_AUTH / STATESYNC_BIND.",
                        resp.status(),
                        url
                    );
                }
            }
            Err(e) => {
                print!("\x1B[2J\x1B[H");
                println!("✗ Cannot connect to statesync background service: {}", e);
                println!("Make sure the statesync background container is running.");
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

fn draw_tui_from_json(status: &serde_json::Value) {
    print!("\x1B[2J\x1B[H");
    println!(
        "\x1B[1m\x1B[36m┌──────────────────────────────────────────────────────────────────────────────┐\x1B[0m"
    );
    println!(
        "\x1B[1m\x1B[36m│                       STATESYNC TERMINAL DASHBOARD v{:>5}                │\x1B[0m",
        VERSION
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
                    "success" => "\x1B[32m",
                    "warn" => "\x1B[33m",
                    "error" => "\x1B[31m",
                    _ => "\x1B[37m",
                };
                println!("  [{}] {}{}\x1B[0m", timestamp, color, message);
            }
        }
    } else {
        println!("  Reading activity logs...");
    }
    println!("\n\x1B[90m(Press Ctrl+C to close and exit dashboard)\x1B[0m");

    use std::io::Write;
    let _ = std::io::stdout().flush();
}
