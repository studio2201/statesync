use super::validation::validate_config;
use super::{Config, MAX_CONFIG_BYTES, ServerConfig};
use anyhow::{Context, Result, anyhow};
use std::env;

impl Config {
    pub fn save(&self) -> Result<()> {
        let path = get_config_path();
        let serialized = serde_json::to_string_pretty(self)?;
        let tmp = format!("{}.tmp", path);
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&tmp)
                .with_context(|| format!("Failed to create temporary config file at {}", tmp))?;
            f.write_all(serialized.as_bytes())?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, path)
            .with_context(|| format!("Failed to install config at {}", path))?;
        Ok(())
    }

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
                        .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
                        .unwrap_or(true),
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
                        .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
                        .unwrap_or(true),
                });
                servers.push(ServerConfig {
                    name: "Jellyfin".to_string(),
                    url: j_url,
                    api_key: j_key,
                    is_emby: false,
                    sync_direction: "both".to_string(),
                    allow_insecure_http: env::var("STATESYNC_ALLOW_INSECURE_HTTP")
                        .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
                        .unwrap_or(true),
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
            super::validation::validate_server(s)
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
            last_full_sync: None,
            sync: super::SyncOptions::default(),
        })
    }
}

pub const DEFAULT_BIND_FOR_BANNER: &str = "127.0.0.1:4601";

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
        sync_threshold_seconds: 5,
        user_mappings: Vec::new(),
        last_full_sync: None,
        sync: super::SyncOptions::default(),
    }
}

pub fn write_default_config_to_disk() -> Result<Config> {
    let config = default_config();
    let serialized = serde_json::to_string_pretty(&config)?;
    let primary = get_config_path();

    match atomic_write(primary, serialized.as_bytes()) {
        Ok(()) => {
            eprintln!(
                "No configuration found. Wrote a default config to {} with no servers. \
                 Add servers via the web UI or by editing this file.",
                primary
            );
            Ok(config)
        }
        Err(primary_err) if primary.starts_with("/config/") => {
            eprintln!(
                "WARN: could not write default config to {}: {}. \
                 /config is likely read-only or owned by another user \
                 (host bind-mount ownership mismatch).",
                primary, primary_err
            );
            let fallback = "/app/config.json";
            match atomic_write(fallback, serialized.as_bytes()) {
                Ok(()) => {
                    eprintln!(
                        "WARN: falling back to {} (in-container; not persisted across restart). \
                         Check the /config volume's permissions.",
                        fallback
                    );
                    Ok(config)
                }
                Err(fb_err) => {
                    eprintln!(
                        "WARN: fallback to {} also failed ({}). \
                         Starting with in-memory default config; changes via the web UI will NOT persist.",
                        fallback, fb_err
                    );
                    Ok(config)
                }
            }
        }
        Err(other) => Err(other),
    }
}

fn atomic_write(path: &str, bytes: &[u8]) -> anyhow::Result<()> {
    use std::io::Write;
    let tmp = format!("{}.tmp", path);
    {
        let mut f = std::fs::File::create(&tmp)
            .with_context(|| format!("Failed to create temporary config file at {}", tmp))?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
        .with_context(|| format!("Failed to install default config at {}", path))?;
    Ok(())
}

pub fn load_or_create_default() -> Result<Config> {
    match Config::load() {
        Ok(c) => Ok(c),
        Err(_) => write_default_config_to_disk(),
    }
}
