//! Native child-process lifecycles and termination-signal delivery, shared by tools,
//! packages, credentials, the toolkit and the native terminal adapter.

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

#[cfg(not(target_arch = "wasm32"))]
mod startup;
#[cfg(not(target_arch = "wasm32"))]
pub use startup::{command, environment, initialize_coding_agent};

pub mod jobs;

#[cfg(unix)]
pub mod job_control;

#[cfg(unix)]
pub use job_control::{continued, stop_process_group};

#[cfg(not(target_arch = "wasm32"))]
pub mod notifications;

#[cfg(unix)]
pub use notifications::DefaultSignalActions;
#[cfg(not(target_arch = "wasm32"))]
pub use notifications::{TerminationSignal, register_signals, signals_for_host};

#[cfg(target_os = "linux")]
pub mod terminal;

#[cfg(unix)]
#[cfg(test)]
mod tests {
    pub(crate) mod runtime;
}
