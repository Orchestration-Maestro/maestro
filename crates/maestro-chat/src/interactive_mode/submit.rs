//! Editor submission, its session commands, the single waiting input operation and
//! the prompt loop.
use super::InteractiveMode;
use super::bash::selected_shell_command;
use maestro_app::messages::UserMessageDelivery;
use maestro_app::{ImageContent, PromptOptions, is_whitespace_scalar};
use maestro_tui::EditorComponent;
use maestro_tui::tui::LocalFuture;
use std::{
    error::Error,
    future::{Future, pending},
    rc::Rc,
};
use tokio::sync::oneshot;

impl InteractiveMode {
    /// Waits for the next submitted editor text the mode handles as input, trimmed.
    ///
    /// The returned future owns its wait and does not borrow the mode. Its first poll
    /// makes it the only waiter: submissions made before then are not kept for it, and a
    /// later call's first poll replaces it, leaving it pending. A submission the mode
    /// handles as a command is not delivered.
    pub fn get_user_input(&self) -> impl Future<Output = String> + use<> {
        let slot = Rc::clone(&self.on_input_callback);
        async move {
            let (sender, receiver) = oneshot::channel();
            let displaced = slot.replace(Some(sender));
            drop(displaced);
            match receiver.await {
                Ok(text) => text,
                Err(_) => pending().await,
            }
        }
    }

    /// Installs submission on the editor without keeping the mode alive: a shell command
    /// runs through the mode, a recognized command runs and reports its own outcome, and
    /// any other trimmed nonempty text is queued as steering during a compaction or a
    /// response and otherwise moves the pending shell rows into the transcript, goes to
    /// the waiting input, if any, then into the editor's history.
    ///
    /// The editor empties itself before calling back. Quit also clears explicitly before
    /// admitting shutdown, without requesting another frame.
    pub(super) fn setup_editor_submit_handler(self: &Rc<Self>) {
        let mode = Rc::downgrade(self);
        self.editor.set_on_submit(Some(Rc::new(move |text| {
            if let Some(mode) = mode.upgrade() {
                mode.handle_editor_submit(text);
            }
        })));
    }

    /// Handles submitted text through the current editor, keeping original call failures.
    fn handle_editor_submit(self: &Rc<Self>, text: &str) {
        let text = text.trim_matches(is_whitespace_scalar);
        if text.is_empty() {
            return;
        }
        if text == "/quit" {
            if let Err(error) = self.current_editor().try_set_text("") {
                return self.report(Err(error));
            }
            self.spawn_shutdown();
            return;
        }
        if let Some((command, exclude_from_context)) = selected_shell_command(text) {
            self.submit_shell_command(text, command, exclude_from_context);
            return;
        }
        if text == "/login" {
            self.report(self.show_login_auth_type_selector());
            self.report(
                self.current_editor()
                    .try_set_text("")
                    .map_err(|error| -> Box<dyn Error> { error }),
            );
            return;
        }
        if text == "/logout" {
            self.report(self.show_logout_selector());
            self.report(
                self.current_editor()
                    .try_set_text("")
                    .map_err(|error| -> Box<dyn Error> { error }),
            );
            return;
        }
        if self.runs_navigation_command(text)
            || self.runs_command(text)
            || self.runs_session_command(text)
            || self.runs_model_command(text)
        {
            return;
        }
        match self.submit_queued_input(text, UserMessageDelivery::Steer) {
            Ok(false) => {}
            queued => return self.report(queued.map(drop)),
        }
        self.flush_pending_bash_components();
        let waiting = self.on_input_callback.borrow_mut().take();
        if let Some(waiting) = waiting {
            // A dropped waiter no longer wants the text; history still records it.
            drop(waiting.send(text.to_owned()));
        }
        if let Err(error) = self.current_editor().try_add_to_history(text) {
            self.report(Err(error));
        }
    }

    /// Runs the session command `text` names, handing a failure beyond the transcript to
    /// the terminal runtime, and says whether `text` was handled as a command.
    fn runs_command(self: &Rc<Self>, text: &str) -> bool {
        if text == "/import" || text.starts_with("/import ") {
            let mode = Rc::clone(self);
            let text = text.to_owned();
            self.tool_environment
                .runtime
                .spawn_local(Box::pin(async move {
                    mode.handle_import_command(&text).await?;
                    mode.clear_editor();
                    Ok(())
                }));
            return true;
        }
        let share = text == "/share";
        if !share && text != "/copy" {
            let result = if text == "/name" || text.starts_with("/name ") {
                self.handle_name_command(text)
            } else if text == "/settings" {
                self.show_settings_selector()
            } else if text == "/session" {
                self.handle_session_command()
            } else if text == "/debug" {
                self.handle_debug_command()
            } else if text == "/hotkeys" {
                self.handle_hotkeys_command()
            } else if text == "/changelog" {
                self.handle_changelog_command()
            } else if text == "/export" || text.starts_with("/export ") {
                self.handle_export_command(text)
            } else {
                return false;
            };
            if let Err(error) = result {
                self.report(Err(error));
            }
            return true;
        }
        let mode = Rc::clone(self);
        self.tool_environment
            .runtime
            .spawn_local(Box::pin(async move {
                let settled = if share {
                    mode.handle_share_command().await
                } else {
                    mode.handle_copy_command().await
                };
                mode.clear_editor();
                settled
            }));
        true
    }

    /// Opens the fork, tree or saved-session panel, or starts a clone, without delivering a
    /// prompt.
    fn runs_navigation_command(self: &Rc<Self>, text: &str) -> bool {
        match text {
            "/fork" => self.report(self.show_user_message_selector()),
            "/tree" => self.report(self.show_tree_selector(None)),
            "/resume" => self.report(self.show_session_selector()),
            "/clone" => {
                if let Err(error) = self.current_editor().try_set_text("") {
                    self.report(Err(error));
                    return true;
                }
                let mode = Rc::clone(self);
                self.tool_environment
                    .runtime
                    .spawn_local(Box::pin(async move { mode.handle_clone_command().await }));
            }
            _ => return false,
        }
        self.report(
            self.current_editor()
                .try_set_text("")
                .map_err(|error| -> Box<dyn Error> { error }),
        );
        true
    }

    /// Starts `/new`, `/reload`, or `/compact` with its instructions when matched, after
    /// clearing the editor, and says whether `text` was handled as a command.
    fn runs_session_command(self: &Rc<Self>, text: &str) -> bool {
        let work: LocalFuture = if text == "/new" {
            Box::pin(Rc::clone(self).handle_clear_command())
        } else if text == "/reload" {
            let mode = Rc::clone(self);
            Box::pin(async move {
                mode.handle_reload_command()
                    .await
                    .map_err(|error| error as Box<dyn Error>)
            })
        } else if text == "/compact" || text.starts_with("/compact ") {
            let instructions = text
                .strip_prefix("/compact ")
                .map(|rest| rest.trim_matches(is_whitespace_scalar).to_owned());
            Box::pin(Rc::clone(self).handle_compact_command(instructions))
        } else {
            return false;
        };
        if let Err(error) = self.current_editor().try_set_text("") {
            self.report(Err(error));
            return true;
        }
        self.tool_environment.runtime.spawn_local(work);
        true
    }

    /// Runs model or thinking selection when `text` names their command, and says
    /// whether it was handled as a command.
    fn runs_model_command(self: &Rc<Self>, text: &str) -> bool {
        if text == "/thinking" || text.starts_with("/thinking ") {
            let result = self
                .current_editor()
                .try_set_text("")
                .map_err(|error| -> Box<dyn Error> { error })
                .and_then(|()| {
                    self.handle_thinking_command(
                        text["/thinking".len()..].trim_matches(is_whitespace_scalar),
                    )
                });
            self.report(result);
            return true;
        }
        if text == "/scoped-models" {
            self.report(self.show_models_selector());
            return true;
        }
        if text != "/model" && !text.starts_with("/model ") {
            return false;
        }
        let search = text["/model".len()..].trim_matches(is_whitespace_scalar);
        let search = Some(search.to_owned()).filter(|search| !search.is_empty());
        let mode = Rc::clone(self);
        let runtime = Rc::clone(&self.tool_environment.runtime);
        runtime.spawn_local(Box::pin(
            async move { mode.handle_model_command(search).await },
        ));
        true
    }
}

impl InteractiveMode {
    /// Initializes the presentation, starts independent checks, shows startup warnings,
    /// sends the startup prompts,
    /// then keeps sending typed prompts to the current session.
    ///
    /// A prompt the session refuses is shown as an error and the loop continues.
    ///
    /// # Errors
    /// Returns an initialization or notice-presentation failure.
    pub async fn run(self: &Rc<Self>) -> Result<(), Box<dyn Error>> {
        self.init().await?;
        self.start_startup_checks();
        if !self.options.migrated_providers.is_empty() {
            self.show_warning(&format!(
                "Migrated credentials to auth.json: {}",
                self.options.migrated_providers.join(", ")
            ))?;
        }
        if let Some(error) = self
            .runtime_host
            .session()
            .model_registry()
            .get_error()
            .filter(|error| !error.is_empty())
        {
            self.show_error(&format!("models.json error: {error}"))?;
        }
        if let Some(message) = self
            .options
            .model_fallback_message
            .as_deref()
            .filter(|message| !message.is_empty())
        {
            self.show_warning(message)?;
        }
        self.check_subscription_warning(None);
        let first = self
            .options
            .initial_message
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(|text| (text, self.options.initial_images.as_deref()));
        let rest = self.options.initial_messages.iter();
        self.run_prompts(
            first
                .into_iter()
                .chain(rest.map(|text| (text.as_str(), None))),
        )
        .await
    }

    /// Sends each startup prompt once, in order, then each text the input waiter selects.
    ///
    /// # Errors
    /// Returns a failure presenting a refused prompt.
    pub(super) async fn run_prompts<'a>(
        &self,
        mut startup: impl Iterator<Item = (&'a str, Option<&'a [ImageContent]>)>,
    ) -> Result<(), Box<dyn Error>> {
        loop {
            let typed;
            let (text, images) = if let Some(prompt) = startup.next() {
                prompt
            } else {
                typed = self.get_user_input().await;
                (typed.as_str(), None)
            };
            let options = PromptOptions {
                images: images.map(<[_]>::to_vec),
                ..PromptOptions::default()
            };
            if let Err(error) = self.runtime_host.session().prompt(text, options).await {
                self.show_error(&error.message)?;
            }
        }
    }
}
