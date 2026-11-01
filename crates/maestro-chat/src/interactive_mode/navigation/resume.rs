//! Saved-session discovery and resuming another session.
use super::InteractiveMode;
use crate::components::session_selector::{SessionSelectorComponent, SessionSelectorOptions};
use maestro_app::presentation_data::input_selectors::{
    SessionListProgress, SessionSelectorCallbacks, SessionsLoader,
};
use maestro_app::{
    AgentSessionRuntimeError, EventError, ReadonlySessionManager, SessionChangeResult,
    SessionManager, SwitchSessionOptions, is_whitespace_scalar,
};
use maestro_tui::tui::LocalFuture;
use std::{
    error::Error,
    rc::{Rc, Weak},
};

impl InteractiveMode {
    /// Shows the saved sessions of this project, or of every project, in the editor's
    /// place; choosing one resumes it.
    ///
    /// # Errors
    /// Returns the failure of loading the selector's key bindings.
    pub(in crate::interactive_mode) fn show_session_selector(
        self: &Rc<Self>,
    ) -> Result<(), Box<dyn Error>> {
        self.show_selector(|done| {
            let runtime = Rc::clone(&self.tool_environment.runtime);
            let clock = Rc::clone(&runtime);
            let home = Rc::clone(&runtime);
            let selector = SessionSelectorComponent::new(
                current_sessions(Rc::downgrade(self)),
                all_sessions(self.runtime_host.services().agent_dir.clone()),
                self.session_callbacks(done),
                SessionSelectorOptions {
                    keybindings: Some((*self.keybindings).clone()),
                    show_rename_hint: Some(true),
                    rename_session: Some(Rc::new(rename_session)),
                    file_operations: None,
                    current_session_file_path: self.runtime_host.session().session_file(),
                    theme: self.themes.theme(),
                    runtime,
                    request_render: self.ui.weak_render_request(),
                    now_ms: Rc::new(move || {
                        let millis = clock.log_context().unix_ms;
                        millis.to_string().parse().unwrap_or(f64::NAN)
                    }),
                    home: Rc::new(move || home.log_context().home.to_string_lossy().into_owned()),
                    styles_enabled: Rc::new(|| true),
                },
            )?;
            Ok((selector.clone(), selector))
        })
    }

    /// Closes the selector before resuming or cancelling, and requests shutdown on exit,
    /// without retaining the mode.
    fn session_callbacks(self: &Rc<Self>, done: Rc<dyn Fn()>) -> SessionSelectorCallbacks {
        let (selecting, exiting) = (Rc::downgrade(self), Rc::downgrade(self));
        let closing = Rc::clone(&done);
        SessionSelectorCallbacks {
            on_select: Rc::new(move |path| {
                closing();
                if let Some(mode) = selecting.upgrade() {
                    spawn_resume(mode, path.to_owned());
                }
            }),
            on_cancel: self.cancelled_selection(done),
            on_exit: Rc::new(move || {
                if let Some(mode) = exiting.upgrade() {
                    mode.spawn_shutdown();
                }
            }),
        }
    }

    /// Stops the working row, then resumes the session at `session_path` with `options`'
    /// continuation. A missing session directory asks whether to resume in the offered
    /// one, retrying with the same continuation; any other failure is fatal.
    ///
    /// # Errors
    /// Returns the failure of the prompt, of the retried switch, of the replay or of the
    /// published theme.
    pub(in crate::interactive_mode) async fn handle_resume_session(
        self: &Rc<Self>,
        session_path: String,
        options: SwitchSessionOptions,
    ) -> Result<SessionChangeResult, EventError> {
        self.clear_status_indicator(None);
        let first = SwitchSessionOptions {
            with_session: options.with_session.clone(),
        };
        let error = match self
            .runtime_host
            .switch_session(&session_path, None, first)
            .await
        {
            Ok(result) => return self.resumed(result, "Resumed session"),
            Err(AgentSessionRuntimeError::MissingSessionCwd(error)) => error,
            Err(error) => self.handle_fatal_runtime_error("Failed to resume session", &error),
        };
        let selected = self.prompt_for_missing_session_cwd(&error).await?;
        let Some(cwd) = selected.filter(|cwd| !cwd.is_empty()) else {
            self.show_status("Resume cancelled")?;
            return Ok(SessionChangeResult { cancelled: true });
        };
        let result = self
            .runtime_host
            .switch_session(&session_path, Some(&cwd), options)
            .await?;
        self.resumed(result, "Resumed session in current cwd")
    }

    /// Replays the resumed session and shows `status`, unless the switch was cancelled.
    fn resumed(
        &self,
        result: SessionChangeResult,
        status: &str,
    ) -> Result<SessionChangeResult, EventError> {
        if !result.cancelled {
            self.render_current_session_state()?;
            self.show_status(status)?;
        }
        Ok(result)
    }
}

/// Resumes the session at `path` in the background, keeping the mode until it settles.
fn spawn_resume(mode: Rc<InteractiveMode>, path: String) {
    let runtime = Rc::clone(&mode.tool_environment.runtime);
    runtime.spawn_local(Box::pin(async move {
        let options = SwitchSessionOptions::default();
        mode.handle_resume_session(path, options)
            .await
            .map(drop)
            .map_err(|error| -> Box<dyn Error> { error })
    }));
}

/// Lists the current project's sessions, reading the live session's directory when invoked.
fn current_sessions(mode: Weak<InteractiveMode>) -> SessionsLoader {
    Rc::new(move |progress| {
        let mode = mode.upgrade();
        Box::pin(async move {
            let Some(mode) = mode else {
                return Ok(Vec::new());
            };
            let session = mode.runtime_host.session();
            let (cwd, dir) = {
                let manager = session.session_manager().borrow();
                (
                    manager.get_cwd().to_owned(),
                    manager.get_session_dir().to_owned(),
                )
            };
            let agent_dir = &mode.runtime_host.services().agent_dir;
            let mut observed = progress_adapter(progress);
            Ok(SessionManager::list(&cwd, Some(&dir), agent_dir, Some(&mut observed)).await?)
        })
    })
}

/// Lists the sessions of every project under `agent_dir`.
fn all_sessions(agent_dir: String) -> SessionsLoader {
    Rc::new(move |progress| {
        let agent_dir = agent_dir.clone();
        Box::pin(async move {
            let mut observed = progress_adapter(progress);
            Ok(SessionManager::list_all(&agent_dir, Some(&mut observed)).await)
        })
    })
}

/// Forwards discovery progress to the selector, never stopping the listing.
fn progress_adapter(
    progress: Option<SessionListProgress>,
) -> impl FnMut(usize, usize) -> std::io::Result<()> {
    move |loaded, total| {
        if let Some(progress) = &progress {
            progress(loaded, total);
        }
        Ok(())
    }
}

/// Records the trimmed nonblank `name` in the session file at `path`.
fn rename_session(path: &str, name: &str) -> LocalFuture {
    let (path, name) = (path.to_owned(), name.to_owned());
    Box::pin(async move {
        let name = name.trim_matches(is_whitespace_scalar);
        if !name.is_empty() {
            SessionManager::open(&path, None, None)?.append_session_info(name)?;
        }
        Ok(())
    })
}
