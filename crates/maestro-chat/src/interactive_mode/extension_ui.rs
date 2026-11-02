//! The extension UI host over the retained interactive mode.
use super::InteractiveMode;
use maestro_app::{
    AutocompleteProviderFactory, EventError, ExtensionUIContextPort, ExtensionUIDialogOptions,
    ExtensionUIFuture, NotificationType, PreparedSettingsEdit, SettingsEdit, TerminalInputHandler,
    ThemeSelection, ThemeSetResult, UIUnsubscribe, WorkingIndicatorOptions,
};
use maestro_theme::{LiveTheme, Theme, ThemeChangeResult, ThemeInfo, ThemeWatchOperations};
use std::rc::Rc;

pub(super) mod custom;
pub(super) mod dialogs;
pub(super) mod editor;
pub(super) mod input;
mod replacements;
mod reset;
mod widgets;

impl ExtensionUIContextPort for InteractiveMode {
    /// Shows an info status or a labelled warning or error notice.
    ///
    /// # Errors
    /// Returns the original theme failure of the selected notice owner.
    fn notify(&self, message: String, kind: Option<NotificationType>) -> Result<(), EventError> {
        match kind {
            Some(NotificationType::Error) => self.show_error(&message)?,
            Some(NotificationType::Warning) => self.show_warning(&message)?,
            None | Some(NotificationType::Info) => self.show_status(&message)?,
        }
        Ok(())
    }

    /// Registers a bound input handler without changing its callback identity.
    ///
    /// # Errors
    /// Registration itself cannot fail; callback failures reach the terminal runtime.
    fn on_terminal_input(
        &self,
        handler: TerminalInputHandler,
    ) -> Result<UIUnsubscribe, EventError> {
        Ok(self.extension_input.register(self, handler))
    }

    /// Shows a selector in place of the editor until it is answered, cancelled, timed
    /// out or aborted.
    fn select(
        &self,
        title: String,
        options: Vec<String>,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<Option<String>> {
        self.show_extension_selector(title, options, opts)
    }

    /// Asks `title` and `message` with `Yes` and `No`; only `Yes` is true.
    fn confirm(
        &self,
        title: String,
        message: String,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<bool> {
        self.show_extension_confirm(&title, &message, opts)
    }

    /// Shows a text input in place of the editor until it is submitted, cancelled,
    /// timed out or aborted.
    fn input(
        &self,
        title: String,
        placeholder: Option<String>,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<Option<String>> {
        self.show_extension_input(title, placeholder.as_deref(), opts)
    }

    /// Shows a multiline editor in place of the editor until it is submitted or
    /// cancelled.
    fn editor(&self, title: String, prefill: Option<String>) -> ExtensionUIFuture<Option<String>> {
        self.show_extension_editor(title, prefill)
    }

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

    /// Replaces the footer with the extension's, or restores the built-in one for `None`.
    ///
    /// # Errors
    /// Returns the original disposal or factory failure.
    fn set_footer(&self, factory: Option<maestro_app::FooterFactory>) -> Result<(), EventError> {
        self.set_extension_footer(factory)
    }

    /// Replaces the header with the extension's, or restores the built-in one for `None`.
    ///
    /// # Errors
    /// Returns the original disposal, factory or expansion failure.
    fn set_header(&self, factory: Option<maestro_app::ComponentFactory>) -> Result<(), EventError> {
        self.set_extension_header(factory)
    }

    /// Shows the extension's component in place of the editor or as an overlay until it
    /// closes or fails.
    fn custom_component(
        &self,
        factory: maestro_app::CustomComponentFactory,
        options: Option<maestro_app::CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        self.show_extension_custom(&factory, options)
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
        self.current_editor()
            .handle_input(&format!("\x1b[200~{text}\x1b[201~"));
        Ok(())
    }

    /// Hands `text` to the current editor unchanged; the editor stores it with its own
    /// newline and tab normalization.
    ///
    /// # Errors
    /// Returns the editor's original failure.
    fn set_editor_text(&self, text: String) -> Result<(), EventError> {
        self.current_editor().try_set_text(&text)
    }

    /// Returns the current editor's expanded text, or its raw text when it has none.
    ///
    /// # Errors
    /// Returns the editor's original failure.
    fn get_editor_text(&self) -> Result<String, EventError> {
        self.expanded_editor_text()
    }

    /// Replaces the editor with the extension's, or restores the default for `None`.
    ///
    /// # Errors
    /// Returns the original text, factory or editor failure.
    fn set_editor_component(
        &self,
        factory: Option<maestro_app::EditorFactory>,
    ) -> Result<(), EventError> {
        self.set_custom_editor_component(factory)
    }

    /// Returns the extension editor factory, if one is set.
    ///
    /// # Errors
    /// This implementation does not return errors.
    fn get_editor_component(&self) -> Result<Option<maestro_app::EditorFactory>, EventError> {
        Ok(self.editor_factory())
    }

    /// Retains the bound factory and rebuilds completion before returning.
    ///
    /// # Errors
    /// Returns the original factory failure; the registration remains for later rebuilds.
    fn add_autocomplete_provider(
        &self,
        factory: AutocompleteProviderFactory,
    ) -> Result<(), EventError> {
        self.autocomplete_provider_wrappers
            .borrow_mut()
            .push(factory);
        self.setup_autocomplete_provider()
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
    /// saved name differs. A policy refusal leaves the palette and settings unchanged.
    /// An unloadable name publishes the fallback without saving or requesting a frame.
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
                let edit = match self.prepare_theme_name(&name) {
                    Ok(edit) => edit,
                    Err(error) => {
                        self.show_error(&error)?;
                        return Ok(ThemeSetResult {
                            success: false,
                            error: Some(error),
                        });
                    }
                };
                let watcher: Rc<dyn ThemeWatchOperations> = self.operations.clone();
                if let ThemeChangeResult::Failure { error } =
                    self.themes.set_theme(&name, Some(watcher))?
                {
                    return Ok(ThemeSetResult {
                        success: false,
                        error: Some(error),
                    });
                }
                if let Some(edit) = edit {
                    self.runtime_host
                        .session()
                        .settings_manager()
                        .borrow_mut()
                        .apply_prepared_edit(edit);
                }
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
    /// Admits a changed theme name before publishing its palette.
    fn prepare_theme_name(&self, name: &str) -> Result<Option<PreparedSettingsEdit>, String> {
        let session = self.runtime_host.session();
        let settings = session.settings_manager();
        let settings = settings.borrow();
        if settings.get_theme().as_deref() == Some(name) {
            return Ok(None);
        }
        settings
            .admit_settings_edit(SettingsEdit::new(&["theme"], name.into()))
            .map(Some)
    }
}
