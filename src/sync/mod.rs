use std::sync::OnceLock;
use tokio::sync::Semaphore;

pub mod favorite_sync;
pub mod live_item_title;
pub mod progress;
pub mod progress_message;
pub mod resolve;
#[cfg(test)]
pub mod tests;

pub use favorite_sync::sync_favorite_to_targets;
pub use progress::sync_progress_to_targets;

fn sync_semaphore() -> &'static Semaphore {
    static S: OnceLock<Semaphore> = OnceLock::new();
    S.get_or_init(|| {
        let permits = std::env::var("STATESYNC_MAX_SYNC_SPAWNS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(8);
        Semaphore::new(permits.max(1))
    })
}
