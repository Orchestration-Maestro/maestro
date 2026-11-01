//! Interactive resource reload with a painted loading box and retained editor focus.
use super::{InteractiveMode, resources::ShowLoadedResourcesOptions, toggles::set_expanded};
use crate::DynamicBorder;
use maestro_app::EventError;
use maestro_theme::{ThemeChangeResult, ThemeColor};
use maestro_tui::{Container, EditorComponent, Spacer, Text};
use std::{error::Error, rc::Rc};

impl InteractiveMode {
    /// Refuses reload during a response or compaction; otherwise resets extension UI,
    /// paints the loading box, reloads the application and refreshes the presentation.
    /// A failure after preparation restores the captured editor without undoing effects.
    ///
    /// # Errors
    /// Returns a preparation failure or the failure of presenting the reload error.
    pub(super) async fn handle_reload_command(self: &Rc<Self>) -> Result<(), EventError> {
        let session = self.runtime_host.session();
        if session.is_streaming() {
            return Ok(
                self.show_warning("Wait for the current response to finish before reloading.")?
            );
        }
        if session.is_compacting() {
            return Ok(self.show_warning("Wait for compaction to finish before reloading.")?);
        }
        self.reset_extension_ui()?;
        let reload_box = self.reload_box()?;
        let previous_editor = self.current_editor();
        self.show_panel(reload_box.clone(), reload_box);
        self.ui.request_render(true);
        let result: Result<(), Box<dyn Error>> = async {
            self.ui.render_pending_frame()?;
            session.reload().await?;
            self.refresh_reloaded_resources()?;
            self.show_panel(self.current_editor(), self.current_editor());
            self.show_loaded_resources(ShowLoadedResourcesOptions {
                show_diagnostics_when_quiet: true,
                ..ShowLoadedResourcesOptions::default()
            })?;
            if let Some(error) = self
                .runtime_host
                .session()
                .model_registry()
                .get_error()
                .filter(|error| !error.is_empty())
            {
                self.show_error(&format!("models.json error: {error}"))?;
            }
            self.show_status("Reloaded keybindings, extensions, skills, prompts, themes")?;
            Ok(())
        }
        .await;
        if let Err(error) = result {
            self.show_panel(previous_editor.clone(), previous_editor);
            self.show_error(&format!("Reload failed: {error}"))?;
        }
        Ok(())
    }

    /// Builds the loading card with live borders, one-row spacers and muted text.
    ///
    /// # Errors
    /// Returns the failure to read the current theme or to color the muted text.
    fn reload_box(&self) -> Result<Rc<Container>, EventError> {
        let container = Rc::new(Container::new());
        container.add_child(Rc::new(DynamicBorder::new(self.themes.theme(), None)));
        container.add_child(Rc::new(Spacer::new(1)));
        let text = self.themes.theme().get()?.fg(
            &ThemeColor::Muted,
            "Reloading keybindings, extensions, skills, prompts, themes...",
        )?;
        container.add_child(Rc::new(Text::new(text, 1, 0, None)));
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(Rc::new(DynamicBorder::new(self.themes.theme(), None)));
        Ok(container)
    }

    /// Refreshes keys, header expansion, themes, settings, completion, shortcuts and chat
    /// in that order, leaving already-applied effects intact if a later operation fails.
    ///
    /// # Errors
    /// Returns the failure of the header expansion, the theme selection or its error
    /// notice, the editor settings, the completion setup or the chat rebuild.
    fn refresh_reloaded_resources(self: &Rc<Self>) -> Result<(), Box<dyn Error>> {
        self.keybindings.reload();
        let custom = self.custom_header.borrow().clone();
        if let Some(custom) = custom {
            if let Some(expand) = &custom.set_expanded {
                expand
                    .call(self.tool_output_expanded.get())
                    .map_err(|error| error as Box<dyn Error>)?;
            }
        } else if let Some(header) = self.built_in_header.borrow().clone() {
            set_expanded(header.as_ref(), self.tool_output_expanded.get())
                .map_err(|error| error as Box<dyn Error>)?;
        }
        let session = self.runtime_host.session();
        self.themes
            .set_registered_themes(session.resource_loader().get_themes().themes.clone());
        let settings = session.settings_manager().borrow();
        self.hide_thinking_block
            .set(settings.get_hide_thinking_block());
        let theme = settings.get_theme();
        drop(settings);
        if let Some(name) = theme.filter(|name| !name.is_empty())
            && let ThemeChangeResult::Failure { error } = self
                .themes
                .set_theme(&name, Some(self.operations.clone()))?
        {
            self.show_error(&format!(
                "Failed to load theme \"{name}\": {error}\nFell back to dark theme."
            ))?;
        }
        self.apply_reload_editor_settings()
            .map_err(|error| error as Box<dyn Error>)?;
        self.setup_autocomplete_provider()
            .map_err(|error| error as Box<dyn Error>)?;
        self.setup_extension_shortcuts(&self.runtime_host.session().extension_runner());
        self.rebuild_chat_from_messages()
            .map_err(|error| -> Box<dyn Error> { error })
    }

    /// Applies editor settings to the default editor and independently to a distinct
    /// current editor, then applies the terminal's cursor and shrink policies.
    ///
    /// # Errors
    /// Returns the custom editor's refusal of the padding or the completion height, or
    /// the terminal's failure to show or hide the hardware cursor.
    fn apply_reload_editor_settings(&self) -> Result<(), EventError> {
        let session = self.runtime_host.session();
        let settings = session.settings_manager().borrow();
        let (padding, maximum) = (
            settings.get_editor_padding_x(),
            settings.get_autocomplete_max_visible(),
        );
        drop(settings);
        self.editor.set_padding_x(padding);
        self.editor.set_autocomplete_max_visible(maximum);
        if self.custom_editor_current() {
            self.current_editor().try_set_padding_x(padding)?;
        }
        if self.custom_editor_current() {
            self.current_editor()
                .try_set_autocomplete_max_visible(maximum)?;
        }
        let settings = session.settings_manager().borrow();
        let (cursor, clear) = (
            settings.get_show_hardware_cursor(),
            settings.get_clear_on_shrink(),
        );
        drop(settings);
        self.ui.set_show_hardware_cursor(cursor)?;
        self.apply_clear_on_shrink(clear);
        Ok(())
    }
}
