//! Public media-server info endpoint.

use super::validation::{valid_server_url, validate_upstream_url};
use crate::client::MediaClient;
use crate::web::WebServerState;
use axum::{Extension, Json, body::Body, extract::Query, http::StatusCode, response::Response};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
/// JSON body for `/api/server-info` (avoids putting API keys in query strings).
pub struct ServerInfoRequest {
    pub url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub is_emby: bool,
}

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
                .unwrap_or_else(|_| {
                    Response::builder()
                        .status(500)
                        .body(Body::from("Internal Server Error"))
                        .unwrap_or_default()
                });
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
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(500)
                    .body(Body::from("Internal Server Error"))
                    .unwrap_or_default()
            });
    }
    if let Err(msg) = validate_upstream_url(&url) {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from(json!({ "error": msg }).to_string()))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(500)
                    .body(Body::from("Internal Server Error"))
                    .unwrap_or_default()
            });
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
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(500)
                    .body(Body::from("Internal Server Error"))
                    .unwrap_or_default()
            }),
        Err(e) => {
            tracing::debug!("get_server_info failed for {}: {}", url, e);
            Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from(
                    json!({ "error": format!("could not reach server: {}", e) }).to_string(),
                ))
                .unwrap_or_else(|_| {
                    Response::builder()
                        .status(500)
                        .body(Body::from("Internal Server Error"))
                        .unwrap_or_default()
                })
        }
    }
}
