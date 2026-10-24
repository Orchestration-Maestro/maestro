//! Queued steering and follow-up input: the pending rows above the editor, restoring the
//! queued text to the editor and the queue key actions.
use super::InteractiveMode;
use maestro_app::agent_session::QueueTexts;
use maestro_app::is_whitespace_scalar;
use maestro_app::messages::UserMessageDelivery;
use maestro_theme::ThemeColor;
use maestro_tui::{EditorComponent, Spacer, TruncatedText};
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
    /// interrupt key restores the queued text and stops a streaming response, and the
    /// dequeue key restores the queued text.
    pub(super) fn setup_queue_key_handlers(self: &Rc<Self>) {
        let mode = Rc::downgrade(self);
        self.editor.set_on_escape(Some(Rc::new(move || {
            if let Some(mode) = mode.upgrade()
                && mode.runtime_host.session().is_streaming()
            {
                let options = RestoreQueuedMessagesOptions {
                    abort: true,
                    current_text: None,
                };
                mode.report(mode.restore_queued_messages_to_editor(options).map(drop));
            }
        })));
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

    /// Restores every queued message to the editor and says how many were restored.
    ///
    /// # Errors
    /// Returns the failure of the published theme's dim colour.
    pub(super) fn handle_dequeue(&self) -> Result<(), Box<dyn Error>> {
        let restored =
            self.restore_queued_messages_to_editor(RestoreQueuedMessagesOptions::default())?;
        self.show_status(&match restored {
            0 => "No queued messages to restore".to_owned(),
            1 => "Restored 1 queued message to editor".to_owned(),
            count => format!("Restored {count} queued messages to editor"),
        })
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

    /// Rebuilds the pending rows: one dim row per steering message, then per follow-up
    /// message, then the dequeue hint with the keys bound now; nothing when no message is
    /// queued.
    ///
    /// # Errors
    /// Returns the failure of the published theme's dim colour.
    pub(super) fn update_pending_messages_display(&self) -> Result<(), Box<dyn Error>> {
        self.pending_messages_container.clear();
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
    /// Returns the failure of the published theme's dim colour, after the turn is stopped.
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
            let current_text = options
                .current_text
                .map_or_else(|| self.editor.get_text(), str::to_owned);
            let combined: Vec<&str> = [queued_text.as_str(), current_text.as_str()]
                .into_iter()
                .filter(|text| !text.trim_matches(is_whitespace_scalar).is_empty())
                .collect();
            self.editor.set_text(&combined.join("\n\n"));
        }
        let shown = self.update_pending_messages_display();
        if options.abort {
            self.runtime_host.session().agent().abort();
        }
        shown.map(|()| queued.len())
    }

    /// Hands the failure of a key action to the terminal host, which reports it.
    fn report(&self, result: Result<(), Box<dyn Error>>) {
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
