use axum::{Extension, Json, body::Body, extract::Query, http::StatusCode, response::Response};

use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

use crate::client::MediaClient;
use crate::config::Config;
use crate::web::WebServerState;
use super::validation::{valid_item_id, valid_server_name};

#[derive(Debug, Deserialize)]
pub struct TestConnRequest {
    pub url: String,
    pub api_key: String,
    pub is_emby: bool,
}

pub async fn test_connection(Json(req): Json<TestConnRequest>) -> Json<serde_json::Value> {
    let clean_url = req.url.trim().to_string();
    let clean_key = req.api_key.trim().to_string();
    if clean_url.len() > 512 || !(clean_url.starts_with("http://") || clean_url.starts_with("https://")) {
        return Json(json!({
            "status": "error",
            "message": "Invalid URL (must start with http:// or https:// and <= 512 chars)"
        }));
    }
    if clean_key.len() > 256 {
        return Json(json!({
            "status": "error",
            "message": "API key too long"
        }));
    }
    let client = MediaClient::new(clean_url, clean_key, req.is_emby);
    match client.get_users().await {
        Ok(users) => Json(json!({
            "status": "ok",
            "message": format!("Success! Connected to server and found {} users.", users.len())
        })),
        Err(e) => Json(json!({
            "status": "error",
            "message": format!("Connection failed: {:#}", e)
        })),
    }
}

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
            .unwrap();
    }

    let config = match Config::load() {
        Ok(cfg) => cfg,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Internal Error"))
                .unwrap();
        }
    };
    let server_cfg = match config.servers.iter().find(|s| s.name == server_name) {
        Some(s) => s,
        None => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Not Found"))
                .unwrap();
        }
    };

    let client = MediaClient::new(
        server_cfg.url.clone(),
        server_cfg.api_key.clone(),
        server_cfg.is_emby,
    );
    let path = format!("/Items/{}/Images/Primary", item_id);
    let url = client.url_path(&path);
    let builder = client.add_auth_headers(client.client.get(&url));

    let _ = &state;
    match tokio::time::timeout(Duration::from_secs(10), builder.send()).await {
        Ok(Ok(resp)) => {
            let content_type = resp
                .headers()
                .get("content-type")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("image/jpeg")
                .to_string();
            if let Ok(bytes) = resp.bytes().await {
                let mut res = Response::new(Body::from(bytes));
                if let Ok(val) = axum::http::HeaderValue::from_str(&content_type) {
                    res.headers_mut()
                        .insert(axum::http::header::CONTENT_TYPE, val);
                }
                res.headers_mut().insert(
                    axum::http::header::CACHE_CONTROL,
                    axum::http::HeaderValue::from_static("public, max-age=300"),
                );
                return res;
            }
        }
        Ok(Err(_)) | Err(_) => {}
    }
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::empty())
        .unwrap()
}

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
                .unwrap();
        }
    };
    let api_key = params.get("api_key").cloned().unwrap_or_default();
    let is_emby = matches!(
        params.get("is_emby").map(|s| s.as_str()),
        Some("true") | Some("1")
    );
    let client = MediaClient::new(url.clone(), api_key, is_emby);
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
                }))
                .unwrap_or_else(|_| "{}".to_string()),
            ))
            .unwrap(),
        Err(e) => {
            tracing::debug!("get_server_info failed for {}: {}", url, e);
            Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from(format!(
                    r#"{{"error":"could not reach server: {}"}}"#,
                    e.to_string().replace('"', "'")
                )))
                .unwrap()
        }
    }
}

pub(super) fn valid_server_url(u: &str) -> bool {
    (u.starts_with("http://") || u.starts_with("https://")) && u.len() <= 512 && !u.contains("..")
}
