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
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub emby_url: String,
    pub api_key: String,
    pub daemon_ip: String,
    pub watchparty_dir: String,
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

    #[serde(alias = "remoteEndPoint", alias = "RemoteEndPoint")]
    pub remote_end_point: Option<String>,
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

    pub async fn refresh_library(&self) -> Result<()> {
        let path = "/emby/Library/Refresh";
        let url = self.auth_url(path);
        let resp = self.client.post(&url)
            .header("X-Emby-Token", &self.api_key)
            .send()
            .await
            .context("Failed to send Library Refresh request")?;
        
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Library Refresh failed: {} - {}", url, body));
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
    pub members: HashSet<String>, // session IDs
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

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

fn clean_watchparty_dir(dir: &str) {
    let path = std::path::Path::new(dir);
    if !path.exists() {
        if let Err(e) = std::fs::create_dir_all(path) {
            error!("Failed to create watchparty directory {}: {}", dir, e);
            return;
        }
    }
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && p.extension().map(|ext| ext == "strm").unwrap_or(false) {
                let _ = std::fs::remove_file(p);
            }
        }
    }
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

    // 1. Detect rooms playing movies and manage the .strm files in the watch party library
    let mut parties_to_create = Vec::new();
    for s in sessions {
        if let Some(ref item) = s.now_playing_item {
            let item_name = item.name.as_deref().unwrap_or("");
            // Exclude our own join card playing to avoid self-referencing
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

    // Clean up members who stopped playing their party item
    for party in state.active_parties.values_mut() {
        party.members.retain(|member_session_id| {
            sessions.iter().any(|s| {
                s.id == *member_session_id 
                && s.now_playing_item.as_ref().map(|item| item.id == party.item_id).unwrap_or(false)
            })
        });
    }

    // Find parties that have no active members left
    let mut parties_to_delete = Vec::new();
    for (item_id, party) in &state.active_parties {
        // If the only person playing is the host or if no one is playing the item, delete it
        let is_any_real_viewer = sessions.iter().any(|s| {
            s.now_playing_item.as_ref().map(|item| item.id == *item_id).unwrap_or(false)
        });
        if !is_any_real_viewer {
            parties_to_delete.push((item_id.clone(), party.item_name.clone()));
        }
    }

    // Perform deletions and file cleanups
    for (item_id, item_name) in parties_to_delete {
        state.active_parties.remove(&item_id);
        
        let filename = format!("Join - {}.strm", sanitize_filename(&item_name));
        let path = std::path::Path::new(&config.watchparty_dir).join(&filename);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
            info!("Deleted watch-party redirect file: {:?}", path);
            
            let client_clone = client.clone();
            tokio::spawn(async move {
                if let Err(e) = client_clone.refresh_library().await {
                    error!("Error triggering library refresh: {}", e);
                }
            });
        }
    }

    // Perform creations
    for (item_id, item_name, host_session_id) in parties_to_create {
        let mut members = HashSet::new();
        members.insert(host_session_id);
        state.active_parties.insert(
            item_id.clone(),
            WatchParty {
                item_id: item_id.clone(),
                item_name: item_name.clone(),
                members,
                collective_state: CollectivePlayState {
                    position_ticks: 0,
                    is_paused: false,
                    last_updated: Instant::now(),
                },
            },
        );

        let filename = format!("Join - {}.strm", sanitize_filename(&item_name));
        let path = std::path::Path::new(&config.watchparty_dir).join(&filename);
        let file_content = format!("http://{}:8090/join?item_id={}", config.daemon_ip, item_id);
        if let Err(e) = std::fs::write(&path, file_content) {
            error!("Failed to create redirect strm file: {}", e);
        } else {
            info!("Created watch-party redirect file: {:?}", path);
            
            let client_clone = client.clone();
            tokio::spawn(async move {
                if let Err(e) = client_clone.refresh_library().await {
                    error!("Error triggering library refresh: {}", e);
                }
            });
        }
    }

    // 2. For each active Watch Party, calculate sync actions among members
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

async fn start_http_server(
    state_lock: Arc<Mutex<AppState>>,
    client: Arc<EmbyClient>,
    config: Config,
) {
    let listener = match TcpListener::bind("0.0.0.0:8090").await {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to bind HTTP server to port 8090: {}", e);
            return;
        }
    };
    info!("HTTP redirector server listening on 0.0.0.0:8090");

    loop {
        let (mut socket, addr) = match listener.accept().await {
            Ok(v) => v,
            Err(_) => continue,
        };

        let state_clone = state_lock.clone();
        let client_clone = client.clone();
        let config_clone = config.clone();

        tokio::spawn(async move {
            let mut buf = [0; 2048];
            let n = match socket.read(&mut buf).await {
                Ok(n) if n > 0 => n,
                _ => return,
            };

            let req_text = String::from_utf8_lossy(&buf[..n]);
            let first_line = req_text.lines().next().unwrap_or("");
            
            if first_line.starts_with("GET /join") {
                let parts: Vec<&str> = first_line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let uri = parts[1];
                    let mut item_id = None;
                    if let Some(pos) = uri.find("item_id=") {
                        let val = &uri[pos + 8..];
                        let end = val.find('&').unwrap_or(val.len());
                        item_id = Some(val[..end].to_string());
                    }

                    if let Some(target_item_id) = item_id {
                        let client_ip = addr.ip().to_string();
                        info!("Received HTTP join request from IP {} for item {}", client_ip, target_item_id);

                        match client_clone.get_sessions().await {
                            Ok(sessions) => {
                                let mut target_session = None;
                                for s in &sessions {
                                    if let Some(ref ep) = s.remote_end_point {
                                        let ep_ip = ep.split(':').next().unwrap_or(ep);
                                        if ep_ip == client_ip || (client_ip == "127.0.0.1" && (ep_ip == "localhost" || ep_ip == "::1" || ep_ip == "127.0.0.1")) {
                                            target_session = Some(s.clone());
                                            break;
                                        }
                                    }
                                }

                                if let Some(s) = target_session {
                                    {
                                        let mut state = state_clone.lock().await;
                                        if let Some(party) = state.active_parties.get_mut(&target_item_id) {
                                            party.members.insert(s.id.clone());
                                        }
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

                                    info!("Redirecting session '{}' (IP: {}) to sync play item {}", s.device_name.as_deref().unwrap_or(&s.device_id), client_ip, target_item_id);

                                    {
                                        let mut state = state_clone.lock().await;
                                        state.set_cooldown(&s.id, Duration::from_secs(config_clone.cooldown_seconds));
                                    }

                                    let client_clone2 = client_clone.clone();
                                    let s_id = s.id.clone();
                                    tokio::spawn(async move {
                                        let act = SyncAction::Play {
                                            item_id: target_item_id,
                                            position_ticks: target_pos,
                                            is_paused: target_paused,
                                        };
                                        if let Err(e) = execute_sync_action(&client_clone2, &s_id, act).await {
                                            error!("Error redirecting session {}: {}", s_id, e);
                                        }
                                    });
                                } else {
                                    warn!("No Emby session found matching client IP: {}", client_ip);
                                }
                            }
                            Err(e) => {
                                error!("Error fetching sessions for IP match: {}", e);
                            }
                        }
                    }
                }
            }

            let resp = "HTTP/1.1 200 OK\r\nContent-Type: video/mp4\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            let _ = socket.write_all(resp.as_bytes()).await;
        });
    }
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

    // Clean up watch party directory
    info!("Cleaning watch-party strm directory: {}", config.watchparty_dir);
    clean_watchparty_dir(&config.watchparty_dir);

    // Attempt to log active sessions at startup
    match client.get_sessions().await {
        Ok(sessions) => {
            info!("Successfully connected to Emby server. Active sessions found:");
            for s in &sessions {
                info!(
                    "  - Device: '{}', Client: '{}', User: '{}', DeviceId: '{}', Remote IP: '{:?}'",
                    s.device_name.as_deref().unwrap_or("Unknown Device"),
                    s.client.as_deref().unwrap_or("Unknown Client"),
                    s.user_name.as_deref().unwrap_or("None"),
                    s.device_id,
                    s.remote_end_point
                );
            }
        }
        Err(e) => {
            warn!("Could not fetch initial sessions from Emby (is the server up?): {}", e);
        }
    }

    // Trigger library scan at start to sync directory cleaning
    let _ = client.refresh_library().await;

    let app_state = Arc::new(Mutex::new(AppState::new()));
    let ws_url = make_ws_url(&config.emby_url, &config.api_key);

    let client_clone = client.clone();
    let state_clone = app_state.clone();
    let config_clone = config.clone();

    // Start TCP HTTP server
    let state_http = app_state.clone();
    let client_http = client.clone();
    let config_http = config.clone();
    tokio::spawn(async move {
        start_http_server(state_http, client_http, config_http).await;
    });

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
    
    // Cleanup strm files on exit
    info!("Cleaning up active watch-party redirect files before exit...");
    clean_watchparty_dir(&config.watchparty_dir);
    let _ = client.refresh_library().await;

    info!("Stopping Emby Sync Play daemon.");
    Ok(())
}
