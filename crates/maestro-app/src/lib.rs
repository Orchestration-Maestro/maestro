//! Application layer shared by every frontend.
//!
//! [`presentation_data`] holds the data frontends render but do not own.

pub mod adapters;
pub mod configuration;
pub mod contribution_files;
pub mod presentation_data;
pub mod utils;

#[cfg(not(target_arch = "wasm32"))]
pub mod agent_session;
#[cfg(not(target_arch = "wasm32"))]
pub mod extension_native_io;
#[cfg(not(target_arch = "wasm32"))]
pub mod extension_operations;
#[cfg(not(target_arch = "wasm32"))]
pub mod model_account;
#[cfg(not(target_arch = "wasm32"))]
pub mod output_guard;
#[cfg(not(target_arch = "wasm32"))]
pub mod resources;
#[cfg(not(target_arch = "wasm32"))]
pub mod runtime;
#[cfg(not(target_arch = "wasm32"))]
pub mod sdk;

/// The skill block a prompt carries when a skill command expands.
mod skill_block;

/// Private declarations grouped by their existing domain owners.
mod exports;

/// Opt-in helpers for controlled tests.
#[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
pub mod test_support;

#[cfg(test)]
mod tests;

#[cfg(test)]
extern crate self as maestro_app;

pub use exports::*;
