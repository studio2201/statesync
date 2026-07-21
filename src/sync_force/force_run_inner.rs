use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::info;

use super::runner::rate_from_env;
use super::sync_loop::{force_sync_favorites_pair, force_sync_pair};
use super::{ForceContext, ForceSyncError, ForceSyncState, ForceSyncStatus, write_status};

pub(super) async fn run_force_sync_inner(
    ctx: &ForceContext,
    _started: chrono::DateTime<chrono::Utc>,
) -> ForceSyncStatus {
    let config = &ctx.config;

    let pairs = super::force_pair_plan::plan_force_pairs(config, &ctx.state).await;

    let mut total_items = 0u64;
    for (src_idx, _, _, src_user_id, _) in &pairs {
        let source_client = ctx.clients[*src_idx].clone();
        if config.sync.force_played || config.sync.force_position {
            if let Ok(count) = source_client.get_user_played_items_count(src_user_id).await {
                total_items = total_items.saturating_add(count);
            }
        }
        if config.sync.force_favorites {
            if let Ok(count) = source_client
                .get_user_favorite_items_count(src_user_id)
                .await
            {
                total_items = total_items.saturating_add(count);
            }
        }
    }

    {
        let mut status = ctx.tracker.status.lock().await;
        status.total_pairs = if total_items > 0 {
            total_items
        } else {
            pairs.len() as u64
        };
        status.phase = Some("preparing".to_string());
    }

    info!(
        "force-sync starting: pairs={} rate={}/sec dry_run={} played={} position={} favorites={}",
        pairs.len(),
        rate_from_env(),
        ctx.dry_run,
        config.sync.force_played,
        config.sync.force_position,
        config.sync.force_favorites,
    );

    let rate = rate_from_env();
    let min_interval =
        Duration::from_micros(((1_000_000.0_f64 / rate as f64).round() as u64).max(1));
    let semaphore = Semaphore::new(rate.min(8) as usize);

    let mut status = ctx.tracker.status.lock().await.clone();
    let mut processed_total: u64 = 0;
    let mut succeeded_total: u64 = 0;
    let mut skipped_total: u64 = 0;
    let mut failed_total: u64 = 0;
    let mut errors: Vec<ForceSyncError> = Vec::new();

    let mut cancelled = false;

    // Phase: played history
    if config.sync.force_played || config.sync.force_position {
        status.phase = Some("played".to_string());
        write_status(&ctx.tracker, &status);
        {
            let mut st = ctx.state.lock().await;
            st.log_event("info", "Force sync: scanning played history");
        }
        for (src_idx, tgt_idx, src_username, src_user_id, tgt_user_id) in &pairs {
            if ctx.tracker.cancel.load(Ordering::SeqCst) {
                cancelled = true;
                break;
            }
            status.current_user = Some(src_username.clone());
            status.phase = Some("played".to_string());
            write_status(&ctx.tracker, &status);

            cancelled = force_sync_pair(
                *src_idx,
                *tgt_idx,
                src_username,
                src_user_id,
                tgt_user_id,
                ctx,
                &mut status,
                &mut processed_total,
                &mut succeeded_total,
                &mut skipped_total,
                &mut failed_total,
                &mut errors,
                &semaphore,
                min_interval,
            )
            .await;

            if cancelled {
                break;
            }
        }
    }

    // Phase: favorites
    if !cancelled && config.sync.force_favorites {
        status.phase = Some("favorites".to_string());
        write_status(&ctx.tracker, &status);
        {
            let mut st = ctx.state.lock().await;
            st.log_event("info", "Force sync: copying favorites");
        }
        for (src_idx, tgt_idx, src_username, src_user_id, tgt_user_id) in &pairs {
            if ctx.tracker.cancel.load(Ordering::SeqCst) {
                cancelled = true;
                break;
            }
            status.current_user = Some(src_username.clone());
            status.phase = Some("favorites".to_string());
            write_status(&ctx.tracker, &status);

            cancelled = force_sync_favorites_pair(
                *src_idx,
                *tgt_idx,
                src_username,
                src_user_id,
                tgt_user_id,
                ctx,
                &mut status,
                &mut processed_total,
                &mut succeeded_total,
                &mut skipped_total,
                &mut failed_total,
                &mut errors,
                &semaphore,
                min_interval,
            )
            .await;

            if cancelled {
                break;
            }
        }
    }

    let now = chrono::Utc::now();
    status.finished_at = Some(now.to_rfc3339());
    status.current_user = None;
    status.errors = errors.clone();
    status.phase = Some("finishing".to_string());
    if cancelled {
        status.state = ForceSyncState::Failed;
        status.last_error = Some("Sync cancelled by user".to_string());
    } else {
        status.state = if failed_total == 0 {
            ForceSyncState::Completed
        } else {
            ForceSyncState::Failed
        };
    }
    status.processed = processed_total;
    status.succeeded = succeeded_total;
    status.skipped = skipped_total;
    status.failed = failed_total;
    status.phase = Some(if cancelled {
        "cancelled".to_string()
    } else {
        "done".to_string()
    });
    write_status(&ctx.tracker, &status);

    {
        let mut st = ctx.state.lock().await;
        let level = if failed_total == 0 && !cancelled {
            "success"
        } else if cancelled {
            "warn"
        } else {
            "error"
        };
        let title = if ctx.dry_run {
            if cancelled {
                "Force preview cancelled"
            } else if failed_total == 0 {
                "Force preview finished (no writes)"
            } else {
                "Force preview finished with errors (no writes)"
            }
        } else if cancelled {
            "Force sync cancelled"
        } else if failed_total == 0 {
            "Force sync finished"
        } else {
            "Force sync finished with errors"
        };
        st.log_event_detail(
            level,
            title,
            Some(format!(
                "processed={} {}={} skipped={} failed={} | played ok={} skip={} fail={} | favorites ok={} skip={} fail={} | skips: already_equal={} no_provider={} no_match={} other={} | {}s",
                status.processed,
                if ctx.dry_run { "would_push" } else { "succeeded" },
                status.succeeded,
                status.skipped,
                status.failed,
                status.by_field.played.ok,
                status.by_field.played.skip,
                status.by_field.played.fail,
                status.by_field.favorite.ok,
                status.by_field.favorite.skip,
                status.by_field.favorite.fail,
                status.skip_reasons.already_equal,
                status.skip_reasons.no_provider,
                status.skip_reasons.no_match,
                status.skip_reasons.other,
                status
                    .started_at
                    .as_ref()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|s| (now - s.with_timezone(&chrono::Utc)).num_seconds())
                    .unwrap_or(0)
            )),
        );
    }

    info!(
        "force-sync {}: processed={} succeeded={} skipped={} failed={}",
        match status.state {
            ForceSyncState::Completed => "completed",
            ForceSyncState::Failed => "failed",
            _ => "ended",
        },
        status.processed,
        status.succeeded,
        status.skipped,
        status.failed
    );

    status
}
