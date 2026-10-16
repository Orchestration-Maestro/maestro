//! Native observations for read-only self-update qualification.

use super::SelfUpdateOperations;
use maestro_process::{OutputMode, SpawnSyncError, SpawnSyncOptions, spawn_sync};
use maestro_resources::{NativeResourceOperations, ResourceOperations};
use std::{io, process::Output};
use tokio::process::Command;

/// Native query capture and write-access observations without an update runner.
///
/// Windows observes existence and read-only attributes, not directory ACLs.
/// An attempted update can therefore still fail with a permissions error.
pub struct NativeSelfUpdateOperations;

impl SelfUpdateOperations for NativeSelfUpdateOperations {
    fn resource_operations(&self) -> &dyn ResourceOperations {
        &NativeResourceOperations
    }
    fn is_windows(&self) -> bool {
        cfg!(windows)
    }
    fn home_dir(&self) -> io::Result<String> {
        maestro_tools::expand_path("~")
    }
    fn check_write_access(&self, path: &str) -> io::Result<()> {
        check_write_access(path)
    }
    fn run_command_sync(&self, command: &str, args: &[String]) -> Result<Output, SpawnSyncError> {
        spawn_sync(
            &mut query_command(command, args),
            SpawnSyncOptions::Capture {
                stdout: OutputMode::Capture,
                stderr: OutputMode::Capture,
                input: None,
                timeout: None,
                max_buffer_bytes: None,
            },
        )
    }
}

/// Use the OS access operation rather than creating or truncating a probe file.
#[cfg(unix)]
fn check_write_access(path: &str) -> io::Result<()> {
    Ok(rustix::fs::access(path, rustix::fs::Access::WRITE_OK)?)
}

/// Match native Windows directory and regular-file attribute access results.
#[cfg(not(unix))]
fn check_write_access(path: &str) -> io::Result<()> {
    let metadata = std::fs::metadata(path)?;
    if metadata.is_dir() || !metadata.permissions().readonly() {
        Ok(())
    } else {
        Err(io::Error::from(io::ErrorKind::PermissionDenied))
    }
}

/// Prepare an operation-owned builder so shell settings cannot leak to later calls.
fn query_command(command: &str, args: &[String]) -> Command {
    if maestro_tools::process::child_process::should_use_windows_shell(command) {
        let text = std::iter::once(command)
            .chain(args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ");
        let launch =
            maestro_process::windows_shell_launch(&text, std::env::var("ComSpec").ok().as_deref());
        let mut process = Command::new(&launch.executable);
        configure_shell_args(&mut process, &launch);
        process
    } else {
        let mut process = Command::new(command);
        process.args(args);
        process
    }
}

/// Preserve the shared launch owner's Windows verbatim-argument policy.
fn configure_shell_args(command: &mut Command, launch: &maestro_process::Launch) {
    #[cfg(windows)]
    if launch.verbatim {
        use std::os::windows::process::CommandExt;
        for arg in &launch.args {
            command.as_std_mut().raw_arg(arg);
        }
        return;
    }
    command.args(&launch.args);
}
