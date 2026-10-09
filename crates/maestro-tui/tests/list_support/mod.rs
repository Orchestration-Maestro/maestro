use maestro_tui::tui::InputHandler;
use maestro_tui::{Component, SelectItem, SelectList, SelectListLayoutOptions, SelectListTheme};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Mutex};

pub static KEYS: Mutex<()> = Mutex::new(());
pub type Observations = Rc<RefCell<Vec<Value>>>;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub case: String,
    pub input: Input,
    pub expected: Value,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub items: Option<Vec<Item>>,
    pub width: Option<usize>,
    pub ops: Option<Vec<Operation>>,
    #[serde(rename = "maxVisible")]
    pub max_visible: Option<usize>,
    pub layout: Option<Layout>,
    pub custom: Option<String>,
    pub trace: Option<bool>,
    pub search: Option<bool>,
    pub cursor: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub value: Option<String>,
    pub id: Option<String>,
    pub label: String,
    pub description: Option<String>,
    #[serde(rename = "currentValue")]
    pub current_value: Option<String>,
    pub values: Option<Vec<String>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub key: Option<String>,
    pub filter: Option<String>,
    pub index: Option<isize>,
    pub width: Option<usize>,
    pub invalidate: Option<bool>,
    pub update: Option<(String, String)>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    #[serde(rename = "minPrimaryColumnWidth")]
    pub min: Option<isize>,
    #[serde(rename = "maxPrimaryColumnWidth")]
    pub max: Option<isize>,
}
/// Reads or checks one saved observable result.
///
/// # Panics
/// Panics on invalid fixture data or a mismatched observation.
#[must_use]
pub fn cases(name: &str) -> Vec<Case> {
    let mut corpus: BTreeMap<String, Vec<Case>> =
        serde_json::from_str(include_str!("../fixtures/lists.json")).unwrap();
    corpus.remove(name).unwrap()
}
#[must_use]
pub fn commands() -> Vec<SelectItem> {
    [
        ("Alpha", "One", Some("first")),
        ("alpine", "Two", Some("second")),
        ("beta", "Three", Some("third")),
        ("gamma", "Four", None),
        ("delta", "Five", None),
        ("zeta", "Six", None),
    ]
    .into_iter()
    .map(|(value, label, description)| SelectItem {
        value: value.into(),
        label: label.into(),
        description: description.map(str::to_owned),
    })
    .collect()
}
pub fn select_theme(trace: bool, calls: &Observations) -> SelectListTheme {
    let style = |name: &'static str| -> Rc<dyn Fn(&str) -> String> {
        let calls = Rc::clone(calls);
        Rc::new(move |text| {
            if trace {
                calls.borrow_mut().push(json!([name, text]));
                format!("\x1b[31m{text}\x1b[0m")
            } else {
                text.to_owned()
            }
        })
    };
    SelectListTheme {
        selected_prefix: style("selectedPrefix"),
        selected_text: style("selectedText"),
        description: style("description"),
        scroll_info: style("scrollInfo"),
        no_match: style("noMatch"),
    }
}
impl Item {
    /// Decodes a command record and rejects settings-only members.
    fn command(self) -> SelectItem {
        assert!(self.id.is_none() && self.current_value.is_none() && self.values.is_none());
        SelectItem {
            value: self.value.unwrap(),
            label: self.label,
            description: self.description,
        }
    }
    /// Decodes a setting record and rejects command-only members.
    fn setting(self) -> SettingItem {
        assert!(self.value.is_none());
        SettingItem {
            id: self.id.unwrap(),
            label: self.label,
            description: self.description,
            current_value: self.current_value.unwrap(),
            values: self.values,
            submenu: None,
        }
    }
}
/// Supplies the oracle's custom primary callback.
fn layout(
    bounds: Option<Layout>,
    custom: Option<String>,
    calls: &Observations,
    identities: &Rc<Vec<String>>,
) -> SelectListLayoutOptions {
    let mut layout = SelectListLayoutOptions::default();
    if let Some(bounds) = bounds {
        layout.min_primary_column_width = bounds.min;
        layout.max_primary_column_width = bounds.max;
    }
    if let Some(custom) = custom {
        let calls = Rc::clone(calls);
        let identities = Rc::clone(identities);
        layout.truncate_primary = Some(Rc::new(move |context| {
            calls.borrow_mut().push(json!([
                "truncate",
                context.text,
                context.max_width,
                context.column_width,
                identities
                    .iter()
                    .position(|value| *value == context.item.value)
                    .unwrap(),
                context.is_selected
            ]));
            if custom == "ellipsis" {
                ellipsis(context.text, context.max_width)
            } else {
                custom.clone()
            }
        }));
    }
    layout
}
/// The recorded custom ellipsis transformation on ASCII primary inputs.
fn ellipsis(text: &str, width: isize) -> String {
    let budget = usize::try_from(width).unwrap_or(0);
    if text.chars().count() <= budget {
        text.into()
    } else {
        format!(
            "{}…",
            text.chars()
                .take(budget.saturating_sub(1))
                .collect::<String>()
        )
    }
}
/// Records command notifications using their original positions.
fn select_callbacks(list: &SelectList, events: &Observations, identities: &Rc<Vec<String>>) {
    for name in ["select", "move"] {
        let events = Rc::clone(events);
        let identities = Rc::clone(identities);
        let callback = Rc::new(move |item: Rc<SelectItem>| {
            events.borrow_mut().push(json!([
                name,
                identities
                    .iter()
                    .position(|value| *value == item.value)
                    .unwrap(),
                item.value
            ]));
        });
        if name == "select" {
            list.set_on_select(Some(callback));
        } else {
            list.set_on_selection_change(Some(callback));
        }
    }
    let cancel = Rc::clone(events);
    list.set_on_cancel(Some(Rc::new(move || {
        cancel.borrow_mut().push(json!(["cancel"]));
    })));
}
/// Reads or checks one saved observable result.
///
/// # Panics
/// Panics on invalid fixture data or a mismatched observation.
pub fn select_case(case: Case) {
    let input = case.input;
    let calls: Observations = Rc::default();
    let events: Observations = Rc::default();
    let items = input.items.map_or_else(commands, |items| {
        items.into_iter().map(Item::command).collect()
    });
    let identities: Rc<Vec<String>> =
        Rc::new(items.iter().map(|item| item.value.clone()).collect());
    let layout = layout(input.layout, input.custom, &calls, &identities);
    assert!(input.search.is_none() && input.cursor.is_none());
    let list = SelectList::new(
        items,
        input.max_visible.unwrap_or(5),
        select_theme(input.trace.unwrap_or(false), &calls),
        layout,
    );
    select_callbacks(&list, &events, &identities);
    let mut frames = Vec::new();
    for op in input.ops.unwrap_or_default() {
        if let Some(filter) = op.filter {
            list.set_filter(&filter);
        }
        if let Some(index) = op.index {
            list.set_selected_index(index);
        }
        if let Some(key) = op.key {
            list.handle_input(&key);
        }
        if let Some(width) = op.width {
            frames.push(list.render(width));
        }
        if op.invalidate.unwrap_or(false) {
            list.invalidate();
        }
        assert!(op.update.is_none());
    }
    if let Some(width) = input.width {
        frames.push(list.render(width));
    }
    let actual = json!({"frames":frames,"selected":list.get_selected_item().map(|item|item.value.clone()),"events":*events.borrow(),"calls":*calls.borrow()});
    assert_eq!(actual, case.expected, "{}", case.case);
}

use maestro_tui::components::settings_list::SettingsListOptions;
use maestro_tui::{SettingItem, SettingsList, SettingsListTheme};
/// Selected-row theme callable.
type RowStyle = Rc<dyn Fn(&str, bool) -> String>;
#[must_use]
pub fn settings() -> Vec<SettingItem> {
    [
        (
            "a",
            "Alpha",
            "off",
            Some(vec!["off", "on"]),
            Some("first paragraph\nnext paragraph"),
        ),
        ("b", "Beta label", "2", Some(vec!["1", "2", "3"]), None),
        ("c", "Gamma", "read only", None, None),
        ("d", "Delta", "unknown", Some(vec!["x", "y"]), None),
    ]
    .into_iter()
    .map(|(id, label, value, values, desc)| SettingItem {
        id: id.into(),
        label: label.into(),
        description: desc.map(str::to_owned),
        current_value: value.into(),
        values: values.map(|v| v.into_iter().map(str::to_owned).collect()),
        submenu: None,
    })
    .collect()
}
#[must_use]
pub fn settings_theme(trace: bool, calls: &Observations) -> SettingsListTheme {
    let row = |name: &'static str| -> RowStyle {
        let calls = Rc::clone(calls);
        Rc::new(move |text, selected| {
            if trace {
                calls.borrow_mut().push(json!([name, text, selected]));
                format!("\x1b[32m{text}\x1b[0m")
            } else {
                text.to_owned()
            }
        })
    };
    let style = |name: &'static str| -> Rc<dyn Fn(&str) -> String> {
        let calls = Rc::clone(calls);
        Rc::new(move |text| {
            if trace {
                calls.borrow_mut().push(json!([name, text]));
                format!("\x1b[32m{text}\x1b[0m")
            } else {
                text.to_owned()
            }
        })
    };
    SettingsListTheme {
        label: row("label"),
        value: row("value"),
        description: style("description"),
        cursor: "→ ".into(),
        hint: style("hint"),
    }
}
/// Reads or checks one saved observable result.
///
/// # Panics
/// Panics on invalid fixture data or a mismatched observation.
pub fn settings_case(case: Case) {
    let input = case.input;
    let calls: Observations = Rc::default();
    let events: Observations = Rc::default();
    let items = input.items.map_or_else(settings, |items| {
        items.into_iter().map(Item::setting).collect()
    });
    assert!(input.layout.is_none() && input.custom.is_none());
    let mut theme = settings_theme(input.trace.unwrap_or(false), &calls);
    if let Some(cursor) = input.cursor {
        theme.cursor = cursor;
    }
    let change = Rc::clone(&events);
    let cancel = Rc::clone(&events);
    let list = SettingsList::new(
        items,
        input.max_visible.unwrap_or(3),
        theme,
        (
            Rc::new(move |id, value| change.borrow_mut().push(json!(["change", id, value]))),
            Rc::new(move || cancel.borrow_mut().push(json!(["cancel"]))),
        ),
        SettingsListOptions {
            enable_search: input.search.unwrap_or(false),
        },
    );
    let mut frames = Vec::new();
    for op in input.ops.unwrap_or_default() {
        assert!(op.filter.is_none() && op.index.is_none());
        if let Some(key) = op.key {
            list.handle_input(&key);
        }
        if let Some((id, value)) = op.update {
            list.update_value(&id, value);
        }
        if let Some(width) = op.width {
            frames.push(list.render(width));
        }
        if op.invalidate.unwrap_or(false) {
            list.invalidate();
        }
    }
    if let Some(width) = input.width {
        frames.push(list.render(width));
    }
    assert_eq!(
        json!({"frames":frames,"events":*events.borrow(),"calls":*calls.borrow()}),
        case.expected,
        "{}",
        case.case
    );
}

/// Reads or checks one saved observable result.
///
/// # Panics
/// Panics on invalid fixture data or a mismatched observation.
pub fn special(case: Case, id: &str, actual: &Value) {
    assert_eq!(case.case, id);
    let input = case.input;
    assert!(
        input.items.is_none()
            && input.width.is_none()
            && input.ops.is_none()
            && input.max_visible.is_none()
            && input.layout.is_none()
            && input.custom.is_none()
            && input.trace.is_none()
            && input.search.is_none()
            && input.cursor.is_none()
    );
    assert_eq!(*actual, case.expected, "{id}");
}

pub struct Child {
    pub text: &'static str,
    pub events: Option<Observations>,
    pub accepts_input: bool,
}
impl Component for Child {
    fn render(&self, width: usize) -> Vec<String> {
        if let Some(events) = &self.events {
            events.borrow_mut().push(json!(["render", width]));
        }
        vec![self.text.into()]
    }
    fn invalidate(&self) {
        if let Some(events) = &self.events {
            events.borrow_mut().push(json!(["invalidate"]));
        }
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        self.accepts_input.then_some(self)
    }
}
impl InputHandler for Child {
    fn handle_input(&self, data: &str) {
        if let Some(events) = &self.events {
            events.borrow_mut().push(json!(["input", data]));
        }
    }
}
