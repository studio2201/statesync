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
pub struct Config {
    pub emby_url: String,
    pub api_key: String,
    pub sync_devices: Vec<String>,
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
    pub item_id: String,
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

pub struct AppState {
    pub collective_state: Option<CollectivePlayState>,
    pub session_history: HashMap<String, SessionHistoryEntry>,
    pub cooldowns: HashMap<String, Instant>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            collective_state: None,
            session_history: HashMap::new(),
            cooldowns: HashMap::new(),
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

    let active_sync_sessions: Vec<&SessionInfo> = sessions
        .iter()
        .filter(|s| config.sync_devices.contains(&s.device_id))
        .collect();

    if active_sync_sessions.is_empty() {
        return Ok(());
    }

    let now = Instant::now();
    let threshold_ticks = (config.sync_threshold_seconds * 10_000_000) as i64;

    // Detect user interactions
    let mut detected_interaction = None;

    for s in &active_sync_sessions {
        if state.is_in_cooldown(&s.id) {
            state.session_history.insert(
                s.device_id.clone(),
                SessionHistoryEntry {
                    item_id: s.now_playing_item.as_ref().map(|item| item.id.clone()),
                    position_ticks: s.play_state.as_ref().and_then(|p| p.position_ticks).unwrap_or(0),
                    is_paused: s.play_state.as_ref().and_then(|p| p.is_paused).unwrap_or(false),
                    last_updated: now,
                },
            );
            continue;
        }

        if let Some(prev) = state.session_history.get(&s.device_id) {
            let curr_item = s.now_playing_item.as_ref().map(|item| item.id.clone());
            let curr_pos = s.play_state.as_ref().and_then(|p| p.position_ticks).unwrap_or(0);
            let curr_paused = s.play_state.as_ref().and_then(|p| p.is_paused).unwrap_or(false);

            let mut interacted = false;
            let mut reason = String::new();

            if curr_item != prev.item_id {
                interacted = true;
                reason = format!(
                    "media item changed from {:?} to {:?}",
                    prev.item_id, curr_item
                );
            } else if curr_paused != prev.is_paused {
                interacted = true;
                reason = format!(
                    "paused state changed from {} to {}",
                    prev.is_paused, curr_paused
                );
            } else if curr_item.is_some() {
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

            if interacted {
                info!(
                    "User interaction detected on device '{}' ({}): {}",
                    s.device_name.as_deref().unwrap_or(&s.device_id),
                    s.client.as_deref().unwrap_or("Unknown Client"),
                    reason
                );
                detected_interaction = Some((s.id.clone(), s.device_id.clone(), curr_item, curr_pos, curr_paused));
                break;
            }
        }
    }

    if let Some((init_session_id, _init_device_id, item_id, position, is_paused)) = detected_interaction {
        let new_collective = item_id.clone().map(|id| CollectivePlayState {
            item_id: id,
            position_ticks: position,
            is_paused,
            last_updated: now,
        });

        state.collective_state = new_collective;

        for s in &active_sync_sessions {
            if s.id == init_session_id {
                state.session_history.insert(
                    s.device_id.clone(),
                    SessionHistoryEntry {
                        item_id: s.now_playing_item.as_ref().map(|item| item.id.clone()),
                        position_ticks: position,
                        is_paused,
                        last_updated: now,
                    },
                );
                continue;
            }

            let s_item = s.now_playing_item.as_ref().map(|item| item.id.clone());
            let s_paused = s.play_state.as_ref().and_then(|p| p.is_paused).unwrap_or(false);
            let s_pos = s.play_state.as_ref().and_then(|p| p.position_ticks).unwrap_or(0);

            let action = match item_id.as_ref() {
                None => {
                    if s_item.is_some() {
                        Some(SyncAction::Stop)
                    } else {
                        None
                    }
                }
                Some(id) => {
                    if s_item.as_ref() != Some(id) {
                        Some(SyncAction::Play {
                            item_id: id.clone(),
                            position_ticks: position,
                            is_paused,
                        })
                    } else {
                        if s_paused != is_paused {
                            if is_paused {
                                Some(SyncAction::Pause)
                            } else {
                                Some(SyncAction::Unpause)
                            }
                        } else if (s_pos - position).abs() > threshold_ticks {
                            Some(SyncAction::Seek { position_ticks: position })
                        } else {
                            None
                        }
                    }
                }
            };

            if let Some(act) = action {
                info!(
                    "Syncing device '{}' to match initiator state: {:?}",
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
        if state.collective_state.is_none() {
            if let Some(playing_session) = active_sync_sessions.iter().find(|s| s.now_playing_item.is_some()) {
                let item = playing_session.now_playing_item.as_ref().unwrap();
                let play_state = playing_session.play_state.as_ref();
                let position = play_state.and_then(|p| p.position_ticks).unwrap_or(0);
                let is_paused = play_state.and_then(|p| p.is_paused).unwrap_or(false);

                info!(
                    "Initializing collective state from active playing session on '{}' (item: {})",
                    playing_session.device_name.as_deref().unwrap_or(&playing_session.device_id),
                    item.id
                );

                state.collective_state = Some(CollectivePlayState {
                    item_id: item.id.clone(),
                    position_ticks: position,
                    is_paused,
                    last_updated: now,
                });
            }
        }

        if let Some(coll) = state.collective_state.clone() {
            for s in &active_sync_sessions {
                if state.is_in_cooldown(&s.id) {
                    continue;
                }

                let s_item = s.now_playing_item.as_ref().map(|item| item.id.clone());
                let s_pos = s.play_state.as_ref().and_then(|p| p.position_ticks).unwrap_or(0);
                let s_paused = s.play_state.as_ref().and_then(|p| p.is_paused).unwrap_or(false);

                let expected_ticks = coll.position_ticks + if coll.is_paused {
                    0
                } else {
                    (now - coll.last_updated).as_nanos() as i64 / 100
                };

                let action = if s_item.as_ref() != Some(&coll.item_id) {
                    Some(SyncAction::Play {
                        item_id: coll.item_id.clone(),
                        position_ticks: expected_ticks,
                        is_paused: coll.is_paused,
                    })
                } else if s_paused != coll.is_paused {
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
                        "Device '{}' is out of sync. Executing correction: {:?}",
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

    for s in &active_sync_sessions {
        state.session_history.insert(
            s.device_id.clone(),
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

    info!("Configuration loaded. Syncing {} devices.", config.sync_devices.len());
    for dev in &config.sync_devices {
        info!("  - {}", dev);
    }

    let client = Arc::new(EmbyClient::new(config.emby_url.clone(), config.api_key.clone()));
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
    info!("Stopping Emby Sync Play daemon.");
    Ok(())
}
