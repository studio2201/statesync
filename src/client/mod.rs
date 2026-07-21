use reqwest::Client;
use std::time::Duration;

/// Missing documentation.
pub mod api;
pub mod api_items;
/// Missing documentation.
pub mod played;
pub mod played_lists;
/// Missing documentation.
pub mod request;
/// Missing documentation.
pub mod types;

#[cfg(test)]
mod tests;

pub use types::{
    NowPlayingItem, PlayState, PlayedItem, SessionInfo, UserDataChangedInfo, UserDataEntry,
    WsMessage,
};

/// Missing documentation.
pub struct MediaClient {
    /// Missing documentation.
    pub client: Client,
    /// Missing documentation.
    pub url: String,
    /// Missing documentation.
    pub api_key: String,
    /// Missing documentation.
    pub is_emby: bool,
}

/// When true, TLS certificate verification is disabled for upstream
/// Emby/Jellyfin HTTPS. Off by default; set `STATESYNC_ACCEPT_INVALID_CERTS=true`
/// only for self-signed LAN certs you intentionally trust.
pub fn accept_invalid_certs_enabled() -> bool {
    std::env::var("STATESYNC_ACCEPT_INVALID_CERTS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
        .unwrap_or(false)
}

impl MediaClient {
    /// Missing documentation.
    pub fn new(url: String, api_key: String, is_emby: bool) -> Self {
        // Always reduce pasted browser URLs to scheme://host:port.
        let clean_url = crate::config::normalize_server_url(&url);
        let clean_api_key = api_key.trim().to_string();
        let accept_invalid = accept_invalid_certs_enabled();
        if accept_invalid {
            tracing::warn!(
                "STATESYNC_ACCEPT_INVALID_CERTS is enabled; TLS certificate verification is disabled for upstream servers"
            );
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .pool_idle_timeout(Duration::from_secs(60))
            .tcp_keepalive(Duration::from_secs(60))
            .tcp_nodelay(true)
            .danger_accept_invalid_certs(accept_invalid)
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            client,
            url: clean_url,
            api_key: clean_api_key,
            is_emby,
        }
    }
}
