use axum::{
    Extension, Router,
    extract::Request,
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, mpsc};

use crate::state::AppState;

pub mod handlers;

#[cfg(test)]
mod tests;

#[derive(Clone)]
pub struct WebServerState {
    pub app_state: Arc<Mutex<AppState>>,
    pub reload_tx: mpsc::Sender<()>,
    #[allow(dead_code)]
    pub bind_addr: String,
    pub web_auth: Option<String>,
    pub version: String,
    pub started_at: String,
    pub started_instant: Instant,
}

const PUBLIC_PATHS: &[&str] = &[
    "/",
    "/manifest.json",
    "/sw.js",
    "/icon.svg",
    "/favicon.jpg",
    "/healthz",
];

pub fn create_router(web_state: Arc<WebServerState>) -> Router {
    let public = Router::new()
        .route("/", get(handlers::serve_index))
        .route("/manifest.json", get(handlers::serve_manifest))
        .route("/sw.js", get(handlers::serve_sw))
        .route("/icon.svg", get(handlers::serve_icon))
        .route("/favicon.jpg", get(handlers::serve_favicon))
        .route("/healthz", get(handlers::serve_healthz));

    let protected = Router::new()
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
        .route(
            "/api/users/refresh",
            axum::routing::post(crate::web_api::post_users_refresh),
        )
        .route(
            "/api/sync/force",
            axum::routing::post(crate::web_api::post_sync_force),
        )
        .route(
            "/api/sync/force/status",
            get(crate::web_api::get_sync_force_status),
        )
        .route(
            "/api/sync/force/cancel",
            axum::routing::post(crate::web_api::post_sync_force_cancel),
        )
        .route("/api/server-info", get(crate::web_api::get_server_info));

    public
        .merge(protected)
        .layer(middleware::from_fn(security_headers))
        .layer(middleware::from_fn_with_state(
            web_state.clone(),
            auth_middleware,
        ))
        .layer(Extension(web_state))
}

async fn security_headers(req: Request, next: Next) -> Response {
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    h.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    h.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("interest-cohort=()"),
    );
    if let Ok(csp) = HeaderValue::from_str(
        "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src https://fonts.gstatic.com; script-src 'self' 'unsafe-inline'; connect-src 'self'",
    ) {
        h.insert(HeaderName::from_static("content-security-policy"), csp);
    }
    resp
}

async fn auth_middleware(
    Extension(state): Extension<Arc<WebServerState>>,
    req: Request,
    next: Next,
) -> Response {
    let token = state.web_auth.as_deref();
    let needs_auth = match token {
        None => false,
        Some(spec) => match spec.strip_prefix("bearer:") {
            Some(_) => true,
            None => {
                tracing::error!(
                    "STATESYNC_WEB_AUTH must start with 'bearer:' (got unsupported scheme); all protected endpoints will reject"
                );
                true
            }
        },
    };

    if !needs_auth {
        return next.run(req).await;
    }

    let path = req.uri().path().to_string();
    if PUBLIC_PATHS.contains(&path.as_str()) {
        return next.run(req).await;
    }

    let expected = match token.unwrap().strip_prefix("bearer:") {
        Some(t) => t,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                r#"{"error":"server misconfigured"}"#,
            )
                .into_response();
        }
    };
    if !constant_time_eq(&extract_bearer(req.headers()), expected) {
        return (
            StatusCode::UNAUTHORIZED,
            [("www-authenticate", "Bearer")],
            r#"{"error":"unauthorized"}"#,
        )
            .into_response();
    }

    next.run(req).await
}

pub fn extract_bearer(headers: &HeaderMap) -> String {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
        })
        .unwrap_or("")
        .to_string()
}

pub fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut acc = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        acc |= x ^ y;
    }
    acc == 0
}
