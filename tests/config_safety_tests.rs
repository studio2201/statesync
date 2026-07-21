//! Bind-address and URL redaction safety helpers (config).

use statesync::config::{is_loopback_bind, redacted_url};

#[test]
fn test_loopback_ipv4() {
    assert!(is_loopback_bind("127.0.0.1:4601"));
}
#[test]
fn test_loopback_ipv4_no_port() {
    assert!(is_loopback_bind("127.0.0.1"));
}
#[test]
fn test_loopback_localhost() {
    assert!(is_loopback_bind("localhost:4601"));
}
#[test]
fn test_loopback_localhost_no_port() {
    assert!(is_loopback_bind("localhost"));
}
#[test]
fn test_loopback_ipv6() {
    assert!(is_loopback_bind("[::1]:4601"));
}
#[test]
fn test_loopback_ipv6_raw() {
    assert!(is_loopback_bind("::1"));
}
#[test]
fn test_loopback_external_ipv4() {
    assert!(!is_loopback_bind("192.168.1.1:4601"));
}
#[test]
fn test_loopback_external_ipv4_no_port() {
    assert!(!is_loopback_bind("8.8.8.8"));
}
#[test]
fn test_loopback_any_bind() {
    assert!(!is_loopback_bind("0.0.0.0:4601"));
}
#[test]
fn test_loopback_any_bind_ipv6() {
    assert!(!is_loopback_bind("[::]:4601"));
}
#[test]
fn test_loopback_invalid_ipv6_brackets() {
    assert!(!is_loopback_bind("[::1"));
}
#[test]
fn test_loopback_strange_hostname() {
    assert!(!is_loopback_bind("local-host"));
}
#[test]
fn test_loopback_ipv6_loopback() {
    assert!(!is_loopback_bind("[0:0:0:0:0:0:0:1]"));
}
#[test]
fn test_loopback_external_host() {
    assert!(!is_loopback_bind("google.com"));
}

#[test]
fn test_redact_url_normal_http() {
    assert_eq!(
        redacted_url("http://127.0.0.1:8096/path"),
        "http://127.0.0.1:8096/..."
    );
}
#[test]
fn test_redact_url_normal_https() {
    assert_eq!(
        redacted_url("https://media.com/foo/bar"),
        "https://media.com/..."
    );
}
#[test]
fn test_redact_url_no_path() {
    assert_eq!(redacted_url("http://localhost"), "http://localhost");
}
#[test]
fn test_redact_url_trailing_slash() {
    assert_eq!(redacted_url("http://localhost/"), "http://localhost");
}
#[test]
fn test_redact_url_no_scheme() {
    assert_eq!(redacted_url("localhost:8096/path"), "localhost:8096/path");
}
#[test]
fn test_redact_url_query() {
    assert_eq!(redacted_url("http://host/path?q=1"), "http://host/...");
}
#[test]
fn test_redact_url_port_slash() {
    assert_eq!(redacted_url("http://host:80/"), "http://host:80");
}
#[test]
fn test_redact_url_auth() {
    assert_eq!(
        redacted_url("http://user:pass@host/path"),
        "http://user:pass@host/..."
    );
}
#[test]
fn test_redact_url_ip_port() {
    assert_eq!(
        redacted_url("http://192.168.1.50:8096"),
        "http://192.168.1.50:8096"
    );
}
#[test]
fn test_redact_url_subdomain() {
    assert_eq!(
        redacted_url("https://sub.domain.com/path"),
        "https://sub.domain.com/..."
    );
}
#[test]
fn test_redact_url_empty() {
    assert_eq!(redacted_url(""), "");
}
#[test]
fn test_redact_url_spaces() {
    assert_eq!(redacted_url("  http://host/path  "), "  http://host/...");
}
#[test]
fn test_redact_url_query_only() {
    assert_eq!(redacted_url("http://host?"), "http://host?");
}
#[test]
fn test_redact_url_fragment() {
    assert_eq!(redacted_url("http://host#frag"), "http://host#frag");
}
