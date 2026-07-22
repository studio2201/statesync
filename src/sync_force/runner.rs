use std::sync::atomic::Ordering;

use super::force_run_inner::run_force_sync_inner;
use super::{ForceByField, ForceContext, ForceSyncState, ForceSyncStatus};

pub(super) fn rate_from_env() -> u32 {
    std::env::var("STATESYNC_FORCE_RATE")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .map(|v| v.clamp(1, 50))
        .unwrap_or(5)
}

pub async fn run_force_sync(ctx: ForceContext) -> ForceSyncStatus {
    {
        let mut running = ctx.tracker.running.lock().await;
        *running = true;
    }
    ctx.tracker
        .force_sync_in_progress
        .store(true, Ordering::SeqCst);
    ctx.tracker.cancel.store(false, Ordering::SeqCst);

    let started = chrono::Utc::now();
    let mut scope = Vec::new();
    if ctx.config.sync.force_played {
        scope.push("played".to_string());
    }
    if ctx.config.sync.force_position {
        scope.push("position".to_string());
    }
    if ctx.config.sync.force_favorites {
        scope.push("favorites".to_string());
    }
    if ctx.dry_run {
        scope.push("dry-run".to_string());
    }
    if let Some(ref u) = ctx.only_user {
        scope.push(format!("user={}", u));
    }
    {
        let mut status = ctx
            .tracker
            .status
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let (story_h, story_d) =
            super::force_story::story_started(ctx.dry_run, ctx.only_user.as_deref());
        let mut running = ForceSyncStatus {
            state: ForceSyncState::Running,
            started_at: Some(started.to_rfc3339()),
            finished_at: None,
            direction: Some(ctx.direction),
            total_pairs: 0,
            processed: 0,
            succeeded: 0,
            skipped: 0,
            failed: 0,
            current_user: ctx.only_user.clone(),
            last_error: None,
            errors: Vec::new(),
            phase: Some("preparing".to_string()),
            by_field: ForceByField::default(),
            scope: scope.clone(),
            skip_reasons: Default::default(),
            dry_run: ctx.dry_run,
            current_source: None,
            current_target: None,
            pair_index: 0,
            pair_total: 0,
            story_headline: Some(story_h.clone()),
            story_detail: Some(story_d.clone()),
        };
        // Keep machine fields aligned with the plain-language story.
        running.phase = Some("preparing".to_string());
        *status = running;
    }
    {
        let mut st = ctx.state.lock().await;
        let (headline, detail) =
            super::force_story::story_started(ctx.dry_run, ctx.only_user.as_deref());
        st.log_event_detail(
            "info",
            &headline,
            Some(format!(
                "{detail} Scope: {}.",
                if scope.is_empty() {
                    "none".to_string()
                } else {
                    scope.join(", ")
                }
            )),
        );
    }

    let result = run_force_sync_inner(&ctx, started).await;

    ctx.tracker
        .force_sync_in_progress
        .store(false, Ordering::SeqCst);
    {
        let mut running = ctx.tracker.running.lock().await;
        *running = false;
    }

    let mut status = ctx
        .tracker
        .status
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    *status = result.clone();
    // Don't persist dry-run results as last_full_sync (would mislead "last force").
    if !result.dry_run {
        if let Ok(mut config) = crate::config::Config::load() {
            config.last_full_sync = Some(result.clone());
            if let Err(e) = config.save() {
                tracing::error!(
                    "run_force_sync: failed to save force sync status to config: {}",
                    e
                );
            }
        }
    }
    status.clone()
}
