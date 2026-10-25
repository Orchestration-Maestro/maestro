//! The session commands the editor starts: a new session and a manual compaction.

use super::InteractiveMode;
use maestro_app::NewSessionCommandOptions;
use maestro_theme::ThemeColor;
use std::{error::Error, rc::Rc};

impl InteractiveMode {
    /// Stops working progress, then starts a new session unless an extension vetoes it,
    /// replays that session and confirms it. A failed replacement or replay ends the process
    /// through the fatal runtime route.
    pub(super) async fn handle_clear_command(self: Rc<Self>) -> Result<(), Box<dyn Error>> {
        self.stop_working_loader();
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
        self.render_current_session_state()?;
        let notice = self.themes.theme().get()?;
        self.add_text(notice.fg(&ThemeColor::Accent, "✓ New session started")?, 1);
        Ok(())
    }
}
