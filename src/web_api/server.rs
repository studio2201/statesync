use axum::{Extension, Json, body::Body, extract::Query, http::StatusCode, response::Response};

use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

use crate::client::MediaClient;
use crate::config::Config;
use crate::web::WebServerState;
use super::validation::{
    valid_item_id, valid_server_name, valid_server_url, validate_upstream_url,
};

#[derive(Debug, Deserialize)]
/// Missing documentation.
pub struct TestConnRequest {
    /// Missing documentation.
    pub url: String,
    /// Missing documentation.
    pub api_key: String,
    /// Missing documentation.
    pub is_emby: bool,
}

#[derive(Debug, Deserialize)]
/// JSON body for `/api/server-info` (avoids putting API keys in query strings).
pub struct ServerInfoRequest {
    pub url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub is_emby: bool,
}

/// Missing documentation.
pub async fn test_connection(Json(req): Json<TestConnRequest>) -> (StatusCode, Json<serde_json::Value>) {
    let clean_url = crate::config::normalize_server_url(&req.url);
    let clean_key = req.api_key.trim().to_string();
    if clean_url.is_empty() || clean_url.len() > 512 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "status": "error",
                "message": "Invalid URL (use http://IP:PORT or https://host:PORT)"
            })),
        );
    }
    if !(clean_url.starts_with("http://") || clean_url.starts_with("https://")) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "status": "error",
                "message": "Invalid URL (must start with http:// or https://)"
            })),
        );
    }
    if let Err(msg) = validate_upstream_url(&clean_url) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "status": "error", "message": msg })),
        );
    }
    if clean_key.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "status": "error",
                "message": "API key is required"
            })),
        );
    }
    if clean_key.len() > 256 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "status": "error",
                "message": "API key too long"
            })),
        );
    }
    // Try both type guesses so a wrong Emby/Jellyfin toggle still succeeds.
    let attempts = if req.is_emby {
        [true, false]
    } else {
        [false, true]
    };
    let mut last_err = String::new();
    for is_emby in attempts {
        let client = MediaClient::new(clean_url.clone(), clean_key.clone(), is_emby);
        match client.get_users().await {
            Ok(users) => {
                let kind = if is_emby { "Emby" } else { "Jellyfin" };
                return (
                    StatusCode::OK,
                    Json(json!({
                        "status": "ok",
                        "message": format!(
                            "Connected to {} at {} ({} users).",
                            kind,
                            clean_url,
                            users.len()
                        ),
                        "is_emby": is_emby,
                        "url": clean_url,
                    })),
                );
            }
            Err(e) => {
                last_err = format!("{:#}", e);
            }
        }
    }
    (
        StatusCode::BAD_GATEWAY,
        Json(json!({
            "status": "error",
            "message": format!(
                "Could not reach {} — check IP/port, API key, and that StateSync can route to that host. Detail: {}",
                clean_url,
                last_err
            )
        })),
    )
}

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
            .unwrap_or_else(|_| Response::builder().status(500).body(Body::from("Internal Server Error")).unwrap_or_default());
    }

    let config = match Config::load() {
        Ok(cfg) => cfg,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Internal Error"))
                .unwrap_or_else(|_| Response::builder().status(500).body(Body::from("Internal Server Error")).unwrap_or_default());
        }
    };
    let server_cfg = match config.servers.iter().find(|s| s.name == server_name) {
        Some(s) => s,
        None => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Not Found"))
                .unwrap_or_else(|_| Response::builder().status(500).body(Body::from("Internal Server Error")).unwrap_or_default());
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
        .unwrap_or_else(|_| Response::builder().status(500).body(Body::from("Internal Server Error")).unwrap_or_default())
}

/// POST `/api/server-info` with JSON body (preferred; keeps API keys out of URLs).
pub async fn post_server_info(
    _state: Extension<Arc<WebServerState>>,
    Json(req): Json<ServerInfoRequest>,
) -> Response {
    fetch_server_info(&req.url, &req.api_key, req.is_emby).await
}

/// GET `/api/server-info?url=...&is_emby=...` without API key (public info only).
pub async fn get_server_info(
    _state: Extension<Arc<WebServerState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let url = match params.get("url") {
        Some(u) if valid_server_url(u) => u.clone(),
        _ => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(r#"{"error":"missing or invalid 'url'"}"#))
                .unwrap_or_else(|_| Response::builder().status(500).body(Body::from("Internal Server Error")).unwrap_or_default());
        }
    };
    // Do not accept api_key via query string (logs/proxies/history risk).
    let is_emby = matches!(
        params.get("is_emby").map(|s| s.as_str()),
        Some("true") | Some("1")
    );
    fetch_server_info(&url, "", is_emby).await
}

async fn fetch_server_info(url: &str, api_key: &str, is_emby: bool) -> Response {
    let url = crate::config::normalize_server_url(url);
    if !valid_server_url(&url) {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from(r#"{"error":"missing or invalid 'url'"}"#))
            .unwrap_or_else(|_| Response::builder().status(500).body(Body::from("Internal Server Error")).unwrap_or_default());
    }
    if let Err(msg) = validate_upstream_url(&url) {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from(format!(r#"{{"error":"{}"}}"#, msg.replace('"', "'"))))
            .unwrap_or_else(|_| Response::builder().status(500).body(Body::from("Internal Server Error")).unwrap_or_default());
    }
    let client = MediaClient::new(url.clone(), api_key.to_string(), is_emby);
    match client.get_public_server_info().await {
        Ok(info) => Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(
                serde_json::to_string(&json!({
                    "name": info.get("ServerName")
                        .or_else(|| info.get("Name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                    "version": info.get("Version").and_then(|v| v.as_str()).unwrap_or(""),
                    "id": info.get("Id").and_then(|v| v.as_str()).unwrap_or(""),
                    "is_emby": info.get("is_emby").and_then(|v| v.as_bool()).unwrap_or(false),
                }))
                .unwrap_or_else(|_| "{}".to_string()),
            ))
            .unwrap_or_else(|_| Response::builder().status(500).body(Body::from("Internal Server Error")).unwrap_or_default()),
        Err(e) => {
            tracing::debug!("get_server_info failed for {}: {}", url, e);
            Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from(format!(
                    r#"{{"error":"could not reach server: {}"}}"#,
                    e.to_string().replace('"', "'")
                )))
                .unwrap_or_else(|_| Response::builder().status(500).body(Body::from("Internal Server Error")).unwrap_or_default())
        }
    }
}


