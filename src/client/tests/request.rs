use super::TEST_LOCK;
use crate::client::MediaClient;
use crate::client::request::retry_enabled;
use crate::client::request::send_with_retry;

#[test]
fn test_client_new() {
    let _guard = match TEST_LOCK.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let client_emby = MediaClient::new(
        "http://127.0.0.1:8096/".to_string(),
        "api_key".to_string(),
        true,
    );
    assert_eq!(client_emby.url, "http://127.0.0.1:8096");
    assert!(client_emby.is_emby);

    let client_jf = MediaClient::new(
        "https://127.0.0.1:8096".to_string(),
        "api_key".to_string(),
        false,
    );
    assert_eq!(client_jf.url, "https://127.0.0.1:8096");
    assert!(!client_jf.is_emby);

    let client_spaces = MediaClient::new(
        "  http://10.0.0.100:8096/  ".to_string(),
        "key".to_string(),
        false,
    );
    assert_eq!(client_spaces.url, "http://10.0.0.100:8096");

    // Query, path, and hash from a pasted browser URL are stripped to origin.
    let client_query = MediaClient::new(
        "http://example.com:8096/web/index.html?foo=bar#!/apikeys".to_string(),
        "key".to_string(),
        true,
    );
    assert_eq!(client_query.url, "http://example.com:8096");
}

#[test]
fn test_retry_enabled() {
    let _guard = match TEST_LOCK.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    unsafe {
        std::env::set_var("STATESYNC_HTTP_RETRY", "off");
    }
    assert!(!retry_enabled());

    unsafe {
        std::env::set_var("STATESYNC_HTTP_RETRY", "on");
    }
    assert!(retry_enabled());

    unsafe {
        std::env::remove_var("STATESYNC_HTTP_RETRY");
    }
    assert!(retry_enabled());
}

#[tokio::test]
async fn test_send_with_retry_success() {
    let _guard = match TEST_LOCK.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let mut server = mockito::Server::new_async().await;
    let mock_call = server
        .mock("GET", "/ok")
        .with_status(200)
        .create_async()
        .await;

    let client = reqwest::Client::new();
    let req = client.get(format!("{}/ok", server.url()));
    let res = send_with_retry(req, "test").await;
    assert!(res.is_ok());
    mock_call.assert_async().await;
}

#[tokio::test]
async fn test_send_with_retry_fails_eventually() {
    let _guard = match TEST_LOCK.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let mut server = mockito::Server::new_async().await;
    let mock_call = server
        .mock("GET", "/fail")
        .with_status(500)
        .expect(3)
        .create_async()
        .await;

    unsafe {
        std::env::remove_var("STATESYNC_HTTP_RETRY");
    }
    let client = reqwest::Client::new();
    let req = client.get(format!("{}/fail", server.url()));
    let res = send_with_retry(req, "test").await;
    assert!(res.is_err());
    mock_call.assert_async().await;
}

#[tokio::test]
async fn test_send_with_retry_unauthorized_fast_fail() {
    let _guard = match TEST_LOCK.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let mut server = mockito::Server::new_async().await;
    let mock_call = server
        .mock("GET", "/auth_fail")
        .with_status(401)
        .expect(1)
        .create_async()
        .await;

    unsafe {
        std::env::remove_var("STATESYNC_HTTP_RETRY");
    }
    let client = reqwest::Client::new();
    let req = client.get(format!("{}/auth_fail", server.url()));
    let res = send_with_retry(req, "test_auth").await;
    assert!(res.is_err());
    mock_call.assert_async().await;
}

#[test]
fn test_url_path() {
    let _guard = match TEST_LOCK.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let client_emby = MediaClient::new("http://localhost".to_string(), "k".to_string(), true);
    assert_eq!(client_emby.url_path("/Users"), "http://localhost/Users");

    // Path after host is stripped; /emby is tried by get_users separately.
    let client_emby_preset =
        MediaClient::new("http://localhost/emby".to_string(), "k".to_string(), true);
    assert_eq!(client_emby_preset.url, "http://localhost");
    assert_eq!(
        client_emby_preset.url_path("/Users"),
        "http://localhost/Users"
    );

    let client_jf = MediaClient::new("http://localhost".to_string(), "k".to_string(), false);
    assert_eq!(client_jf.url_path("/Users"), "http://localhost/Users");
}

#[test]
fn test_add_auth_headers() {
    let _guard = match TEST_LOCK.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let client_emby =
        MediaClient::new("http://localhost".to_string(), "emby_key".to_string(), true);
    let req = client_emby.client.get("http://localhost");
    let req = client_emby.add_auth_headers(req).build().unwrap();
    assert_eq!(req.headers().get("X-Emby-Token").unwrap(), "emby_key");
    assert_eq!(
        req.headers().get("X-MediaBrowser-Token").unwrap(),
        "emby_key"
    );
    assert!(
        req.headers()
            .get("Authorization")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("emby_key")
    );

    let client_jf = MediaClient::new("http://localhost".to_string(), "jf_key".to_string(), false);
    let req = client_jf.client.get("http://localhost");
    let req = client_jf.add_auth_headers(req).build().unwrap();
    assert_eq!(req.headers().get("X-MediaBrowser-Token").unwrap(), "jf_key");
    assert_eq!(req.headers().get("X-Emby-Token").unwrap(), "jf_key");
}
