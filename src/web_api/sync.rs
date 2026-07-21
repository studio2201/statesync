use axum::{Extension, body::Body, http::StatusCode, response::Response};
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
            .unwrap_or_else(|_| {
                axum::response::Response::builder()
                    .status(500)
                    .body(axum::body::Body::from("Internal Server Error"))
                    .unwrap_or_default()
            });
    }
    Response::builder()
        .status(StatusCode::OK)
        .body(Body::from("Reload triggered successfully"))
        .unwrap_or_else(|_| {
            axum::response::Response::builder()
                .status(500)
                .body(axum::body::Body::from("Internal Server Error"))
                .unwrap_or_default()
        })
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
                .unwrap_or_else(|_| {
                    axum::response::Response::builder()
                        .status(500)
                        .body(axum::body::Body::from("Internal Server Error"))
                        .unwrap_or_default()
                });
        }
    };
    if config.servers.is_empty() {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from(
                r#"{"status":"error","message":"no servers configured"}"#,
            ))
            .unwrap_or_else(|_| {
                axum::response::Response::builder()
                    .status(500)
                    .body(axum::body::Body::from("Internal Server Error"))
                    .unwrap_or_default()
            });
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
        .unwrap_or_else(|_| {
            axum::response::Response::builder()
                .status(500)
                .body(axum::body::Body::from("Internal Server Error"))
                .unwrap_or_default()
        })
}
