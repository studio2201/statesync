use std::sync::Arc;
use std::time::Duration;
use statesync::client::MediaClient;
use statesync::config::Config;
use statesync::state::AppState;
use statesync::sync_force::{Direction, ForceContext, ForceSyncState, run_force_sync};

fn parse_sync_force_args(args: &[String]) -> Direction {
    for a in args.iter().skip(2) {
        if let Some(v) = a.strip_prefix("--direction=") {
            return match v {
                "emby-to-jellyfin" => Direction::EmbyToJellyfin,
                "jellyfin-to-emby" => Direction::JellyfinToEmby,
                _ => Direction::Both,
            };
        }
        if a == "--help" || a == "-h" {
            println!(
                "Usage: statesync --sync-force [--direction=emby-to-jellyfin|jellyfin-to-emby|both]"
            );
            std::process::exit(0);
        }
    }
    statesync::sync_force::direction_from_env()
}

pub async fn run_sync_force_cli(args: &[String]) -> anyhow::Result<()> {
    let config = Config::load()?;
    if config.servers.is_empty() {
        eprintln!("No servers configured.");
        std::process::exit(1);
    }
    let server_names: Vec<String> = config.servers.iter().map(|s| s.name.clone()).collect();
    let mut clients = Vec::new();
    for s in &config.servers {
        let client = Arc::new(MediaClient::new(
            s.url.clone(),
            s.api_key.clone(),
            s.is_emby,
        ));
        clients.push(client);
    }
    let state = Arc::new(tokio::sync::Mutex::new(AppState::new(Vec::new())));
    let tracker = state.lock().await.sync_force.clone();
    let direction = parse_sync_force_args(args);

    println!(
        "Starting --sync-force: direction={:?}, rate={}/sec, servers={}",
        direction,
        std::env::var("STATESYNC_FORCE_RATE")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(5),
        server_names.join(",")
    );

    let last_print = Arc::new(tokio::sync::Mutex::new(std::time::Instant::now()));
    let printer = {
        let tracker = tracker.clone();
        let last_print = last_print.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
                let status = tracker.status.lock().await.clone();
                if status.state != ForceSyncState::Running {
                    if status.state != ForceSyncState::Idle {
                        println!(
                            "[{:?}] done: processed={} ok={} skip={} fail={}",
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
                        "[running] {}/{} processed (ok={} skip={} fail={}) user={}",
                        status.processed,
                        status.total_pairs,
                        status.succeeded,
                        status.skipped,
                        status.failed,
                        status.current_user.as_deref().unwrap_or("?"),
                    );
                    *last = now;
                }
            }
        })
    };

    let ctx = ForceContext {
        config,
        clients,
        state,
        tracker: tracker.clone(),
        direction,
    };
    let status = run_force_sync(ctx).await;
    let _ = printer.await;

    println!(
        "--sync-force {:?}: processed={} ok={} skip={} fail={}",
        status.state, status.processed, status.succeeded, status.skipped, status.failed
    );
    if status.state == ForceSyncState::Failed {
        std::process::exit(1);
    }
    Ok(())
}
