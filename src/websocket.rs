use std::sync::Arc;
use std::time::{Duration, Instant};
use serde_json::json;
use tokio::sync::{Mutex, broadcast};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessageProto;
use tracing::{info, warn, error};

use crate::config::Config;
use crate::client::{WsMessage, SessionInfo, MediaClient};
use crate::state::{AppState, SyncHistoryValue};

pub fn make_ws_url(url: &str, api_key: &str, is_emby: bool) -> String {
    let base = url.trim_end_matches('/');
    let ws_base = if base.starts_with("https://") { base.replace("https://", "wss://") } else if base.starts_with("http://") { base.replace("http://", "ws://") } else { format!("ws://{}", base) };
    format!("{}{}?api_key={}&deviceId=statesync", ws_base, if is_emby { "/embywebsocket" } else { "/socket" }, api_key)
}

pub async fn handle_websocket_loop(
    source_index: usize,
    ws_url: &str,
    target_clients: Vec<(usize, Arc<MediaClient>)>,
    state_lock: Arc<Mutex<AppState>>,
    config: Config,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    loop {
        let source_name = {
            let state = state_lock.lock().await;
            if source_index >= state.caches.len() {
                return;
            }
            state.caches[source_index].name.clone()
        };

        // Report Reconnecting status
        {
            let mut state = state_lock.lock().await;
            if source_index < state.websocket_statuses.len() {
                state.websocket_statuses[source_index] = "Reconnecting".to_string();
            }
        }

        info!("Connecting to '{}' WebSocket: {}", source_name, ws_url);
        
        let conn_result = tokio::select! {
            _ = shutdown_rx.recv() => {
                let mut state = state_lock.lock().await;
                if source_index < state.websocket_statuses.len() {
                    state.websocket_statuses[source_index] = "Offline".to_string();
                }
                return;
            }
            res = connect_async(ws_url) => res,
        };

        match conn_result {
            Ok((mut ws_stream, _)) => {
                info!("'{}' WebSocket connected.", source_name);
                
                // Report Connected status
                {
                    let mut state = state_lock.lock().await;
                    if source_index < state.websocket_statuses.len() {
                        state.websocket_statuses[source_index] = "Connected".to_string();
                    }
                }

                let start_msg = json!({
                    "MessageType": "SessionsStart",
                    "Data": "0,1000"
                });
                if let Err(e) = ws_stream.send(WsMessageProto::Text(start_msg.to_string().into())).await {
                    error!("Failed to send subscribe message for '{}': {}", source_name, e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }

                loop {
                    let next_msg = tokio::select! {
                        _ = shutdown_rx.recv() => {
                            let mut state = state_lock.lock().await;
                            if source_index < state.websocket_statuses.len() {
                                state.websocket_statuses[source_index] = "Offline".to_string();
                            }
                            return;
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
                                            let mut state = state_lock.lock().await;
                                            let now = Instant::now();
                                            
                                            // Clear old active sessions for this source server
                                            state.active_sessions.retain(|(srv, _), _| srv != &source_name);

                                            for s in &sessions {
                                                if let (Some(user_name), Some(item), Some(play_state)) = (&s.user_name, &s.now_playing_item, &s.play_state) {
                                                    let user_lower = user_name.to_lowercase();
                                                    let position = play_state.position_ticks.unwrap_or(0);
                                                    let is_paused = play_state.is_paused.unwrap_or(false);

                                                    // Record currently playing active session
                                                    let pos_secs = position as f64 / 10_000_000.0;
                                                    state.active_sessions.insert(
                                                        (source_name.clone(), s.id.clone()),
                                                        (user_name.clone(), item.name.clone().unwrap_or_default(), pos_secs, is_paused)
                                                    );

                                                    let source_item_providers = {
                                                        let source_cache = &state.caches[source_index];
                                                        source_cache.id_to_providers.get(&item.id).cloned()
                                                    };

                                                    if let Some((imdb_id, tmdb_id)) = source_item_providers {
                                                        let provider_id = if !imdb_id.is_empty() {
                                                            imdb_id.clone()
                                                        } else if !tmdb_id.is_empty() {
                                                            tmdb_id.clone()
                                                        } else {
                                                            continue;
                                                        };

                                                        // Check sync history to prevent loops
                                                        let history_key = (user_lower.clone(), provider_id.clone());
                                                        if let Some(history) = state.last_syncs.get(&history_key) {
                                                            let age = now - history.timestamp;
                                                            let pos_diff = (position - history.position_ticks).abs();
                                                            let threshold_ticks = (config.sync_threshold_seconds * 10_000_000) as i64;
                                                            
                                                            if age < Duration::from_secs(5) && pos_diff < threshold_ticks {
                                                                continue;
                                                            }
                                                        }

                                                        // Sync to all OTHER target servers
                                                        for &(target_index, ref client_target) in &target_clients {
                                                            let (target_item_id, target_user_id, target_name) = {
                                                                let target_cache = &state.caches[target_index];
                                                                let mut t_item_id = None;
                                                                if !imdb_id.is_empty() {
                                                                    t_item_id = target_cache.imdb_to_id.get(&imdb_id).cloned();
                                                                }
                                                                if t_item_id.is_none() && !tmdb_id.is_empty() {
                                                                    t_item_id = target_cache.tmdb_to_id.get(&tmdb_id).cloned();
                                                                }
                                                                let t_user_id = crate::state::find_mapped_user_id(&user_lower, &target_cache.users);
                                                                let t_name = target_cache.name.clone();
                                                                (t_item_id, t_user_id, t_name)
                                                            };

                                                            if let (Some(t_item_id), Some(t_user_id)) = (target_item_id, target_user_id) {
                                                                state.last_syncs.insert(history_key.clone(), SyncHistoryValue {
                                                                    position_ticks: position,
                                                                    timestamp: now,
                                                                });

                                                                let secs = std::time::SystemTime::now()
                                                                    .duration_since(std::time::UNIX_EPOCH)
                                                                    .unwrap_or_default()
                                                                    .as_secs();
                                                                let hours = (secs / 3600) % 24;
                                                                let mins = (secs / 60) % 60;
                                                                let w_secs = secs % 60;
                                                                let timestamp = format!("{:02}:{:02}:{:02}", hours, mins, w_secs);

                                                                let entry = crate::state::SyncLogEntry {
                                                                    timestamp,
                                                                    user: user_name.clone(),
                                                                    item: item.name.as_deref().unwrap_or(&item.id).to_string(),
                                                                    source_name: source_name.clone(),
                                                                    source_is_emby: config.servers[source_index].is_emby,
                                                                    target_name: target_name.clone(),
                                                                    target_is_emby: config.servers[target_index].is_emby,
                                                                    position_secs: pos_secs,
                                                                    is_paused,
                                                                };
                                                                
                                                                info!(
                                                                    "Synced '{}' for {} from '{}' -> '{}' to {:.1}s{}",
                                                                    entry.item,
                                                                    entry.user,
                                                                    entry.source_name,
                                                                    entry.target_name,
                                                                    entry.position_secs,
                                                                    if entry.is_paused { " (paused)" } else { "" }
                                                                );
                                                                state.log_sync(entry);

                                                                let client_target_clone = client_target.clone();
                                                                tokio::spawn(async move {
                                                                    if let Err(e) = client_target_clone.update_progress(&t_user_id, &t_item_id, position, is_paused).await {
                                                                        error!("Error updating target playstate progress: {}", e);
                                                                    }
                                                                });
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
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
            }
            Err(e) => {
                error!("Failed to connect to '{}' WebSocket: {}. Retrying in 5 seconds...", source_name, e);
            }
        }
        
        // Wait 5 seconds before retrying, unless shut down
        tokio::select! {
            _ = shutdown_rx.recv() => {
                let mut state = state_lock.lock().await;
                if source_index < state.websocket_statuses.len() {
                    state.websocket_statuses[source_index] = "Offline".to_string();
                }
                return;
            }
            _ = tokio::time::sleep(Duration::from_secs(5)) => {}
        }
    }
}
