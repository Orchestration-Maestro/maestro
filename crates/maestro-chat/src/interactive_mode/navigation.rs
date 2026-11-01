//! Forking from an earlier user message, cloning the current leaf and the double interrupt
//! that opens the configured navigation.
mod resume;
mod tree;

use super::InteractiveMode;
use crate::components::user_message_selector::{
    UserMessageItem, UserMessageSelectorComponent, UserMessageSelectorOptions,
};
use maestro_app::{DoubleEscapeAction, ForkOptions, ForkPosition, ReadonlySessionManager};
use maestro_tui::Component;
use std::{error::Error, rc::Rc};

impl InteractiveMode {
    /// Shows the earlier user messages in the editor's place, forks the chosen one and
    /// reports the outcome; a session without messages reports that nothing can be forked.
    ///
    /// # Errors
    /// Returns the failure of the published theme or of building the selector.
    pub(super) fn show_user_message_selector(self: &Rc<Self>) -> Result<(), Box<dyn Error>> {
        let messages = self.runtime_host.session().get_user_messages_for_forking();
        if messages.is_empty() {
            return self
                .show_status("No messages to fork from")
                .map_err(Into::into);
        }
        let initial_selected_id = messages.last().map(|message| message.entry_id.clone());
        let items: Vec<_> = messages
            .iter()
            .map(|message| UserMessageItem {
                id: message.entry_id.clone(),
                text: message.text.clone(),
                timestamp: None,
            })
            .collect();
        self.show_selector(|done| {
            let selector = Rc::new(UserMessageSelectorComponent::new(
                items,
                self.fork_selection(Rc::clone(&done)),
                self.cancelled_selection(Rc::clone(&done)),
                UserMessageSelectorOptions {
                    initial_selected_id,
                    theme: self.themes.theme(),
                    runtime: Rc::clone(&self.tool_environment.runtime),
                },
            ));
            let focus: Rc<dyn Component> = selector.get_message_list().clone();
            Ok((selector, focus))
        })
    }

    /// The selection callback: forks before `entry_id` on the terminal runtime.
    fn fork_selection(self: &Rc<Self>, done: Rc<dyn Fn()>) -> Rc<dyn Fn(&str)> {
        let mode = Rc::downgrade(self);
        Rc::new(move |entry_id: &str| {
            if let Some(mode) = mode.upgrade() {
                spawn_fork(mode, entry_id.to_owned(), Rc::clone(&done));
            }
        })
    }

    /// The cancellation callback: closes the selector, then requests a frame.
    fn cancelled_selection(self: &Rc<Self>, done: Rc<dyn Fn()>) -> Rc<dyn Fn()> {
        let mode = Rc::downgrade(self);
        Rc::new(move || {
            done();
            if let Some(mode) = mode.upgrade() {
                mode.ui.request_render(false);
            }
        })
    }

    /// Forks the current leaf at its own position, so the replacement session starts from
    /// exactly this branch, then reports the outcome; a session without a leaf reports that
    /// nothing can be cloned.
    ///
    /// # Errors
    /// Returns the failure of the session's replay or of the published theme.
    pub(super) async fn handle_clone_command(self: &Rc<Self>) -> Result<(), Box<dyn Error>> {
        let leaf_id = self
            .runtime_host
            .session()
            .session_manager()
            .borrow()
            .get_leaf_id()
            .map(str::to_owned);
        let Some(leaf_id) = leaf_id else {
            return self.show_status("Nothing to clone yet").map_err(Into::into);
        };
        let options = ForkOptions {
            position: Some(ForkPosition::At),
            ..ForkOptions::default()
        };
        let result = match self.runtime_host.fork(&leaf_id, options).await {
            Ok(result) => result,
            Err(error) => return self.show_error(&error.to_string()).map_err(Into::into),
        };
        if result.cancelled {
            self.ui.request_render(false);
            return Ok(());
        }
        self.render_current_session_state()
            .map_err(|error| -> Box<dyn Error> { error })?;
        self.current_editor()
            .try_set_text("")
            .map_err(|error| -> Box<dyn Error> { error })?;
        self.show_status("Cloned to new session")
            .map_err(Into::into)
    }

    /// Applies double interrupt navigation only when the current editor reports empty text.
    pub(super) fn handle_empty_editor_escape(self: &Rc<Self>) {
        match self.current_editor().try_get_text() {
            Ok(text)
                if text
                    .trim_matches(maestro_app::is_whitespace_scalar)
                    .is_empty() =>
            {
                self.handle_double_escape();
            }
            Ok(_) => {}
            Err(error) => self.report(Err(error)),
        }
    }

    /// Opens the tree or the fork panel, as configured, when a second interrupt press on
    /// the empty editor comes strictly within 500 ms of the unpaired first.
    pub(super) fn handle_double_escape(self: &Rc<Self>) {
        let session = self.runtime_host.session();
        let action = session
            .settings_manager()
            .borrow()
            .get_double_escape_action();
        if action == DoubleEscapeAction::None {
            return;
        }
        let now = self.lifecycle.effects.now_ms();
        if i128::from(now) - i128::from(self.last_escape_time.get()) < 500 {
            self.report(if action == DoubleEscapeAction::Tree {
                self.show_tree_selector(None)
            } else {
                self.show_user_message_selector()
            });
            self.last_escape_time.set(0);
        } else {
            self.last_escape_time.set(now);
        }
    }
}

/// Runs the fork in the background, keeping the mode until it settles.
fn spawn_fork(mode: Rc<InteractiveMode>, entry_id: String, done: Rc<dyn Fn()>) {
    let runtime = Rc::clone(&mode.tool_environment.runtime);
    runtime.spawn_local(Box::pin(async move {
        let reported = run_fork(&mode, &entry_id, &done).await;
        mode.report(reported);
        Ok::<(), Box<dyn Error>>(())
    }));
}

/// Forks before `entry_id` and reports the finished, cancelled or failed outcome.
async fn run_fork(
    mode: &Rc<InteractiveMode>,
    entry_id: &str,
    done: &Rc<dyn Fn()>,
) -> Result<(), Box<dyn Error>> {
    let outcome = mode
        .runtime_host
        .fork(entry_id, ForkOptions::default())
        .await;
    match outcome {
        Ok(result) if result.cancelled => {
            done();
            mode.ui.request_render(false);
            Ok(())
        }
        Ok(result) => fork_succeeded(mode, done, result.selected_text.as_deref()),
        Err(error) => {
            done();
            mode.show_error(&error.to_string()).map_err(Into::into)
        }
    }
}

/// Rebuilds the replacement session's presentation, applies the fork's selected text and
/// reports the fork.
fn fork_succeeded(
    mode: &Rc<InteractiveMode>,
    done: &Rc<dyn Fn()>,
    selected_text: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    mode.render_current_session_state()
        .map_err(|error| -> Box<dyn Error> { error })?;
    mode.current_editor()
        .try_set_text(selected_text.unwrap_or(""))
        .map_err(|error| -> Box<dyn Error> { error })?;
    done();
    mode.show_status("Forked to new session")
        .map_err(Into::into)
}
