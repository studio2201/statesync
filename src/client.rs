use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Response};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

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

#[derive(Debug, Clone, Deserialize)]
pub struct UserItem {
    #[serde(alias = "id", alias = "Id")]
    pub id: String,
    #[serde(default, alias = "Played", alias = "played")]
    pub played: bool,
    #[serde(
        default,
        alias = "PlaybackPositionTicks",
        alias = "playbackPositionTicks"
    )]
    pub playback_position_ticks: Option<i64>,
    #[serde(default, alias = "LastPlayedDate", alias = "lastPlayedDate")]
    pub last_played_date: Option<String>,
    #[serde(default)]
    pub imdb_id: Option<String>,
    #[serde(default)]
    pub tmdb_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserDataSnapshot {
    #[serde(default, alias = "Played", alias = "played")]
    pub played: bool,
    #[serde(
        default,
        alias = "PlaybackPositionTicks",
        alias = "playbackPositionTicks"
    )]
    pub playback_position_ticks: Option<i64>,
    #[serde(default, alias = "LastPlayedDate", alias = "lastPlayedDate")]
    pub last_played_date: Option<String>,
}

pub struct MediaClient {
    pub client: Client,
    pub url: String,
    pub api_key: String,
    pub is_emby: bool,
}

fn retry_enabled() -> bool {
    std::env::var("STATESYNC_HTTP_RETRY")
        .map(|v| !v.eq_ignore_ascii_case("off"))
        .unwrap_or(true)
}

async fn send_with_retry(req: reqwest::RequestBuilder, label: &str) -> Result<Response> {
    let enabled = retry_enabled();
    let mut last_err: Option<anyhow::Error> = None;
    let mut backoff_ms = 500u64;
    let max_attempts = if enabled { 3 } else { 1 };
    for attempt in 0..max_attempts {
        let attempt_req = req
            .try_clone()
            .ok_or_else(|| anyhow!("request body not cloneable for retry"))?;
        match attempt_req.send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    return Ok(resp);
                }
                if !enabled
                    || status == reqwest::StatusCode::UNAUTHORIZED
                    || status == reqwest::StatusCode::FORBIDDEN
                {
                    return Err(anyhow!("{}: HTTP {}", label, status));
                }
                if status.is_server_error() {
                    last_err = Some(anyhow!("{}: HTTP {}", label, status));
                } else {
                    return Err(anyhow!("{}: HTTP {}", label, status));
                }
            }
            Err(e) => {
                last_err = Some(anyhow!("{}: {}", label, e));
            }
        }
        if attempt + 1 < max_attempts {
            tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
            backoff_ms = (backoff_ms * 2).min(8000);
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("{} failed", label)))
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

    pub fn url_path(&self, path: &str) -> String {
        let prefix = if self.is_emby { "/emby" } else { "" };
        format!("{}{}{}", self.url, prefix, path)
    }

    pub fn add_auth_headers(
        &self,
        mut builder: reqwest::RequestBuilder,
    ) -> reqwest::RequestBuilder {
        if self.is_emby {
            builder = builder.header("X-Emby-Token", &self.api_key);
        } else {
            builder = builder.header("X-MediaBrowser-Token", &self.api_key);
        }
        builder
    }

    pub async fn get_users(&self) -> Result<HashMap<String, String>> {
        let url = self.url_path("/Users");
        let resp = self
            .add_auth_headers(self.client.get(&url))
            .send()
            .await
            .context("Failed to get users list")?;

        let data: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse users response")?;

        let mut map = HashMap::new();
        if let Some(arr) = data.as_array() {
            for u in arr {
                if let (Some(name), Some(id)) = (
                    u.get("Name").and_then(|n| n.as_str()),
                    u.get("Id").and_then(|id| id.as_str()),
                ) {
                    map.insert(name.to_lowercase(), id.to_string());
                }
            }
        }
        Ok(map)
    }

    pub async fn get_library_items(&self) -> Result<HashMap<String, (String, String)>> {
        let mut all_items: HashMap<String, (String, String)> = HashMap::new();
        let mut start_index: usize = 0;
        let page_size: usize = 500;
        loop {
            let path = format!(
                "/Items?Recursive=true&Fields=ProviderIds&IncludeItemTypes=Movie,Episode&StartIndex={}&Limit={}",
                start_index, page_size
            );
            let url = self.url_path(&path);
            let resp = send_with_retry(
                self.add_auth_headers(self.client.get(&url)),
                "get_library_items",
            )
            .await
            .with_context(|| format!("Failed to get library items (StartIndex={})", start_index))?;

            let data: serde_json::Value = resp
                .json()
                .await
                .context("Failed to parse library response")?;

            let items = data.get("Items").and_then(|i| i.as_array());
            let total = data
                .get("TotalRecordCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let page_count = items.map(|a| a.len()).unwrap_or(0);
            if let Some(arr) = items {
                for item in arr {
                    if let Some(id) = item.get("Id").and_then(|id| id.as_str()) {
                        let mut imdb = String::new();
                        let mut tmdb = String::new();
                        if let Some(providers) = item.get("ProviderIds") {
                            if let Some(val) = providers.get("Imdb").and_then(|v| v.as_str()) {
                                imdb = val.to_string();
                            }
                            if let Some(val) = providers.get("Tmdb").and_then(|v| v.as_str()) {
                                tmdb = val.to_string();
                            }
                        }
                        all_items.insert(id.to_string(), (imdb, tmdb));
                    }
                }
            }
            if page_count < page_size || all_items.len() >= total {
                break;
            }
            start_index += page_size;
            if start_index > 100_000 {
                break;
            }
        }
        Ok(all_items)
    }

    pub async fn get_item_providers(
        &self,
        user_id: &str,
        item_id: &str,
    ) -> Result<(String, String)> {
        let path = format!("/Users/{}/Items/{}", user_id, item_id);
        let url = self.url_path(&path);
        let resp = self
            .add_auth_headers(self.client.get(&url))
            .send()
            .await
            .context("Failed to get item details")?;

        let data: serde_json::Value = resp.json().await.context("Failed to parse item response")?;

        let mut imdb = String::new();
        let mut tmdb = String::new();
        if let Some(providers) = data.get("ProviderIds") {
            if let Some(val) = providers.get("Imdb").and_then(|v| v.as_str()) {
                imdb = val.to_string();
            }
            if let Some(val) = providers.get("Tmdb").and_then(|v| v.as_str()) {
                tmdb = val.to_string();
            }
        }
        Ok((imdb, tmdb))
    }

    pub async fn find_item_by_provider(
        &self,
        user_id: &str,
        imdb_id: &str,
        tmdb_id: &str,
    ) -> Result<Option<(String, String, String)>> {
        let mut path = format!("/Users/{}/Items?Recursive=true&Fields=ProviderIds", user_id);
        if !imdb_id.is_empty() {
            path.push_str(&format!(
                "&AnyProviderIdTypes=Imdb&ProviderIds={}",
                percent_encoding::utf8_percent_encode(imdb_id, percent_encoding::NON_ALPHANUMERIC)
            ));
        } else if !tmdb_id.is_empty() {
            path.push_str(&format!(
                "&AnyProviderIdTypes=Tmdb&ProviderIds={}",
                percent_encoding::utf8_percent_encode(tmdb_id, percent_encoding::NON_ALPHANUMERIC)
            ));
        } else {
            return Ok(None);
        }

        let url = self.url_path(&path);
        let resp = self
            .add_auth_headers(self.client.get(&url))
            .send()
            .await
            .context("Failed to search item by provider ID")?;

        let data: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse search response")?;

        if let Some(items) = data.get("Items").and_then(|i| i.as_array()) {
            if let Some(item) = items.first() {
                if let Some(id) = item.get("Id").and_then(|id| id.as_str()) {
                    let mut imdb = String::new();
                    let mut tmdb = String::new();
                    if let Some(providers) = item.get("ProviderIds") {
                        if let Some(val) = providers.get("Imdb").and_then(|v| v.as_str()) {
                            imdb = val.to_string();
                        }
                        if let Some(val) = providers.get("Tmdb").and_then(|v| v.as_str()) {
                            tmdb = val.to_string();
                        }
                    }
                    return Ok(Some((id.to_string(), imdb, tmdb)));
                }
            }
        }
        Ok(None)
    }

    pub async fn update_progress(
        &self,
        user_id: &str,
        item_id: &str,
        position_ticks: i64,
        played: bool,
    ) -> Result<()> {
        let path = format!("/Users/{}/Items/{}/UserData", user_id, item_id);
        let url = self.url_path(&path);
        let body = serde_json::json!({
            "PlaybackPositionTicks": position_ticks,
            "Played": played,
        });

        let resp = self
            .add_auth_headers(self.client.post(&url))
            .json(&body)
            .send()
            .await
            .context("Failed to send UserData progress update request")?;

        if !resp.status().is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            return Err(anyhow!(
                "UserData progress update failed: {} - {}",
                url,
                body_text
            ));
        }
        Ok(())
    }

    pub async fn get_user_played_items(
        &self,
        user_id: &str,
        filter: &str,
        start_index: usize,
        limit: usize,
    ) -> Result<Vec<UserItem>> {
        let path = if self.is_emby {
            format!(
                "/Users/{}/Items?Recursive=true&Fields=ProviderIds&{}&StartIndex={}&Limit={}",
                user_id, filter, start_index, limit
            )
        } else {
            format!(
                "/Users/{}/Items?Recursive=true&Fields=ProviderIds&Filters={}&StartIndex={}&Limit={}",
                user_id, filter, start_index, limit
            )
        };
        let url = self.url_path(&path);
        let resp = send_with_retry(
            self.add_auth_headers(self.client.get(&url)),
            "get_user_played_items",
        )
        .await
        .with_context(|| {
            format!(
                "Failed to list user items (user={}, filter={}, page={})",
                user_id, filter, start_index
            )
        })?;
        let data: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse user items response")?;
        let arr = data
            .get("Items")
            .and_then(|i| i.as_array())
            .cloned()
            .unwrap_or_default();
        let mut out = Vec::with_capacity(arr.len());
        for mut v in arr {
            let imdb = v
                .get("ProviderIds")
                .and_then(|p| p.get("Imdb"))
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            let tmdb = v
                .get("ProviderIds")
                .and_then(|p| p.get("Tmdb"))
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            if let Some(imdb) = imdb {
                if let Some(map) = v.as_object_mut() {
                    map.insert("imdb_id".to_string(), serde_json::Value::String(imdb));
                }
            }
            if let Some(tmdb) = tmdb {
                if let Some(map) = v.as_object_mut() {
                    map.insert("tmdb_id".to_string(), serde_json::Value::String(tmdb));
                }
            }
            match serde_json::from_value::<UserItem>(v) {
                Ok(item) => out.push(item),
                Err(_) => continue,
            }
        }
        if self.is_emby {
            self.attach_emby_provider_ids(user_id, &mut out).await;
        }
        Ok(out)
    }

    async fn attach_emby_provider_ids(&self, user_id: &str, items: &mut [UserItem]) {
        for item in items.iter_mut() {
            if item.imdb_id.is_some() || item.tmdb_id.is_some() {
                continue;
            }
            if let Ok((imdb, tmdb)) = self.get_item_providers(user_id, &item.id).await {
                if !imdb.is_empty() {
                    item.imdb_id = Some(imdb);
                }
                if !tmdb.is_empty() {
                    item.tmdb_id = Some(tmdb);
                }
            }
        }
    }

    pub async fn get_item_userdata(
        &self,
        user_id: &str,
        item_id: &str,
    ) -> Result<UserDataSnapshot> {
        let path = format!("/Users/{}/Items/{}/UserData", user_id, item_id);
        let url = self.url_path(&path);
        let resp = send_with_retry(
            self.add_auth_headers(self.client.get(&url)),
            "get_item_userdata",
        )
        .await
        .with_context(|| format!("Failed to get item userdata for {}", item_id))?;
        if !resp.status().is_success() {
            return Err(anyhow!("get_item_userdata failed: {}", resp.status()));
        }
        let snap: UserDataSnapshot = resp
            .json()
            .await
            .context("Failed to parse item userdata response")?;
        Ok(snap)
    }
}
