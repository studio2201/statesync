use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{error, info};

use crate::client::MediaClient;
use crate::config::Config;
use crate::state::{AppState, SyncHistoryValue};
use super::resolve::{resolve_item_providers, resolve_target_user, resolve_target_item};

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
    source_client: &Arc<MediaClient>,
    item_name: Option<String>,
) {
    {
        let st = state_lock.lock().await;
        if st
            .sync_force
            .force_sync_in_progress
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            tracing::debug!(
                "force-sync in progress; skipping live sync for {} on {}",
                user_name,
                source_name
            );
            return;
        }
    }
    let _permit = super::sync_semaphore().acquire().await;
    let user_lower = user_name.to_lowercase();

    let item_title = match item_name {
        Some(ref name) if !name.is_empty() => name.clone(),
        _ => {
            let mut found_name = None;
            {
                let st = state_lock.lock().await;
                for (_, name, _, _, id) in st.active_sessions.values() {
                    if id == source_item_id {
                        found_name = Some(name.clone());
                        break;
                    }
                }
            }
            if let Some(name) = found_name {
                name
            } else {
                let src_user_id = {
                    let st = state_lock.lock().await;
                    st.caches[source_index].users.get(&user_lower).cloned()
                };
                if let Some(uid) = src_user_id {
                    match source_client.get_item_name(&uid, source_item_id).await {
                        Ok(name) => name,
                        Err(_) => format!("item ID '{}'", source_item_id),
                    }
                } else {
                    format!("item ID '{}'", source_item_id)
                }
            }
        }
    };

    let (imdb_id, tmdb_id) = match resolve_item_providers(
        source_index,
        source_item_id,
        source_client,
        &user_lower,
        state_lock,
        source_name,
    )
    .await
    {
        Some(ids) => ids,
        None => return,
    };

    if imdb_id.is_empty() && tmdb_id.is_empty() {
        return;
    }

    for &(target_index, ref client_target) in target_clients {
        if config.servers[target_index].sync_direction == "send" {
            continue;
        }

        let target_user_id =
            resolve_target_user(target_index, &user_lower, client_target, config, state_lock).await;

        let target_name = {
            let state = state_lock.lock().await;
            if target_index >= state.caches.len() {
                continue;
            }
            state.caches[target_index].name.clone()
        };

        let target_item_id = resolve_target_item(
            target_index,
            &imdb_id,
            &tmdb_id,
            &target_name,
            target_user_id.as_deref(),
            client_target,
            state_lock,
        )
        .await;

        if let (Some(t_item_id), Some(t_user_id)) = (target_item_id, target_user_id) {
            let now = Instant::now();
            let history_key = (
                user_lower.clone(),
                if !imdb_id.is_empty() {
                    imdb_id.clone()
                } else {
                    tmdb_id.clone()
                },
            );

            let mut state = state_lock.lock().await;
            if let Some(last_sync) = state.last_syncs.get(&history_key) {
                let tick_diff = last_sync.position_ticks.abs_diff(position);
                let time_diff = last_sync.timestamp.elapsed();

                if tick_diff < (config.sync_threshold_seconds as u64 * 10_000_000)
                    && time_diff < Duration::from_secs(5)
                    && !played
                {
                    continue;
                }
            }

            let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
            let pos_secs = position as f64 / 10_000_000.0;
            let message = if played {
                format!(
                    "{} finished watching '{}'",
                    user_name, item_title
                )
            } else {
                let h = (pos_secs / 3600.0).floor() as u32;
                let m = ((pos_secs % 3600.0) / 60.0).floor() as u32;
                let s = (pos_secs % 60.0).floor() as u32;
                let duration_str = if h > 0 {
                    format!("{:02}:{:02}:{:02}", h, m, s)
                } else {
                    format!("{:02}:{:02}", m, s)
                };
                format!(
                    "{} synced progress on '{}' to {}",
                    user_name, item_title, duration_str
                )
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

            // Optimistic insertion to prevent feedback loops during the async HTTP update.
            state.last_syncs.insert(
                history_key.clone(),
                SyncHistoryValue {
                    position_ticks: position,
                    timestamp: now,
                },
            );

            if state.last_syncs.len() > 10_000 {
                let cutoff = std::time::Instant::now() - std::time::Duration::from_secs(86400);
                state.last_syncs.retain(|_, v| v.timestamp > cutoff);
            }

            let client_target_clone = client_target.clone();
            let target_name_clone = target_name.clone();
            let state_lock_clone = state_lock.clone();
            let history_key_clone = history_key.clone();
            let t_item_id_for_update = t_item_id.clone();
            let t_user_id_for_update = t_user_id.clone();
            drop(state);

            tokio::spawn(async move {
                let res = client_target_clone
                    .update_progress(
                        &t_user_id_for_update,
                        &t_item_id_for_update,
                        position,
                        played,
                    )
                    .await;
                if let Err(e) = res {
                    error!("Error updating target playstate: {}", e);
                    let mut state = state_lock_clone.lock().await;
                    state.last_syncs.remove(&history_key_clone);
                    state.log_event(
                        "error",
                        &format!("Sync failed to '{}': {}", target_name_clone, e),
                    );
                }
            });
        }
    }
}
