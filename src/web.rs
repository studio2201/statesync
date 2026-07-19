use axum::{Extension, Router, response::Html, routing::get};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

use crate::state::AppState;

pub struct WebServerState {
    pub app_state: Arc<Mutex<AppState>>,
    pub reload_tx: mpsc::Sender<()>,
}

pub fn create_router(web_state: Arc<WebServerState>) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/manifest.json", get(serve_manifest))
        .route("/sw.js", get(serve_sw))
        .route("/icon.svg", get(serve_icon))
        .route("/favicon.jpg", get(serve_favicon))
        .route(
            "/api/config",
            get(crate::web_api::get_config).post(crate::web_api::post_config),
        )
        .route("/api/status", get(crate::web_api::get_status))
        .route("/api/poster", get(crate::web_api::serve_poster))
        .route(
            "/api/test_connection",
            get(crate::web_api::get_config).post(crate::web_api::test_connection),
        )
        .route(
            "/api/reload",
            axum::routing::post(crate::web_api::post_reload),
        )
        .layer(Extension(web_state))
}

async fn serve_index() -> Html<String> {
    Html(crate::dashboard::render_dashboard().into_string())
}

async fn serve_manifest() -> impl axum::response::IntoResponse {
    (
        [("content-type", "application/manifest+json")],
        r##"{"name":"StateSync","short_name":"StateSync","start_url":"/","display":"standalone","background_color":"#03060f","theme_color":"#03060f","icons":[{"src":"/icon.svg","sizes":"192x192","type":"image/svg+xml"},{"src":"/icon.svg","sizes":"512x512","type":"image/svg+xml"}]}"##,
    )
}

async fn serve_icon() -> impl axum::response::IntoResponse {
    (
        [("content-type", "image/svg+xml")],
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect width="100" height="100" fill="#03060f"/><circle cx="50" cy="50" r="30" stroke="#00f0ff" stroke-width="6" fill="none"/></svg>"##,
    )
}

async fn serve_favicon() -> impl axum::response::IntoResponse {
    (
        [("content-type", "image/jpeg")],
        include_bytes!("favicon.jpg").as_slice(),
    )
}

async fn serve_sw() -> impl axum::response::IntoResponse {
    (
        [("content-type", "application/javascript")],
        "self.addEventListener('install',(e)=>{self.skipWaiting();});self.addEventListener('fetch',(e)=>{e.respondWith(fetch(e.request));});",
    )
}
