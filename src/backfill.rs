use crate::client::MediaClient;
use crate::config::Config;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tracing::{info, warn};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Direction {
    EmbyToJellyfin,
    JellyfinToEmby,
    #[default]
    Both,
}

impl Direction {
    pub fn from_env() -> Self {
        match std::env::var("STATESYNC_BACKFILL_DIRECTION")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "emby-to-jellyfin" => Direction::EmbyToJellyfin,
            "jellyfin-to-emby" => Direction::JellyfinToEmby,
            _ => Direction::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MergePolicy {
    #[default]
    Max,
    SourceWins,
    Newest,
}

impl MergePolicy {
    pub fn from_env() -> Self {
        match std::env::var("STATESYNC_BACKFILL_MERGE")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "max" => MergePolicy::Max,
            "newest" => MergePolicy::Newest,
            "source-wins" => MergePolicy::SourceWins,
            _ => MergePolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    Played,
    Resumable,
    #[default]
    All,
}

impl Scope {
    pub fn from_env() -> Self {
        match std::env::var("STATESYNC_BACKFILL_SCOPE")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "played" => Scope::Played,
            "resumable" => Scope::Resumable,
            _ => Scope::All,
        }
    }

    pub fn jellyfin_filter(self) -> &'static str {
        match self {
            Scope::Played => "IsPlayed",
            Scope::Resumable => "IsResumable",
            Scope::All => "IsPlayedOrResumable",
        }
    }

    pub fn emby_filter(self) -> &'static str {
        match self {
            Scope::Played => "IsPlayed=true",
            Scope::Resumable => "IsResumable=true",
            Scope::All => "IsPlayedOrResumable=true",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackfillOptions {
    #[serde(default)]
    pub direction: Direction,
    #[serde(default)]
    pub merge: MergePolicy,
    #[serde(default = "default_rate")]
    pub rate: u32,
    #[serde(default)]
    pub scope: Scope,
    #[serde(default)]
    pub force: bool,
}

fn default_rate() -> u32 {
    5
}

impl Default for BackfillOptions {
    fn default() -> Self {
        Self {
            direction: Direction::default(),
            merge: MergePolicy::default(),
            rate: default_rate(),
            scope: Scope::default(),
            force: false,
        }
    }
}

impl BackfillOptions {
    pub fn from_env_or_default() -> Self {
        Self {
            direction: Direction::from_env(),
            merge: MergePolicy::from_env(),
            rate: std::env::var("STATESYNC_BACKFILL_RATE")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .map(|v| v.clamp(1, 50))
                .unwrap_or(5),
            scope: Scope::from_env(),
            force: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BackfillState {
    Idle,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackfillError {
    pub user: String,
    pub server: String,
    pub item_id: Option<String>,
    pub provider: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackfillStatus {
    pub state: BackfillState,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub options: Option<BackfillOptions>,
    pub total_pairs: u64,
    pub processed: u64,
    pub succeeded: u64,
    pub skipped: u64,
    pub failed: u64,
    pub current_pair: Option<String>,
    pub last_error: Option<String>,
    pub errors: Vec<BackfillError>,
}

impl BackfillStatus {
    pub fn idle() -> Self {
        Self {
            state: BackfillState::Idle,
            started_at: None,
            finished_at: None,
            options: None,
            total_pairs: 0,
            processed: 0,
            succeeded: 0,
            skipped: 0,
            failed: 0,
            current_pair: None,
            last_error: None,
            errors: Vec::new(),
        }
    }
}

pub struct BackfillTracker {
    pub status: Mutex<BackfillStatus>,
    pub running: Mutex<bool>,
    pub cancel: Mutex<bool>,
}

impl Default for BackfillTracker {
    fn default() -> Self {
        Self {
            status: Mutex::new(BackfillStatus::idle()),
            running: Mutex::new(false),
            cancel: Mutex::new(false),
        }
    }
}

pub const BACKFILL_ERROR_CAP: usize = 100;
pub const BACKFILL_MAX_ITEMS: usize = 100_000;
pub const BACKFILL_PAGE_TIMEOUT: Duration = Duration::from_secs(60);
pub const BACKFILL_UPDATE_TIMEOUT: Duration = Duration::from_secs(30);
pub const STATUS_WRITE_INTERVAL: Duration = Duration::from_millis(500);

pub struct BackfillContext {
    pub config: Config,
    pub clients: Vec<Arc<MediaClient>>,
    pub state: Arc<Mutex<AppState>>,
    pub tracker: Arc<BackfillTracker>,
    pub options: BackfillOptions,
    pub server_names: Vec<String>,
}

pub async fn run_backfill(ctx: BackfillContext) -> BackfillStatus {
    {
        let running = ctx.tracker.running.lock().await;
        if *running {
            drop(running);
            return ctx.tracker.status.lock().await.clone();
        }
    }
    {
        let mut running = ctx.tracker.running.lock().await;
        *running = true;
    }
    {
        let mut cancel = ctx.tracker.cancel.lock().await;
        *cancel = false;
    }

    let started = chrono::Utc::now();
    {
        let mut status = ctx.tracker.status.lock().await;
        *status = BackfillStatus {
            state: BackfillState::Running,
            started_at: Some(started.to_rfc3339()),
            finished_at: None,
            options: Some(ctx.options.clone()),
            total_pairs: 0,
            processed: 0,
            succeeded: 0,
            skipped: 0,
            failed: 0,
            current_pair: None,
            last_error: None,
            errors: Vec::new(),
        };
    }

    let result = run_backfill_inner(&ctx, started).await;

    let mut running = ctx.tracker.running.lock().await;
    *running = false;
    drop(running);

    let mut status = ctx.tracker.status.lock().await;
    *status = result;
    status.clone()
}

async fn run_backfill_inner(
    ctx: &BackfillContext,
    started: chrono::DateTime<chrono::Utc>,
) -> BackfillStatus {
    let BackfillContext {
        config,
        clients,
        state,
        tracker,
        options,
        server_names,
    } = ctx;

    let is_emby: Vec<bool> = config.servers.iter().map(|s| s.is_emby).collect();
    let pairs: Vec<(usize, usize)> = build_pairs(options.direction, &config.servers, &is_emby);

    let user_lists: Vec<((usize, usize), Vec<String>)> =
        collect_users_per_pair(state, &pairs).await;
    let total_pairs: u64 = user_lists.iter().map(|(_, users)| users.len() as u64).sum();
    let initial_status = BackfillStatus {
        state: BackfillState::Running,
        started_at: Some(started.to_rfc3339()),
        finished_at: None,
        options: Some(options.clone()),
        total_pairs,
        processed: 0,
        succeeded: 0,
        skipped: 0,
        failed: 0,
        current_pair: None,
        last_error: None,
        errors: Vec::new(),
    };
    write_status(tracker, &initial_status).await;

    info!(
        "Starting backfill: direction={:?}, merge={:?}, scope={:?}, force={}, rate={}/s, total_pairs={}",
        options.direction, options.merge, options.scope, options.force, options.rate, total_pairs
    );

    let effective_merge = if options.force {
        MergePolicy::SourceWins
    } else {
        options.merge
    };
    let min_interval = Duration::from_micros(
        ((1_000_000.0_f64 / options.rate.max(1) as f64).round() as u64).max(1),
    );
    let semaphore = Semaphore::new(options.rate.min(8) as usize);
    let mut last_status_write = Instant::now();

    let mut final_status = initial_status.clone();
    let mut cancelled = false;

    'outer: for ((src_idx, tgt_idx), users) in user_lists {
        let source_client = clients[src_idx].clone();
        let target_client = clients[tgt_idx].clone();
        for user_id in users {
            if *tracker.cancel.lock().await {
                cancelled = true;
                break 'outer;
            }

            let pair_label = format!(
                "{} -> {} (user {})",
                server_names[src_idx], server_names[tgt_idx], user_id
            );
            final_status.current_pair = Some(pair_label);
            write_status(tracker, &final_status).await;

            let target_user_id =
                match lookup_target_user(state, src_idx, tgt_idx, &user_id, &config.user_mappings)
                    .await
                {
                    Some(uid) => uid,
                    None => {
                        final_status.skipped += 1;
                        final_status.processed += 1;
                        push_error(
                            &mut final_status,
                            BackfillError {
                                user: user_id.clone(),
                                server: server_names[src_idx].clone(),
                                item_id: None,
                                provider: None,
                                message: format!(
                                    "no user mapping to target '{}'",
                                    server_names[tgt_idx]
                                ),
                            },
                        );
                        write_status_throttled(tracker, &final_status, &mut last_status_write)
                            .await;
                        continue;
                    }
                };

            let filter = if is_emby[src_idx] {
                options.scope.emby_filter()
            } else {
                options.scope.jellyfin_filter()
            };
            let mut page: usize = 0;
            let page_size: usize = 500;
            loop {
                if *tracker.cancel.lock().await {
                    cancelled = true;
                    break 'outer;
                }
                if page * page_size >= BACKFILL_MAX_ITEMS {
                    warn!(
                        "Backfill reached {} item cap; stopping at user {} on {}",
                        BACKFILL_MAX_ITEMS, user_id, server_names[src_idx]
                    );
                    break;
                }
                let items_res = tokio::time::timeout(
                    BACKFILL_PAGE_TIMEOUT,
                    source_client.get_user_played_items(
                        &user_id,
                        filter,
                        page * page_size,
                        page_size,
                    ),
                )
                .await;
                let items = match items_res {
                    Ok(Ok(items)) => items,
                    Ok(Err(e)) => {
                        final_status.failed += 1;
                        final_status.processed += 1;
                        push_error(
                            &mut final_status,
                            BackfillError {
                                user: user_id.clone(),
                                server: server_names[src_idx].clone(),
                                item_id: None,
                                provider: None,
                                message: format!("list failed: {}", e),
                            },
                        );
                        write_status_throttled(tracker, &final_status, &mut last_status_write)
                            .await;
                        break;
                    }
                    Err(_) => {
                        final_status.failed += 1;
                        final_status.processed += 1;
                        push_error(
                            &mut final_status,
                            BackfillError {
                                user: user_id.clone(),
                                server: server_names[src_idx].clone(),
                                item_id: None,
                                provider: None,
                                message: format!("list timeout after {:?}", BACKFILL_PAGE_TIMEOUT),
                            },
                        );
                        write_status_throttled(tracker, &final_status, &mut last_status_write)
                            .await;
                        break;
                    }
                };
                if items.is_empty() {
                    break;
                }
                for item in items {
                    if *tracker.cancel.lock().await {
                        cancelled = true;
                        break 'outer;
                    }
                    let _permit = semaphore.acquire().await;
                    let started_at_item = Instant::now();
                    process_one(
                        state,
                        tracker,
                        &mut final_status,
                        src_idx,
                        tgt_idx,
                        &user_id,
                        &target_user_id,
                        &source_client,
                        &target_client,
                        &item,
                        effective_merge,
                        options.force,
                    )
                    .await;
                    let elapsed = started_at_item.elapsed();
                    if elapsed < min_interval {
                        tokio::time::sleep(min_interval - elapsed).await;
                    }
                    write_status_throttled(tracker, &final_status, &mut last_status_write).await;
                }
                page += 1;
            }
        }
    }

    let now = chrono::Utc::now();
    final_status.finished_at = Some(now.to_rfc3339());
    final_status.current_pair = None;
    final_status.state = if cancelled {
        BackfillState::Cancelled
    } else if final_status.failed == 0 {
        BackfillState::Completed
    } else {
        BackfillState::Failed
    };

    info!(
        "Backfill {}: processed={}, succeeded={}, skipped={}, failed={}",
        match final_status.state {
            BackfillState::Completed => "completed",
            BackfillState::Failed => "failed",
            BackfillState::Cancelled => "cancelled",
            _ => "ended",
        },
        final_status.processed,
        final_status.succeeded,
        final_status.skipped,
        final_status.failed
    );

    final_status
}

fn build_pairs(
    direction: Direction,
    servers: &[crate::config::ServerConfig],
    is_emby: &[bool],
) -> Vec<(usize, usize)> {
    let eligible_sources: Vec<usize> = (0..servers.len())
        .filter(|&i| servers[i].sync_direction != "receive")
        .collect();
    let eligible_targets: Vec<usize> = (0..servers.len())
        .filter(|&i| servers[i].sync_direction != "send")
        .collect();

    let source_filter: Box<dyn Fn(usize) -> bool> = match direction {
        Direction::EmbyToJellyfin => Box::new(|i| is_emby.get(i).copied().unwrap_or(false)),
        Direction::JellyfinToEmby => Box::new(|i| !is_emby.get(i).copied().unwrap_or(false)),
        Direction::Both => Box::new(|_| true),
    };
    let target_filter: Box<dyn Fn(usize) -> bool> = match direction {
        Direction::EmbyToJellyfin => Box::new(|i| !is_emby.get(i).copied().unwrap_or(false)),
        Direction::JellyfinToEmby => Box::new(|i| is_emby.get(i).copied().unwrap_or(false)),
        Direction::Both => Box::new(|_| true),
    };

    let mut pairs = Vec::new();
    for &s in &eligible_sources {
        if !source_filter(s) {
            continue;
        }
        for &t in &eligible_targets {
            if s == t || !target_filter(t) {
                continue;
            }
            pairs.push((s, t));
        }
    }
    pairs
}

async fn collect_users_per_pair(
    state: &Arc<Mutex<AppState>>,
    pairs: &[(usize, usize)],
) -> Vec<((usize, usize), Vec<String>)> {
    let state_guard = state.lock().await;
    let mut out = Vec::with_capacity(pairs.len());
    let mut seen_sources: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut user_lists: std::collections::HashMap<usize, Vec<String>> =
        std::collections::HashMap::new();
    for &(src, _tgt) in pairs {
        if seen_sources.insert(src) {
            if let Some(cache) = state_guard.caches.get(src) {
                user_lists.insert(src, cache.users.values().cloned().collect());
            } else {
                user_lists.insert(src, Vec::new());
            }
        }
    }
    drop(state_guard);
    for &(src, tgt) in pairs {
        let users = user_lists.get(&src).cloned().unwrap_or_default();
        out.push(((src, tgt), users));
    }
    out
}

async fn lookup_target_user(
    state: &Arc<Mutex<AppState>>,
    src_idx: usize,
    tgt_idx: usize,
    source_user_id: &str,
    custom_mappings: &[Vec<String>],
) -> Option<String> {
    let state_guard = state.lock().await;
    let src_username = state_guard.caches.get(src_idx).and_then(|c| {
        c.users.iter().find_map(|(name, id)| {
            if id == source_user_id {
                Some(name.clone())
            } else {
                None
            }
        })
    });
    match src_username {
        Some(name) => {
            let tgt_users = state_guard
                .caches
                .get(tgt_idx)
                .map(|c| &c.users)
                .cloned()
                .unwrap_or_default();
            crate::state::find_mapped_user_id(&name, &tgt_users, custom_mappings)
        }
        None => None,
    }
}

#[allow(clippy::too_many_arguments)]
async fn process_one(
    state: &Arc<Mutex<AppState>>,
    _tracker: &Arc<BackfillTracker>,
    status: &mut BackfillStatus,
    _src_idx: usize,
    tgt_idx: usize,
    source_user_id: &str,
    target_user_id: &str,
    _source_client: &Arc<MediaClient>,
    target_client: &Arc<MediaClient>,
    item: &crate::client::UserItem,
    merge_policy: MergePolicy,
    force: bool,
) {
    let source_pos = item.playback_position_ticks.unwrap_or(0);
    let source_played = item.played;
    let source_last_played = item.last_played_date.as_deref();
    let imdb_id = item.imdb_id.clone().unwrap_or_default();
    let tmdb_id = item.tmdb_id.clone().unwrap_or_default();

    if imdb_id.is_empty() && tmdb_id.is_empty() {
        status.skipped += 1;
        status.processed += 1;
        return;
    }

    let (target_item_id, target_pos, target_played, target_last_played) = match resolve_target_item(
        state,
        tgt_idx,
        &imdb_id,
        &tmdb_id,
        target_user_id,
        target_client,
    )
    .await
    {
        Some(v) => v,
        None => {
            status.skipped += 1;
            status.processed += 1;
            return;
        }
    };

    let history_key = (
        source_user_id.to_lowercase(),
        if !imdb_id.is_empty() {
            imdb_id.clone()
        } else {
            tmdb_id.clone()
        },
    );

    if !force {
        let state_guard = state.lock().await;
        if let Some(prev) = state_guard.last_syncs.get(&history_key) {
            if prev.position_ticks >= source_pos.max(target_pos) {
                status.skipped += 1;
                status.processed += 1;
                return;
            }
        }
    }

    let (final_pos, final_played) = match merge_policy {
        MergePolicy::SourceWins => (source_pos, source_played),
        MergePolicy::Max => (source_pos.max(target_pos), source_played || target_played),
        MergePolicy::Newest => match (source_last_played, target_last_played) {
            (Some(s), Some(t)) if *t > *s => (target_pos, target_played),
            (Some(_), None) => (target_pos, target_played),
            (None, Some(_)) => (source_pos, source_played),
            _ => (source_pos, source_played),
        },
    };

    let update_res = tokio::time::timeout(
        BACKFILL_UPDATE_TIMEOUT,
        target_client.update_progress(target_user_id, &target_item_id, final_pos, final_played),
    )
    .await;

    match update_res {
        Ok(Ok(())) => {
            let mut state_guard = state.lock().await;
            state_guard.last_syncs.insert(
                history_key,
                crate::state::SyncHistoryValue {
                    position_ticks: final_pos,
                    timestamp: Instant::now(),
                },
            );
            drop(state_guard);
            status.succeeded += 1;
            status.processed += 1;
        }
        Ok(Err(e)) => {
            status.failed += 1;
            status.processed += 1;
            push_error(
                status,
                BackfillError {
                    user: source_user_id.to_string(),
                    server: tracker_internal_server_name(state, tgt_idx).await,
                    item_id: Some(target_item_id),
                    provider: if !imdb_id.is_empty() {
                        Some(imdb_id)
                    } else {
                        Some(tmdb_id)
                    },
                    message: e.to_string(),
                },
            );
        }
        Err(_) => {
            status.failed += 1;
            status.processed += 1;
            push_error(
                status,
                BackfillError {
                    user: source_user_id.to_string(),
                    server: tracker_internal_server_name(state, tgt_idx).await,
                    item_id: Some(target_item_id),
                    provider: if !imdb_id.is_empty() {
                        Some(imdb_id)
                    } else {
                        Some(tmdb_id)
                    },
                    message: format!("update timeout after {:?}", BACKFILL_UPDATE_TIMEOUT),
                },
            );
        }
    }
}

async fn tracker_internal_server_name(_state: &Arc<Mutex<AppState>>, _idx: usize) -> String {
    String::new()
}

async fn resolve_target_item(
    state: &Arc<Mutex<AppState>>,
    tgt_idx: usize,
    imdb_id: &str,
    tmdb_id: &str,
    target_user_id: &str,
    target_client: &Arc<MediaClient>,
) -> Option<(String, i64, bool, Option<String>)> {
    let cached_target_id: Option<String> = {
        let state_guard = state.lock().await;
        let cache = state_guard.caches.get(tgt_idx)?;
        if !imdb_id.is_empty() {
            cache.imdb_to_id.get(imdb_id).cloned()
        } else {
            None
        }
        .or_else(|| {
            if !tmdb_id.is_empty() {
                cache.tmdb_to_id.get(tmdb_id).cloned()
            } else {
                None
            }
        })
        .filter(|id| id != "[ NOT_FOUND ]")
    };

    let target_item_id = match cached_target_id {
        Some(id) => id,
        None => {
            let res = target_client
                .find_item_by_provider(target_user_id, imdb_id, tmdb_id)
                .await
                .ok()
                .flatten();
            match res {
                Some((id, _i, _t)) => {
                    let mut state_guard = state.lock().await;
                    if let Some(cache) = state_guard.caches.get_mut(tgt_idx) {
                        cache
                            .id_to_providers
                            .entry(id.clone())
                            .or_insert_with(|| (String::new(), String::new()));
                        let entry = cache.id_to_providers.get_mut(&id).unwrap();
                        if !imdb_id.is_empty() {
                            entry.0 = imdb_id.to_string();
                        }
                        if !tmdb_id.is_empty() {
                            entry.1 = tmdb_id.to_string();
                        }
                        if !imdb_id.is_empty() {
                            cache.imdb_to_id.insert(imdb_id.to_string(), id.clone());
                        }
                        if !tmdb_id.is_empty() {
                            cache.tmdb_to_id.insert(tmdb_id.to_string(), id.clone());
                        }
                    }
                    id
                }
                None => return None,
            }
        }
    };

    let current = target_client
        .get_item_userdata(target_user_id, &target_item_id)
        .await
        .ok();
    match current {
        Some(ud) => Some((
            target_item_id,
            ud.playback_position_ticks.unwrap_or(0),
            ud.played,
            ud.last_played_date,
        )),
        None => Some((target_item_id, 0, false, None)),
    }
}

async fn write_status(tracker: &BackfillTracker, status: &BackfillStatus) {
    let mut guard = tracker.status.lock().await;
    *guard = status.clone();
}

async fn write_status_throttled(
    tracker: &BackfillTracker,
    status: &BackfillStatus,
    last_write: &mut Instant,
) {
    if last_write.elapsed() >= STATUS_WRITE_INTERVAL {
        *last_write = Instant::now();
        write_status(tracker, status).await;
    }
}

fn push_error(status: &mut BackfillStatus, err: BackfillError) {
    let mut deque: VecDeque<BackfillError> = status.errors.drain(..).collect();
    while deque.len() >= BACKFILL_ERROR_CAP {
        deque.pop_front();
    }
    deque.push_back(err.clone());
    status.errors = deque.into_iter().collect();
    status.last_error = Some(err.message);
}

pub async fn cancel_backfill(tracker: &BackfillTracker) {
    let mut c = tracker.cancel.lock().await;
    *c = true;
}

pub async fn snapshot_status(tracker: &BackfillTracker) -> BackfillStatus {
    tracker.status.lock().await.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options_sensible() {
        let o = BackfillOptions::default();
        assert_eq!(o.direction, Direction::Both);
        assert_eq!(o.merge, MergePolicy::Max);
        assert_eq!(o.scope, Scope::All);
        assert_eq!(o.rate, 5);
        assert!(!o.force);
    }

    #[test]
    fn scope_filter_is_correct() {
        assert_eq!(Scope::Played.jellyfin_filter(), "IsPlayed");
        assert_eq!(Scope::Resumable.jellyfin_filter(), "IsResumable");
        assert_eq!(Scope::All.jellyfin_filter(), "IsPlayedOrResumable");
        assert_eq!(Scope::Played.emby_filter(), "IsPlayed=true");
    }

    #[test]
    fn direction_default_from_env_unknown() {
        unsafe {
            std::env::remove_var("STATESYNC_BACKFILL_DIRECTION");
        }
        assert_eq!(Direction::from_env(), Direction::Both);
        unsafe {
            std::env::set_var("STATESYNC_BACKFILL_DIRECTION", "emby-to-jellyfin");
        }
        assert_eq!(Direction::from_env(), Direction::EmbyToJellyfin);
        unsafe {
            std::env::set_var("STATESYNC_BACKFILL_DIRECTION", "jellyfin-to-emby");
        }
        assert_eq!(Direction::from_env(), Direction::JellyfinToEmby);
        unsafe {
            std::env::remove_var("STATESYNC_BACKFILL_DIRECTION");
        }
    }

    #[test]
    fn merge_policy_default_unknown_value() {
        unsafe {
            std::env::remove_var("STATESYNC_BACKFILL_MERGE");
        }
        assert_eq!(MergePolicy::from_env(), MergePolicy::Max);
        unsafe {
            std::env::set_var("STATESYNC_BACKFILL_MERGE", "newest");
        }
        assert_eq!(MergePolicy::from_env(), MergePolicy::Newest);
        unsafe {
            std::env::set_var("STATESYNC_BACKFILL_MERGE", "source-wins");
        }
        assert_eq!(MergePolicy::from_env(), MergePolicy::SourceWins);
        unsafe {
            std::env::set_var("STATESYNC_BACKFILL_MERGE", "garbage");
        }
        assert_eq!(MergePolicy::from_env(), MergePolicy::default());
        unsafe {
            std::env::remove_var("STATESYNC_BACKFILL_MERGE");
        }
    }

    #[test]
    fn scope_default_unknown_value() {
        unsafe {
            std::env::remove_var("STATESYNC_BACKFILL_SCOPE");
        }
        assert_eq!(Scope::from_env(), Scope::All);
        unsafe {
            std::env::set_var("STATESYNC_BACKFILL_SCOPE", "played");
        }
        assert_eq!(Scope::from_env(), Scope::Played);
        unsafe {
            std::env::remove_var("STATESYNC_BACKFILL_SCOPE");
        }
    }

    #[test]
    fn options_serde_roundtrip() {
        let json = r#"{"direction":"emby-to-jellyfin","merge":"max","rate":10,"scope":"played","force":true}"#;
        let o: BackfillOptions = serde_json::from_str(json).unwrap();
        assert_eq!(o.direction, Direction::EmbyToJellyfin);
        assert_eq!(o.merge, MergePolicy::Max);
        assert_eq!(o.rate, 10);
        assert_eq!(o.scope, Scope::Played);
        assert!(o.force);
    }

    #[test]
    fn idle_status_has_zero_counters() {
        let s = BackfillStatus::idle();
        assert_eq!(s.state, BackfillState::Idle);
        assert_eq!(s.processed, 0);
        assert!(s.errors.is_empty());
    }

    #[test]
    fn push_error_caps_at_limit() {
        let mut s = BackfillStatus::idle();
        for i in 0..(BACKFILL_ERROR_CAP + 50) {
            push_error(
                &mut s,
                BackfillError {
                    user: format!("u{}", i),
                    server: "s".into(),
                    item_id: None,
                    provider: None,
                    message: format!("err {}", i),
                },
            );
        }
        assert_eq!(s.errors.len(), BACKFILL_ERROR_CAP);
        assert_eq!(s.errors[0].user, "u50");
    }

    #[test]
    fn rate_clamped_to_range() {
        unsafe {
            std::env::set_var("STATESYNC_BACKFILL_RATE", "100");
        }
        let o = BackfillOptions::from_env_or_default();
        assert_eq!(o.rate, 50);
        unsafe {
            std::env::set_var("STATESYNC_BACKFILL_RATE", "0");
        }
        let o = BackfillOptions::from_env_or_default();
        assert_eq!(o.rate, 1);
        unsafe {
            std::env::remove_var("STATESYNC_BACKFILL_RATE");
        }
    }

    #[test]
    fn build_pairs_skips_send_or_receive() {
        let servers = vec![
            crate::config::ServerConfig {
                name: "a".into(),
                url: "http://a".into(),
                api_key: "k".into(),
                is_emby: true,
                sync_direction: "both".into(),
                allow_insecure_http: true,
            },
            crate::config::ServerConfig {
                name: "b".into(),
                url: "http://b".into(),
                api_key: "k".into(),
                is_emby: false,
                sync_direction: "send".into(),
                allow_insecure_http: true,
            },
            crate::config::ServerConfig {
                name: "c".into(),
                url: "http://c".into(),
                api_key: "k".into(),
                is_emby: false,
                sync_direction: "receive".into(),
                allow_insecure_http: true,
            },
        ];
        let is_emby = vec![true, false, false];
        let pairs = build_pairs(Direction::Both, &servers, &is_emby);
        // a=both: source ok, target ok
        // b=send: source ok (can emit), target skipped (cannot receive)
        // c=receive: source skipped (cannot be queried for sync), target ok
        // Eligible pairs: (a,c), (b,a), (b,c)
        let mut pairs = pairs;
        pairs.sort();
        let mut expected = vec![(0, 2), (1, 0), (1, 2)];
        expected.sort();
        assert_eq!(pairs, expected);
    }
}
