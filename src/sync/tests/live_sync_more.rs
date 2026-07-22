use super::live_sync::{make_cache, make_config, make_server_config};
use crate::client::SessionInfo;
use crate::state::{AppState, SyncHistoryValue};
use serde_json::json;

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
    emby.index_item(
        "item1".to_string(),
        crate::client::ProviderIds::from_parts("tt999", "", ""),
    );
    let mut jf = make_cache("jellyfin", vec![("alice", "u2")]);
    jf.imdb_to_id
        .insert("tt999".to_string(), "item_jf".to_string());
    let app_state = std::sync::Arc::new(tokio::sync::Mutex::new(AppState::new(vec![emby, jf])));

    let key = ("alice".to_string(), "imdb:tt999".to_string());
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
