use crate::sync_force::SyncForceTracker;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

pub mod cache;
#[cfg(test)]
pub mod tests;
#[cfg(test)]
mod tests_more;
pub mod user_mapping;

pub use cache::{ServerCache, init_server_cache};
pub use user_mapping::find_mapped_user_id;

#[derive(Debug, Clone)]
pub struct SyncHistoryValue {
    pub position_ticks: i64,
    pub timestamp: Instant,
    /// Whether the last synced update marked the item as played.
    pub played: bool,
    /// Last synced favorite flag (None = never synced favorites for this key).
    pub favorite: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncLogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
    /// Extra technical detail (IDs, errors) — shown in UI and included in copy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
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
        .unwrap_or(100)
        .max(1)
}

impl AppState {
    pub fn new(caches: Vec<ServerCache>) -> Self {
        let count = caches.len();
        let retention = default_log_retention();
        let tracker = SyncForceTracker::default();
        if let Ok(config) = crate::config::Config::load() {
            if let Some(fs) = config.last_full_sync {
                if let Ok(mut status) = tracker.status.try_lock() {
                    *status = fs;
                }
            }
        }
        Self {
            caches,
            last_syncs: HashMap::new(),
            websocket_statuses: vec!["Offline".to_string(); count],
            sync_logs: Vec::new(),
            active_sessions: HashMap::new(),
            log_retention: retention,
            sync_force: Arc::new(tracker),
        }
    }

    pub fn log_event(&mut self, level: &str, msg: &str) {
        self.log_event_detail(level, msg, None);
    }

    /// Log with optional technical detail line.
    pub fn log_event_detail(&mut self, level: &str, msg: &str, detail: Option<String>) {
        let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
        self.sync_logs.insert(
            0,
            SyncLogEntry {
                timestamp,
                level: level.to_string(),
                message: msg.to_string(),
                detail,
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
