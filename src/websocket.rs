use std::sync::Arc;
use std::time::Duration;
use serde_json::json;
use tokio::sync::{Mutex, broadcast};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessageProto;
use tracing::{info, warn, error};

use crate::config::Config;
use crate::client::{WsMessage, SessionInfo, MediaClient};
use crate::state::AppState;

pub fn make_ws_url(url: &str, api_key: &str, is_emby: bool) -> String {
    let base = url.trim_end_matches('/');
    let ws_base = if base.starts_with("https://") { base.replace("https://", "wss://") } else if base.starts_with("http://") { base.replace("http://", "ws://") } else { format!("ws://{}", base) };
    format!("{}{}?api_key={}&deviceId=statesync", ws_base, if is_emby { "/embywebsocket" } else { "/socket" }, api_key)
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
    loop {
        let source_name = {
            let state = state_lock.lock().await;
            if source_index >= state.caches.len() { return; }
            state.caches[source_index].name.clone()
        };

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
                state.log_event("success", &format!("'{}' WebSocket connected.", source_name));
                state.websocket_statuses[source_index] = "Connected".to_string();
                drop(state);

                let start_msg = json!({ "MessageType": "SessionsStart", "Data": "0,1000" }).to_string();
                if let Err(e) = ws_stream.send(WsMessageProto::Text(start_msg.into())).await {
                    error!("Failed to send subscribe message for '{}': {}", source_name, e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }

                let mut ping_interval = tokio::time::interval(Duration::from_secs(30));
                ping_interval.tick().await;

                loop {
                    let next_msg = tokio::select! {
                        _ = shutdown_rx.recv() => {
                            state_lock.lock().await.websocket_statuses[source_index] = "Offline".to_string();
                            return;
                        }
                        _ = ping_interval.tick() => {
                            if let Err(e) = ws_stream.send(WsMessageProto::Ping(Vec::new().into())).await {
                                warn!("Failed to send ping to '{}': {}", source_name, e);
                                break;
                            }
                            continue;
                        }
                        msg = ws_stream.next() => msg,
                    };

                    let msg = match next_msg {
                        Some(m) => m,
                        None => break,
                    };

                    match msg {
                        Ok(WsMessageProto::Text(text)) => {
                            if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                                if ws_msg.message_type == "Sessions" {
                                    if let Some(ref data) = ws_msg.data {
                                        if let Ok(sessions) = serde_json::from_value::<Vec<SessionInfo>>(data.clone()) {
                                            let mut missing_users = Vec::new();
                                            {
                                                let state = state_lock.lock().await;
                                                for s in &sessions {
                                                    if let Some(user_name) = &s.user_name {
                                                        let user_lower = user_name.to_lowercase();
                                                        if source_index < state.caches.len() && !state.caches[source_index].users.contains_key(&user_lower) {
                                                            missing_users.push(user_name.clone());
                                                        }
                                                    }
                                                }
                                            }
                                            if !missing_users.is_empty() {
                                                info!("Detected new users {:?} on '{}'. Hot-reloading user list...", missing_users, source_name);
                                                if let Ok(new_users) = source_client.get_users().await {
                                                    let mut state = state_lock.lock().await;
                                                    if source_index < state.caches.len() { state.caches[source_index].users = new_users; }
                                                }
                                            }
                                            let mut state = state_lock.lock().await;
                                            state.active_sessions.retain(|(srv, _), _| srv != &source_name);

                                            for s in &sessions {
                                                if let (Some(user_name), Some(item), Some(play_state)) = (&s.user_name, &s.now_playing_item, &s.play_state) {
                                                    let position = play_state.position_ticks.unwrap_or(0);
                                                    let is_paused = play_state.is_paused.unwrap_or(false);
                                                    let pos_secs = position as f64 / 10_000_000.0;
                                                    state.active_sessions.insert(
                                                        (source_name.clone(), s.id.clone()),
                                                        (user_name.clone(), item.name.clone().unwrap_or_default(), pos_secs, is_paused, item.id.clone())
                                                    );

                                                    if config.servers[source_index].sync_direction == "receive" { continue; }

                                                    let user_name_clone = user_name.clone();
                                                    let item_id_clone = item.id.clone();
                                                    let source_name_clone = source_name.clone();
                                                    let state_lock_clone = state_lock.clone();
                                                    let target_clients_clone = target_clients.clone();
                                                    let config_clone = config.clone();

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
                                                        ).await;
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                                if ws_msg.message_type == "UserDataChanged" {
                                    if let Some(ref data) = ws_msg.data {
                                        if let Ok(info) = serde_json::from_value::<crate::client::UserDataChangedInfo>(data.clone()) {
                                            let user_name = {
                                                let state = state_lock.lock().await;
                                                state.caches[source_index].users.iter()
                                                    .find(|(_, id)| *id == &info.user_id)
                                                    .map(|(name, _)| name.clone())
                                            };
                                            if let Some(user_name) = user_name {
                                                for entry in &info.user_data_list {
                                                    if config.servers[source_index].sync_direction == "receive" { continue; }

                                                    let user_name_clone = user_name.clone();
                                                    let item_id_clone = entry.item_id.clone();
                                                    let pos = entry.playback_position_ticks.unwrap_or(0);
                                                    let played = entry.played;
                                                    let source_name_clone = source_name.clone();
                                                    let state_lock_clone = state_lock.clone();
                                                    let target_clients_clone = target_clients.clone();
                                                    let config_clone = config.clone();

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
                                                        ).await;
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Ok(WsMessageProto::Ping(payload)) => {
                            if let Err(e) = ws_stream.send(WsMessageProto::Pong(payload)).await {
                                warn!("Failed to send pong to '{}': {}", source_name, e);
                                break;
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            error!("WebSocket stream error on '{}': {}", source_name, e);
                            break;
                        }
                    }
                }
                warn!("'{}' WebSocket disconnected. Reconnecting in 5 seconds...", source_name);
                state_lock.lock().await.log_event("warn", &format!("'{}' WebSocket disconnected. Reconnecting...", source_name));
            }
            Err(e) => {
                error!("Failed to connect to '{}' WebSocket: {}. Retrying in 5 seconds...", source_name, e);
                state_lock.lock().await.log_event("error", &format!("Failed to connect to '{}' WebSocket: {}", source_name, e));
            }
        }
        
        tokio::select! {
            _ = shutdown_rx.recv() => {
                state_lock.lock().await.websocket_statuses[source_index] = "Offline".to_string();
                return;
            }
            _ = tokio::time::sleep(Duration::from_secs(5)) => {}
        }
    }
}
