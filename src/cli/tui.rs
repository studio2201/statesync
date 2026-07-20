use std::time::Duration;
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub async fn run_tui(bind_addr: &str, web_auth: Option<&str>) -> anyhow::Result<()> {
    let url = std::env::var("STATESYNC_TUI_URL")
        .unwrap_or_else(|_| format!("http://{}/api/status", bind_addr));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    loop {
        let mut req = client.get(&url);
        if let Some(spec) = web_auth {
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
            println!("  StateSync is resting. Connect your media servers to start bridging watch states.");
        } else {
            for s in servers {
                let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
                let ws_status = s
                    .get("websocket_status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Offline");
                let users_count = s.get("users_count").and_then(|v| v.as_u64()).unwrap_or(0);
                let media_count = s.get("media_count").and_then(|v| v.as_u64()).unwrap_or(0);

                let status_color = if ws_status == "Connected" || ws_status == "Synchronizing" {
                    "\x1B[32m"
                } else if ws_status == "Scanning" || ws_status == "Validating" || ws_status == "Connecting" {
                    "\x1B[33m"
                } else {
                    "\x1B[31m"
                };
                println!(
                    "  • \x1B[1m{:<12}\x1B[0m: {}{:<13}\x1B[0m ({} Users | {} Cached Media Items)",
                    name, status_color, ws_status, users_count, media_count
                );
            }
        }
    } else {
        println!("  Loading server status details...");
    }
    println!();

    println!("\x1B[1m\x1B[33m[ ACTIVE STREAMS ]\x1B[0m");
    if let Some(sessions) = status.get("active_sessions").and_then(|v| v.as_array()) {
        if sessions.is_empty() {
            println!("  All quiet. StateSync is waiting for someone to play a movie or show.");
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
                let mins = (position / 60.0).floor() as u32;
                let secs = (position % 60.0).floor() as u32;
                let duration_str = format!("{:02}:{:02}", mins, secs);

                if !is_paused {
                    println!(
                        "  • {} {} is watching '{}' on {} (actively syncing)",
                        play_icon, user, item, server
                    );
                } else {
                    println!(
                        "  • {} {} paused '{}' on {} (locked at {})",
                        play_icon, user, item, server, duration_str
                    );
                }
            }
        }
    } else {
        println!("  Reading active streams...");
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
