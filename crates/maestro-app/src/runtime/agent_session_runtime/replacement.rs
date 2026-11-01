//! Replacing the session with a new one, another session file or an imported file.
use super::{
    AgentSessionRuntime, AgentSessionRuntimeError, CreateAgentSessionRuntimeOptions,
    SessionImportFileNotFoundError,
};
use crate::agent_session::thrown;
use maestro_extensions::types::{
    NewSessionCommandOptions, SessionChangeResult, SwitchSessionOptions,
};
use maestro_extensions::{SessionBeforeSwitchReason, SessionShutdownReason, SessionStartReason};
use maestro_session::session_cwd::assert_session_cwd_exists;
use maestro_session::session_manager::{NewSessionOptions, ReadonlySessionManager};
use maestro_session::{Presence, SessionManager};
use std::{cell::RefCell, path::Path, rc::Rc};

impl AgentSessionRuntime {
    /// Resumes the session file at `session_path`, in `cwd_override` when supplied, unless an
    /// extension cancels the switch. Rebinding and continuation retain the resumed session
    /// if another operation replaces it while a callback is suspended.
    ///
    /// # Errors
    /// Returns the missing-directory error, a history failure, an extension error listener's
    /// failure or the factory's failure; a failure after the teardown restores nothing.
    pub async fn switch_session(
        &self,
        session_path: &str,
        cwd_override: Option<&str>,
        options: SwitchSessionOptions,
    ) -> Result<SessionChangeResult, AgentSessionRuntimeError> {
        if self
            .emit_before_switch(
                SessionBeforeSwitchReason::Resume,
                Some(session_path.to_owned()),
            )
            .await?
        {
            return Ok(SessionChangeResult { cancelled: true });
        }
        let previous_session_file = self.session().session_file();
        let session_manager =
            SessionManager::open(session_path, None, cwd_override).map_err(thrown)?;
        assert_session_cwd_exists(&session_manager, &self.cwd())?;
        self.resume(session_manager, previous_session_file).await?;
        self.finish_session_replacement(&self.session(), options.with_session.as_ref())
            .await?;
        Ok(SessionChangeResult { cancelled: false })
    }

    /// Starts a new session in the current directory, optionally recording `parent_session`,
    /// prepares its history with `setup` and continues with `with_session`, unless an
    /// extension cancels the switch. The new history is kept in memory exactly when the
    /// current one is, and stored in the current session directory otherwise. Setup, message
    /// reload and continuation retain the created session if another operation replaces it
    /// while a callback is suspended.
    ///
    /// # Errors
    /// Returns a history failure, an extension error listener's failure, the setup's failure
    /// or the factory's failure; a failure after the teardown restores nothing.
    pub async fn new_session(
        &self,
        options: NewSessionCommandOptions,
    ) -> Result<SessionChangeResult, AgentSessionRuntimeError> {
        if self
            .emit_before_switch(SessionBeforeSwitchReason::New, None)
            .await?
        {
            return Ok(SessionChangeResult { cancelled: true });
        }
        let session = self.session();
        let previous_session_file = session.session_file();
        let persisted = session.session_manager().borrow().is_persisted();
        let session_dir = session
            .session_manager()
            .borrow()
            .get_session_dir()
            .to_owned();
        let cwd = self.cwd();
        let agent_dir = self.services().agent_dir.clone();
        let mut session_manager = if persisted {
            SessionManager::create(&cwd, Some(&session_dir), &agent_dir)
        } else {
            SessionManager::in_memory(Some(&cwd))
        }
        .map_err(thrown)?;
        if let Some(parent_session) = options.parent_session {
            session_manager.new_session(NewSessionOptions {
                parent_session: Presence::Present(parent_session),
                ..NewSessionOptions::default()
            });
        }
        let target = session_manager.get_session_file().map(str::to_owned);
        self.teardown_current(SessionShutdownReason::New, target)
            .await?;
        self.replace(CreateAgentSessionRuntimeOptions {
            cwd,
            agent_dir,
            session_manager: Rc::new(RefCell::new(session_manager)),
            session_start_event: Some(Self::start_event(
                SessionStartReason::New,
                previous_session_file,
            )),
        })
        .await?;
        let session = self.session();
        if let Some(setup) = options.setup {
            setup(Rc::clone(session.session_manager()))
                .await
                .map_err(thrown)?;
            let context = session
                .session_manager()
                .borrow()
                .build_session_context()
                .map_err(thrown)?;
            let messages = crate::agent_session::agent_messages(context.messages)?;
            crate::utils::lock::write(session.state()).set_messages(messages);
        }
        self.finish_session_replacement(&session, options.with_session.as_ref())
            .await?;
        Ok(SessionChangeResult { cancelled: false })
    }

    /// Copies the session file at `input_path` into the session directory unless it is
    /// already there, then resumes it, in `cwd_override` when supplied, unless an extension
    /// cancels the switch.
    ///
    /// # Errors
    /// Returns [`AgentSessionRuntimeError::SessionImportFileNotFound`] for a missing file, the
    /// missing-directory error, a file or history failure, an extension error listener's
    /// failure or the factory's failure.
    pub async fn import_from_jsonl(
        &self,
        input_path: &str,
        cwd_override: Option<&str>,
    ) -> Result<SessionChangeResult, AgentSessionRuntimeError> {
        let resolved = maestro_resources::paths::resolve(&[input_path]).map_err(thrown)?;
        if !Path::new(&resolved).exists() {
            return Err(AgentSessionRuntimeError::SessionImportFileNotFound(
                SessionImportFileNotFoundError {
                    file_path: resolved,
                },
            ));
        }
        let session_dir = self
            .session()
            .session_manager()
            .borrow()
            .get_session_dir()
            .to_owned();
        std::fs::create_dir_all(&session_dir).map_err(thrown)?;
        let destination =
            maestro_path::join(&[&session_dir, &maestro_path::basename(&resolved, None)]);
        if self
            .emit_before_switch(SessionBeforeSwitchReason::Resume, Some(destination.clone()))
            .await?
        {
            return Ok(SessionChangeResult { cancelled: true });
        }
        let previous_session_file = self.session().session_file();
        if maestro_resources::paths::resolve(&[&destination]).map_err(thrown)? != resolved {
            std::fs::copy(&resolved, &destination).map_err(thrown)?;
        }
        let session_manager =
            SessionManager::open(&destination, Some(&session_dir), cwd_override).map_err(thrown)?;
        assert_session_cwd_exists(&session_manager, &self.cwd())?;
        self.resume(session_manager, previous_session_file).await?;
        self.finish_session_replacement(&self.session(), None)
            .await?;
        Ok(SessionChangeResult { cancelled: false })
    }

    /// Tears the current session down and resumes `session_manager` in its own directory.
    async fn resume(
        &self,
        session_manager: SessionManager,
        previous_session_file: Option<String>,
    ) -> Result<(), AgentSessionRuntimeError> {
        let target = session_manager.get_session_file().map(str::to_owned);
        self.teardown_current(SessionShutdownReason::Resume, target)
            .await?;
        self.replace(CreateAgentSessionRuntimeOptions {
            cwd: session_manager.get_cwd().to_owned(),
            agent_dir: self.services().agent_dir.clone(),
            session_manager: Rc::new(RefCell::new(session_manager)),
            session_start_event: Some(Self::start_event(
                SessionStartReason::Resume,
                previous_session_file,
            )),
        })
        .await?;
        Ok(())
    }
}
