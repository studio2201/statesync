use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use statesync::dashboard::{render_dashboard, render_full_js};
use statesync::web::{create_router, WebServerState};
use statesync::state::AppState;
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
        version: "v0.28.33".to_string(),
        started_at: "2026-07-20T00:00:00Z".to_string(),
        started_instant: Instant::now(),
    })
}

// 1. Web Dashboard Usability: Form Elements & Action Buttons
#[test]
fn test_dashboard_usability_html_structure() {
    let html_string = render_dashboard().into_string();

    // Verify critical usability modal & form IDs exist for user interaction
    assert!(html_string.contains("id=\"serverModal\""), "Modal element #serverModal missing");
    assert!(html_string.contains("id=\"serverForm\""), "Form element #serverForm missing");
    assert!(html_string.contains("id=\"serverName\""), "Input #serverName missing");
    assert!(html_string.contains("id=\"serverUrl\""), "Input #serverUrl missing");
    assert!(html_string.contains("id=\"serverKey\""), "Input #serverKey missing");
    assert!(html_string.contains("id=\"serverType\""), "Select dropdown #serverType missing");
    assert!(html_string.contains("id=\"serverDirection\""), "Select dropdown #serverDirection missing");

    // Verify Action Buttons with clear, intuitive labels
    assert!(html_string.contains("AUTO"), "Autofill button label missing");
    assert!(html_string.contains("PING LINK"), "Ping link button label missing");
    assert!(html_string.contains("SAVE"), "Save button label missing");
    assert!(html_string.contains("SAVE SETTINGS"), "Save settings button label missing");
}

// 2. Toast Notification Usability: Human-Readable User Feedback
#[test]
fn test_dashboard_usability_toast_feedback() {
    let js_code = render_full_js();

    // Assert user-facing notification function showToast is defined
    assert!(js_code.contains("showToast"), "showToast helper missing in JavaScript assets");

    // Assert human-readable status toasts for user actions
    assert!(js_code.contains("LINK DATA INCOMPLETE"), "Missing missing-data toast message");
    assert!(js_code.contains("PINGING LINK ADDRESS..."), "Missing pinging progress toast message");
    assert!(js_code.contains("AUTO FILLED:"), "Missing autofill success toast message");
    assert!(js_code.contains("AUTO FILL FAILED:"), "Missing autofill failure toast message");
}

// 3. API Error Usability: Clear Diagnostic Feedback over HTTP
#[tokio::test]
async fn test_api_test_connection_usability_error_messages() {
    let web_state = make_test_web_state();
    let app = create_router(web_state);

    // Test invalid scheme usability response
    let invalid_req = Request::builder()
        .method("POST")
        .uri("/api/test_connection")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"url": "ftp://bad-scheme", "api_key": "key", "is_emby": false}"#))
        .unwrap();

    let response = app.clone().oneshot(invalid_req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json_res: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json_res["status"], "error");
    assert!(
        json_res["message"].as_str().unwrap().contains("Invalid URL"),
        "User error message must explicitly state invalid URL scheme"
    );

    // Test missing key usability response
    let missing_data_req = Request::builder()
        .method("POST")
        .uri("/api/test_connection")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"url": "http://127.0.0.1:8096", "api_key": "  ", "is_emby": false}"#))
        .unwrap();

    let response2 = app.oneshot(missing_data_req).await.unwrap();
    let body_bytes2 = axum::body::to_bytes(response2.into_body(), usize::MAX).await.unwrap();
    let json_res2: serde_json::Value = serde_json::from_slice(&body_bytes2).unwrap();
    assert_eq!(json_res2["status"], "error");
}

// 4. Server Info Autofill Usability: Auto-detect Server Type
#[tokio::test]
async fn test_api_server_info_usability_autofill() {
    let web_state = make_test_web_state();
    let app = create_router(web_state);

    // Request missing 'url' query param
    let req = Request::builder()
        .method("GET")
        .uri("/api/server-info")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json_res: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(
        json_res["error"].as_str().unwrap().contains("missing or invalid 'url'"),
        "User feedback must clearly explain missing 'url' parameter"
    );
}
