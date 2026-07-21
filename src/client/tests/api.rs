use crate::client::MediaClient;
use super::TEST_LOCK;
#[tokio::test]
async fn test_get_public_server_info() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut server = mockito::Server::new_async().await;
    let mock_ok = server.mock("GET", "/System/Info/Public")
        .with_status(200)
        .with_body(r#"{"Version": "1.0", "ServerName": "TestServer"}"#)
        .create_async().await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let res = client.get_public_server_info().await.unwrap();
    assert_eq!(res.get("Version").unwrap().as_str().unwrap(), "1.0");
    mock_ok.assert_async().await;
    let mock_err = server.mock("GET", "/System/Info/Public")
        .with_status(500)
        .create_async().await;
    let res_err = client.get_public_server_info().await;
    assert!(res_err.is_err());
    mock_err.assert_async().await;
}
#[tokio::test]
async fn test_get_users_error() {
    let _guard = TEST_LOCK.lock().unwrap();
    unsafe {
        std::env::set_var("STATESYNC_HTTP_RETRY", "off");
    }
    let mut server = mockito::Server::new_async().await;
    let mock_err = server.mock("GET", "/Users?StartIndex=0&Limit=500")
        .with_status(500)
        .create_async().await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let res = client.get_users().await;
    assert!(res.is_err());
    mock_err.assert_async().await;
    let mock_404 = server.mock("GET", "/Users?StartIndex=0&Limit=500")
        .with_status(404)
        .create_async().await;
    let res_404 = client.get_users().await;
    assert!(res_404.is_err());
    mock_404.assert_async().await;
    let mock_empty = server.mock("GET", "/Users?StartIndex=0&Limit=500")
        .with_status(200)
        .with_body(r#"{}"#)
        .create_async().await;
    let users_empty = client.get_users().await.unwrap();
    assert_eq!(users_empty.len(), 0);
    mock_empty.assert_async().await;
    unsafe {
        std::env::remove_var("STATESYNC_HTTP_RETRY");
    }
}
#[tokio::test]
async fn test_get_library_items_error() {
    let _guard = TEST_LOCK.lock().unwrap();
    unsafe {
        std::env::set_var("STATESYNC_HTTP_RETRY", "off");
    }
    let mut server = mockito::Server::new_async().await;
    let mock_err = server.mock("GET", "/Items?Recursive=true&Fields=ProviderIds&IncludeItemTypes=Movie,Episode&StartIndex=0&Limit=500")
        .with_status(500)
        .create_async().await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let res = client.get_library_items().await;
    assert!(res.is_err());
    mock_err.assert_async().await;
    unsafe {
        std::env::remove_var("STATESYNC_HTTP_RETRY");
    }
}
#[tokio::test]
async fn test_get_item_providers() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut server = mockito::Server::new_async().await;
    let mock_ok = server.mock("GET", "/Users/u1/Items/item1")
        .with_status(200)
        .with_body(r#"{"ProviderIds": {"Imdb": "tt123", "Tmdb": "tm456"}}"#)
        .create_async().await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let (imdb, tmdb) = client.get_item_providers("u1", "item1").await.unwrap();
    assert_eq!(imdb, "tt123");
    assert_eq!(tmdb, "tm456");
    mock_ok.assert_async().await;
    let mock_missing = server.mock("GET", "/Users/u1/Items/item1")
        .with_status(200)
        .with_body(r#"{"ProviderIds": {}}"#)
        .create_async().await;
    let (imdb2, tmdb2) = client.get_item_providers("u1", "item1").await.unwrap();
    assert_eq!(imdb2, "");
    assert_eq!(tmdb2, "");
    mock_missing.assert_async().await;
}
#[tokio::test]
async fn test_get_item_name() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut server = mockito::Server::new_async().await;
    let mock_ok = server.mock("GET", "/Users/u1/Items/item1")
        .with_status(200)
        .with_body(r#"{"Name": "Good Movie"}"#)
        .create_async().await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let name = client.get_item_name("u1", "item1").await.unwrap();
    assert_eq!(name, "Good Movie");
    mock_ok.assert_async().await;
}
#[tokio::test]
async fn test_get_item_providers_lowercase_keys() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut server = mockito::Server::new_async().await;
    let mock_ok = server.mock("GET", "/Users/u1/Items/item1")
        .with_status(200)
        .with_body(r#"{"ProviderIds": {"imdb": "tt12345", "tmdb": "tm6789"}}"#)
        .create_async().await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let (imdb, tmdb) = client.get_item_providers("u1", "item1").await.unwrap();
    assert_eq!(imdb, "tt12345");
    assert_eq!(tmdb, "tm6789");
    mock_ok.assert_async().await;
}
#[tokio::test]
async fn test_get_item_name_missing() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut server = mockito::Server::new_async().await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let mock_missing = server.mock("GET", "/Users/u1/Items/item1")
        .with_status(200)
        .with_body(r#"{}"#)
        .create_async().await;
    let name_missing = client.get_item_name("u1", "item1").await.unwrap();
    assert_eq!(name_missing, "Unknown Item");
    mock_missing.assert_async().await;
}
#[tokio::test]
async fn test_find_item_by_provider() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut server = mockito::Server::new_async().await;
    let mock_imdb = server.mock("GET", "/Users/u1/Items?Recursive=true&Fields=ProviderIds&AnyProviderIdTypes=Imdb&ProviderIds=tt123")
        .with_status(200)
        .with_body(r#"{"Items": [{"Id": "item_123", "ProviderIds": {"Imdb": "tt123"}}]}"#)
        .create_async().await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let res = client.find_item_by_provider("u1", "tt123", "").await.unwrap().unwrap();
    assert_eq!(res.0, "item_123");
    assert_eq!(res.1, "tt123");
    mock_imdb.assert_async().await;
    // TMDb lookup
    let mock_tmdb = server.mock("GET", "/Users/u1/Items?Recursive=true&Fields=ProviderIds&AnyProviderIdTypes=Tmdb&ProviderIds=tm456")
        .with_status(200)
        .with_body(r#"{"Items": [{"Id": "item_456", "ProviderIds": {"Tmdb": "tm456"}}]}"#)
        .create_async().await;
    let res_tmdb = client.find_item_by_provider("u1", "", "tm456").await.unwrap().unwrap();
    assert_eq!(res_tmdb.0, "item_456");
    assert_eq!(res_tmdb.2, "tm456");
    mock_tmdb.assert_async().await;
    // Empty providers lookup
    let res_empty = client.find_item_by_provider("u1", "", "").await.unwrap();
    assert!(res_empty.is_none());
}
#[tokio::test]
async fn test_update_progress() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut server = mockito::Server::new_async().await;
    let mock_ok = server.mock("POST", "/Users/u1/Items/item1/UserData")
        .with_status(200)
        .create_async().await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let res = client.update_progress("u1", "item1", 1000, true).await;
    assert!(res.is_ok());
    mock_ok.assert_async().await;
    let mock_err = server.mock("POST", "/Users/u1/Items/item1/UserData")
        .with_status(500)
        .with_body("error message")
        .create_async().await;
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
    unsafe { std::env::set_var("STATESYNC_HTTP_RETRY", "off"); }
    let res_404 = client.get_user_played_items("u1", 0, 10).await;
    assert!(res_404.is_err());
    mock_404.assert_async().await;
    unsafe { std::env::remove_var("STATESYNC_HTTP_RETRY"); }
}
#[tokio::test]
async fn test_get_user_played_items_count() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut server = mockito::Server::new_async().await;
    let mock_ok = server.mock("GET", "/Users/u1/Items?Recursive=true&Filters=IsPlayed&Limit=0")
        .with_status(200)
        .with_body(r#"{"TotalRecordCount": 42}"#)
        .create_async().await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let count = client.get_user_played_items_count("u1").await.unwrap();
    assert_eq!(count, 42);
    mock_ok.assert_async().await;
    // Test 500 error response
    let mock_500 = server.mock("GET", "/Users/u1/Items?Recursive=true&Filters=IsPlayed&Limit=0")
        .with_status(500)
        .create_async().await;
    unsafe { std::env::set_var("STATESYNC_HTTP_RETRY", "off"); }
    let res_500 = client.get_user_played_items_count("u1").await;
    assert!(res_500.is_err());
    mock_500.assert_async().await;
    unsafe { std::env::remove_var("STATESYNC_HTTP_RETRY"); }
}
