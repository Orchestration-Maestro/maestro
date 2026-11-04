//! Strict display corpus queries using the shared settings fixture owner.
use super::settings_selection_support::corpus_cases;
use serde::{Deserialize, de::DeserializeOwned};
/// Decodes the complete display corpus.
pub fn display_cases<I: DeserializeOwned, E: DeserializeOwned>(name: &str) -> Vec<(I, E)> {
    corpus_cases(
        include_str!("../fixtures/settings_display.json"),
        &[
            "display_rows_follow_terminal_capabilities",
            "display_booleans_notify_only_their_own_callback",
            "numeric_settings_cycle_fixed_choices_and_wrap",
            "numeric_constructor_values_keep_supplied_spelling",
            "display_activation_updates_list_before_callback",
            "display_capabilities_are_sampled_once",
            "display_search_uses_existing_keybindings",
            "display_rows_preserve_styles_and_wrapping",
        ],
        134,
        name,
    )
}
/// Supplied terminal image protocol.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Protocol {
    /// Inline image transport, or unsupported.
    pub protocol: Option<String>,
}

/// A controlled terminal with the supplied inline protocol.
pub fn terminal(protocol: Option<&str>) -> maestro_tui::TerminalImage {
    let terminal = super::settings_selection_support::no_images();
    terminal.set_capabilities(capabilities(protocol));
    terminal
}
/// Resolves the recorded protocol to a complete explicit capability value.
pub fn capabilities(protocol: Option<&str>) -> maestro_tui::TerminalCapabilities {
    maestro_tui::TerminalCapabilities {
        images: protocol.map(|value| match value {
            "kitty" => maestro_tui::ImageProtocol::Kitty,
            "iterm2" => maestro_tui::ImageProtocol::Iterm2,
            _ => panic!("unknown protocol"),
        }),
        true_color: true,
        hyperlinks: true,
    }
}

/// Independent boolean constructor input and capability.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Boolean {
    /// Target row identity.
    pub id: String,
    /// Selected constructor field.
    pub field: String,
    /// Selected current value.
    pub current: bool,
    /// Inline transport.
    pub protocol: Option<String>,
}
/// All display boolean fields, set opposite the chosen field.
pub const BOOLEAN_FIELDS: [&str; 7] = [
    "showImages",
    "autoResizeImages",
    "blockImages",
    "enableSkillCommands",
    "showHardwareCursor",
    "clearOnShrink",
    "showTerminalProgress",
];
pub use super::settings_selection_support::select_with_images as select_display;
/// Numeric constructor route with lossless special-number spelling.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Numeric {
    /// Selected row.
    pub id: String,
    /// Supplied field.
    pub field: String,
    /// Supplied binary64 spelling.
    pub current: String,
}
/// Applies the supplied number without normalization.
pub fn set_numeric(config: &mut maestro_chat::SettingsConfig, field: &str, value: &str) {
    let value = value.parse::<f64>().unwrap();
    match field {
        "imageWidthCells" => config.image_width_cells = value,
        "editorPaddingX" => config.editor_padding_x = value,
        "autocompleteMaxVisible" => config.autocomplete_max_visible = value,
        _ => panic!("unknown numeric constructor field"),
    }
}

pub use super::settings_selection_support::{Target, observing};
/// Wraps only the selected callback, preserving its real typed payload.
pub fn observe_callback(
    callbacks: &mut maestro_chat::SettingsCallbacks,
    id: &str,
    target: &Target,
    events: &super::settings_selection_support::Events,
) {
    /// Installs the same observation around either scalar callback type.
    macro_rules! observe {
        ($field:ident) => {{
            callbacks.$field = observing(callbacks.$field.clone(), target.clone(), events.clone());
        }};
    }
    match id {
        "show-images" => observe!(on_show_images_change),
        "image-width-cells" => observe!(on_image_width_cells_change),
        "auto-resize-images" => observe!(on_auto_resize_images_change),
        "block-images" => observe!(on_block_images_change),
        "skill-commands" => observe!(on_enable_skill_commands_change),
        "show-hardware-cursor" => observe!(on_show_hardware_cursor_change),
        "editor-padding" => observe!(on_editor_padding_x_change),
        "autocomplete-max-visible" => observe!(on_autocomplete_max_visible_change),
        "clear-on-shrink" => observe!(on_clear_on_shrink_change),
        "terminal-progress" => observe!(on_show_terminal_progress_change),
        _ => panic!("unknown callback row"),
    }
}

/// Initial and replacement capability observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityChange {
    /// Capability supplied before construction.
    pub initial: Option<String>,
    /// Capability supplied before invalidation.
    pub later: Option<String>,
}
/// Retained panel and newly constructed panel observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityFrames {
    /// Initial panel.
    pub before: super::settings_selection_support::Frame,
    /// Invalidated retained panel.
    pub after: super::settings_selection_support::Frame,
    /// Fresh panel inventory and rendering.
    pub fresh: super::settings_selection_support::Inventory,
}
/// Checks all inventory fields through rendering and real activation.
pub fn assert_panel(
    selector: &maestro_chat::SettingsSelectorComponent,
    events: &super::settings_selection_support::Events,
    expected: &super::settings_selection_support::Inventory,
    images: bool,
) {
    use super::settings_selection_support::{assert_inventory_row, input};
    use maestro_tui::Component;
    assert_eq!(selector.render(80), expected.lines);
    assert_eq!(events.get(), expected.events);
    assert_eq!(expected.items.len(), if images { 24 } else { 22 });
    for (index, item) in expected.items.iter().enumerate() {
        assert_inventory_row(selector, item);
        if index + 1 < expected.items.len() {
            input(&**selector.get_settings_list(), &["\x1b[B"]);
        }
    }
}

/// Full-label search and capability input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Search {
    /// Expected row identity, absent for hidden rows.
    pub id: Option<String>,
    /// Actual search input.
    pub query: String,
    /// Construction-time transport.
    pub protocol: Option<String>,
}
/// Remapped-key observations; hidden rows have no obsolete-key observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchFrames {
    /// After filtering.
    pub before: super::settings_selection_support::Frame,
    /// After the obsolete confirmation key.
    pub old_key: Option<super::settings_selection_support::Frame>,
    /// After confirmation and cancellation.
    pub after: super::settings_selection_support::Frame,
}
/// Selected row and viewport width.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StyledInput {
    /// Target identity.
    pub id: String,
    /// Viewport cells.
    pub width: usize,
}
/// Observations around live palette publication.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StyledFrames {
    /// Constructor palette.
    pub initial: super::settings_selection_support::Frame,
    /// Replacement palette.
    pub changed: super::settings_selection_support::Frame,
}
/// Expected typed notification for a searched row identity.
pub fn callback_name(id: &str) -> &str {
    match id {
        "show-images" => "ShowImages",
        "image-width-cells" => "ImageWidthCells",
        "auto-resize-images" => "AutoResizeImages",
        "block-images" => "BlockImages",
        "skill-commands" => "EnableSkillCommands",
        "show-hardware-cursor" => "ShowHardwareCursor",
        "editor-padding" => "EditorPaddingX",
        "autocomplete-max-visible" => "AutocompleteMaxVisible",
        "clear-on-shrink" => "ClearOnShrink",
        "terminal-progress" => "ShowTerminalProgress",
        _ => panic!("unknown callback identity"),
    }
}
