use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use statesync::state::AppState;
use statesync::web::{WebServerState, create_router};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, mpsc};
use tower::util::ServiceExt;

fn make_test_web_state() -> Arc<WebServerState> {
    let app_state = Arc::new(Mutex::new(AppState::new(vec![])));
    let (reload_tx, _) = mpsc::channel(1);
    Arc::new(WebServerState {
        app_state,
        reload_tx,
        bind_addr: "127.0.0.1:0".to_string(),
        web_auth: None,
        version: "v0.28.34".to_string(),
        started_at: "2026-07-20T00:00:00Z".to_string(),
        started_instant: Instant::now(),
    })
}

// 1. End-to-End Test: Serve Index Dashboard HTML
#[tokio::test]
async fn test_e2e_serve_dashboard_html() {
    let web_state = make_test_web_state();
    let app = create_router(web_state);

    let req = Request::builder()
        .method("GET")
        .uri("/")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body_bytes.to_vec()).unwrap();

    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("StateSync"));
}

// 2. End-to-End Test: Web API Server Info Query
#[tokio::test]
async fn test_e2e_api_server_info_query() {
    let mut server = mockito::Server::new_async().await;
    let mock_public = server.mock("GET", "/System/Info/Public")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"ServerName": "Test Emby Server", "Version": "4.8.8.0", "ProductName": "Emby Server"}"#)
        .create_async().await;

    let web_state = make_test_web_state();
    let app = create_router(web_state);

    let encoded_url = utf8_percent_encode(&server.url(), NON_ALPHANUMERIC).to_string();
    let uri = format!("/api/server-info?url={}&is_emby=true", encoded_url);
    let req = Request::builder()
        .method("GET")
        .uri(&uri)
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json_res: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json_res["name"], "Test Emby Server");
    assert_eq!(json_res["version"], "4.8.8.0");
    assert_eq!(json_res["is_emby"], true);
    mock_public.assert_async().await;
}

// 3. End-to-End Test: Test Connection Ping Endpoint for Emby & Jellyfin
#[tokio::test]
async fn test_e2e_api_test_connection_emby_and_jellyfin() {
    let mut server = mockito::Server::new_async().await;

    // Mock Emby /Users response
    let mock_users = server
        .mock("GET", "/Users?StartIndex=0&Limit=500")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"Items": [{"Name": "alice", "Id": "u1"}], "TotalRecordCount": 1}"#)
        .create_async()
        .await;

    let web_state = make_test_web_state();
    let app = create_router(web_state);

    let req_payload = serde_json::json!({
        "url": server.url(),
        "api_key": "test_api_key",
        "is_emby": true
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/test_connection")
        .header("content-type", "application/json")
        .body(Body::from(req_payload.to_string()))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json_res: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json_res["status"], "ok");
    let msg = json_res["message"].as_str().unwrap();
    assert!(
        msg.contains("Connected") && msg.contains("1 users"),
        "unexpected message: {}",
        msg
    );
    mock_users.assert_async().await;
}
