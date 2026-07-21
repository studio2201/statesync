use super::*;
use axum::http::HeaderMap;

#[test]
fn test_constant_time_eq() {
    assert!(constant_time_eq("hello", "hello"));
    assert!(!constant_time_eq("hello", "world"));
    assert!(!constant_time_eq("hello", "helloo"));
}

#[test]
fn test_extract_bearer() {
    let mut headers = HeaderMap::new();
    headers.insert(axum::http::header::AUTHORIZATION, "Bearer mytoken".parse().unwrap());
    assert_eq!(extract_bearer(&headers), "mytoken");

    let mut headers_lower = HeaderMap::new();
    headers_lower.insert(axum::http::header::AUTHORIZATION, "bearer mytoken2".parse().unwrap());
    assert_eq!(extract_bearer(&headers_lower), "mytoken2");

    let headers_empty = HeaderMap::new();
    assert_eq!(extract_bearer(&headers_empty), "");
}

#[tokio::test]
async fn test_serve_manifest() {
    let resp = handlers::serve_manifest().await.into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    assert_eq!(resp.headers().get("content-type").unwrap().to_str().unwrap(), "application/manifest+json");
}

#[tokio::test]
async fn test_serve_icon() {
    let resp = handlers::serve_icon().await.into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    assert_eq!(resp.headers().get("content-type").unwrap().to_str().unwrap(), "image/svg+xml");
}

#[tokio::test]
async fn test_serve_favicon() {
    let resp = handlers::serve_favicon().await.into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    assert_eq!(resp.headers().get("content-type").unwrap().to_str().unwrap(), "image/jpeg");
    let bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .expect("favicon body");
    assert!(
        bytes.len() > 1000,
        "favicon must embed real JPEG bytes, got {} bytes",
        bytes.len()
    );
    // JPEG magic number
    assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
}

#[tokio::test]
async fn test_serve_sw() {
    let resp = handlers::serve_sw().await.into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    assert_eq!(resp.headers().get("content-type").unwrap().to_str().unwrap(), "application/javascript");
}

#[tokio::test]
async fn test_serve_healthz_unhealthy() {
    let cache = crate::state::ServerCache {
        name: "test".to_string(),
        users: std::collections::HashMap::new(),
        imdb_to_id: std::collections::HashMap::new(),
        tmdb_to_id: std::collections::HashMap::new(),
        id_to_providers: std::collections::HashMap::new(),
    };
    let app_state = Arc::new(Mutex::new(AppState::new(vec![cache])));
    // Leave websocket_statuses empty or "Error" so it is disconnected
    let web_state = Arc::new(WebServerState {
        app_state,
        reload_tx: mpsc::channel(1).0,
        bind_addr: "127.0.0.1:0".to_string(),
        web_auth: None,
        version: "test".to_string(),
        started_at: "2025-01-01".to_string(),
        started_instant: Instant::now(),
    });
    
    let resp = handlers::serve_healthz(Extension(web_state)).await.into_response();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_serve_healthz_healthy() {
    let cache = crate::state::ServerCache {
        name: "test".to_string(),
        users: [("alice".to_string(), "u1".to_string())].into_iter().collect(),
        imdb_to_id: std::collections::HashMap::new(),
        tmdb_to_id: std::collections::HashMap::new(),
        id_to_providers: std::collections::HashMap::new(),
    };
    let app_state = Arc::new(Mutex::new(AppState::new(vec![cache])));
    // simulate connected status to trigger healthy
    {
        let mut st = app_state.lock().await;
        st.websocket_statuses = vec!["Connected".to_string()];
    }
    let web_state = Arc::new(WebServerState {
        app_state,
        reload_tx: mpsc::channel(1).0,
        bind_addr: "127.0.0.1:0".to_string(),
        web_auth: None,
        version: "test".to_string(),
        started_at: "2025-01-01".to_string(),
        started_instant: Instant::now(),
    });
    
    let resp = handlers::serve_healthz(Extension(web_state)).await.into_response();
    assert_eq!(resp.status(), StatusCode::OK);
}
