use super::force_constants::FORCE_UPDATE_TIMEOUT;
use super::helpers::publish_counts;
use super::{ForceContext, ForceSyncError, ForceSyncStatus, push_error};
use crate::client::PlayedItem;
use crate::state::SyncHistoryValue;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

#[allow(clippy::too_many_arguments)]
pub async fn process_played_items_batch(
    items: Vec<PlayedItem>,
    _src_idx: usize,
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
    force_played: bool,
    force_position: bool,
    dry_run: bool,
    last_status_write: &mut Instant,
    cancelled: &mut bool,
) {
    let target_client = ctx.clients[tgt_idx].clone();
    for item in items {
        if ctx.tracker.cancel.load(Ordering::SeqCst) {
            *cancelled = true;
            break;
        }
        let started_item = Instant::now();
        let providers = item.provider_ids();
        if providers.is_empty() {
            *skipped_total += 1;
            *processed_total += 1;
            status.by_field.played.skip += 1;
            status.skip_reasons.no_provider += 1;
            publish_counts(
                &ctx.tracker,
                status,
                *processed_total,
                *succeeded_total,
                *skipped_total,
                *failed_total,
                last_status_write,
                false,
            );
            continue;
        }
        // Hold concurrency slot only for network work — not for rate-limit sleep.
        let permit = semaphore.acquire().await;
        // Cache-first (in-memory ServerCache), then HTTP only on miss.
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
                status.by_field.played.skip += 1;
                status.skip_reasons.no_match += 1;
                publish_counts(
                    &ctx.tracker,
                    status,
                    *processed_total,
                    *succeeded_total,
                    *skipped_total,
                    *failed_total,
                    last_status_write,
                    false,
                );
                continue;
            }
        };
        let source_pos = item.playback_position_ticks.unwrap_or(0);
        let write_pos = if force_position {
            Some(source_pos)
        } else {
            None
        };
        let write_played = if force_played { Some(true) } else { None };
        if write_pos.is_none() && write_played.is_none() {
            drop(permit);
            *skipped_total += 1;
            *processed_total += 1;
            status.skip_reasons.other += 1;
            publish_counts(
                &ctx.tracker,
                status,
                *processed_total,
                *succeeded_total,
                *skipped_total,
                *failed_total,
                last_status_write,
                false,
            );
            continue;
        }
        if let Ok(tgt_ud) = target_client
            .get_item_user_data(tgt_user_id, &target_item_id)
            .await
        {
            if super::force_equal::played_state_already_equal(
                force_played,
                force_position,
                source_pos,
                &tgt_ud,
            ) {
                drop(permit);
                // No write → do not apply min_interval sleep (would stall large equal libraries).
                *skipped_total += 1;
                *processed_total += 1;
                status.by_field.played.skip += 1;
                status.skip_reasons.already_equal += 1;
                publish_counts(
                    &ctx.tracker,
                    status,
                    *processed_total,
                    *succeeded_total,
                    *skipped_total,
                    *failed_total,
                    last_status_write,
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
                target_client.update_user_data(
                    tgt_user_id,
                    &target_item_id,
                    write_pos,
                    write_played,
                    None,
                ),
            )
            .await
        };
        match update_res {
            Ok(Ok(())) => {
                if !dry_run {
                    if let Some(hk) = providers.history_key() {
                        let key = (src_username.to_lowercase(), hk);
                        let mut st = ctx.state.lock().await;
                        let prev_fav = st.last_syncs.get(&key).and_then(|v| v.favorite);
                        st.last_syncs.insert(
                            key,
                            SyncHistoryValue {
                                position_ticks: source_pos,
                                timestamp: Instant::now(),
                                played: true,
                                favorite: prev_fav,
                            },
                        );
                        drop(st);
                    }
                }
                *succeeded_total += 1;
                *processed_total += 1;
                status.by_field.played.ok += 1;
                if force_position && source_pos > 0 {
                    status.by_field.position.ok += 1;
                }
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
                status.by_field.played.fail += 1;
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
                        message: format!("update timeout after {:?}", FORCE_UPDATE_TIMEOUT),
                    },
                );
                *failed_total += 1;
                *processed_total += 1;
                status.by_field.played.fail += 1;
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
            last_status_write,
            false,
        );
    }
}
