//! Interactive presentation for extension command-context session operations.
use super::InteractiveMode;
use maestro_app::{
    CommandContextOperations, EventError, ExtensionCommandContextActions, ForkOptions,
    NavigateTreeOptions, NewSessionCommandOptions, SessionChangeResult,
};
use std::rc::{Rc, Weak};

impl InteractiveMode {
    /// Binds session controls to the current mode without retaining it in its runner.
    /// Each admitted operation keeps the mode alive until that operation settles.
    pub(super) fn command_context_actions(self: &Rc<Self>) -> ExtensionCommandContextActions {
        let (idle, new, fork, navigate, switch, reload) = (
            self.this.clone(),
            self.this.clone(),
            self.this.clone(),
            self.this.clone(),
            self.this.clone(),
            self.this.clone(),
        );
        ExtensionCommandContextActions::new(CommandContextOperations {
            wait_for_idle: Rc::new(move || {
                let mode = current(&idle);
                Box::pin(async move {
                    mode?.runtime_host.session().wait_for_idle().await;
                    Ok(())
                })
            }),
            new_session: Rc::new(move |options| {
                let mode = current(&new);
                Box::pin(
                    async move { mode?.command_new_session(options.unwrap_or_default()).await },
                )
            }),
            fork: Rc::new(move |id, options| {
                let mode = current(&fork);
                Box::pin(async move { mode?.command_fork(&id, options.unwrap_or_default()).await })
            }),
            navigate_tree: Rc::new(move |id, options| {
                let mode = current(&navigate);
                Box::pin(async move {
                    mode?
                        .command_navigate_tree(&id, options.unwrap_or_default())
                        .await
                })
            }),
            switch_session: Rc::new(move |path, options| {
                let mode = current(&switch);
                Box::pin(async move {
                    mode?
                        .handle_resume_session(path, options.unwrap_or_default())
                        .await
                })
            }),
            reload: Rc::new(move || {
                let mode = current(&reload);
                Box::pin(async move { mode?.handle_reload_command().await })
            }),
        })
    }

    /// Replays a successful new session without adding the editor command's notice.
    async fn command_new_session(
        &self,
        options: NewSessionCommandOptions,
    ) -> Result<SessionChangeResult, EventError> {
        self.clear_status_indicator(None);
        let result: Result<_, EventError> = async {
            let result = self.runtime_host.new_session(options).await?;
            if !result.cancelled {
                self.render_current_session_state()?;
                self.ui.request_render(false);
            }
            Ok(result)
        }
        .await;
        match result {
            Ok(result) => Ok(result),
            Err(error) => {
                self.handle_fatal_runtime_error("Failed to create session", error.as_ref())
            }
        }
    }

    /// Replays a successful fork, restoring even an absent or empty selected draft.
    async fn command_fork(
        &self,
        id: &str,
        options: ForkOptions,
    ) -> Result<SessionChangeResult, EventError> {
        let result: Result<_, EventError> = async {
            let result = self.runtime_host.fork(id, options).await?;
            if !result.cancelled {
                self.render_current_session_state()?;
                self.current_editor()
                    .try_set_text(result.selected_text.as_deref().unwrap_or(""))?;
                self.show_status("Forked to new session")?;
            }
            Ok(SessionChangeResult {
                cancelled: result.cancelled,
            })
        }
        .await;
        match result {
            Ok(result) => Ok(result),
            Err(error) => self.handle_fatal_runtime_error("Failed to fork session", error.as_ref()),
        }
    }

    /// Replays successful navigation, preserves a live draft and starts queued delivery.
    async fn command_navigate_tree(
        self: &Rc<Self>,
        id: &str,
        options: NavigateTreeOptions,
    ) -> Result<SessionChangeResult, EventError> {
        let result = self
            .runtime_host
            .session()
            .navigate_tree(id, options)
            .await?;
        if result.cancelled {
            return Ok(SessionChangeResult { cancelled: true });
        }
        self.present_tree_navigation(result)?;
        Ok(SessionChangeResult { cancelled: false })
    }
}

/// Resolves an admitted callback's mode, just as the runtime resolves its weak owner.
fn current(mode: &Weak<InteractiveMode>) -> Result<Rc<InteractiveMode>, EventError> {
    mode.upgrade()
        .ok_or_else(|| "the interactive mode has ended".into())
}
