//! Sessions event handling.

use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

use crate::client::{MediaClient, SessionInfo};
use crate::config::Config;
use crate::state::AppState;

/// Hot-load server metadata cache in the background after connect failures.
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
                    && !state.caches[source_index].users.contains_key(&user_lower)
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
                state.caches[source_index].merge_users(new_users);
            }
        }
    }

    // Snapshot previous sessions so we can skip unchanged positions (poll is ~1s).
    let prev_sessions = {
        let state = state_lock.lock().await;
        state
            .active_sessions
            .iter()
            .filter(|((srv, _), _)| srv == source_name)
            .map(|((_, sid), v)| (sid.clone(), v.clone()))
            .collect::<std::collections::HashMap<_, _>>()
    };

    let mut state = state_lock.lock().await;
    state
        .active_sessions
        .retain(|(srv, _), _| srv != source_name);

    for s in &sessions {
        if let (Some(user_name), Some(item), Some(play_state)) =
            (&s.user_name, &s.now_playing_item, &s.play_state)
        {
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
        }
    }
    drop(state);

    if config.servers[source_index].sync_direction == "receive" {
        return;
    }
    if !config.sync.live_position && !config.sync.live_played {
        return;
    }

    // Only sync on meaningful position/pause change (poll is ~1s).
    let thresh = (config.sync_threshold_seconds as i64).saturating_mul(10_000_000);
    for s in &sessions {
        if let (Some(user_name), Some(item), Some(play_state)) =
            (&s.user_name, &s.now_playing_item, &s.play_state)
        {
            let Some(position) = play_state.position_ticks else {
                continue;
            };
            let is_paused = play_state.is_paused.unwrap_or(false);
            if let Some((_, _, prev_secs, prev_paused, prev_item)) = prev_sessions.get(&s.id) {
                let prev_ticks = (*prev_secs * 10_000_000.0) as i64;
                if prev_item == &item.id
                    && (prev_ticks.abs_diff(position) as i64) < thresh
                    && *prev_paused == is_paused
                {
                    continue;
                }
            }
            let (un, iid, iname, sn, st, tc, cfg, sc) = (
                user_name.clone(),
                item.id.clone(),
                item.name.clone(),
                source_name.to_string(),
                state_lock.clone(),
                target_clients.to_vec(),
                config.clone(),
                source_client.clone(),
            );
            tokio::spawn(async move {
                crate::sync::sync_progress_to_targets(
                    &un,
                    &iid,
                    position,
                    false,
                    &sn,
                    source_index,
                    &st,
                    &tc,
                    &cfg,
                    &sc,
                    iname,
                )
                .await;
            });
        }
    }
}
