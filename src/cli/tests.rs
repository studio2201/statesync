use statesync::config::{Config, ServerConfig};
use statesync::state::AppState;
use super::init_clients_parallel;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_init_clients_parallel_connection_failure() {
    let config = Config {
        servers: vec![ServerConfig {
            name: "failing_server".to_string(),
            url: "http://127.0.0.1:59999".to_string(), // guaranteed to fail to connect
            api_key: "some_key".to_string(),
            is_emby: false,
            sync_direction: "both".to_string(),
            allow_insecure_http: true,
        }],
        sync_threshold_seconds: 5,
        user_mappings: vec![],
        last_full_sync: None,
        sync: Default::default(),
    };

    let app_state = Arc::new(Mutex::new(AppState::new(vec![])));
    
    // Initialize websocket_statuses in app_state like main.rs does
    {
        let mut state = app_state.lock().await;
        state.websocket_statuses = vec!["Offline".to_string(); config.servers.len()];
    }

    // Run init_clients_parallel
    let (clients, caches) = init_clients_parallel(&config, &app_state)
        .await
        .unwrap();

    // Assert that the client and cache were still created
    assert_eq!(clients.len(), 1);
    assert_eq!(caches.len(), 1);
    assert_eq!(caches[0].name, "failing_server");
    assert!(caches[0].users.is_empty());

    let state = app_state.lock().await;
    // The status should have transitioned to "Error"
    assert_eq!(state.websocket_statuses[0], "Error");

    // The failure event should have been logged in sync_logs
    assert!(!state.sync_logs.is_empty());
    let error_log = &state.sync_logs[0];
    assert_eq!(error_log.level, "error");
    assert!(error_log.message.contains("Failed to connect / init cache for 'failing_server'"));
}

static CLI_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn test_resolve_bind_addr() {
    let _guard = CLI_TEST_LOCK.lock().unwrap();
    unsafe {
        std::env::set_var("STATESYNC_BIND", "127.0.0.1:9000");
    }
    assert_eq!(super::helpers::resolve_bind_addr(), "127.0.0.1:9000");

    unsafe {
        std::env::remove_var("STATESYNC_BIND");
    }
    assert_eq!(super::helpers::resolve_bind_addr(), super::helpers::DEFAULT_BIND);
}


#[test]
fn test_resolve_web_auth_disabled() {
    let _guard = CLI_TEST_LOCK.lock().unwrap();
    // Auth is intentionally always off — even if env is set.
    unsafe {
        std::env::set_var("STATESYNC_WEB_AUTH", "bearer:secret");
    }
    assert_eq!(super::helpers::resolve_web_auth(), None);
    unsafe {
        std::env::remove_var("STATESYNC_WEB_AUTH");
    }
    assert_eq!(super::helpers::resolve_web_auth(), None);
}

#[test]
fn test_print_help() {
    // Just verify it prints and doesn't panic
    super::helpers::print_help();
}

#[test]
fn test_parse_sync_force_args() {
    use statesync::sync_force::Direction;

    let args1 = vec!["binary".to_string(), "--sync-force".to_string()];
    assert_eq!(super::force_sync::parse_sync_force_args(&args1), (Direction::Both, false));

    let args2 = vec!["binary".to_string(), "--sync-force".to_string(), "--dry-run".to_string()];
    assert_eq!(super::force_sync::parse_sync_force_args(&args2), (Direction::Both, true));

    let args3 = vec!["binary".to_string(), "--sync-force".to_string(), "--preview".to_string()];
    assert_eq!(super::force_sync::parse_sync_force_args(&args3), (Direction::Both, true));
}

#[test]
fn test_draw_tui_from_json() {
    let status_json = serde_json::json!({
        "servers": [
            {
                "name": "Server1",
                "websocket_status": "Synchronizing",
                "users_count": 5,
                "media_count": 100
            },
            {
                "name": "Server2",
                "websocket_status": "Offline",
                "users_count": 0,
                "media_count": 0
            }
        ],
        "active_sessions": [
            {
                "server": "Server1",
                "user": "Alice",
                "item": "Test Movie",
                "position": 120.0,
                "is_paused": false
            }
        ],
        "last_full_sync": {
            "state": "Completed",
            "succeeded": 10,
            "skipped": 50,
            "failed": 0,
            "skip_reasons": { "already_equal": 40, "no_provider": 10 }
        },
        "sync_logs": [
            {
                "timestamp": "12:00:00",
                "level": "success",
                "message": "Synced watch state",
                "detail": "source=A → target=B"
            }
        ]
    });
    super::tui::draw_tui_from_json(&status_json);
}

#[tokio::test]
async fn test_trigger_reload_success() {
    let _guard = CLI_TEST_LOCK.lock().unwrap();
    let mut server = mockito::Server::new_async().await;
    let mock_call = server.mock("POST", "/api/reload")
        .with_status(200)
        .create_async().await;

    unsafe {
        std::env::set_var("STATESYNC_RELOAD_URL", format!("{}/api/reload", server.url()));
        std::env::set_var("STATESYNC_WEB_AUTH", "bearer:mysecret");
    }

    let res = super::dry_run::trigger_reload().await;
    assert!(res.is_ok());

    mock_call.assert_async().await;

    unsafe {
        std::env::remove_var("STATESYNC_RELOAD_URL");
        std::env::remove_var("STATESYNC_WEB_AUTH");
    }
}
