//! Cleanup of the interactive presentation's current owners.

use super::InteractiveMode;
use std::io;

impl InteractiveMode {
    /// Clears configured terminal progress, releases the loader, footer and session
    /// subscription, then stops the initialized UI. The shared theme watcher stays live.
    ///
    /// # Errors
    /// Returns the original progress-clearing or UI-stop error. Failed UI stop leaves
    /// the presentation initialized; earlier cleanup is not undone.
    pub fn stop(&self) -> io::Result<()> {
        if self
            .runtime_host
            .session()
            .settings_manager()
            .borrow()
            .get_show_terminal_progress()
        {
            self.ui.terminal().borrow_mut().set_progress(false)?;
        }
        self.release_working_loader();
        self.footer.dispose();
        self.footer_data_provider.dispose();
        if let Some(unsubscribe) = self.unsubscribe.take() {
            unsubscribe();
        }
        if self.is_initialized.get() {
            self.ui.stop()?;
            self.is_initialized.set(false);
        }
        Ok(())
    }
}
