use crate::client::UserDataEntry;
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
    let mut cache = ServerCache::empty(name.to_string());
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
    cache.index_item(
        "item1".to_string(),
        crate::client::ProviderIds::from_parts("tt1234567", "tm1", "73244"),
    );
    assert_eq!(
        cache.id_to_providers.get("item1").unwrap().imdb,
        "tt1234567"
    );
    assert!(cache.imdb_to_id.contains_key("tt1234567"));
    assert!(cache.tmdb_to_id.contains_key("tm1"));
    assert!(cache.tvdb_to_id.contains_key("73244"));
    assert_eq!(
        cache
            .lookup_item_id(&crate::client::ProviderIds::from_parts("", "", "73244"))
            .as_deref(),
        Some("item1")
    );
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
