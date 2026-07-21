//! Now-playing poster proxy.

use super::validation::{valid_item_id, valid_server_name};
use crate::client::MediaClient;
use crate::config::Config;
use crate::web::WebServerState;
use axum::{Extension, body::Body, extract::Query, http::StatusCode, response::Response};
use std::sync::Arc;
use std::time::Duration;

/// Missing documentation.
pub async fn serve_poster(
    Extension(state): Extension<Arc<WebServerState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let server_name = params.get("server").cloned().unwrap_or_default();
    let item_id = params.get("item_id").cloned().unwrap_or_default();

    if !valid_server_name(&server_name) || !valid_item_id(&item_id) {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("Bad Request"))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(500)
                    .body(Body::from("Internal Server Error"))
                    .unwrap_or_default()
            });
    }

    let config = match Config::load() {
        Ok(cfg) => cfg,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Internal Error"))
                .unwrap_or_else(|_| {
                    Response::builder()
                        .status(500)
                        .body(Body::from("Internal Server Error"))
                        .unwrap_or_default()
                });
        }
    };
    let server_cfg = match config.servers.iter().find(|s| s.name == server_name) {
        Some(s) => s,
        None => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Not Found"))
                .unwrap_or_else(|_| {
                    Response::builder()
                        .status(500)
                        .body(Body::from("Internal Server Error"))
                        .unwrap_or_default()
                });
        }
    };

    let client = MediaClient::new(
        server_cfg.url.clone(),
        server_cfg.api_key.clone(),
        server_cfg.is_emby,
    );
    // MaxWidth keeps thumbnails small; Emby/Jellyfin return Primary art for the item.
    let path = format!("/Items/{}/Images/Primary?maxWidth=120&quality=80", item_id);
    let url = client.url_path(&path);
    let builder = client.add_auth_headers(client.client.get(&url));

    let _ = &state;
    match tokio::time::timeout(Duration::from_secs(10), builder.send()).await {
        Ok(Ok(resp)) => {
            if !resp.status().is_success() {
                return Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::from("No poster"))
                    .unwrap_or_else(|_| {
                        Response::builder()
                            .status(500)
                            .body(Body::from("Internal Server Error"))
                            .unwrap_or_default()
                    });
            }
            let content_type = resp
                .headers()
                .get("content-type")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("image/jpeg")
                .to_string();
            // Only proxy real image payloads (avoid HTML error pages as "images").
            let is_image = content_type.starts_with("image/");
            if let Ok(bytes) = resp.bytes().await {
                if is_image && !bytes.is_empty() {
                    let mut res = Response::new(Body::from(bytes));
                    if let Ok(val) = axum::http::HeaderValue::from_str(&content_type) {
                        res.headers_mut()
                            .insert(axum::http::header::CONTENT_TYPE, val);
                    }
                    res.headers_mut().insert(
                        axum::http::header::CACHE_CONTROL,
                        axum::http::HeaderValue::from_static("private, max-age=300"),
                    );
                    return res;
                }
            }
        }
        Ok(Err(_)) | Err(_) => {}
    }
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .unwrap_or_else(|_| {
            Response::builder()
                .status(500)
                .body(Body::from("Internal Server Error"))
                .unwrap_or_default()
        })
}
