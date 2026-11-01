//! Native child-process lifecycles and termination-signal delivery, shared by tools,
//! packages, credentials, the toolkit and the native terminal adapter.

#[cfg(not(target_arch = "wasm32"))]
pub mod byte_queue;

#[cfg(not(target_arch = "wasm32"))]
pub mod child_process;

#[cfg(not(target_arch = "wasm32"))]
pub use child_process::{
    OutputMode, SpawnSyncError, SpawnSyncOptions, exit_signal_name, spawn, spawn_sync,
};

#[cfg(not(target_arch = "wasm32"))]
pub mod shell;

#[cfg(not(target_arch = "wasm32"))]
pub use shell::{Launch, LaunchContext, windows_shell_launch};

#[cfg(not(target_arch = "wasm32"))]
mod startup;
#[cfg(not(target_arch = "wasm32"))]
pub use startup::{command, environment, initialize_coding_agent};

pub mod foreground;

#[cfg(unix)]
pub mod job_control;

#[cfg(unix)]
pub use job_control::{continued, stop_process_group};

#[cfg(not(target_arch = "wasm32"))]
pub mod termination;

#[cfg(unix)]
pub use termination::DefaultSignalActions;
#[cfg(not(target_arch = "wasm32"))]
pub use termination::{TerminationSignal, register_signals, signals_for_host};

#[cfg(target_os = "linux")]
pub mod pty;

#[cfg(unix)]
#[cfg(test)]
mod tests {
    #[path = "../../tests/support/poll.rs"]
    pub(crate) mod poll;
    pub(crate) mod runtime;
}
