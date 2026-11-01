//! Shared custom renderer recipes for browser tool dispatch witnesses.
#![cfg(test)]
mod recipes;
pub use recipes::*;

#[cfg(not(target_arch = "wasm32"))]
mod console_controls;

/// Read semantic observations of the recorded headers, collapse, console and forwarding cases.
/// # Panics
/// Panics if the committed observations are invalid JSON.
#[must_use]
pub fn cases() -> serde_json::Value {
    serde_json::from_str(include_str!("cases.json")).unwrap()
}
#[cfg(target_arch = "wasm32")]
mod lifetimes;

pub mod forwarding;
