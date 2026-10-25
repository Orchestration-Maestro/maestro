//! Ordered daemon events retained for replay by API consumers.

/// Shared bounded event history.
pub mod event_hub;

/// Synchronous requests to a caller-selected daemon endpoint.
#[cfg(unix)]
pub mod client;
