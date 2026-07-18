use std::collections::HashMap;
use serde::Deserialize;
use reqwest::Client;
use anyhow::{Result, Context, anyhow};

#[derive(Debug, Clone, Deserialize)]
pub struct WsMessage {
    #[serde(alias = "messageType", alias = "MessageType")]
    pub message_type: String,
    #[serde(alias = "data", alias = "Data")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserDataChangedInfo {
    #[serde(alias = "userId", alias = "UserId")]
    pub user_id: String,
    #[serde(alias = "userDataList", alias = "UserDataList")]
    pub user_data_list: Vec<UserDataEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserDataEntry {
    #[serde(alias = "itemId", alias = "ItemId")]
    pub item_id: String,
    #[serde(alias = "played", alias = "Played")]
    pub played: bool,
    #[serde(alias = "playbackPositionTicks", alias = "PlaybackPositionTicks")]
    pub playback_position_ticks: Option<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct SessionInfo {
    #[serde(alias = "id", alias = "Id")]
    pub id: String,
    #[serde(alias = "userName", alias = "UserName")]
    pub user_name: Option<String>,
    #[serde(alias = "nowPlayingItem", alias = "NowPlayingItem")]
    pub now_playing_item: Option<NowPlayingItem>,
    #[serde(alias = "playState", alias = "PlayState")]
    pub play_state: Option<PlayState>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct NowPlayingItem {
    #[serde(alias = "id", alias = "Id")]
    pub id: String,
    #[serde(alias = "name", alias = "Name")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PlayState {
    #[serde(alias = "positionTicks", alias = "PositionTicks")]
    pub position_ticks: Option<i64>,
    #[serde(alias = "isPaused", alias = "IsPaused")]
    pub is_paused: Option<bool>,
}

pub struct MediaClient {
    client: Client,
    url: String,
    api_key: String,
    is_emby: bool,
}

impl MediaClient {
    pub fn new(url: String, api_key: String, is_emby: bool) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .connect_timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            client,
            url: url.trim_end_matches('/').to_string(),
            api_key,
            is_emby,
        }
    }

    fn url_path(&self, path: &str) -> String {
        let prefix = if self.is_emby { "/emby" } else { "" };
        format!("{}{}{}", self.url, prefix, path)
    }

    fn auth_url(&self, path: &str) -> String {
        let separator = if path.contains('?') { '&' } else { '?' };
        format!("{}{}api_key={}", self.url_path(path), separator, self.api_key)
    }

    pub async fn get_users(&self) -> Result<HashMap<String, String>> {
        let url = self.auth_url("/Users");
        let resp = self.client.get(&url)
            .header("X-Emby-Token", &self.api_key)
            .send()
            .await
            .context("Failed to get users list")?;
        
        let data: serde_json::Value = resp.json()
            .await
            .context("Failed to parse users response")?;
        
        let mut map = HashMap::new();
        if let Some(arr) = data.as_array() {
            for u in arr {
                if let (Some(name), Some(id)) = (u.get("Name").and_then(|n| n.as_str()), u.get("Id").and_then(|id| id.as_str())) {
                    map.insert(name.to_lowercase(), id.to_string());
                }
            }
        }
        Ok(map)
    }

    pub async fn get_library_items(&self, user_id: &str) -> Result<HashMap<String, (String, String)>> {
        let path = format!("/Users/{}/Items?Recursive=true&Fields=ProviderIds&IncludeItemTypes=Movie,Episode", user_id);
        let url = self.auth_url(&path);
        let resp = self.client.get(&url)
            .header("X-Emby-Token", &self.api_key)
            .send()
            .await
            .context("Failed to get library items")?;
        
        let data: serde_json::Value = resp.json()
            .await
            .context("Failed to parse library response")?;
        
        let mut map = HashMap::new();
        if let Some(arr) = data.get("Items").and_then(|i| i.as_array()) {
            for item in arr {
                if let Some(id) = item.get("Id").and_then(|id| id.as_str()) {
                    let mut imdb = String::new();
                    let mut tmdb = String::new();
                    if let Some(providers) = item.get("ProviderIds") {
                        if let Some(val) = providers.get("Imdb").and_then(|v| v.as_str()) { imdb = val.to_string(); }
                        if let Some(val) = providers.get("Tmdb").and_then(|v| v.as_str()) { tmdb = val.to_string(); }
                    }
                    map.insert(id.to_string(), (imdb, tmdb));
                }
            }
        }
        Ok(map)
    }

    pub async fn update_progress(&self, user_id: &str, item_id: &str, position_ticks: i64, played: bool) -> Result<()> {
        let path = format!("/Users/{}/Items/{}/UserData", user_id, item_id);
        let url = self.auth_url(&path);
        let body = serde_json::json!({
            "PlaybackPositionTicks": position_ticks,
            "PlayCount": if played { 1 } else { 0 },
            "IsFavorite": false,
            "Played": played,
        });
        let resp = self.client.post(&url)
            .header("X-Emby-Token", &self.api_key)
            .json(&body)
            .send()
            .await
            .context("Failed to send UserData progress update request")?;
        
        if !resp.status().is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("UserData progress update failed: {} - {}", url, body_text));
        }
        Ok(())
    }
}
