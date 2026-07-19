use crate::client::{MediaClient, PlayedItem};
use crate::config::Config;
use crate::state::{AppState, SyncHistoryValue};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForceSyncOptions {
    #[serde(default)]
    pub direction: Direction,
}

/// Default rate: 5 items/sec, matching prior backfill behavior.
fn rate_from_env() -> u32 {
    std::env::var("STATESYNC_FORCE_RATE")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .map(|v| v.clamp(1, 50))
        .unwrap_or(5)
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Direction {
    EmbyToJellyfin,
    JellyfinToEmby,
    #[default]
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ForceSyncState {
    Idle,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForceSyncError {
    pub user: String,
    pub server: String,
    pub item_id: Option<String>,
    pub provider: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForceSyncStatus {
    pub state: ForceSyncState,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub direction: Option<Direction>,
    pub total_pairs: u64,
    pub processed: u64,
    pub succeeded: u64,
    pub skipped: u64,
    pub failed: u64,
    pub current_user: Option<String>,
    pub last_error: Option<String>,
    pub errors: Vec<ForceSyncError>,
}

impl ForceSyncStatus {
    pub fn idle() -> Self {
        Self {
            state: ForceSyncState::Idle,
            started_at: None,
            finished_at: None,
            direction: None,
            total_pairs: 0,
            processed: 0,
            succeeded: 0,
            skipped: 0,
            failed: 0,
            current_user: None,
            last_error: None,
            errors: Vec::new(),
        }
    }
}

impl Default for ForceSyncStatus {
    fn default() -> Self {
        Self::idle()
    }
}

#[derive(Default)]
pub struct SyncForceTracker {
    pub status: Mutex<ForceSyncStatus>,
    pub running: Mutex<bool>,
    pub force_sync_in_progress: std::sync::atomic::AtomicBool,
}

pub struct ForceContext {
    pub config: Config,
    pub clients: Vec<Arc<MediaClient>>,
    pub state: Arc<Mutex<AppState>>,
    pub tracker: Arc<SyncForceTracker>,
    pub direction: Direction,
}

pub async fn run_force_sync(ctx: ForceContext) -> ForceSyncStatus {
    {
        let mut running = ctx.tracker.running.lock().await;
        if *running {
            return ctx.tracker.status.lock().await.clone();
        }
        *running = true;
    }
    ctx.tracker
        .force_sync_in_progress
        .store(true, Ordering::SeqCst);

    let started = chrono::Utc::now();
    {
        let mut status = ctx.tracker.status.lock().await;
        *status = ForceSyncStatus {
            state: ForceSyncState::Running,
            started_at: Some(started.to_rfc3339()),
            finished_at: None,
            direction: Some(ctx.direction),
            total_pairs: 0,
            processed: 0,
            succeeded: 0,
            skipped: 0,
            failed: 0,
            current_user: None,
            last_error: None,
            errors: Vec::new(),
        };
    }

    let result = run_force_sync_inner(&ctx, started).await;

    ctx.tracker
        .force_sync_in_progress
        .store(false, Ordering::SeqCst);
    {
        let mut running = ctx.tracker.running.lock().await;
        *running = false;
    }

    let mut status = ctx.tracker.status.lock().await;
    *status = result;
    status.clone()
}

const FORCE_ERROR_CAP: usize = 100;
const FORCE_PAGE_TIMEOUT: Duration = Duration::from_secs(60);
const FORCE_UPDATE_TIMEOUT: Duration = Duration::from_secs(30);
const FORCE_ITEM_CAP: usize = 100_000;

async fn run_force_sync_inner(
    ctx: &ForceContext,
    _started: chrono::DateTime<chrono::Utc>,
) -> ForceSyncStatus {
    let ForceContext {
        config,
        clients,
        state,
        tracker,
        direction,
    } = ctx;

    let is_emby: Vec<bool> = config.servers.iter().map(|s| s.is_emby).collect();

    // Determine source/target eligibility per direction.
    let sources: Vec<usize> = (0..config.servers.len())
        .filter(|&i| config.servers[i].sync_direction != "receive")
        .filter(|&i| match direction {
            Direction::EmbyToJellyfin => is_emby.get(i).copied().unwrap_or(false),
            Direction::JellyfinToEmby => !is_emby.get(i).copied().unwrap_or(false),
            Direction::Both => true,
        })
        .collect();
    let targets: Vec<usize> = (0..config.servers.len())
        .filter(|&i| config.servers[i].sync_direction != "send")
        .filter(|&i| match direction {
            Direction::EmbyToJellyfin => !is_emby.get(i).copied().unwrap_or(false),
            Direction::JellyfinToEmby => is_emby.get(i).copied().unwrap_or(false),
            Direction::Both => true,
        })
        .collect();

    // Collect (source_user_id, target_user_id) pairs.
    let pairs: Vec<(usize, usize, String, String)> = {
        let state_guard = state.lock().await;
        let mut result = Vec::new();
        for &src in &sources {
            let cache = match state_guard.caches.get(src) {
                Some(c) => c,
                None => continue,
            };
            for (username, src_user_id) in &cache.users {
                // Map to target user via existing logic.
                for &tgt in &targets {
                    if src == tgt {
                        continue;
                    }
                    if let Some(tgt_cache) = state_guard.caches.get(tgt) {
                        if let Some(tgt_id) = crate::state::find_mapped_user_id(
                            username,
                            &tgt_cache.users,
                            &config.user_mappings,
                        ) {
                            result.push((src, tgt, src_user_id.clone(), tgt_id));
                        }
                    }
                }
            }
        }
        result
    };

    {
        let mut status = tracker.status.lock().await;
        status.total_pairs = pairs.len() as u64;
    }

    info!(
        "force-sync starting: direction={:?}, pairs={}, rate={}/sec",
        direction,
        pairs.len(),
        rate_from_env()
    );

    let rate = rate_from_env();
    let min_interval =
        Duration::from_micros(((1_000_000.0_f64 / rate as f64).round() as u64).max(1));
    let semaphore = Semaphore::new(rate.min(8) as usize);

    let mut status = tracker.status.lock().await.clone();
    let mut processed_total: u64 = 0;
    let mut succeeded_total: u64 = 0;
    let mut skipped_total: u64 = 0;
    let mut failed_total: u64 = 0;
    let mut errors: Vec<ForceSyncError> = Vec::new();

    for (src_idx, tgt_idx, src_user_id, tgt_user_id) in &pairs {
        status.current_user = Some(src_user_id.clone());
        write_status(tracker, &status);

        let source_client = clients[*src_idx].clone();
        let target_client = clients[*tgt_idx].clone();
        let page_size: usize = 500;

        let mut page: usize = 0;
        loop {
            if page * page_size >= FORCE_ITEM_CAP {
                warn!(
                    "force-sync reached {} item cap; stopping at user {} on {}",
                    FORCE_ITEM_CAP, src_user_id, config.servers[*src_idx].name
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
                        &mut errors,
                        &mut status,
                        ForceSyncError {
                            user: src_user_id.clone(),
                            server: config.servers[*src_idx].name.clone(),
                            item_id: None,
                            provider: None,
                            message: format!("list failed: {}", e),
                        },
                    );
                    failed_total += 1;
                    write_status(tracker, &status);
                    break;
                }
                Err(_) => {
                    push_error(
                        &mut errors,
                        &mut status,
                        ForceSyncError {
                            user: src_user_id.clone(),
                            server: config.servers[*src_idx].name.clone(),
                            item_id: None,
                            provider: None,
                            message: format!("list timeout after {:?}", FORCE_PAGE_TIMEOUT),
                        },
                    );
                    failed_total += 1;
                    write_status(tracker, &status);
                    break;
                }
            };
            if items.is_empty() {
                break;
            }
            for item in items {
                let started_item = Instant::now();
                let _permit = semaphore.acquire().await;

                let imdb = item.imdb_id.clone().unwrap_or_default();
                let tmdb = item.tmdb_id.clone().unwrap_or_default();
                if imdb.is_empty() && tmdb.is_empty() {
                    skipped_total += 1;
                    processed_total += 1;
                    continue;
                }
                // Resolve target item.
                let resolved = target_client
                    .find_item_by_provider(tgt_user_id, &imdb, &tmdb)
                    .await
                    .ok()
                    .flatten();
                let target_item_id = match resolved {
                    Some((id, _i, _t)) => id,
                    None => {
                        skipped_total += 1;
                        processed_total += 1;
                        continue;
                    }
                };
                // Update target with source-wins policy.
                let source_pos = item.playback_position_ticks.unwrap_or(0);
                let update_res = tokio::time::timeout(
                    FORCE_UPDATE_TIMEOUT,
                    target_client.update_progress(tgt_user_id, &target_item_id, source_pos, true),
                )
                .await;
                match update_res {
                    Ok(Ok(())) => {
                        let key = (
                            src_user_id.to_lowercase(),
                            if !imdb.is_empty() {
                                imdb.clone()
                            } else {
                                tmdb.clone()
                            },
                        );
                        let mut st = state.lock().await;
                        st.last_syncs.insert(
                            key,
                            SyncHistoryValue {
                                position_ticks: source_pos,
                                timestamp: Instant::now(),
                            },
                        );
                        drop(st);
                        succeeded_total += 1;
                        processed_total += 1;
                    }
                    Ok(Err(e)) => {
                        push_error(
                            &mut errors,
                            &mut status,
                            ForceSyncError {
                                user: src_user_id.clone(),
                                server: config.servers[*tgt_idx].name.clone(),
                                item_id: Some(target_item_id),
                                provider: if !imdb.is_empty() {
                                    Some(imdb)
                                } else {
                                    Some(tmdb)
                                },
                                message: e.to_string(),
                            },
                        );
                        failed_total += 1;
                        processed_total += 1;
                    }
                    Err(_) => {
                        push_error(
                            &mut errors,
                            &mut status,
                            ForceSyncError {
                                user: src_user_id.clone(),
                                server: config.servers[*tgt_idx].name.clone(),
                                item_id: Some(target_item_id),
                                provider: if !imdb.is_empty() {
                                    Some(imdb)
                                } else {
                                    Some(tmdb)
                                },
                                message: format!("update timeout after {:?}", FORCE_UPDATE_TIMEOUT),
                            },
                        );
                        failed_total += 1;
                        processed_total += 1;
                    }
                }
                let elapsed = started_item.elapsed();
                if elapsed < min_interval {
                    tokio::time::sleep(min_interval - elapsed).await;
                }
                status.processed = processed_total;
                status.succeeded = succeeded_total;
                status.skipped = skipped_total;
                status.failed = failed_total;
                write_status(tracker, &status);
            }
            page += 1;
        }
    }

    let now = chrono::Utc::now();
    status.finished_at = Some(now.to_rfc3339());
    status.current_user = None;
    status.errors = errors.clone();
    status.state = if failed_total == 0 {
        ForceSyncState::Completed
    } else {
        ForceSyncState::Failed
    };
    status.processed = processed_total;
    status.succeeded = succeeded_total;
    status.skipped = skipped_total;
    status.failed = failed_total;
    write_status(tracker, &status);

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

fn write_status(tracker: &SyncForceTracker, status: &ForceSyncStatus) {
    if let Ok(mut guard) = tracker.status.try_lock() {
        *guard = status.clone();
    }
}

fn push_error(errors: &mut Vec<ForceSyncError>, status: &mut ForceSyncStatus, err: ForceSyncError) {
    if errors.len() >= FORCE_ERROR_CAP {
        let drain_to = errors.len() - FORCE_ERROR_CAP + 1;
        errors.drain(0..drain_to);
    }
    errors.push(err.clone());
    status.last_error = Some(err.message);
    status.errors = errors.clone();
}

pub async fn snapshot_status(tracker: &SyncForceTracker) -> ForceSyncStatus {
    tracker.status.lock().await.clone()
}

pub fn direction_from_env() -> Direction {
    match std::env::var("STATESYNC_FORCE_DIRECTION")
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "emby-to-jellyfin" => Direction::EmbyToJellyfin,
        "jellyfin-to-emby" => Direction::JellyfinToEmby,
        _ => Direction::Both,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            std::env::remove_var("STATESYNC_FORCE_RATE");
        }
        assert_eq!(rate_from_env(), 5);
    }

    #[test]
    fn idle_status_is_clean() {
        let s = ForceSyncStatus::idle();
        assert_eq!(s.state, ForceSyncState::Idle);
        assert_eq!(s.processed, 0);
        assert!(s.errors.is_empty());
    }

    #[test]
    fn idle_status_has_no_finished_at() {
        let s = ForceSyncStatus::idle();
        assert!(s.started_at.is_none());
        assert!(s.finished_at.is_none());
        assert!(s.processed == 0);
    }

    #[test]
    fn errors_capped_at_limit() {
        let mut errors = Vec::new();
        let mut status = ForceSyncStatus::idle();
        for i in 0..(FORCE_ERROR_CAP + 50) {
            push_error(
                &mut errors,
                &mut status,
                ForceSyncError {
                    user: format!("u{}", i),
                    server: "s".into(),
                    item_id: None,
                    provider: None,
                    message: format!("err {}", i),
                },
            );
        }
        assert_eq!(errors.len(), FORCE_ERROR_CAP);
        assert_eq!(errors[0].user, "u50");
    }
}
