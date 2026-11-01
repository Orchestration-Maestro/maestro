//! The daemon API's replayed events, its socket client and the live daemon status.

/// Shared bounded event history.
pub mod event_hub;

/// Synchronous requests to a caller-selected daemon endpoint.
#[cfg(unix)]
pub mod client;

/// The live status a daemon reports to a ping.
#[cfg(unix)]
pub mod status;

#[cfg(unix)]
pub use status::{RuntimeStatus, read_runtime_status_at};
