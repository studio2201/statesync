//! HTTP API handlers for config, status, sync, and media servers.

pub mod config;
pub mod connection_test;
pub mod force_api;
pub mod poster_proxy;
pub mod server;
pub mod server_info;
pub mod status;
pub mod sync;
#[cfg(test)]
pub mod tests;
pub mod users;
pub mod validation;

pub use config::{get_config, mask_api_key, post_config};
pub use force_api::{get_sync_force_status, post_sync_force, post_sync_force_cancel};
pub use server::{get_server_info, post_server_info, serve_poster, test_connection};
pub use status::{CacheStats, cache_stats, get_status};
pub use sync::{post_reload, post_users_refresh};
pub use users::post_clear_watched;
