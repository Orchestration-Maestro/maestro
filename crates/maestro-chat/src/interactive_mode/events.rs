//! The single session-event subscriber and the metadata it refreshes.

use super::InteractiveMode;
use super::progress::{CompactionOutcome, RetryEnd, RetryStart};
use maestro_app::ReadonlySessionManager;
use maestro_app::configuration::config::APP_TITLE;
use maestro_app::{AgentEvent, AgentSessionEvent};
use maestro_tui::Component;
use std::{error::Error, rc::Rc, sync::PoisonError};

impl InteractiveMode {
    /// Registers the one listener that delivers session events synchronously and keeps no
    /// reference to the mode; its removal operation stays here for the session lifecycle.
    pub(super) fn subscribe_to_agent(self: &Rc<Self>) {
        let mode = Rc::downgrade(self);
        let removal =
            self.runtime_host
                .session()
                .subscribe(Rc::new(move |event: &AgentSessionEvent| {
                    let Some(mode) = mode.upgrade() else {
                        return;
                    };
                    if let Err(error) = mode.handle_event(event) {
                        mode.report(Err(error));
                    }
                }));
        self.unsubscribe.replace(Some(Box::new(removal)));
    }

    /// Handles one session event, initializing the presentation first, then refreshing the
    /// metadata the event names from the session's live state.
    ///
    /// # Errors
    /// Returns the failure of the initialization, of the terminal title or of the published
    /// theme.
    pub(super) fn handle_event(
        self: &Rc<Self>,
        event: &AgentSessionEvent,
    ) -> Result<(), Box<dyn Error>> {
        if !self.is_initialized.get() {
            self.init()?;
        }
        self.footer.invalidate();
        match event {
            AgentSessionEvent::SessionInfoChanged { .. } => {
                self.update_terminal_title()?;
                self.ui.request_render(false);
            }
            AgentSessionEvent::ThinkingLevelChanged { .. } => self.update_editor_border_color()?,
            AgentSessionEvent::CompactionStart { reason } => self.start_compaction(*reason)?,
            // The captured input and the end's retry choice arrive with their own owner.
            AgentSessionEvent::CompactionEnd {
                reason,
                result,
                aborted,
                error_message,
                ..
            } => self.end_compaction(&CompactionOutcome {
                reason: *reason,
                result: result.as_ref(),
                aborted: *aborted,
                error_message: error_message.as_deref(),
            })?,
            AgentSessionEvent::AutoRetryStart {
                attempt,
                max_attempts,
                delay_ms,
                ..
            } => self.start_retry(&RetryStart {
                attempt: *attempt,
                max_attempts: *max_attempts,
                delay_ms: *delay_ms,
            }),
            AgentSessionEvent::AutoRetryEnd {
                success,
                attempt,
                final_error,
            } => self.end_retry(&RetryEnd {
                success: *success,
                attempt: *attempt,
                final_error: final_error.as_deref(),
            })?,
            AgentSessionEvent::Agent(event) => {
                let (started, ended) = {
                    let event = event.read().unwrap_or_else(PoisonError::into_inner);
                    (
                        matches!(&*event, AgentEvent::AgentStart),
                        matches!(&*event, AgentEvent::AgentEnd { .. }),
                    )
                };
                if started {
                    self.start_agent_progress()?;
                } else if ended {
                    self.end_agent_progress()?;
                }
            }
            AgentSessionEvent::QueueUpdate { .. } => {}
        }
        Ok(())
    }

    /// Sets the window title to the session's name, when it has one, and the lexical basename
    /// of the session's working directory.
    ///
    /// # Errors
    /// Returns the invalid stored name or the terminal's title-writing failure.
    pub(super) fn update_terminal_title(&self) -> Result<(), Box<dyn Error>> {
        let title = {
            let session = self.runtime_host.session();
            let manager = session.session_manager().borrow();
            let basename = maestro_path::basename(manager.get_cwd(), None);
            match manager.get_session_name()? {
                Some(name) => format!("{APP_TITLE} - {name} - {basename}"),
                None => format!("{APP_TITLE} - {basename}"),
            }
        };
        self.ui.terminal().borrow_mut().set_title(&title)?;
        Ok(())
    }
}
