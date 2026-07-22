//! Force-sync types, options, and tracker.
use crate::client::MediaClient;
use crate::config::Config;
use crate::state::AppState;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::Mutex;

fn default_force_direction() -> Direction {
    Direction::Both
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ForceSyncOptions {
    /// Always mesh both ways among send/receive servers. Kept for API compatibility.
    #[serde(default = "default_force_direction")]
    pub direction: Direction,
    /// If true, count would-be writes but do not change any server.
    #[serde(default)]
    pub dry_run: bool,
    /// If set, only this person (and linked aliases) is force-synced.
    #[serde(default)]
    pub user: Option<String>,
}

/// Force-sync direction. Runtime always meshes send→receive (Both).
/// Legacy variant names still deserialize for old clients/CLIs.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Default)]
#[serde(rename_all = "PascalCase")]
pub enum Direction {
    #[default]
    #[serde(alias = "both", alias = "BOTH")]
    Both,
    /// Deprecated — ignored (treated as Both).
    #[serde(
        alias = "emby_to_jellyfin",
        alias = "embytojellyfin",
        alias = "EmbyToJellyfin"
    )]
    EmbyToJellyfin,
    /// Deprecated — ignored (treated as Both).
    #[serde(
        alias = "jellyfin_to_emby",
        alias = "jellyfintoemby",
        alias = "JellyfinToEmby"
    )]
    JellyfinToEmby,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum ForceSyncState {
    Idle,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ForceSyncError {
    pub user: String,
    pub server: String,
    pub item_id: Option<String>,
    pub provider: Option<String>,
    pub message: String,
}

/// Per-signal counters for force sync storytelling in the WUI.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FieldCounters {
    #[serde(default)]
    pub ok: u64,
    #[serde(default)]
    pub skip: u64,
    #[serde(default)]
    pub fail: u64,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ForceByField {
    #[serde(default)]
    pub played: FieldCounters,
    #[serde(default)]
    pub position: FieldCounters,
    #[serde(default)]
    pub favorite: FieldCounters,
}

/// Why force sync skipped an item (aggregated for WUI / activity log).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SkipReasons {
    /// Source item had empty Imdb and Tmdb in Emby/Jellyfin ProviderIds (API metadata).
    #[serde(default)]
    pub no_provider: u64,
    /// Provider present but no matching item on target library.
    #[serde(default)]
    pub no_match: u64,
    /// Target already has the same played / favorite / position state.
    #[serde(default)]
    pub already_equal: u64,
    /// Other skips (disabled scopes, etc.).
    #[serde(default)]
    pub other: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    /// Machine phase: preparing | played | favorites | finishing | done | cancelled
    #[serde(default)]
    pub phase: Option<String>,
    /// Per-field counters (played / position / favorite).
    #[serde(default)]
    pub by_field: ForceByField,
    /// Which force scopes were enabled for this run.
    #[serde(default)]
    pub scope: Vec<String>,
    /// Aggregate skip reasons (trust at scale).
    #[serde(default)]
    pub skip_reasons: SkipReasons,
    /// True when this run did not write (preview only).
    #[serde(default)]
    pub dry_run: bool,
    /// Source media server name for the active pair (first principles: where we read).
    #[serde(default)]
    pub current_source: Option<String>,
    /// Destination media server name for the active pair (where we may write).
    #[serde(default)]
    pub current_target: Option<String>,
    /// 1-based index of the active person/server direction among pair_total.
    #[serde(default)]
    pub pair_index: u64,
    /// How many person×direction pairs this run will walk.
    #[serde(default)]
    pub pair_total: u64,
    /// Short plain-language title for the live banner.
    #[serde(default)]
    pub story_headline: Option<String>,
    /// Full plain-language explanation (who, route, what, what skip means).
    #[serde(default)]
    pub story_detail: Option<String>,
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
            phase: None,
            by_field: ForceByField::default(),
            scope: Vec::new(),
            skip_reasons: SkipReasons::default(),
            dry_run: false,
            current_source: None,
            current_target: None,
            pair_index: 0,
            pair_total: 0,
            story_headline: None,
            story_detail: None,
        }
    }
}

impl Default for ForceSyncStatus {
    fn default() -> Self {
        Self::idle()
    }
}

pub struct SyncForceTracker {
    pub force_sync_in_progress: AtomicBool,
    pub running: Mutex<bool>,
    pub cancel: AtomicBool,
    /// std mutex: progress is written from the force loop and read by HTTP polls.
    /// tokio::Mutex + try_lock previously dropped updates while the API held the lock.
    pub status: std::sync::Mutex<ForceSyncStatus>,
}

impl Default for SyncForceTracker {
    fn default() -> Self {
        Self {
            force_sync_in_progress: AtomicBool::new(false),
            running: Mutex::new(false),
            cancel: AtomicBool::new(false),
            status: std::sync::Mutex::new(ForceSyncStatus::idle()),
        }
    }
}

pub struct ForceContext {
    pub direction: Direction,
    pub config: Config,
    pub clients: Vec<Arc<MediaClient>>,
    pub state: Arc<Mutex<AppState>>,
    pub tracker: Arc<SyncForceTracker>,
    /// Preview only — no UserData writes.
    pub dry_run: bool,
    /// Limit mesh to this username (and mapped aliases). None = all allowed users.
    pub only_user: Option<String>,
}
