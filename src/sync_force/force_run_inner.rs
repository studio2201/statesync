use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::info;

use super::force_story::{
    apply_story, story_counting, story_favorites, story_finished, story_played,
};
use super::runner::rate_from_env;
use super::sync_loop::{force_sync_favorites_pair, force_sync_pair};
use super::{ForceContext, ForceSyncError, ForceSyncState, ForceSyncStatus, write_status};

pub(super) async fn run_force_sync_inner(
    ctx: &ForceContext,
    _started: chrono::DateTime<chrono::Utc>,
) -> ForceSyncStatus {
    let config = &ctx.config;

    let pairs =
        super::force_pair_plan::plan_force_pairs(config, &ctx.state, ctx.only_user.as_deref())
            .await;
    let pair_n = pairs.len() as u64;

    let mut total_items = 0u64;
    for (i, (src_idx, _, src_username, src_user_id, _)) in pairs.iter().enumerate() {
        let src_name = config.servers[*src_idx].name.as_str();
        let (h, d) = story_counting(src_username, src_name, (i as u64) + 1, pair_n.max(1));
        {
            let mut st = ctx
                .tracker
                .status
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            apply_story(
                &mut st,
                "preparing",
                h,
                d,
                Some(src_username),
                Some(src_name),
                None,
                (i as u64) + 1,
                pair_n,
            );
            st.total_pairs = total_items.max(pair_n);
        }
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
        let mut status = ctx
            .tracker
            .status
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        status.total_pairs = if total_items > 0 {
            total_items
        } else {
            pair_n
        };
        status.phase = Some("preparing".to_string());
        if pairs.is_empty() {
            apply_story(
                &mut status,
                "preparing",
                "Nothing to force-sync",
                "No person×server routes were built. Check that servers are connected, people are linked (same name or Link users), and ignore/allow lists are not excluding everyone.",
                None,
                None,
                None,
                0,
                0,
            );
        }
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

    let mut status = ctx.tracker.snapshot_status();
    let mut processed_total: u64 = 0;
    let mut succeeded_total: u64 = 0;
    let mut skipped_total: u64 = 0;
    let mut failed_total: u64 = 0;
    let mut errors: Vec<ForceSyncError> = Vec::new();

    let mut cancelled = false;

    if config.sync.force_played || config.sync.force_position {
        {
            let mut st = ctx.state.lock().await;
            st.log_event(
                "info",
                "Force sync: starting watched-history pass (all linked routes)",
            );
        }
        for (i, (src_idx, tgt_idx, src_username, src_user_id, tgt_user_id)) in
            pairs.iter().enumerate()
        {
            if ctx.tracker.cancel.load(Ordering::SeqCst) {
                cancelled = true;
                break;
            }
            let src_name = config.servers[*src_idx].name.as_str();
            let tgt_name = config.servers[*tgt_idx].name.as_str();
            let pair_i = (i as u64) + 1;
            let (h, d) = story_played(
                src_username,
                src_name,
                tgt_name,
                pair_i,
                pair_n.max(1),
                ctx.dry_run,
            );
            apply_story(
                &mut status,
                "played",
                h.clone(),
                d.clone(),
                Some(src_username),
                Some(src_name),
                Some(tgt_name),
                pair_i,
                pair_n,
            );
            write_status(&ctx.tracker, &status);
            {
                let mut st = ctx.state.lock().await;
                st.log_event_detail("info", &h, Some(d));
            }

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

    if !cancelled && config.sync.force_favorites {
        {
            let mut st = ctx.state.lock().await;
            st.log_event(
                "info",
                "Force sync: starting favorites pass (all linked routes)",
            );
        }
        for (i, (src_idx, tgt_idx, src_username, src_user_id, tgt_user_id)) in
            pairs.iter().enumerate()
        {
            if ctx.tracker.cancel.load(Ordering::SeqCst) {
                cancelled = true;
                break;
            }
            let src_name = config.servers[*src_idx].name.as_str();
            let tgt_name = config.servers[*tgt_idx].name.as_str();
            let pair_i = (i as u64) + 1;
            let (h, d) = story_favorites(
                src_username,
                src_name,
                tgt_name,
                pair_i,
                pair_n.max(1),
                ctx.dry_run,
            );
            apply_story(
                &mut status,
                "favorites",
                h.clone(),
                d.clone(),
                Some(src_username),
                Some(src_name),
                Some(tgt_name),
                pair_i,
                pair_n,
            );
            write_status(&ctx.tracker, &status);
            {
                let mut st = ctx.state.lock().await;
                st.log_event_detail("info", &h, Some(d));
            }

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
    status.errors = errors.clone();
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
    let (fh, fd) = story_finished(
        cancelled,
        ctx.dry_run,
        failed_total,
        processed_total,
        succeeded_total,
        skipped_total,
    );
    apply_story(
        &mut status,
        if cancelled { "cancelled" } else { "done" },
        fh.clone(),
        fd.clone(),
        None,
        None,
        None,
        pair_n,
        pair_n,
    );
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
        st.log_event_detail(
            level,
            &fh,
            Some(format!(
                "{fd} | played ok={} skip={} fail={} | favorites ok={} skip={} fail={} | already_equal={} no_provider={} no_match={} other={} | {}s",
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
