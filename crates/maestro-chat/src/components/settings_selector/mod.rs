//! Searchable session, display and behavior settings with nested selectors.
mod items;
mod submenus;
use super::dynamic_border::DynamicBorder;
use maestro_app::{
    DoubleEscapeAction, MessageDeliveryMode, ThinkingLevel, Transport, TransportSetting,
    TreeFilterMode, WarningSettings,
};
use maestro_theme::LiveTheme;
use maestro_tui::components::settings_list::SettingsListOptions;
use maestro_tui::{Component, Container, SettingsList, TerminalImage};
use std::rc::Rc;

/// Session policies supplied to the settings panel.
pub struct SessionSettings {
    /// Whether automatic context compaction is enabled.
    pub auto_compact: bool,
    /// Whether startup output is quiet.
    pub quiet_startup: bool,
    /// Whether skills register commands.
    pub enable_skill_commands: bool,
}
/// Resolved terminal display policies supplied to the panel.
pub struct TerminalDisplaySettings {
    /// Whether the hardware cursor is shown.
    pub show_hardware_cursor: bool,
    /// Whether shrinking content clears empty rows.
    pub clear_on_shrink: bool,
    /// Whether terminal tab progress is shown.
    pub show_terminal_progress: bool,
}
/// Presentation and update policies supplied to the settings panel.
pub struct BehaviorSettings {
    /// Whether thinking blocks are hidden.
    pub hide_thinking_block: bool,
    /// Whether update notes are condensed.
    pub collapse_changelog: bool,
    /// Whether anonymous install telemetry is enabled.
    pub enable_install_telemetry: bool,
}
/// A theme-name notification callable.
pub type ThemeCallback = Rc<dyn Fn(&str)>;

/// Supplied current settings and available submenu choices.
pub struct SettingsConfig {
    /// Session boolean policies.
    pub session: SessionSettings,
    /// Behavior boolean policies.
    pub behavior: BehaviorSettings,
    /// Whether inline images are shown.
    pub show_images: bool,
    /// Preferred inline image width in cells.
    pub image_width_cells: f64,
    /// Whether attached images are resized.
    pub auto_resize_images: bool,
    /// Whether provider requests exclude images.
    pub block_images: bool,
    /// Resolved terminal display policies.
    pub terminal: TerminalDisplaySettings,
    /// Supplied horizontal editor padding.
    pub editor_padding_x: f64,
    /// Supplied autocomplete window height.
    pub autocomplete_max_visible: f64,
    /// Steering mode.
    pub steering_mode: MessageDeliveryMode,
    /// Follow up mode.
    pub follow_up_mode: MessageDeliveryMode,
    /// Transport.
    pub transport: TransportSetting,
    /// Thinking level.
    pub thinking_level: ThinkingLevel,
    /// Available thinking levels.
    pub available_thinking_levels: Vec<ThinkingLevel>,
    /// Current theme.
    pub current_theme: String,
    /// Available themes.
    pub available_themes: Vec<String>,
    /// Double escape action.
    pub double_escape_action: DoubleEscapeAction,
    /// Tree filter mode.
    pub tree_filter_mode: TreeFilterMode,
    /// Warnings.
    pub warnings: WarningSettings,
}
/// Immediate notifications for the selected setting.
/// Preference callbacks return false on refusal, retaining the previous displayed value.
pub struct SettingsCallbacks {
    /// On auto compact change.
    pub on_auto_compact_change: Rc<dyn Fn(bool) -> bool>,
    /// Reports a chosen show images value.
    pub on_show_images_change: Rc<dyn Fn(bool) -> bool>,
    /// Reports a chosen image width cells value.
    pub on_image_width_cells_change: Rc<dyn Fn(f64) -> bool>,
    /// Reports a chosen auto resize images value.
    pub on_auto_resize_images_change: Rc<dyn Fn(bool) -> bool>,
    /// Reports a chosen block images value.
    pub on_block_images_change: Rc<dyn Fn(bool) -> bool>,
    /// Reports a chosen enable skill commands value.
    pub on_enable_skill_commands_change: Rc<dyn Fn(bool) -> bool>,
    /// Reports a chosen show hardware cursor value.
    pub on_show_hardware_cursor_change: Rc<dyn Fn(bool) -> bool>,
    /// Reports a chosen editor padding x value.
    pub on_editor_padding_x_change: Rc<dyn Fn(f64) -> bool>,
    /// Reports a chosen autocomplete max visible value.
    pub on_autocomplete_max_visible_change: Rc<dyn Fn(f64) -> bool>,
    /// Reports a chosen clear on shrink value.
    pub on_clear_on_shrink_change: Rc<dyn Fn(bool) -> bool>,
    /// Reports a chosen show terminal progress value.
    pub on_show_terminal_progress_change: Rc<dyn Fn(bool) -> bool>,
    /// On steering mode change.
    pub on_steering_mode_change: Rc<dyn Fn(MessageDeliveryMode) -> bool>,
    /// On follow up mode change.
    pub on_follow_up_mode_change: Rc<dyn Fn(MessageDeliveryMode) -> bool>,
    /// On transport change.
    pub on_transport_change: Rc<dyn Fn(Transport) -> bool>,
    /// On thinking level change.
    pub on_thinking_level_change: Rc<dyn Fn(ThinkingLevel) -> bool>,
    /// On theme change.
    pub on_theme_change: Rc<dyn Fn(&str) -> bool>,
    /// On theme preview.
    pub on_theme_preview: Option<ThemeCallback>,
    /// On hide thinking block change.
    pub on_hide_thinking_block_change: Rc<dyn Fn(bool) -> bool>,
    /// On collapse changelog change.
    pub on_collapse_changelog_change: Rc<dyn Fn(bool) -> bool>,
    /// On enable install telemetry change.
    pub on_enable_install_telemetry_change: Rc<dyn Fn(bool) -> bool>,
    /// On double escape action change.
    pub on_double_escape_action_change: Rc<dyn Fn(DoubleEscapeAction) -> bool>,
    /// On tree filter mode change.
    pub on_tree_filter_mode_change: Rc<dyn Fn(TreeFilterMode) -> bool>,
    /// On quiet startup change.
    pub on_quiet_startup_change: Rc<dyn Fn(bool) -> bool>,
    /// On warnings change.
    pub on_warnings_change: Rc<dyn Fn(WarningSettings) -> bool>,
    /// On cancel.
    pub on_cancel: Rc<dyn Fn()>,
}
/// A settings panel retaining its independently usable input list.
pub struct SettingsSelectorComponent {
    /// Border and list children.
    container: Container,
    /// Retained input target.
    settings_list: Rc<SettingsList>,
}
impl SettingsSelectorComponent {
    /// Builds the searchable settings rows from supplied state.
    #[must_use]
    pub fn new(
        config: SettingsConfig,
        callbacks: SettingsCallbacks,
        theme: LiveTheme,
        terminal_image: &TerminalImage,
    ) -> Self {
        let supports_images = terminal_image.get_capabilities().images.is_some();
        let callbacks = Rc::new(callbacks);
        let rows = items::rows(config, &callbacks, &theme, supports_images);
        let cancel = callbacks.on_cancel.clone();
        let settings_list = Rc::new(SettingsList::new(
            rows,
            10,
            maestro_theme::get_settings_list_theme(theme.clone()),
            (
                Rc::new(move |id, value| items::change(&callbacks, id, value)),
                cancel,
            ),
            SettingsListOptions {
                enable_search: true,
            },
        ));
        let container = Container::new();
        container.add_child(Rc::new(DynamicBorder::new(theme.clone(), None)));
        container.add_child(settings_list.clone());
        container.add_child(Rc::new(DynamicBorder::new(theme, None)));
        Self {
            container,
            settings_list,
        }
    }
    /// Returns the identical retained input list.
    #[must_use]
    pub fn get_settings_list(&self) -> &Rc<SettingsList> {
        &self.settings_list
    }
}
component_children!(SettingsSelectorComponent, Container, container);
impl Component for SettingsSelectorComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.container.render(width)
    }
    fn invalidate(&self) {
        self.container.invalidate();
    }
}
