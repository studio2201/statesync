use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tracing::{info, error};

use crate::config::Config;
use crate::client::MediaClient;
use crate::state::{AppState, SyncHistoryValue};

pub async fn sync_progress_to_targets(
    user_name: &str,
    source_item_id: &str,
    position: i64,
    played: bool,
    source_name: &str,
    source_index: usize,
    state_lock: &Arc<Mutex<AppState>>,
    target_clients: &[(usize, Arc<MediaClient>)],
    config: &Config,
) {
    let (imdb_id, tmdb_id) = {
        let state = state_lock.lock().await;
        if source_index >= state.caches.len() { return; }
        match state.caches[source_index].id_to_providers.get(source_item_id) {
            Some(provs) => provs.clone(),
            None => return,
        }
    };

    let user_lower = user_name.to_lowercase();
    for &(target_index, ref client_target) in target_clients {
        if config.servers[target_index].sync_direction == "send" { continue; }

        let mut state = state_lock.lock().await;
        let mut target_user_id = crate::state::find_mapped_user_id(&user_lower, &state.caches[target_index].users);
        if target_user_id.is_none() {
            drop(state);
            if let Ok(new_users) = client_target.get_users().await {
                let mut state_write = state_lock.lock().await;
                if target_index < state_write.caches.len() { state_write.caches[target_index].users = new_users; }
            }
            state = state_lock.lock().await;
            target_user_id = crate::state::find_mapped_user_id(&user_lower, &state.caches[target_index].users);
        }

        let (target_item_id, target_name) = {
            let target_cache = &state.caches[target_index];
            let mut t_item_id = None;
            if !imdb_id.is_empty() { t_item_id = target_cache.imdb_to_id.get(&imdb_id).cloned(); }
            if t_item_id.is_none() && !tmdb_id.is_empty() { t_item_id = target_cache.tmdb_to_id.get(&tmdb_id).cloned(); }
            (t_item_id, target_cache.name.clone())
        };

        if let (Some(t_item_id), Some(t_user_id)) = (target_item_id, target_user_id) {
            let now = Instant::now();
            let history_key = (user_lower.clone(), if !imdb_id.is_empty() { imdb_id.clone() } else { tmdb_id.clone() });
            state.last_syncs.insert(history_key.clone(), SyncHistoryValue { position_ticks: position, timestamp: now });

            let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
            let pos_secs = position as f64 / 10_000_000.0;
            let message = if played {
                format!("Synced watch state (watched) for {} to '{}'", user_name, source_item_id)
            } else {
                format!("Synced progress for {} to {:.1}s", user_name, pos_secs)
            };

            let log_entry = crate::state::SyncLogEntry {
                timestamp,
                level: "success".to_string(),
                message: message.clone(),
                source_name: Some(source_name.to_string()),
                source_is_emby: Some(config.servers[source_index].is_emby),
                target_name: Some(target_name.clone()),
                target_is_emby: Some(config.servers[target_index].is_emby),
            };
            info!("{}", message);
            state.log_sync(log_entry);

            let client_target_clone = client_target.clone();
            let target_name_clone = target_name.clone();
            let state_lock_clone = state_lock.clone();
            tokio::spawn(async move {
                if let Err(e) = client_target_clone.update_progress(&t_user_id, &t_item_id, position, played).await {
                    error!("Error updating target playstate: {}", e);
                    state_lock_clone.lock().await.log_event("error", &format!("Sync failed to '{}': {}", target_name_clone, e));
                }
            });
        }
    }
}
