//! Installation configuration and startup observations.

pub mod config;
pub mod maestro_user_agent;
pub mod timings;

#[cfg(not(target_arch = "wasm32"))]
pub mod migrations;
