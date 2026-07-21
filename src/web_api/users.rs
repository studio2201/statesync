//! Per-user actions (clear watched).

use axum::{Extension, Json, body::Body, http::StatusCode, response::Response};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

use crate::client::MediaClient;
use crate::config::Config;
use crate::web::WebServerState;

#[derive(Debug, Deserialize)]
pub struct ClearWatchedRequest {
    /// Display username from the dashboard (any linked alias works).
    pub name: String,
}

/// POST /api/users/clear_watched — mark all played items unwatched for this person on every server.
pub async fn post_clear_watched(
    Extension(state): Extension<Arc<WebServerState>>,
    Json(req): Json<ClearWatchedRequest>,
) -> Response {
    let name = req.name.trim().to_string();
    if name.is_empty() || name.len() > 64 {
        return json_err(StatusCode::BAD_REQUEST, "name is required");
    }

    let tracker = {
        let st = state.app_state.lock().await;
        st.sync_force.clone()
    };
    {
        let mut running = tracker.running.lock().await;
        if *running {
            return json_err(
                StatusCode::CONFLICT,
                "force sync or clear-watched already in progress",
            );
        }
        *running = true;
    }
    tracker
        .force_sync_in_progress
        .store(true, std::sync::atomic::Ordering::SeqCst);

    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            *tracker.running.lock().await = false;
            tracker
                .force_sync_in_progress
                .store(false, std::sync::atomic::Ordering::SeqCst);
            return json_err(StatusCode::INTERNAL_SERVER_ERROR, &format!("config: {}", e));
        }
    };
    if config.servers.is_empty() {
        *tracker.running.lock().await = false;
        tracker
            .force_sync_in_progress
            .store(false, std::sync::atomic::Ordering::SeqCst);
        return json_err(StatusCode::BAD_REQUEST, "no servers configured");
    }

    {
        let mut st = state.app_state.lock().await;
        st.log_event_detail(
            "warn",
            &format!("Clear watched started for '{}'", name),
            Some("Marks all played items unwatched on every connected server for this person. Irreversible.".into()),
        );
    }

    let app_state = state.app_state.clone();
    let name_clone = name.clone();
    tokio::spawn(async move {
        let result = clear_watched_for_user(&app_state, &config, &name_clone).await;
        {
            let mut st = app_state.lock().await;
            match result {
                Ok(summary) => {
                    st.log_event_detail(
                        "success",
                        &format!("Clear watched finished for '{}'", name_clone),
                        Some(summary),
                    );
                }
                Err(e) => {
                    st.log_event_detail(
                        "error",
                        &format!("Clear watched failed for '{}'", name_clone),
                        Some(e),
                    );
                }
            }
        }
        tracker
            .force_sync_in_progress
            .store(false, std::sync::atomic::Ordering::SeqCst);
        *tracker.running.lock().await = false;
    });

    Response::builder()
        .status(StatusCode::ACCEPTED)
        .body(Body::from(
            json!({
                "status": "ok",
                "message": format!("Clearing watched history for '{}' on all servers…", name)
            })
            .to_string(),
        ))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(500)
                .body(Body::from("Internal Server Error"))
                .unwrap_or_default()
        })
}

async fn clear_watched_for_user(
    app_state: &Arc<tokio::sync::Mutex<crate::state::AppState>>,
    config: &Config,
    name: &str,
) -> Result<String, String> {
    let page_size = 200usize;
    let mut total_cleared = 0u64;
    let mut total_failed = 0u64;
    let mut servers_touched = 0u32;

    for (i, s) in config.servers.iter().enumerate() {
        let client = MediaClient::new(s.url.clone(), s.api_key.clone(), s.is_emby);
        let user_id = {
            let st = app_state.lock().await;
            let cache = st
                .caches
                .get(i)
                .ok_or_else(|| "cache missing".to_string())?;
            // Prefer exact key match (users map is lowercased keys in some paths)
            let direct = cache.users.get(&name.to_lowercase()).cloned();
            direct.or_else(|| {
                crate::state::find_mapped_user_id(name, &cache.users, &config.user_mappings)
            })
        };
        let Some(user_id) = user_id else {
            continue;
        };
        servers_touched += 1;
        let mut page = 0usize;
        loop {
            let items = client
                .get_user_played_items(&user_id, page * page_size, page_size)
                .await
                .map_err(|e| format!("{} list played: {}", s.name, e))?;
            if items.is_empty() {
                break;
            }
            for item in items {
                match client
                    .update_user_data(&user_id, &item.id, Some(0), Some(false), None)
                    .await
                {
                    Ok(()) => total_cleared += 1,
                    Err(_) => total_failed += 1,
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            page += 1;
            if page * page_size >= 100_000 {
                break;
            }
        }
        // Drop local debounce history for this user so live sync can re-learn.
        {
            let mut st = app_state.lock().await;
            let prefix = name.to_lowercase();
            st.last_syncs.retain(|(u, _), _| u.to_lowercase() != prefix);
        }
    }

    if servers_touched == 0 {
        return Err(format!(
            "user '{}' not found on any server cache — Refresh users first",
            name
        ));
    }
    Ok(format!(
        "servers={} cleared={} failed={}",
        servers_touched, total_cleared, total_failed
    ))
}

fn json_err(status: StatusCode, message: &str) -> Response {
    Response::builder()
        .status(status)
        .body(Body::from(
            json!({ "status": "error", "message": message }).to_string(),
        ))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(500)
                .body(Body::from("Internal Server Error"))
                .unwrap_or_default()
        })
}
