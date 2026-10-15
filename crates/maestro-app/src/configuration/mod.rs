//! Installation configuration and startup observations.

pub mod config;
pub mod maestro_user_agent;
pub mod telemetry;
pub mod timings;

#[cfg(not(target_arch = "wasm32"))]
pub mod migrations;
#[cfg(not(target_arch = "wasm32"))]
pub mod version_check;
