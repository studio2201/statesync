use super::config::mask_api_key;
use super::validation::{valid_item_id, valid_server_name};

#[test]
fn test_mask_api_key() {
    assert_eq!(mask_api_key(""), "");
    assert_eq!(mask_api_key("12345678"), "••••••••");
    assert_eq!(mask_api_key("123456789"), "1234••••••••6789");
    assert_eq!(mask_api_key("my_secret_token_1234"), "my_s••••••••1234");
}

#[test]
fn test_valid_item_id() {
    assert!(valid_item_id("abc123XYZ_-"));
    assert!(!valid_item_id(""));
    assert!(!valid_item_id("../etc/passwd"));
    assert!(!valid_item_id("a b"));
    assert!(!valid_item_id(&"a".repeat(65)));
}

#[test]
fn test_valid_server_name() {
    assert!(valid_server_name("green"));
    assert!(valid_server_name("my-server_01.local"));
    assert!(valid_server_name("name with space"));
    assert!(valid_server_name("192.168.1.50"));
    assert!(!valid_server_name(""));
    assert!(!valid_server_name("../etc"));
    assert!(!valid_server_name("a/b"));
}

#[test]
fn test_valid_server_url() {
    use super::validation::{valid_server_url, validate_upstream_url};
    assert!(valid_server_url("http://localhost:8096"));
    assert!(valid_server_url("https://emby.example.com"));
    assert!(valid_server_url("192.168.1.50:8096")); // bare host:port OK
    assert!(!valid_server_url("ftp://localhost:8096"));
    // Path is stripped to origin — becomes valid base URL
    assert!(valid_server_url("http://localhost/web/index.html"));
    assert!(!valid_server_url(&format!("http://{}", "a".repeat(510))));
    assert!(validate_upstream_url("http://169.254.169.254/").is_err());
    assert!(validate_upstream_url("http://10.0.0.10:8096").is_ok());
}

#[tokio::test]
async fn test_cache_stats() {
    use crate::state::{AppState, ServerCache};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let app_state = Arc::new(Mutex::new(AppState::new(vec![])));
    let stats = super::status::cache_stats(&app_state).await;
    assert_eq!(stats.total_servers, 0);
    assert_eq!(stats.total_users, 0);

    let cache = ServerCache {
        name: "test_server".to_string(),
        users: [("alice".to_string(), "u1".to_string())]
            .into_iter()
            .collect(),
        imdb_to_id: std::collections::HashMap::new(),
        tmdb_to_id: std::collections::HashMap::new(),
        id_to_providers: std::collections::HashMap::new(),
    };
    let app_state_with_cache = Arc::new(Mutex::new(AppState::new(vec![cache])));
    let stats2 = super::status::cache_stats(&app_state_with_cache).await;
    assert_eq!(stats2.total_servers, 1);
    assert_eq!(stats2.total_users, 1);
}

#[tokio::test]
async fn test_post_reload_channel_closed() {
    use crate::state::AppState;
    use crate::web::WebServerState;
    use axum::Extension;
    use axum::response::IntoResponse;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::sync::mpsc;

    let app_state = Arc::new(Mutex::new(AppState::new(vec![])));
    let (tx, rx) = mpsc::channel(1);
    drop(rx); // close receiver to force send failure

    let web_state = Arc::new(WebServerState {
        app_state,
        reload_tx: tx,
        bind_addr: "127.0.0.1:0".to_string(),
        web_auth: None,
        version: "test".to_string(),
        started_at: "2025-01-01".to_string(),
        started_instant: std::time::Instant::now(),
    });

    let resp = super::sync::post_reload(Extension(web_state))
        .await
        .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

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
        bind_addr: "127.0.0.1:0".to_string(),
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
        bind_addr: "127.0.0.1:0".to_string(),
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
        bind_addr: "127.0.0.1:0".to_string(),
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
