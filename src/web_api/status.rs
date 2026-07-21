use axum::{Extension, Json};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::{Config, redacted_url};
use crate::state::AppState;
use crate::web::WebServerState;

/// Missing documentation.
pub struct CacheStats {
    /// Missing documentation.
    pub total_servers: usize,
    /// Missing documentation.
    pub connected_count: usize,
    /// Missing documentation.
    pub ever_connected_count: usize,
    /// Missing documentation.
    pub total_users: usize,
}

/// Missing documentation.
pub async fn cache_stats(app_state: &Arc<Mutex<AppState>>) -> CacheStats {
    let state = app_state.lock().await;
    let total_servers = state.caches.len();
    let connected_count = state
        .websocket_statuses
        .iter()
        .filter(|s| s.as_str() == "Connected" || s.as_str() == "Synchronizing")
        .count();
    let ever_connected_count = state
        .websocket_statuses
        .iter()
        .filter(|s| s.as_str() != "Offline")
        .count();
    let total_users: usize = state.caches.iter().map(|c| c.users.len()).sum();
    CacheStats {
        total_servers,
        connected_count,
        ever_connected_count,
        total_users,
    }
}

/// Missing documentation.
pub async fn get_status(
    Extension(state): Extension<Arc<WebServerState>>,
) -> Json<serde_json::Value> {
    let config = Config::load().unwrap_or_else(|_| crate::config::default_config());
    let redacted_url_for = |name: &str| -> String {
        config
            .servers
            .iter()
            .find(|s| s.name == name)
            .map(|s| redacted_url(&s.url))
            .unwrap_or_default()
    };

    // Refactored to clone state fields and release the lock immediately,
    // avoiding long-lived lock holding during serialization and user mapping computations.
    let (caches_data, websocket_statuses, active_sessions, sync_logs, sync_force) = {
        let app_state = state.app_state.lock().await;
        let caches_data: Vec<(String, HashMap<String, String>, usize)> = app_state
            .caches
            .iter()
            .map(|c| (c.name.clone(), c.users.clone(), c.id_to_providers.len()))
            .collect();
        (
            caches_data,
            app_state.websocket_statuses.clone(),
            app_state.active_sessions.clone(),
            app_state.sync_logs.clone(),
            app_state.sync_force.clone(),
        )
    };

    let tracker_status = sync_force.status.lock().await.clone();

    let mut servers_status = Vec::new();
    let mut users_by_server = Vec::new();
    for (i, (name, users, media_len)) in caches_data.iter().enumerate() {
        let ws_status = websocket_statuses
            .get(i)
            .cloned()
            .unwrap_or_else(|| "Offline".to_string());
        servers_status.push(json!({
            "name": name,
            "url": redacted_url_for(name),
            "users_count": users.len(),
            "media_count": media_len,
            "websocket_status": ws_status
        }));
        let mut names: Vec<String> = users.keys().cloned().collect();
        names.sort();
        users_by_server.push(json!({
            "index": i,
            "name": name,
            "users": names,
        }));
    }

    let mut users: Vec<serde_json::Value> = Vec::new();
    let mut processed: HashSet<(usize, String)> = HashSet::new();
    for (srv_idx, (_, users_map, _)) in caches_data.iter().enumerate() {
        let mut sorted_users: Vec<&String> = users_map.keys().collect();
        sorted_users.sort();
        for username in sorted_users {
            if processed.contains(&(srv_idx, username.clone())) {
                continue;
            }
            let mut servers_idx = vec![srv_idx];
            processed.insert((srv_idx, username.clone()));
            for (other_idx, (_, other_users_map, _)) in caches_data.iter().enumerate() {
                if other_idx == srv_idx {
                    continue;
                }
                let matched_name = crate::state::find_mapped_user_id(
                    username,
                    other_users_map,
                    &config.user_mappings,
                )
                .and_then(|target_id| {
                    other_users_map
                        .iter()
                        .find(|(_, id)| *id == &target_id)
                        .map(|(name, _)| name.clone())
                });
                if let Some(name) = matched_name {
                    servers_idx.push(other_idx);
                    processed.insert((other_idx, name));
                }
            }
            servers_idx.sort();
            users.push(json!({
                "name": username,
                "servers": servers_idx,
            }));
        }
    }

    let mut sessions_json = Vec::new();
    for ((server, _), (user, item, position, is_paused, item_id)) in &active_sessions {
        let poster_url = format!(
            "/api/poster?server={}&item_id={}",
            utf8_percent_encode(server, NON_ALPHANUMERIC),
            item_id
        );
        sessions_json.push(json!({
            "server": server,
            "user": user,
            "item": item,
            "position": position,
            "is_paused": is_paused,
            "poster_url": poster_url
        }));
    }

    let last_full_sync = json!({
        "state": tracker_status.state,
        "started_at": tracker_status.started_at,
        "finished_at": tracker_status.finished_at,
        "processed": tracker_status.processed,
        "succeeded": tracker_status.succeeded,
        "skipped": tracker_status.skipped,
        "failed": tracker_status.failed,
        "total_pairs": tracker_status.total_pairs,
        "current_user": tracker_status.current_user,
        "last_error": tracker_status.last_error,
        "phase": tracker_status.phase,
        "by_field": tracker_status.by_field,
        "scope": tracker_status.scope,
        "skip_reasons": tracker_status.skip_reasons,
        "dry_run": tracker_status.dry_run,
    });

    Json(json!({
        "status": "active",
        "version": state.version,
        "started_at": state.started_at,
        "uptime_seconds": state.started_instant.elapsed().as_secs(),
        "servers": servers_status,
        "users": users,
        "users_by_server": users_by_server,
        "user_mappings": config.user_mappings,
        "sync": config.sync,
        "active_sessions": sessions_json,
        "sync_logs": sync_logs,
        "last_full_sync": last_full_sync
    }))
}
