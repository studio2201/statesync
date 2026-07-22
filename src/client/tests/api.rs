use super::TEST_LOCK;
use crate::client::MediaClient;
#[tokio::test]
async fn test_get_public_server_info() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut server = mockito::Server::new_async().await;
    let mock_ok = server
        .mock("GET", "/System/Info/Public")
        .with_status(200)
        .with_body(r#"{"Version": "1.0", "ServerName": "TestServer"}"#)
        .create_async()
        .await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let res = client.get_public_server_info().await.unwrap();
    assert_eq!(res.get("Version").unwrap().as_str().unwrap(), "1.0");
    mock_ok.assert_async().await;
    let mock_err = server
        .mock("GET", "/System/Info/Public")
        .with_status(500)
        .create_async()
        .await;
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
    // Both path variants fail
    let mock_err = server
        .mock("GET", "/Users?StartIndex=0&Limit=500")
        .with_status(500)
        .create_async()
        .await;
    let mock_err_emby = server
        .mock("GET", "/emby/Users?StartIndex=0&Limit=500")
        .with_status(500)
        .create_async()
        .await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let res = client.get_users().await;
    assert!(res.is_err());
    mock_err.assert_async().await;
    mock_err_emby.assert_async().await;

    // Emby prefers /emby/Users first
    let mock_emby_ok = server
        .mock("GET", "/emby/Users?StartIndex=0&Limit=500")
        .with_status(200)
        .with_body(r#"{"Items":[{"Name":"Alice","Id":"u1"}],"TotalRecordCount":1}"#)
        .create_async()
        .await;
    let client_emby = MediaClient::new(server.url(), "key".to_string(), true);
    let users = client_emby.get_users().await.unwrap();
    assert_eq!(users.get("alice").map(|s| s.as_str()), Some("u1"));
    mock_emby_ok.assert_async().await;

    let mock_empty = server
        .mock("GET", "/Users?StartIndex=0&Limit=500")
        .with_status(200)
        .with_body(r#"{}"#)
        .create_async()
        .await;
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
    let mock_ok = server
        .mock("GET", "/Users/u1/Items/item1")
        .with_status(200)
        .with_body(r#"{"ProviderIds": {"Imdb": "tt123", "Tmdb": "tm456"}}"#)
        .create_async()
        .await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let p = client.get_item_providers("u1", "item1").await.unwrap();
    assert_eq!(p.imdb, "tt123");
    assert_eq!(p.tmdb, "tm456");
    mock_ok.assert_async().await;
    let mock_missing = server
        .mock("GET", "/Users/u1/Items/item1")
        .with_status(200)
        .with_body(r#"{"ProviderIds": {}}"#)
        .create_async()
        .await;
    let p2 = client.get_item_providers("u1", "item1").await.unwrap();
    assert!(p2.is_empty());
    mock_missing.assert_async().await;
}
#[tokio::test]
async fn test_get_item_name() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut server = mockito::Server::new_async().await;
    let mock_ok = server
        .mock("GET", "/Users/u1/Items/item1")
        .with_status(200)
        .with_body(r#"{"Name": "Good Movie"}"#)
        .create_async()
        .await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let name = client.get_item_name("u1", "item1").await.unwrap();
    assert_eq!(name, "Good Movie");
    mock_ok.assert_async().await;
}
#[tokio::test]
async fn test_get_item_providers_lowercase_keys() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut server = mockito::Server::new_async().await;
    let mock_ok = server
        .mock("GET", "/Users/u1/Items/item1")
        .with_status(200)
        .with_body(r#"{"ProviderIds": {"imdb": "tt12345", "tmdb": "tm6789"}}"#)
        .create_async()
        .await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let p = client.get_item_providers("u1", "item1").await.unwrap();
    assert_eq!(p.imdb, "tt12345");
    assert_eq!(p.tmdb, "tm6789");
    mock_ok.assert_async().await;
}
#[tokio::test]
async fn test_get_item_name_missing() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut server = mockito::Server::new_async().await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let mock_missing = server
        .mock("GET", "/Users/u1/Items/item1")
        .with_status(200)
        .with_body(r#"{}"#)
        .create_async()
        .await;
    let name_missing = client.get_item_name("u1", "item1").await.unwrap();
    assert_eq!(name_missing, "Unknown Item");
    mock_missing.assert_async().await;
}
#[tokio::test]
async fn test_find_item_by_provider() {
    use crate::client::ProviderIds;
    let _guard = TEST_LOCK.lock().unwrap();
    let mut server = mockito::Server::new_async().await;
    let mock_imdb = server.mock("GET", "/Users/u1/Items?Recursive=true&Fields=ProviderIds&AnyProviderIdTypes=Imdb&ProviderIds=tt123")
        .with_status(200)
        .with_body(r#"{"Items": [{"Id": "item_123", "ProviderIds": {"Imdb": "tt123"}}]}"#)
        .create_async().await;
    let client = MediaClient::new(server.url(), "key".to_string(), false);
    let res = client
        .find_item_by_provider("u1", &ProviderIds::from_parts("tt123", "", ""))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(res.0, "item_123");
    assert_eq!(res.1.imdb, "tt123");
    mock_imdb.assert_async().await;
    // TMDb lookup
    let mock_tmdb = server.mock("GET", "/Users/u1/Items?Recursive=true&Fields=ProviderIds&AnyProviderIdTypes=Tmdb&ProviderIds=tm456")
        .with_status(200)
        .with_body(r#"{"Items": [{"Id": "item_456", "ProviderIds": {"Tmdb": "tm456"}}]}"#)
        .create_async().await;
    let res_tmdb = client
        .find_item_by_provider("u1", &ProviderIds::from_parts("", "tm456", ""))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(res_tmdb.0, "item_456");
    assert_eq!(res_tmdb.1.tmdb, "tm456");
    mock_tmdb.assert_async().await;
    // TVDB lookup
    let mock_tvdb = server.mock("GET", "/Users/u1/Items?Recursive=true&Fields=ProviderIds&AnyProviderIdTypes=Tvdb&ProviderIds=73244")
        .with_status(200)
        .with_body(r#"{"Items": [{"Id": "item_tv", "ProviderIds": {"Tvdb": "73244"}}]}"#)
        .create_async().await;
    let res_tvdb = client
        .find_item_by_provider("u1", &ProviderIds::from_parts("", "", "73244"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(res_tvdb.0, "item_tv");
    assert_eq!(res_tvdb.1.tvdb, "73244");
    mock_tvdb.assert_async().await;
    // Empty providers lookup
    let res_empty = client
        .find_item_by_provider("u1", &ProviderIds::default())
        .await
        .unwrap();
    assert!(res_empty.is_none());
}
