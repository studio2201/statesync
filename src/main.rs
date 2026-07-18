use std::collections::{HashMap, HashSet};
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
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub emby_url: String,
    pub api_key: String,
    #[serde(default = "default_threshold_seconds")]
    pub sync_threshold_seconds: u64,
    #[serde(default = "default_cooldown_seconds")]
    pub cooldown_seconds: u64,
}

fn default_threshold_seconds() -> u64 {
    3
}

fn default_cooldown_seconds() -> u64 {
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
    
    #[serde(alias = "deviceId", alias = "DeviceId")]
    pub device_id: String,
    
    #[serde(alias = "deviceName", alias = "DeviceName")]
    pub device_name: Option<String>,
    
    #[serde(alias = "client", alias = "Client")]
    pub client: Option<String>,
    
    #[serde(alias = "userName", alias = "UserName")]
    pub user_name: Option<String>,
    
    #[serde(alias = "nowPlayingItem", alias = "NowPlayingItem")]
    pub now_playing_item: Option<NowPlayingItem>,
    
    #[serde(alias = "playState", alias = "PlayState")]
    pub play_state: Option<PlayState>,

    #[serde(alias = "playlistItemId", alias = "PlaylistItemId")]
    pub playlist_item_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct NowPlayingItem {
    #[serde(alias = "id", alias = "Id")]
    pub id: String,
    
    #[serde(alias = "name", alias = "Name")]
    pub name: Option<String>,
    
    #[serde(alias = "runTimeTicks", alias = "RunTimeTicks")]
    pub run_time_ticks: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PlayState {
    #[serde(alias = "positionTicks", alias = "PositionTicks")]
    pub position_ticks: Option<i64>,
    
    #[serde(alias = "isPaused", alias = "IsPaused")]
    pub is_paused: Option<bool>,
    
    #[serde(alias = "isMuted", alias = "IsMuted")]
    pub is_muted: Option<bool>,
    
    #[serde(alias = "volumeLevel", alias = "VolumeLevel")]
    pub volume_level: Option<i32>,

    #[serde(alias = "playlistItemId", alias = "PlaylistItemId")]
    pub playlist_item_id: Option<String>,
}

pub struct EmbyClient {
    client: Client,
    base_url: String,
    api_key: String,
}

impl EmbyClient {
    pub fn new(base_url: String, api_key: String) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        }
    }

    fn auth_url(&self, path: &str) -> String {
        format!("{}{}?api_key={}", self.base_url, path, self.api_key)
    }

    pub async fn get_sessions(&self) -> Result<Vec<SessionInfo>> {
        let url = self.auth_url("/emby/Sessions");
        let resp = self.client.get(&url)
            .header("X-Emby-Token", &self.api_key)
            .send()
            .await
            .context("Failed to send get_sessions request")?;
        
        let sessions = resp.json::<Vec<SessionInfo>>()
            .await
            .context("Failed to parse get_sessions response")?;
        Ok(sessions)
    }

    pub async fn get_all_users(&self) -> Result<Vec<String>> {
        let url = self.auth_url("/emby/Users");
        let resp = self.client.get(&url)
            .header("X-Emby-Token", &self.api_key)
            .send()
            .await
            .context("Failed to get users list")?;
        
        let users: serde_json::Value = resp.json()
            .await
            .context("Failed to parse users list")?;
        
        let mut user_ids = Vec::new();
        if let Some(arr) = users.as_array() {
            for u in arr {
                if let Some(id) = u.get("Id").and_then(|id| id.as_str()) {
                    user_ids.push(id.to_string());
                }
            }
        }
        Ok(user_ids)
    }

    pub async fn create_playlist(&self, user_id: &str, name: &str, item_id: &str) -> Result<String> {
        let path = "/emby/Playlists";
        let encoded_name = utf8_percent_encode(name, NON_ALPHANUMERIC).to_string();
        let url = format!(
            "{}{}?UserId={}&Name={}&Ids={}&api_key={}",
            self.base_url, path, user_id, encoded_name, item_id, self.api_key
        );
        let resp = self.client.post(&url)
            .header("X-Emby-Token", &self.api_key)
            .send()
            .await
            .context("Failed to send Create Playlist request")?;
        
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Create Playlist failed: {} - {}", url, body));
        }

        let data: serde_json::Value = resp.json()
            .await
            .context("Failed to parse Create Playlist response")?;
        
        let playlist_id = data.get("Id")
            .and_then(|id| id.as_str())
            .ok_or_else(|| anyhow!("Create Playlist response missing Id field: {:?}", data))?;
        
        Ok(playlist_id.to_string())
    }

    pub async fn get_playlist_item_ids(&self, playlist_id: &str, user_id: &str) -> Result<Vec<String>> {
        let url = format!(
            "{}/Playlists/{}/Items?UserId={}&api_key={}",
            self.base_url, playlist_id, user_id, self.api_key
        );
        let resp = self.client.get(&url)
            .header("X-Emby-Token", &self.api_key)
            .send()
            .await
            .context("Failed to get playlist items")?;
        
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Get playlist items failed: {} - {}", url, body));
        }

        let data: serde_json::Value = resp.json()
            .await
            .context("Failed to parse playlist items")?;
        
        let mut ids = Vec::new();
        if let Some(items) = data.get("Items").and_then(|i| i.as_array()) {
            for item in items {
                if let Some(pl_item_id) = item.get("PlaylistItemId").and_then(|id| id.as_str()) {
                    ids.push(pl_item_id.to_string());
                }
            }
        }
        Ok(ids)
    }

    pub async fn delete_playlist(&self, playlist_id: &str) -> Result<()> {
        let path = format!("/emby/Items/{}", playlist_id);
        let url = self.auth_url(&path);
        let resp = self.client.delete(&url)
            .header("X-Emby-Token", &self.api_key)
            .send()
            .await
            .context("Failed to send Delete Playlist request")?;
        
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Delete Playlist failed: {} - {}", url, body));
        }
        Ok(())
    }

    pub async fn clean_orphaned_playlists(&self) -> Result<()> {
        let users = self.get_all_users().await?;
        for user_id in users {
            let url = format!(
                "{}/emby/Items?UserId={}&IncludeItemTypes=Playlist&Recursive=true&api_key={}",
                self.base_url, user_id, self.api_key
            );
            let resp = self.client.get(&url)
                .header("X-Emby-Token", &self.api_key)
                .send()
                .await
                .context("Failed to query playlists")?;
            
            let data: serde_json::Value = resp.json()
                .await
                .context("Failed to parse playlists list")?;
            
            if let Some(items) = data.get("Items").and_then(|i| i.as_array()) {
                for item in items {
                    if let (Some(id), Some(name)) = (item.get("Id").and_then(|id| id.as_str()), item.get("Name").and_then(|n| n.as_str())) {
                        if name.starts_with("Join ") {
                            info!("Cleaning up orphaned watch-party playlist for user {}: {} ({})", user_id, name, id);
                            let _ = self.delete_playlist(id).await;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn play(&self, session_id: &str, item_id: &str, position_ticks: i64) -> Result<()> {
        let path = format!("/emby/Sessions/{}/Playing", session_id);
        let url = format!(
            "{}{}?ItemIds={}&PlayCommand=PlayNow&StartPositionTicks={}&api_key={}",
            self.base_url, path, item_id, position_ticks, self.api_key
        );
        let resp = self.client.post(&url)
            .header("X-Emby-Token", &self.api_key)
            .send()
            .await
            .context("Failed to send Play command")?;
        
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Play command failed: {} - {}", url, body));
        }
        Ok(())
    }

    pub async fn pause(&self, session_id: &str) -> Result<()> {
        let path = format!("/emby/Sessions/{}/Playing/Pause", session_id);
        let url = self.auth_url(&path);
        let resp = self.client.post(&url)
            .header("X-Emby-Token", &self.api_key)
            .send()
            .await
            .context("Failed to send Pause command")?;
        
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Pause command failed: {} - {}", url, body));
        }
        Ok(())
    }

    pub async fn unpause(&self, session_id: &str) -> Result<()> {
        let path = format!("/emby/Sessions/{}/Playing/Unpause", session_id);
        let url = self.auth_url(&path);
        let resp = self.client.post(&url)
            .header("X-Emby-Token", &self.api_key)
            .send()
            .await
            .context("Failed to send Unpause command")?;
        
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Unpause command failed: {} - {}", url, body));
        }
        Ok(())
    }

    pub async fn seek(&self, session_id: &str, position_ticks: i64) -> Result<()> {
        let path = format!("/emby/Sessions/{}/Playing/Seek", session_id);
        let url = format!(
            "{}{}?SeekPositionTicks={}&api_key={}",
            self.base_url, path, position_ticks, self.api_key
        );
        let resp = self.client.post(&url)
            .header("X-Emby-Token", &self.api_key)
            .send()
            .await
            .context("Failed to send Seek command")?;
        
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Seek command failed: {} - {}", url, body));
        }
        Ok(())
    }

    pub async fn stop(&self, session_id: &str) -> Result<()> {
        let path = format!("/emby/Sessions/{}/Playing/Stop", session_id);
        let url = self.auth_url(&path);
        let resp = self.client.post(&url)
            .header("X-Emby-Token", &self.api_key)
            .send()
            .await
            .context("Failed to send Stop command")?;
        
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Stop command failed: {} - {}", url, body));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CollectivePlayState {
    pub position_ticks: i64,
    pub is_paused: bool,
    pub last_updated: Instant,
}

#[derive(Debug, Clone)]
pub struct SessionHistoryEntry {
    pub item_id: Option<String>,
    pub position_ticks: i64,
    pub is_paused: bool,
    pub last_updated: Instant,
}

#[derive(Debug, Clone)]
pub struct WatchParty {
    pub item_id: String,
    pub item_name: String,
    pub user_playlists: HashMap<String, String>, // User ID -> Playlist ID
    pub playlist_item_ids: HashSet<String>,      // Set of EntryIds (PlaylistItemId)
    pub members: HashSet<String>,                // session IDs
    pub collective_state: CollectivePlayState,
}

pub struct AppState {
    pub session_history: HashMap<String, SessionHistoryEntry>,
    pub cooldowns: HashMap<String, Instant>,
    pub active_parties: HashMap<String, WatchParty>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            session_history: HashMap::new(),
            cooldowns: HashMap::new(),
            active_parties: HashMap::new(),
        }
    }

    pub fn clean_cooldowns(&mut self) {
        let now = Instant::now();
        self.cooldowns.retain(|_, expiry| *expiry > now);
    }

    pub fn is_in_cooldown(&self, session_id: &str) -> bool {
        if let Some(expiry) = self.cooldowns.get(session_id) {
            *expiry > Instant::now()
        } else {
            false
        }
    }

    pub fn set_cooldown(&mut self, session_id: &str, duration: Duration) {
        self.cooldowns.insert(session_id.to_string(), Instant::now() + duration);
    }
}

#[derive(Debug)]
enum SyncAction {
    Play { item_id: String, position_ticks: i64, is_paused: bool },
    Pause,
    Unpause,
    Seek { position_ticks: i64 },
    #[allow(dead_code)]
    Stop,
}

fn deserialize_data<T>(val: &serde_json::Value) -> Result<T, serde_json::Error>
where
    T: serde::de::DeserializeOwned,
{
    match serde_json::from_value::<T>(val.clone()) {
        Ok(res) => Ok(res),
        Err(e) => {
            if let Some(s) = val.as_str() {
                serde_json::from_str::<T>(s)
            } else {
                Err(e)
            }
        }
    }
}

fn make_ws_url(emby_url: &str, api_key: &str) -> String {
    let base = emby_url.trim_end_matches('/');
    let ws_base = if base.starts_with("https://") {
        base.replace("https://", "wss://")
    } else if base.starts_with("http://") {
        base.replace("http://", "ws://")
    } else {
        format!("ws://{}", base)
    };
    format!("{}/embywebsocket?api_key={}&deviceId=emby-syncplay-daemon", ws_base, api_key)
}

async fn execute_sync_action(
    client: &EmbyClient,
    session_id: &str,
    action: SyncAction,
) -> Result<()> {
    match action {
        SyncAction::Play { item_id, position_ticks, is_paused } => {
            info!("Executing PLAY item {} at position {} ticks (paused: {}) on session {}", item_id, position_ticks, is_paused, session_id);
            client.play(session_id, &item_id, position_ticks).await?;
            if is_paused {
                tokio::time::sleep(Duration::from_millis(500)).await;
                client.pause(session_id).await?;
            }
        }
        SyncAction::Pause => {
            info!("Executing PAUSE on session {}", session_id);
            client.pause(session_id).await?;
        }
        SyncAction::Unpause => {
            info!("Executing UNPAUSE on session {}", session_id);
            client.unpause(session_id).await?;
        }
        SyncAction::Seek { position_ticks } => {
            info!("Executing SEEK to {} ticks on session {}", position_ticks, session_id);
            client.seek(session_id, position_ticks).await?;
        }
        SyncAction::Stop => {
            info!("Executing STOP on session {}", session_id);
            client.stop(session_id).await?;
        }
    }
    Ok(())
}

async fn run_sync_logic(
    sessions: &[SessionInfo],
    client: &Arc<EmbyClient>,
    state_lock: &Arc<Mutex<AppState>>,
    config: &Config,
) -> Result<()> {
    let mut state = state_lock.lock().await;
    state.clean_cooldowns();

    let now = Instant::now();
    let threshold_ticks = (config.sync_threshold_seconds * 10_000_000) as i64;

    // 1. Intercept joins: Check if any session played one of our active watch-party playlists
    for s in sessions {
        if state.is_in_cooldown(&s.id) {
            continue;
        }

        // Check if the current session reports a playlist item ID that matches one of our watch-parties
        let s_playlist_item_id = s.playlist_item_id.as_ref()
            .or_else(|| s.play_state.as_ref().and_then(|p| p.playlist_item_id.as_ref()));

        if let Some(pl_item_id) = s_playlist_item_id {
            let mut match_item_id = None;
            for party in state.active_parties.values() {
                if party.playlist_item_ids.contains(pl_item_id) {
                    match_item_id = Some(party.item_id.clone());
                    break;
                }
            }

            if let Some(target_item_id) = match_item_id {
                info!(
                    "Session '{}' played sync playlist for '{}' -> Joining watch party!",
                    s.device_name.as_deref().unwrap_or(&s.device_id),
                    s.now_playing_item.as_ref().and_then(|i| i.name.as_deref()).unwrap_or("Movie")
                );

                if let Some(party) = state.active_parties.get_mut(&target_item_id) {
                    party.members.insert(s.id.clone());
                }

                let other_member = sessions.iter().find(|os| {
                    os.id != s.id 
                    && os.now_playing_item.as_ref().map(|ti| ti.id == target_item_id).unwrap_or(false)
                });

                let (target_pos, target_paused) = if let Some(os) = other_member {
                    let pos = os.play_state.as_ref().and_then(|p| p.position_ticks).unwrap_or(0);
                    let paused = os.play_state.as_ref().and_then(|p| p.is_paused).unwrap_or(false);
                    (pos, paused)
                } else {
                    (0, false)
                };

                let client_clone = client.clone();
                let session_id = s.id.clone();
                let cooldown_dur = Duration::from_secs(config.cooldown_seconds);

                state.set_cooldown(&session_id, cooldown_dur);

                tokio::spawn(async move {
                    let act = SyncAction::Play {
                        item_id: target_item_id,
                        position_ticks: target_pos,
                        is_paused: target_paused,
                    };
                    if let Err(e) = execute_sync_action(&client_clone, &session_id, act).await {
                        error!("Error redirecting session {}: {}", session_id, e);
                    }
                });

                return Ok(());
            }
        }
    }

    // 2. Manage Dynamic Watch-Party Playlists (Create/Delete)
    let mut parties_to_create = Vec::new();
    for s in sessions {
        if let Some(ref item) = s.now_playing_item {
            let item_name = item.name.as_deref().unwrap_or("");
            if !item_name.starts_with("Join ") {
                if !state.active_parties.contains_key(&item.id) {
                    parties_to_create.push((
                        item.id.clone(),
                        item_name.to_string(),
                        s.id.clone(),
                    ));
                }
            }
        }
    }

    for party in state.active_parties.values_mut() {
        party.members.retain(|member_session_id| {
            sessions.iter().any(|s| {
                s.id == *member_session_id 
                && s.now_playing_item.as_ref().map(|item| item.id == party.item_id).unwrap_or(false)
            })
        });
    }

    let mut parties_to_delete = Vec::new();
    for (item_id, party) in &state.active_parties {
        if party.members.is_empty() {
            parties_to_delete.push((item_id.clone(), party.user_playlists.clone()));
        }
    }

    // Drop lock to perform asynchronous network requests
    drop(state);

    for (item_id, user_playlists) in parties_to_delete {
        let client_clone = client.clone();
        let state_lock_clone = state_lock.clone();
        tokio::spawn(async move {
            for (user_id, playlist_id) in user_playlists {
                info!("Deleting watch-party playlist ID {} for user {}", playlist_id, user_id);
                if let Err(e) = client_clone.delete_playlist(&playlist_id).await {
                    error!("Error deleting playlist: {}", e);
                }
            }
            let mut state = state_lock_clone.lock().await;
            state.active_parties.remove(&item_id);
        });
    }

    for (item_id, item_name, host_session_id) in parties_to_create {
        let pl_name = format!("Join - {}", item_name);
        let client_clone = client.clone();
        let state_lock_clone = state_lock.clone();
        
        tokio::spawn(async move {
            info!("Creating watch-party playlist for all users: {}", pl_name);
            match client_clone.get_all_users().await {
                Ok(users) => {
                    let mut user_playlists = HashMap::new();
                    let mut playlist_item_ids = HashSet::new();
                    
                    for user_id in users {
                        match client_clone.create_playlist(&user_id, &pl_name, &item_id).await {
                            Ok(playlist_id) => {
                                info!("Created playlist ID {} for user {}", playlist_id, user_id);
                                user_playlists.insert(user_id.clone(), playlist_id.clone());
                                
                                // Fetch the PlaylistItemId entries in this playlist
                                tokio::time::sleep(Duration::from_millis(150)).await;
                                match client_clone.get_playlist_item_ids(&playlist_id, &user_id).await {
                                    Ok(ids) => {
                                        for id in ids {
                                            playlist_item_ids.insert(id);
                                        }
                                    }
                                    Err(e) => {
                                        error!("Error fetching items for playlist {}: {}", playlist_id, e);
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Error creating playlist for user {}: {}", user_id, e);
                            }
                        }
                    }
                    
                    if !user_playlists.is_empty() {
                        let mut state = state_lock_clone.lock().await;
                        let mut members = HashSet::new();
                        members.insert(host_session_id);
                        state.active_parties.insert(
                            item_id.clone(),
                            WatchParty {
                                item_id,
                                item_name,
                                user_playlists,
                                playlist_item_ids,
                                members,
                                collective_state: CollectivePlayState {
                                    position_ticks: 0,
                                    is_paused: false,
                                    last_updated: Instant::now(),
                                },
                            },
                        );
                    }
                }
                Err(e) => {
                    error!("Error fetching user list: {}", e);
                }
            }
        });
    }

    let mut state = state_lock.lock().await;

    // 3. For each active Watch Party, calculate sync actions among members
    let parties: Vec<WatchParty> = state.active_parties.values().cloned().collect();

    for party in parties {
        let mut detected_interaction = None;

        for s in sessions {
            if !party.members.contains(&s.id) {
                continue;
            }
            if state.is_in_cooldown(&s.id) {
                state.session_history.insert(
                    s.id.clone(),
                    SessionHistoryEntry {
                        item_id: s.now_playing_item.as_ref().map(|item| item.id.clone()),
                        position_ticks: s.play_state.as_ref().and_then(|p| p.position_ticks).unwrap_or(0),
                        is_paused: s.play_state.as_ref().and_then(|p| p.is_paused).unwrap_or(false),
                        last_updated: now,
                    },
                );
                continue;
            }

            if let Some(prev) = state.session_history.get(&s.id) {
                let curr_item = s.now_playing_item.as_ref().map(|item| item.id.clone());
                let curr_pos = s.play_state.as_ref().and_then(|p| p.position_ticks).unwrap_or(0);
                let curr_paused = s.play_state.as_ref().and_then(|p| p.is_paused).unwrap_or(false);

                let mut interacted = false;
                let mut reason = String::new();

                if curr_item.as_ref() == Some(&party.item_id) {
                    if curr_paused != prev.is_paused {
                        interacted = true;
                        reason = format!(
                            "paused state changed from {} to {}",
                            prev.is_paused, curr_paused
                        );
                    } else {
                        let elapsed_ticks = if prev.is_paused {
                            0
                        } else {
                            (now - prev.last_updated).as_nanos() as i64 / 100
                        };
                        let expected_pos = prev.position_ticks + elapsed_ticks;
                        let diff = (curr_pos - expected_pos).abs();
                        if diff > threshold_ticks {
                            interacted = true;
                            reason = format!(
                                "position seek detected: prev_pos={}, expected={}, current={}, diff_secs={:.2}",
                                prev.position_ticks, expected_pos, curr_pos, diff as f64 / 10_000_000.0
                            );
                        }
                    }
                }

                if interacted {
                    info!(
                        "User interaction detected on watch-party member '{}' ({}): {}",
                        s.device_name.as_deref().unwrap_or(&s.device_id),
                        s.client.as_deref().unwrap_or("Unknown Client"),
                        reason
                    );
                    detected_interaction = Some((s.id.clone(), curr_pos, curr_paused));
                    break;
                }
            }
        }

        if let Some((init_session_id, position, is_paused)) = detected_interaction {
            let new_collective = CollectivePlayState {
                position_ticks: position,
                is_paused,
                last_updated: now,
            };

            if let Some(p) = state.active_parties.get_mut(&party.item_id) {
                p.collective_state = new_collective;
            }

            for s in sessions {
                if !party.members.contains(&s.id) || s.id == init_session_id {
                    if s.id == init_session_id {
                        state.session_history.insert(
                            s.id.clone(),
                            SessionHistoryEntry {
                                item_id: s.now_playing_item.as_ref().map(|item| item.id.clone()),
                                position_ticks: position,
                                is_paused,
                                last_updated: now,
                            },
                        );
                    }
                    continue;
                }

                let s_paused = s.play_state.as_ref().and_then(|p| p.is_paused).unwrap_or(false);
                let s_pos = s.play_state.as_ref().and_then(|p| p.position_ticks).unwrap_or(0);

                let action = if s_paused != is_paused {
                    if is_paused {
                        Some(SyncAction::Pause)
                    } else {
                        Some(SyncAction::Unpause)
                    }
                } else if (s_pos - position).abs() > threshold_ticks {
                    Some(SyncAction::Seek { position_ticks: position })
                } else {
                    None
                };

                if let Some(act) = action {
                    info!(
                        "Syncing watch-party member '{}' to match initiator state: {:?}",
                        s.device_name.as_deref().unwrap_or(&s.device_id),
                        act
                    );

                    let client_clone = client.clone();
                    let session_id = s.id.clone();
                    let cooldown_dur = Duration::from_secs(config.cooldown_seconds);

                    state.set_cooldown(&session_id, cooldown_dur);

                    tokio::spawn(async move {
                        if let Err(e) = execute_sync_action(&client_clone, &session_id, act).await {
                            error!("Error executing sync action on session {}: {}", session_id, e);
                        }
                    });
                }
            }
        } else {
            let coll = party.collective_state.clone();
            for s in sessions {
                if !party.members.contains(&s.id) || state.is_in_cooldown(&s.id) {
                    continue;
                }

                let s_pos = s.play_state.as_ref().and_then(|p| p.position_ticks).unwrap_or(0);
                let s_paused = s.play_state.as_ref().and_then(|p| p.is_paused).unwrap_or(false);

                let expected_ticks = coll.position_ticks + if coll.is_paused {
                    0
                } else {
                    (now - coll.last_updated).as_nanos() as i64 / 100
                };

                let action = if s_paused != coll.is_paused {
                    if coll.is_paused {
                        Some(SyncAction::Pause)
                    } else {
                        Some(SyncAction::Unpause)
                    }
                } else if (s_pos - expected_ticks).abs() > threshold_ticks {
                    Some(SyncAction::Seek { position_ticks: expected_ticks })
                } else {
                    None
                };

                if let Some(act) = action {
                    info!(
                        "Watch-party member '{}' is out of sync. Executing correction: {:?}",
                        s.device_name.as_deref().unwrap_or(&s.device_id),
                        act
                    );

                    let client_clone = client.clone();
                    let session_id = s.id.clone();
                    let cooldown_dur = Duration::from_secs(config.cooldown_seconds);

                    state.set_cooldown(&session_id, cooldown_dur);

                    tokio::spawn(async move {
                        if let Err(e) = execute_sync_action(&client_clone, &session_id, act).await {
                            error!("Error executing sync correction on session {}: {}", session_id, e);
                        }
                    });
                }
            }
        }
    }

    for s in sessions {
        state.session_history.insert(
            s.id.clone(),
            SessionHistoryEntry {
                item_id: s.now_playing_item.as_ref().map(|item| item.id.clone()),
                position_ticks: s.play_state.as_ref().and_then(|p| p.position_ticks).unwrap_or(0),
                is_paused: s.play_state.as_ref().and_then(|p| p.is_paused).unwrap_or(false),
                last_updated: now,
            },
        );
    }

    Ok(())
}

async fn handle_ws_message(
    text: &str,
    client: &Arc<EmbyClient>,
    state_lock: &Arc<Mutex<AppState>>,
    config: &Config,
) -> Result<()> {
    let ws_msg: WsMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(_) => return Ok(()),
    };

    if ws_msg.message_type == "Sessions" {
        if let Some(data) = ws_msg.data {
            let sessions: Vec<SessionInfo> = deserialize_data(&data)
                .context("Failed to deserialize sessions list from WebSocket")?;
            run_sync_logic(&sessions, client, state_lock, config).await?;
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting Emby Sync Play daemon...");

    let config_path = "config.json";
    let config_data = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read configuration file: {}", config_path))?;
    
    let config: Config = serde_json::from_str(&config_data)
        .context("Failed to parse configuration file")?;

    info!("Configuration loaded. Connecting to Emby: {}", config.emby_url);

    let client = Arc::new(EmbyClient::new(config.emby_url.clone(), config.api_key.clone()));

    // Attempt to log active sessions at startup
    match client.get_sessions().await {
        Ok(sessions) => {
            info!("Successfully connected to Emby server. Active sessions found:");
            for s in &sessions {
                info!(
                    "  - Device: '{}', Client: '{}', User: '{}', DeviceId: '{}'",
                    s.device_name.as_deref().unwrap_or("Unknown Device"),
                    s.client.as_deref().unwrap_or("Unknown Client"),
                    s.user_name.as_deref().unwrap_or("None"),
                    s.device_id
                );
            }
        }
        Err(e) => {
            warn!("Could not fetch initial sessions from Emby (is the server up?): {}", e);
        }
    }

    // Clean up any orphaned watch-party playlists left over from a previous run
    if let Err(e) = client.clean_orphaned_playlists().await {
        warn!("Failed to clean up orphaned playlists at startup: {}", e);
    }

    let app_state = Arc::new(Mutex::new(AppState::new()));
    let ws_url = make_ws_url(&config.emby_url, &config.api_key);

    let client_clone = client.clone();
    let state_clone = app_state.clone();
    let config_clone = config.clone();

    // Start WebSocket reconnection loop
    tokio::spawn(async move {
        loop {
            info!("Connecting to Emby WebSocket: {}", ws_url);
            match connect_async(&ws_url).await {
                Ok((mut ws_stream, _)) => {
                    info!("WebSocket connection established.");

                    let start_msg = json!({
                        "MessageType": "SessionsStart",
                        "Data": "0,1000"
                    });
                    if let Err(e) = ws_stream.send(WsMessageProto::Text(start_msg.to_string().into())).await {
                        error!("Failed to send subscribe message: {}", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                    info!("Subscribed to Sessions updates.");

                    while let Some(msg) = ws_stream.next().await {
                        match msg {
                            Ok(WsMessageProto::Text(text)) => {
                                if let Err(e) = handle_ws_message(&text, &client_clone, &state_clone, &config_clone).await {
                                    error!("Error handling WS message: {}", e);
                                }
                            }
                            Ok(_) => {}
                            Err(e) => {
                                error!("WebSocket stream error: {}", e);
                                break;
                            }
                        }
                    }
                    warn!("WebSocket connection lost. Reconnecting in 5 seconds...");
                }
                Err(e) => {
                    error!("Failed to connect to WebSocket: {}. Retrying in 5 seconds...", e);
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });

    tokio::signal::ctrl_c().await?;
    
    // Attempt graceful cleanup of any remaining playlists before exit
    info!("Cleaning up active watch-party playlists before exit...");
    let active_playlists_to_clean: Vec<String> = {
        let state = app_state.lock().await;
        state.active_parties.values()
            .flat_map(|pl| pl.user_playlists.values().cloned())
            .collect()
    };
    for pl_id in active_playlists_to_clean {
        let _ = client.delete_playlist(&pl_id).await;
    }

    info!("Stopping Emby Sync Play daemon.");
    Ok(())
}
