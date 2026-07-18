use std::sync::Arc;
use axum::{
    routing::get,
    Json, Router, response::Html, Extension,
};
use tokio::sync::{mpsc, Mutex};
use serde_json::json;
use serde::Deserialize;

use crate::config::Config;
use crate::state::AppState;
use crate::client::MediaClient;

pub struct WebServerState {
    pub app_state: Arc<Mutex<AppState>>,
    pub reload_tx: mpsc::Sender<()>,
}

#[derive(Debug, Deserialize)]
struct TestConnRequest {
    url: String,
    api_key: String,
    is_emby: bool,
}

pub fn create_router(web_state: Arc<WebServerState>) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/manifest.json", get(serve_manifest))
        .route("/sw.js", get(serve_sw))
        .route("/api/config", get(get_config).post(post_config))
        .route("/api/status", get(get_status))
        .route("/api/test_connection", get(get_config).post(test_connection)) // bind get just to support router routing check
        .layer(Extension(web_state))
}

async fn serve_index() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

async fn serve_manifest() -> impl axum::response::IntoResponse {
    let manifest = r##"{
  "name": "StateSync",
  "short_name": "StateSync",
  "start_url": "/",
  "display": "standalone",
  "background_color": "#03060f",
  "theme_color": "#03060f",
  "icons": [
    {
      "src": "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'><rect width='100' height='100' fill='%2303060f'/><circle cx='50' cy='50' r='30' stroke='%2300f0ff' stroke-width='6' fill='none'/></svg>",
      "sizes": "192x192",
      "type": "image/svg+xml"
    },
    {
      "src": "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'><rect width='100' height='100' fill='%2303060f'/><circle cx='50' cy='50' r='30' stroke='%2300f0ff' stroke-width='6' fill='none'/></svg>",
      "sizes": "512x512",
      "type": "image/svg+xml"
    }
  ]
}"##;
    ([("content-type", "application/json")], manifest)
}

async fn serve_sw() -> impl axum::response::IntoResponse {
    let sw = r#"self.addEventListener('install', (e) => { self.skipWaiting(); });
self.addEventListener('fetch', (e) => { e.respondWith(fetch(e.request)); });"#;
    ([("content-type", "application/javascript")], sw)
}

fn mask_api_key(key: &str) -> String {
    if key.len() <= 8 {
        "••••••••".to_string()
    } else {
        format!("{}••••••••{}", &key[..4], &key[key.len() - 4..])
    }
}

async fn get_config() -> Json<Config> {
    let mut config = Config::load().unwrap_or(Config { servers: vec![], sync_threshold_seconds: 5 });
    for s in &mut config.servers {
        s.api_key = mask_api_key(&s.api_key);
    }
    Json(config)
}

async fn post_config(
    Extension(state): Extension<Arc<WebServerState>>,
    Json(mut new_config): Json<Config>,
) -> Json<serde_json::Value> {
    // If the new config contains masked keys, merge them with the existing keys on disk
    if let Ok(old_config) = Config::load() {
        for s in &mut new_config.servers {
            if s.api_key.contains('•') || s.api_key.trim().is_empty() {
                if let Some(old_s) = old_config.servers.iter().find(|os| os.url == s.url) {
                    s.api_key = old_s.api_key.clone();
                }
            }
        }
    }

    let path = crate::config::get_config_path();
    let serialized = serde_json::to_string_pretty(&new_config).unwrap_or_default();
    if let Err(e) = std::fs::write(path, serialized) {
        return Json(json!({ "status": "error", "message": format!("Failed to save config: {}", e) }));
    }

    let _ = state.reload_tx.send(()).await;
    Json(json!({ "status": "ok", "message": "Configuration saved. Sync service is reloading..." }))
}

async fn test_connection(Json(req): Json<TestConnRequest>) -> Json<serde_json::Value> {
    let client = MediaClient::new(req.url, req.api_key, req.is_emby);
    match client.get_users().await {
        Ok(users) => Json(json!({
            "status": "ok",
            "message": format!("Success! Connected to server and found {} users.", users.len())
        })),
        Err(e) => Json(json!({
            "status": "error",
            "message": format!("Connection failed: {}", e)
        }))
    }
}

async fn get_status(Extension(state): Extension<Arc<WebServerState>>) -> Json<serde_json::Value> {
    let app_state = state.app_state.lock().await;
    
    let mut servers_status = Vec::new();
    for (i, cache) in app_state.caches.iter().enumerate() {
        let ws_status = app_state.websocket_statuses.get(i).cloned().unwrap_or_else(|| "Offline".to_string());
        servers_status.push(json!({
            "name": cache.name,
            "users_count": cache.users.len(),
            "media_count": cache.id_to_providers.len(),
            "websocket_status": ws_status
        }));
    }
    
    let mut synced_users = Vec::new();
    if let Some(first_cache) = app_state.caches.first() {
        for username in first_cache.users.keys() {
            if app_state.caches.iter().skip(1).all(|c| crate::state::find_mapped_user_id(username, &c.users).is_some()) {
                synced_users.push(username.clone());
            }
        }
    }
    
    let mut active_sessions = Vec::new();
    for ((server, _), (user, item, position, is_paused)) in &app_state.active_sessions {
        active_sessions.push(json!({
            "server": server,
            "user": user,
            "item": item,
            "position": position,
            "is_paused": is_paused
        }));
    }
    
    Json(json!({
        "status": "active",
        "servers": servers_status,
        "synced_users": synced_users,
        "active_sessions": active_sessions,
        "sync_logs": app_state.sync_logs
    }))
}
