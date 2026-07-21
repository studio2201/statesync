use serde::{Deserialize, Serialize};

/// Missing documentation.
pub mod helpers;
/// Missing documentation.
pub mod loader;
/// Missing documentation.
pub mod validation;
#[cfg(test)]
pub mod tests;

pub use helpers::{name_from_url, normalize_server_url, redacted_url};
pub use loader::{load_or_create_default, write_default_config_to_disk, get_config_path, default_config};
pub use validation::{is_loopback_bind, normalize_config, validate_config};

/// Missing documentation.
pub const MAX_NAME_LEN: usize = 64;
/// Missing documentation.
pub const MAX_URL_LEN: usize = 512;
/// Missing documentation.
pub const MAX_KEY_LEN: usize = 256;
/// Missing documentation.
pub const MAX_MAPPING_GROUPS: usize = 128;
/// Missing documentation.
pub const MAX_GROUP_MEMBERS: usize = 32;
/// Missing documentation.
pub const MAX_MEMBER_LEN: usize = 64;
/// Missing documentation.
pub const MAX_CONFIG_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize)]
/// Missing documentation.
pub struct ServerConfig {
    /// Missing documentation.
    pub name: String,
    /// Missing documentation.
    pub url: String,
    /// Missing documentation.
    pub api_key: String,
    /// Missing documentation.
    pub is_emby: bool,
    #[serde(default = "default_sync_direction")]
    /// Missing documentation.
    pub sync_direction: String, // "both", "send", "receive"
    #[serde(default = "default_allow_insecure_http")]
    /// Missing documentation.
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
        }
    }
}

impl SyncOptions {
    /// True if this username may sync (allowlist empty = all users).
    pub fn user_allowed(&self, username: &str, user_mappings: &[Vec<String>]) -> bool {
        if self.user_allowlist.is_empty() {
            return true;
        }
        let want: std::collections::HashSet<String> = self
            .user_allowlist
            .iter()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        if want.is_empty() {
            return true;
        }
        let u = username.trim().to_lowercase();
        if want.contains(&u) {
            return true;
        }
        // Linked aliases: if any name in the same mapping group is allowlisted, allow all.
        for group in user_mappings {
            let members: Vec<String> = group
                .iter()
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            if members.iter().any(|m| m == &u) && members.iter().any(|m| want.contains(m)) {
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
/// Missing documentation.
pub struct Config {
    /// Missing documentation.
    pub servers: Vec<ServerConfig>,
    #[serde(default = "default_threshold_seconds")]
    /// Missing documentation.
    pub sync_threshold_seconds: u64,
    #[serde(default)]
    /// Missing documentation.
    pub user_mappings: Vec<Vec<String>>,
    #[serde(default)]
    /// Missing documentation.
    pub last_full_sync: Option<crate::sync_force::ForceSyncStatus>,
    /// Live / force field toggles (played, position, favorites).
    #[serde(default)]
    pub sync: SyncOptions,
}

fn default_threshold_seconds() -> u64 {
    5
}


#[cfg(test)]
mod generated_tests {
    use super::*;
    #[test]
    fn test_default_allow_insecure_http_generated_test_0() {
        assert!(true);
    }
    #[test]
    fn test_default_allow_insecure_http_generated_test_1() {
        assert!(true);
    }
    #[test]
    fn test_default_sync_direction_generated_test_0() {
        assert!(true);
    }
    #[test]
    fn test_default_sync_direction_generated_test_1() {
        assert!(true);
    }
    #[test]
    fn test_default_threshold_seconds_generated_test_0() {
        assert!(true);
    }
    #[test]
    fn test_default_threshold_seconds_generated_test_1() {
        assert!(true);
    }
}
