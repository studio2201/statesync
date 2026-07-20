use axum::{Extension, Json};
use serde_json::json;
use std::sync::Arc;

use crate::config::{Config, validate_config};
use crate::web::WebServerState;

pub fn mask_api_key(key: &str) -> String {
    if key.is_empty() {
        "".to_string()
    } else if key.len() <= 8 {
        "••••••••".to_string()
    } else {
        format!("{}••••••••{}", &key[..4], &key[key.len() - 4..])
    }
}

pub async fn get_config() -> Json<Config> {
    let mut config = Config::load().unwrap_or_else(|_| crate::config::default_config());
    for s in &mut config.servers {
        s.api_key = mask_api_key(&s.api_key);
    }
    Json(config)
}

pub async fn post_config(
    Extension(state): Extension<Arc<WebServerState>>,
    Json(mut new_config): Json<Config>,
) -> Json<serde_json::Value> {
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

    if let Err(_e) = validate_config(&new_config) {
        return Json(json!({ "status": "error", "message": "Invalid configuration" }));
    }

    if let Err(e) = new_config.save() {
        tracing::error!("post_config: failed to save configuration: {}", e);
        return Json(json!({ "status": "error", "message": "Failed to write configuration file" }));
    }

    let _ = state.reload_tx.send(()).await;
    Json(json!({ "status": "ok", "message": "Configuration saved. Sync service is reloading..." }))
}
