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

/// The agent directory beneath the home directory.
///
/// # Errors
/// Returns a native error when the environment names no home directory.
pub(crate) fn agent_base() -> io::Result<String> {
    let home = std::env::var("HOME")
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "no home directory"))?;
    Ok(maestro_path::join(&[&home, ".maestro/agent"]))
}

/// The session root beneath the home directory.
///
/// # Errors
/// Returns a native error when the environment names no home directory.
pub(crate) fn session_base() -> io::Result<String> {
    Ok(maestro_path::join(&[&agent_base()?, "sessions"]))
}

/// Resolve an authored path against the process working directory.
///
/// # Errors
/// Returns the native working-directory failure.
pub(crate) fn resolve_path(path: &str) -> io::Result<String> {
    let resolved = maestro_path::resolve_with(&[&[path]], Vec::new, || {
        std::env::current_dir().map(|dir| dir.to_string_lossy().into_owned())
    })?;
    Ok(resolved.into_iter().next().unwrap_or_default())
}

/// Whether a recorded JSON value counts as present for a source `||` guard.
pub(crate) fn present(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(flag) => *flag,
        serde_json::Value::Number(number) => number.as_f64().is_some_and(|number| number != 0.0),
        serde_json::Value::String(text) => !text.is_empty(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => true,
    }
}

/// Whether a directory entry name is a session journal.
pub(crate) fn is_journal(name: &str) -> bool {
    name.as_bytes().ends_with(b".jsonl")
}
