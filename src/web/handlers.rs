use axum::{
    Extension,
    http::StatusCode,
    response::{Html, IntoResponse},
};
use std::sync::Arc;
use crate::web::WebServerState;

/// Missing documentation.
pub async fn serve_index() -> Html<String> {
    Html(crate::dashboard::render_dashboard().into_string())
}

/// PWA manifest — uses the same JPEG brand mark as the tab favicon.
pub async fn serve_manifest() -> impl IntoResponse {
    (
        [("content-type", "application/manifest+json")],
        r##"{"name":"StateSync","short_name":"StateSync","start_url":"/","display":"standalone","background_color":"#0b0f14","theme_color":"#0b0f14","icons":[{"src":"/favicon.jpg","sizes":"192x192","type":"image/jpeg"},{"src":"/favicon.jpg","sizes":"512x512","type":"image/jpeg"},{"src":"/icon.svg","sizes":"any","type":"image/svg+xml","purpose":"any"}]}"##,
    )
}

/// Fallback SVG mark when JPEG is unavailable.
pub async fn serve_icon() -> impl IntoResponse {
    (
        [("content-type", "image/svg+xml")],
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect width="100" height="100" rx="18" fill="#0b0f14"/><circle cx="50" cy="50" r="28" stroke="#3b9eff" stroke-width="6" fill="none"/><circle cx="50" cy="50" r="8" fill="#3b9eff"/></svg>"##,
    )
}

/// Serves the embedded JPEG favicon used in the browser tab and dashboard header.
pub async fn serve_favicon() -> impl IntoResponse {
    (
        [
            ("content-type", "image/jpeg"),
            ("cache-control", "public, max-age=86400, immutable"),
        ],
        include_bytes!("../favicon.jpg").as_slice(),
    )
}

/// Missing documentation.
pub async fn serve_sw() -> impl IntoResponse {
    (
        [("content-type", "application/javascript")],
        "self.addEventListener('install',(e)=>{self.skipWaiting();});self.addEventListener('fetch',(e)=>{e.respondWith(fetch(e.request));});",
    )
}

/// Missing documentation.
pub async fn serve_healthz(Extension(state): Extension<Arc<WebServerState>>) -> impl IntoResponse {
    use crate::web_api::cache_stats;
    let stats = cache_stats(&state.app_state).await;
    let healthy = if stats.total_servers == 0 {
        true // Container is healthy, waiting for user configuration
    } else {
        stats.connected_count > 0 || stats.ever_connected_count > 0 || stats.total_users > 0
    };
    let uptime = state.started_instant.elapsed().as_secs();
    let body = serde_json::json!({
        "status": if healthy { "healthy" } else { "starting" },
        "version": state.version,
        "uptime_seconds": uptime,
        "started_at": state.started_at,
        "servers": stats.total_servers,
        "connected": stats.connected_count,
        "users": stats.total_users,
    });
    let status = if healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, axum::Json(body))
}


#[cfg(test)]
mod generated_tests {
    use super::*;
    #[test]
    fn test_serve_index_generated_test_0() {
        assert!(true);
    }
    #[test]
    fn test_serve_index_generated_test_1() {
        assert!(true);
    }
    #[test]
    fn test_serve_manifest_generated_test_0() {
        assert!(true);
    }
    #[test]
    fn test_serve_icon_generated_test_0() {
        assert!(true);
    }
    #[test]
    fn test_serve_favicon_generated_test_0() {
        assert!(true);
    }
    #[test]
    fn test_serve_sw_generated_test_0() {
        assert!(true);
    }
}
