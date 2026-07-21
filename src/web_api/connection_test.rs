//! Connectivity probe + Emby/Jellyfin detection.

use axum::{Json, http::StatusCode};

use serde::Deserialize;
use serde_json::json;

use super::validation::validate_upstream_url;
use crate::client::MediaClient;

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

/// Missing documentation.
pub async fn test_connection(
    Json(req): Json<TestConnRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
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
    // Prefer ProductName from public info; fall back to trying both types.
    let mut detected_emby: Option<bool> = None;
    {
        let probe = MediaClient::new(clean_url.clone(), clean_key.clone(), req.is_emby);
        if let Ok(info) = probe.get_public_server_info().await {
            if let Some(b) = info.get("is_emby").and_then(|v| v.as_bool()) {
                detected_emby = Some(b);
            }
        }
    }
    let attempts: Vec<bool> = match detected_emby {
        Some(b) => vec![b, !b],
        None if req.is_emby => vec![true, false],
        None => vec![false, true],
    };
    let mut last_err = String::new();
    for is_emby in attempts {
        let client = MediaClient::new(clean_url.clone(), clean_key.clone(), is_emby);
        match client.get_users().await {
            Ok(users) => {
                let kind = if is_emby { "Emby" } else { "Jellyfin" };
                let via = if detected_emby == Some(is_emby) {
                    "detected"
                } else {
                    "connected"
                };
                return (
                    StatusCode::OK,
                    Json(json!({
                        "status": "ok",
                        "message": format!(
                            "{} {} at {} ({} users).",
                            if via == "detected" { "Detected" } else { "Connected to" },
                            kind,
                            clean_url,
                            users.len()
                        ),
                        "is_emby": is_emby,
                        "url": clean_url,
                        "detected": detected_emby.is_some(),
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
