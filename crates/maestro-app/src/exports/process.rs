//! Existing process owners exposed at the application root.

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_process::{
    Launch, LaunchContext, OutputMode, SpawnSyncError, SpawnSyncOptions, TerminationSignal, spawn,
    spawn_sync, windows_shell_launch,
};

#[cfg(unix)]
pub use maestro_process::{DefaultSignalActions, continued, stop_process_group};
