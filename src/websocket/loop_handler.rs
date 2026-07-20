use futures_util::{SinkExt, StreamExt};
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
use super::{next_backoff, redact_api_key, handlers};

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
            if let Err(e) = handlers::init_cache_in_background(
                source_index,
                &source_name,
                &source_client,
                &state_lock,
            )
            .await
            {
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
                state.websocket_statuses[source_index] = "Synchronizing".to_string();
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
                                            handlers::handle_sessions_event(
                                                sessions,
                                                source_index,
                                                &source_name,
                                                &source_client,
                                                &target_clients,
                                                &state_lock,
                                                &config,
                                            )
                                            .await;
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
                                            handlers::handle_userdata_changed_event(
                                                info,
                                                source_index,
                                                &source_name,
                                                &source_client,
                                                &target_clients,
                                                &state_lock,
                                                &config,
                                            )
                                            .await;
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
                let err_str = redact_api_key(&e.to_string());
                error!("Failed to connect to '{}' WebSocket: {}.", source_name, err_str);
                state_lock.lock().await.log_event(
                    "error",
                    &format!("Failed to connect to '{}' WebSocket: {}", source_name, err_str),
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
