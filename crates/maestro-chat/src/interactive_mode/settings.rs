//! Change settings live from the interactive mode.
use super::InteractiveMode;
use crate::components::settings_selector::http_idle_timeout_label;
use crate::{
    AssistantMessageComponent, BehaviorSettings, SessionSettings, SettingsCallbacks,
    SettingsConfig, SettingsSelectorComponent, TerminalDisplaySettings, ToolExecutionComponent,
};
use maestro_app::{
    DoubleEscapeAction, PreparedSettingsEdit, SettingsEdit, SettingsManager, ThinkingLevel,
    Transport, TreeFilterMode, WarningSettings, delivery_mode, queue_mode,
};
use maestro_theme::ThemeChangeResult;
use maestro_tui::{Component, EditorComponent};
use serde_json::{Value, json};
use std::{any::Any, error::Error, rc::Rc};

impl InteractiveMode {
    /// Shows the settings panel in place of the editor over the current session.
    ///
    /// # Errors
    /// Returns the failure of reading the theme inventory, which leaves the editor and
    /// focus untouched.
    pub(super) fn show_settings_selector(self: &Rc<Self>) -> Result<(), Box<dyn Error>> {
        self.show_selector(|done| {
            let selector = Rc::new(SettingsSelectorComponent::new(
                self.settings_config()?,
                self.settings_callbacks(done),
                self.themes.theme(),
                &self.tool_environment.terminal_image,
            ));
            let focus = selector.get_settings_list().clone();
            Ok((selector, focus))
        })
    }

    /// Reads the live session and preferences once for the panel.
    pub(super) fn settings_config(&self) -> Result<SettingsConfig, Box<dyn Error>> {
        let session = self.runtime_host.session();
        let settings = session.settings_manager().borrow();
        Ok(SettingsConfig {
            session: SessionSettings {
                auto_compact: session.auto_compaction_enabled(),
                quiet_startup: settings.get_quiet_startup(),
                enable_skill_commands: settings.get_enable_skill_commands(),
            },
            behavior: BehaviorSettings {
                hide_thinking_block: self.hide_thinking_block.get(),
                collapse_changelog: settings.get_collapse_changelog(),
                enable_install_telemetry: settings.get_enable_install_telemetry(),
                http_idle_timeout_ms: settings.get_http_idle_timeout_ms(),
            },
            terminal: TerminalDisplaySettings {
                show_hardware_cursor: settings.get_show_hardware_cursor(),
                clear_on_shrink: settings.get_clear_on_shrink(),
                show_terminal_progress: settings.get_show_terminal_progress(),
            },
            show_images: settings.get_show_images(),
            image_width_cells: settings.get_image_width_cells(),
            auto_resize_images: settings.get_image_auto_resize(),
            block_images: settings.get_block_images(),
            editor_padding_x: settings.get_editor_padding_x(),
            autocomplete_max_visible: settings.get_autocomplete_max_visible(),
            steering_mode: delivery_mode(session.steering_mode()),
            follow_up_mode: delivery_mode(session.follow_up_mode()),
            transport: settings.get_transport(),
            thinking_level: session.thinking_level(),
            available_thinking_levels: session.get_available_thinking_levels(),
            current_theme: settings
                .get_theme()
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "dark".to_owned()),
            available_themes: self.themes.get_available_themes()?,
            double_escape_action: settings.get_double_escape_action(),
            tree_filter_mode: settings.get_tree_filter_mode(),
            warnings: settings.get_warnings(),
        })
    }

    /// Binds every panel notification without keeping the mode alive.
    pub(super) fn settings_callbacks(self: &Rc<Self>, done: Rc<dyn Fn()>) -> SettingsCallbacks {
        let mode = Rc::downgrade(self);
        SettingsCallbacks {
            on_auto_compact_change: self.fallible_setting_callback(Self::settings_auto_compact),
            on_show_images_change: self.fallible_setting_callback(Self::settings_show_images),
            on_image_width_cells_change: self.fallible_setting_callback(Self::settings_image_width),
            on_auto_resize_images_change: self.save_setting(&["images", "autoResize"], Value::Bool),
            on_block_images_change: self.save_setting(&["images", "blockImages"], Value::Bool),
            on_enable_skill_commands_change: self
                .fallible_setting_callback(Self::settings_skill_commands),
            on_steering_mode_change: self.fallible_setting_callback(|mode, value| {
                mode.runtime_host
                    .session()
                    .set_steering_mode(queue_mode(&value))
                    .map_err(session_setting_error)
            }),
            on_follow_up_mode_change: self.fallible_setting_callback(|mode, value| {
                mode.runtime_host
                    .session()
                    .set_follow_up_mode(queue_mode(&value))
                    .map_err(session_setting_error)
            }),
            on_transport_change: self.fallible_setting_callback(Self::settings_transport),
            on_thinking_level_change: self.fallible_setting_callback(Self::settings_thinking),
            on_theme_change: self.settings_theme_callback(false),
            on_theme_preview: Some(self.settings_theme_preview()),
            on_hide_thinking_block_change: self
                .fallible_setting_callback(Self::settings_hide_thinking),
            on_collapse_changelog_change: self.save_setting(&["collapseChangelog"], Value::Bool),
            on_enable_install_telemetry_change: self
                .save_setting(&["enableInstallTelemetry"], Value::Bool),
            on_http_idle_timeout_ms_change: self
                .fallible_setting_callback(Self::settings_http_idle_timeout),
            on_double_escape_action_change: self
                .save_setting(&["doubleEscapeAction"], |value: DoubleEscapeAction| {
                    value.as_str().into()
                }),
            on_tree_filter_mode_change: self
                .save_setting(&["treeFilterMode"], |value: TreeFilterMode| {
                    value.as_str().into()
                }),
            on_show_hardware_cursor_change: self
                .fallible_setting_callback(Self::settings_hardware_cursor),
            on_editor_padding_x_change: self.fallible_setting_callback(Self::settings_padding),
            on_autocomplete_max_visible_change: self
                .fallible_setting_callback(Self::settings_completion_height),
            on_quiet_startup_change: self.save_setting(&["quietStartup"], Value::Bool),
            on_clear_on_shrink_change: self
                .fallible_setting_callback(Self::settings_clear_on_shrink),
            on_show_terminal_progress_change: self
                .save_setting(&["terminal", "showTerminalProgress"], Value::Bool),
            on_warnings_change: self
                .save_setting(&["warnings"], |value: WarningSettings| value.0.into()),
            on_cancel: Rc::new(move || {
                if let Some(mode) = mode.upgrade() {
                    done();
                    mode.ui.request_render(false);
                }
            }),
        }
    }

    /// Routes a notification through the existing error owner, retaining admission outcome.
    fn fallible_setting_callback<T: 'static>(
        self: &Rc<Self>,
        apply: impl Fn(&Self, T) -> Result<(), Box<dyn Error>> + 'static,
    ) -> Rc<dyn Fn(T) -> bool> {
        let mode = Rc::downgrade(self);
        Rc::new(move |value| {
            let Some(mode) = mode.upgrade() else {
                return false;
            };
            let Err(error) = apply(&mode, value) else {
                return true;
            };
            if let Some(refusal) = error.downcast_ref::<SettingsRefusal>() {
                mode.report(mode.show_error(&refusal.0).map_err(Into::into));
                return false;
            }
            mode.report(Err(error));
            true
        })
    }

    /// Saves an ordinary preference using its literal JSON representation.
    fn save_setting<T: 'static>(
        self: &Rc<Self>,
        path: &'static [&'static str],
        value: impl Fn(T) -> serde_json::Value + 'static,
    ) -> Rc<dyn Fn(T) -> bool> {
        self.fallible_setting_callback(move |mode, selected| {
            let edit = mode.admit_setting(SettingsEdit::new(path, value(selected)))?;
            mode.runtime_host
                .session()
                .settings_manager()
                .borrow_mut()
                .apply_prepared_edit(edit);
            Ok(())
        })
    }

    /// Admits the exact value before any presentation changes.
    fn admit_setting(&self, edit: SettingsEdit) -> Result<PreparedSettingsEdit, Box<dyn Error>> {
        self.runtime_host
            .session()
            .settings_manager()
            .borrow()
            .admit_settings_edit(edit)
            .map_err(|message| Box::new(SettingsRefusal(message)) as Box<dyn Error>)
    }

    /// Keeps preview and commit on the same theme selection owner.
    fn settings_theme_callback(self: &Rc<Self>, preview: bool) -> Rc<dyn Fn(&str) -> bool> {
        let callback = self.fallible_setting_callback(move |mode, name: String| {
            mode.settings_theme(&name, preview)
        });
        Rc::new(move |name| callback(name.to_owned()))
    }

    /// Previews a theme without admitting or saving a preference.
    fn settings_theme_preview(self: &Rc<Self>) -> Rc<dyn Fn(&str)> {
        let preview = self.settings_theme_callback(true);
        Rc::new(move |name| {
            preview(name);
        })
    }

    /// Publishes a theme under the supplied watch effects; committing also saves the
    /// requested name.
    fn settings_theme(&self, name: &str, preview: bool) -> Result<(), Box<dyn Error>> {
        let edit = if preview {
            None
        } else {
            Some(self.admit_setting(SettingsEdit::new(&["theme"], json!(name)))?)
        };
        let result = self.themes.set_theme(name, Some(self.operations.clone()))?;
        if preview {
            if matches!(result, ThemeChangeResult::Success) {
                self.ui.invalidate();
                self.ui.request_render(false);
            }
            return Ok(());
        }
        if let Some(edit) = edit {
            self.runtime_host
                .session()
                .settings_manager()
                .borrow_mut()
                .apply_prepared_edit(edit);
        }
        self.ui.invalidate();
        if let ThemeChangeResult::Failure { error } = result {
            self.show_error(&format!(
                "Failed to load theme \"{name}\": {error}\nFell back to dark theme."
            ))?;
        }
        Ok(())
    }

    /// Saves the session compaction policy before refreshing the footer.
    fn settings_auto_compact(&self, enabled: bool) -> Result<(), Box<dyn Error>> {
        self.runtime_host
            .session()
            .set_auto_compaction_enabled(enabled)
            .map_err(session_setting_error)?;
        self.footer.set_auto_compact_enabled(enabled);
        Ok(())
    }

    /// Saves inline image visibility and applies it to retained tool rows.
    fn settings_show_images(&self, enabled: bool) -> Result<(), Box<dyn Error>> {
        let edit = self.admit_setting(SettingsEdit::new(
            &["terminal", "showImages"],
            json!(enabled),
        ))?;
        self.runtime_host
            .session()
            .settings_manager()
            .borrow_mut()
            .apply_prepared_edit(edit);
        self.settings_tools(|tool| tool.set_show_images(enabled))
    }

    /// Saves inline image width and applies it to retained tool rows.
    fn settings_image_width(&self, width: f64) -> Result<(), Box<dyn Error>> {
        let edit = self.admit_setting(SettingsManager::prepare_image_width_cells(width))?;
        self.runtime_host
            .session()
            .settings_manager()
            .borrow_mut()
            .apply_prepared_edit(edit);
        self.settings_tools(|tool| tool.set_image_width_cells(width))
    }

    /// Applies one image option to each retained tool in the captured child walk.
    fn settings_tools(
        &self,
        update: impl Fn(&ToolExecutionComponent) -> Result<(), maestro_app::DiagnosticErrorInfo>,
    ) -> Result<(), Box<dyn Error>> {
        for child in self.chat_container.walk() {
            if let Some(tool) = (&*child as &dyn Any).downcast_ref::<ToolExecutionComponent>() {
                update(tool)?;
            }
        }
        Ok(())
    }

    /// Saves skill command admission before rebuilding completion.
    ///
    /// # Errors
    /// Returns the original wrapper or current-editor failure after saving the setting.
    fn settings_skill_commands(&self, enabled: bool) -> Result<(), Box<dyn Error>> {
        let edit =
            self.admit_setting(SettingsEdit::new(&["enableSkillCommands"], json!(enabled)))?;
        self.runtime_host
            .session()
            .settings_manager()
            .borrow_mut()
            .apply_prepared_edit(edit);
        self.setup_autocomplete_provider()
            .map_err(|error| -> Box<dyn Error> { error })
    }

    /// Saves the idle limit for HTTP waits and names it in the transcript.
    fn settings_http_idle_timeout(&self, milliseconds: f64) -> Result<(), Box<dyn Error>> {
        let value = SettingsManager::prepare_http_idle_timeout_ms(milliseconds)
            .ok_or_else(|| SettingsRefusal("invalid HTTP idle timeout".into()))?;
        let edit = self.admit_setting(value)?;
        self.runtime_host
            .session()
            .settings_manager()
            .borrow_mut()
            .apply_prepared_edit(edit);
        self.show_status(&format!(
            "HTTP idle timeout: {}",
            http_idle_timeout_label(milliseconds)
        ))?;
        Ok(())
    }

    /// Saves the transport before changing the next request's live preference.
    fn settings_transport(&self, transport: Transport) -> Result<(), Box<dyn Error>> {
        self.runtime_host
            .session()
            .set_transport(transport)
            .map_err(session_setting_error)?;
        Ok(())
    }

    /// Selects thinking before refreshing the footer and the editor decoration.
    pub(super) fn settings_thinking(&self, level: ThinkingLevel) -> Result<(), Box<dyn Error>> {
        self.runtime_host
            .session()
            .set_thinking_level(level)
            .map_err(session_setting_error)?;
        self.footer.invalidate();
        self.update_editor_border_color()?;
        Ok(())
    }

    /// Saves thinking visibility before updating and rebuilding the transcript.
    fn settings_hide_thinking(&self, hidden: bool) -> Result<(), Box<dyn Error>> {
        let edit = self.admit_setting(SettingsEdit::new(&["hideThinkingBlock"], json!(hidden)))?;
        self.hide_thinking_block.set(hidden);
        self.runtime_host
            .session()
            .settings_manager()
            .borrow_mut()
            .apply_prepared_edit(edit);
        for child in self.chat_container.walk() {
            if let Some(row) = (&*child as &dyn Any).downcast_ref::<AssistantMessageComponent>() {
                row.set_hide_thinking_block(hidden);
            }
        }
        self.rebuild_chat_from_messages()
            .map_err(|error| -> Box<dyn Error> { error })
    }

    /// Saves the cursor preference before applying its immediate terminal effect.
    fn settings_hardware_cursor(&self, enabled: bool) -> Result<(), Box<dyn Error>> {
        let edit =
            self.admit_setting(SettingsEdit::new(&["showHardwareCursor"], json!(enabled)))?;
        self.runtime_host
            .session()
            .settings_manager()
            .borrow_mut()
            .apply_prepared_edit(edit);
        self.ui.set_show_hardware_cursor(enabled)?;
        Ok(())
    }

    /// Saves padding and updates the default editor, then a distinct current editor.
    ///
    /// # Errors
    /// Returns the current editor's original failure.
    fn settings_padding(&self, padding: f64) -> Result<(), Box<dyn Error>> {
        let edit = self.admit_setting(SettingsManager::prepare_editor_padding_x(padding))?;
        self.runtime_host
            .session()
            .settings_manager()
            .borrow_mut()
            .apply_prepared_edit(edit);
        self.editor.set_padding_x(padding);
        if self.custom_editor_current() {
            self.current_editor()
                .try_set_padding_x(padding)
                .map_err(|error| -> Box<dyn Error> { error })?;
        }
        Ok(())
    }

    /// Saves the completion height and updates the default editor, then a distinct current
    /// editor.
    ///
    /// # Errors
    /// Returns the current editor's original failure.
    fn settings_completion_height(&self, height: f64) -> Result<(), Box<dyn Error>> {
        let edit = self.admit_setting(SettingsManager::prepare_autocomplete_max_visible(height))?;
        self.runtime_host
            .session()
            .settings_manager()
            .borrow_mut()
            .apply_prepared_edit(edit);
        self.editor.set_autocomplete_max_visible(height);
        if self.custom_editor_current() {
            self.current_editor()
                .try_set_autocomplete_max_visible(height)
                .map_err(|error| -> Box<dyn Error> { error })?;
        }
        Ok(())
    }

    /// Saves the shrink policy and applies it to the terminal.
    pub(super) fn settings_clear_on_shrink(&self, enabled: bool) -> Result<(), Box<dyn Error>> {
        let edit = self.admit_setting(SettingsEdit::new(
            &["terminal", "clearOnShrink"],
            json!(enabled),
        ))?;
        self.runtime_host
            .session()
            .settings_manager()
            .borrow_mut()
            .apply_prepared_edit(edit);
        self.apply_clear_on_shrink(enabled);
        Ok(())
    }
}

/// A refused preference, distinct from an error after its effects began.
#[derive(Debug)]
struct SettingsRefusal(String);
impl std::fmt::Display for SettingsRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl Error for SettingsRefusal {}

/// Keeps session admission failures distinguishable from later effect failures.
fn session_setting_error(error: maestro_app::DiagnosticErrorInfo) -> Box<dyn Error> {
    if error.name.as_deref() == Some("ConfigurationRefusal") {
        Box::new(SettingsRefusal(error.message))
    } else {
        Box::new(error)
    }
}
