use super::TEST_LOCK;
use crate::client::MediaClient;

#[tokio::test]
async fn test_update_progress() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut server = mockito::Server::new_async().await;
    let mock_ok = server
        .mock("POST", "/Users/u1/Items/item1/UserData")
        .with_status(200)
        .create_async()
        .await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let res = client.update_progress("u1", "item1", 1000, true).await;
    assert!(res.is_ok());
    mock_ok.assert_async().await;
    // send_with_retry attempts up to 3 times on 5xx when retry is enabled.
    let mock_err = server
        .mock("POST", "/Users/u1/Items/item1/UserData")
        .with_status(500)
        .with_body("error message")
        .expect(3)
        .create_async()
        .await;
    let res_err = client.update_progress("u1", "item1", 1000, true).await;
    assert!(res_err.is_err());
    mock_err.assert_async().await;
}
#[tokio::test]
async fn test_get_user_played_items() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut server = mockito::Server::new_async().await;
    let mock_ok = server.mock("GET", "/Users/u1/Items?Recursive=true&Fields=ProviderIds,UserData&Filters=IsPlayed&StartIndex=0&Limit=10")
        .with_status(200)
        .with_body(r#"{"Items": [{"Id": "item1", "Played": true, "PlaybackPositionTicks": 1000, "ProviderIds": {"Imdb": "tt123"}}]}"#)
        .create_async().await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let items = client.get_user_played_items("u1", 0, 10).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "item1");
    assert!(items[0].played);
    assert_eq!(items[0].playback_position_ticks, Some(1000));
    assert_eq!(items[0].imdb_id.as_deref(), Some("tt123"));
    mock_ok.assert_async().await;
    // Test Empty JSON response
    let mock_empty = server.mock("GET", "/Users/u1/Items?Recursive=true&Fields=ProviderIds,UserData&Filters=IsPlayed&StartIndex=0&Limit=10")
        .with_status(200)
        .with_body(r#"{}"#)
        .create_async().await;
    let items_empty = client.get_user_played_items("u1", 0, 10).await.unwrap();
    assert_eq!(items_empty.len(), 0);
    mock_empty.assert_async().await;
    // Test 404 response
    let mock_404 = server.mock("GET", "/Users/u1/Items?Recursive=true&Fields=ProviderIds,UserData&Filters=IsPlayed&StartIndex=0&Limit=10")
        .with_status(404)
        .create_async().await;
    unsafe {
        std::env::set_var("STATESYNC_HTTP_RETRY", "off");
    }
    let res_404 = client.get_user_played_items("u1", 0, 10).await;
    assert!(res_404.is_err());
    mock_404.assert_async().await;
    unsafe {
        std::env::remove_var("STATESYNC_HTTP_RETRY");
    }
}
#[tokio::test]
async fn test_get_user_played_items_count() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut server = mockito::Server::new_async().await;
    let mock_ok = server
        .mock(
            "GET",
            "/Users/u1/Items?Recursive=true&Filters=IsPlayed&Limit=0",
        )
        .with_status(200)
        .with_body(r#"{"TotalRecordCount": 42}"#)
        .create_async()
        .await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let count = client.get_user_played_items_count("u1").await.unwrap();
    assert_eq!(count, 42);
    mock_ok.assert_async().await;
    // Test 500 error response
    let mock_500 = server
        .mock(
            "GET",
            "/Users/u1/Items?Recursive=true&Filters=IsPlayed&Limit=0",
        )
        .with_status(500)
        .create_async()
        .await;
    unsafe {
        std::env::set_var("STATESYNC_HTTP_RETRY", "off");
    }
    let res_500 = client.get_user_played_items_count("u1").await;
    assert!(res_500.is_err());
    mock_500.assert_async().await;
    unsafe {
        std::env::remove_var("STATESYNC_HTTP_RETRY");
    }
}

#[tokio::test]
async fn test_update_favorite_posts_is_favorite_only() {
    let _guard = match super::TEST_LOCK.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/Users/u1/Items/item1/UserData")
        .match_body(mockito::Matcher::PartialJsonString(
            r#"{"IsFavorite":true}"#.to_string(),
        ))
        .with_status(200)
        .with_body("{}")
        .create_async()
        .await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    client.update_favorite("u1", "item1", true).await.unwrap();
    mock.assert_async().await;
}

#[test]
fn test_userdata_entry_deserializes_favorite() {
    let v: crate::client::UserDataEntry = serde_json::from_value(serde_json::json!({
        "ItemId": "abc",
        "Played": false,
        "IsFavorite": true
    }))
    .unwrap();
    assert_eq!(v.item_id, "abc");
    assert_eq!(v.is_favorite, Some(true));
    assert!(!v.played);
}

#[tokio::test]
async fn test_get_item_user_data() {
    let _guard = match super::TEST_LOCK.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/Users/u1/Items/item1/UserData")
        .with_status(200)
        .with_body(r#"{"Played":true,"PlaybackPositionTicks":1000,"IsFavorite":true}"#)
        .create_async()
        .await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let ud = client.get_item_user_data("u1", "item1").await.unwrap();
    assert!(ud.played);
    assert_eq!(ud.playback_position_ticks, Some(1000));
    assert_eq!(ud.is_favorite, Some(true));
    mock.assert_async().await;
}
