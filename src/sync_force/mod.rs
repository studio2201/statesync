//! Force-sync orchestration (historical backfill).

pub mod force_constants;
pub mod force_equal;
pub mod force_favorites_pair;
pub mod force_pair_plan;
pub mod force_played_items;
pub mod force_played_pair;
pub mod force_run_inner;
pub mod force_story;
pub mod force_types;
pub mod helpers;
pub mod runner;
pub mod sync_loop;
#[cfg(test)]
pub mod tests;

pub use force_types::*;
pub use helpers::{direction_from_env, push_error, write_status};
pub use runner::run_force_sync;

/// Snapshot force-sync status for CLI/API.
pub async fn snapshot_status(tracker: &SyncForceTracker) -> ForceSyncStatus {
    tracker.snapshot_status()
}

/// Request cancel of an in-progress force sync.
pub async fn cancel_backfill(tracker: &SyncForceTracker) {
    tracker
        .cancel
        .store(true, std::sync::atomic::Ordering::SeqCst);
}
