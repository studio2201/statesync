use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::client::MediaClient;
use crate::config::Config;
use crate::state::AppState;

pub mod loop_handler;
pub mod handlers;
pub use loop_handler::handle_websocket_loop;

pub fn make_ws_url(url: &str, api_key: &str, is_emby: bool) -> String {
    let base = url.trim_end_matches('/');
    let ws_base = if base.starts_with("https://") {
        base.replace("https://", "wss://")
    } else if base.starts_with("http://") {
        base.replace("http://", "ws://")
    } else {
        format!("ws://{}", base)
    };

    let encoded_key = utf8_percent_encode(api_key, NON_ALPHANUMERIC).to_string();
    format!(
        "{}{}?api_key={}&deviceId=statesync",
        ws_base,
        if is_emby { "/embywebsocket" } else { "/socket" },
        encoded_key
    )
}

fn next_backoff(attempt: u32) -> Duration {
    let base_ms = 1_000u64;
    let cap_ms = 60_000u64;
    let exp = base_ms.saturating_mul(2u64.saturating_pow(attempt.min(10)));
    let capped = exp.min(cap_ms);
    let jitter = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0))
        % (capped / 4 + 1);
    Duration::from_millis(capped + jitter)
}

#[allow(clippy::too_many_arguments)]
fn spawn_userdata_sync(
    user_name_clone: String,
    item_id_clone: String,
    pos: i64,
    played: bool,
    source_name_clone: String,
    source_index: usize,
    state_lock_clone: Arc<Mutex<AppState>>,
    target_clients_clone: Vec<(usize, Arc<MediaClient>)>,
    config_clone: Config,
    source_client_clone: Arc<MediaClient>,
) {
    tokio::spawn(async move {
        crate::sync::sync_progress_to_targets(
            &user_name_clone,
            &item_id_clone,
            pos,
            played,
            &source_name_clone,
            source_index,
            &state_lock_clone,
            &target_clients_clone,
            &config_clone,
            &source_client_clone,
            None,
        )
        .await;
    });
}

fn redact_api_key(msg: &str) -> String {
    let mut result = String::new();
    let mut current = msg;
    while let Some(idx) = current.find("api_key=") {
        result.push_str(&current[..idx]);
        result.push_str("api_key=[REDACTED]");
        let rest = &current[idx + 8..];
        if let Some(amp_idx) = rest.find('&') {
            current = &rest[amp_idx..];
        } else {
            current = "";
            break;
        }
    }
    result.push_str(current);
    result
}
