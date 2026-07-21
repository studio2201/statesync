use crate::config::helpers::{name_from_url, normalize_server_url, redacted_url};
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

#[test]
fn test_normalize_server_url_strips_any_pasted_web_ui() {
    // Full Emby/Jellyfin browser address (path + hash) → origin only
    assert_eq!(
        normalize_server_url("http://10.0.0.5:8096/web/index.html#!/apikeys"),
        "http://10.0.0.5:8096"
    );
    assert_eq!(
        normalize_server_url("https://media.example.com:8920/web/index.html#/dashboard"),
        "https://media.example.com:8920"
    );
    assert_eq!(
        normalize_server_url("http://emby.lan:8096/web/index.html?foo=1#/home"),
        "http://emby.lan:8096"
    );
    // Bare host:port
    assert_eq!(
        normalize_server_url("10.0.0.5:8096"),
        "http://10.0.0.5:8096"
    );
    // Trailing slash / path only
    assert_eq!(
        normalize_server_url("http://10.0.0.5:8096/"),
        "http://10.0.0.5:8096"
    );
    assert_eq!(
        normalize_server_url("http://10.0.0.5:8096/emby"),
        "http://10.0.0.5:8096"
    );
    // Auto names keep the port so same host / different ports stay unique.
    assert_eq!(
        name_from_url("http://media.example.com:8096/web/"),
        "media.example.com:8096"
    );
    assert_eq!(name_from_url("http://10.0.0.5:8096"), "10.0.0.5:8096");
    assert_eq!(name_from_url("http://10.0.0.5:8920"), "10.0.0.5:8920");
    assert_eq!(name_from_url("10.0.0.5:8096"), "10.0.0.5:8096");
    // No explicit port → host only
    assert_eq!(name_from_url("http://emby.lan/web/"), "emby.lan");
    assert_eq!(name_from_url("http://[fe80::1]:8096/"), "[fe80::1]:8096");
}

#[test]
fn test_sync_options_default_power_law() {
    let s = crate::config::SyncOptions::default();
    assert!(s.live_played && s.live_position && s.live_favorites);
    assert!(s.force_played && s.force_position && s.force_favorites);
}

#[test]
fn test_sync_options_missing_fields_deserialize() {
    let cfg: crate::config::Config = serde_json::from_str(r#"{"servers":[]}"#).unwrap();
    assert!(cfg.sync.live_favorites);
    // Old configs may still contain force_unwatch; ignore unknown via serde default path
    let _old: crate::config::Config = serde_json::from_str(
        r#"{"servers":[],"sync":{"force_unwatch":true,"live_played":true}}"#,
    )
    .unwrap();
}
