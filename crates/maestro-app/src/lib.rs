//! Application layer shared by every frontend.
//!
//! [`presentation_data`] holds the data frontends render but do not own.

pub mod browser;
pub mod configuration;
pub mod contribution_files;
pub mod presentation_data;
pub mod utils;

#[cfg(not(target_arch = "wasm32"))]
pub mod agent_session;
#[cfg(not(target_arch = "wasm32"))]
pub mod connection_runner;
#[cfg(not(target_arch = "wasm32"))]
pub mod developer_tools;
#[cfg(not(target_arch = "wasm32"))]
pub mod model_account;
#[cfg(not(target_arch = "wasm32"))]
pub mod output_guard;
#[cfg(not(target_arch = "wasm32"))]
pub mod resources;
#[cfg(not(target_arch = "wasm32"))]
pub mod rpc_transport;
#[cfg(not(target_arch = "wasm32"))]
pub mod rpc_types;
#[cfg(not(target_arch = "wasm32"))]
pub mod runtime;
#[cfg(not(target_arch = "wasm32"))]
pub mod sdk;

/// The skill block a prompt carries when a skill command expands.
mod skill_block;

/// Private declarations grouped by their existing domain owners.
mod exports;

#[cfg(test)]
mod tests;

#[cfg(test)]
extern crate self as maestro_app;

pub use exports::*;
