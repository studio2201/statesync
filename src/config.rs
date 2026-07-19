use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::env;

pub const MAX_NAME_LEN: usize = 64;
pub const MAX_URL_LEN: usize = 512;
pub const MAX_KEY_LEN: usize = 256;
pub const MAX_MAPPING_GROUPS: usize = 128;
pub const MAX_GROUP_MEMBERS: usize = 32;
pub const MAX_MEMBER_LEN: usize = 64;
pub const MAX_CONFIG_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub name: String,
    pub url: String,
    pub api_key: String,
    pub is_emby: bool,
    #[serde(default = "default_sync_direction")]
    pub sync_direction: String, // "both", "send", "receive"
    #[serde(default)]
    pub allow_insecure_http: bool,
}

fn default_sync_direction() -> String {
    "both".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub servers: Vec<ServerConfig>,
    #[serde(default = "default_threshold_seconds")]
    pub sync_threshold_seconds: u64,
    #[serde(default)]
    pub user_mappings: Vec<Vec<String>>,
}

fn default_threshold_seconds() -> u64 {
    5
}

fn validate_server(s: &ServerConfig) -> Result<()> {
    if s.name.is_empty() || s.name.len() > MAX_NAME_LEN {
        return Err(anyhow!(
            "server name must be 1..={} chars (got {})",
            MAX_NAME_LEN,
            s.name.len()
        ));
    }
    if s.url.len() > MAX_URL_LEN || !(s.url.starts_with("http://") || s.url.starts_with("https://"))
    {
        return Err(anyhow!(
            "server '{}': url must start with http:// or https:// and be <={} chars",
            s.name,
            MAX_URL_LEN
        ));
    }
    if s.url.starts_with("http://") && !s.allow_insecure_http {
        return Err(anyhow!(
            "server '{}': http:// url rejected (set allow_insecure_http: true to override)",
            s.name
        ));
    }
    if s.api_key.len() > MAX_KEY_LEN {
        return Err(anyhow!(
            "server '{}': api_key too long ({} > {})",
            s.name,
            s.api_key.len(),
            MAX_KEY_LEN
        ));
    }
    match s.sync_direction.as_str() {
        "both" | "send" | "receive" => {}
        _ => {
            return Err(anyhow!(
                "server '{}': sync_direction must be one of both|send|receive",
                s.name
            ));
        }
    }
    Ok(())
}

pub fn validate_config(cfg: &Config) -> Result<()> {
    if cfg.servers.len() > 20 {
        return Err(anyhow!(
            "too many servers configured ({} > 20)",
            cfg.servers.len()
        ));
    }
    for s in &cfg.servers {
        validate_server(s)?;
    }
    if cfg.user_mappings.len() > MAX_MAPPING_GROUPS {
        return Err(anyhow!(
            "too many user_mapping groups ({} > {})",
            cfg.user_mappings.len(),
            MAX_MAPPING_GROUPS
        ));
    }
    for group in &cfg.user_mappings {
        if group.len() > MAX_GROUP_MEMBERS {
            return Err(anyhow!(
                "user_mapping group has too many members ({} > {})",
                group.len(),
                MAX_GROUP_MEMBERS
            ));
        }
        for name in group {
            if name.is_empty() || name.len() > MAX_MEMBER_LEN {
                return Err(anyhow!(
                    "user_mapping member name must be 1..={} chars",
                    MAX_MEMBER_LEN
                ));
            }
        }
    }
    Ok(())
}

pub fn is_loopback_bind(addr: &str) -> bool {
    let host = if let Some(rest) = addr.strip_prefix('[') {
        match rest.find(']') {
            Some(end) => &rest[..end],
            None => return false,
        }
    } else if addr.matches(':').count() > 1 {
        return addr == "::1";
    } else {
        match addr.rsplit_once(':') {
            Some((h, _)) => h,
            None => addr,
        }
    };
    matches!(host, "127.0.0.1" | "::1" | "localhost")
}

pub fn redacted_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    if let Some(idx) = trimmed.find("://") {
        let rest = &trimmed[idx + 3..];
        if let Some(slash) = rest.find('/') {
            return format!("{}://{}/...", &trimmed[..idx], &rest[..slash]);
        }
        return format!("{}://{}", &trimmed[..idx], rest);
    }
    trimmed.to_string()
}

impl Config {
    pub fn load() -> Result<Self> {
        let mut servers = Vec::new();

        // 1. Check for flat environment variables: STATESYNC_SERVER_0_URL, etc. (for Unraid form inputs)
        for i in 0..20 {
            let url_var = format!("STATESYNC_SERVER_{}_URL", i);
            if let Ok(url) = env::var(&url_var) {
                if url.trim().is_empty() {
                    continue;
                }
                let name_var = format!("STATESYNC_SERVER_{}_NAME", i);
                let key_var = format!("STATESYNC_SERVER_{}_API_KEY", i);
                let type_var = format!("STATESYNC_SERVER_{}_TYPE", i);
                let dir_var = format!("STATESYNC_SERVER_{}_DIRECTION", i);

                let name = env::var(&name_var).unwrap_or_else(|_| format!("Server {}", i));
                let api_key = env::var(&key_var).with_context(|| {
                    format!("Missing API key environment variable: {}", key_var)
                })?;

                let is_emby = env::var(&type_var)
                    .map(|val| val.to_lowercase() == "emby")
                    .unwrap_or(false);

                let sync_direction = env::var(&dir_var).unwrap_or_else(|_| "both".to_string());

                servers.push(ServerConfig {
                    name,
                    url,
                    api_key,
                    is_emby,
                    sync_direction,
                    allow_insecure_http: env::var(format!("STATESYNC_SERVER_{}_INSECURE", i))
                        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                        .unwrap_or(false),
                });
            }
        }

        // 2. Fallback to standard two-server environment variables
        if servers.is_empty() {
            let emby_url = env::var("STATESYNC_EMBY_URL").ok();
            let emby_key = env::var("STATESYNC_EMBY_API_KEY").ok();
            let jf_url = env::var("STATESYNC_JELLYFIN_URL").ok();
            let jf_key = env::var("STATESYNC_JELLYFIN_API_KEY").ok();

            if let (Some(e_url), Some(e_key), Some(j_url), Some(j_key)) =
                (emby_url, emby_key, jf_url, jf_key)
            {
                servers.push(ServerConfig {
                    name: "Emby".to_string(),
                    url: e_url,
                    api_key: e_key,
                    is_emby: true,
                    sync_direction: "both".to_string(),
                    allow_insecure_http: env::var("STATESYNC_ALLOW_INSECURE_HTTP")
                        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                        .unwrap_or(false),
                });
                servers.push(ServerConfig {
                    name: "Jellyfin".to_string(),
                    url: j_url,
                    api_key: j_key,
                    is_emby: false,
                    sync_direction: "both".to_string(),
                    allow_insecure_http: env::var("STATESYNC_ALLOW_INSECURE_HTTP")
                        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                        .unwrap_or(false),
                });
            }
        }

        // 3. Fallback to config.json
        if servers.is_empty() {
            let paths = [
                get_config_path(),
                "/etc/statesync/config.json",
                "/app/config.json",
                "config.json",
            ];
            for path in &paths {
                if std::path::Path::new(path).exists() {
                    let data = std::fs::read_to_string(path)
                        .with_context(|| "Failed to read configuration file")?;
                    if data.len() > MAX_CONFIG_BYTES {
                        return Err(anyhow!(
                            "configuration file too large ({} > {} bytes)",
                            data.len(),
                            MAX_CONFIG_BYTES
                        ));
                    }
                    let config: Config = serde_json::from_str(&data)
                        .context("Failed to parse configuration file")?;
                    validate_config(&config).context("Invalid configuration")?;
                    return Ok(config);
                }
            }
        }

        if servers.is_empty() {
            return Err(anyhow!(
                "Configuration not found. Please provide environment variables (e.g. STATESYNC_SERVER_0_URL and STATESYNC_SERVER_0_API_KEY) or a config.json file."
            ));
        }

        for s in &servers {
            validate_server(s)
                .with_context(|| format!("Invalid env-supplied config for '{}'", s.name))?;
        }

        let threshold = env::var("STATESYNC_SYNC_THRESHOLD_SECONDS")
            .ok()
            .and_then(|val| val.parse::<u64>().ok())
            .unwrap_or(5);

        Ok(Self {
            servers,
            sync_threshold_seconds: threshold,
            user_mappings: Vec::new(),
        })
    }
}

pub const DEFAULT_BIND_FOR_BANNER: &str = "127.0.0.1:8754";

pub fn get_config_path() -> &'static str {
    if std::path::Path::new("/config").exists() {
        "/config/config.json"
    } else if std::path::Path::new("/etc/statesync").exists() {
        "/etc/statesync/config.json"
    } else if std::path::Path::new("/app").exists() {
        "/app/config.json"
    } else {
        "config.json"
    }
}

pub fn default_config() -> Config {
    Config {
        servers: Vec::new(),
        sync_threshold_seconds: default_threshold_seconds(),
        user_mappings: Vec::new(),
    }
}

pub fn write_default_config_to_disk() -> Result<Config> {
    use std::io::Write;
    let config = default_config();
    let path = get_config_path();
    let serialized = serde_json::to_string_pretty(&config)?;
    let tmp = format!("{}.tmp", path);
    {
        let mut f = std::fs::File::create(&tmp)
            .with_context(|| format!("Failed to create temporary config file at {}", tmp))?;
        f.write_all(serialized.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
        .with_context(|| format!("Failed to install default config at {}", path))?;
    Ok(config)
}

pub fn load_or_create_default() -> Result<Config> {
    match Config::load() {
        Ok(c) => Ok(c),
        Err(_) => {
            let path = get_config_path();
            let cfg = write_default_config_to_disk()?;
            eprintln!(
                "No configuration found. Wrote a default config to {} with no servers. \
                 Add servers via the web UI or by editing this file.",
                path
            );
            Ok(cfg)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_rejects_http_without_allow_flag() {
        let json = r#"{
            "servers": [
                {"name":"s","url":"http://x:8096","api_key":"k","is_emby":true}
            ]
        }"#;
        let cfg: Config = serde_json::from_str(json).unwrap();
        assert!(validate_config(&cfg).is_err());
    }

    #[test]
    fn test_accepts_http_with_allow_flag() {
        let json = r#"{
            "servers": [
                {"name":"s","url":"http://x:8096","api_key":"k","is_emby":true,"allow_insecure_http":true}
            ]
        }"#;
        let cfg: Config = serde_json::from_str(json).unwrap();
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
        assert!(is_loopback_bind("127.0.0.1:8754"));
        assert!(is_loopback_bind("localhost:8754"));
        assert!(is_loopback_bind("[::1]:8754"));
        assert!(is_loopback_bind("::1"));
        assert!(!is_loopback_bind("0.0.0.0:8754"));
        assert!(!is_loopback_bind("192.168.1.10:8754"));
        assert!(!is_loopback_bind("::1:8754"));
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
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let serialized =
            serde_json::to_string_pretty(&default_config()).unwrap();
        std::fs::write(&path, &serialized).unwrap();
        let data = std::fs::read_to_string(&path).unwrap();
        let c: Config = serde_json::from_str(&data).unwrap();
        assert!(c.servers.is_empty());
        assert!(validate_config(&c).is_ok());
    }
}
