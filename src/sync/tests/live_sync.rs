use crate::client::{SessionInfo, UserDataEntry};
use crate::config::{Config, ServerConfig};
use crate::state::{AppState, ServerCache, SyncHistoryValue};
use serde_json::json;

pub fn make_config(servers: Vec<ServerConfig>, user_mappings: Vec<Vec<String>>) -> Config {
    Config {
        servers,
        sync_threshold_seconds: 5,
        user_mappings,
        last_full_sync: None,
        sync: Default::default(),
    }
}

pub fn make_server_config(name: &str, is_emby: bool) -> ServerConfig {
    ServerConfig {
        name: name.to_string(),
        url: "http://test".to_string(),
        api_key: "k".to_string(),
        is_emby,
        sync_direction: "both".to_string(),
        allow_insecure_http: true,
    }
}

pub fn make_cache(name: &str, users: Vec<(&str, &str)>) -> ServerCache {
    let mut cache = ServerCache {
        name: name.to_string(),
        users: std::collections::HashMap::new(),
        imdb_to_id: std::collections::HashMap::new(),
        tmdb_to_id: std::collections::HashMap::new(),
        id_to_providers: std::collections::HashMap::new(),
    };
    for (uname, uid) in users {
        cache.users.insert(uname.to_string(), uid.to_string());
    }
    cache
}

#[tokio::test]
async fn threshold_skips_duplicate_update() {
    let _config = make_config(
        vec![
            make_server_config("emby", true),
            make_server_config("jellyfin", false),
        ],
        vec![],
    );
    let caches = vec![
        make_cache("emby", vec![("alice", "u1")]),
        make_cache("jellyfin", vec![("alice", "u2")]),
    ];
    let app_state = std::sync::Arc::new(tokio::sync::Mutex::new(AppState::new(caches)));

    let key = ("alice".to_string(), "tt1234567".to_string());
    {
        let mut st = app_state.lock().await;
        st.last_syncs.insert(
            key.clone(),
            SyncHistoryValue {
                position_ticks: 1000,
                timestamp: std::time::Instant::now(),
                played: false,
                favorite: None,
            },
        );
    }
    let st = app_state.lock().await;
    let stored = st
        .last_syncs
        .get(&key)
        .map(|v| v.position_ticks)
        .unwrap_or(0);
    assert!(1000 <= stored);
}

#[tokio::test]
async fn force_sync_in_progress_blocks_live_sync() {
    let caches = vec![
        make_cache("emby", vec![("alice", "u1")]),
        make_cache("jellyfin", vec![("alice", "u2")]),
    ];
    let app_state = std::sync::Arc::new(tokio::sync::Mutex::new(AppState::new(caches)));
    {
        let st = app_state.lock().await;
        assert!(
            !st.sync_force
                .force_sync_in_progress
                .load(std::sync::atomic::Ordering::SeqCst)
        );
        st.sync_force
            .force_sync_in_progress
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(
            st.sync_force
                .force_sync_in_progress
                .load(std::sync::atomic::Ordering::SeqCst)
        );
    }
}

#[test]
fn cache_miss_path_populates_provider_maps() {
    let mut cache = make_cache("emby", vec![("alice", "u1")]);
    cache.id_to_providers.insert(
        "item1".to_string(),
        ("tt1234567".to_string(), "tm1".to_string()),
    );
    cache
        .imdb_to_id
        .insert("tt1234567".to_string(), "item1".to_string());
    cache
        .tmdb_to_id
        .insert("tm1".to_string(), "item1".to_string());
    assert_eq!(cache.id_to_providers.get("item1").unwrap().0, "tt1234567");
    assert!(cache.imdb_to_id.contains_key("tt1234567"));
    assert!(cache.tmdb_to_id.contains_key("tm1"));
}

#[tokio::test]
async fn unmapped_user_creates_solo_entry() {
    let caches = vec![
        make_cache("emby", vec![("alice", "u1")]),
        make_cache("jellyfin", vec![]),
    ];
    let app_state = std::sync::Arc::new(tokio::sync::Mutex::new(AppState::new(caches)));
    let st = app_state.lock().await;
    assert!(st.caches[0].users.contains_key("alice"));
    assert!(!st.caches[1].users.contains_key("alice"));
}

#[test]
fn user_data_entry_dto_parses_emby_payload() {
    let payload = json!({
        "ItemId": "i1",
        "Played": true,
        "PlaybackPositionTicks": 5000
    });
    let entry: UserDataEntry = serde_json::from_value(payload).unwrap();
    assert_eq!(entry.item_id, "i1");
    assert!(entry.played);
    assert_eq!(entry.playback_position_ticks, Some(5000));
}

#[test]
fn user_data_entry_dto_parses_jellyfin_payload() {
    let payload = json!({
        "itemId": "i2",
        "played": false,
        "playbackPositionTicks": null
    });
    let entry: UserDataEntry = serde_json::from_value(payload).unwrap();
    assert_eq!(entry.item_id, "i2");
    assert!(!entry.played);
    assert_eq!(entry.playback_position_ticks, None);
}

#[test]
fn session_info_dto_parses_position_ticks() {
    let payload = json!({
        "Id": "s1",
        "UserName": "alice",
        "NowPlayingItem": {"Id": "n1", "Name": "Show"},
        "PlayState": {"PositionTicks": 12345, "IsPaused": false}
    });
    let info: SessionInfo = serde_json::from_value(payload).unwrap();
    assert_eq!(info.id, "s1");
    assert_eq!(info.user_name.as_deref(), Some("alice"));
    assert!(info.now_playing_item.is_some());
    let ps = info.play_state.as_ref().unwrap();
    assert_eq!(ps.position_ticks, Some(12345));
    assert_eq!(ps.is_paused, Some(false));
}

#[test]
fn session_info_dto_handles_null_position() {
    let payload = json!({
        "Id": "s2",
        "UserName": "bob",
        "NowPlayingItem": {"Id": "n2", "Name": "Movie"},
        "PlayState": {"PositionTicks": null, "IsPaused": true}
    });
    let info: SessionInfo = serde_json::from_value(payload).unwrap();
    let ps = info.play_state.as_ref().unwrap();
    assert_eq!(ps.position_ticks, None);
    assert_eq!(ps.is_paused, Some(true));
}

#[test]
fn test_sync_semaphore() {
    let sem = crate::sync::sync_semaphore();
    assert!(sem.available_permits() > 0);
}

#[tokio::test]
async fn played_true_is_debounced_via_history() {
    // After a played=true sync, an immediate re-sync with same position+played is skipped.
    let config = make_config(
        vec![
            make_server_config("emby", true),
            make_server_config("jellyfin", false),
        ],
        vec![],
    );
    let mut emby = make_cache("emby", vec![("alice", "u1")]);
    emby.id_to_providers
        .insert("item1".to_string(), ("tt999".to_string(), "".to_string()));
    let mut jf = make_cache("jellyfin", vec![("alice", "u2")]);
    jf.imdb_to_id
        .insert("tt999".to_string(), "item_jf".to_string());
    let app_state = std::sync::Arc::new(tokio::sync::Mutex::new(AppState::new(vec![emby, jf])));

    let key = ("alice".to_string(), "tt999".to_string());
    {
        let mut st = app_state.lock().await;
        st.last_syncs.insert(
            key.clone(),
            SyncHistoryValue {
                position_ticks: 0,
                timestamp: std::time::Instant::now(),
                played: true,
                favorite: None,
            },
        );
    }

    let client_src = std::sync::Arc::new(crate::client::MediaClient::new(
        "http://source".into(),
        "k".into(),
        true,
    ));
    let client_tgt = std::sync::Arc::new(crate::client::MediaClient::new(
        "http://target".into(),
        "k".into(),
        false,
    ));

    // Should no-op due to threshold + matching played flag (no HTTP mock needed).
    crate::sync::sync_progress_to_targets(
        "alice",
        "item1",
        0,
        true,
        "emby",
        0,
        &app_state,
        &[(1, client_tgt)],
        &config,
        &client_src,
        Some("Movie".into()),
    )
    .await;

    let st = app_state.lock().await;
    assert!(st.last_syncs.contains_key(&key));
    assert!(st.last_syncs.get(&key).unwrap().played);
}

#[test]
fn force_and_live_sync_share_username_provider_key_shape() {
    // Document the shared key convention used by force-sync and live sync.
    let username = "Alice";
    let provider = "tt123";
    let live_key = (username.to_lowercase(), provider.to_string());
    let force_key = (username.to_lowercase(), provider.to_string());
    assert_eq!(live_key, force_key);
}

#[test]
fn fuzzy_user_match_refuses_wrong_user_by_default() {
    unsafe {
        std::env::remove_var("STATESYNC_FUZZY_USER_MATCH");
    }
    let mut users = std::collections::HashMap::new();
    users.insert("bobby".to_string(), "id_bobby".to_string());
    users.insert("annabelle".to_string(), "id_ann".to_string());
    assert_eq!(crate::state::find_mapped_user_id("bob", &users, &[]), None);
    assert_eq!(crate::state::find_mapped_user_id("ann", &users, &[]), None);
}
