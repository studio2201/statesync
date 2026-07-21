use crate::config::{Config, default_config, validate_config, ServerConfig};

fn valid_server(name: &str) -> ServerConfig {
    ServerConfig {
        name: name.to_string(),
        url: "https://s0".to_string(),
        api_key: "key".to_string(),
        is_emby: true,
        sync_direction: "both".to_string(),
        allow_insecure_http: true,
    }
}

#[test]
fn test_rejects_http_when_explicitly_disallowed() {
    let json = r#"{"servers":[{"name":"s","url":"http://x:8096","api_key":"k","is_emby":true,"allow_insecure_http":false}]}"#;
    let cfg: Config = serde_json::from_str(json).unwrap();
    assert!(validate_config(&cfg).is_err());
}

#[test]
fn test_accepts_http_by_default() {
    let json = r#"{"servers":[{"name":"s","url":"http://x:8096","api_key":"k","is_emby":true}]}"#;
    let cfg: Config = serde_json::from_str(json).unwrap();
    assert!(cfg.servers[0].allow_insecure_http);
    assert!(validate_config(&cfg).is_ok());
}

#[test]
fn test_accepts_https_by_default() {
    let json = r#"{"servers":[{"name":"s","url":"https://x:8096","api_key":"k","is_emby":true}]}"#;
    let cfg: Config = serde_json::from_str(json).unwrap();
    assert!(validate_config(&cfg).is_ok());
}

#[test]
fn test_invalid_schemes_rejected() {
    let mut cfg = default_config();
    cfg.servers.push(valid_server("bad"));
    cfg.servers[0].url = "ftp://127.0.0.1:8096".to_string();
    assert!(validate_config(&cfg).is_err());
    cfg.servers[0].url = "ws://127.0.0.1:8096".to_string();
    assert!(validate_config(&cfg).is_err());
    cfg.servers[0].url = "127.0.0.1:8096".to_string();
    assert!(validate_config(&cfg).is_err());
}

#[test]
fn test_excessive_lengths_rejected() {
    let mut cfg = default_config();
    cfg.servers.push(valid_server(&"a".repeat(65)));
    assert!(validate_config(&cfg).is_err());
    cfg.servers[0].name = "ok_name".to_string();
    cfg.servers[0].url = format!("http://{}", "a".repeat(510));
    assert!(validate_config(&cfg).is_err());
    cfg.servers[0].url = "http://127.0.0.1:8096".to_string();
    cfg.servers[0].api_key = "a".repeat(257);
    assert!(validate_config(&cfg).is_err());
}

#[test]
fn test_config_default_helpers() {
    use crate::config::{default_allow_insecure_http, default_sync_direction, default_threshold_seconds};
    assert!(default_allow_insecure_http());
    assert_eq!(default_sync_direction(), "both");
    assert_eq!(default_threshold_seconds(), 5);
}

#[test]
fn test_validate_server_name_empty() {
    let mut cfg = default_config();
    cfg.servers.push(valid_server(""));
    assert!(validate_config(&cfg).is_err());
}

#[test]
fn test_validate_server_sync_direction_invalid() {
    let mut cfg = default_config();
    cfg.servers.push(valid_server("test"));
    cfg.servers[0].sync_direction = "invalid_dir".to_string();
    assert!(validate_config(&cfg).is_err());
}

#[test]
fn test_validate_config_too_many_servers() {
    let mut cfg = default_config();
    for i in 0..21 {
        cfg.servers.push(valid_server(&format!("server{}", i)));
    }
    assert!(validate_config(&cfg).is_err());
}

#[test]
fn test_validate_config_user_mappings_limits() {
    let mut cfg = default_config();
    cfg.user_mappings = vec![vec!["a".to_string(), "b".to_string()]; 129];
    assert!(validate_config(&cfg).is_err());
    cfg.user_mappings = vec![vec!["user".to_string(); 33]];
    assert!(validate_config(&cfg).is_err());
    cfg.user_mappings = vec![vec!["".to_string(), "user2".to_string()]];
    assert!(validate_config(&cfg).is_err());
    cfg.user_mappings = vec![vec!["a".repeat(65), "user2".to_string()]];
    assert!(validate_config(&cfg).is_err());
}

#[test]
fn test_validate_server_name_length_boundaries() {
    let mut cfg = default_config();
    cfg.servers.push(valid_server(&"a".repeat(64)));
    assert!(validate_config(&cfg).is_ok());
}

#[test]
fn test_validate_server_url_length_boundaries() {
    let mut cfg = default_config();
    cfg.servers.push(valid_server("s"));
    cfg.servers[0].url = format!("https://{}", "a".repeat(504));
    assert!(validate_config(&cfg).is_ok());
    cfg.servers[0].url = format!("https://{}", "a".repeat(505));
    assert!(validate_config(&cfg).is_err());
}

#[test]
fn test_validate_server_key_length_boundaries() {
    let mut cfg = default_config();
    cfg.servers.push(valid_server("s"));
    cfg.servers[0].api_key = "k".repeat(256);
    assert!(validate_config(&cfg).is_ok());
}

#[test]
fn test_validate_config_servers_boundaries() {
    let mut cfg = default_config();
    assert!(validate_config(&cfg).is_ok());
    for i in 0..20 {
        cfg.servers.push(valid_server(&format!("s{}", i)));
    }
    assert!(validate_config(&cfg).is_ok());
}

#[test]
fn test_validate_config_user_mappings_boundaries() {
    let mut cfg = default_config();
    cfg.user_mappings = vec![vec!["a".to_string(), "b".to_string()]; 128];
    assert!(validate_config(&cfg).is_ok());
}

#[test]
fn test_validate_config_group_members_boundaries() {
    let mut cfg = default_config();
    cfg.user_mappings = vec![vec!["u".to_string(); 32]];
    assert!(validate_config(&cfg).is_ok());
}

#[test]
fn test_validate_config_member_len_boundaries() {
    let mut cfg = default_config();
    cfg.user_mappings = vec![vec!["a".repeat(64), "b".to_string()]];
    assert!(validate_config(&cfg).is_ok());
}

#[test]
fn test_validate_server_sync_direction_uppercase() {
    let mut cfg = default_config();
    cfg.servers.push(valid_server("test"));
    cfg.servers[0].sync_direction = "BOTH".to_string();
    assert!(validate_config(&cfg).is_err());
}

#[test]
fn test_validate_server_url_no_host() {
    let mut cfg = default_config();
    cfg.servers.push(valid_server("test"));
    cfg.servers[0].url = "https:///".to_string();
    assert!(validate_config(&cfg).is_ok());
}

#[test]
fn test_validate_server_url_whitespace() {
    let mut cfg = default_config();
    cfg.servers.push(valid_server("test"));
    cfg.servers[0].url = "https://s0 ".to_string();
    assert!(validate_config(&cfg).is_ok());
}

#[test]
fn test_validate_config_empty_servers() {
    let cfg = default_config();
    assert!(validate_config(&cfg).is_ok());
}

#[test]
fn test_validate_server_sync_direction_send_receive() {
    let mut cfg = default_config();
    cfg.servers.push(valid_server("s1"));
    cfg.servers[0].sync_direction = "send".to_string();
    assert!(validate_config(&cfg).is_ok());
    cfg.servers[0].sync_direction = "receive".to_string();
    assert!(validate_config(&cfg).is_ok());
}
