//! Native developer reports over recorded sessions.
//!
//! Each module is one checkout-run report command: `cost` totals recorded
//! message costs, `stats` summarises usage, `session_transcripts` writes
//! transcript files and `tool_stats` writes the interactive tool report. The
//! operations take their arguments, clock reading and streams from the caller,
//! so the example programs only wire them.

pub(crate) mod calendar;
pub mod cost;
pub(crate) mod formatting;
pub(crate) mod jsonl;
pub mod session_transcripts;
pub mod stats;
pub mod tool_stats;

use std::io;

/// The agent directory beneath the host's home directory.
///
/// # Errors
/// Returns a not-found error when the host names no home directory, or an
/// invalid-data one when it is not valid UTF-8.
pub(crate) fn agent_base() -> io::Result<String> {
    Ok(maestro_path::join(&[
        &maestro_resources::paths::home_directory()?,
        ".maestro/agent",
    ]))
}

/// The session root beneath the host's home directory.
///
/// # Errors
/// Returns the home directory failure of [`agent_base`].
pub(crate) fn session_base() -> io::Result<String> {
    Ok(maestro_path::join(&[&agent_base()?, "sessions"]))
}

/// The session root beneath the `HOME` variable, which the cost report alone reads.
///
/// # Errors
/// Returns a native error when the variable names no directory.
pub(crate) fn cost_session_base() -> io::Result<String> {
    let home = std::env::var("HOME")
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "no home directory"))?;
    Ok(maestro_path::join(&[&home, ".maestro/agent/sessions"]))
}

/// Resolve an authored path against the process working directory.
///
/// # Errors
/// Returns the native working-directory failure.
pub(crate) fn resolve_path(path: &str) -> io::Result<String> {
    let resolved = maestro_path::resolve_with(
        &[&[path]],
        maestro_resources::drive_directories,
        maestro_resources::current_directory,
    )?;
    Ok(resolved.into_iter().next().unwrap_or_default())
}

/// Whether a directory entry name is a session journal.
pub(crate) fn is_journal(name: &str) -> bool {
    name.as_bytes().ends_with(b".jsonl")
}
