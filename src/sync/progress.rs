use super::resolve::{resolve_item_providers, resolve_target_item, resolve_target_user};
use crate::client::MediaClient;
use crate::config::Config;
use crate::state::{AppState, SyncHistoryValue};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{error, info};
/// Live-sync playback progress and/or played flag to all target servers.
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
    let send_position = config.sync.live_position;
    let send_played = config.sync.live_played;
    if !(send_position || (send_played && played)) {
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
                "force-sync in progress; skipping live sync for {} on {}",
                user_name,
                source_name
            );
            return;
        }
    }
    let _permit = super::sync_semaphore().acquire().await;
    let user_lower = user_name.to_lowercase();
    let item_title = super::live_item_title::resolve_live_item_title(
        item_name,
        source_item_id,
        &user_lower,
        source_index,
        state_lock,
        source_client,
    )
    .await;
    let providers = match resolve_item_providers(
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
    if providers.is_empty() {
        return;
    }
    let history_provider = match providers.history_key() {
        Some(k) => k,
        None => return,
    };
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
            let mut state = state_lock.lock().await;
            state.log_event_detail(
                "warn",
                &format!(
                    "No mapped user for '{}' on target '{}' — progress not synced",
                    user_name, target_name
                ),
                Some(format!(
                    "source_user={} source_server={} target_server={}. Open Settings → link users, or make usernames match.",
                    user_name, source_name, target_name
                )),
            );
            continue;
        }
        let target_item_id = resolve_target_item(
            target_index,
            &providers,
            &target_name,
            target_user_id.as_deref(),
            client_target,
            state_lock,
        )
        .await;
        if target_item_id.is_none() {
            let mut state = state_lock.lock().await;
            state.log_event_detail(
                "warn",
                &format!(
                    "No matching library item on '{}' for '{}'",
                    target_name, item_title
                ),
                Some(format!(
                    "{} source_item={} source_server={}",
                    providers.display_short(),
                    source_item_id,
                    source_name
                )),
            );
            continue;
        }
        if let (Some(t_item_id), Some(t_user_id)) = (target_item_id, target_user_id) {
            let now = Instant::now();
            let history_key = (user_lower.clone(), history_provider.clone());
            let mut state = state_lock.lock().await;
            if let Some(last_sync) = state.last_syncs.get(&history_key) {
                let tick_diff = last_sync.position_ticks.abs_diff(position);
                let time_diff = last_sync.timestamp.elapsed();
                let within_threshold = tick_diff
                    < (config.sync_threshold_seconds.saturating_mul(10_000_000))
                    && time_diff < Duration::from_secs(5);
                if within_threshold && last_sync.played == played {
                    continue;
                }
            }
            let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
            let message = super::progress_message::format_progress_message(
                user_name,
                &item_title,
                position,
                played,
                send_played,
            );
            let log_entry = crate::state::SyncLogEntry {
                timestamp,
                level: "success".to_string(),
                message: message.clone(),
                detail: Some(format!(
                    "source={} → target={} | user={} | item_src={} item_tgt={} | {} | ticks={} played={} | live_position={} live_played={}",
                    source_name,
                    target_name,
                    user_name,
                    source_item_id,
                    t_item_id,
                    providers.display_short(),
                    position,
                    played,
                    send_position,
                    send_played
                )),
                source_name: Some(source_name.to_string()),
                source_is_emby: Some(config.servers[source_index].is_emby),
                target_name: Some(target_name.clone()),
                target_is_emby: Some(config.servers[target_index].is_emby),
            };
            info!("{}", message);
            state.log_sync(log_entry);
            let prev_fav = state.last_syncs.get(&history_key).and_then(|v| v.favorite);
            state.last_syncs.insert(
                history_key.clone(),
                SyncHistoryValue {
                    position_ticks: position,
                    timestamp: now,
                    played,
                    favorite: prev_fav,
                },
            );
            if state.last_syncs.len() > 10_000 {
                let cutoff = std::time::Instant::now() - std::time::Duration::from_secs(86400);
                state.last_syncs.retain(|_, v| v.timestamp > cutoff);
                if state.last_syncs.len() > 9_000 {
                    state.last_syncs.clear();
                }
            }
            let client_target_clone = client_target.clone();
            let target_name_clone = target_name.clone();
            let state_lock_clone = state_lock.clone();
            let history_key_clone = history_key.clone();
            let t_item_id_for_update = t_item_id.clone();
            let t_user_id_for_update = t_user_id.clone();
            let pos_opt = if send_position { Some(position) } else { None };
            let played_opt = if send_played { Some(played) } else { None };
            drop(state);
            tokio::spawn(async move {
                let res = client_target_clone
                    .update_user_data(
                        &t_user_id_for_update,
                        &t_item_id_for_update,
                        pos_opt,
                        played_opt,
                        None,
                    )
                    .await;
                if let Err(e) = res {
                    error!("Error updating target playstate: {}", e);
                    let mut state = state_lock_clone.lock().await;
                    state.last_syncs.remove(&history_key_clone);
                    state.log_event_detail(
                        "error",
                        &format!("Sync failed to '{}'", target_name_clone),
                        Some(e.to_string()),
                    );
                }
            });
        }
    }
}
