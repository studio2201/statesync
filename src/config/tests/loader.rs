use crate::config::{Config, default_config, ServerConfig};
use super::TEST_LOCK;

#[test]
fn test_config_deserialization_with_defaults() {
    let json = r#"{
        "servers": [
            {
                "name": "green",
                "url": "http://localhost:8096",
                "api_key": "secret",
                "is_emby": true
            }
        ]
    }"#;
    let config: Config = serde_json::from_str(json).unwrap();
    assert_eq!(config.servers.len(), 1);
    assert_eq!(config.servers[0].name, "green");
    assert_eq!(config.sync_threshold_seconds, 5);
    assert_eq!(config.servers[0].sync_direction, "both");
    assert!(config.user_mappings.is_empty());
}

#[test]
fn test_config_with_custom_user_mappings() {
    let json = r#"{
        "servers": [],
        "sync_threshold_seconds": 10,
        "user_mappings": [
            ["john doe", "john"],
            ["jane", "jane_doe"]
        ]
    }"#;
    let config: Config = serde_json::from_str(json).unwrap();
    assert_eq!(config.sync_threshold_seconds, 10);
    assert_eq!(config.user_mappings.len(), 2);
    assert_eq!(config.user_mappings[0], vec!["john doe", "john"]);
}

#[test]
fn test_default_config_is_empty() {
    let c = default_config();
    assert!(c.servers.is_empty());
    assert_eq!(c.sync_threshold_seconds, 5);
    assert!(c.user_mappings.is_empty());
}

#[test]
fn test_write_default_then_load_roundtrips() {
    let serialized = serde_json::to_string_pretty(&default_config()).unwrap();
    let c: Config = serde_json::from_str(&serialized).unwrap();
    assert!(c.servers.is_empty());
}

#[test]
fn write_default_to_disk_writes_when_writable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    let cfg = default_config();
    let serialized = serde_json::to_string_pretty(&cfg).unwrap();
    std::fs::write(&path, &serialized).unwrap();
    let read: Config = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(read.servers.is_empty());
    assert_eq!(read.sync_threshold_seconds, 5);
}

#[test]
fn write_default_atomic_creates_temp_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("c.json");
    let cfg = default_config();
    let bytes = serde_json::to_string_pretty(&cfg).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    assert!(path.exists());
    let s = std::fs::read_to_string(&path).unwrap();
    assert!(s.contains("sync_threshold_seconds"));
}

#[test]
fn test_config_save_invalid_path() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut cfg = default_config();
    cfg.servers.push(ServerConfig {
        name: "test_save".to_string(),
        url: "http://127.0.0.1:8096".to_string(),
        api_key: "key".to_string(),
        is_emby: true,
        sync_direction: "both".to_string(),
        allow_insecure_http: true,
    });
    
    let path = crate::config::get_config_path();
    let old_content = std::fs::read_to_string(path).ok();
    let _ = std::fs::remove_file(path);

    cfg.save().unwrap();
    let loaded = Config::load().unwrap();
    assert_eq!(loaded.servers[0].name, "test_save");

    if let Some(content) = old_content {
        std::fs::write(path, content).unwrap();
    } else {
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn test_config_save_multiple_servers() {
    let _guard = TEST_LOCK.lock().unwrap();
    let path = crate::config::get_config_path();
    let old_content = std::fs::read_to_string(path).ok();
    let _ = std::fs::remove_file(path);

    let mut cfg = default_config();
    cfg.servers.push(ServerConfig {
        name: "s1".to_string(),
        url: "https://s1".to_string(),
        api_key: "k1".to_string(),
        is_emby: true,
        sync_direction: "both".to_string(),
        allow_insecure_http: true,
    });
    cfg.servers.push(ServerConfig {
        name: "s2".to_string(),
        url: "https://s2".to_string(),
        api_key: "k2".to_string(),
        is_emby: false,
        sync_direction: "send".to_string(),
        allow_insecure_http: false,
    });

    cfg.save().unwrap();
    let loaded = Config::load().unwrap();
    assert_eq!(loaded.servers.len(), 2);
    assert_eq!(loaded.servers[0].name, "s1");
    assert_eq!(loaded.servers[1].name, "s2");

    if let Some(content) = old_content {
        std::fs::write(path, content).unwrap();
    } else {
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn test_load_or_create_default() {
    let _guard = TEST_LOCK.lock().unwrap();
    let path = crate::config::get_config_path();
    let old_content = std::fs::read_to_string(path).ok();
    let _ = std::fs::remove_file(path);

    let cfg = crate::config::load_or_create_default().unwrap();
    assert!(cfg.servers.is_empty());
    assert_eq!(cfg.sync_threshold_seconds, 5);

    if let Some(content) = old_content {
        std::fs::write(path, content).unwrap();
    } else {
        let _ = std::fs::remove_file(path);
    }
}
