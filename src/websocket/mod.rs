use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::client::MediaClient;
use crate::config::Config;
use crate::state::AppState;

pub mod handlers;
pub mod loop_handler;
pub mod session_events;
pub mod userdata_events;
pub use loop_handler::handle_websocket_loop;

#[cfg(test)]
mod tests;

pub fn make_ws_url(url: &str, api_key: &str, is_emby: bool) -> String {
    let clean_url = url.trim().trim_end_matches('/');
    let lower_url = clean_url.to_lowercase();
    let ws_base = if lower_url.starts_with("https://") {
        format!("wss://{}", &clean_url[8..])
    } else if lower_url.starts_with("http://") {
        format!("ws://{}", &clean_url[7..])
    } else {
        format!("ws://{}", clean_url)
    };

    let ws_path = if is_emby {
        if ws_base.ends_with("/embywebsocket") {
            ""
        } else if ws_base.ends_with("/emby") {
            "websocket"
        } else {
            "/embywebsocket"
        }
    } else {
        if ws_base.ends_with("/socket") {
            ""
        } else {
            "/socket"
        }
    };

    let encoded_key = utf8_percent_encode(api_key.trim(), NON_ALPHANUMERIC).to_string();
    format!(
        "{}{}?api_key={}&deviceId=statesync",
        ws_base, ws_path, encoded_key
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
