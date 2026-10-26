//! Queued steering and follow-up input: routing input typed during a response or a
//! compaction, the pending rows above the editor, restoring the queued text to the editor
//! and the queue key actions.
use super::InteractiveMode;
use maestro_app::agent_session::{PromptOptions, QueueTexts};
use maestro_app::is_whitespace_scalar;
use maestro_app::messages::UserMessageDelivery;
use maestro_theme::ThemeColor;
use maestro_tui::{Spacer, TruncatedText};
use std::{error::Error, iter, rc::Rc};

/// A message typed while the session compacts, kept until the compaction ends.
#[derive(Clone)]
pub(super) struct CompactionQueuedMessage {
    /// The selected text.
    pub(super) text: String,
    /// How the message is delivered.
    pub(super) mode: UserMessageDelivery,
}

/// How queued text returns to the editor.
#[derive(Clone, Copy, Default)]
pub(super) struct RestoreQueuedMessagesOptions<'a> {
    /// Whether the running turn stops after the text is restored.
    pub(super) abort: bool,
    /// The draft kept after the queued text, instead of the editor's current text.
    pub(super) current_text: Option<&'a str>,
}

impl InteractiveMode {
    /// Installs the queue actions on the editor without keeping the mode alive: the
    /// interrupt key restores the queued text and stops a streaming response, cancels a
    /// running shell, or else clears a shell command from the editor and restores the
    /// thinking border, or on the empty editor opens the configured navigation after a
    /// second press; the follow-up key sends the editor's text as a follow-up, and the
    /// dequeue key restores the queued text.
    pub(super) fn setup_queue_key_handlers(self: &Rc<Self>) {
        let mode = Rc::downgrade(self);
        self.editor.set_on_escape(Some(Rc::new(move || {
            let Some(mode) = mode.upgrade() else {
                return;
            };
            let session = mode.runtime_host.session();
            if session.is_streaming() {
                let options = RestoreQueuedMessagesOptions {
                    abort: true,
                    current_text: None,
                };
                return mode.report(mode.restore_queued_messages_to_editor(options).map(drop));
            }
            if session.is_bash_running() {
                session.abort_bash();
                return;
            }
            if !mode.is_bash_mode.get() {
                mode.handle_empty_editor_escape();
                return;
            }
            if let Err(error) = mode.current_editor().try_set_text("") {
                return mode.report(Err(error));
            }
            mode.is_bash_mode.set(false);
            mode.report(mode.update_editor_border_color().map_err(Into::into));
        })));
        let mode = Rc::downgrade(self);
        self.editor.on_action(
            "app.message.followUp".to_owned(),
            Rc::new(move || {
                if let Some(mode) = mode.upgrade() {
                    mode.report(mode.handle_follow_up());
                }
            }),
        );
        let mode = Rc::downgrade(self);
        self.editor.on_action(
            "app.message.dequeue".to_owned(),
            Rc::new(move || {
                if let Some(mode) = mode.upgrade() {
                    mode.report(mode.handle_dequeue());
                }
            }),
        );
    }

    /// Sends the editor's text, expanded when the editor expands it and trimmed, as a
    /// follow-up: it is queued during a compaction or a response, and otherwise the editor
    /// is emptied and the text goes to the installed submission. Blank text is ignored.
    ///
    /// # Errors
    /// Returns the editor's original failure or the published theme's dim-colour failure.
    pub(super) fn handle_follow_up(self: &Rc<Self>) -> Result<(), Box<dyn Error>> {
        let text = self
            .expanded_editor_text()
            .map_err(|error| -> Box<dyn Error> { error })?;
        let text = text.trim_matches(is_whitespace_scalar);
        if text.is_empty() || self.submit_queued_input(text, UserMessageDelivery::FollowUp)? {
            return Ok(());
        }
        let editor = self.current_editor();
        if let Some(submit) = editor.on_submit() {
            editor
                .try_set_text("")
                .map_err(|error| -> Box<dyn Error> { error })?;
            submit(text);
        }
        Ok(())
    }

    /// Queues `text` for `mode` when the session compacts or streams, returning whether
    /// it did. During a compaction a registered command runs at once and other text is
    /// kept until the compaction ends; while a response streams, the text is prompted for
    /// `mode` and the pending rows are refreshed once it is queued.
    ///
    /// # Errors
    /// Returns the editor's original failure or the published theme's dim-colour failure.
    pub(super) fn submit_queued_input(
        self: &Rc<Self>,
        text: &str,
        mode: UserMessageDelivery,
    ) -> Result<bool, Box<dyn Error>> {
        let session = self.runtime_host.session();
        if session.is_compacting() {
            if !self.is_extension_command(text) {
                self.queue_compaction_message(text.to_owned(), mode)?;
                return Ok(true);
            }
            self.start_prompt(text, None);
        } else if session.is_streaming() {
            self.start_prompt(text, Some(mode));
        } else {
            return Ok(false);
        }
        Ok(true)
    }

    /// Records `text` in the history, empties the editor and starts the session prompt
    /// now. The terminal host awaits it and reports its failure; with a streaming delivery
    /// the pending rows are refreshed and a frame requested once it succeeds. Until then
    /// the prompt keeps the mode alive.
    fn start_prompt(self: &Rc<Self>, text: &str, streaming_behavior: Option<UserMessageDelivery>) {
        if let Err(error) = self.current_editor().try_add_to_history(text) {
            return self.report(Err(error));
        }
        if let Err(error) = self.current_editor().try_set_text("") {
            return self.report(Err(error));
        }
        let refresh = streaming_behavior.is_some();
        let options = PromptOptions {
            streaming_behavior,
            ..PromptOptions::default()
        };
        let prompt = self.runtime_host.session().prompt(text, options);
        let mode = Rc::clone(self);
        self.tool_environment
            .runtime
            .spawn_local(Box::pin(async move {
                prompt.await?;
                if refresh {
                    mode.update_pending_messages_display()?;
                    mode.ui.request_render(false);
                }
                Ok(())
            }));
    }

    /// Keeps `text` for `mode` until the compaction ends, records it in the history,
    /// empties the editor, refreshes the pending rows and says so on the status line.
    ///
    /// # Errors
    /// Returns the editor's original failure or the published theme's dim-colour failure.
    pub(super) fn queue_compaction_message(
        &self,
        text: String,
        mode: UserMessageDelivery,
    ) -> Result<(), Box<dyn Error>> {
        self.current_editor()
            .try_add_to_history(&text)
            .map_err(|error| -> Box<dyn Error> { error })?;
        self.compaction_queued_messages
            .borrow_mut()
            .push(CompactionQueuedMessage { text, mode });
        self.current_editor()
            .try_set_text("")
            .map_err(|error| -> Box<dyn Error> { error })?;
        self.update_pending_messages_display()?;
        self.show_status("Queued message for after compaction")
            .map_err(Into::into)
    }

    /// Whether `text` starts with `/` and names a registered command, up to its first space.
    pub(super) fn is_extension_command(&self, text: &str) -> bool {
        let Some(command) = text.strip_prefix('/') else {
            return false;
        };
        let name = command.split_once(' ').map_or(command, |(name, _)| name);
        let runner = self.runtime_host.session().extension_runner();
        runner.get_command(name).is_some()
    }

    /// Restores every queued message to the editor and says how many were restored.
    ///
    /// # Errors
    /// Returns the editor's original failure or the published theme's dim-colour failure.
    pub(super) fn handle_dequeue(&self) -> Result<(), Box<dyn Error>> {
        let restored =
            self.restore_queued_messages_to_editor(RestoreQueuedMessagesOptions::default())?;
        self.show_status(&match restored {
            0 => "No queued messages to restore".to_owned(),
            1 => "Restored 1 queued message to editor".to_owned(),
            count => format!("Restored {count} queued messages to editor"),
        })
        .map_err(Into::into)
    }

    /// The session's queued steering and follow-up texts, each followed by the messages
    /// kept during compaction with the same delivery, all left queued.
    pub(super) fn get_all_queued_messages(&self) -> QueueTexts {
        let session = self.runtime_host.session();
        let mut queued = QueueTexts {
            steering: session.get_steering_messages(),
            follow_up: session.get_follow_up_messages(),
        };
        let local = self.compaction_queued_messages.borrow().clone();
        append_compaction_queue(&mut queued, local);
        queued
    }

    /// Empties the session's queues, then the messages kept during compaction, and returns
    /// their texts grouped as `get_all_queued_messages` groups them.
    pub(super) fn clear_all_queues(&self) -> QueueTexts {
        let mut queued = self.runtime_host.session().clear_queue();
        let local = self.compaction_queued_messages.take();
        append_compaction_queue(&mut queued, local);
        queued
    }

    /// Rebuilds the pending rows: the shell rows still shown there, then one dim row per
    /// steering message, then per follow-up message, then the dequeue hint with the keys
    /// bound now; without queued messages only the shell rows remain.
    ///
    /// # Errors
    /// Returns the failure of the published theme's dim colour.
    pub(super) fn update_pending_messages_display(&self) -> Result<(), maestro_theme::ThemeError> {
        self.pending_messages_container.clear();
        for shell in self.pending_shells.borrow().iter() {
            let row = Rc::clone(shell);
            self.pending_messages_container.add_child(row);
        }
        let QueueTexts {
            steering,
            follow_up,
        } = self.get_all_queued_messages();
        if steering.is_empty() && follow_up.is_empty() {
            return Ok(());
        }
        let theme = self.themes.theme().get()?;
        self.pending_messages_container
            .add_child(Rc::new(Spacer::new(1)));
        let hint = format!(
            "↳ {} to edit all queued messages",
            Self::get_app_key_display("app.message.dequeue")
        );
        let rows = steering
            .iter()
            .map(|message| format!("Steering: {message}"))
            .chain(
                follow_up
                    .iter()
                    .map(|message| format!("Follow-up: {message}")),
            )
            .chain(iter::once(hint));
        for row in rows {
            let row = theme.fg(&ThemeColor::Dim, &row)?;
            self.pending_messages_container
                .add_child(Rc::new(TruncatedText::new(row, 1, 0)));
        }
        Ok(())
    }

    /// Empties the queues into the editor, steering before follow-up, each separated by a
    /// blank line and followed by the draft when it is not blank, then refreshes the pending
    /// rows and stops the running turn when asked. Returns how many messages were restored;
    /// with none the editor is left unchanged.
    ///
    /// # Errors
    /// Returns the editor's original failure before refreshing or stopping the turn,
    /// or the theme failure after the turn is stopped.
    pub(super) fn restore_queued_messages_to_editor(
        &self,
        options: RestoreQueuedMessagesOptions<'_>,
    ) -> Result<usize, Box<dyn Error>> {
        let QueueTexts {
            steering,
            follow_up,
        } = self.clear_all_queues();
        let queued: Vec<String> = steering.into_iter().chain(follow_up).collect();
        if !queued.is_empty() {
            let queued_text = queued.join("\n\n");
            let current_text = match options.current_text {
                Some(text) => text.to_owned(),
                None => self
                    .current_editor()
                    .try_get_text()
                    .map_err(|error| -> Box<dyn Error> { error })?,
            };
            let combined: Vec<&str> = [queued_text.as_str(), current_text.as_str()]
                .into_iter()
                .filter(|text| !text.trim_matches(is_whitespace_scalar).is_empty())
                .collect();
            self.current_editor()
                .try_set_text(&combined.join("\n\n"))
                .map_err(|error| -> Box<dyn Error> { error })?;
        }
        let shown = self.update_pending_messages_display();
        if options.abort {
            self.runtime_host.session().agent().abort();
        }
        Ok(shown.map(|()| queued.len())?)
    }

    /// Hands the failure of a key action to the terminal host, which reports it.
    pub(super) fn report(&self, result: Result<(), Box<dyn Error>>) {
        if result.is_err() {
            let runtime = &self.tool_environment.runtime;
            runtime.spawn_local(Box::pin(std::future::ready(result)));
        }
    }
}

/// Appends each kept message's text to the queue of its delivery, in order.
fn append_compaction_queue(queued: &mut QueueTexts, local: Vec<CompactionQueuedMessage>) {
    for CompactionQueuedMessage { text, mode } in local {
        match mode {
            UserMessageDelivery::Steer => queued.steering.push(text),
            UserMessageDelivery::FollowUp => queued.follow_up.push(text),
        }
    }
}
