//! Native child-process lifecycles shared by tools, packages, credentials and the toolkit.

#[cfg(not(target_arch = "wasm32"))]
pub mod byte_queue;

#[cfg(not(target_arch = "wasm32"))]
pub mod children;

#[cfg(not(target_arch = "wasm32"))]
pub use children::{
    OutputMode, SpawnSyncError, SpawnSyncOptions, exit_signal_name, spawn, spawn_sync,
};

#[cfg(not(target_arch = "wasm32"))]
pub mod commands;

#[cfg(not(target_arch = "wasm32"))]
pub use commands::{Launch, LaunchContext, windows_shell_launch};

pub mod jobs;

#[cfg(not(target_arch = "wasm32"))]
pub mod signals;

#[cfg(unix)]
pub use signals::DefaultSignalActions;
#[cfg(not(target_arch = "wasm32"))]
pub use signals::TerminationSignal;

#[cfg(target_os = "linux")]
pub mod terminal;

#[cfg(unix)]
#[cfg(test)]
mod tests {
    pub(crate) mod runtime;
}
