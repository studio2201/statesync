#[cfg(test)]
mod tests {
    use crate::config::{Config, default_config, validate_config};
    use crate::config::helpers::redacted_url;
    use crate::config::validation::is_loopback_bind;

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
    fn test_rejects_http_when_explicitly_disallowed() {
        let json = r#"{
            "servers": [
                {"name":"s","url":"http://x:8096","api_key":"k","is_emby":true,"allow_insecure_http":false}
            ]
        }"#;
        let cfg: Config = serde_json::from_str(json).unwrap();
        assert!(validate_config(&cfg).is_err());
    }

    #[test]
    fn test_accepts_http_by_default() {
        let json = r#"{
            "servers": [
                {"name":"s","url":"http://x:8096","api_key":"k","is_emby":true}
            ]
        }"#;
        let cfg: Config = serde_json::from_str(json).unwrap();
        assert!(cfg.servers[0].allow_insecure_http);
        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    fn test_accepts_https_by_default() {
        let json = r#"{
            "servers": [
                {"name":"s","url":"https://x:8096","api_key":"k","is_emby":true}
            ]
        }"#;
        let cfg: Config = serde_json::from_str(json).unwrap();
        assert!(cfg.servers[0].allow_insecure_http);
        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    fn test_redacted_url_strips_path_and_query() {
        assert_eq!(
            redacted_url("http://192.168.1.1:8096/foo"),
            "http://192.168.1.1:8096/..."
        );
        assert_eq!(
            redacted_url("https://emby.example.com/"),
            "https://emby.example.com"
        );
        assert_eq!(
            redacted_url("https://emby.example.com"),
            "https://emby.example.com"
        );
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
        assert!(validate_config(&c).is_ok());
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
        assert!(read.user_mappings.is_empty());
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
    fn test_invalid_schemes_rejected() {
        use crate::config::ServerConfig;
        
        let mut cfg = default_config();
        cfg.servers.push(ServerConfig {
            name: "bad_server".to_string(),
            url: "ftp://127.0.0.1:8096".to_string(),
            api_key: "key".to_string(),
            is_emby: true,
            sync_direction: "both".to_string(),
            allow_insecure_http: true,
        });
        assert!(validate_config(&cfg).is_err(), "ftp scheme should be rejected");

        cfg.servers[0].url = "ws://127.0.0.1:8096".to_string();
        assert!(validate_config(&cfg).is_err(), "ws scheme should be rejected");

        cfg.servers[0].url = "127.0.0.1:8096".to_string();
        assert!(validate_config(&cfg).is_err(), "no scheme should be rejected");
    }

    #[test]
    fn test_excessive_lengths_rejected() {
        use crate::config::ServerConfig;

        let mut cfg = default_config();
        
        // Name too long (> 64 chars)
        cfg.servers.push(ServerConfig {
            name: "a".repeat(65),
            url: "http://127.0.0.1:8096".to_string(),
            api_key: "key".to_string(),
            is_emby: true,
            sync_direction: "both".to_string(),
            allow_insecure_http: true,
        });
        assert!(validate_config(&cfg).is_err(), "overly long name should be rejected");

        // URL too long (> 512 chars)
        cfg.servers[0].name = "ok_name".to_string();
        cfg.servers[0].url = format!("http://{}", "a".repeat(510));
        assert!(validate_config(&cfg).is_err(), "overly long url should be rejected");

        // API Key too long (> 256 chars)
        cfg.servers[0].url = "http://127.0.0.1:8096".to_string();
        cfg.servers[0].api_key = "a".repeat(257);
        assert!(validate_config(&cfg).is_err(), "overly long api key should be rejected");
    }
}
