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

use crate::configuration::config;

/// The configured agent directory: `MAESTRO_CODING_AGENT_DIR` or its default.
///
/// # Errors
/// Returns the home lookup failure of the default.
pub(crate) fn agent_base() -> io::Result<String> {
    config::get_agent_dir()
}

/// The session root beneath the configured agent directory.
///
/// # Errors
/// Returns the failure of [`agent_base`].
pub(crate) fn session_base() -> io::Result<String> {
    Ok(config::paths::get_sessions_dir(&agent_base()?))
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
