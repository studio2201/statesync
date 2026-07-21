use crate::state::AppState;
use super::live_sync::{make_config, make_server_config, make_cache};

#[tokio::test]
async fn test_resolve_item_providers_cache_hit() {
    let mut cache = make_cache("emby", vec![("alice", "u1")]);
    cache.id_to_providers.insert("item1".to_string(), ("imdb123".to_string(), "tmdb456".to_string()));
    let state = std::sync::Arc::new(tokio::sync::Mutex::new(AppState::new(vec![cache])));
    let client = std::sync::Arc::new(crate::client::MediaClient::new("http://test".to_string(), "key".to_string(), false));

    let res = crate::sync::resolve::resolve_item_providers(
        0,
        "item1",
        &client,
        "alice",
        &state,
        "emby"
    ).await;

    assert_eq!(res, Some(("imdb123".to_string(), "tmdb456".to_string())));
}

#[tokio::test]
async fn test_resolve_item_providers_cache_miss_success() {
    let mut server = mockito::Server::new_async().await;
    let mock_call = server.mock("GET", "/Users/u1/Items/item1")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"ProviderIds": {"Imdb": "imdb123", "Tmdb": "tmdb456"}}"#)
        .create_async().await;

    let cache = make_cache("emby", vec![("alice", "u1")]);
    let state = std::sync::Arc::new(tokio::sync::Mutex::new(AppState::new(vec![cache])));
    let client = std::sync::Arc::new(crate::client::MediaClient::new(server.url(), "key".to_string(), false));

    let res = crate::sync::resolve::resolve_item_providers(
        0,
        "item1",
        &client,
        "alice",
        &state,
        "emby"
    ).await;

    mock_call.assert_async().await;
    assert_eq!(res, Some(("imdb123".to_string(), "tmdb456".to_string())));

    // Check if cache got updated
    let st = state.lock().await;
    assert_eq!(st.caches[0].id_to_providers.get("item1").unwrap(), &("imdb123".to_string(), "tmdb456".to_string()));
}

#[tokio::test]
async fn test_resolve_target_user_fresh_fetch() {
    let mut server = mockito::Server::new_async().await;
    let mock_call = server.mock("GET", "/Users?StartIndex=0&Limit=500")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"Items": [{"Name": "Bob", "Id": "u_bob"}], "TotalRecordCount": 1}"#)
        .create_async().await;

    let caches = vec![make_cache("jellyfin", vec![])];
    let state = std::sync::Arc::new(tokio::sync::Mutex::new(AppState::new(caches)));
    let client = std::sync::Arc::new(crate::client::MediaClient::new(server.url(), "key".to_string(), false));
    let config = make_config(vec![make_server_config("jellyfin", false)], vec![]);

    let res = crate::sync::resolve::resolve_target_user(
        0,
        "bob",
        &client,
        &config,
        &state
    ).await;

    mock_call.assert_async().await;
    assert_eq!(res, Some("u_bob".to_string()));
}

#[tokio::test]
async fn test_resolve_target_item_cache_hit() {
    let mut cache = make_cache("jellyfin", vec![]);
    cache.imdb_to_id.insert("imdb123".to_string(), "item_jf".to_string());
    let state = std::sync::Arc::new(tokio::sync::Mutex::new(AppState::new(vec![cache])));
    let client = std::sync::Arc::new(crate::client::MediaClient::new("http://test".to_string(), "key".to_string(), false));

    let res = crate::sync::resolve::resolve_target_item(
        0,
        "imdb123",
        "",
        "jellyfin",
        Some("u1"),
        &client,
        &state
    ).await;

    assert_eq!(res, Some("item_jf".to_string()));
}

#[tokio::test]
async fn test_resolve_target_item_negative_cached() {
    let mut cache = make_cache("jellyfin", vec![]);
    cache.imdb_to_id.insert("imdb123".to_string(), "[ NOT_FOUND ]".to_string());
    let state = std::sync::Arc::new(tokio::sync::Mutex::new(AppState::new(vec![cache])));
    let client = std::sync::Arc::new(crate::client::MediaClient::new("http://test".to_string(), "key".to_string(), false));

    let res = crate::sync::resolve::resolve_target_item(
        0,
        "imdb123",
        "",
        "jellyfin",
        Some("u1"),
        &client,
        &state
    ).await;

    assert_eq!(res, None);
}

#[tokio::test]
async fn test_resolve_target_item_dynamic_search_success() {
    let mut server = mockito::Server::new_async().await;
    let mock_call = server.mock("GET", "/Users/u1/Items?Recursive=true&Fields=ProviderIds&AnyProviderIdTypes=Imdb&ProviderIds=imdb123")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"Items": [{"Id": "item_resolved", "ProviderIds": {"Imdb": "imdb123"}}]}"#)
        .create_async().await;

    let cache = make_cache("jellyfin", vec![]);
    let state = std::sync::Arc::new(tokio::sync::Mutex::new(AppState::new(vec![cache])));
    let client = std::sync::Arc::new(crate::client::MediaClient::new(server.url(), "key".to_string(), false));

    let res = crate::sync::resolve::resolve_target_item(
        0,
        "imdb123",
        "",
        "jellyfin",
        Some("u1"),
        &client,
        &state
    ).await;

    mock_call.assert_async().await;
    assert_eq!(res, Some("item_resolved".to_string()));
}

#[tokio::test]
async fn test_sync_progress_to_targets_success() {
    let mut server_target = mockito::Server::new_async().await;
    let mock_update = server_target.mock("POST", "/Users/u2/Items/item_jf/UserData")
        .with_status(200)
        .with_body(r#"{"Played": true, "PlaybackPositionTicks": 5000}"#)
        .create_async().await;

    let config = make_config(
        vec![
            make_server_config("emby", true),
            make_server_config("jellyfin", false),
        ],
        vec![],
    );
    let caches = vec![
        {
            let mut c = make_cache("emby", vec![("alice", "u1")]);
            c.id_to_providers.insert("item_emby".to_string(), ("imdb123".to_string(), "".to_string()));
            c
        },
        {
            let mut c = make_cache("jellyfin", vec![("alice", "u2")]);
            c.imdb_to_id.insert("imdb123".to_string(), "item_jf".to_string());
            c
        }
    ];
    let app_state = std::sync::Arc::new(tokio::sync::Mutex::new(AppState::new(caches)));
    let client_source = std::sync::Arc::new(crate::client::MediaClient::new("http://source".to_string(), "key".to_string(), true));
    let client_target = std::sync::Arc::new(crate::client::MediaClient::new(server_target.url(), "key".to_string(), false));

    crate::sync::sync_progress_to_targets(
        "alice",
        "item_emby",
        5000,
        true,
        "emby",
        0,
        &app_state,
        &[(1, client_target.clone())],
        &config,
        &client_source,
        Some("Test Movie".to_string())
    ).await;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    mock_update.assert_async().await;

    let st = app_state.lock().await;
    let key = ("alice".to_string(), "imdb123".to_string());
    assert!(st.last_syncs.contains_key(&key));
}
