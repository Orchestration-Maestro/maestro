//! Native child-process lifecycles shared by tools, packages, credentials and the toolkit.

#[cfg(not(target_arch = "wasm32"))]
pub mod child_process;

#[cfg(not(target_arch = "wasm32"))]
pub use child_process::{OutputMode, SpawnSyncError, SpawnSyncOptions, spawn, spawn_sync};
