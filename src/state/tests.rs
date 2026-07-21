use super::cache::ServerCache;
use super::user_mapping::find_mapped_user_id;
use std::collections::HashMap;

#[test]
fn test_exact_username_match() {
    let mut target_users = HashMap::new();
    target_users.insert("john".to_string(), "id123".to_string());
    let mapped = find_mapped_user_id("JOHN", &target_users, &[]);
    assert_eq!(mapped, Some("id123".to_string()));
}

#[test]
fn test_substring_username_match_disabled_by_default() {
    unsafe {
        std::env::remove_var("STATESYNC_FUZZY_USER_MATCH");
    }
    let mut target_users = HashMap::new();
    target_users.insert("john".to_string(), "id123".to_string());
    let mapped = find_mapped_user_id("John Doe", &target_users, &[]);
    assert_eq!(mapped, None);
}

#[test]
fn test_substring_username_match_when_enabled() {
    unsafe {
        std::env::set_var("STATESYNC_FUZZY_USER_MATCH", "true");
    }
    let mut target_users = HashMap::new();
    target_users.insert("john".to_string(), "id123".to_string());
    let mapped = find_mapped_user_id("John Doe", &target_users, &[]);
    assert_eq!(mapped, Some("id123".to_string()));

    let mut target_users2 = HashMap::new();
    target_users2.insert("john doe".to_string(), "id456".to_string());
    let mapped2 = find_mapped_user_id("john", &target_users2, &[]);
    assert_eq!(mapped2, Some("id456".to_string()));
    unsafe {
        std::env::remove_var("STATESYNC_FUZZY_USER_MATCH");
    }
}

#[test]
fn test_custom_username_mapping_override() {
    let mut target_users = HashMap::new();
    target_users.insert("john_alt".to_string(), "id999".to_string());
    target_users.insert("john".to_string(), "id123".to_string());
    let custom_mappings = vec![vec!["john_special".to_string(), "john_alt".to_string()]];
    let mapped = find_mapped_user_id("john_special", &target_users, &custom_mappings);
    assert_eq!(mapped, Some("id999".to_string()));
}

#[test]
fn test_username_collision_prevention() {
    let mut target_users = HashMap::new();
    target_users.insert("john smith".to_string(), "id777".to_string());
    let mapped = find_mapped_user_id("john doe", &target_users, &[]);
    assert_eq!(mapped, None);
}

#[test]
fn test_substring_length_guard_rejects_short_lookalikes() {
    let mut target_users = HashMap::new();
    target_users.insert("alice".to_string(), "id_a".to_string());
    target_users.insert("aaron".to_string(), "id_b".to_string());
    let mapped = find_mapped_user_id("a", &target_users, &[]);
    assert_eq!(mapped, None);
}

#[test]
fn test_substring_picks_closest_match() {
    unsafe {
        std::env::set_var("STATESYNC_FUZZY_USER_MATCH", "true");
    }
    let mut target_users = HashMap::new();
    target_users.insert("alice smith".to_string(), "id_long".to_string());
    target_users.insert("alice".to_string(), "id_short".to_string());
    // Exact match still wins when present.
    let mapped = find_mapped_user_id("alice", &target_users, &[]);
    assert_eq!(mapped, Some("id_short".to_string()));
    unsafe {
        std::env::remove_var("STATESYNC_FUZZY_USER_MATCH");
    }
}

#[test]
fn merge_users_preserves_existing_entries() {
    let mut cache = ServerCache {
        name: "emby".to_string(),
        users: HashMap::new(),
        imdb_to_id: HashMap::new(),
        tmdb_to_id: HashMap::new(),
        id_to_providers: HashMap::new(),
    };
    cache.users.insert("alice".to_string(), "u1".to_string());
    cache.users.insert("bob".to_string(), "u2".to_string());
    cache.users.insert("carol".to_string(), "u3".to_string());

    let mut fresh = HashMap::new();
    fresh.insert("alice".to_string(), "u1".to_string());
    fresh.insert("dave".to_string(), "u4".to_string());
    cache.merge_users(fresh);

    assert!(cache.users.contains_key("alice"));
    assert!(cache.users.contains_key("bob"));
    assert!(cache.users.contains_key("carol"));
    assert!(cache.users.contains_key("dave"));
    assert_eq!(cache.users.len(), 4);
}

#[test]
fn merge_users_empty_fresh_is_noop() {
    let mut cache = ServerCache {
        name: "emby".to_string(),
        users: HashMap::new(),
        imdb_to_id: HashMap::new(),
        tmdb_to_id: HashMap::new(),
        id_to_providers: HashMap::new(),
    };
    cache.users.insert("alice".to_string(), "u1".to_string());
    cache.merge_users(HashMap::new());
    assert_eq!(cache.users.len(), 1);
    assert!(cache.users.contains_key("alice"));
}

#[test]
fn test_default_log_retention() {
    unsafe {
        std::env::set_var("STATESYNC_LOG_RETENTION", "5");
    }
    let state = super::AppState::new(vec![]);
    assert_eq!(state.log_retention, 5);

    unsafe {
        std::env::set_var("STATESYNC_LOG_RETENTION", "0");
    }
    let state_min = super::AppState::new(vec![]);
    assert_eq!(state_min.log_retention, 1);

    unsafe {
        std::env::remove_var("STATESYNC_LOG_RETENTION");
    }
    let state_default = super::AppState::new(vec![]);
    assert_eq!(state_default.log_retention, 100);
}

#[test]
fn test_app_state_log_event_retention() {
    let mut state = super::AppState::new(vec![]);
    state.log_retention = 3;

    state.log_event("info", "msg 1");
    state.log_event("warn", "msg 2");
    state.log_event("error", "msg 3");
    assert_eq!(state.sync_logs.len(), 3);
    assert_eq!(state.sync_logs[0].message, "msg 3");

    state.log_event("info", "msg 4");
    assert_eq!(state.sync_logs.len(), 3);
    assert_eq!(state.sync_logs[0].message, "msg 4");
    assert_eq!(state.sync_logs[2].message, "msg 2");
}

#[test]
fn test_app_state_log_sync() {
    let mut state = super::AppState::new(vec![]);
    state.log_retention = 2;

    let entry1 = super::SyncLogEntry {
        timestamp: "12:00".to_string(),
        level: "success".to_string(),
        message: "synced 1".to_string(),
        detail: None,
        source_name: None,
        source_is_emby: None,
        target_name: None,
        target_is_emby: None,
    };
    let entry2 = super::SyncLogEntry {
        timestamp: "12:01".to_string(),
        level: "success".to_string(),
        message: "synced 2".to_string(),
        detail: None,
        source_name: None,
        source_is_emby: None,
        target_name: None,
        target_is_emby: None,
    };
    state.log_sync(entry1);
    state.log_sync(entry2);
    assert_eq!(state.sync_logs.len(), 2);
    assert_eq!(state.sync_logs[0].message, "synced 2");

    let entry3 = super::SyncLogEntry {
        timestamp: "12:02".to_string(),
        level: "success".to_string(),
        message: "synced 3".to_string(),
        detail: None,
        source_name: None,
        source_is_emby: None,
        target_name: None,
        target_is_emby: None,
    };
    state.log_sync(entry3);
    assert_eq!(state.sync_logs.len(), 2);
    assert_eq!(state.sync_logs[0].message, "synced 3");
}

#[tokio::test]
async fn test_init_server_cache_empty() {
    // We need to test init_server_cache which calls MediaClient get_users and get_library_items.
    // We can spin up a mockito server to intercept these calls!
    let mut server = mockito::Server::new_async().await;

    let users_mock = server
        .mock("GET", "/Users?StartIndex=0&Limit=500")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"Items": [], "TotalRecordCount": 0}"#)
        .create_async()
        .await;

    let items_mock = server.mock("GET", "/Items?Recursive=true&Fields=ProviderIds&IncludeItemTypes=Movie,Episode&StartIndex=0&Limit=500")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"Items": [], "TotalRecordCount": 0}"#)
            .create_async().await;

    let client = crate::client::MediaClient::new(server.url(), "key".to_string(), false);
    let cache = super::init_server_cache("test_cache", &client)
        .await
        .unwrap();

    users_mock.assert_async().await;
    items_mock.assert_async().await;

    assert_eq!(cache.name, "test_cache");
    assert!(cache.users.is_empty());
    assert!(cache.imdb_to_id.is_empty());
}

#[tokio::test]
async fn test_init_server_cache_with_data() {
    let mut server = mockito::Server::new_async().await;
    let users_mock = server
        .mock("GET", "/Users?StartIndex=0&Limit=500")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"Items": [{"Name": "Alice", "Id": "u123"}], "TotalRecordCount": 1}"#)
        .create_async()
        .await;
    let items_mock = server.mock("GET", "/Items?Recursive=true&Fields=ProviderIds&IncludeItemTypes=Movie,Episode&StartIndex=0&Limit=500")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"Items": [{"Id": "item_1", "ProviderIds": {"Imdb": "tt123", "Tmdb": "tm456"}}], "TotalRecordCount": 1}"#)
            .create_async().await;

    let client = crate::client::MediaClient::new(server.url(), "key".to_string(), false);
    let cache = super::init_server_cache("test_cache", &client)
        .await
        .unwrap();

    users_mock.assert_async().await;
    items_mock.assert_async().await;

    assert_eq!(cache.name, "test_cache");
    assert_eq!(cache.users.get("alice").unwrap(), "u123");
    assert_eq!(cache.imdb_to_id.get("tt123").unwrap(), "item_1");
    assert_eq!(cache.tmdb_to_id.get("tm456").unwrap(), "item_1");
    let provs = cache.id_to_providers.get("item_1").unwrap();
    assert_eq!(provs.0, "tt123");
    assert_eq!(provs.1, "tm456");
}
