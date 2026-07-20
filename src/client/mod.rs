use reqwest::Client;
use std::time::Duration;

pub mod types;
pub mod request;
pub mod api;
pub mod played;

pub use types::{WsMessage, UserDataChangedInfo, UserDataEntry, SessionInfo, NowPlayingItem, PlayState, PlayedItem};

pub struct MediaClient {
    pub client: Client,
    pub url: String,
    pub api_key: String,
    pub is_emby: bool,
}

impl MediaClient {
    pub fn new(url: String, api_key: String, is_emby: bool) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .pool_idle_timeout(Duration::from_secs(60))
            .tcp_keepalive(Duration::from_secs(60))
            .tcp_nodelay(true)
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            client,
            url: url.trim_end_matches('/').to_string(),
            api_key,
            is_emby,
        }
    }
}
