use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

use crate::client::{MediaClient, SessionInfo, UserDataChangedInfo};
use crate::config::Config;
use crate::state::AppState;
use super::spawn_userdata_sync;

pub async fn init_cache_in_background(
    source_index: usize,
    source_name: &str,
    source_client: &Arc<MediaClient>,
    state_lock: &Arc<Mutex<AppState>>,
) -> Result<(), anyhow::Error> {
    info!(
        "Attempting background cache initialization for '{}'...",
        source_name
    );
    let cache = crate::state::init_server_cache(source_name, source_client).await?;
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
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_sessions_event(
    sessions: Vec<SessionInfo>,
    source_index: usize,
    source_name: &str,
    source_client: &Arc<MediaClient>,
    target_clients: &[(usize, Arc<MediaClient>)],
    state_lock: &Arc<Mutex<AppState>>,
    config: &Config,
) {
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
        if let Ok(new_users) = source_client.get_users().await {
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
        .retain(|(srv, _), _| srv != source_name);

    for s in &sessions {
        if let (Some(user_name), Some(item), Some(play_state)) = (
            &s.user_name,
            &s.now_playing_item,
            &s.play_state,
        ) {
            let Some(position) = play_state.position_ticks else {
                continue;
            };
            let is_paused = play_state.is_paused.unwrap_or(false);
            let pos_secs = position as f64 / 10_000_000.0;
            state.active_sessions.insert(
                (source_name.to_string(), s.id.clone()),
                (
                    user_name.clone(),
                    item.name.clone().unwrap_or_default(),
                    pos_secs,
                    is_paused,
                    item.id.clone(),
                ),
            );

            if config.servers[source_index].sync_direction == "receive" {
                continue;
            }

            let user_name_clone = user_name.clone();
            let item_id_clone = item.id.clone();
            let item_name_opt = item.name.clone();
            let source_name_clone = source_name.to_string();
            let state_lock_clone = state_lock.clone();
            let target_clients_clone = target_clients.to_vec();
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

#[allow(clippy::too_many_arguments)]
pub async fn handle_userdata_changed_event(
    info: UserDataChangedInfo,
    source_index: usize,
    source_name: &str,
    source_client: &Arc<MediaClient>,
    target_clients: &[(usize, Arc<MediaClient>)],
    state_lock: &Arc<Mutex<AppState>>,
    config: &Config,
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
            if config.servers[source_index].sync_direction == "receive" {
                continue;
            }

            let user_name_clone = user_name.clone();
            let item_id_clone = entry.item_id.clone();
            let Some(pos) = entry.playback_position_ticks else {
                if !entry.played {
                    continue;
                }
                spawn_userdata_sync(
                    user_name_clone,
                    item_id_clone,
                    0,
                    entry.played,
                    source_name.to_string(),
                    source_index,
                    state_lock.clone(),
                    target_clients.to_vec(),
                    config.clone(),
                    source_client.clone(),
                );
                continue;
            };
            spawn_userdata_sync(
                user_name_clone,
                item_id_clone,
                pos,
                entry.played,
                source_name.to_string(),
                source_index,
                state_lock.clone(),
                target_clients.to_vec(),
                config.clone(),
                source_client.clone(),
            );
        }
    }
}
