//! Strict corpus queries and controlled selector construction.
use maestro_app::{
    DoubleEscapeAction, MessageDeliveryMode, ThinkingLevel, Transport, TransportSetting,
    TreeFilterMode, WarningSettings,
};
use maestro_chat::{
    BehaviorSettings, SessionSettings, SettingsCallbacks, SettingsConfig, TerminalDisplaySettings,
};
use maestro_theme::{ColorMode, ColorValue, Theme, ThemeColor, ThemeOptions};
use maestro_tui::Component;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};

pub use super::selector::{Frame, Frames, input, levels};
/// Decode one nonempty locally owned corpus group.
pub fn cases<I: DeserializeOwned, E: DeserializeOwned>(name: &str) -> Vec<(I, E)> {
    let owners = [
        "settings_rows_keep_text_order_and_initial_values",
        "boolean_changes_report_only_the_selected_setting",
        "delivery_modes_remain_independent_on_constructor_and_change",
        "transport_cycles_all_four_typed_choices",
        "escape_action_and_tree_filter_cover_every_choice",
        "settings_search_dispatches_visible_rows_and_cancel",
        "settings_selected_descriptions_and_scroll_are_exact",
        "settings_layout_reuses_terminal_width_rules",
        "thinking_submenu_keeps_descriptions_and_typed_confirmation",
        "thinking_submenu_handles_empty_singleton_duplicates_and_missing_current",
        "thinking_cancel_retains_value_and_reopens_same_selection",
        "submenus_cap_visible_options_at_ten",
        "theme_navigation_preview_cancel_and_confirm_follow_callback_order",
        "theme_cancel_restores_latest_committed_value",
        "warning_edits_keep_unknown_members_snapshots_and_reopened_state",
        "settings_live_colors_keep_captured_cursor_and_submenu_chrome",
        "settings_and_submenus_use_live_keybindings",
        "settings_list_handle_survives_chrome_removal",
        "warning_callback_reenters_after_state_is_committed",
    ];
    corpus_cases(
        include_str!("../fixtures/settings_selection.json"),
        &owners,
        106,
        name,
    )
}
/// Decodes a strict, complete fixture with its owning test inventory.
pub fn corpus_cases<I: DeserializeOwned, E: DeserializeOwned>(
    fixture: &str,
    owners: &[&str],
    count: usize,
    name: &str,
) -> Vec<(I, E)> {
    let rows = super::selector::load(fixture);
    assert_eq!(rows.len(), count);
    super::selector::cases(rows, owners, name)
}
/// Row metadata, observed through its rendering and activation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    /// Identity.
    pub id: String,
    /// Label.
    pub label: String,
    /// Description.
    pub description: String,
    /// Initial value.
    pub current_value: String,
    /// Cyclic options.
    pub values: Option<Vec<String>>,
    /// Opens a child.
    pub submenu: bool,
}
/// Constructor inventory.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    /// Every row.
    pub items: Vec<Item>,
    /// Styled rows.
    pub lines: Vec<String>,
    /// Ordered events.
    pub events: Vec<Vec<Value>>,
}
/// Observes complete output.
pub fn frame(component: &dyn Component, events: &Events, width: usize) -> Frame {
    Frame {
        lines: component.render(width),
        events: events.get(),
    }
}
/// Exact source palette.
pub fn colors(shift: u8) -> Rc<Theme> {
    Rc::new(
        Theme::new(
            [
                (ThemeColor::Border, ColorValue::Index(31 + shift)),
                (ThemeColor::Accent, ColorValue::Index(32 + shift)),
                (ThemeColor::Muted, ColorValue::Index(33 + shift)),
                (ThemeColor::Dim, ColorValue::Index(34 + shift)),
            ],
            [],
            ColorMode::Truecolor,
            ThemeOptions::default(),
        )
        .unwrap(),
    )
}
/// Supplied initial settings.
pub fn config() -> SettingsConfig {
    SettingsConfig {
        session: SessionSettings {
            auto_compact: true,
            quiet_startup: false,
            enable_skill_commands: true,
        },
        behavior: BehaviorSettings {
            hide_thinking_block: false,
            collapse_changelog: true,
            enable_install_telemetry: false,
            http_idle_timeout_ms: 300_000.0,
        },
        show_images: false,
        image_width_cells: 80.0,
        auto_resize_images: true,
        block_images: false,
        terminal: TerminalDisplaySettings {
            show_hardware_cursor: false,
            clear_on_shrink: false,
            show_terminal_progress: false,
        },
        editor_padding_x: 0.0,
        autocomplete_max_visible: 5.0,
        steering_mode: MessageDeliveryMode::OneAtATime,
        follow_up_mode: MessageDeliveryMode::All,
        transport: TransportSetting::Auto,
        thinking_level: ThinkingLevel::Medium,
        available_thinking_levels: levels(),
        current_theme: "dark".into(),
        available_themes: vec!["dark".into(), "light".into()],
        double_escape_action: DoubleEscapeAction::Tree,
        tree_filter_mode: TreeFilterMode::Default,
        warnings: WarningSettings::default(),
    }
}
/// Ordered callback records.
#[derive(Clone, Default)]
pub struct Events(pub Rc<RefCell<Vec<Vec<Value>>>>);
impl Events {
    /// Captures all events.
    pub fn get(&self) -> Vec<Vec<Value>> {
        self.0.borrow().clone()
    }
    /// Appends a typed value.
    fn push(&self, name: &str, value: Value) {
        self.0.borrow_mut().push(vec![name.into(), value]);
    }
    /// Boolean callback.
    fn boolean(&self, name: &'static str) -> Rc<dyn Fn(bool)> {
        let events = self.clone();
        Rc::new(move |value| events.push(name, value.into()))
    }
    /// Numeric callback for finite chosen values.
    fn numeric(&self, name: &'static str) -> Rc<dyn Fn(f64)> {
        let events = self.clone();
        Rc::new(move |value| {
            let value = serde_json::from_str(ryu_js::Buffer::new().format(value)).unwrap();
            events.push(name, value);
        })
    }
    /// Text callback.
    fn text(&self, name: &'static str) -> Rc<dyn Fn(&str)> {
        let events = self.clone();
        Rc::new(move |value| events.push(name, value.into()))
    }
    /// Every constructor notification.
    pub fn callbacks(&self, preview: bool) -> SettingsCallbacks {
        let steering = self.clone();
        let follow = self.clone();
        let transport = self.clone();
        let thinking = self.clone();
        let action = self.clone();
        let filter = self.clone();
        let warnings = self.clone();
        let cancel = self.clone();
        SettingsCallbacks {
            on_auto_compact_change: self.boolean("AutoCompact"),
            on_show_images_change: self.boolean("ShowImages"),
            on_image_width_cells_change: self.numeric("ImageWidthCells"),
            on_auto_resize_images_change: self.boolean("AutoResizeImages"),
            on_block_images_change: self.boolean("BlockImages"),
            on_enable_skill_commands_change: self.boolean("EnableSkillCommands"),
            on_show_hardware_cursor_change: self.boolean("ShowHardwareCursor"),
            on_editor_padding_x_change: self.numeric("EditorPaddingX"),
            on_autocomplete_max_visible_change: self.numeric("AutocompleteMaxVisible"),
            on_clear_on_shrink_change: self.boolean("ClearOnShrink"),
            on_show_terminal_progress_change: self.boolean("ShowTerminalProgress"),

            on_steering_mode_change: Rc::new(move |value: MessageDeliveryMode| {
                steering.push("SteeringMode", value.as_str().into());
            }),
            on_follow_up_mode_change: Rc::new(move |value: MessageDeliveryMode| {
                follow.push("FollowUpMode", value.as_str().into());
            }),
            on_transport_change: Rc::new(move |value: Transport| {
                transport.push("Transport", serde_json::to_value(value).unwrap());
            }),
            on_thinking_level_change: Rc::new(move |value: ThinkingLevel| {
                thinking.push("ThinkingLevel", serde_json::to_value(value).unwrap());
            }),
            on_theme_change: self.text("Theme"),
            on_theme_preview: preview.then(|| self.text("preview")),
            on_hide_thinking_block_change: self.boolean("HideThinkingBlock"),
            on_collapse_changelog_change: self.boolean("CollapseChangelog"),
            on_enable_install_telemetry_change: self.boolean("EnableInstallTelemetry"),
            on_http_idle_timeout_ms_change: self.numeric("HttpIdleTimeoutMs"),
            on_double_escape_action_change: Rc::new(move |value: DoubleEscapeAction| {
                action.push("DoubleEscapeAction", value.as_str().into());
            }),
            on_tree_filter_mode_change: Rc::new(move |value: TreeFilterMode| {
                filter.push("TreeFilterMode", value.as_str().into());
            }),
            on_quiet_startup_change: self.boolean("QuietStartup"),
            on_warnings_change: Rc::new(move |value: WarningSettings| {
                let keys: Vec<_> = value.0.keys().cloned().collect();
                warnings.0.borrow_mut().push(vec![
                    "Warnings".into(),
                    serde_json::to_value(value).unwrap(),
                    serde_json::to_value(keys).unwrap(),
                ]);
            }),
            on_cancel: Rc::new(move || cancel.0.borrow_mut().push(vec!["cancel".into()])),
        }
    }
}
pub use super::keybindings::bindings_guard;
/// Moves through the original ordered list using live keys.
pub fn select(selector: &maestro_chat::SettingsSelectorComponent, id: &str) {
    select_with_images(selector, id, false);
}
/// Moves through the capability-specific ordered list using live keys.
pub fn select_with_images(
    selector: &maestro_chat::SettingsSelectorComponent,
    id: &str,
    images: bool,
) {
    let ids = [
        "autocompact",
        "show-images",
        "image-width-cells",
        "auto-resize-images",
        "block-images",
        "skill-commands",
        "show-hardware-cursor",
        "editor-padding",
        "autocomplete-max-visible",
        "clear-on-shrink",
        "terminal-progress",
        "steering-mode",
        "follow-up-mode",
        "transport",
        "http-idle-timeout",
        "hide-thinking",
        "collapse-changelog",
        "quiet-startup",
        "install-telemetry",
        "double-escape-action",
        "tree-filter-mode",
        "warnings",
        "thinking",
        "theme",
    ];
    let index = ids
        .iter()
        .filter(|candidate| images || !matches!(**candidate, "show-images" | "image-width-cells"))
        .position(|candidate| *candidate == id)
        .unwrap();
    for _ in 0..index {
        input(&**selector.get_settings_list(), &["\x1b[B"]);
    }
}
/// Initial, first-change and restored frames.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreeFrames {
    /// Before activation.
    pub before: Frame,
    /// First activation.
    pub one: Frame,
    /// Final activation.
    pub after: Frame,
}
/// Both ends of an interaction.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeforeAfter {
    /// Before input.
    pub before: Frame,
    /// After input.
    pub after: Frame,
}
/// Boolean constructor route.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BooleanInput {
    /// Row identity.
    pub id: String,
    /// Constructor field.
    pub field: String,
    /// Supplied value.
    pub current: bool,
}
/// Applies one supplied boolean without changing its sibling policies.
pub fn set_boolean(config: &mut SettingsConfig, field: &str, value: bool) {
    match field {
        "showImages" => config.show_images = value,
        "autoResizeImages" => config.auto_resize_images = value,
        "blockImages" => config.block_images = value,
        "enableSkillCommands" => config.session.enable_skill_commands = value,
        "showHardwareCursor" => config.terminal.show_hardware_cursor = value,
        "clearOnShrink" => config.terminal.clear_on_shrink = value,
        "showTerminalProgress" => config.terminal.show_terminal_progress = value,
        "autoCompact" => config.session.auto_compact = value,
        "hideThinkingBlock" => config.behavior.hide_thinking_block = value,
        "collapseChangelog" => config.behavior.collapse_changelog = value,
        "quietStartup" => config.session.quiet_startup = value,
        "enableInstallTelemetry" => config.behavior.enable_install_telemetry = value,
        _ => panic!("unknown boolean constructor field"),
    }
}
/// Independently supplied delivery modes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryInput {
    /// Selected field.
    pub field: String,
    /// Selected row.
    pub id: String,
    /// Independent sibling.
    pub sibling: String,
    /// Initial spelling.
    pub current: String,
}
/// Retains an open delivery spelling at construction.
pub fn delivery(value: &str) -> MessageDeliveryMode {
    match value {
        "all" => MessageDeliveryMode::All,
        "one-at-a-time" => MessageDeliveryMode::OneAtATime,
        other => MessageDeliveryMode::Unknown(other.into()),
    }
}
/// Sets only one delivery field.
pub fn set_delivery(config: &mut SettingsConfig, field: &str, value: &str) {
    match field {
        "steeringMode" => config.steering_mode = delivery(value),
        "followUpMode" => config.follow_up_mode = delivery(value),
        _ => panic!("unknown delivery constructor field"),
    }
}
/// Supplied current string.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Current {
    /// Current spelling.
    pub current: String,
}
/// Converts retained transport spellings for construction.
pub fn transport(value: &str) -> TransportSetting {
    match value {
        "sse" => TransportSetting::Sse,
        "websocket" => TransportSetting::Websocket,
        "websocket-cached" => TransportSetting::WebsocketCached,
        "auto" => TransportSetting::Auto,
        other => TransportSetting::Unknown(other.into()),
    }
}
/// Supplied action or filter.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionInput {
    /// Row identity.
    pub id: String,
    /// Constructor field.
    pub field: String,
    /// Current spelling.
    pub current: String,
}
/// Constructs existing typed action/filter records.
pub fn set_action(config: &mut SettingsConfig, field: &str, value: &str) {
    match field {
        "doubleEscapeAction" => {
            config.double_escape_action = match value {
                "tree" => DoubleEscapeAction::Tree,
                "fork" => DoubleEscapeAction::Fork,
                "none" => DoubleEscapeAction::None,
                other => DoubleEscapeAction::Unknown(other.into()),
            }
        }
        "treeFilterMode" => {
            config.tree_filter_mode = match value {
                "default" => TreeFilterMode::Default,
                "no-tools" => TreeFilterMode::NoTools,
                "user-only" => TreeFilterMode::UserOnly,
                "labeled-only" => TreeFilterMode::LabeledOnly,
                "all" => TreeFilterMode::All,
                _ => panic!("unknown filter"),
            }
        }
        _ => panic!("unknown action constructor field"),
    }
}
/// Search input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    /// Query bytes.
    pub query: String,
}
/// Row navigation query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Id {
    /// Target row.
    pub id: String,
}
/// Main or submenu viewport query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    /// Optional child surface.
    pub surface: Option<String>,
    /// Viewport cells.
    pub width: usize,
}
/// Reasoning levels supplied in any order.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThinkingInput {
    /// Supplied sequence.
    pub available: Vec<ThinkingLevel>,
    /// Current level.
    pub current: ThinkingLevel,
}
/// Thinking window input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Window {
    /// Option count.
    pub n: usize,
    /// Actual supplied levels.
    pub available: Vec<ThinkingLevel>,
}
/// Theme constructor inputs.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeInput {
    /// Names including duplicates.
    pub available: Vec<String>,
    /// Current name.
    pub current: String,
    /// Preview callback presence.
    pub preview: bool,
}
/// Warning constructor route.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WarningsInput {
    /// Raw owned warning record.
    pub warnings: WarningSettings,
}
/// Theme publication snapshots.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Colors {
    /// Main at construction.
    pub initial: Frame,
    /// Newly opened child.
    pub open: Frame,
    /// Child after palette replacement.
    pub changed: Frame,
    /// Main after cancellation.
    pub main: Frame,
}
/// Original handle and replaced chrome observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Children {
    /// Repeated getter identity.
    pub identical: bool,
    /// Empty chrome.
    pub cleared: Vec<String>,
    /// Retained list rows.
    pub retained: Vec<String>,
    /// Restored component.
    pub restored: Frame,
}
/// Removes terminal controls using the toolkit's owning scanner.
pub fn plain(text: &str) -> String {
    let mut output = String::new();
    let mut pos = 0;
    while pos < text.len() {
        if let Some(code) = maestro_tui::extract_ansi_code(text, pos) {
            pos += code.length;
        } else {
            let ch = text[pos..].chars().next().unwrap();
            output.push(ch);
            pos += ch.len_utf8();
        }
    }
    output
}

/// Checks one recorded row via rendering and activation, without a production snapshot API.
pub fn assert_inventory_row(selector: &maestro_chat::SettingsSelectorComponent, item: &Item) {
    let list = selector.get_settings_list();
    let rows: Vec<_> = selector
        .render(240)
        .iter()
        .map(|line| plain(line))
        .collect();
    let selected = rows.iter().find(|line| line.starts_with("→ ")).unwrap();
    assert!(selected.starts_with(&format!("→ {}", item.label)));
    assert!(selected.ends_with(&item.current_value));
    assert!(rows.contains(&format!(
        "  {}",
        crate::platform_keys::on_this_platform(&item.description)
    )));
    let before = list.render(240);
    input(&**list, &["\r"]);
    if item.submenu {
        assert!(item.values.is_none());
        assert_ne!(list.render(240), before);
        input(&**list, &["\x1b"]);
    } else {
        let choices = item.values.as_ref().unwrap();
        assert!(!choices.is_empty());
        for choice in choices
            .iter()
            .cycle()
            .skip(
                choices
                    .iter()
                    .position(|value| value == &item.current_value)
                    .unwrap()
                    + 1,
            )
            .take(choices.len())
        {
            let rows: Vec<_> = list.render(240).iter().map(|line| plain(line)).collect();
            assert!(
                rows.iter()
                    .find(|line| line.starts_with("→ "))
                    .unwrap()
                    .ends_with(choice)
            );
            input(&**list, &["\r"]);
        }
        // The final activation started the next cycle; restore the original value.
        list.update_value(&item.id, item.current_value.clone());
        let rows: Vec<_> = list.render(240).iter().map(|line| plain(line)).collect();
        assert!(
            rows.iter()
                .find(|line| line.starts_with("→ "))
                .unwrap()
                .ends_with(&item.current_value)
        );
    }
}

/// A controlled terminal without inline image support.
pub fn no_images() -> maestro_tui::TerminalImage {
    let terminal = maestro_tui::TerminalImage::new(|_| None, || 1);
    terminal.set_capabilities(maestro_tui::TerminalCapabilities::default());
    terminal
}

/// Callback reentry target kept non-owning to avoid retaining the list.
pub type Target = std::rc::Rc<std::cell::RefCell<std::rc::Weak<maestro_tui::SettingsList>>>;
/// Records the live list after the original typed notification.
pub fn observing<T: 'static>(
    original: std::rc::Rc<dyn Fn(T)>,
    target: Target,
    events: super::settings_selection_support::Events,
) -> std::rc::Rc<dyn Fn(T)> {
    use maestro_tui::Component;
    std::rc::Rc::new(move |value| {
        original(value);
        let list = target.borrow().upgrade().unwrap();
        let lines = list.render(80);
        events.0.borrow_mut().push(vec![
            "during-callback".into(),
            serde_json::to_value(lines).unwrap(),
        ]);
    })
}
