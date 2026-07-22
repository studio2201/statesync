use super::helpers::publish_counts;
use super::{ForceContext, ForceSyncError, ForceSyncStatus, push_error, write_status};
use crate::client::PlayedItem;
use crate::state::SyncHistoryValue;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::force_constants::{FORCE_ITEM_CAP, FORCE_PAGE_TIMEOUT, FORCE_UPDATE_TIMEOUT};

pub async fn force_sync_favorites_pair(
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
    if !ctx.config.sync.force_favorites {
        return false;
    }
    let source_client = ctx.clients[src_idx].clone();
    let target_client = ctx.clients[tgt_idx].clone();
    let page_size: usize = 500;
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
            break;
        }
        let items_res = tokio::time::timeout(
            FORCE_PAGE_TIMEOUT,
            source_client.get_user_favorite_items(src_user_id, page * page_size, page_size),
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
                        message: format!("favorites list failed: {}", e),
                    },
                );
                *failed_total += 1;
                status.by_field.favorite.fail += 1;
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
                        message: format!("favorites list timeout after {:?}", FORCE_PAGE_TIMEOUT),
                    },
                );
                *failed_total += 1;
                status.by_field.favorite.fail += 1;
                write_status(&ctx.tracker, status);
                break;
            }
        };
        if items.is_empty() {
            break;
        }
        for item in items {
            if ctx.tracker.cancel.load(Ordering::SeqCst) {
                cancelled = true;
                break;
            }
            let started_item = Instant::now();
            let providers = item.provider_ids();
            if providers.is_empty() {
                *skipped_total += 1;
                *processed_total += 1;
                status.by_field.favorite.skip += 1;
                status.skip_reasons.no_provider += 1;
                publish_counts(
                    &ctx.tracker,
                    status,
                    *processed_total,
                    *succeeded_total,
                    *skipped_total,
                    *failed_total,
                    &mut last_status_write,
                    false,
                );
                continue;
            }
            let permit = semaphore.acquire().await;
            let target_name = ctx.config.servers[tgt_idx].name.clone();
            let target_item_id = crate::sync::resolve::resolve_target_item(
                tgt_idx,
                &providers,
                &target_name,
                Some(tgt_user_id),
                &target_client,
                &ctx.state,
            )
            .await;
            let target_item_id = match target_item_id {
                Some(id) => id,
                None => {
                    drop(permit);
                    *skipped_total += 1;
                    *processed_total += 1;
                    status.by_field.favorite.skip += 1;
                    status.skip_reasons.no_match += 1;
                    publish_counts(
                        &ctx.tracker,
                        status,
                        *processed_total,
                        *succeeded_total,
                        *skipped_total,
                        *failed_total,
                        &mut last_status_write,
                        false,
                    );
                    continue;
                }
            };
            // Already favorited on target → skip write.
            if let Ok(tgt_ud) = target_client
                .get_item_user_data(tgt_user_id, &target_item_id)
                .await
            {
                if tgt_ud.is_favorite == Some(true) {
                    drop(permit);
                    // No write → skip min_interval pacing (equal libraries must not stall).
                    *skipped_total += 1;
                    *processed_total += 1;
                    status.by_field.favorite.skip += 1;
                    status.skip_reasons.already_equal += 1;
                    publish_counts(
                        &ctx.tracker,
                        status,
                        *processed_total,
                        *succeeded_total,
                        *skipped_total,
                        *failed_total,
                        &mut last_status_write,
                        false,
                    );
                    continue;
                }
            }
            let update_res = if dry_run {
                Ok(Ok(()))
            } else {
                tokio::time::timeout(
                    FORCE_UPDATE_TIMEOUT,
                    target_client.update_favorite(tgt_user_id, &target_item_id, true),
                )
                .await
            };
            match update_res {
                Ok(Ok(())) => {
                    if !dry_run {
                        if let Some(hk) = providers.history_key() {
                            let key = (src_username.to_lowercase(), hk);
                            let mut st = ctx.state.lock().await;
                            let prev = st.last_syncs.get(&key).cloned();
                            st.last_syncs.insert(
                                key,
                                SyncHistoryValue {
                                    position_ticks: prev
                                        .as_ref()
                                        .map(|p| p.position_ticks)
                                        .unwrap_or(0),
                                    timestamp: Instant::now(),
                                    played: prev.as_ref().map(|p| p.played).unwrap_or(false),
                                    favorite: Some(true),
                                },
                            );
                            drop(st);
                        }
                    }
                    *succeeded_total += 1;
                    *processed_total += 1;
                    status.by_field.favorite.ok += 1;
                }
                Ok(Err(e)) => {
                    push_error(
                        errors,
                        status,
                        ForceSyncError {
                            user: src_user_id.to_string(),
                            server: ctx.config.servers[tgt_idx].name.clone(),
                            item_id: Some(target_item_id),
                            provider: providers.history_key(),
                            message: e.to_string(),
                        },
                    );
                    *failed_total += 1;
                    *processed_total += 1;
                    status.by_field.favorite.fail += 1;
                }
                Err(_) => {
                    push_error(
                        errors,
                        status,
                        ForceSyncError {
                            user: src_user_id.to_string(),
                            server: ctx.config.servers[tgt_idx].name.clone(),
                            item_id: Some(target_item_id),
                            provider: providers.history_key(),
                            message: format!(
                                "favorite update timeout after {:?}",
                                FORCE_UPDATE_TIMEOUT
                            ),
                        },
                    );
                    *failed_total += 1;
                    *processed_total += 1;
                    status.by_field.favorite.fail += 1;
                }
            }
            drop(permit);
            let elapsed = started_item.elapsed();
            if elapsed < min_interval {
                tokio::time::sleep(min_interval - elapsed).await;
            }
            publish_counts(
                &ctx.tracker,
                status,
                *processed_total,
                *succeeded_total,
                *skipped_total,
                *failed_total,
                &mut last_status_write,
                false,
            );
        }
        if cancelled {
            break;
        }
        page += 1;
    }
    write_status(&ctx.tracker, status);
    cancelled
}
