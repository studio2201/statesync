use crate::client::MediaClient;
use anyhow::Result;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ServerCache {
    pub name: String,
    pub users: HashMap<String, String>, // username (lowercase) -> UserId
    pub imdb_to_id: HashMap<String, String>, // ImdbId -> ItemId
    pub tmdb_to_id: HashMap<String, String>, // TmdbId -> ItemId
    pub id_to_providers: HashMap<String, (String, String)>, // ItemId -> (ImdbId, TmdbId)
}

impl ServerCache {
    /// Merge freshly-fetched users into this cache, preserving any
    /// existing entries. A transient API hiccup that returns fewer
    /// users than the cache currently has will no longer drop them.
    pub fn merge_users(&mut self, fresh: HashMap<String, String>) {
        for (k, v) in fresh {
            self.users.entry(k).or_insert(v);
        }
    }
}

pub async fn init_server_cache(name: &str, client: &MediaClient) -> Result<ServerCache> {
    let users = client.get_users().await?;
    let items = client.get_library_items().await?;

    let mut imdb_to_id: HashMap<String, Vec<String>> = HashMap::new();
    let mut tmdb_to_id: HashMap<String, Vec<String>> = HashMap::new();
    let mut id_to_providers = HashMap::new();

    for (id, (imdb, tmdb)) in items {
        if !imdb.is_empty() {
            imdb_to_id.entry(imdb.clone()).or_default().push(id.clone());
        }
        if !tmdb.is_empty() {
            tmdb_to_id.entry(tmdb.clone()).or_default().push(id.clone());
        }
        id_to_providers.insert(id, (imdb, tmdb));
    }

    let mut imdb_flat = HashMap::new();
    for (k, mut v) in imdb_to_id {
        if v.len() > 1 {
            tracing::warn!(
                "server '{}': IMDb id '{}' maps to {} items; using first id '{}' (multi-version libraries may sync the wrong version)",
                name,
                k,
                v.len(),
                v.first().map(|s| s.as_str()).unwrap_or("?")
            );
        }
        if let Some(first) = v.drain(..).next() {
            imdb_flat.insert(k, first);
        }
    }
    let mut tmdb_flat = HashMap::new();
    for (k, mut v) in tmdb_to_id {
        if v.len() > 1 {
            tracing::warn!(
                "server '{}': TMDb id '{}' maps to {} items; using first id '{}' (multi-version libraries may sync the wrong version)",
                name,
                k,
                v.len(),
                v.first().map(|s| s.as_str()).unwrap_or("?")
            );
        }
        if let Some(first) = v.drain(..).next() {
            tmdb_flat.insert(k, first);
        }
    }

    Ok(ServerCache {
        name: name.to_string(),
        users,
        imdb_to_id: imdb_flat,
        tmdb_to_id: tmdb_flat,
        id_to_providers,
    })
}
