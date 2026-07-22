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
    assert!(validate_upstream_url("http://evil@169.254.169.254/").is_err());
    assert!(validate_upstream_url("http://2852039166/").is_err());
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

    let mut cache = ServerCache::empty("test_server");
    cache
        .users
        .insert("alice".to_string(), "u1".to_string());
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
