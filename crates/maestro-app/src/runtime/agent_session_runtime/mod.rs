//! The current session and its directory-bound services, and the operations that replace
//! them: a new session, another session file, a fork and an imported file.
mod fork;
mod replacement;

pub use fork::ForkResult;

use super::agent_session_services::{AgentSessionRuntimeDiagnostic, AgentSessionServices};
use crate::agent_session::{AgentSession, thrown};
use maestro_extensions::runner::emit_session_shutdown_event;
use maestro_extensions::types::{SessionChangeResult, WithSession};
use maestro_extensions::{
    CommandContextOperations, EventError, ExtensionCommandContextActions, LoadExtensionsResult,
    SessionBeforeSwitchEvent, SessionBeforeSwitchReason, SessionShutdownEvent,
    SessionShutdownReason, SessionStartEvent,
};
use maestro_models::{DiagnosticErrorInfo, Execution, Local};
use maestro_session::{
    Presence, SessionManager,
    session_cwd::{MissingSessionCwdError, assert_session_cwd_exists},
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

/// The local future the runtime's callbacks return.
pub type RuntimeFuture<T> = <Local as Execution>::Future<'static, Result<T, DiagnosticErrorInfo>>;

/// Where and over which history the factory creates a runtime.
pub struct CreateAgentSessionRuntimeOptions {
    /// The session working directory.
    pub cwd: String,
    /// The user configuration directory.
    pub agent_dir: String,
    /// The session history.
    pub session_manager: Rc<RefCell<SessionManager>>,
    /// The start event the new session's extensions receive.
    pub session_start_event: Option<SessionStartEvent>,
}

/// A created session with its services and the problems found creating them.
pub struct CreateAgentSessionRuntimeResult {
    /// The session.
    pub session: Rc<AgentSession>,
    /// The services bound to the session's working directory.
    pub services: Rc<AgentSessionServices<Local>>,
    /// Problems found while creating the services and the session.
    pub diagnostics: Vec<AgentSessionRuntimeDiagnostic>,
    /// The loaded extensions.
    pub extensions_result: Rc<LoadExtensionsResult<Local>>,
    /// Why the recorded model was not restored, or that no model is available.
    pub model_fallback_message: Option<String>,
}

/// Creates the services and the session for one working directory and history; the runtime
/// keeps it for every replacement.
pub type CreateAgentSessionRuntimeFactory =
    Rc<dyn Fn(CreateAgentSessionRuntimeOptions) -> RuntimeFuture<CreateAgentSessionRuntimeResult>>;

/// Rebinds a host to a replacement session before the caller continues against it.
pub type RebindSession = Rc<dyn Fn(Rc<AgentSession>) -> RuntimeFuture<()>>;

/// Host teardown that runs before the current session is invalidated; its failure stops
/// the teardown before the session is disposed.
pub type BeforeSessionInvalidate = Rc<dyn Fn() -> Result<(), EventError>>;

/// Why a runtime operation failed.
#[derive(Debug)]
pub enum AgentSessionRuntimeError {
    /// The session's stored working directory no longer exists.
    MissingSessionCwd(MissingSessionCwdError),
    /// The file to import does not exist.
    SessionImportFileNotFound(SessionImportFileNotFoundError),
    /// Any other failure.
    Failed(DiagnosticErrorInfo),
}

impl From<DiagnosticErrorInfo> for AgentSessionRuntimeError {
    fn from(error: DiagnosticErrorInfo) -> Self {
        Self::Failed(error)
    }
}

impl From<MissingSessionCwdError> for AgentSessionRuntimeError {
    fn from(error: MissingSessionCwdError) -> Self {
        Self::MissingSessionCwd(error)
    }
}

impl std::fmt::Display for AgentSessionRuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingSessionCwd(error) => error.fmt(formatter),
            Self::SessionImportFileNotFound(error) => error.fmt(formatter),
            Self::Failed(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for AgentSessionRuntimeError {}

/// The file an import names does not exist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionImportFileNotFoundError {
    /// The resolved path of the missing file.
    pub file_path: String,
}

impl std::fmt::Display for SessionImportFileNotFoundError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "File not found: {}", self.file_path)
    }
}

impl std::error::Error for SessionImportFileNotFoundError {}

/// The runtime a stored command operation belongs to, while its caller keeps it.
fn current(runtime: &Weak<AgentSessionRuntime>) -> Result<Rc<AgentSessionRuntime>, EventError> {
    runtime
        .upgrade()
        .ok_or_else(|| "the session runtime has ended".into())
}

/// The session and services now in effect.
struct Current {
    /// The session.
    session: Rc<AgentSession>,
    /// Its services.
    services: Rc<AgentSessionServices<Local>>,
    /// Problems found while creating them.
    diagnostics: Vec<AgentSessionRuntimeDiagnostic>,
    /// Why the recorded model was not restored.
    model_fallback_message: Option<String>,
}

/// Owns the current session and its directory-bound services. A replacement tears the
/// current one down first, then creates and applies the next; a creation failure is returned
/// without restoring anything.
pub struct AgentSessionRuntime {
    /// The session and services now in effect.
    current: RefCell<Current>,
    /// Creates every replacement.
    create_runtime: CreateAgentSessionRuntimeFactory,
    /// Rebinds the host to each replacement.
    rebind_session: RefCell<Option<RebindSession>>,
    /// Runs after the shutdown handlers and before the session is invalidated.
    before_session_invalidate: RefCell<Option<BeforeSessionInvalidate>>,
}

impl AgentSessionRuntime {
    /// A runtime over a created session and its services, keeping `create_runtime` for
    /// replacements.
    #[must_use]
    pub fn new(
        result: CreateAgentSessionRuntimeResult,
        create_runtime: CreateAgentSessionRuntimeFactory,
    ) -> Self {
        Self {
            current: RefCell::new(Current {
                session: result.session,
                services: result.services,
                diagnostics: result.diagnostics,
                model_fallback_message: result.model_fallback_message,
            }),
            create_runtime,
            rebind_session: RefCell::default(),
            before_session_invalidate: RefCell::default(),
        }
    }

    /// The services bound to the current session's working directory.
    #[must_use]
    pub fn services(&self) -> Rc<AgentSessionServices<Local>> {
        Rc::clone(&self.current.borrow().services)
    }

    /// The current session.
    #[must_use]
    pub fn session(&self) -> Rc<AgentSession> {
        Rc::clone(&self.current.borrow().session)
    }

    /// The current working directory.
    #[must_use]
    pub fn cwd(&self) -> String {
        self.current.borrow().services.cwd.clone()
    }

    /// Problems found while creating the current services and session.
    #[must_use]
    pub fn diagnostics(&self) -> Vec<AgentSessionRuntimeDiagnostic> {
        self.current.borrow().diagnostics.clone()
    }

    /// Why the current session's recorded model was not restored.
    #[must_use]
    pub fn model_fallback_message(&self) -> Option<String> {
        self.current.borrow().model_fallback_message.clone()
    }

    /// Sets the operation that rebinds the host to each replacement session.
    pub fn set_rebind_session(&self, rebind_session: Option<RebindSession>) {
        *self.rebind_session.borrow_mut() = rebind_session;
    }

    /// Sets a synchronous callback that runs after the shutdown handlers finish and before
    /// the current session is invalidated, for host teardown that must not wait, such as
    /// detaching extension components before their context becomes stale. Its failure is
    /// the operation's failure, and the session is then neither invalidated nor disposed.
    pub fn set_before_session_invalidate(
        &self,
        before_session_invalidate: Option<BeforeSessionInvalidate>,
    ) {
        *self.before_session_invalidate.borrow_mut() = before_session_invalidate;
    }

    /// The command-context operations a host binds for the current session.
    ///
    /// Each operation resolves the runtime and its current session when it runs, so a command
    /// reaches the session a replacement installed.
    #[must_use]
    pub fn command_context_actions(self: &Rc<Self>) -> ExtensionCommandContextActions {
        let runtime = Rc::downgrade(self);
        let (idle, new, fork, navigate, switch, reload) = (
            runtime.clone(),
            runtime.clone(),
            runtime.clone(),
            runtime.clone(),
            runtime.clone(),
            runtime,
        );
        ExtensionCommandContextActions::new(CommandContextOperations {
            wait_for_idle: Rc::new(move || {
                let runtime = current(&idle);
                Box::pin(async move {
                    runtime?.session().wait_for_idle().await;
                    Ok(())
                })
            }),
            new_session: Rc::new(move |options| {
                let runtime = current(&new);
                Box::pin(
                    async move { Ok(runtime?.new_session(options.unwrap_or_default()).await?) },
                )
            }),
            fork: Rc::new(move |entry_id, options| {
                let runtime = current(&fork);
                Box::pin(async move {
                    let forked = runtime?
                        .fork(&entry_id, options.unwrap_or_default())
                        .await?;
                    Ok(SessionChangeResult {
                        cancelled: forked.cancelled,
                    })
                })
            }),
            navigate_tree: Rc::new(move |target_id, options| {
                let runtime = current(&navigate);
                Box::pin(async move {
                    let session = runtime?.session();
                    let navigated = session
                        .navigate_tree(&target_id, options.unwrap_or_default())
                        .await?;
                    Ok(SessionChangeResult {
                        cancelled: navigated.cancelled,
                    })
                })
            }),
            switch_session: Rc::new(move |session_path, options| {
                let runtime = current(&switch);
                Box::pin(async move {
                    Ok(runtime?
                        .switch_session(&session_path, None, options.unwrap_or_default())
                        .await?)
                })
            }),
            reload: Rc::new(move || {
                let runtime = current(&reload);
                Box::pin(async move { Ok(runtime?.session().reload().await?) })
            }),
        })
    }

    /// Tells the extensions about a coming switch; whether one cancelled it.
    async fn emit_before_switch(
        &self,
        reason: SessionBeforeSwitchReason,
        target_session_file: Option<String>,
    ) -> Result<bool, DiagnosticErrorInfo> {
        let runner = self.session().extension_runner();
        if !runner.has_handlers("session_before_switch") {
            return Ok(false);
        }
        let mut event = SessionBeforeSwitchEvent {
            reason,
            target_session_file: target_session_file.map_or(Presence::Missing, Presence::Present),
        };
        let result = runner.emit(&mut event).await.map_err(thrown)?;
        Ok(result.is_some_and(|result| matches!(result.cancel, Presence::Present(true))))
    }

    /// Shuts the current session down for `reason`, runs the invalidation callback and
    /// disposes the session; a callback failure stops before the disposal.
    async fn teardown_current(
        &self,
        reason: SessionShutdownReason,
        target_session_file: Option<String>,
    ) -> Result<(), DiagnosticErrorInfo> {
        let session = self.session();
        let mut event = SessionShutdownEvent {
            reason,
            target_session_file: target_session_file.map_or(Presence::Missing, Presence::Present),
        };
        emit_session_shutdown_event(&session.extension_runner(), &mut event)
            .await
            .map_err(thrown)?;
        let callback = self.before_session_invalidate.borrow().clone();
        if let Some(callback) = callback {
            callback().map_err(thrown)?;
        }
        session.dispose();
        Ok(())
    }

    /// Creates the next runtime and makes it current.
    async fn replace(
        &self,
        options: CreateAgentSessionRuntimeOptions,
    ) -> Result<(), DiagnosticErrorInfo> {
        let result = (self.create_runtime)(options).await?;
        *self.current.borrow_mut() = Current {
            session: result.session,
            services: result.services,
            diagnostics: result.diagnostics,
            model_fallback_message: result.model_fallback_message,
        };
        Ok(())
    }

    /// Rebinds the host to the captured replacement, then continues against that session
    /// even if another operation replaces the current session during rebinding.
    async fn finish_session_replacement(
        &self,
        session: &Rc<AgentSession>,
        with_session: Option<&WithSession>,
    ) -> Result<(), DiagnosticErrorInfo> {
        let rebind = self.rebind_session.borrow().clone();
        if let Some(rebind) = rebind {
            rebind(Rc::clone(session)).await?;
        }
        if let Some(with_session) = with_session {
            with_session(session.create_replaced_session_context())
                .await
                .map_err(thrown)?;
        }
        Ok(())
    }

    /// The start event of a replacement for `reason`, carrying the previous session file.
    fn start_event(
        reason: maestro_extensions::SessionStartReason,
        previous: Option<String>,
    ) -> SessionStartEvent {
        SessionStartEvent {
            reason,
            previous_session_file: previous.map_or(Presence::Missing, Presence::Present),
        }
    }

    /// Shuts the current session down for the last time and disposes it.
    ///
    /// # Errors
    /// Returns an extension error listener's failure.
    pub async fn dispose(&self) -> Result<(), DiagnosticErrorInfo> {
        self.teardown_current(SessionShutdownReason::Quit, None)
            .await
    }
}

/// Creates the first runtime from `create_runtime` once the stored working directory is
/// known to exist, keeping the factory for later replacements.
///
/// # Errors
/// Returns the missing-directory error or the factory's failure.
pub async fn create_agent_session_runtime(
    create_runtime: CreateAgentSessionRuntimeFactory,
    options: CreateAgentSessionRuntimeOptions,
) -> Result<AgentSessionRuntime, AgentSessionRuntimeError> {
    assert_session_cwd_exists(&*options.session_manager.borrow(), &options.cwd)?;
    let result = create_runtime(options).await?;
    Ok(AgentSessionRuntime::new(result, create_runtime))
}
