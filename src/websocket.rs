use std::sync::Arc;
use std::time::{Duration, Instant};
use serde_json::json;
use tokio::sync::Mutex;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessageProto;
use tracing::{info, warn, error};

use crate::config::Config;
use crate::client::{WsMessage, SessionInfo, MediaClient};
use crate::state::{AppState, SyncHistoryValue};

pub fn make_ws_url(url: &str, api_key: &str, is_emby: bool) -> String {
    let base = url.trim_end_matches('/');
    let ws_base = if base.starts_with("https://") {
        base.replace("https://", "wss://")
    } else if base.starts_with("http://") {
        base.replace("http://", "ws://")
    } else {
        format!("ws://{}", base)
    };
    
    let path = if is_emby { "/embywebsocket" } else { "/socket" };
    format!("{}{}?api_key={}&deviceId=statesync", ws_base, path, api_key)
}

pub async fn handle_websocket_loop(
    ws_url: &str,
    is_source_emby: bool,
    client_target: Arc<MediaClient>,
    state_lock: Arc<Mutex<AppState>>,
    config: Config,
) {
    loop {
        info!("Connecting to {} WebSocket: {}", if is_source_emby { "Emby" } else { "Jellyfin" }, ws_url);
        match connect_async(ws_url).await {
            Ok((mut ws_stream, _)) => {
                info!("{} WebSocket connected.", if is_source_emby { "Emby" } else { "Jellyfin" });

                let start_msg = json!({
                    "MessageType": "SessionsStart",
                    "Data": "0,1000"
                });
                if let Err(e) = ws_stream.send(WsMessageProto::Text(start_msg.to_string().into())).await {
                    error!("Failed to send subscribe message: {}", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }

                while let Some(msg) = ws_stream.next().await {
                    match msg {
                        Ok(WsMessageProto::Text(text)) => {
                            if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                                if ws_msg.message_type == "Sessions" {
                                    if let Some(ref data) = ws_msg.data {
                                        if let Ok(sessions) = serde_json::from_value::<Vec<SessionInfo>>(data.clone()) {
                                            let mut state = state_lock.lock().await;
                                            let now = Instant::now();
                                            
                                            for s in &sessions {
                                                if let (Some(user_name), Some(item), Some(play_state)) = (&s.user_name, &s.now_playing_item, &s.play_state) {
                                                    let user_lower = user_name.to_lowercase();
                                                    let position = play_state.position_ticks.unwrap_or(0);
                                                    let is_paused = play_state.is_paused.unwrap_or(false);

                                                    let source_cache = if is_source_emby { &state.emby_cache } else { &state.jellyfin_cache };
                                                    if let Some((imdb_id, tmdb_id)) = source_cache.id_to_providers.get(&item.id) {
                                                        let provider_id = if !imdb_id.is_empty() {
                                                            imdb_id.clone()
                                                        } else if !tmdb_id.is_empty() {
                                                            tmdb_id.clone()
                                                        } else {
                                                            continue;
                                                        };

                                                        let history_key = (user_lower.clone(), provider_id.clone());
                                                        if let Some(history) = state.last_syncs.get(&history_key) {
                                                            let age = now - history.timestamp;
                                                            let pos_diff = (position - history.position_ticks).abs();
                                                            let threshold_ticks = (config.sync_threshold_seconds * 10_000_000) as i64;
                                                            
                                                            if age < Duration::from_secs(5) && pos_diff < threshold_ticks {
                                                                continue;
                                                            }
                                                        }

                                                        let (target_item_id, target_user_id) = {
                                                            let target_cache = if is_source_emby { &state.jellyfin_cache } else { &state.emby_cache };
                                                            let mut t_item_id = None;
                                                            if !imdb_id.is_empty() {
                                                                t_item_id = target_cache.imdb_to_id.get(imdb_id).cloned();
                                                            }
                                                            if t_item_id.is_none() && !tmdb_id.is_empty() {
                                                                t_item_id = target_cache.tmdb_to_id.get(tmdb_id).cloned();
                                                            }
                                                            let t_user_id = target_cache.users.get(&user_lower).cloned();
                                                            (t_item_id, t_user_id)
                                                        };

                                                        if let (Some(t_item_id), Some(t_user_id)) = (target_item_id, target_user_id) {
                                                            state.last_syncs.insert(history_key, SyncHistoryValue {
                                                                position_ticks: position,
                                                                timestamp: now,
                                                            });

                                                            info!(
                                                                "Syncing playstate for user '{}' from {} -> {}: '{}' at {:.1}s (paused: {})",
                                                                user_name,
                                                                if is_source_emby { "Emby" } else { "Jellyfin" },
                                                                if is_source_emby { "Jellyfin" } else { "Emby" },
                                                                item.name.as_deref().unwrap_or(&item.id),
                                                                position as f64 / 10_000_000.0,
                                                                is_paused
                                                            );

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
                        Ok(_) => {}
                        Err(e) => {
                            error!("WebSocket stream error: {}", e);
                            break;
                        }
                    }
                }
                warn!("{} WebSocket disconnected. Reconnecting in 5 seconds...", if is_source_emby { "Emby" } else { "Jellyfin" });
            }
            Err(e) => {
                error!("Failed to connect to {} WebSocket: {}. Retrying in 5 seconds...", if is_source_emby { "Emby" } else { "Jellyfin" }, e);
            }
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
