#[tokio::test]
async fn test_post_reload_success() {
    use crate::state::AppState;
    use crate::web::WebServerState;
    use axum::Extension;
    use axum::response::IntoResponse;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::sync::mpsc;

    let app_state = Arc::new(Mutex::new(AppState::new(vec![])));
    let (tx, _rx) = mpsc::channel(2);

    let web_state = Arc::new(WebServerState {
        app_state,
        reload_tx: tx,
        web_auth: None,
        version: "test".to_string(),
        started_at: "2025-01-01".to_string(),
        started_instant: std::time::Instant::now(),
    });

    let resp = super::sync::post_reload(Extension(web_state))
        .await
        .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
}

#[tokio::test]
async fn test_test_connection_invalid_params() {
    use super::server::{TestConnRequest, test_connection};
    use axum::Json;
    use axum::http::StatusCode;

    let req_bad_url = TestConnRequest {
        url: "ftp://bad-scheme".to_string(),
        api_key: "key".to_string(),
        is_emby: false,
    };
    let (status, res) = test_connection(Json(req_bad_url)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(res.get("status").unwrap().as_str().unwrap(), "error");

    let req_long_key = TestConnRequest {
        url: "http://localhost:8096".to_string(),
        api_key: "a".repeat(257),
        is_emby: false,
    };
    let (status2, res2) = test_connection(Json(req_long_key)).await;
    assert_eq!(status2, StatusCode::BAD_REQUEST);
    assert_eq!(res2.get("status").unwrap().as_str().unwrap(), "error");

    let req_meta = TestConnRequest {
        url: "http://169.254.169.254/latest".to_string(),
        api_key: "key".to_string(),
        is_emby: false,
    };
    let (status3, res3) = test_connection(Json(req_meta)).await;
    assert_eq!(status3, StatusCode::BAD_REQUEST);
    assert!(
        res3.get("message")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("Blocked")
    );
}

#[tokio::test]
async fn test_test_connection_returns_detailed_error() {
    use super::server::{TestConnRequest, test_connection};
    use axum::Json;
    use axum::http::StatusCode;

    let req_fail = TestConnRequest {
        url: "http://127.0.0.1:1".to_string(), // Unreachable port
        api_key: "key".to_string(),
        is_emby: false,
    };
    let (status, res) = test_connection(Json(req_fail)).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(res.get("status").unwrap().as_str().unwrap(), "error");
    let msg = res.get("message").unwrap().as_str().unwrap();
    assert!(
        msg.contains("Could not reach") || msg.contains("Connection failed"),
        "unexpected error message: {}",
        msg
    );
}

#[test]
fn test_valid_server_url_whitespace_and_case() {
    use super::validation::valid_server_url;
    assert!(valid_server_url("  HTTPS://Media-Server:8096/  "));
    assert!(valid_server_url("http://192.168.1.10:8096"));
    assert!(!valid_server_url("ftp://server"));
}

#[tokio::test]
async fn test_serve_poster_bad_request_rfc_9110() {
    use super::server::serve_poster;
    use crate::state::AppState;
    use crate::web::WebServerState;
    use axum::Extension;
    use axum::extract::Query;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let app_state = Arc::new(Mutex::new(AppState::new(vec![])));
    let (tx, _rx) = tokio::sync::mpsc::channel(1);
    let web_state = Arc::new(WebServerState {
        app_state,
        reload_tx: tx,
        web_auth: None,
        version: "test".to_string(),
        started_at: "2025-01-01".to_string(),
        started_instant: std::time::Instant::now(),
    });

    let mut params = HashMap::new();
    params.insert("server".to_string(), "bad name!".to_string());
    params.insert("item_id".to_string(), "123".to_string());

    let res = serve_poster(Extension(web_state.clone()), Query(params)).await;
    assert_eq!(res.status(), axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_get_server_info_bad_request_rfc_9110() {
    use super::server::get_server_info;
    use crate::state::AppState;
    use crate::web::WebServerState;
    use axum::Extension;
    use axum::extract::Query;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let app_state = Arc::new(Mutex::new(AppState::new(vec![])));
    let (tx, _rx) = tokio::sync::mpsc::channel(1);
    let web_state = Arc::new(WebServerState {
        app_state,
        reload_tx: tx,
        web_auth: None,
        version: "test".to_string(),
        started_at: "2025-01-01".to_string(),
        started_instant: std::time::Instant::now(),
    });

    let params = HashMap::new();
    // Missing "url" parameter completely

    let res = get_server_info(Extension(web_state), Query(params)).await;
    assert_eq!(res.status(), axum::http::StatusCode::BAD_REQUEST);
}
