//! The daemon API's replayed events, its socket client and the live daemon status.

/// Shared bounded event history.
pub mod event_hub;

/// Synchronous requests to a caller-selected daemon endpoint.
#[cfg(unix)]
pub mod transport;

/// The live status a daemon reports to a ping.
#[cfg(unix)]
pub mod health;

#[cfg(unix)]
pub use health::{RuntimeStatus, read_runtime_status_at};
