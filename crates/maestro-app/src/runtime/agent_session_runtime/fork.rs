//! Forking the session before or at one of its entries.
use super::{AgentSessionRuntime, AgentSessionRuntimeError, CreateAgentSessionRuntimeOptions};
use crate::agent_session::{AgentSession, content_text, thrown};
use maestro_extensions::types::ForkOptions;
use maestro_extensions::{
    ForkPosition, SessionBeforeForkEvent, SessionShutdownReason, SessionStartReason,
};
use maestro_models::DiagnosticErrorInfo;
use maestro_session::session_manager::{NewSessionOptions, ReadonlySessionManager};
use maestro_session::{Presence, SessionManager};
use std::{cell::RefCell, rc::Rc};

/// The outcome of a fork.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ForkResult {
    /// An extension cancelled the fork.
    pub cancelled: bool,
    /// The text of the user message a fork before it starts from.
    pub selected_text: Option<String>,
}

impl AgentSessionRuntime {
    /// Forks the session at `entry_id`, or before it when it is a user message, unless an
    /// extension cancels. A persisted session forks into a new file; an in-memory one
    /// branches its own history. Rebinding and continuation retain the forked session
    /// if another operation replaces it while a callback is suspended.
    ///
    /// # Errors
    /// Returns `Invalid entry ID for forking`, `Persisted session is missing a session file`,
    /// `Failed to create forked session`, a history failure, an extension error listener's
    /// failure or the factory's failure.
    pub async fn fork(
        &self,
        entry_id: &str,
        options: ForkOptions,
    ) -> Result<ForkResult, AgentSessionRuntimeError> {
        let position = options.position.unwrap_or(ForkPosition::Before);
        if self.emit_before_fork(entry_id, position).await? {
            return Ok(ForkResult {
                cancelled: true,
                selected_text: None,
            });
        }
        let session = self.session();
        let (target_leaf_id, selected_text) =
            fork_target(&session.session_manager().borrow(), entry_id, position)?;
        let previous_session_file = session.session_file();
        let session_manager = if session.session_manager().borrow().is_persisted() {
            self.persisted_fork(target_leaf_id.as_deref())?
        } else {
            branch_in_place(
                &session,
                target_leaf_id.as_deref(),
                previous_session_file.clone(),
            )?
        };
        let (target, cwd) = {
            let manager = session_manager.borrow();
            let cwd = if target_leaf_id.is_some() && manager.is_persisted() {
                manager.get_cwd().to_owned()
            } else {
                self.cwd()
            };
            (manager.get_session_file().map(str::to_owned), cwd)
        };
        self.teardown_current(SessionShutdownReason::Fork, target)
            .await?;
        self.replace(CreateAgentSessionRuntimeOptions {
            cwd,
            agent_dir: self.services().agent_dir.clone(),
            session_manager,
            session_start_event: Some(Self::start_event(
                SessionStartReason::Fork,
                previous_session_file,
            )),
        })
        .await?;
        self.finish_session_replacement(&self.session(), options.with_session.as_ref())
            .await?;
        Ok(ForkResult {
            cancelled: false,
            selected_text,
        })
    }

    /// The history a persisted session forks into: a new file whose parent is the current
    /// one, or the current file's branch up to `target_leaf_id` written to a new file.
    fn persisted_fork(
        &self,
        target_leaf_id: Option<&str>,
    ) -> Result<Rc<RefCell<SessionManager>>, DiagnosticErrorInfo> {
        let session = self.session();
        let current_file = session.session_file().ok_or_else(|| {
            DiagnosticErrorInfo::error("Persisted session is missing a session file")
        })?;
        let session_dir = session
            .session_manager()
            .borrow()
            .get_session_dir()
            .to_owned();
        let manager = match target_leaf_id {
            None => {
                let agent_dir = self.services().agent_dir.clone();
                let mut manager =
                    SessionManager::create(&self.cwd(), Some(&session_dir), &agent_dir)
                        .map_err(thrown)?;
                manager.new_session(NewSessionOptions {
                    parent_session: Presence::Present(current_file),
                    ..NewSessionOptions::default()
                });
                manager
            }
            Some(leaf) => {
                let mut source = SessionManager::open(&current_file, Some(&session_dir), None)
                    .map_err(thrown)?;
                let forked = source
                    .create_branched_session(leaf)
                    .map_err(thrown)?
                    .ok_or_else(|| DiagnosticErrorInfo::error("Failed to create forked session"))?;
                SessionManager::open(&forked, Some(&session_dir), None).map_err(thrown)?
            }
        };
        Ok(Rc::new(RefCell::new(manager)))
    }

    /// Tells the extensions about a coming fork; whether one cancelled it.
    async fn emit_before_fork(
        &self,
        entry_id: &str,
        position: ForkPosition,
    ) -> Result<bool, DiagnosticErrorInfo> {
        let runner = self.session().extension_runner();
        if !runner.has_handlers("session_before_fork") {
            return Ok(false);
        }
        let mut event = SessionBeforeForkEvent {
            entry_id: entry_id.to_owned(),
            position,
        };
        let result = runner.emit(&mut event).await.map_err(thrown)?;
        Ok(result.is_some_and(|result| matches!(result.cancel, Presence::Present(true))))
    }
}

/// The in-memory history of `session`, moved to a new session whose parent is the current
/// file, or to its branch up to `target_leaf_id`.
fn branch_in_place(
    session: &AgentSession,
    target_leaf_id: Option<&str>,
    parent: Option<String>,
) -> Result<Rc<RefCell<SessionManager>>, DiagnosticErrorInfo> {
    let manager = Rc::clone(session.session_manager());
    match target_leaf_id {
        None => {
            manager.borrow_mut().new_session(NewSessionOptions {
                parent_session: parent.map_or(Presence::Missing, Presence::Present),
                ..NewSessionOptions::default()
            });
        }
        Some(leaf) => {
            manager
                .borrow_mut()
                .create_branched_session(leaf)
                .map_err(thrown)?;
        }
    }
    Ok(manager)
}

/// The leaf a fork keeps and, before a user message, that message's text.
///
/// # Errors
/// Returns `Invalid entry ID for forking` for an unknown entry, or for one that is not a
/// user message when forking before it.
fn fork_target(
    manager: &SessionManager,
    entry_id: &str,
    position: ForkPosition,
) -> Result<(Option<String>, Option<String>), DiagnosticErrorInfo> {
    let invalid = || DiagnosticErrorInfo::error("Invalid entry ID for forking");
    let entry = manager.get_entry(entry_id).ok_or_else(invalid)?;
    if position == ForkPosition::At {
        return Ok((entry["id"].as_str().map(str::to_owned), None));
    }
    let message = &entry["message"];
    if entry["type"] != "message" || message["role"] != "user" {
        return Err(invalid());
    }
    let text = content_text(message.get("content"));
    Ok((entry["parentId"].as_str().map(str::to_owned), Some(text)))
}
