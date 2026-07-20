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
