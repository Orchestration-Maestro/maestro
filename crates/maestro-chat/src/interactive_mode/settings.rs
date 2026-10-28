//! Change settings live from the interactive mode.
use super::InteractiveMode;
use crate::{
    AssistantMessageComponent, BehaviorSettings, SessionSettings, SettingsCallbacks,
    SettingsConfig, SettingsSelectorComponent, TerminalDisplaySettings, ToolExecutionComponent,
};
use maestro_app::{
    SettingsManager, ThinkingLevel, Transport, TransportSetting, delivery_mode, queue_mode,
};
use maestro_theme::ThemeChangeResult;
use maestro_tui::{Component, EditorComponent};
use std::{any::Any, error::Error, rc::Rc, sync::PoisonError};

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
            on_auto_compact_change: self.setting_callback(Self::settings_auto_compact),
            on_show_images_change: self.fallible_setting_callback(Self::settings_show_images),
            on_image_width_cells_change: self.fallible_setting_callback(Self::settings_image_width),
            on_auto_resize_images_change: self.save_setting(SettingsManager::set_image_auto_resize),
            on_block_images_change: self.save_setting(SettingsManager::set_block_images),
            on_enable_skill_commands_change: self
                .fallible_setting_callback(Self::settings_skill_commands),
            on_steering_mode_change: self.fallible_setting_callback(|mode, value| {
                mode.runtime_host
                    .session()
                    .set_steering_mode(queue_mode(&value))?;
                Ok(())
            }),
            on_follow_up_mode_change: self.fallible_setting_callback(|mode, value| {
                mode.runtime_host
                    .session()
                    .set_follow_up_mode(queue_mode(&value))?;
                Ok(())
            }),
            on_transport_change: self.setting_callback(Self::settings_transport),
            on_thinking_level_change: self.fallible_setting_callback(Self::settings_thinking),
            on_theme_change: self.settings_theme_callback(false),
            on_theme_preview: Some(self.settings_theme_callback(true)),
            on_hide_thinking_block_change: self
                .fallible_setting_callback(Self::settings_hide_thinking),
            on_collapse_changelog_change: self
                .save_setting(SettingsManager::set_collapse_changelog),
            on_enable_install_telemetry_change: self
                .save_setting(SettingsManager::set_enable_install_telemetry),
            on_double_escape_action_change: self
                .save_setting(SettingsManager::set_double_escape_action),
            on_tree_filter_mode_change: self.save_setting(SettingsManager::set_tree_filter_mode),
            on_show_hardware_cursor_change: self
                .fallible_setting_callback(Self::settings_hardware_cursor),
            on_editor_padding_x_change: self.fallible_setting_callback(Self::settings_padding),
            on_autocomplete_max_visible_change: self
                .fallible_setting_callback(Self::settings_completion_height),
            on_quiet_startup_change: self.save_setting(SettingsManager::set_quiet_startup),
            on_clear_on_shrink_change: self.setting_callback(Self::settings_clear_on_shrink),
            on_show_terminal_progress_change: self
                .save_setting(SettingsManager::set_show_terminal_progress),
            on_warnings_change: self.save_setting(SettingsManager::set_warnings),
            on_cancel: Rc::new(move || {
                if let Some(mode) = mode.upgrade() {
                    done();
                    mode.ui.request_render(false);
                }
            }),
        }
    }

    /// Runs a synchronous notification without retaining the mode.
    fn setting_callback<T: 'static>(
        self: &Rc<Self>,
        apply: impl Fn(&Self, T) + 'static,
    ) -> Rc<dyn Fn(T)> {
        let mode = Rc::downgrade(self);
        Rc::new(move |value| {
            if let Some(mode) = mode.upgrade() {
                apply(&mode, value);
            }
        })
    }

    /// Routes a fallible notification through the existing error owner.
    fn fallible_setting_callback<T: 'static>(
        self: &Rc<Self>,
        apply: impl Fn(&Self, T) -> Result<(), Box<dyn Error>> + 'static,
    ) -> Rc<dyn Fn(T)> {
        self.setting_callback(move |mode, value| mode.report(apply(mode, value)))
    }

    /// Saves a preference through its existing settings owner before any later effect.
    fn save_setting<T: 'static>(
        self: &Rc<Self>,
        setter: fn(&mut SettingsManager, T),
    ) -> Rc<dyn Fn(T)> {
        self.setting_callback(move |mode, value| {
            setter(
                &mut mode.runtime_host.session().settings_manager().borrow_mut(),
                value,
            );
        })
    }

    /// Keeps preview and commit on the same theme selection owner.
    fn settings_theme_callback(self: &Rc<Self>, preview: bool) -> Rc<dyn Fn(&str)> {
        let mode = Rc::downgrade(self);
        Rc::new(move |name| {
            if let Some(mode) = mode.upgrade() {
                mode.report(mode.settings_theme(name, preview));
            }
        })
    }

    /// Publishes a theme under the supplied watch effects; committing also saves the
    /// requested name.
    fn settings_theme(&self, name: &str, preview: bool) -> Result<(), Box<dyn Error>> {
        let result = self.themes.set_theme(name, Some(self.operations.clone()))?;
        if preview {
            if matches!(result, ThemeChangeResult::Success) {
                self.ui.invalidate();
                self.ui.request_render(false);
            }
            return Ok(());
        }
        self.runtime_host
            .session()
            .settings_manager()
            .borrow_mut()
            .set_theme(name.to_owned());
        self.ui.invalidate();
        if let ThemeChangeResult::Failure { error } = result {
            self.show_error(&format!(
                "Failed to load theme \"{name}\": {error}\nFell back to dark theme."
            ))?;
        }
        Ok(())
    }

    /// Saves the session compaction policy before refreshing the footer.
    fn settings_auto_compact(&self, enabled: bool) {
        self.runtime_host
            .session()
            .set_auto_compaction_enabled(enabled);
        self.footer.set_auto_compact_enabled(enabled);
    }

    /// Saves inline image visibility and applies it to retained tool rows.
    fn settings_show_images(&self, enabled: bool) -> Result<(), Box<dyn Error>> {
        self.runtime_host
            .session()
            .settings_manager()
            .borrow_mut()
            .set_show_images(enabled);
        self.settings_tools(|tool| tool.set_show_images(enabled))
    }

    /// Saves inline image width and applies it to retained tool rows.
    fn settings_image_width(&self, width: f64) -> Result<(), Box<dyn Error>> {
        self.runtime_host
            .session()
            .settings_manager()
            .borrow_mut()
            .set_image_width_cells(width);
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
        self.runtime_host
            .session()
            .settings_manager()
            .borrow_mut()
            .set_enable_skill_commands(enabled);
        self.setup_autocomplete_provider()
            .map_err(|error| -> Box<dyn Error> { error })
    }

    /// Saves the transport before changing the next request's live preference.
    fn settings_transport(&self, transport: Transport) {
        let saved = match &transport {
            Transport::Auto => TransportSetting::Auto,
            Transport::Sse => TransportSetting::Sse,
            Transport::Websocket => TransportSetting::Websocket,
            Transport::WebsocketCached => TransportSetting::WebsocketCached,
        };
        let session = self.runtime_host.session();
        session.settings_manager().borrow_mut().set_transport(saved);
        *session
            .agent()
            .transport
            .write()
            .unwrap_or_else(PoisonError::into_inner) = transport;
    }

    /// Selects thinking before refreshing the footer and the editor decoration.
    pub(super) fn settings_thinking(&self, level: ThinkingLevel) -> Result<(), Box<dyn Error>> {
        self.runtime_host.session().set_thinking_level(level)?;
        self.footer.invalidate();
        self.update_editor_border_color()?;
        Ok(())
    }

    /// Saves thinking visibility before updating and rebuilding the transcript.
    fn settings_hide_thinking(&self, hidden: bool) -> Result<(), Box<dyn Error>> {
        self.hide_thinking_block.set(hidden);
        self.runtime_host
            .session()
            .settings_manager()
            .borrow_mut()
            .set_hide_thinking_block(hidden);
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
        self.runtime_host
            .session()
            .settings_manager()
            .borrow_mut()
            .set_show_hardware_cursor(enabled);
        self.ui.set_show_hardware_cursor(enabled)?;
        Ok(())
    }

    /// Saves padding and updates the default editor, then a distinct current editor.
    ///
    /// # Errors
    /// Returns the current editor's original failure.
    fn settings_padding(&self, padding: f64) -> Result<(), Box<dyn Error>> {
        self.runtime_host
            .session()
            .settings_manager()
            .borrow_mut()
            .set_editor_padding_x(padding);
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
        self.runtime_host
            .session()
            .settings_manager()
            .borrow_mut()
            .set_autocomplete_max_visible(height);
        self.editor.set_autocomplete_max_visible(height);
        if self.custom_editor_current() {
            self.current_editor()
                .try_set_autocomplete_max_visible(height)
                .map_err(|error| -> Box<dyn Error> { error })?;
        }
        Ok(())
    }

    /// Saves the shrink policy and applies it to the terminal.
    fn settings_clear_on_shrink(&self, enabled: bool) {
        self.runtime_host
            .session()
            .settings_manager()
            .borrow_mut()
            .set_clear_on_shrink(enabled);
        self.ui.set_clear_on_shrink(enabled);
    }
}
