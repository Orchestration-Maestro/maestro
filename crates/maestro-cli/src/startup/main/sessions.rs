//! Session lookups and the operation the authored arguments select.

use crate::input::args::Args;
use maestro_app::{Presence, SessionInfo, SessionManager};
use std::io;

/// Where a session argument resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedSession {
    /// An authored path, used as written.
    Path(String),
    /// A session of the current project.
    Local(String),
    /// A session of another project, with that project's directory.
    Global {
        /// The session file.
        path: String,
        /// The directory the session belongs to.
        cwd: String,
    },
    /// No session matched the argument.
    NotFound(String),
}

/// The suffix that makes an authored argument a session file.
const SESSION_FILE_SUFFIX: &str = ".jsonl";

/// Whether an authored argument names a file rather than an identifier prefix.
///
/// The suffix comparison is case-sensitive, so an uppercase spelling stays an identifier.
fn is_authored_path(session_arg: &str) -> bool {
    session_arg.contains('/')
        || session_arg.contains('\\')
        || session_arg.ends_with(SESSION_FILE_SUFFIX)
}

/// Whether a listed session's retained identifier starts with `session_arg`.
fn matches_prefix(session: &SessionInfo, session_arg: &str) -> bool {
    match &session.id {
        Presence::Present(serde_json::Value::String(id)) => id.starts_with(session_arg),
        Presence::Missing | Presence::Null | Presence::Present(_) => false,
    }
}

/// The first listed session whose identifier starts with `session_arg`.
fn first_match(sessions: Vec<SessionInfo>, session_arg: &str) -> Option<SessionInfo> {
    sessions
        .into_iter()
        .find(|session| matches_prefix(session, session_arg))
}

/// Select the fork of every resolved session, including one of another project.
///
/// An explicit fork copies the matched session into the current project without asking,
/// so a foreign match keeps its path and directory for reporting only.
fn fork_selection(resolved: ResolvedSession) -> SessionSelection {
    match resolved {
        ResolvedSession::Path(path)
        | ResolvedSession::Local(path)
        | ResolvedSession::Global { path, .. } => {
            SessionSelection::Selected(SessionOperation::Fork { path })
        }
        ResolvedSession::NotFound(argument) => SessionSelection::NotFound(argument),
    }
}

/// Select opening a resolved session, confirming a foreign one before copying it in.
fn open_selection(resolved: ResolvedSession) -> SessionSelection {
    match resolved {
        ResolvedSession::Path(path) | ResolvedSession::Local(path) => {
            SessionSelection::Selected(SessionOperation::Open { path })
        }
        ResolvedSession::Global { path, cwd } => SessionSelection::ConfirmFork { path, cwd },
        ResolvedSession::NotFound(argument) => SessionSelection::NotFound(argument),
    }
}

/// Resolve a session argument to a file.
///
/// An argument naming a path is used as written. Otherwise the current project's
/// sessions are searched first and only then every project, keeping the first match of
/// each listing.
///
/// # Errors
/// Returns the listing failure of the current project.
pub async fn resolve_session_path(
    session_arg: &str,
    cwd: &str,
    session_dir: Option<&str>,
    agent_dir: &str,
) -> io::Result<ResolvedSession> {
    if is_authored_path(session_arg) {
        return Ok(ResolvedSession::Path(session_arg.to_owned()));
    }
    let local = SessionManager::list(cwd, session_dir, agent_dir, None).await?;
    if let Some(session) = first_match(local, session_arg) {
        return Ok(ResolvedSession::Local(session.path));
    }
    if let Some(session) = first_match(SessionManager::list_all(agent_dir, None).await, session_arg)
    {
        return Ok(ResolvedSession::Global {
            path: session.path,
            cwd: session.cwd,
        });
    }
    Ok(ResolvedSession::NotFound(session_arg.to_owned()))
}

/// A session operation that needs no presentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionOperation {
    /// An unsaved session.
    InMemory,
    /// Copy the session at this path into the current project.
    Fork {
        /// The session file to copy.
        path: String,
    },
    /// Open the session at this path in place.
    Open {
        /// The session file to open.
        path: String,
    },
    /// Continue the most recently active session.
    ContinueRecent,
    /// Start a new session.
    Create,
}

/// What the authored arguments and the session lookup select.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionSelection {
    /// The operation to perform.
    Selected(SessionOperation),
    /// A match in another project, to confirm before forking into this one.
    ConfirmFork {
        /// The matched session file.
        path: String,
        /// The directory the session belongs to.
        cwd: String,
    },
    /// A session to choose from every project.
    Resume,
    /// No session matched the argument.
    NotFound(String),
}

/// Select the fork of the session `argument` names.
async fn forked(
    argument: &str,
    cwd: &str,
    session_dir: Option<&str>,
    agent_dir: &str,
) -> io::Result<SessionSelection> {
    let resolved = resolve_session_path(argument, cwd, session_dir, agent_dir).await?;
    Ok(fork_selection(resolved))
}

/// Select opening the session `argument` names.
async fn opened(
    argument: &str,
    cwd: &str,
    session_dir: Option<&str>,
    agent_dir: &str,
) -> io::Result<SessionSelection> {
    let resolved = resolve_session_path(argument, cwd, session_dir, agent_dir).await?;
    Ok(open_selection(resolved))
}

/// Select the session operation, resolving a fork or session argument first.
///
/// An unsaved session and a continue request look nothing up while the operation is
/// selected; continuing finds the latest history when the operation is performed. A fork
/// request takes precedence over a session request, which takes precedence over resume
/// and continue. A fork of another project's session copies it in without confirmation;
/// only a session request confirms before copying it in.
///
/// # Errors
/// Returns the listing failure of `resolve_session_path`.
pub async fn select_session_operation(
    parsed: &Args,
    cwd: &str,
    session_dir: Option<&str>,
    agent_dir: &str,
) -> io::Result<SessionSelection> {
    if parsed.no_session == Some(true) {
        return Ok(SessionSelection::Selected(SessionOperation::InMemory));
    }
    if let Some(fork) = parsed.fork.as_deref().filter(|value| !value.is_empty()) {
        return forked(fork, cwd, session_dir, agent_dir).await;
    }
    if let Some(session) = parsed.session.as_deref().filter(|value| !value.is_empty()) {
        return opened(session, cwd, session_dir, agent_dir).await;
    }
    if parsed.resume == Some(true) {
        return Ok(SessionSelection::Resume);
    }
    if parsed.r#continue == Some(true) {
        return Ok(SessionSelection::Selected(SessionOperation::ContinueRecent));
    }
    Ok(SessionSelection::Selected(SessionOperation::Create))
}

/// Copy `source` into the current project as a new session.
///
/// # Errors
/// Returns the fork failure. An empty or invalid source fails before anything is created;
/// a later directory, write or reopen failure leaves the records already written in place.
pub fn fork_session(
    source: &str,
    cwd: &str,
    session_dir: Option<&str>,
    agent_dir: &str,
) -> io::Result<SessionManager> {
    SessionManager::fork_from(source, cwd, session_dir, agent_dir)
}

/// Perform a selected operation.
///
/// # Errors
/// Returns the open, fork, read or directory failure of the operation.
pub fn open_selected_session(
    operation: &SessionOperation,
    cwd: &str,
    session_dir: Option<&str>,
    agent_dir: &str,
) -> io::Result<SessionManager> {
    match operation {
        SessionOperation::InMemory => SessionManager::in_memory(Some(cwd)),
        SessionOperation::Fork { path } => fork_session(path, cwd, session_dir, agent_dir),
        SessionOperation::Open { path } => SessionManager::open(path, session_dir, None),
        SessionOperation::ContinueRecent => {
            SessionManager::continue_recent(cwd, session_dir, agent_dir)
        }
        SessionOperation::Create => SessionManager::create(cwd, session_dir, agent_dir),
    }
}
