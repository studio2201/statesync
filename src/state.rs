use crate::client::MediaClient;
use crate::sync_force::SyncForceTracker;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

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

#[derive(Debug, Clone)]
pub struct SyncHistoryValue {
    pub position_ticks: i64,
    pub timestamp: Instant,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncLogEntry {
    pub timestamp: String,
    pub level: String,
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
    pub log_retention: usize,
    pub sync_force: Arc<SyncForceTracker>,
}

fn default_log_retention() -> usize {
    std::env::var("STATESYNC_LOG_RETENTION")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(30)
        .max(1)
}

impl AppState {
    pub fn new(caches: Vec<ServerCache>) -> Self {
        let count = caches.len();
        let retention = default_log_retention();
        Self {
            caches,
            last_syncs: HashMap::new(),
            websocket_statuses: vec!["Offline".to_string(); count],
            sync_logs: Vec::new(),
            active_sessions: HashMap::new(),
            log_retention: retention,
            sync_force: Arc::new(SyncForceTracker::default()),
        }
    }

    pub fn log_event(&mut self, level: &str, msg: &str) {
        let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
        self.sync_logs.insert(
            0,
            SyncLogEntry {
                timestamp,
                level: level.to_string(),
                message: msg.to_string(),
                source_name: None,
                source_is_emby: None,
                target_name: None,
                target_is_emby: None,
            },
        );
        if self.sync_logs.len() > self.log_retention {
            self.sync_logs.truncate(self.log_retention);
        }
    }

    pub fn log_sync(&mut self, entry: SyncLogEntry) {
        self.sync_logs.insert(0, entry);
        if self.sync_logs.len() > self.log_retention {
            self.sync_logs.truncate(self.log_retention);
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
    for (k, v) in imdb_to_id {
        if let Some(first) = v.into_iter().next() {
            imdb_flat.insert(k, first);
        }
    }
    let mut tmdb_flat = HashMap::new();
    for (k, v) in tmdb_to_id {
        if let Some(first) = v.into_iter().next() {
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

fn min_substring_len(a: &str, b: &str) -> usize {
    (a.len().min(b.len()) / 2).max(3)
}

pub fn find_mapped_user_id(
    source_username: &str,
    target_users: &HashMap<String, String>,
    custom_mappings: &[Vec<String>],
) -> Option<String> {
    let src_lower = source_username.to_lowercase();

    for group in custom_mappings {
        if group.iter().any(|u| u.to_lowercase() == src_lower) {
            for mapped_name in group {
                let mapped_lower = mapped_name.to_lowercase();
                if mapped_lower != src_lower {
                    if let Some(id) = target_users.get(&mapped_lower) {
                        return Some(id.clone());
                    }
                }
            }
        }
    }

    if let Some(id) = target_users.get(&src_lower) {
        return Some(id.clone());
    }

    let mut candidates: Vec<(&String, &String)> = target_users
        .iter()
        .filter(|(tgt_name, _)| {
            let tgt_lower = tgt_name.to_lowercase();
            let min_len = min_substring_len(&src_lower, &tgt_lower);
            if src_lower.len() < min_len || tgt_lower.len() < min_len {
                return false;
            }
            tgt_lower.contains(&src_lower) || src_lower.contains(&tgt_lower)
        })
        .collect();
    candidates.sort_by(|a, b| {
        let a_diff = (a.0.len() as i64 - src_lower.len() as i64).abs();
        let b_diff = (b.0.len() as i64 - src_lower.len() as i64).abs();
        a_diff.cmp(&b_diff)
    });
    if let Some((_, id)) = candidates.into_iter().next() {
        return Some(id.clone());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_username_match() {
        let mut target_users = HashMap::new();
        target_users.insert("john".to_string(), "id123".to_string());
        let mapped = find_mapped_user_id("JOHN", &target_users, &[]);
        assert_eq!(mapped, Some("id123".to_string()));
    }

    #[test]
    fn test_substring_username_match() {
        let mut target_users = HashMap::new();
        target_users.insert("john".to_string(), "id123".to_string());
        let mapped = find_mapped_user_id("John Doe", &target_users, &[]);
        assert_eq!(mapped, Some("id123".to_string()));

        let mut target_users2 = HashMap::new();
        target_users2.insert("john doe".to_string(), "id456".to_string());
        let mapped2 = find_mapped_user_id("john", &target_users2, &[]);
        assert_eq!(mapped2, Some("id456".to_string()));
    }

    #[test]
    fn test_custom_username_mapping_override() {
        let mut target_users = HashMap::new();
        target_users.insert("john_alt".to_string(), "id999".to_string());
        target_users.insert("john".to_string(), "id123".to_string());
        let custom_mappings = vec![vec!["john_special".to_string(), "john_alt".to_string()]];
        let mapped = find_mapped_user_id("john_special", &target_users, &custom_mappings);
        assert_eq!(mapped, Some("id999".to_string()));
    }

    #[test]
    fn test_username_collision_prevention() {
        let mut target_users = HashMap::new();
        target_users.insert("john smith".to_string(), "id777".to_string());
        let mapped = find_mapped_user_id("john doe", &target_users, &[]);
        assert_eq!(mapped, None);
    }

    #[test]
    fn test_substring_length_guard_rejects_short_lookalikes() {
        let mut target_users = HashMap::new();
        target_users.insert("alice".to_string(), "id_a".to_string());
        target_users.insert("aaron".to_string(), "id_b".to_string());
        let mapped = find_mapped_user_id("a", &target_users, &[]);
        assert_eq!(mapped, None);
    }

    #[test]
    fn test_substring_picks_closest_match() {
        let mut target_users = HashMap::new();
        target_users.insert("alice smith".to_string(), "id_long".to_string());
        target_users.insert("alice".to_string(), "id_short".to_string());
        let mapped = find_mapped_user_id("alice", &target_users, &[]);
        assert_eq!(mapped, Some("id_short".to_string()));
    }

    #[test]
    fn merge_users_preserves_existing_entries() {
        // Regression: a transient /Users call returning fewer users
        // should not shrink the cache. Existing entries win.
        let mut cache = ServerCache {
            name: "emby".to_string(),
            users: HashMap::new(),
            imdb_to_id: HashMap::new(),
            tmdb_to_id: HashMap::new(),
            id_to_providers: HashMap::new(),
        };
        cache.users.insert("alice".to_string(), "u1".to_string());
        cache.users.insert("bob".to_string(), "u2".to_string());
        cache.users.insert("carol".to_string(), "u3".to_string());

        // Simulate a refresh that returns only alice and dave (a new user)
        let mut fresh = HashMap::new();
        fresh.insert("alice".to_string(), "u1".to_string());
        fresh.insert("dave".to_string(), "u4".to_string());
        cache.merge_users(fresh);

        // All original users still present
        assert!(cache.users.contains_key("alice"));
        assert!(cache.users.contains_key("bob"));
        assert!(cache.users.contains_key("carol"));
        // New user added
        assert!(cache.users.contains_key("dave"));
        // Total: 4
        assert_eq!(cache.users.len(), 4);
    }

    #[test]
    fn merge_users_empty_fresh_is_noop() {
        let mut cache = ServerCache {
            name: "emby".to_string(),
            users: HashMap::new(),
            imdb_to_id: HashMap::new(),
            tmdb_to_id: HashMap::new(),
            id_to_providers: HashMap::new(),
        };
        cache.users.insert("alice".to_string(), "u1".to_string());
        cache.merge_users(HashMap::new());
        assert_eq!(cache.users.len(), 1);
        assert!(cache.users.contains_key("alice"));
    }
}
