use std::collections::HashMap;
use std::time::Instant;
use anyhow::{Result, anyhow};
use crate::client::MediaClient;

#[derive(Debug, Clone)]
pub struct ServerCache {
    pub name: String,
    pub users: HashMap<String, String>, // username (lowercase) -> UserId
    pub imdb_to_id: HashMap<String, String>, // ImdbId -> ItemId
    pub tmdb_to_id: HashMap<String, String>, // TmdbId -> ItemId
    pub id_to_providers: HashMap<String, (String, String)>, // ItemId -> (ImdbId, TmdbId)
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SyncHistoryValue {
    pub position_ticks: i64,
    pub timestamp: Instant,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncLogEntry {
    pub timestamp: String,
    pub level: String, // "info", "warn", "error", "success"
    pub message: String,
    pub source_name: Option<String>,
    pub source_is_emby: Option<bool>,
    pub target_name: Option<String>,
    pub target_is_emby: Option<bool>,
}

pub struct AppState {
    pub caches: Vec<ServerCache>,
    pub last_syncs: HashMap<(String, String), SyncHistoryValue>,
    pub websocket_statuses: Vec<String>,
    pub sync_logs: Vec<SyncLogEntry>,
    pub active_sessions: HashMap<(String, String), (String, String, f64, bool, String)>,
}

impl AppState {
    pub fn new(caches: Vec<ServerCache>) -> Self {
        let count = caches.len();
        Self {
            caches,
            last_syncs: HashMap::new(),
            websocket_statuses: vec!["Offline".to_string(); count],
            sync_logs: Vec::new(),
            active_sessions: HashMap::new(),
        }
    }

    pub fn log_event(&mut self, level: &str, msg: &str) {
        let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();

        self.sync_logs.insert(0, SyncLogEntry {
            timestamp,
            level: level.to_string(),
            message: msg.to_string(),
            source_name: None,
            source_is_emby: None,
            target_name: None,
            target_is_emby: None,
        });
        if self.sync_logs.len() > 30 {
            self.sync_logs.truncate(30);
        }
    }

    pub fn log_sync(&mut self, entry: SyncLogEntry) {
        self.sync_logs.insert(0, entry);
        if self.sync_logs.len() > 30 {
            self.sync_logs.truncate(30);
        }
    }
}

pub async fn init_server_cache(name: &str, client: &MediaClient) -> Result<ServerCache> {
    let users = client.get_users().await?;
    let first_user_id = users.values().next().ok_or_else(|| anyhow!("No users found on server '{}'", name))?;
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
        name: name.to_string(),
        users,
        imdb_to_id,
        tmdb_to_id,
        id_to_providers,
    })
}

pub fn find_mapped_user_id(
    source_username: &str,
    target_users: &HashMap<String, String>,
) -> Option<String> {
    let src_lower = source_username.to_lowercase();
    if let Some(id) = target_users.get(&src_lower) {
        return Some(id.clone());
    }
    let src_first = src_lower.split_whitespace().next().unwrap_or(&src_lower);
    for (tgt_name, tgt_id) in target_users {
        let tgt_first = tgt_name.split_whitespace().next().unwrap_or(tgt_name);
        if src_first == tgt_first {
            return Some(tgt_id.clone());
        }
    }
    None
}
