//! Force-sync status / direction type contracts.

use statesync::sync_force::{Direction, ForceSyncState, ForceSyncStatus};

#[test]
fn test_force_sync_status_idle() {
    let status = ForceSyncStatus::idle();
    assert_eq!(status.state, ForceSyncState::Idle);
    assert!(status.started_at.is_none());
    assert!(status.finished_at.is_none());
    assert_eq!(status.processed, 0);
}

#[test]
fn test_force_sync_status_default() {
    let status = ForceSyncStatus::default();
    assert_eq!(status.state, ForceSyncState::Idle);
    assert!(status.errors.is_empty());
}

#[test]
fn test_force_sync_state_equality() {
    assert_eq!(ForceSyncState::Idle, ForceSyncState::Idle);
    assert_ne!(ForceSyncState::Running, ForceSyncState::Completed);
}

#[test]
fn test_force_sync_direction_equality() {
    assert_eq!(Direction::Both, Direction::Both);
    assert_ne!(Direction::EmbyToJellyfin, Direction::JellyfinToEmby);
}

#[test]
fn test_force_sync_status_fields() {
    let status = ForceSyncStatus {
        state: ForceSyncState::Completed,
        started_at: Some("start".to_string()),
        finished_at: Some("finish".to_string()),
        direction: Some(Direction::EmbyToJellyfin),
        total_pairs: 10,
        processed: 10,
        succeeded: 8,
        skipped: 1,
        failed: 1,
        current_user: None,
        last_error: Some("err".to_string()),
        errors: vec![],
        phase: None,
        by_field: Default::default(),
        scope: Vec::new(),
        skip_reasons: Default::default(),
        dry_run: false,
    };
    assert_eq!(status.state, ForceSyncState::Completed);
    assert_eq!(status.total_pairs, 10);
    assert_eq!(status.succeeded, 8);
    assert_eq!(status.skipped, 1);
    assert_eq!(status.failed, 1);
}

#[test]
fn test_force_sync_state_debug() {
    assert!(format!("{:?}", ForceSyncState::Running).contains("Running"));
}

#[test]
fn test_force_sync_direction_debug() {
    assert!(format!("{:?}", Direction::Both).contains("Both"));
}

#[test]
fn test_force_sync_status_debug() {
    let status = ForceSyncStatus::idle();
    assert!(format!("{:?}", status).contains("state"));
}
