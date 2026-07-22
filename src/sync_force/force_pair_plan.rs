use crate::config::Config;
use crate::state::AppState;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Build (src_idx, tgt_idx, username, src_user_id, tgt_user_id) pairs for force mesh.
///
/// `only_user`: when set, only that person (+ linked aliases) is included — intentional
/// per-user force, even if they are on the ignore list.
pub async fn plan_force_pairs(
    config: &Config,
    state: &Arc<Mutex<AppState>>,
    only_user: Option<&str>,
) -> Vec<(usize, usize, String, String, String)> {
    let sources: Vec<usize> = (0..config.servers.len())
        .filter(|&i| config.servers[i].sync_direction != "receive")
        .collect();
    let targets: Vec<usize> = (0..config.servers.len())
        .filter(|&i| config.servers[i].sync_direction != "send")
        .collect();

    let state_guard = state.lock().await;
    let mut result = Vec::new();
    for &src in &sources {
        let cache = match state_guard.caches.get(src) {
            Some(c) => c,
            None => continue,
        };
        for (username, src_user_id) in &cache.users {
            if let Some(filter) = only_user {
                if !crate::config::SyncOptions::user_matches_filter(
                    username,
                    filter,
                    &config.user_mappings,
                ) {
                    continue;
                }
            } else if !config.sync.user_allowed(username, &config.user_mappings) {
                continue;
            }
            for &tgt in &targets {
                if src == tgt {
                    continue;
                }
                if let Some(tgt_cache) = state_guard.caches.get(tgt) {
                    if let Some(tgt_id) = crate::state::find_mapped_user_id(
                        username,
                        &tgt_cache.users,
                        &config.user_mappings,
                    ) {
                        result.push((src, tgt, username.clone(), src_user_id.clone(), tgt_id));
                    }
                }
            }
        }
    }
    result
}
