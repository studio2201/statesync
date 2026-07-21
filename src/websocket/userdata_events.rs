//! UserDataChanged event handling.

use super::spawn_userdata_sync;
use crate::client::{MediaClient, UserDataChangedInfo};
use crate::config::Config;
use crate::state::AppState;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

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
    let mut user_name = {
        let state = state_lock.lock().await;
        if source_index >= state.caches.len() {
            return;
        }
        state.caches[source_index]
            .users
            .iter()
            .find(|(_, id)| *id == &info.user_id)
            .map(|(name, _)| name.clone())
    };
    // Hot-reload users on cache miss (same pattern as Sessions).
    if user_name.is_none() {
        info!(
            "User id {} not in cache on '{}'; hot-reloading users...",
            info.user_id, source_name
        );
        if let Ok(new_users) = source_client.get_users().await {
            let mut state = state_lock.lock().await;
            if source_index < state.caches.len() {
                state.caches[source_index].merge_users(new_users);
                user_name = state.caches[source_index]
                    .users
                    .iter()
                    .find(|(_, id)| *id == &info.user_id)
                    .map(|(name, _)| name.clone());
            }
        }
    }
    let Some(user_name) = user_name else {
        tracing::debug!(
            "Dropping UserDataChanged for unknown user id {} on '{}'",
            info.user_id,
            source_name
        );
        return;
    };
    for entry in &info.user_data_list {
        if config.servers[source_index].sync_direction == "receive" {
            continue;
        }

        // Favorites path (independent of progress).
        if let Some(is_fav) = entry.is_favorite {
            if config.sync.live_favorites {
                let un = user_name.clone();
                let iid = entry.item_id.clone();
                let sn = source_name.to_string();
                let st = state_lock.clone();
                let tc = target_clients.to_vec();
                let cfg = config.clone();
                let sc = source_client.clone();
                tokio::spawn(async move {
                    crate::sync::sync_favorite_to_targets(
                        &un,
                        &iid,
                        is_fav,
                        &sn,
                        source_index,
                        &st,
                        &tc,
                        &cfg,
                        &sc,
                        None,
                    )
                    .await;
                });
            }
        }

        let user_name_clone = user_name.clone();
        let item_id_clone = entry.item_id.clone();
        let Some(pos) = entry.playback_position_ticks else {
            if !entry.played {
                continue;
            }
            if !config.sync.live_played {
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
        if !(config.sync.live_position || (config.sync.live_played && entry.played)) {
            continue;
        }
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
