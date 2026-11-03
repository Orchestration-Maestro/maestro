//! The session commands the editor starts: a new session and a manual compaction.

use super::InteractiveMode;
use maestro_app::{NewSessionCommandOptions, ReadonlySessionManager};
use maestro_theme::ThemeColor;
use serde_json::Value;
use std::{error::Error, rc::Rc};

impl InteractiveMode {
    /// Stops working progress, then starts a new session unless an extension vetoes it,
    /// replays that session and confirms it. A failed replacement or replay ends the process
    /// through the fatal runtime route.
    pub(super) async fn handle_clear_command(self: Rc<Self>) -> Result<(), Box<dyn Error>> {
        self.clear_status_indicator(None);
        if let Err(error) = self.start_new_session().await {
            self.handle_fatal_runtime_error("Failed to create session", error.as_ref());
        }
        Ok(())
    }

    /// Replaces the session unless vetoed, then replays it under one success notice.
    ///
    /// # Errors
    /// Returns the replacement's failure, the replay's failure or the theme's.
    async fn start_new_session(&self) -> Result<(), Box<dyn Error>> {
        let result = self
            .runtime_host
            .new_session(NewSessionCommandOptions::default())
            .await?;
        if result.cancelled {
            return Ok(());
        }
        self.render_current_session_state()
            .map_err(|error| -> Box<dyn Error> { error })?;
        let notice = self.themes.theme().get()?;
        self.add_text(notice.fg(&ThemeColor::Accent, "✓ New session started")?, 1);
        Ok(())
    }

    /// Warns when the session holds fewer than two message entries; otherwise stops working
    /// progress and compacts with `custom_instructions`. The compaction's events show its
    /// outcome, a failure included, and deliver the input queued meanwhile.
    pub(super) async fn handle_compact_command(
        self: Rc<Self>,
        custom_instructions: Option<String>,
    ) -> Result<(), Box<dyn Error>> {
        let session = self.runtime_host.session();
        let messages = session
            .session_manager()
            .borrow()
            .get_entries()
            .into_iter()
            .filter(|entry| entry.get("type").and_then(Value::as_str) == Some("message"))
            .count();
        if messages < 2 {
            self.show_warning("Nothing to compact (no messages yet)")?;
            return Ok(());
        }
        self.clear_status_indicator(None);
        drop(session.compact(custom_instructions.as_deref()).await);
        Ok(())
    }
}
