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
        Self {
            client: Client::new(),
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
        let path = format!("/Users/{}/Items?IncludeItemTypes=Movie,Episode&Recursive=true&Fields=ProviderIds", user_id);
        let url = self.auth_url(&path);
        let resp = self.client.get(&url)
            .header("X-Emby-Token", &self.api_key)
            .send()
            .await
            .context("Failed to get library items")?;
        
        let data: serde_json::Value = resp.json()
            .await
            .context("Failed to parse library items")?;
        
        let mut map = HashMap::new();
        if let Some(items) = data.get("Items").and_then(|i| i.as_array()) {
            for item in items {
                if let Some(id) = item.get("Id").and_then(|id| id.as_str()) {
                    let mut imdb = String::new();
                    let mut tmdb = String::new();
                    if let Some(provider_ids) = item.get("ProviderIds") {
                        if let Some(val) = provider_ids.get("Imdb").and_then(|v| v.as_str()) {
                            imdb = val.to_string();
                        }
                        if let Some(val) = provider_ids.get("Tmdb").and_then(|v| v.as_str()) {
                            tmdb = val.to_string();
                        }
                    }
                    map.insert(id.to_string(), (imdb, tmdb));
                }
            }
        }
        Ok(map)
    }

    pub async fn update_progress(&self, user_id: &str, item_id: &str, position_ticks: i64, is_paused: bool) -> Result<()> {
        let path = format!("/Users/{}/PlayingItems/{}/Progress", user_id, item_id);
        let url = self.auth_url(&path);
        let body = serde_json::json!({
            "PositionTicks": position_ticks,
            "IsPaused": is_paused,
            "IsMuted": false,
        });
        let resp = self.client.post(&url)
            .header("X-Emby-Token", &self.api_key)
            .json(&body)
            .send()
            .await
            .context("Failed to send progress update request")?;
        
        if !resp.status().is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Progress update failed: {} - {}", url, body_text));
        }
        Ok(())
    }
}
