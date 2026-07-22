use crate::client::MediaClient;
use crate::client::ProviderIds;
use anyhow::Result;
use std::collections::HashMap;

const NOT_FOUND: &str = "[ NOT_FOUND ]";

#[derive(Debug, Clone)]
pub struct ServerCache {
    pub name: String,
    pub users: HashMap<String, String>, // username (lowercase) -> UserId
    pub imdb_to_id: HashMap<String, String>,
    pub tmdb_to_id: HashMap<String, String>,
    pub tvdb_to_id: HashMap<String, String>,
    pub id_to_providers: HashMap<String, ProviderIds>,
}

impl ServerCache {
    pub fn empty(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            users: HashMap::new(),
            imdb_to_id: HashMap::new(),
            tmdb_to_id: HashMap::new(),
            tvdb_to_id: HashMap::new(),
            id_to_providers: HashMap::new(),
        }
    }

    /// Merge freshly-fetched users into this cache, preserving any
    /// existing entries. A transient API hiccup that returns fewer
    /// users than the cache currently has will no longer drop them.
    pub fn merge_users(&mut self, fresh: HashMap<String, String>) {
        for (k, v) in fresh {
            self.users.entry(k).or_insert(v);
        }
    }

    /// In-memory lookup: Imdb → Tmdb → Tvdb. Skips negative cache entries.
    pub fn lookup_item_id(&self, p: &ProviderIds) -> Option<String> {
        let hit = |map: &HashMap<String, String>, key: &str| -> Option<String> {
            if key.is_empty() {
                return None;
            }
            match map.get(key) {
                Some(id) if id != NOT_FOUND => Some(id.clone()),
                _ => None,
            }
        };
        hit(&self.imdb_to_id, &p.imdb)
            .or_else(|| hit(&self.tmdb_to_id, &p.tmdb))
            .or_else(|| hit(&self.tvdb_to_id, &p.tvdb))
    }

    /// True if we already searched and know this provider set is missing on this server.
    pub fn is_negative_cached(&self, p: &ProviderIds) -> bool {
        let neg = |map: &HashMap<String, String>, key: &str| -> bool {
            !key.is_empty() && map.get(key).map(|id| id == NOT_FOUND).unwrap_or(false)
        };
        // Only treat as negative if every non-empty key is negative (or empty).
        let mut any = false;
        let mut all_neg = true;
        for (map, key) in [
            (&self.imdb_to_id, p.imdb.as_str()),
            (&self.tmdb_to_id, p.tmdb.as_str()),
            (&self.tvdb_to_id, p.tvdb.as_str()),
        ] {
            if key.is_empty() {
                continue;
            }
            any = true;
            if !neg(map, key) {
                all_neg = false;
            }
        }
        any && all_neg
    }

    pub fn index_item(&mut self, item_id: String, p: ProviderIds) {
        if !p.imdb.is_empty() {
            self.imdb_to_id.insert(p.imdb.clone(), item_id.clone());
        }
        if !p.tmdb.is_empty() {
            self.tmdb_to_id.insert(p.tmdb.clone(), item_id.clone());
        }
        if !p.tvdb.is_empty() {
            self.tvdb_to_id.insert(p.tvdb.clone(), item_id.clone());
        }
        self.id_to_providers.insert(item_id, p);
    }

    pub fn index_not_found(&mut self, p: &ProviderIds) {
        if !p.imdb.is_empty() {
            self.imdb_to_id
                .insert(p.imdb.clone(), NOT_FOUND.to_string());
        }
        if !p.tmdb.is_empty() {
            self.tmdb_to_id
                .insert(p.tmdb.clone(), NOT_FOUND.to_string());
        }
        if !p.tvdb.is_empty() {
            self.tvdb_to_id
                .insert(p.tvdb.clone(), NOT_FOUND.to_string());
        }
    }
}

pub async fn init_server_cache(name: &str, client: &MediaClient) -> Result<ServerCache> {
    let users = client.get_users().await?;
    let items = client.get_library_items().await?;

    let mut cache = ServerCache {
        name: name.to_string(),
        users,
        imdb_to_id: HashMap::new(),
        tmdb_to_id: HashMap::new(),
        tvdb_to_id: HashMap::new(),
        id_to_providers: HashMap::new(),
    };

    for (id, providers) in items {
        // First-writer wins when one external id maps to multiple library versions.
        if !providers.imdb.is_empty() {
            cache
                .imdb_to_id
                .entry(providers.imdb.clone())
                .or_insert_with(|| id.clone());
        }
        if !providers.tmdb.is_empty() {
            cache
                .tmdb_to_id
                .entry(providers.tmdb.clone())
                .or_insert_with(|| id.clone());
        }
        if !providers.tvdb.is_empty() {
            cache
                .tvdb_to_id
                .entry(providers.tvdb.clone())
                .or_insert_with(|| id.clone());
        }
        cache.id_to_providers.insert(id, providers);
    }

    Ok(cache)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_prefers_imdb_then_tmdb_then_tvdb() {
        let mut c = ServerCache {
            name: "s".into(),
            users: HashMap::new(),
            imdb_to_id: HashMap::new(),
            tmdb_to_id: HashMap::new(),
            tvdb_to_id: HashMap::new(),
            id_to_providers: HashMap::new(),
        };
        c.index_item(
            "a".into(),
            ProviderIds::from_parts("tt1", "", ""),
        );
        c.index_item(
            "b".into(),
            ProviderIds::from_parts("", "99", ""),
        );
        c.index_item(
            "c".into(),
            ProviderIds::from_parts("", "", "73244"),
        );
        assert_eq!(
            c.lookup_item_id(&ProviderIds::from_parts("tt1", "99", "73244"))
                .as_deref(),
            Some("a")
        );
        assert_eq!(
            c.lookup_item_id(&ProviderIds::from_parts("", "", "73244"))
                .as_deref(),
            Some("c")
        );
    }

    #[test]
    fn negative_cache_skips_lookup() {
        let mut c = ServerCache {
            name: "s".into(),
            users: HashMap::new(),
            imdb_to_id: HashMap::new(),
            tmdb_to_id: HashMap::new(),
            tvdb_to_id: HashMap::new(),
            id_to_providers: HashMap::new(),
        };
        let p = ProviderIds::from_parts("", "", "1");
        c.index_not_found(&p);
        assert!(c.lookup_item_id(&p).is_none());
        assert!(c.is_negative_cached(&p));
    }
}
