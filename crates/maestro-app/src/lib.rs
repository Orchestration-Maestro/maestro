//! Application layer shared by every frontend.
//!
//! [`presentation_data`] holds the data frontends render but do not own.

pub mod browser;
pub mod configuration;
pub mod presentation_data;
pub mod utils;

#[cfg(not(target_arch = "wasm32"))]
pub mod output_guard;

/// Private declarations grouped by their existing domain owners.
mod exports;

#[cfg(test)]
mod tests;

pub use exports::*;
