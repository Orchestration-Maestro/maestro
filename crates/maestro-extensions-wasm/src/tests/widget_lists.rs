//! Selection lists, settings lists and loaders both test hosts keep, with the nested calls
//! their input makes.
//!
//! A selection row is `> label` when selected and `  label` otherwise, painted by the
//! selected-text style; an authored truncation replaces the row. A settings row is the label
//! and the value, each through its row style. A loader renders `loader <message>`. Enter
//! confirms or activates, Escape cancels, the arrow keys move the selection.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::json;

use super::observed::Observed;
use super::widget_host::{Paint, Part};
use crate::SelectItem;

/// What the host tells an authored widget callback.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A confirmed choice.
    Select(SelectItem),
    /// A cancellation.
    Cancel,
    /// A moved selection.
    SelectionChange(SelectItem),
    /// A setting's id and new value.
    Change(String, String),
    /// A loader's cancellation.
    Abort,
    /// A row to truncate: its text, width and whether it is selected.
    Truncate(SelectItem, i64, bool),
    /// A settings row's text and whether it is selected.
    Row(String, bool),
    /// A submenu to open for the setting at this index with its current value.
    Submenu(usize, String),
    /// An editor's submitted or changed text.
    Text(String),
    /// An editor key or action.
    Notice,
    /// Input offered to an editor's interceptor.
    Shortcut(String),
}

/// One nested call an input makes.
#[derive(Clone, Debug, PartialEq)]
pub enum Call {
    /// An authored widget callback by identity.
    Widget(u32, Event),
    /// Input to an authored component by identity.
    Input(u32, String),
}

/// One setting the host keeps.
#[derive(Clone)]
pub struct Setting {
    /// Identity.
    pub id: String,
    /// Label.
    pub label: String,
    /// Current value.
    pub value: String,
    /// Values activation cycles through.
    pub values: Option<Vec<String>>,
    /// The authored submenu's identity.
    pub submenu: Option<u32>,
}

/// The state of one list or loader.
#[derive(Default)]
pub struct List {
    /// Choices of a selection list.
    pub items: Vec<SelectItem>,
    /// Indexes of the choices the filter keeps.
    pub filtered: Vec<usize>,
    /// Selected row.
    pub selected: usize,
    /// The selected-text style.
    pub selected_style: Option<Paint>,
    /// The authored truncation.
    pub truncate: Option<u32>,
    /// Handlers by slot name.
    pub handlers: HashMap<&'static str, u32>,
    /// Settings of a settings list.
    pub settings: Vec<Setting>,
    /// Label and value row styles.
    pub rows: Option<(Paint, Paint)>,
    /// The open submenu: its setting and its component's identity.
    pub submenu: Option<(usize, u32)>,
    /// A loader's message, and whether input can cancel it.
    pub loader: Option<(String, bool)>,
    /// A loader's signal.
    pub signal: Arc<AtomicBool>,
}

/// Lists by widget key.
#[derive(Default)]
pub struct Lists(HashMap<u32, List>);

impl Lists {
    /// Takes the callbacks kept by list event slots, leaving those slots empty.
    pub fn take_handlers(&mut self) -> Vec<u32> {
        self.0
            .values_mut()
            .flat_map(|list| std::mem::take(&mut list.handlers).into_values())
            .collect()
    }
}

/// The list under `key`.
pub fn list(observed: &mut Observed, key: u32) -> Result<&mut List, String> {
    observed
        .ui
        .lists
        .0
        .get_mut(&key)
        .ok_or_else(|| format!("widget {key} is no list"))
}

/// Keeps a list under a widget key.
pub fn keep(observed: &mut Observed, key: u32, mut list: List) {
    list.filtered = (0..list.items.len()).collect();
    observed.ui.lists.0.insert(key, list);
}

/// A row painted by host or authored `paint`.
fn painted(paint: Option<&Paint>, row: String) -> Part {
    match paint {
        Some(Paint::Host(role)) => Part::Row(format!("<{role}>{row}")),
        Some(Paint::Authored(id)) => Part::Style(*id, row),
        None => Part::Row(row),
    }
}

/// A settings row style applied to `text`.
fn row(paint: &Paint, text: &str, selected: bool) -> Part {
    match paint {
        Paint::Host(role) => Part::Row(format!("<{role} {selected}>{text}")),
        Paint::Authored(id) => Part::Widget(*id, Event::Row(text.to_owned(), selected)),
    }
}

/// The pieces of rendering a list or loader, or `None` for another widget.
pub fn render(observed: &mut Observed, key: u32, width: u32) -> Option<Vec<Part>> {
    let list = observed.ui.lists.0.get(&key)?;
    if let Some((message, _)) = &list.loader {
        return Some(vec![Part::Row(format!("loader {message}"))]);
    }
    if let Some((_, component)) = list.submenu {
        return Some(vec![Part::Component(component)]);
    }
    if let Some((label, value)) = &list.rows {
        let parts = list
            .settings
            .iter()
            .enumerate()
            .flat_map(|(index, setting)| {
                let selected = index == list.selected;
                [
                    row(label, &setting.label, selected),
                    row(value, &setting.value, selected),
                ]
            });
        return Some(parts.collect());
    }
    let parts = list.filtered.iter().enumerate().map(|(row, &index)| {
        let item = list.items[index].clone();
        let selected = row == list.selected;
        match list.truncate {
            Some(id) => Part::Widget(id, Event::Truncate(item, i64::from(width), selected)),
            None if selected => painted(list.selected_style.as_ref(), format!("> {}", item.label)),
            None => Part::Row(format!("  {}", item.label)),
        }
    });
    Some(parts.collect())
}

/// The selected choice of the list under `key`.
pub fn selected(list: &List) -> Option<SelectItem> {
    list.filtered
        .get(list.selected)
        .map(|&index| list.items[index].clone())
}

/// The nested call one handler makes, when the handler is set.
fn handler(list: &List, slot: &str, event: Event) -> Option<Call> {
    list.handlers.get(slot).map(|&id| Call::Widget(id, event))
}

/// Records one input and returns the authored calls it makes, in order.
///
/// # Errors
/// Returns a message when the widget is no list.
pub fn input(observed: &mut Observed, key: u32, data: &str) -> Result<Vec<Call>, String> {
    observed.log(format!("widget {key} input {}", json!(data)));
    let list = list(observed, key)?;
    if let Some((_, cancellable)) = &list.loader {
        if data != "\x1b" || !cancellable {
            return Ok(Vec::new());
        }
        list.signal.store(true, Ordering::SeqCst);
        return Ok(handler(list, "abort", Event::Abort).into_iter().collect());
    }
    if list.rows.is_some() {
        return Ok(settings_input(list, data));
    }
    let count = list.filtered.len();
    let calls = match data {
        "\r" => selected(list).and_then(|item| handler(list, "select", Event::Select(item))),
        "\x1b" => handler(list, "cancel", Event::Cancel),
        "\x1b[A" | "\x1b[B" if count > 0 => {
            let step = if data == "\x1b[A" { count - 1 } else { 1 };
            list.selected = (list.selected + step) % count;
            let item = selected(list);
            item.and_then(|item| handler(list, "selectionChange", Event::SelectionChange(item)))
        }
        _ => None,
    };
    Ok(calls.into_iter().collect())
}

/// The authored calls one settings input makes.
fn settings_input(list: &mut List, data: &str) -> Vec<Call> {
    if let Some((_, component)) = list.submenu {
        return vec![Call::Input(component, data.to_owned())];
    }
    if data == "\x1b" {
        return handler(list, "cancel", Event::Cancel).into_iter().collect();
    }
    let count = list.settings.len();
    if matches!(data, "\x1b[A" | "\x1b[B") && count > 0 {
        let step = if data == "\x1b[A" { count - 1 } else { 1 };
        list.selected = (list.selected + step) % count;
        return Vec::new();
    }
    let index = list.selected;
    let Some(setting) = list.settings.get_mut(index).filter(|_| data == "\r") else {
        return Vec::new();
    };
    if let Some(submenu) = setting.submenu {
        return vec![Call::Widget(
            submenu,
            Event::Submenu(index, setting.value.clone()),
        )];
    }
    let Some(values) = &setting.values else {
        return Vec::new();
    };
    let next = values.iter().position(|value| *value == setting.value);
    let next = next.map_or(0, |position| (position + 1) % values.len());
    setting.value = values[next].clone();
    let change = Event::Change(setting.id.clone(), setting.value.clone());
    handler(list, "change", change).into_iter().collect()
}

/// Closes the open submenu of the list under `key`, recording it, and returns the change the
/// chosen value reports.
///
/// # Errors
/// Returns a message when no submenu is open.
pub fn done(
    observed: &mut Observed,
    key: u32,
    value: Option<&str>,
) -> Result<Option<Call>, String> {
    observed.log(format!("widget {key} done {}", json!(value)));
    let list = list(observed, key)?;
    let (index, _) = list.submenu.take().ok_or("no submenu is open")?;
    list.selected = index;
    let Some(value) = value else {
        return Ok(None);
    };
    let setting = &mut list.settings[index];
    setting.value = value.to_owned();
    let change = Event::Change(setting.id.clone(), value.to_owned());
    Ok(handler(list, "change", change))
}
