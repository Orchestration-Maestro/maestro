//! Recorded selector cases and their strict corpus validation.
#![cfg(test)]
use maestro_app::ThinkingLevel;
use maestro_theme::{ColorMode, ColorValue, Theme, ThemeColor, ThemeOptions};
use maestro_tui::{Component, SelectList};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};

pub use crate::keybindings::bindings_guard;
pub use crate::selector::{input, levels};

/// Decode one nonempty locally owned corpus group.
pub fn cases<I: DeserializeOwned, E: DeserializeOwned>(name: &str) -> Vec<(I, E)> {
    let rows = super::selector::load(include_str!("fixtures/simple_selectors.json"));
    let owners = [
        "thinking_levels_keep_descriptions_and_current_selection",
        "thinking_supplied_order_duplicates_and_missing_current",
        "thinking_selection_reports_typed_levels",
        "select_list_layout_uses_bounded_primary_columns",
        "theme_inventory_marks_only_the_current_name",
        "theme_preview_follows_navigation_not_programmatic_changes",
        "theme_window_shows_ten_names_with_scroll_position",
        "image_choices_render_and_report_both_boolean_values",
        "message_initial_identity_defaults_to_newest",
        "message_initial_duplicate_uses_first_and_empty_id_is_absent",
        "message_text_replaces_only_lf_and_trims_defined_whitespace",
        "message_clipping_preserves_cells_graphemes_and_escape_sequences",
        "message_window_centers_and_clamps_ten_rows",
        "message_navigation_wraps_and_empty_confirmation_is_inert",
        "selectors_read_live_bindings_and_reject_wrong_modifiers",
        "message_binding_collision_prefers_navigation",
        "message_callbacks_are_replaceable_clearable_and_reentrant",
        "selector_live_colors_preserve_construction_time_help",
        "selector_children_and_focus_list_keep_distinct_identities",
        "empty_message_auto_cancel_uses_one_original_delayed_callback",
        "theme_discovery_failure_propagates_without_callbacks",
    ];
    super::selector::cases(rows, &owners, name)
}
/// Exact three-role palette used by the recordings.
pub fn colors(shift: u8) -> Rc<Theme> {
    Rc::new(
        Theme::new(
            [
                (ThemeColor::Border, ColorValue::Index(31 + shift)),
                (ThemeColor::Accent, ColorValue::Index(32 + shift)),
                (ThemeColor::Muted, ColorValue::Index(33 + shift)),
            ],
            [],
            ColorMode::Truecolor,
            ThemeOptions::default(),
        )
        .unwrap(),
    )
}
/// Ordered caller notifications, keeping value types intact.
#[derive(Clone, Default)]
pub struct Events(Rc<RefCell<Vec<Vec<Value>>>>);
impl Events {
    /// Current ordered transcript.
    pub fn get(&self) -> Vec<Vec<Value>> {
        self.0.borrow().clone()
    }
    /// Append a named event with its optional value.
    pub fn push(&self, name: &str, value: Option<Value>) {
        let mut event = vec![Value::from(name)];
        event.extend(value);
        self.0.borrow_mut().push(event);
    }
    /// Thinking confirmation.
    pub fn thinking(&self) -> Rc<dyn Fn(ThinkingLevel)> {
        let events = self.clone();
        Rc::new(move |value| events.push("select", Some(serde_json::to_value(value).unwrap())))
    }
    /// Cancellation.
    pub fn cancel(&self) -> Rc<dyn Fn()> {
        let events = self.clone();
        Rc::new(move || events.push("cancel", None))
    }
}
/// Thinking inventory query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThinkingInput {
    /// Preselection.
    pub current: ThinkingLevel,
    /// Supplied order.
    pub levels: Vec<ThinkingLevel>,
}
/// Selected record with absent descriptions distinguished from empty text.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Item {
    /// Stored identity.
    value: String,
    /// Display label.
    label: String,
    /// Optional description.
    description: Option<String>,
}
/// Entire component output and ordered callbacks.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    /// Complete styled rows.
    lines: Vec<String>,
    /// Selected record or absence.
    selected: Option<Item>,
    /// Ordered notifications.
    events: Vec<Vec<Value>>,
}
/// Project observable output without losing rows or notification identity.
pub fn snapshot(
    component: &dyn Component,
    list: &SelectList,
    events: &Events,
    width: usize,
) -> Snapshot {
    Snapshot {
        lines: component.render(width),
        selected: list.get_selected_item().map(|item| Item {
            value: item.value.clone(),
            label: item.label.clone(),
            description: item.description.clone(),
        }),
        events: events.get(),
    }
}
/// Thinking inventory with independently supplied availability.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AvailableInput {
    /// Current level.
    pub current: ThinkingLevel,
    /// Supplied levels.
    pub available: Vec<ThinkingLevel>,
}
/// Current level query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurrentLevel {
    /// Current level.
    pub current: ThinkingLevel,
}
/// Two full snapshots.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeforeAfter {
    /// Initial state.
    pub before: Snapshot,
    /// State after input.
    pub after: Snapshot,
}
/// Ordered notifications alone.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventOutput {
    /// Notifications.
    pub events: Vec<Vec<Value>>,
}
/// Image-display starting value.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageInput {
    /// Starting bool.
    pub current: bool,
}
impl Events {
    /// Callback recording each value under `name`.
    pub fn record<T: Into<Value> + 'static>(&self, name: &'static str) -> Rc<dyn Fn(T)> {
        let events = self.clone();
        Rc::new(move |value: T| events.push(name, Some(value.into())))
    }
}
