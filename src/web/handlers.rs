use axum::{
    Extension,
    http::StatusCode,
    response::{Html, IntoResponse},
};
use std::sync::Arc;
use crate::web::WebServerState;

pub async fn serve_index() -> Html<String> {
    Html(crate::dashboard::render_dashboard().into_string())
}

pub async fn serve_manifest() -> impl IntoResponse {
    (
        [("content-type", "application/manifest+json")],
        r##"{"name":"StateSync","short_name":"StateSync","start_url":"/","display":"standalone","background_color":"#03060f","theme_color":"#03060f","icons":[{"src":"/icon.svg","sizes":"192x192","type":"image/svg+xml"},{"src":"/icon.svg","sizes":"512x512","type":"image/svg+xml"}]}"##,
    )
}

pub async fn serve_icon() -> impl IntoResponse {
    (
        [("content-type", "image/svg+xml")],
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect width="100" height="100" fill="#03060f"/><circle cx="50" cy="50" r="30" stroke="#00f0ff" stroke-width="6" fill="none"/></svg>"##,
    )
}

pub async fn serve_favicon() -> impl IntoResponse {
    (
        [
            ("content-type", "image/jpeg"),
            ("cache-control", "public, max-age=86400, immutable"),
        ],
        include_bytes!("../favicon.jpg").as_slice(),
    )
}

pub async fn serve_sw() -> impl IntoResponse {
    (
        [("content-type", "application/javascript")],
        "self.addEventListener('install',(e)=>{self.skipWaiting();});self.addEventListener('fetch',(e)=>{e.respondWith(fetch(e.request));});",
    )
}

pub async fn serve_healthz(Extension(state): Extension<Arc<WebServerState>>) -> impl IntoResponse {
    use crate::web_api::cache_stats;
    let stats = cache_stats(&state.app_state).await;
    let healthy = stats.total_servers > 0
        && (stats.connected_count > 0 || stats.ever_connected_count > 0 || stats.total_users > 0);
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
