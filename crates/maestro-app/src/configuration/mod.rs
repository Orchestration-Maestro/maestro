//! Installation configuration and startup observations.

pub mod config;
pub mod maestro_user_agent;
pub mod telemetry;
pub mod timings;

#[cfg(not(target_arch = "wasm32"))]
pub mod migrations;
#[cfg(not(target_arch = "wasm32"))]
pub mod version_check;

/// Whether an observed environment flag is present and nonempty.
/// Values such as `0`, `false` and a space still count as set.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn flag_is_set(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.is_empty())
}
