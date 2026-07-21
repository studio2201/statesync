use axum::{Extension, Json, http::StatusCode};
use serde_json::json;
pub use shared_core::mask_api_key;
use std::sync::Arc;

use crate::config::{Config, normalize_config, validate_config};
use crate::web::WebServerState;

/// Missing documentation.
pub async fn get_config() -> Json<Config> {
    let mut config = Config::load().unwrap_or_else(|_| crate::config::default_config());
    for s in &mut config.servers {
        s.api_key = mask_api_key(&s.api_key);
    }
    Json(config)
}

/// Missing documentation.
pub async fn post_config(
    Extension(state): Extension<Arc<WebServerState>>,
    Json(mut new_config): Json<Config>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Ok(old_config) = Config::load() {
        new_config.last_full_sync = old_config.last_full_sync.clone();
        for s in &mut new_config.servers {
            if s.api_key.contains('•') || s.api_key.trim().is_empty() {
                if let Some(old_s) = old_config
                    .servers
                    .iter()
                    .find(|os| os.url == s.url || os.name == s.name)
                {
                    s.api_key = old_s.api_key.clone();
                }
            }
        }
    }

    // Fill empty names from URL host; normalize bare IPs to http://…
    normalize_config(&mut new_config);
    // Web UI always allows LAN http:// when the user typed an http URL.
    for s in &mut new_config.servers {
        if s.url.starts_with("http://") {
            s.allow_insecure_http = true;
        }
    }

    if let Err(e) = validate_config(&new_config) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "status": "error", "message": format!("Invalid configuration: {}", e) })),
        );
    }

    if let Err(e) = new_config.save() {
        tracing::error!("post_config: failed to save configuration: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "status": "error", "message": "Failed to write configuration file" })),
        );
    }

    let _ = state.reload_tx.send(()).await;
    (
        StatusCode::OK,
        Json(
            json!({ "status": "ok", "message": "Configuration saved. Sync service is reloading..." }),
        ),
    )
}
