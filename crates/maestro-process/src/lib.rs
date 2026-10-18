//! Native child-process lifecycles shared by tools, packages, credentials and the toolkit.

#[cfg(not(target_arch = "wasm32"))]
pub mod child_process;

#[cfg(not(target_arch = "wasm32"))]
pub use child_process::{OutputMode, SpawnSyncError, SpawnSyncOptions, spawn, spawn_sync};

#[cfg(not(target_arch = "wasm32"))]
pub mod shell;

#[cfg(not(target_arch = "wasm32"))]
pub use shell::{Launch, windows_shell_launch};

#[cfg(target_os = "linux")]
pub mod pty;

#[cfg(unix)]
#[cfg(test)]
mod tests {
    pub(crate) mod runtime;
}
