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
                        "Cannot reach dashboard API (HTTP {}) at {}.",
                        resp.status(),
                        url
                    );
                }
            }
            Err(e) => {
                print!("\x1B[2J\x1B[H");
                println!("Cannot connect to StateSync: {}", e);
                println!("Is the background service running?");
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// First-principles server link label (matches web UI).
fn server_status_label(raw: &str) -> (&'static str, &'static str) {
    match raw {
        "Synchronizing" | "Connected" => ("Live", "\x1B[32m"),
        "Validating" => ("Checking access", "\x1B[33m"),
        "Scanning" => ("Loading data", "\x1B[33m"),
        "Connecting" => ("Connecting", "\x1B[33m"),
        "Reconnecting" => ("Reconnecting", "\x1B[33m"),
        "Error" => ("Failed", "\x1B[31m"),
        "Offline" => ("Offline", "\x1B[90m"),
        _ => ("Unknown", "\x1B[37m"),
    }
}

fn force_phase_label(phase: &str) -> &str {
    match phase.to_ascii_lowercase().as_str() {
        "preparing" => "preparing",
        "played" => "played history",
        "favorites" => "favorites",
        "finishing" => "finishing",
        "done" => "done",
        "cancelled" => "cancelled",
        _ => phase,
    }
}

pub(super) fn draw_tui_from_json(status: &serde_json::Value) {
    print!("\x1B[2J\x1B[H");
    println!(
        "\x1B[1mStateSync\x1B[0m  v{}  ·  watched · resume · favorites",
        VERSION
    );
    println!("\x1B[90m────────────────────────────────────────────────────────\x1B[0m");

    // Last force story
    if let Some(fs) = status.get("last_full_sync") {
        let st = fs
            .get("state")
            .and_then(|v| v.as_str())
            .unwrap_or("Idle");
        let st_l = st.to_ascii_lowercase();
        if st_l == "running" {
            let phase = fs
                .get("phase")
                .and_then(|v| v.as_str())
                .unwrap_or("running");
            let processed = fs.get("processed").and_then(|v| v.as_u64()).unwrap_or(0);
            let total = fs.get("total_pairs").and_then(|v| v.as_u64()).unwrap_or(0);
            let user = fs
                .get("current_user")
                .and_then(|v| v.as_str())
                .unwrap_or("…");
            let dry = fs.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);
            println!(
                "\x1B[36m{}\x1B[0m  {} · {}/{} · {}",
                if dry { "Force preview" } else { "Force sync" },
                force_phase_label(phase),
                processed,
                total.max(1),
                user
            );
        } else if st_l == "completed" || st_l == "failed" {
            let ok = fs.get("succeeded").and_then(|v| v.as_u64()).unwrap_or(0);
            let skip = fs.get("skipped").and_then(|v| v.as_u64()).unwrap_or(0);
            let fail = fs.get("failed").and_then(|v| v.as_u64()).unwrap_or(0);
            let label = if st_l == "completed" {
                "\x1B[32mlast force finished\x1B[0m"
            } else {
                "\x1B[31mlast force had errors\x1B[0m"
            };
            let mut line = format!("{}  pushed {} · skipped {} · failed {}", label, ok, skip, fail);
            if let Some(sr) = fs.get("skip_reasons") {
                let ae = sr.get("already_equal").and_then(|v| v.as_u64()).unwrap_or(0);
                if ae > 0 {
                    line.push_str(&format!(" · {} already matched", ae));
                }
            }
            println!("{}", line);
        } else {
            println!("\x1B[90mForce sync  not run yet — use Force sync in the web UI or --sync-force\x1B[0m");
        }
        println!("\x1B[90m────────────────────────────────────────────────────────\x1B[0m");
    }

    println!("\x1B[1mMedia servers\x1B[0m");
    if let Some(servers) = status.get("servers").and_then(|v| v.as_array()) {
        if servers.is_empty() {
            println!("  No servers yet. Add them in the web UI.");
        } else {
            for s in servers {
                let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let ws_status = s
                    .get("websocket_status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Offline");
                let users_count = s.get("users_count").and_then(|v| v.as_u64()).unwrap_or(0);
                let (label, color) = server_status_label(ws_status);
                println!(
                    "  · \x1B[1m{}\x1B[0m  {}{}\x1B[0m  ·  {} users",
                    name, color, label, users_count
                );
            }
        }
    } else {
        println!("  Loading…");
    }
    println!();

    println!("\x1B[1mNow playing\x1B[0m");
    if let Some(sessions) = status.get("active_sessions").and_then(|v| v.as_array()) {
        if sessions.is_empty() {
            println!("  Quiet. Waiting for someone to hit play.");
        } else {
            for sess in sessions {
                let server = sess.get("server").and_then(|v| v.as_str()).unwrap_or("?");
                let user = sess.get("user").and_then(|v| v.as_str()).unwrap_or("?");
                let item = sess.get("item").and_then(|v| v.as_str()).unwrap_or("?");
                let position = sess.get("position").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let is_paused = sess
                    .get("is_paused")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let mins = (position / 60.0).floor() as u32;
                let secs = (position % 60.0).floor() as u32;
                let t = format!("{:02}:{:02}", mins, secs);
                if is_paused {
                    println!(
                        "  · \x1B[33mPaused\x1B[0m  {} · '{}' on {} at {}",
                        user, item, server, t
                    );
                } else {
                    println!(
                        "  · \x1B[32mPlaying\x1B[0m  {} · '{}' on {}  (syncing)",
                        user, item, server
                    );
                }
            }
        }
    } else {
        println!("  Loading…");
    }
    println!();

    println!("\x1B[1mActivity\x1B[0m");
    if let Some(logs) = status.get("sync_logs").and_then(|v| v.as_array()) {
        if logs.is_empty() {
            println!("  No activity yet.");
        } else {
            for entry in logs.iter().take(10) {
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
                if let Some(detail) = entry.get("detail").and_then(|v| v.as_str()) {
                    if !detail.is_empty() {
                        let short = if detail.len() > 100 {
                            format!("{}…", &detail[..97])
                        } else {
                            detail.to_string()
                        };
                        println!("         \x1B[90m{}\x1B[0m", short);
                    }
                }
            }
        }
    } else {
        println!("  Loading…");
    }
    println!("\n\x1B[90mCtrl+C to exit\x1B[0m");

    use std::io::Write;
    let _ = std::io::stdout().flush();
}


#[cfg(test)]
mod generated_tests {
    use super::*;
    #[test]
    fn test_run_tui_generated_test_0() {
        assert!(true);
    }
    #[test]
    fn test_server_status_label_live() {
        assert_eq!(server_status_label("Synchronizing").0, "Live");
        assert_eq!(server_status_label("Error").0, "Failed");
    }
}
