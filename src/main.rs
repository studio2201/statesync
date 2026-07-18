use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessageProto;
use reqwest::Client;
use anyhow::{Result, Context, anyhow};
use tracing::{info, warn, error};
use tracing_subscriber;

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

#[derive(Debug, Clone, Deserialize)]
pub struct WsMessage {
    #[serde(alias = "messageType", alias = "MessageType")]
    pub message_type: String,
    #[serde(alias = "data", alias = "Data")]
    pub data: Option<serde_json::Value>,
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
        let path = format!("/Users/{}/PlayingItems/{}/Progress?positionTicks={}&isPaused={}", user_id, item_id, position_ticks, is_paused);
        let url = self.auth_url(&path);
        let resp = self.client.post(&url)
            .header("X-Emby-Token", &self.api_key)
            .send()
            .await
            .context("Failed to send progress update request")?;
        
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Progress update failed: {} - {}", url, body));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ServerCache {
    pub users: HashMap<String, String>, // username (lowercase) -> UserId
    pub imdb_to_id: HashMap<String, String>, // ImdbId -> ItemId
    pub tmdb_to_id: HashMap<String, String>, // TmdbId -> ItemId
    pub id_to_providers: HashMap<String, (String, String)>, // ItemId -> (ImdbId, TmdbId)
}

#[derive(Debug, Clone)]
pub struct SyncHistoryValue {
    pub position_ticks: i64,
    pub timestamp: Instant,
}

pub struct AppState {
    pub emby_cache: ServerCache,
    pub jellyfin_cache: ServerCache,
    // Map of (username, provider_id) -> SyncHistoryValue
    pub last_syncs: HashMap<(String, String), SyncHistoryValue>,
}

impl AppState {
    pub fn new(emby_cache: ServerCache, jellyfin_cache: ServerCache) -> Self {
        Self {
            emby_cache,
            jellyfin_cache,
            last_syncs: HashMap::new(),
        }
    }
}

fn make_ws_url(url: &str, api_key: &str, is_emby: bool) -> String {
    let base = url.trim_end_matches('/');
    let ws_base = if base.starts_with("https://") {
        base.replace("https://", "wss://")
    } else if base.starts_with("http://") {
        base.replace("http://", "ws://")
    } else {
        format!("ws://{}", base)
    };
    
    let path = if is_emby { "/embywebsocket" } else { "/socket" };
    format!("{}{}?api_key={}&deviceId=emby-jellyfin-bridge", ws_base, path, api_key)
}

async fn init_server_cache(client: &MediaClient) -> Result<ServerCache> {
    let users = client.get_users().await?;
    let first_user_id = users.values().next().ok_or_else(|| anyhow!("No users found on server"))?;
    let items = client.get_library_items(first_user_id).await?;
    
    let mut imdb_to_id = HashMap::new();
    let mut tmdb_to_id = HashMap::new();
    let mut id_to_providers = HashMap::new();
    
    for (id, (imdb, tmdb)) in items {
        if !imdb.is_empty() {
            imdb_to_id.insert(imdb.clone(), id.clone());
        }
        if !tmdb.is_empty() {
            tmdb_to_id.insert(tmdb.clone(), id.clone());
        }
        id_to_providers.insert(id, (imdb, tmdb));
    }
    
    Ok(ServerCache {
        users,
        imdb_to_id,
        tmdb_to_id,
        id_to_providers,
    })
}

async fn handle_websocket_loop(
    ws_url: &str,
    is_source_emby: bool,
    _client_source: Arc<MediaClient>,
    client_target: Arc<MediaClient>,
    state_lock: Arc<Mutex<AppState>>,
    config: Config,
) {
    loop {
        info!("Connecting to {} WebSocket: {}", if is_source_emby { "Emby" } else { "Jellyfin" }, ws_url);
        match connect_async(ws_url).await {
            Ok((mut ws_stream, _)) => {
                info!("{} WebSocket connected.", if is_source_emby { "Emby" } else { "Jellyfin" });

                let start_msg = json!({
                    "MessageType": "SessionsStart",
                    "Data": "0,1000"
                });
                if let Err(e) = ws_stream.send(WsMessageProto::Text(start_msg.to_string().into())).await {
                    error!("Failed to send subscribe message: {}", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }

                while let Some(msg) = ws_stream.next().await {
                    match msg {
                        Ok(WsMessageProto::Text(text)) => {
                            if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                                if ws_msg.message_type == "Sessions" {
                                    if let Some(ref data) = ws_msg.data {
                                        if let Ok(sessions) = serde_json::from_value::<Vec<SessionInfo>>(data.clone()) {
                                            let mut state = state_lock.lock().await;
                                            let now = Instant::now();
                                            
                                            for s in &sessions {
                                                if let (Some(user_name), Some(item), Some(play_state)) = (&s.user_name, &s.now_playing_item, &s.play_state) {
                                                    let user_lower = user_name.to_lowercase();
                                                    let position = play_state.position_ticks.unwrap_or(0);
                                                    let is_paused = play_state.is_paused.unwrap_or(false);

                                                    // Fetch source item's provider IDs (IMDb/TMDb)
                                                    let source_cache = if is_source_emby { &state.emby_cache } else { &state.jellyfin_cache };
                                                    if let Some((imdb_id, tmdb_id)) = source_cache.id_to_providers.get(&item.id) {
                                                        
                                                        // Determine a primary global provider ID for history mapping
                                                        let provider_id = if !imdb_id.is_empty() {
                                                            imdb_id.clone()
                                                        } else if !tmdb_id.is_empty() {
                                                            tmdb_id.clone()
                                                        } else {
                                                            continue; // No provider IDs, can't sync
                                                        };

                                                        // Check sync history to prevent loops
                                                        let history_key = (user_lower.clone(), provider_id.clone());
                                                        if let Some(history) = state.last_syncs.get(&history_key) {
                                                            let age = now - history.timestamp;
                                                            let pos_diff = (position - history.position_ticks).abs();
                                                            let threshold_ticks = (config.sync_threshold_seconds * 10_000_000) as i64;
                                                            
                                                            if age < Duration::from_secs(5) && pos_diff < threshold_ticks {
                                                                continue; // Skip loop echo
                                                            }
                                                        }

                                                        // Look up matching target item ID (isolated block to prevent immutable borrow lock of state)
                                                        let (target_item_id, target_user_id) = {
                                                            let target_cache = if is_source_emby { &state.jellyfin_cache } else { &state.emby_cache };
                                                            let mut t_item_id = None;
                                                            if !imdb_id.is_empty() {
                                                                t_item_id = target_cache.imdb_to_id.get(imdb_id).cloned();
                                                            }
                                                            if t_item_id.is_none() && !tmdb_id.is_empty() {
                                                                t_item_id = target_cache.tmdb_to_id.get(tmdb_id).cloned();
                                                            }
                                                            let t_user_id = target_cache.users.get(&user_lower).cloned();
                                                            (t_item_id, t_user_id)
                                                        };

                                                        if let (Some(t_item_id), Some(t_user_id)) = (target_item_id, target_user_id) {
                                                            // Update last sync state (safe to mutate now that immutable borrows are dropped)
                                                            state.last_syncs.insert(history_key, SyncHistoryValue {
                                                                position_ticks: position,
                                                                timestamp: now,
                                                            });

                                                            info!(
                                                                "Syncing playstate for user '{}' from {} -> {}: '{}' at {:.1}s (paused: {})",
                                                                user_name,
                                                                if is_source_emby { "Emby" } else { "Jellyfin" },
                                                                if is_source_emby { "Jellyfin" } else { "Emby" },
                                                                item.name.as_deref().unwrap_or(&item.id),
                                                                position as f64 / 10_000_000.0,
                                                                is_paused
                                                            );

                                                            let client_target_clone = client_target.clone();
                                                            
                                                            tokio::spawn(async move {
                                                                if let Err(e) = client_target_clone.update_progress(&t_user_id, &t_item_id, position, is_paused).await {
                                                                    error!("Error updating target playstate progress: {}", e);
                                                                }
                                                            });
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            error!("WebSocket stream error: {}", e);
                            break;
                        }
                    }
                }
                warn!("{} WebSocket disconnected. Reconnecting in 5 seconds...", if is_source_emby { "Emby" } else { "Jellyfin" });
            }
            Err(e) => {
                error!("Failed to connect to {} WebSocket: {}. Retrying in 5 seconds...", if is_source_emby { "Emby" } else { "Jellyfin" }, e);
            }
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting Emby-Jellyfin Playstate Sync Sidecar...");

    let config_path = "config.json";
    let config_data = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read configuration file: {}", config_path))?;
    
    let config: Config = serde_json::from_str(&config_data)
        .context("Failed to parse configuration file")?;

    info!("Connecting to Emby: {}", config.emby.url);
    let emby_client = Arc::new(MediaClient::new(config.emby.url.clone(), config.emby.api_key.clone(), true));
    
    info!("Connecting to Jellyfin: {}", config.jellyfin.url);
    let jellyfin_client = Arc::new(MediaClient::new(config.jellyfin.url.clone(), config.jellyfin.api_key.clone(), false));

    // Initialize Caches
    info!("Initializing Emby metadata cache...");
    let emby_cache = init_server_cache(&emby_client).await
        .context("Failed to initialize Emby metadata cache")?;
    info!("Emby cache loaded: {} users, {} matched media items.", emby_cache.users.len(), emby_cache.id_to_providers.len());

    info!("Initializing Jellyfin metadata cache...");
    let jellyfin_cache = init_server_cache(&jellyfin_client).await
        .context("Failed to initialize Jellyfin metadata cache")?;
    info!("Jellyfin cache loaded: {} users, {} matched media items.", jellyfin_cache.users.len(), jellyfin_cache.id_to_providers.len());

    let app_state = Arc::new(Mutex::new(AppState::new(emby_cache, jellyfin_cache)));

    let emby_ws_url = make_ws_url(&config.emby.url, &config.emby.api_key, true);
    let jellyfin_ws_url = make_ws_url(&config.jellyfin.url, &config.jellyfin.api_key, false);

    // Launch Emby WS listener
    let emby_ws_url_clone = emby_ws_url.clone();
    let emby_client_clone = emby_client.clone();
    let jellyfin_client_clone = jellyfin_client.clone();
    let state_clone = app_state.clone();
    let config_clone = config.clone();
    tokio::spawn(async move {
        handle_websocket_loop(
            &emby_ws_url_clone,
            true, // source is emby
            emby_client_clone,
            jellyfin_client_clone,
            state_clone,
            config_clone,
        ).await;
    });

    // Launch Jellyfin WS listener
    let jellyfin_ws_url_clone = jellyfin_ws_url.clone();
    let emby_client_clone2 = emby_client.clone();
    let jellyfin_client_clone2 = jellyfin_client.clone();
    let state_clone2 = app_state.clone();
    let config_clone2 = config.clone();
    tokio::spawn(async move {
        handle_websocket_loop(
            &jellyfin_ws_url_clone,
            false, // source is jellyfin
            jellyfin_client_clone2,
            emby_client_clone2,
            state_clone2,
            config_clone2,
        ).await;
    });

    info!("Playstate Sync Sidecar started. Press Ctrl+C to stop.");
    tokio::signal::ctrl_c().await?;
    info!("Stopping Playstate Sync Sidecar.");
    Ok(())
}
