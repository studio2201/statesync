//! StateSync: sync watched / resume / favorites across Emby and Jellyfin.
#![allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::collapsible_if,
    clippy::single_match,
    // Test modules named `tests` under `mod tests` — common Rust layout.
    clippy::module_inception,
    // std::sync::Mutex in tests held across mockito `.await` (not production paths).
    clippy::await_holding_lock,
)]

/// Missing documentation.
pub mod client;
/// Missing documentation.
pub mod config;
pub mod dashboard;
/// Missing documentation.
pub mod state;
/// Missing documentation.
pub mod sync;
/// Missing documentation.
pub mod sync_force;
/// Missing documentation.
pub mod web;
/// Missing documentation.
pub mod web_api;
/// Missing documentation.
pub mod websocket;
