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

pub mod client;
pub mod config;
pub mod dashboard;
pub mod state;
pub mod sync;
pub mod sync_force;
pub mod web;
pub mod web_api;
pub mod websocket;
