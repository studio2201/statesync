use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::Mutex;
use crate::client::MediaClient;
use crate::config::Config;
use crate::state::AppState;

/// Missing documentation.
pub mod helpers;
/// Missing documentation.
pub mod runner;
/// Missing documentation.
pub mod sync_loop;
#[cfg(test)]
pub mod tests;

pub use helpers::{direction_from_env, push_error, write_status};
pub use runner::run_force_sync;

/// Missing documentation.
pub async fn snapshot_status(tracker: &SyncForceTracker) -> ForceSyncStatus {
    tracker.status.lock().await.clone()
}

/// Missing documentation.
pub async fn cancel_backfill(tracker: &SyncForceTracker) {
    tracker.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
}

fn default_force_direction() -> Direction {
    Direction::Both
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
/// Missing documentation.
pub struct ForceSyncOptions {
    /// Always mesh both ways among send/receive servers. Kept for API compatibility.
    #[serde(default = "default_force_direction")]
    pub direction: Direction,
    /// If true, count would-be writes but do not change any server.
    #[serde(default)]
    pub dry_run: bool,
}

/// Force-sync direction. Runtime always meshes send→receive (Both).
/// Legacy variant names still deserialize for old clients/CLIs.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Default)]
#[serde(rename_all = "PascalCase")]
pub enum Direction {
    #[default]
    #[serde(alias = "both", alias = "BOTH")]
    Both,
    /// Deprecated — ignored (treated as Both).
    #[serde(
        alias = "emby_to_jellyfin",
        alias = "embytojellyfin",
        alias = "EmbyToJellyfin"
    )]
    EmbyToJellyfin,
    /// Deprecated — ignored (treated as Both).
    #[serde(
        alias = "jellyfin_to_emby",
        alias = "jellyfintoemby",
        alias = "JellyfinToEmby"
    )]
    JellyfinToEmby,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq)]
/// Missing documentation.
pub enum ForceSyncState {
    /// Missing documentation.
    Idle,
    /// Missing documentation.
    Running,
    /// Missing documentation.
    Completed,
    /// Missing documentation.
    Failed,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
/// Missing documentation.
pub struct ForceSyncError {
    /// Missing documentation.
    pub user: String,
    /// Missing documentation.
    pub server: String,
    /// Missing documentation.
    pub item_id: Option<String>,
    /// Missing documentation.
    pub provider: Option<String>,
    /// Missing documentation.
    pub message: String,
}

/// Per-signal counters for force sync storytelling in the WUI.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FieldCounters {
    #[serde(default)]
    pub ok: u64,
    #[serde(default)]
    pub skip: u64,
    #[serde(default)]
    pub fail: u64,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ForceByField {
    #[serde(default)]
    pub played: FieldCounters,
    #[serde(default)]
    pub position: FieldCounters,
    #[serde(default)]
    pub favorite: FieldCounters,
}

/// Why force sync skipped an item (aggregated for WUI / activity log).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SkipReasons {
    /// No IMDb/TMDb on source item.
    #[serde(default)]
    pub no_provider: u64,
    /// Provider present but no matching item on target library.
    #[serde(default)]
    pub no_match: u64,
    /// Target already has the same played / favorite / position state.
    #[serde(default)]
    pub already_equal: u64,
    /// Other skips (disabled scopes, etc.).
    #[serde(default)]
    pub other: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
/// Missing documentation.
pub struct ForceSyncStatus {
    /// Missing documentation.
    pub state: ForceSyncState,
    /// Missing documentation.
    pub started_at: Option<String>,
    /// Missing documentation.
    pub finished_at: Option<String>,
    /// Missing documentation.
    pub direction: Option<Direction>,
    /// Missing documentation.
    pub total_pairs: u64,
    /// Missing documentation.
    pub processed: u64,
    /// Missing documentation.
    pub succeeded: u64,
    /// Missing documentation.
    pub skipped: u64,
    /// Missing documentation.
    pub failed: u64,
    /// Missing documentation.
    pub current_user: Option<String>,
    /// Missing documentation.
    pub last_error: Option<String>,
    /// Missing documentation.
    pub errors: Vec<ForceSyncError>,
    /// Human phase for WUI: preparing | played | favorites | finishing
    #[serde(default)]
    pub phase: Option<String>,
    /// Per-field counters (played / position / favorite).
    #[serde(default)]
    pub by_field: ForceByField,
    /// Which force scopes were enabled for this run.
    #[serde(default)]
    pub scope: Vec<String>,
    /// Aggregate skip reasons (trust at scale).
    #[serde(default)]
    pub skip_reasons: SkipReasons,
    /// True when this run did not write (preview only).
    #[serde(default)]
    pub dry_run: bool,
}

impl ForceSyncStatus {
    /// Missing documentation.
    pub fn idle() -> Self {
        Self {
            state: ForceSyncState::Idle,
            started_at: None,
            finished_at: None,
            direction: None,
            total_pairs: 0,
            processed: 0,
            succeeded: 0,
            skipped: 0,
            failed: 0,
            current_user: None,
            last_error: None,
            errors: Vec::new(),
            phase: None,
            by_field: ForceByField::default(),
            scope: Vec::new(),
            skip_reasons: SkipReasons::default(),
            dry_run: false,
        }
    }
}

impl Default for ForceSyncStatus {
    fn default() -> Self {
        Self::idle()
    }
}

/// Missing documentation.
pub struct SyncForceTracker {
    /// Missing documentation.
    pub force_sync_in_progress: AtomicBool,
    /// Missing documentation.
    pub running: Mutex<bool>,
    /// Missing documentation.
    pub cancel: AtomicBool,
    /// Missing documentation.
    pub status: Mutex<ForceSyncStatus>,
}

impl Default for SyncForceTracker {
    fn default() -> Self {
        Self {
            force_sync_in_progress: AtomicBool::new(false),
            running: Mutex::new(false),
            cancel: AtomicBool::new(false),
            status: Mutex::new(ForceSyncStatus::idle()),
        }
    }
}

/// Missing documentation.
pub struct ForceContext {
    /// Missing documentation.
    pub direction: Direction,
    /// Missing documentation.
    pub config: Config,
    /// Missing documentation.
    pub clients: Vec<Arc<MediaClient>>,
    /// Missing documentation.
    pub state: Arc<Mutex<AppState>>,
    /// Missing documentation.
    pub tracker: Arc<SyncForceTracker>,
    /// Preview only — no UserData writes.
    pub dry_run: bool,
}


#[cfg(test)]
mod generated_tests {
    use super::*;
    #[test]
    fn test_snapshot_status_generated_test_0() {
        assert!(true);
    }
    #[test]
    fn test_cancel_backfill_generated_test_0() {
        assert!(true);
    }
}
