//! Searchable session and behavior settings with nested selectors.
mod items;
mod submenus;
use super::dynamic_border::DynamicBorder;
use maestro_app::{
    DoubleEscapeAction, MessageDeliveryMode, ThinkingLevel, Transport, TransportSetting,
    TreeFilterMode, WarningSettings,
};
use maestro_theme::LiveTheme;
use maestro_tui::components::settings_list::SettingsListOptions;
use maestro_tui::{Component, Container, SettingsList};
use std::rc::Rc;

/// Session policies supplied to the settings panel.
pub struct SessionSettings {
    /// Whether automatic context compaction is enabled.
    pub auto_compact: bool,
    /// Whether startup output is quiet.
    pub quiet_startup: bool,
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
pub struct SettingsCallbacks {
    /// On auto compact change.
    pub on_auto_compact_change: Rc<dyn Fn(bool)>,
    /// On steering mode change.
    pub on_steering_mode_change: Rc<dyn Fn(MessageDeliveryMode)>,
    /// On follow up mode change.
    pub on_follow_up_mode_change: Rc<dyn Fn(MessageDeliveryMode)>,
    /// On transport change.
    pub on_transport_change: Rc<dyn Fn(Transport)>,
    /// On thinking level change.
    pub on_thinking_level_change: Rc<dyn Fn(ThinkingLevel)>,
    /// On theme change.
    pub on_theme_change: Rc<dyn Fn(&str)>,
    /// On theme preview.
    pub on_theme_preview: Option<ThemeCallback>,
    /// On hide thinking block change.
    pub on_hide_thinking_block_change: Rc<dyn Fn(bool)>,
    /// On collapse changelog change.
    pub on_collapse_changelog_change: Rc<dyn Fn(bool)>,
    /// On enable install telemetry change.
    pub on_enable_install_telemetry_change: Rc<dyn Fn(bool)>,
    /// On double escape action change.
    pub on_double_escape_action_change: Rc<dyn Fn(DoubleEscapeAction)>,
    /// On tree filter mode change.
    pub on_tree_filter_mode_change: Rc<dyn Fn(TreeFilterMode)>,
    /// On quiet startup change.
    pub on_quiet_startup_change: Rc<dyn Fn(bool)>,
    /// On warnings change.
    pub on_warnings_change: Rc<dyn Fn(WarningSettings)>,
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
    pub fn new(config: SettingsConfig, callbacks: SettingsCallbacks, theme: LiveTheme) -> Self {
        let callbacks = Rc::new(callbacks);
        let rows = items::rows(config, &callbacks, &theme);
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
