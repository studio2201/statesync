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
    let mut status = ForceSyncStatus::idle();
    status.state = ForceSyncState::Completed;
    status.started_at = Some("start".to_string());
    status.finished_at = Some("finish".to_string());
    status.direction = Some(Direction::EmbyToJellyfin);
    status.total_pairs = 10;
    status.processed = 10;
    status.succeeded = 8;
    status.skipped = 1;
    status.failed = 1;
    status.last_error = Some("err".to_string());
    status.current_source = Some("Emby".into());
    status.current_target = Some("Jellyfin".into());
    status.story_headline = Some("Force sync finished".into());
    assert_eq!(status.state, ForceSyncState::Completed);
    assert_eq!(status.total_pairs, 10);
    assert_eq!(status.succeeded, 8);
    assert_eq!(status.skipped, 1);
    assert_eq!(status.failed, 1);
    assert_eq!(status.current_source.as_deref(), Some("Emby"));
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
