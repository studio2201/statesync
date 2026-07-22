use crate::client::{MediaClient, ProviderIds};
use crate::config::Config;
use crate::state::AppState;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

pub async fn resolve_item_providers(
    source_index: usize,
    source_item_id: &str,
    source_client: &Arc<MediaClient>,
    user_lower: &str,
    state_lock: &Arc<Mutex<AppState>>,
    source_name: &str,
) -> Option<ProviderIds> {
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
            if let Ok(provs) = source_client.get_item_providers(&uid, source_item_id).await {
                let mut state_write = state_lock.lock().await;
                state_write.caches[source_index].index_item(source_item_id.to_string(), provs.clone());
                Some(provs)
            } else {
                None
            }
        } else {
            None
        }
    }
}

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

/// Resolve target library item: in-memory ServerCache first, then HTTP search.
pub async fn resolve_target_item(
    target_index: usize,
    providers: &ProviderIds,
    target_name: &str,
    target_user_id: Option<&str>,
    client_target: &Arc<MediaClient>,
    state_lock: &Arc<Mutex<AppState>>,
) -> Option<String> {
    if providers.is_empty() {
        return None;
    }
    let mut state = state_lock.lock().await;
    if let Some(id) = state.caches[target_index].lookup_item_id(providers) {
        return Some(id);
    }
    if state.caches[target_index].is_negative_cached(providers) {
        return None;
    }
    drop(state);

    let mut resolved: Option<(String, ProviderIds)> = None;
    let mut resolved_err: Option<String> = None;
    if let Some(t_uid) = target_user_id {
        info!(
            "Cache miss on target '{}' for ({}). Searching target library...",
            target_name,
            providers.display_short()
        );
        match client_target.find_item_by_provider(t_uid, providers).await {
            Ok(res) => resolved = res,
            Err(e) => resolved_err = Some(e.to_string()),
        }
    }
    let mut state = state_lock.lock().await;
    if let Some((id, found)) = resolved {
        // Prefer known source ids; fill gaps from search result.
        let mut merged = providers.clone();
        if merged.imdb.is_empty() {
            merged.imdb = found.imdb;
        }
        if merged.tmdb.is_empty() {
            merged.tmdb = found.tmdb;
        }
        if merged.tvdb.is_empty() {
            merged.tvdb = found.tvdb;
        }
        state.caches[target_index].index_item(id.clone(), merged);
        Some(id)
    } else if resolved_err.is_none() {
        state.caches[target_index].index_not_found(providers);
        None
    } else {
        if let Some(err) = resolved_err {
            tracing::warn!(
                "Target '{}' lookup error (will not poison cache): {}",
                target_name,
                err
            );
        }
        None
    }
}
