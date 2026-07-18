use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub url: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub emby: ServerConfig,
    pub jellyfin: ServerConfig,
    #[serde(default = "default_threshold_seconds")]
    pub sync_threshold_seconds: u64,
}

fn default_threshold_seconds() -> u64 {
    5
}
