use serde::{Deserialize, Serialize};

pub mod helpers;
pub mod loader;
#[cfg(test)]
pub mod tests;
pub mod url_safety;
pub mod validation;

pub use helpers::{name_from_url, normalize_server_url, redacted_url};
pub use loader::{
    default_config, get_config_path, load_or_create_default, write_default_config_to_disk,
};
pub use url_safety::{valid_server_url, validate_upstream_url};
pub use validation::{is_loopback_bind, normalize_config, validate_config};

pub const MAX_NAME_LEN: usize = 64;
pub const MAX_URL_LEN: usize = 512;
pub const MAX_KEY_LEN: usize = 256;
pub const MAX_MAPPING_GROUPS: usize = 128;
pub const MAX_GROUP_MEMBERS: usize = 32;
pub const MAX_MEMBER_LEN: usize = 64;
pub const MAX_CONFIG_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub name: String,
    pub url: String,
    pub api_key: String,
    pub is_emby: bool,
    #[serde(default = "default_sync_direction")]
    pub sync_direction: String, // "both", "send", "receive"
    #[serde(default = "default_allow_insecure_http")]
    pub allow_insecure_http: bool,
}

fn default_allow_insecure_http() -> bool {
    true
}

fn default_sync_direction() -> String {
    "both".to_string()
}

/// What StateSync is allowed to copy (safe power-law set).
///
/// Live = as play/favorite events happen. Force = historical backfill.
/// Missing fields in old configs deserialize to these defaults.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct SyncOptions {
    /// Live: copy Played flag from UserData / finish events.
    #[serde(default = "default_true")]
    pub live_played: bool,
    /// Live: copy playback position while watching.
    #[serde(default = "default_true")]
    pub live_position: bool,
    /// Live: copy IsFavorite (heart).
    #[serde(default = "default_true")]
    pub live_favorites: bool,
    /// Force: push played history.
    #[serde(default = "default_true")]
    pub force_played: bool,
    /// Force: push in-progress positions with played history.
    #[serde(default = "default_true")]
    pub force_position: bool,
    /// Force: push favorites.
    #[serde(default = "default_true")]
    pub force_favorites: bool,
    /// If non-empty, only these usernames (case-insensitive) take part in live/force sync.
    /// Empty = everyone. Also matches names linked via user_mappings.
    #[serde(default)]
    pub user_allowlist: Vec<String>,
    /// Usernames that never live- or force-sync (guests, kids, test accounts).
    /// Takes priority over the allowlist. Linked aliases share the ignore.
    #[serde(default)]
    pub user_ignorelist: Vec<String>,
}

fn default_true() -> bool {
    true
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            live_played: true,
            live_position: true,
            live_favorites: true,
            force_played: true,
            force_position: true,
            force_favorites: true,
            user_allowlist: Vec::new(),
            user_ignorelist: Vec::new(),
        }
    }
}

impl SyncOptions {
    fn name_set(list: &[String]) -> std::collections::HashSet<String> {
        list.iter()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    }

    fn group_touches(
        set: &std::collections::HashSet<String>,
        username: &str,
        maps: &[Vec<String>],
    ) -> bool {
        let u = username.trim().to_lowercase();
        if set.contains(&u) {
            return true;
        }
        for group in maps {
            let members: Vec<String> = group
                .iter()
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            if members.iter().any(|m| m == &u) && members.iter().any(|m| set.contains(m)) {
                return true;
            }
        }
        false
    }

    /// True if this person is on the ignore list (or linked to someone who is).
    pub fn user_is_ignored(&self, username: &str, user_mappings: &[Vec<String>]) -> bool {
        let ignored = Self::name_set(&self.user_ignorelist);
        if ignored.is_empty() {
            return false;
        }
        Self::group_touches(&ignored, username, user_mappings)
    }

    /// True if `username` is the same person as `filter` (exact or linked mapping).
    pub fn user_matches_filter(
        username: &str,
        filter: &str,
        user_mappings: &[Vec<String>],
    ) -> bool {
        let u = username.trim().to_lowercase();
        let f = filter.trim().to_lowercase();
        if u.is_empty() || f.is_empty() {
            return false;
        }
        if u == f {
            return true;
        }
        for group in user_mappings {
            let members: Vec<String> = group
                .iter()
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            if members.iter().any(|m| m == &u) && members.iter().any(|m| m == &f) {
                return true;
            }
        }
        false
    }

    /// True if this username may sync (not ignored; allowlist empty = all users).
    pub fn user_allowed(&self, username: &str, user_mappings: &[Vec<String>]) -> bool {
        if self.user_is_ignored(username, user_mappings) {
            return false;
        }
        let want = Self::name_set(&self.user_allowlist);
        if want.is_empty() {
            return true;
        }
        Self::group_touches(&want, username, user_mappings)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub servers: Vec<ServerConfig>,
    #[serde(default = "default_threshold_seconds")]
    pub sync_threshold_seconds: u64,
    #[serde(default)]
    pub user_mappings: Vec<Vec<String>>,
    #[serde(default)]
    pub last_full_sync: Option<crate::sync_force::ForceSyncStatus>,
    /// Live / force field toggles (played, position, favorites).
    #[serde(default)]
    pub sync: SyncOptions,
}

fn default_threshold_seconds() -> u64 {
    5
}
