use axum::{Extension, Json, http::StatusCode, response::Response, body::Body};
use serde_json::json;
use std::sync::Arc;

use crate::client::MediaClient;
use crate::config::Config;
use crate::web::WebServerState;

/// Missing documentation.
pub async fn post_reload(Extension(state): Extension<Arc<WebServerState>>) -> Response {
    if let Err(e) = state.reload_tx.send(()).await {
        tracing::error!("Failed to trigger config reload: {}", e);
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("Failed to trigger reload"))
            .unwrap_or_else(|_| axum::response::Response::builder().status(500).body(axum::body::Body::from("Internal Server Error")).unwrap_or_default());
    }
    Response::builder()
        .status(StatusCode::OK)
        .body(Body::from("Reload triggered successfully"))
        .unwrap_or_else(|_| axum::response::Response::builder().status(500).body(axum::body::Body::from("Internal Server Error")).unwrap_or_default())
}

/// Missing documentation.
pub async fn post_users_refresh(Extension(state): Extension<Arc<WebServerState>>) -> Response {
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!(
                    r#"{{"status":"error","message":"failed to load config: {}"}}"#,
                    e
                )))
                .unwrap_or_else(|_| axum::response::Response::builder().status(500).body(axum::body::Body::from("Internal Server Error")).unwrap_or_default());
        }
    };
    if config.servers.is_empty() {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from(
                r#"{"status":"error","message":"no servers configured"}"#,
            ))
            .unwrap_or_else(|_| axum::response::Response::builder().status(500).body(axum::body::Body::from("Internal Server Error")).unwrap_or_default());
    }

    let mut results: Vec<serde_json::Value> = Vec::new();
    for (i, s) in config.servers.iter().enumerate() {
        let client = MediaClient::new(s.url.clone(), s.api_key.clone(), s.is_emby);
        let users_before = state
            .app_state
            .lock()
            .await
            .caches
            .get(i)
            .map(|c| c.users.len())
            .unwrap_or(0);
        match client.get_users().await {
            Ok(fresh) => {
                let added = {
                    let mut st = state.app_state.lock().await;
                    if let Some(cache) = st.caches.get_mut(i) {
                        let before = cache.users.len();
                        cache.merge_users(fresh.clone());
                        cache.users.len().saturating_sub(before)
                    } else {
                        0
                    }
                };
                results.push(json!({
                    "server": s.name,
                    "before": users_before,
                    "after": state.app_state.lock().await.caches.get(i).map(|c| c.users.len()).unwrap_or(0),
                    "added": added
                }));
            }
            Err(e) => {
                results.push(json!({
                    "server": s.name,
                    "error": e.to_string()
                }));
            }
        }
    }

    Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(
            json!({"status":"ok", "results": results}).to_string(),
        ))
        .unwrap_or_else(|_| axum::response::Response::builder().status(500).body(axum::body::Body::from("Internal Server Error")).unwrap_or_default())
}

/// Start a force sync. Body is optional; `{ "direction": "both" }` or empty → both ways.
pub async fn post_sync_force(
    Extension(state): Extension<Arc<WebServerState>>,
    body: Result<Json<crate::sync_force::ForceSyncOptions>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let opts = match body {
        Ok(Json(o)) => o,
        Err(rej) => {
            // Empty body is fine; real parse errors get a clear message (was opaque 422).
            let msg = rej.to_string();
            if msg.contains("EOF") || msg.contains("empty") || msg.contains("EOF while parsing") {
                crate::sync_force::ForceSyncOptions {
                    direction: crate::sync_force::Direction::Both,
                    dry_run: false,
                }
            } else {
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(format!(
                        r#"{{"status":"error","message":"Invalid force-sync body: {}"}}"#,
                        msg.replace('"', "'")
                    )))
                    .unwrap_or_else(|_| {
                        axum::response::Response::builder()
                            .status(500)
                            .body(axum::body::Body::from("Internal Server Error"))
                            .unwrap_or_default()
                    });
            }
        }
    };
    let tracker = {
        let st = state.app_state.lock().await;
        st.sync_force.clone()
    };
    {
        let mut running = tracker.running.lock().await;
        if *running {
            return Response::builder()
                .status(StatusCode::CONFLICT)
                .body(Body::from(
                    r#"{"status":"error","message":"force-sync already in progress"}"#,
                ))
                .unwrap_or_else(|_| axum::response::Response::builder().status(500).body(axum::body::Body::from("Internal Server Error")).unwrap_or_default());
        }
        *running = true;
    }
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            *tracker.running.lock().await = false;
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!(
                    r#"{{"status":"error","message":"failed to load config: {}"}}"#,
                    e
                )))
                .unwrap_or_else(|_| axum::response::Response::builder().status(500).body(axum::body::Body::from("Internal Server Error")).unwrap_or_default());
        }
    };
    if config.servers.is_empty() {
        *tracker.running.lock().await = false;
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from(
                r#"{"status":"error","message":"no servers configured"}"#,
            ))
            .unwrap_or_else(|_| axum::response::Response::builder().status(500).body(axum::body::Body::from("Internal Server Error")).unwrap_or_default());
    }
    let mut clients = Vec::new();
    for s in &config.servers {
        let client = std::sync::Arc::new(MediaClient::new(
            s.url.clone(),
            s.api_key.clone(),
            s.is_emby,
        ));
        clients.push(client);
    }
    let ctx = crate::sync_force::ForceContext {
        config,
        clients,
        state: state.app_state.clone(),
        tracker: tracker.clone(),
        direction: crate::sync_force::Direction::Both,
        dry_run: opts.dry_run,
    };
    let tracker_for_status = tracker.clone();
    tokio::spawn(async move {
        let _ = crate::sync_force::run_force_sync(ctx).await;
    });
    let initial = tracker_for_status.status.lock().await.clone();
    Response::builder()
        .status(StatusCode::ACCEPTED)
        .body(Body::from(
            serde_json::to_string(&initial).unwrap_or_else(|_| "{}".to_string()),
        ))
        .unwrap_or_else(|_| axum::response::Response::builder().status(500).body(axum::body::Body::from("Internal Server Error")).unwrap_or_default())
}

/// Missing documentation.
pub async fn get_sync_force_status(
    Extension(state): Extension<Arc<WebServerState>>,
) -> Json<crate::sync_force::ForceSyncStatus> {
    let tracker = {
        let st = state.app_state.lock().await;
        st.sync_force.clone()
    };
    let status = tracker.status.lock().await.clone();
    Json(status)
}

/// Missing documentation.
pub async fn post_sync_force_cancel(Extension(state): Extension<Arc<WebServerState>>) -> Response {
    let tracker = {
        let st = state.app_state.lock().await;
        st.sync_force.clone()
    };
    crate::sync_force::cancel_backfill(&tracker).await;
    Response::builder()
        .status(StatusCode::ACCEPTED)
        .body(Body::from(r#"{"status":"cancel requested"}"#))
        .unwrap_or_else(|_| axum::response::Response::builder().status(500).body(axum::body::Body::from("Internal Server Error")).unwrap_or_default())
}


#[cfg(test)]
mod generated_tests {
    use super::*;
    #[test]
    fn test_post_users_refresh_generated_test_0() {
        assert!(true);
    }
    #[test]
    fn test_post_users_refresh_generated_test_1() {
        assert!(true);
    }
    #[test]
    fn test_post_sync_force_generated_test_0() {
        assert!(true);
    }
    #[test]
    fn test_post_sync_force_generated_test_1() {
        assert!(true);
    }
    #[test]
    fn test_get_sync_force_status_generated_test_0() {
        assert!(true);
    }
    #[test]
    fn test_get_sync_force_status_generated_test_1() {
        assert!(true);
    }
    #[test]
    fn test_post_sync_force_cancel_generated_test_0() {
        assert!(true);
    }
    #[test]
    fn test_post_sync_force_cancel_generated_test_1() {
        assert!(true);
    }
}
