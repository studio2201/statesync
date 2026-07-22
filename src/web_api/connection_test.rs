//! Connectivity probe + Emby/Jellyfin detection.

use axum::{Json, http::StatusCode};

use serde::Deserialize;
use serde_json::json;

use super::validation::validate_upstream_url;
use crate::client::MediaClient;
use crate::config::Config;

#[derive(Debug, Deserialize)]
pub struct TestConnRequest {
    pub url: String,
    pub api_key: String,
    pub is_emby: bool,
    /// When set, blank/masked API keys reuse the saved key for that config index.
    #[serde(default)]
    pub server_index: Option<usize>,
}

fn key_is_placeholder(key: &str) -> bool {
    let t = key.trim();
    t.is_empty() || t.contains('•') || t.contains('*')
}

/// Resolve URL + API key, reusing saved credentials when the form left the key blank.
fn resolve_credentials(req: &TestConnRequest) -> Result<(String, String, bool), String> {
    let mut clean_url = crate::config::normalize_server_url(&req.url);
    let mut clean_key = req.api_key.trim().to_string();
    let mut used_saved_key = false;

    if key_is_placeholder(&clean_key) {
        if let Some(idx) = req.server_index {
            if let Ok(cfg) = Config::load() {
                if let Some(saved) = cfg.servers.get(idx) {
                    clean_key = saved.api_key.clone();
                    used_saved_key = true;
                    // If the form URL is empty/broken, fall back to saved URL.
                    if clean_url.is_empty() {
                        clean_url = crate::config::normalize_server_url(&saved.url);
                    }
                }
            }
        }
    }

    if clean_url.is_empty() || clean_url.len() > 512 {
        return Err("Invalid URL (use http://IP:PORT or https://host:PORT)".into());
    }
    if !(clean_url.starts_with("http://") || clean_url.starts_with("https://")) {
        return Err("Invalid URL (must start with http:// or https://)".into());
    }
    if let Err(msg) = validate_upstream_url(&clean_url) {
        return Err(msg.to_string());
    }
    if key_is_placeholder(&clean_key) {
        return Err(
            "API key is required. For a saved server, leave the key blank to re-test the stored key — or paste a new key."
                .into(),
        );
    }
    if clean_key.len() > 256 {
        return Err("API key too long".into());
    }
    Ok((clean_url, clean_key, used_saved_key))
}

pub async fn test_connection(
    Json(req): Json<TestConnRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let (clean_url, clean_key, used_saved_key) = match resolve_credentials(&req) {
        Ok(v) => v,
        Err(message) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "status": "error", "message": message })),
            );
        }
    };

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
                let key_note = if used_saved_key {
                    " (saved API key)"
                } else {
                    ""
                };
                return (
                    StatusCode::OK,
                    Json(json!({
                        "status": "ok",
                        "message": format!(
                            "OK — {clean_url} · {} users{key_note}. Same path Live uses when saved.",
                            users.len()
                        ),
                        "is_emby": is_emby,
                        "url": clean_url,
                        "detected": detected_emby.is_some(),
                        "used_saved_key": used_saved_key,
                    })),
                );
            }
            Err(e) => {
                last_err = format!("{:#}", e);
            }
        }
    }
    let hint = if used_saved_key {
        " Saved key was used (field left blank). If Live still shows green, the process may still hold an older good session — save again or check the key was not rotated."
    } else {
        " Check address, API key, and that this container can reach that host (same network / host access)."
    };
    (
        StatusCode::BAD_GATEWAY,
        Json(json!({
            "status": "error",
            "message": format!(
                "Could not reach {clean_url}.{hint} Detail: {last_err}"
            )
        })),
    )
}
