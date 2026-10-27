//! Executable-only child environment defaults without changing the host environment.
use std::{
    ffi::{OsStr, OsString},
    sync::atomic::{AtomicBool, Ordering},
};
use tokio::process::Command;

/// Whether this executable identifies its children as coding-agent work.
static CODING_AGENT: AtomicBool = AtomicBool::new(false);

/// Enable the coding-agent marker for subsequently constructed native child commands.
///
/// Embedding callers do not enable this default; explicit environment edits still win.
pub fn initialize_coding_agent() {
    CODING_AGENT.store(true, Ordering::Relaxed);
}

/// Construct a native command with the executable's child defaults before caller edits.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    if CODING_AGENT.load(Ordering::Relaxed) {
        command.env("MAESTRO_CODING_AGENT", "true");
    }
    command
}

/// Observe inherited child variables with the executable's default marker applied.
///
/// Callers that rebuild an inherited environment use this before their explicit edits.
pub fn environment() -> impl Iterator<Item = (OsString, OsString)> {
    std::env::vars_os().chain(CODING_AGENT.load(Ordering::Relaxed).then(|| {
        (
            OsString::from("MAESTRO_CODING_AGENT"),
            OsString::from("true"),
        )
    }))
}
