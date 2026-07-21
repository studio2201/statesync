use crate::config::helpers::redacted_url;
use crate::config::validation::is_loopback_bind;

#[test]
fn test_redacted_url_strips_path_and_query() {
    assert_eq!(redacted_url("http://192.168.1.1:8096/foo"), "http://192.168.1.1:8096/...");
    assert_eq!(redacted_url("https://emby.example.com/"), "https://emby.example.com");
    assert_eq!(redacted_url("https://emby.example.com"), "https://emby.example.com");
    assert_eq!(redacted_url("not-a-url"), "not-a-url");
}

#[test]
fn test_is_loopback_bind() {
    assert!(is_loopback_bind("127.0.0.1:4601"));
    assert!(is_loopback_bind("localhost:4601"));
    assert!(is_loopback_bind("[::1]:4601"));
    assert!(is_loopback_bind("::1"));
    assert!(!is_loopback_bind("0.0.0.0:4601"));
    assert!(!is_loopback_bind("192.168.1.10:4601"));
    assert!(!is_loopback_bind("::1:4601"));
}

#[test]
fn test_redacted_url_various_schemes() {
    assert_eq!(redacted_url("http://user:pass@host:port/path"), "http://user:pass@host:port/...");
    assert_eq!(redacted_url("https://my-host.com"), "https://my-host.com");
    assert_eq!(redacted_url("http://127.0.0.1"), "http://127.0.0.1");
}

#[test]
fn test_is_loopback_bind_edge_cases() {
    assert!(is_loopback_bind("127.0.0.1"));
    assert!(is_loopback_bind("localhost"));
    assert!(!is_loopback_bind("127.0.0.2:80"));
    assert!(is_loopback_bind("[::1]"));
}
