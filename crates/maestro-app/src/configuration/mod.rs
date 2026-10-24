//! Installation configuration and startup observations.

pub mod config;
pub mod identification;
pub mod installation;
pub mod timings;

#[cfg(not(target_arch = "wasm32"))]
pub mod conversion;
#[cfg(not(target_arch = "wasm32"))]
pub mod releases;

/// Whether a nonempty environment flag disables the operation that reads it.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn flag_is_set(key: &str) -> bool {
    std::env::var(key).is_ok_and(|value| !value.is_empty())
}
