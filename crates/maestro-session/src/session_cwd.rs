//! Diagnosis of a stored session working directory that no longer exists.
//! Whether a missing directory blocks startup is decided by the caller.
#[cfg(not(target_arch = "wasm32"))]
use crate::session_manager::ReadonlySessionManager;
use std::fmt;

/// A session whose stored working directory is unavailable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionCwdIssue {
    /// Selected session file, when one is selected.
    pub session_file: Option<String>,
    /// Stored working directory that does not exist.
    pub session_cwd: String,
    /// Working directory offered instead.
    pub fallback_cwd: String,
}

/// Error whose message was formatted once from the issue it was built with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingSessionCwdError {
    /// The diagnosed issue; editing it leaves the captured message unchanged.
    pub issue: SessionCwdIssue,
    /// Message formatted by [`Self::new`].
    message: String,
}

impl MissingSessionCwdError {
    /// Capture the formatted message of an issue.
    #[must_use]
    pub fn new(issue: SessionCwdIssue) -> Self {
        Self {
            message: format_missing_session_cwd_error(&issue),
            issue,
        }
    }

    /// Stable error name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        "MissingSessionCwdError"
    }
}

impl fmt::Display for MissingSessionCwdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for MissingSessionCwdError {}

/// Format the error text; the session-file line appears only for a nonempty file.
#[must_use]
pub fn format_missing_session_cwd_error(issue: &SessionCwdIssue) -> String {
    let file = issue
        .session_file
        .as_deref()
        .filter(|file| !file.is_empty())
        .map_or_else(String::new, |file| format!("\nSession file: {file}"));
    format!(
        "Stored session working directory does not exist: {}{file}\nCurrent working directory: {}",
        issue.session_cwd, issue.fallback_cwd
    )
}

/// Format the prompt offering the fallback directory.
#[must_use]
pub fn format_missing_session_cwd_prompt(issue: &SessionCwdIssue) -> String {
    format!(
        "cwd from session file does not exist\n{}\n\ncontinue in current cwd\n{}",
        issue.session_cwd, issue.fallback_cwd
    )
}

/// Report a selected session file whose nonempty stored working directory is missing.
/// Any existing path, symlink targets followed, counts as present; the fallback is never probed.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn get_missing_session_cwd_issue(
    session_manager: &dyn ReadonlySessionManager,
    fallback_cwd: &str,
) -> Option<SessionCwdIssue> {
    let session_file = session_manager
        .get_session_file()
        .filter(|file| !file.is_empty())?;
    let session_cwd = session_manager.get_cwd();
    if session_cwd.is_empty() || std::path::Path::new(session_cwd).exists() {
        return None;
    }
    Some(SessionCwdIssue {
        session_file: Some(session_file.to_owned()),
        session_cwd: session_cwd.to_owned(),
        fallback_cwd: fallback_cwd.to_owned(),
    })
}

/// Succeed unless [`get_missing_session_cwd_issue`] finds an issue.
///
/// # Errors
/// Returns the issue wrapped in [`MissingSessionCwdError`].
#[cfg(not(target_arch = "wasm32"))]
pub fn assert_session_cwd_exists(
    session_manager: &dyn ReadonlySessionManager,
    fallback_cwd: &str,
) -> Result<(), MissingSessionCwdError> {
    get_missing_session_cwd_issue(session_manager, fallback_cwd)
        .map_or(Ok(()), |issue| Err(MissingSessionCwdError::new(issue)))
}
