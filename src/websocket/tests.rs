use super::handlers::{
    handle_sessions_event, handle_userdata_changed_event, init_cache_in_background,
};
use super::{make_ws_url, next_backoff, redact_api_key};
use crate::client::{
    MediaClient, NowPlayingItem, PlayState, SessionInfo, UserDataChangedInfo, UserDataEntry,
};
use crate::config::{Config, ServerConfig};
use crate::state::AppState;
use std::sync::Arc;
use tokio::sync::Mutex;

#[test]
fn test_make_ws_url() {
    let ws = make_ws_url("http://127.0.0.1:8096", "api-key_123", true);
    assert!(ws.starts_with("ws://127.0.0.1:8096/embywebsocket?api_key="));

    let wss = make_ws_url("https://media.local/", "secret", false);
    assert!(wss.starts_with("wss://media.local/socket?api_key="));

    let ws_no_scheme = make_ws_url("localhost:8096", "key", false);
    assert!(ws_no_scheme.starts_with("ws://localhost:8096/socket?api_key="));
}

#[test]
fn test_make_ws_url_deduplication() {
    let ws_emby = make_ws_url("http://192.168.3.10:8096/emby", "k", true);
    assert!(ws_emby.starts_with("ws://192.168.3.10:8096/embywebsocket?api_key="));

    let ws_emby_ws = make_ws_url("http://192.168.3.10:8096/embywebsocket", "k", true);
    assert!(ws_emby_ws.starts_with("ws://192.168.3.10:8096/embywebsocket?api_key="));

    let ws_jf_socket = make_ws_url("http://192.168.3.10:8096/socket", "k", false);
    assert!(ws_jf_socket.starts_with("ws://192.168.3.10:8096/socket?api_key="));
}

#[test]
fn test_make_ws_url_mixed_case() {
    let wss_upper = make_ws_url("HTTPS://Media-Server:8096/", "key", false);
    assert!(wss_upper.starts_with("wss://Media-Server:8096/socket?api_key="));
}

#[test]
fn test_next_backoff() {
    let bo0 = next_backoff(0);
    assert!(bo0.as_millis() >= 1000 && bo0.as_millis() < 2000);

    let bo10 = next_backoff(10);
    assert!(bo10.as_millis() >= 60000);
}

#[test]
fn test_redact_api_key() {
    let msg = "ws://localhost/socket?api_key=secret_123&deviceId=test";
    let redacted = redact_api_key(msg);
    assert_eq!(
        redacted,
        "ws://localhost/socket?api_key=[REDACTED]&deviceId=test"
    );

    let msg2 = "ws://localhost/socket?api_key=secret_123";
    let redacted2 = redact_api_key(msg2);
    assert_eq!(redacted2, "ws://localhost/socket?api_key=[REDACTED]");

    let msg_none = "ws://localhost/socket?deviceId=test";
    let redacted_none = redact_api_key(msg_none);
    assert_eq!(redacted_none, msg_none);
}

#[tokio::test]
async fn test_init_cache_in_background() {
    let mut server = mockito::Server::new_async().await;

    let users_mock = server
        .mock("GET", "/Users?StartIndex=0&Limit=500")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"Items": [{"Name": "Alice", "Id": "u1"}], "TotalRecordCount": 1}"#)
        .create_async()
        .await;

    let items_mock = server.mock("GET", "/Items?Recursive=true&Fields=ProviderIds&IncludeItemTypes=Movie,Episode&StartIndex=0&Limit=500")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"Items": []}"#)
            .create_async().await;

    let client = Arc::new(MediaClient::new(server.url(), "key".to_string(), false));
    let state = Arc::new(Mutex::new(AppState::new(vec![crate::state::ServerCache {
        name: "test".to_string(),
        users: std::collections::HashMap::new(),
        imdb_to_id: std::collections::HashMap::new(),
        tmdb_to_id: std::collections::HashMap::new(),
        id_to_providers: std::collections::HashMap::new(),
    }])));

    let res = init_cache_in_background(0, "test", &client, &state).await;
    assert!(res.is_ok());

    users_mock.assert_async().await;
    items_mock.assert_async().await;

    let st = state.lock().await;
    assert_eq!(st.caches[0].users.get("alice").unwrap(), "u1");
}

#[tokio::test]
async fn test_handle_sessions_event() {
    let caches = vec![crate::state::ServerCache {
        name: "test_server".to_string(),
        users: [("alice".to_string(), "u1".to_string())]
            .into_iter()
            .collect(),
        imdb_to_id: std::collections::HashMap::new(),
        tmdb_to_id: std::collections::HashMap::new(),
        id_to_providers: std::collections::HashMap::new(),
    }];
    let state = Arc::new(Mutex::new(AppState::new(caches)));

    let sessions = vec![SessionInfo {
        id: "session_1".to_string(),
        user_name: Some("alice".to_string()),
        now_playing_item: Some(NowPlayingItem {
            id: "item_123".to_string(),
            name: Some("Movie".to_string()),
        }),
        play_state: Some(PlayState {
            position_ticks: Some(5000),
            is_paused: Some(false),
        }),
    }];

    let client = Arc::new(MediaClient::new(
        "http://localhost".to_string(),
        "key".to_string(),
        false,
    ));
    let config = Config {
        servers: vec![ServerConfig {
            name: "test_server".to_string(),
            url: "http://localhost".to_string(),
            api_key: "key".to_string(),
            is_emby: false,
            sync_direction: "both".to_string(),
            allow_insecure_http: true,
        }],
        sync_threshold_seconds: 5,
        user_mappings: vec![],
        last_full_sync: None,
        sync: Default::default(),
    };

    handle_sessions_event(sessions, 0, "test_server", &client, &[], &state, &config).await;

    let st = state.lock().await;
    assert!(
        st.active_sessions
            .contains_key(&("test_server".to_string(), "session_1".to_string()))
    );
}

#[tokio::test]
async fn test_handle_userdata_changed_event() {
    let caches = vec![crate::state::ServerCache {
        name: "test_server".to_string(),
        users: [("alice".to_string(), "u1".to_string())]
            .into_iter()
            .collect(),
        imdb_to_id: std::collections::HashMap::new(),
        tmdb_to_id: std::collections::HashMap::new(),
        id_to_providers: std::collections::HashMap::new(),
    }];
    let state = Arc::new(Mutex::new(AppState::new(caches)));

    let info = UserDataChangedInfo {
        user_id: "u1".to_string(),
        user_data_list: vec![UserDataEntry {
            item_id: "item_1".to_string(),
            played: true,
            playback_position_ticks: Some(1000),
            is_favorite: None,
        }],
    };

    let client = Arc::new(MediaClient::new(
        "http://localhost".to_string(),
        "key".to_string(),
        false,
    ));
    let config = Config {
        servers: vec![ServerConfig {
            name: "test_server".to_string(),
            url: "http://localhost".to_string(),
            api_key: "key".to_string(),
            is_emby: false,
            sync_direction: "both".to_string(),
            allow_insecure_http: true,
        }],
        sync_threshold_seconds: 5,
        user_mappings: vec![],
        last_full_sync: None,
        sync: Default::default(),
    };

    handle_userdata_changed_event(info, 0, "test_server", &client, &[], &state, &config).await;

    // No panic, successfully parsed and passed
}
