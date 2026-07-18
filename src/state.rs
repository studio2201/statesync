use std::collections::HashMap;
use std::time::Instant;
use anyhow::{Result, anyhow};
use crate::client::MediaClient;

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

pub async fn init_server_cache(client: &MediaClient) -> Result<ServerCache> {
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
