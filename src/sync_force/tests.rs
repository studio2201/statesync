#[cfg(test)]
mod tests {
    use crate::sync_force::{ForceSyncState, ForceSyncStatus, SyncForceTracker, ForceSyncError, helpers};
    use crate::sync_force::runner::rate_from_env;

    #[test]
    fn force_direction_accepts_lowercase_both() {
        // UI historically sent "both"; that caused HTTP 422 before aliases.
        let opts: crate::sync_force::ForceSyncOptions =
            serde_json::from_str(r#"{"direction":"both"}"#).unwrap();
        assert_eq!(opts.direction, crate::sync_force::Direction::Both);
        let opts2: crate::sync_force::ForceSyncOptions =
            serde_json::from_str(r#"{"direction":"Both"}"#).unwrap();
        assert_eq!(opts2.direction, crate::sync_force::Direction::Both);
        let opts3: crate::sync_force::ForceSyncOptions = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(opts3.direction, crate::sync_force::Direction::Both);
    }

    #[test]
    fn idle_status_is_clean() {
        let status = ForceSyncStatus::idle();
        assert_eq!(status.state, ForceSyncState::Idle);
        assert!(status.started_at.is_none());
        assert!(status.finished_at.is_none());
        assert!(status.errors.is_empty());
    }

    #[test]
    fn idle_status_has_no_finished_at() {
        let status = ForceSyncStatus::idle();
        assert!(status.finished_at.is_none());
    }

    #[test]
    fn running_status_carries_progress_counts() {
        let status = ForceSyncStatus {
            state: ForceSyncState::Running,
            started_at: Some("now".to_string()),
            finished_at: None,
            direction: None,
            total_pairs: 5,
            processed: 2,
            succeeded: 2,
            skipped: 0,
            failed: 0,
            current_user: None,
            last_error: None,
            errors: Vec::new(),
            phase: None,
            by_field: Default::default(),
            scope: Vec::new(),
            skip_reasons: Default::default(),
            dry_run: false,
        };
        assert_eq!(status.processed, 2);
        assert_eq!(status.total_pairs, 5);
    }

    #[test]
    fn rate_clamped_to_range() {
        unsafe {
            std::env::set_var("STATESYNC_FORCE_RATE", "100");
        }
        assert_eq!(rate_from_env(), 50);

        unsafe {
            std::env::set_var("STATESYNC_FORCE_RATE", "0");
        }
        assert_eq!(rate_from_env(), 1);

        unsafe {
            std::env::set_var("STATESYNC_FORCE_RATE", "25");
        }
        assert_eq!(rate_from_env(), 25);

        unsafe {
            std::env::remove_var("STATESYNC_FORCE_RATE");
        }
        assert_eq!(rate_from_env(), 5);
    }

    #[test]
    fn cancel_backfill_sets_flag() {
        let tracker = SyncForceTracker::default();
        assert!(!tracker.cancel.load(std::sync::atomic::Ordering::SeqCst));
        tracker.cancel_backfill();
        assert!(tracker.cancel.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn errors_capped_at_limit() {
        let mut status = ForceSyncStatus::idle();
        let mut errors = Vec::new();

        for i in 0..150 {
            helpers::push_error(
                &mut errors,
                &mut status,
                ForceSyncError {
                    user: "u".to_string(),
                    server: "s".to_string(),
                    item_id: None,
                    provider: None,
                    message: format!("err {}", i),
                },
            );
        }

        assert_eq!(errors.len(), 100);
        assert_eq!(errors[0].message, "err 50");
        assert_eq!(errors[99].message, "err 149");
    }

    #[test]
    fn test_direction_from_env() {
        // Type-based force directions are retired — always Both.
        unsafe {
            std::env::set_var("STATESYNC_FORCE_DIRECTION", "emby_to_jellyfin");
        }
        assert_eq!(helpers::direction_from_env(), crate::sync_force::Direction::Both);

        unsafe {
            std::env::set_var("STATESYNC_FORCE_DIRECTION", "jellyfin_to_emby");
        }
        assert_eq!(helpers::direction_from_env(), crate::sync_force::Direction::Both);

        unsafe {
            std::env::remove_var("STATESYNC_FORCE_DIRECTION");
        }
        assert_eq!(helpers::direction_from_env(), crate::sync_force::Direction::Both);
    }

    #[test]
    fn test_write_status() {
        let tracker = SyncForceTracker::default();
        let mut status = ForceSyncStatus::idle();
        status.processed = 42;
        helpers::write_status(&tracker, &status);

        let snap = tracker.snapshot_status();
        assert_eq!(snap.processed, 42);
    }

    #[tokio::test]
    async fn test_module_snapshot_status_and_cancel() {
        let tracker = SyncForceTracker::default();
        let snap = crate::sync_force::snapshot_status(&tracker).await;
        assert_eq!(snap.state, ForceSyncState::Idle);

        crate::sync_force::cancel_backfill(&tracker).await;
        assert!(tracker.cancel.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_force_sync_pair_cancelled_immediately() {
        let server = mockito::Server::new_async().await;
        let client = std::sync::Arc::new(crate::client::MediaClient::new(server.url(), "key".to_string(), false));
        let tracker = std::sync::Arc::new(SyncForceTracker::default());
        tracker.cancel.store(true, std::sync::atomic::Ordering::SeqCst);

        let config = crate::config::default_config();
        let state = std::sync::Arc::new(tokio::sync::Mutex::new(crate::state::AppState::new(vec![])));
        let ctx = crate::sync_force::ForceContext {
            direction: crate::sync_force::Direction::Both,
            config,
            clients: vec![client.clone(), client.clone()],
            state,
            tracker: tracker.clone(),
            dry_run: false,
        };

        let mut status = ForceSyncStatus::idle();
        let mut processed = 0;
        let mut succeeded = 0;
        let mut skipped = 0;
        let mut failed = 0;
        let mut errors = vec![];
        let sem = tokio::sync::Semaphore::new(1);

        let cancelled = crate::sync_force::sync_loop::force_sync_pair(
            0,
            1,
            "alice",
            "u1",
            "u2",
            &ctx,
            &mut status,
            &mut processed,
            &mut succeeded,
            &mut skipped,
            &mut failed,
            &mut errors,
            &sem,
            std::time::Duration::from_millis(1),
        ).await;

        assert!(cancelled);
        assert_eq!(processed, 0);
    }
}
