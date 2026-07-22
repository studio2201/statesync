use super::helpers::{format_force_skip_story, init_clients_parallel, print_force_progress};
use statesync::config::Config;
use statesync::state::AppState;
use statesync::sync_force::{Direction, ForceContext, ForceSyncState, run_force_sync};
use std::sync::Arc;
use std::time::Duration;

/// Returns (direction always Both, dry_run).
pub(super) fn parse_sync_force_args(args: &[String]) -> (Direction, bool) {
    let mut dry_run = false;
    for a in args.iter().skip(2) {
        if a == "--dry-run" || a == "--preview" {
            dry_run = true;
        }
        if a == "--help" || a == "-h" {
            println!("Force sync — backfill watched, resume, and favorites\n");
            println!("Usage:");
            println!("  statesync --sync-force");
            println!("  statesync --sync-force --dry-run   # preview only, no writes\n");
            println!(
                "Meshes all send→receive server pairs (per-server send/receive still applies)."
            );
            println!(
                "Uses Settings scopes (force played / position / favorites) and user allowlist."
            );
            println!("Leaves already-matching library titles unchanged. Story: phases + no-change reasons.");
            println!("Rate: STATESYNC_FORCE_RATE items/sec (default 5, max 50).");
            std::process::exit(0);
        }
    }
    (Direction::Both, dry_run)
}

pub async fn run_sync_force_cli(args: &[String]) -> anyhow::Result<()> {
    let config = Config::load()?;
    if config.servers.is_empty() {
        eprintln!("No servers configured. Add servers in the web UI first.");
        std::process::exit(1);
    }
    let server_names: Vec<String> = config.servers.iter().map(|s| s.name.clone()).collect();
    let state = Arc::new(tokio::sync::Mutex::new(AppState::new(Vec::new())));
    {
        let mut st = state.lock().await;
        st.websocket_statuses = vec!["Offline".to_string(); config.servers.len()];
    }

    println!("Force sync — loading libraries…");
    let (clients, caches) = init_clients_parallel(&config, &state).await?;
    if clients.len() != config.servers.len() {
        eprintln!(
            "Could not initialize all servers ({}/{}). Aborting.",
            clients.len(),
            config.servers.len()
        );
        std::process::exit(1);
    }
    let empty_user_caches: Vec<&str> = caches
        .iter()
        .filter(|c| c.users.is_empty())
        .map(|c| c.name.as_str())
        .collect();
    if !empty_user_caches.is_empty() {
        eprintln!(
            "Warning: no users loaded for: {}. Force may do little work.",
            empty_user_caches.join(", ")
        );
    }
    {
        let mut st = state.lock().await;
        st.caches = caches;
    }

    let tracker = state.lock().await.sync_force.clone();
    let (direction, dry_run) = parse_sync_force_args(args);
    let rate = std::env::var("STATESYNC_FORCE_RATE")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(5)
        .clamp(1, 50);

    let sync = &config.sync;
    let mut scope_bits = Vec::new();
    if sync.force_played {
        scope_bits.push("played");
    }
    if sync.force_position {
        scope_bits.push("position");
    }
    if sync.force_favorites {
        scope_bits.push("favorites");
    }
    if dry_run {
        scope_bits.push("dry-run");
    }

    println!(
        "Starting {} · {} · {} item/s · servers: {}",
        if dry_run {
            "force preview (no writes)"
        } else {
            "force sync"
        },
        if scope_bits.is_empty() {
            "nothing enabled in Settings".to_string()
        } else {
            scope_bits.join(" + ")
        },
        rate,
        server_names.join(", ")
    );
    if !sync.user_allowlist.is_empty() {
        println!("  user allowlist: {}", sync.user_allowlist.join(", "));
    }
    if !sync.user_ignorelist.is_empty() {
        println!("  ignored users: {}", sync.user_ignorelist.join(", "));
    }
    println!("Live play sync pauses until this finishes.\n");

    let last_print = Arc::new(tokio::sync::Mutex::new(std::time::Instant::now()));
    let printer = {
        let tracker = tracker.clone();
        let last_print = last_print.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
                let status = tracker.snapshot_status();
                if status.state != ForceSyncState::Running {
                    break;
                }
                let now = std::time::Instant::now();
                let mut last = last_print.lock().await;
                if now.duration_since(*last).as_secs() >= 2 {
                    print_force_progress(&status);
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
        dry_run,
        only_user: None,
    };
    let status = run_force_sync(ctx).await;
    let _ = printer.await;

    let verb = match status.state {
        ForceSyncState::Completed if dry_run => "Force preview finished (no writes)",
        ForceSyncState::Completed => "Force sync finished cleanly",
        ForceSyncState::Failed
            if status.last_error.as_deref() == Some("Sync cancelled by user") =>
        {
            "Force sync cancelled"
        }
        ForceSyncState::Failed if dry_run => "Force preview finished with some failures (no writes)",
        ForceSyncState::Failed => "Force sync finished with some failures",
        _ => "Force sync ended",
    };
    println!(
        "{} · checked {} · {} {} · no change {} · failed {}",
        verb,
        status.processed,
        if dry_run { "would update" } else { "updated" },
        status.succeeded,
        status.skipped,
        status.failed
    );
    let skip_story = format_force_skip_story(&status);
    if !skip_story.is_empty() {
        println!("  {}", skip_story.trim_start_matches(" · "));
    }
    let bf = &status.by_field;
    if bf.played.ok + bf.played.skip + bf.played.fail > 0
        || bf.favorite.ok + bf.favorite.skip + bf.favorite.fail > 0
    {
        println!(
            "  played {} updated / {} no change / {} fail · favorites {} updated / {} no change / {} fail",
            bf.played.ok,
            bf.played.skip,
            bf.played.fail,
            bf.favorite.ok,
            bf.favorite.skip,
            bf.favorite.fail
        );
    }
    if let Some(err) = &status.last_error {
        println!("  last error: {}", err);
    }

    if status.state == ForceSyncState::Failed {
        std::process::exit(1);
    }
    Ok(())
}
