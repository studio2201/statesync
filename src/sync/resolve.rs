use crate::client::MediaClient;
use crate::config::Config;
use crate::state::AppState;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

/// Missing documentation.
pub async fn resolve_item_providers(
    source_index: usize,
    source_item_id: &str,
    source_client: &Arc<MediaClient>,
    user_lower: &str,
    state_lock: &Arc<Mutex<AppState>>,
    source_name: &str,
) -> Option<(String, String)> {
    let state = state_lock.lock().await;
    if source_index >= state.caches.len() {
        return None;
    }

    if let Some(provs) = state.caches[source_index]
        .id_to_providers
        .get(source_item_id)
    {
        Some(provs.clone())
    } else {
        drop(state);
        let src_user_id = {
            let state_read = state_lock.lock().await;
            state_read.caches[source_index]
                .users
                .get(user_lower)
                .cloned()
        };

        if let Some(uid) = src_user_id {
            info!(
                "Cache miss on '{}' for item {}. Resolving details dynamically...",
                source_name, source_item_id
            );
            if let Ok((imdb, tmdb)) = source_client.get_item_providers(&uid, source_item_id).await {
                let mut state_write = state_lock.lock().await;
                state_write.caches[source_index]
                    .id_to_providers
                    .insert(source_item_id.to_string(), (imdb.clone(), tmdb.clone()));
                if !imdb.is_empty() {
                    state_write.caches[source_index]
                        .imdb_to_id
                        .insert(imdb.clone(), source_item_id.to_string());
                }
                if !tmdb.is_empty() {
                    state_write.caches[source_index]
                        .tmdb_to_id
                        .insert(tmdb.clone(), source_item_id.to_string());
                }
                Some((imdb, tmdb))
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// Missing documentation.
pub async fn resolve_target_user(
    target_index: usize,
    user_lower: &str,
    client_target: &Arc<MediaClient>,
    config: &Config,
    state_lock: &Arc<Mutex<AppState>>,
) -> Option<String> {
    let mut state = state_lock.lock().await;
    let mut target_user_id = crate::state::find_mapped_user_id(
        user_lower,
        &state.caches[target_index].users,
        &config.user_mappings,
    );
    if target_user_id.is_none() {
        drop(state);
        if let Ok(new_users) = client_target.get_users().await {
            let mut state_write = state_lock.lock().await;
            if target_index < state_write.caches.len() {
                // Merge so a partial /Users response cannot drop known users.
                state_write.caches[target_index].merge_users(new_users);
            }
        }
        state = state_lock.lock().await;
        target_user_id = crate::state::find_mapped_user_id(
            user_lower,
            &state.caches[target_index].users,
            &config.user_mappings,
        );
    }
    target_user_id
}

/// Missing documentation.
pub async fn resolve_target_item(
    target_index: usize,
    imdb_id: &str,
    tmdb_id: &str,
    target_name: &str,
    target_user_id: Option<&str>,
    client_target: &Arc<MediaClient>,
    state_lock: &Arc<Mutex<AppState>>,
) -> Option<String> {
    let mut state = state_lock.lock().await;
    let mut target_item_id = None;
    let mut is_negative_cached = false;
    {
        let target_cache = &state.caches[target_index];
        if !imdb_id.is_empty() {
            target_item_id = target_cache.imdb_to_id.get(imdb_id).cloned();
        }
        if target_item_id.is_none() && !tmdb_id.is_empty() {
            target_item_id = target_cache.tmdb_to_id.get(tmdb_id).cloned();
        }
        if let Some(ref id) = target_item_id {
            if id == "[ NOT_FOUND ]" {
                is_negative_cached = true;
                target_item_id = None;
            }
        }
    }

    if target_item_id.is_none() && !is_negative_cached {
        drop(state);
        let mut resolved: Option<(String, String, String)> = None;
        let mut resolved_err: Option<String> = None;
        if let Some(t_uid) = target_user_id {
            info!(
                "Cache miss on target '{}' for (IMDb: {}, TMDb: {}). Searching target library...",
                target_name, imdb_id, tmdb_id
            );
            match client_target
                .find_item_by_provider(t_uid, imdb_id, tmdb_id)
                .await
            {
                Ok(res) => resolved = res,
                Err(e) => resolved_err = Some(e.to_string()),
            }
        }
        state = state_lock.lock().await;
        if let Some((id, _imdb, _tmdb)) = resolved {
            state.caches[target_index]
                .id_to_providers
                .insert(id.clone(), (imdb_id.to_string(), tmdb_id.to_string()));
            if !imdb_id.is_empty() {
                state.caches[target_index]
                    .imdb_to_id
                    .insert(imdb_id.to_string(), id.clone());
            }
            if !tmdb_id.is_empty() {
                state.caches[target_index]
                    .tmdb_to_id
                    .insert(tmdb_id.to_string(), id.clone());
            }
            target_item_id = Some(id);
        } else if resolved_err.is_none() {
            if !imdb_id.is_empty() {
                state.caches[target_index]
                    .imdb_to_id
                    .insert(imdb_id.to_string(), "[ NOT_FOUND ]".to_string());
            }
            if !tmdb_id.is_empty() {
                state.caches[target_index]
                    .tmdb_to_id
                    .insert(tmdb_id.to_string(), "[ NOT_FOUND ]".to_string());
            }
        } else if let Some(err) = resolved_err {
            tracing::warn!(
                "Target '{}' lookup error (will not poison cache): {}",
                target_name,
                err
            );
        }
    }
    target_item_id
}
