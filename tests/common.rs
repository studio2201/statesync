//! Common test helpers: spin up a mockito server and a MediaClient
//! pointing at it, with helper methods to register canned responses.
//!
//! mockito 1.7's `Server::new_async()` returns a `ServerGuard` which
//! derefs to `Server`; the helpers below take `&ServerGuard` and
//! forward calls.

use mockito::Server;
use statesync::client::MediaClient;

pub struct MockServer {
    pub server: ServerGuard,
}

pub use mockito::ServerGuard;

pub async fn mock_server() -> MockServer {
    let server = Server::new_async().await;
    MockServer { server }
}

pub fn server_url(mock: &MockServer) -> String {
    mock.server.url()
}

pub fn media_client(mock: &MockServer, is_emby: bool) -> MediaClient {
    MediaClient::new(server_url(mock), "test-api-key".to_string(), is_emby)
}

pub fn mock_get_users(mock: &mut MockServer, users: Vec<serde_json::Value>) -> mockito::Mock {
    mock.server
        .mock("GET", mockito::Matcher::Regex(r"^/Users.*$".to_string()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&users).unwrap())
        .create()
}

pub fn mock_get_paginated_users(
    mock: &mut MockServer,
    pages: Vec<Vec<serde_json::Value>>,
) -> Vec<mockito::Mock> {
    let total: usize = pages.iter().map(|p| p.len()).sum();
    let mut mocks = Vec::new();
    for (i, page) in pages.iter().enumerate() {
        let body = serde_json::json!({
            "Items": page,
            "TotalRecordCount": total,
        });
        let m = mock
            .server
            .mock(
                "GET",
                format!("/Users?StartIndex={}&Limit=500", i * 500).as_str(),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body.to_string())
            .create();
        mocks.push(m);
    }
    mocks
}

pub fn mock_get_public_info(
    mock: &mut MockServer,
    server_name: &str,
    version: &str,
) -> mockito::Mock {
    mock.server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/System/Info/Public.*$".to_string()),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            serde_json::json!({
                "ServerName": server_name,
                "Version": version,
                "Id": "test-id",
            })
            .to_string(),
        )
        .create()
}

pub fn mock_get_user_played_items(
    mock: &mut MockServer,
    user_id: &str,
    pages: Vec<Vec<serde_json::Value>>,
) -> Vec<mockito::Mock> {
    let total: usize = pages.iter().map(|p| p.len()).sum();
    let mut mocks = Vec::new();
    for (i, page) in pages.iter().enumerate() {
        let body = serde_json::json!({
            "Items": page,
            "TotalRecordCount": total,
        });
        let m = mock
            .server
            .mock(
                "GET",
                format!(
                    "/Users/{}/Items?Recursive=true&Fields=ProviderIds&Filters=IsPlayed=true&StartIndex={}&Limit=500",
                    user_id, i * 500
                )
                .as_str(),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body.to_string())
            .create();
        mocks.push(m);
    }
    mocks
}

pub fn mock_update_progress(mock: &mut MockServer, user_id: &str, item_id: &str) -> mockito::Mock {
    mock.server
        .mock(
            "POST",
            format!("/Users/{}/Items/{}/UserData", user_id, item_id).as_str(),
        )
        .with_status(204)
        .create()
}

pub fn mock_find_item_by_provider(
    mock: &mut MockServer,
    user_id: &str,
    imdb: &str,
    item_id: &str,
) -> mockito::Mock {
    mock.server
        .mock(
            "GET",
            format!(
                "/Users/{}/Items?Recursive=true&Fields=ProviderIds&AnyProviderIdTypes=Imdb&ProviderIds={}",
                user_id, imdb
            )
            .as_str(),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            serde_json::json!({
                "Items": [{
                    "Id": item_id,
                    "ProviderIds": {"Imdb": imdb, "Tmdb": ""},
                }],
            })
            .to_string(),
        )
        .create()
}
