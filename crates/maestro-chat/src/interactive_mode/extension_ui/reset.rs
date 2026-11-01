//! Extension presentation cleanup before session invalidation.
use super::super::InteractiveMode;
use maestro_app::EventError;
use maestro_tui::Component;

impl InteractiveMode {
    /// Cancels the stacked dialogs, abandons the custom views, removes
    /// the extension input listeners, restores the built-in footer and header, clears the
    /// widgets and status texts, restores the default editor with a fresh completion
    /// provider and no shortcuts, rewrites the title, and restores the working and
    /// hidden-thinking defaults.
    ///
    /// # Errors
    /// Returns the first cleanup failure, keeping the effects before it.
    pub(in super::super) fn reset_extension_ui(&self) -> Result<(), EventError> {
        self.cancel_dialogs();
        self.abandon_custom_views()?;
        self.extension_input.clear();
        self.set_extension_footer(None)?;
        self.set_extension_header(None)?;
        self.clear_extension_widgets()?;
        self.footer_data_provider.clear_extension_statuses();
        self.footer.invalidate();
        self.set_custom_editor_component(None)?;
        self.autocomplete_provider_wrappers.borrow_mut().clear();
        self.setup_autocomplete_provider()?;
        self.editor.set_on_extension_shortcut(None);
        self.update_terminal_title()?;
        self.reset_working();
        Ok(())
    }
}
