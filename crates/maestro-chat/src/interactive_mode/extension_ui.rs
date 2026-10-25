//! The extension UI host over the retained interactive mode.
use super::InteractiveMode;
use maestro_app::{
    EventError, ExtensionUIContextPort, ThemeSelection, ThemeSetResult, WorkingIndicatorOptions,
};
use maestro_theme::{LiveTheme, Theme, ThemeChangeResult, ThemeInfo, ThemeWatchOperations};
use maestro_tui::tui::InputHandler;
use std::rc::Rc;

mod widgets;

impl ExtensionUIContextPort for InteractiveMode {
    /// Updates the keyed footer status and requests a frame.
    ///
    /// # Errors
    /// This implementation does not return errors.
    fn set_status(&self, key: String, text: Option<String>) -> Result<(), EventError> {
        self.set_extension_status(&key, text.as_deref());
        Ok(())
    }

    /// Disposes the previous keyed widget and installs the supplied replacement.
    ///
    /// # Errors
    /// Returns the original disposal, factory or text-theme failure.
    fn set_widget(
        &self,
        key: String,
        content: Option<maestro_app::WidgetContent>,
        options: Option<maestro_app::ExtensionWidgetOptions>,
    ) -> Result<(), EventError> {
        self.set_extension_widget(key, content, options)
    }

    /// Forwards the supplied text to the terminal's title setter unchanged.
    ///
    /// # Errors
    /// Returns the terminal's failure to write the title.
    fn set_title(&self, title: String) -> Result<(), EventError> {
        self.ui.terminal().borrow_mut().set_title(&title)?;
        Ok(())
    }

    /// Delivers `text` to the editor as a bracketed paste.
    ///
    /// # Errors
    /// Returns the host's failure; the editor itself cannot fail here.
    fn paste_to_editor(&self, text: String) -> Result<(), EventError> {
        self.editor
            .handle_input(&format!("\x1b[200~{text}\x1b[201~"));
        Ok(())
    }

    /// Replaces the editor text with `text`, unchanged.
    ///
    /// # Errors
    /// Returns the host's failure; the editor itself cannot fail here.
    fn set_editor_text(&self, text: String) -> Result<(), EventError> {
        self.editor.set_text(&text);
        Ok(())
    }

    /// Returns the editor's expanded text, or its plain text when it has none.
    ///
    /// # Errors
    /// Returns the host's failure; reading the editor cannot fail here.
    fn get_editor_text(&self) -> Result<String, EventError> {
        Ok(self.editor.get_expanded_text())
    }

    /// Returns the live view of the published theme.
    fn theme(&self) -> LiveTheme {
        self.themes.theme()
    }

    /// Lists every available theme name with its first owner's path.
    ///
    /// # Errors
    /// Returns the shipped-data, custom-directory or ordering failure.
    fn get_all_themes(&self) -> Result<Vec<ThemeInfo>, EventError> {
        Ok(self.themes.get_available_themes_with_paths()?)
    }

    /// Loads `name` for reading without publishing it.
    ///
    /// # Errors
    /// Returns the host's failure; a name with no theme is absence.
    fn get_theme(&self, name: String) -> Result<Option<Rc<Theme>>, EventError> {
        Ok(self.themes.get_theme_by_name(&name))
    }

    /// Publishes a theme by name or instance, saving a changed name before the frame.
    ///
    /// A named selection starts the supplied theme watch and saves `name` only when the
    /// saved name differs; a rejected name publishes the fallback and is reported without
    /// saving it or requesting a frame.
    ///
    /// # Errors
    /// Returns the failure of the published-theme callback or of the fallback load.
    fn set_theme(&self, theme: ThemeSelection) -> Result<ThemeSetResult, EventError> {
        match theme {
            ThemeSelection::Theme(theme) => {
                self.themes.set_theme_instance(theme.get()?)?;
                self.ui.request_render(false);
                Ok(ThemeSetResult {
                    success: true,
                    error: None,
                })
            }
            ThemeSelection::Name(name) => {
                let watcher: Rc<dyn ThemeWatchOperations> = self.operations.clone();
                if let ThemeChangeResult::Failure { error } =
                    self.themes.set_theme(&name, Some(watcher))?
                {
                    return Ok(ThemeSetResult {
                        success: false,
                        error: Some(error),
                    });
                }
                self.save_theme_name(&name);
                self.ui.request_render(false);
                Ok(ThemeSetResult {
                    success: true,
                    error: None,
                })
            }
        }
    }

    /// Returns whether tool output is expanded.
    ///
    /// # Errors
    /// Returns the host's failure; reading the flag cannot fail here.
    fn get_tools_expanded(&self) -> Result<bool, EventError> {
        Ok(self.tool_output_expanded.get())
    }

    /// Applies `expanded` to every expandable row and requests a frame.
    ///
    /// # Errors
    /// Returns the failure of a tool row's native formatting or of a text rendering.
    fn set_tools_expanded(&self, expanded: bool) -> Result<(), EventError> {
        InteractiveMode::set_tools_expanded(self, expanded)
    }

    /// Sets the working message, `None` restoring the default.
    ///
    /// # Errors
    /// Returns the host's failure; setting the message cannot fail here.
    fn set_working_message(&self, message: Option<String>) -> Result<(), EventError> {
        InteractiveMode::set_working_message(self, message);
        Ok(())
    }

    /// Shows or hides the working row.
    ///
    /// # Errors
    /// Returns the host's failure; showing the row cannot fail here.
    fn set_working_visible(&self, visible: bool) -> Result<(), EventError> {
        InteractiveMode::set_working_visible(self, visible);
        Ok(())
    }

    /// Sets the working indicator, `None` restoring the toolkit's own.
    ///
    /// # Errors
    /// Returns the host's failure; setting the indicator cannot fail here.
    fn set_working_indicator(
        &self,
        options: Option<WorkingIndicatorOptions>,
    ) -> Result<(), EventError> {
        InteractiveMode::set_working_indicator(self, options);
        Ok(())
    }

    /// Sets the hidden-thinking label, `None` restoring the default.
    ///
    /// # Errors
    /// Returns the host's failure; setting the label cannot fail here.
    fn set_hidden_thinking_label(&self, label: Option<String>) -> Result<(), EventError> {
        InteractiveMode::set_hidden_thinking_label(self, label);
        Ok(())
    }
}

impl InteractiveMode {
    /// Saves `name` as the configured theme unless it is already saved.
    fn save_theme_name(&self, name: &str) {
        let session = self.runtime_host.session();
        let settings = session.settings_manager();
        let saved = settings.borrow().get_theme();
        if saved.as_deref() != Some(name) {
            settings.borrow_mut().set_theme(name.to_owned());
        }
    }
}
