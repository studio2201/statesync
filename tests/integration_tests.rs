use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use statesync::client::{UserDataChangedInfo, WsMessage};
use statesync::config::{Config, ServerConfig, get_config_path};
use statesync::state::{AppState, find_mapped_user_id};
use statesync::web::{WebServerState, create_router};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, mpsc};
use tower::util::ServiceExt;

fn make_test_config() -> Config {
    Config {
        servers: vec![ServerConfig {
            name: "test_server".to_string(),
            url: "http://localhost:8096".to_string(),
            api_key: "my_super_secret_api_key_123456".to_string(),
            is_emby: true,
            sync_direction: "both".to_string(),
            allow_insecure_http: true,
        }],
        sync_threshold_seconds: 5,
        user_mappings: vec![],
        last_full_sync: None,
    }
}

fn make_web_state(
    app_state: Arc<Mutex<AppState>>,
    reload_tx: mpsc::Sender<()>,
) -> Arc<WebServerState> {
    Arc::new(WebServerState {
        app_state,
        reload_tx,
        bind_addr: "127.0.0.1:0".to_string(),
        web_auth: None,
        version: "test".to_string(),
        started_at: "2025-01-01T00:00:00Z".to_string(),
        started_instant: Instant::now(),
    })
}

// 1. Security Test: Verify API credentials masking in status output
#[tokio::test]
async fn test_api_config_endpoint_masks_keys() {
    let temp_cfg_path = get_config_path();
    let old_content = std::fs::read_to_string(temp_cfg_path).ok();

    let test_config = make_test_config();
    std::fs::write(temp_cfg_path, serde_json::to_string(&test_config).unwrap()).unwrap();

    let app_state = Arc::new(Mutex::new(AppState::new(vec![])));
    let (reload_tx, _) = mpsc::channel(1);
    let web_state = make_web_state(app_state, reload_tx);
    let app = create_router(web_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/config")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let parsed: Config = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(parsed.servers.len(), 1);
    assert_eq!(parsed.servers[0].api_key, "my_s••••••••3456");

    if let Some(content) = old_content {
        std::fs::write(temp_cfg_path, content).unwrap();
    } else {
        let _ = std::fs::remove_file(temp_cfg_path);
    }
}

// 1b. Security Test: Public assets reachable without auth; /api/config requires auth when configured.
#[tokio::test]
async fn test_public_paths_no_auth_needed() {
    let app_state = Arc::new(Mutex::new(AppState::new(vec![])));
    let (reload_tx, _) = mpsc::channel(1);
    let web_state = make_web_state(app_state, reload_tx);
    let app = create_router(web_state);

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_api_protected_with_bearer_auth() {
    let app_state = Arc::new(Mutex::new(AppState::new(vec![])));
    let (reload_tx, _) = mpsc::channel(1);
    let web_state = Arc::new(WebServerState {
        app_state,
        reload_tx,
        bind_addr: "0.0.0.0:4407".to_string(),
        web_auth: Some("bearer:secret123".to_string()),
        version: "test".to_string(),
        started_at: "2025-01-01T00:00:00Z".to_string(),
        started_instant: std::time::Instant::now(),
    });
    let app = create_router(web_state);

    let r1 = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r1.status(), StatusCode::UNAUTHORIZED);

    let r2 = app
        .oneshot(
            Request::builder()
                .uri("/api/status")
                .header("authorization", "Bearer secret123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r2.status(), StatusCode::OK);
}

// 2. RFC Test: WebSocket keep-alive URL query string structure
#[test]
fn test_rfc_websocket_url_structure() {
    let ws_url = statesync::websocket::make_ws_url("http://192.0.2.1:8096", "secret_key_123", true);
    assert!(ws_url.contains("ws://"));
    assert!(ws_url.contains("api_key=secret%5Fkey%5F123"));
    assert!(ws_url.contains("deviceId=statesync"));
    assert!(ws_url.contains("/embywebsocket"));

    let wss_url =
        statesync::websocket::make_ws_url("https://media.myserver.com/", "other_key", false);
    assert!(wss_url.contains("wss://"));
    assert!(wss_url.contains("api_key=other%5Fkey"));
    assert!(wss_url.contains("/socket"));
}

// 3. RFC Test: WebSocket incoming payload formats
#[test]
fn test_rfc_websocket_payload_deserialization() {
    let payload = r#"{
        "MessageType": "UserDataChanged",
        "Data": {
            "UserId": "user123",
            "UserDataList": [
                {
                    "ItemId": "item999",
                    "Played": true,
                    "PlaybackPositionTicks": 10000000
                }
            ]
        }
    }"#;

    let ws_msg: WsMessage = serde_json::from_str(payload).unwrap();
    assert_eq!(ws_msg.message_type, "UserDataChanged");

    let data = ws_msg.data.unwrap();
    let info: UserDataChangedInfo = serde_json::from_value(data).unwrap();
    assert_eq!(info.user_id, "user123");
    assert_eq!(info.user_data_list.len(), 1);
    assert_eq!(info.user_data_list[0].item_id, "item999");
    assert!(info.user_data_list[0].played);
    assert_eq!(
        info.user_data_list[0].playback_position_ticks,
        Some(10000000)
    );
}

// 4. Performance Audit Test: Measure user lookup speeds
#[test]
fn test_performance_audit_user_lookup_latency() {
    let mut target_users = HashMap::new();
    for i in 0..1000 {
        target_users.insert(format!("user_{}", i), format!("id_{}", i));
    }
    let custom_mappings = vec![
        vec!["alias_0".to_string(), "user_0".to_string()],
        vec!["alias_999".to_string(), "user_999".to_string()],
    ];

    let start = Instant::now();
    for _ in 0..10_000 {
        let res = find_mapped_user_id("alias_0", &target_users, &custom_mappings);
        assert!(res.is_some());
    }
    let duration = start.elapsed();
    println!("Processed 10,000 mapping lookups in {:?}", duration);

    // Performance constraint check: must execute within 10 milliseconds
    assert!(duration.as_millis() < 10);
}
