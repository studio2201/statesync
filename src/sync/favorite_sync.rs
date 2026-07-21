use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{error, info};

use super::resolve::{resolve_item_providers, resolve_target_item, resolve_target_user};
use crate::client::MediaClient;
use crate::config::Config;
use crate::state::{AppState, SyncHistoryValue};

pub async fn sync_favorite_to_targets(
    user_name: &str,
    source_item_id: &str,
    is_favorite: bool,
    source_name: &str,
    source_index: usize,
    state_lock: &Arc<Mutex<AppState>>,
    target_clients: &[(usize, Arc<MediaClient>)],
    config: &Config,
    source_client: &Arc<MediaClient>,
    item_name: Option<String>,
) {
    if !config.sync.live_favorites {
        return;
    }
    if !config.sync.user_allowed(user_name, &config.user_mappings) {
        return;
    }
    {
        let st = state_lock.lock().await;
        if st
            .sync_force
            .force_sync_in_progress
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            tracing::debug!(
                "force-sync in progress; skipping live favorite for {} on {}",
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
        _ => format!("item ID '{}'", source_item_id),
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
        let mut state = state_lock.lock().await;
        state.log_event_detail(
            "warn",
            &format!("Skipped favorite for '{}' (no IMDb/TMDb)", item_title),
            Some(format!(
                "user={} source={} item={}",
                user_name, source_name, source_item_id
            )),
        );
        return;
    }

    for &(target_index, ref client_target) in target_clients {
        if config.servers[target_index].sync_direction == "send" {
            continue;
        }
        let target_name = {
            let state = state_lock.lock().await;
            if target_index >= state.caches.len() {
                continue;
            }
            state.caches[target_index].name.clone()
        };
        let target_user_id =
            resolve_target_user(target_index, &user_lower, client_target, config, state_lock).await;
        if target_user_id.is_none() {
            continue;
        }
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
            let history_key = (
                user_lower.clone(),
                if !imdb_id.is_empty() {
                    imdb_id.clone()
                } else {
                    tmdb_id.clone()
                },
            );
            let now = Instant::now();
            let mut state = state_lock.lock().await;
            if let Some(last) = state.last_syncs.get(&history_key) {
                if last.favorite == Some(is_favorite)
                    && last.timestamp.elapsed() < Duration::from_secs(5)
                {
                    continue;
                }
            }
            let message = if is_favorite {
                format!("{} favorited '{}'", user_name, item_title)
            } else {
                format!("{} unfavorited '{}'", user_name, item_title)
            };
            let prev = state.last_syncs.get(&history_key).cloned();
            state.log_sync(crate::state::SyncLogEntry {
                timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                level: "success".to_string(),
                message: message.clone(),
                detail: Some(format!(
                    "source={} → target={} | user={} | item_src={} item_tgt={} | imdb={} tmdb={} | IsFavorite={}",
                    source_name,
                    target_name,
                    user_name,
                    source_item_id,
                    t_item_id,
                    if imdb_id.is_empty() { "—" } else { &imdb_id },
                    if tmdb_id.is_empty() { "—" } else { &tmdb_id },
                    is_favorite
                )),
                source_name: Some(source_name.to_string()),
                source_is_emby: Some(config.servers[source_index].is_emby),
                target_name: Some(target_name.clone()),
                target_is_emby: Some(config.servers[target_index].is_emby),
            });
            info!("{}", message);
            state.last_syncs.insert(
                history_key.clone(),
                SyncHistoryValue {
                    position_ticks: prev.as_ref().map(|p| p.position_ticks).unwrap_or(0),
                    timestamp: now,
                    played: prev.as_ref().map(|p| p.played).unwrap_or(false),
                    favorite: Some(is_favorite),
                },
            );
            let client_target_clone = client_target.clone();
            let target_name_clone = target_name.clone();
            let state_lock_clone = state_lock.clone();
            let history_key_clone = history_key.clone();
            drop(state);
            tokio::spawn(async move {
                if let Err(e) = client_target_clone
                    .update_favorite(&t_user_id, &t_item_id, is_favorite)
                    .await
                {
                    error!("Error updating target favorite: {}", e);
                    let mut state = state_lock_clone.lock().await;
                    state.last_syncs.remove(&history_key_clone);
                    state.log_event_detail(
                        "error",
                        &format!("Favorite sync failed to '{}'", target_name_clone),
                        Some(e.to_string()),
                    );
                }
            });
        }
    }
}
