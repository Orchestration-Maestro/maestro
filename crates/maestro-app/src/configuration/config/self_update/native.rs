//! Native observations for read-only self-update qualification.

use super::SelfUpdateOperations;
use maestro_process::{OutputMode, SpawnSyncError, SpawnSyncOptions};
use std::process::Output;

/// Native read-only query capture without an update runner.
pub struct NativeSelfUpdateOperations;

impl SelfUpdateOperations for NativeSelfUpdateOperations {
    fn is_windows(&self) -> bool {
        cfg!(windows)
    }
    fn run_command_sync(&self, command: &str, args: &[String]) -> Result<Output, SpawnSyncError> {
        maestro_process::Launch {
            executable: command.into(),
            args: args.into(),
            verbatim: false,
        }
        .with_context(std::iter::empty::<(&str, &str)>(), None)
        .spawn_sync(SpawnSyncOptions::Capture {
            stdout: OutputMode::Capture,
            stderr: OutputMode::Capture,
            input: None,
            timeout: None,
            max_buffer_bytes: None,
        })
    }
}
