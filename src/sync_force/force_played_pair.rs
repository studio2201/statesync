use super::force_constants::{FORCE_ITEM_CAP, FORCE_PAGE_TIMEOUT};
use super::{ForceContext, ForceSyncError, ForceSyncStatus, push_error, write_status};
use crate::client::PlayedItem;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

pub async fn force_sync_pair(
    src_idx: usize,
    tgt_idx: usize,
    src_username: &str,
    src_user_id: &str,
    tgt_user_id: &str,
    ctx: &ForceContext,
    status: &mut ForceSyncStatus,
    processed_total: &mut u64,
    succeeded_total: &mut u64,
    skipped_total: &mut u64,
    failed_total: &mut u64,
    errors: &mut Vec<ForceSyncError>,
    semaphore: &tokio::sync::Semaphore,
    min_interval: Duration,
) -> bool {
    if !ctx.config.sync.force_played && !ctx.config.sync.force_position {
        return false;
    }
    let source_client = ctx.clients[src_idx].clone();
    let _target_client = ctx.clients[tgt_idx].clone();
    let page_size: usize = 500;
    let force_played = ctx.config.sync.force_played;
    let force_position = ctx.config.sync.force_position;
    let dry_run = ctx.dry_run;
    let mut last_status_write = Instant::now() - Duration::from_secs(1);

    let mut page: usize = 0;
    let mut cancelled = false;
    loop {
        if ctx.tracker.cancel.load(Ordering::SeqCst) {
            cancelled = true;
            break;
        }
        if page * page_size >= FORCE_ITEM_CAP {
            tracing::warn!(
                "force-sync reached {} item cap; stopping at user {} on {}",
                FORCE_ITEM_CAP,
                src_user_id,
                ctx.config.servers[src_idx].name
            );
            break;
        }
        let items_res = tokio::time::timeout(
            FORCE_PAGE_TIMEOUT,
            source_client.get_user_played_items(src_user_id, page * page_size, page_size),
        )
        .await;
        let items: Vec<PlayedItem> = match items_res {
            Ok(Ok(items)) => items,
            Ok(Err(e)) => {
                push_error(
                    errors,
                    status,
                    ForceSyncError {
                        user: src_user_id.to_string(),
                        server: ctx.config.servers[src_idx].name.clone(),
                        item_id: None,
                        provider: None,
                        message: format!("list failed: {}", e),
                    },
                );
                *failed_total += 1;
                status.by_field.played.fail += 1;
                write_status(&ctx.tracker, status);
                break;
            }
            Err(_) => {
                push_error(
                    errors,
                    status,
                    ForceSyncError {
                        user: src_user_id.to_string(),
                        server: ctx.config.servers[src_idx].name.clone(),
                        item_id: None,
                        provider: None,
                        message: format!("list timeout after {:?}", FORCE_PAGE_TIMEOUT),
                    },
                );
                *failed_total += 1;
                status.by_field.played.fail += 1;
                write_status(&ctx.tracker, status);
                break;
            }
        };
        if items.is_empty() {
            break;
        }
        let mut cancelled_flag = false;
        super::force_played_items::process_played_items_batch(
            items,
            src_idx,
            tgt_idx,
            src_username,
            src_user_id,
            tgt_user_id,
            ctx,
            status,
            processed_total,
            succeeded_total,
            skipped_total,
            failed_total,
            errors,
            semaphore,
            min_interval,
            force_played,
            force_position,
            dry_run,
            &mut last_status_write,
            &mut cancelled_flag,
        )
        .await;
        if cancelled_flag {
            cancelled = true;
            break;
        }

        if cancelled {
            break;
        }
        page += 1;
    }
    write_status(&ctx.tracker, status);
    cancelled
}
