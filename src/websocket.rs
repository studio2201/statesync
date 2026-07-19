use futures_util::{SinkExt, StreamExt};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, broadcast};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessageProto;
use tracing::{error, info, warn};

use crate::client::{MediaClient, SessionInfo, WsMessage};
use crate::config::Config;
use crate::state::AppState;

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

pub async fn handle_websocket_loop(
    source_index: usize,
    ws_url: &str,
    source_client: Arc<MediaClient>,
    target_clients: Vec<(usize, Arc<MediaClient>)>,
    state_lock: Arc<Mutex<AppState>>,
    config: Config,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    let mut backoff_attempt: u32 = 0;
    loop {
        let source_name = {
            let state = state_lock.lock().await;
            if source_index >= state.caches.len() {
                return;
            }
            state.caches[source_index].name.clone()
        };

        let cache_uninitialized = {
            let state = state_lock.lock().await;
            source_index < state.caches.len() && state.caches[source_index].users.is_empty()
        };
        if cache_uninitialized {
            info!(
                "Attempting background cache initialization for '{}'...",
                source_name
            );
            match crate::state::init_server_cache(&source_name, &source_client).await {
                Ok(cache) => {
                    let mut state = state_lock.lock().await;
                    if source_index < state.caches.len() {
                        state.caches[source_index] = cache;
                    }
                    info!(
                        "Cache initialized successfully in background for '{}'.",
                        source_name
                    );
                    state.log_event(
                        "success",
                        &format!("Cache initialized for '{}'", source_name),
                    );
                }
                Err(e) => {
                    warn!(
                        "Background cache init failed for '{}': {}. Retrying in 10s...",
                        source_name, e
                    );
                    tokio::select! {
                        _ = shutdown_rx.recv() => return,
                        _ = tokio::time::sleep(Duration::from_secs(10)) => {}
                    }
                    continue;
                }
            }
        }

        if source_index >= state_lock.lock().await.websocket_statuses.len() {
            return;
        }
        state_lock.lock().await.websocket_statuses[source_index] = "Reconnecting".to_string();

        let conn_result = tokio::select! {
            _ = shutdown_rx.recv() => {
                state_lock.lock().await.websocket_statuses[source_index] = "Offline".to_string();
                return;
            }
            res = connect_async(ws_url) => res,
        };

        match conn_result {
            Ok((mut ws_stream, _)) => {
                info!("'{}' WebSocket connected.", source_name);
                let mut state = state_lock.lock().await;
                state.log_event(
                    "success",
                    &format!("'{}' WebSocket connected.", source_name),
                );
                state.websocket_statuses[source_index] = "Connected".to_string();
                drop(state);
                backoff_attempt = 0;

                let start_msg =
                    json!({ "MessageType": "SessionsStart", "Data": "0,1000" }).to_string();
                if let Err(e) = ws_stream.send(WsMessageProto::Text(start_msg.into())).await {
                    error!(
                        "Failed to send subscribe message for '{}': {}",
                        source_name, e
                    );
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }

                let mut last_activity = Instant::now();
                let mut ping_interval = tokio::time::interval(Duration::from_secs(30));
                ping_interval.tick().await;

                loop {
                    let next_msg = tokio::select! {
                        _ = shutdown_rx.recv() => {
                            state_lock.lock().await.websocket_statuses[source_index] = "Offline".to_string();
                            return;
                        }
                        _ = ping_interval.tick() => {
                            if last_activity.elapsed() > Duration::from_secs(45) {
                                warn!("WebSocket connection to '{}' timed out. Reconnecting...", source_name);
                                break;
                            }
                            if let Err(e) = ws_stream.send(WsMessageProto::Ping(Vec::new().into())).await {
                                warn!("Failed to send ping to '{}': {}", source_name, e);
                                break;
                            }
                            last_activity = Instant::now();
                            continue;
                        }
                        msg = ws_stream.next() => msg,
                    };

                    let msg = match next_msg {
                        Some(m) => m,
                        None => break,
                    };

                    last_activity = Instant::now();

                    match msg {
                        Ok(WsMessageProto::Text(text)) => {
                            if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                                if ws_msg.message_type == "Sessions" {
                                    if let Some(ref data) = ws_msg.data {
                                        if let Ok(sessions) =
                                            serde_json::from_value::<Vec<SessionInfo>>(data.clone())
                                        {
                                            let mut missing_users = Vec::new();
                                            {
                                                let state = state_lock.lock().await;
                                                for s in &sessions {
                                                    if let Some(user_name) = &s.user_name {
                                                        let user_lower = user_name.to_lowercase();
                                                        if source_index < state.caches.len()
                                                            && !state.caches[source_index]
                                                                .users
                                                                .contains_key(&user_lower)
                                                        {
                                                            missing_users.push(user_name.clone());
                                                        }
                                                    }
                                                }
                                            }
                                            if !missing_users.is_empty() {
                                                info!(
                                                    "Detected new users {:?} on '{}'. Hot-reloading user list (merging)...",
                                                    missing_users, source_name
                                                );
                                                if let Ok(new_users) =
                                                    source_client.get_users().await
                                                {
                                                    let mut state = state_lock.lock().await;
                                                    if source_index < state.caches.len() {
                                                        for (k, v) in new_users {
                                                            state.caches[source_index]
                                                                .users
                                                                .entry(k)
                                                                .or_insert(v);
                                                        }
                                                    }
                                                }
                                            }
                                            let mut state = state_lock.lock().await;
                                            state
                                                .active_sessions
                                                .retain(|(srv, _), _| srv != &source_name);

                                            for s in &sessions {
                                                if let (
                                                    Some(user_name),
                                                    Some(item),
                                                    Some(play_state),
                                                ) = (
                                                    &s.user_name,
                                                    &s.now_playing_item,
                                                    &s.play_state,
                                                ) {
                                                    let Some(position) = play_state.position_ticks
                                                    else {
                                                        continue;
                                                    };
                                                    let is_paused =
                                                        play_state.is_paused.unwrap_or(false);
                                                    let pos_secs = position as f64 / 10_000_000.0;
                                                    state.active_sessions.insert(
                                                        (source_name.clone(), s.id.clone()),
                                                        (
                                                            user_name.clone(),
                                                            item.name.clone().unwrap_or_default(),
                                                            pos_secs,
                                                            is_paused,
                                                            item.id.clone(),
                                                        ),
                                                    );

                                                    if config.servers[source_index].sync_direction
                                                        == "receive"
                                                    {
                                                        continue;
                                                    }

                                                    let user_name_clone = user_name.clone();
                                                    let item_id_clone = item.id.clone();
                                                    let item_name_opt = item.name.clone();
                                                    let source_name_clone = source_name.clone();
                                                    let state_lock_clone = state_lock.clone();
                                                    let target_clients_clone =
                                                        target_clients.clone();
                                                    let config_clone = config.clone();
                                                    let source_client_clone = source_client.clone();
 
                                                    tokio::spawn(async move {
                                                        crate::sync::sync_progress_to_targets(
                                                            &user_name_clone,
                                                            &item_id_clone,
                                                            position,
                                                            false,
                                                            &source_name_clone,
                                                            source_index,
                                                            &state_lock_clone,
                                                            &target_clients_clone,
                                                            &config_clone,
                                                            &source_client_clone,
                                                            item_name_opt,
                                                        )
                                                        .await;
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                                if ws_msg.message_type == "UserDataChanged" {
                                    if let Some(ref data) = ws_msg.data {
                                        if let Ok(info) = serde_json::from_value::<
                                            crate::client::UserDataChangedInfo,
                                        >(
                                            data.clone()
                                        ) {
                                            let user_name = {
                                                let state = state_lock.lock().await;
                                                state.caches[source_index]
                                                    .users
                                                    .iter()
                                                    .find(|(_, id)| *id == &info.user_id)
                                                    .map(|(name, _)| name.clone())
                                            };
                                            if let Some(user_name) = user_name {
                                                for entry in &info.user_data_list {
                                                    if config.servers[source_index].sync_direction
                                                        == "receive"
                                                    {
                                                        continue;
                                                    }

                                                    let user_name_clone = user_name.clone();
                                                    let item_id_clone = entry.item_id.clone();
                                                    let Some(pos) = entry.playback_position_ticks
                                                    else {
                                                        if !entry.played {
                                                            continue;
                                                        }
                                                        return spawn_userdata_sync(
                                                            user_name_clone,
                                                            item_id_clone,
                                                            0,
                                                            entry.played,
                                                            source_name.clone(),
                                                            source_index,
                                                            state_lock.clone(),
                                                            target_clients.clone(),
                                                            config.clone(),
                                                            source_client.clone(),
                                                        );
                                                    };
                                                    spawn_userdata_sync(
                                                        user_name_clone,
                                                        item_id_clone,
                                                        pos,
                                                        entry.played,
                                                        source_name.clone(),
                                                        source_index,
                                                        state_lock.clone(),
                                                        target_clients.clone(),
                                                        config.clone(),
                                                        source_client.clone(),
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Ok(WsMessageProto::Ping(payload)) => {
                            last_activity = Instant::now();
                            if let Err(e) = ws_stream.send(WsMessageProto::Pong(payload)).await {
                                warn!("Failed to send pong to '{}': {}", source_name, e);
                                break;
                            }
                        }
                        Ok(WsMessageProto::Pong(_)) => {
                            last_activity = Instant::now();
                        }
                        Ok(WsMessageProto::Close(_)) => {
                            warn!("'{}' sent close frame. Reconnecting...", source_name);
                            break;
                        }
                        Ok(_) => {}
                        Err(e) => {
                            error!("WebSocket stream error on '{}': {}", source_name, e);
                            break;
                        }
                    }
                }
                warn!("'{}' WebSocket disconnected. Reconnecting...", source_name);
                state_lock.lock().await.log_event(
                    "warn",
                    &format!("'{}' WebSocket disconnected. Reconnecting...", source_name),
                );
            }
            Err(e) => {
                error!("Failed to connect to '{}' WebSocket: {}.", source_name, e);
                state_lock.lock().await.log_event(
                    "error",
                    &format!("Failed to connect to '{}' WebSocket: {}", source_name, e),
                );
            }
        }

        let backoff = next_backoff(backoff_attempt);
        backoff_attempt = backoff_attempt.saturating_add(1);
        tokio::select! {
            _ = shutdown_rx.recv() => {
                state_lock.lock().await.websocket_statuses[source_index] = "Offline".to_string();
                return;
            }
            _ = tokio::time::sleep(backoff) => {}
        }
    }
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
