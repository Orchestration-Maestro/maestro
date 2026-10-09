//! Settings rows with optional label search and submenu activation.

use crate::{
    Component, Input, TruncateOptions, fuzzy_filter, get_keybindings, truncate_to_width,
    tui::InputHandler, visible_width, wrap_text_with_ansi,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Repeatable submenu completion callable.
type Done = Rc<dyn Fn(Option<String>)>;
/// Submenu factory receiving the current value.
type Submenu = Rc<dyn Fn(String, Done) -> Rc<dyn Component>>;
/// Setting change notification.
type Change = Rc<dyn Fn(&str, &str)>;
/// Row styling with selected state.
type RowStyle = Rc<dyn Fn(&str, bool) -> String>;
/// A displayed setting with optional activation behavior.
pub struct SettingItem {
    /// Setting identity; duplicate IDs are permitted.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Selected-row description.
    pub description: Option<String>,
    /// Displayed value.
    pub current_value: String,
    /// Values available for cycling.
    pub values: Option<Vec<String>>,
    /// Submenu factory, taking precedence over cycling.
    pub submenu: Option<Submenu>,
}
/// Settings styling callbacks and cursor prefix.
pub struct SettingsListTheme {
    /// Styles padded labels.
    pub label: RowStyle,
    /// Styles clipped values.
    pub value: RowStyle,
    /// Styles indented description rows.
    pub description: Rc<dyn Fn(&str) -> String>,
    /// Literal selected-row prefix.
    pub cursor: String,
    /// Styles hints and scroll metadata.
    pub hint: Rc<dyn Fn(&str) -> String>,
}
/// Optional settings search.
#[derive(Clone, Copy, Default)]
pub struct SettingsListOptions {
    /// Enables label search using the shared matcher.
    pub enable_search: bool,
}
/// A setting whose current value is shared by views and completion callbacks.
struct SharedSetting {
    /// Setting identity.
    id: String,
    /// Display label.
    label: String,
    /// Selected description.
    description: Option<String>,
    /// Shared current value.
    value: RefCell<String>,
    /// Cycling choices.
    values: Option<Vec<String>>,
    /// Submenu factory.
    submenu: Option<Submenu>,
}
/// Active view state held weakly by completion callbacks.
#[derive(Default)]
struct SettingsState {
    /// Current matching collection identity.
    filtered: RefCell<Rc<Vec<Rc<SharedSetting>>>>,
    /// Selected row.
    selected: Cell<usize>,
    /// Active child component.
    child: RefCell<Option<Rc<dyn Component>>>,
    /// Index saved when opening a child.
    opener: Cell<Option<usize>>,
}
impl SettingsState {
    /// Releases the child before restoring the saved opener selection.
    fn close(&self) {
        let old = self.child.replace(None);
        drop(old);
        if let Some(index) = self.opener.take() {
            self.selected.set(index);
        }
    }
}
/// A terminal settings component with optional label search.
pub struct SettingsList {
    /// Original records.
    items: Rc<Vec<Rc<SharedSetting>>>,
    /// Replaceable view state.
    state: Rc<SettingsState>,
    /// Row styling.
    theme: SettingsListTheme,
    /// Persistent search editor.
    search: Option<Input>,
    /// Maximum visible rows.
    max_visible: usize,
    /// Activation notification.
    on_change: Change,
    /// Cancellation notification.
    on_cancel: Rc<dyn Fn()>,
}
impl SettingsList {
    /// Owns settings and styling, retaining notification callables.
    #[must_use]
    pub fn new(
        items: Vec<SettingItem>,
        max_visible: usize,
        theme: SettingsListTheme,
        callbacks: (Change, Rc<dyn Fn()>),
        options: SettingsListOptions,
    ) -> Self {
        let items: Rc<Vec<_>> = Rc::new(
            items
                .into_iter()
                .map(|item| {
                    Rc::new(SharedSetting {
                        id: item.id,
                        label: item.label,
                        description: item.description,
                        value: RefCell::new(item.current_value),
                        values: item.values,
                        submenu: item.submenu,
                    })
                })
                .collect(),
        );
        let state = Rc::new(SettingsState::default());
        state.filtered.replace(Rc::clone(&items));
        Self {
            items,
            state,
            theme,
            search: options.enable_search.then(Input::new),
            max_visible,
            on_change: callbacks.0,
            on_cancel: callbacks.1,
        }
    }
    /// Updates the first matching ID without change notification.
    pub fn update_value(&self, id: &str, new_value: String) {
        if let Some(item) = self.items.iter().find(|item| item.id == id) {
            item.value.replace(new_value);
        }
    }
    /// Captures the display collection for one operation.
    fn display_items(&self) -> Rc<Vec<Rc<SharedSetting>>> {
        if self.search.is_some() {
            self.state.filtered.borrow().clone()
        } else {
            Rc::clone(&self.items)
        }
    }
    /// Filters labels through the shared matcher, resetting selection.
    fn apply_filter(&self, query: &str) {
        let filtered = Rc::new(
            fuzzy_filter(&self.items, query, |item| &item.label)
                .into_iter()
                .cloned()
                .collect(),
        );
        let old = self.state.filtered.replace(filtered);
        drop(old);
        self.state.selected.set(0);
    }
    /// Appends the main-list hint after a separator.
    fn hint(&self, lines: &mut Vec<String>, width: usize) {
        lines.push(String::new());
        let text = if self.search.is_some() {
            "  Type to search · Enter/Space to change · Esc to cancel"
        } else {
            "  Enter/Space to change · Esc to cancel"
        };
        lines.push(truncate_to_width(
            &(self.theme.hint)(text),
            width,
            TruncateOptions::default(),
        ));
    }
}
impl SettingsList {
    /// Renders the captured main-list view with live current values.
    fn main_list(&self, width: usize) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(search) = &self.search {
            lines.extend(search.render(width));
            lines.push(String::new());
        }
        if self.items.is_empty() {
            lines.push(clip(&(self.theme.hint)("  No settings available"), width));
            if self.search.is_some() {
                self.hint(&mut lines, width);
            }
            return lines;
        }
        let items = self.display_items();
        if items.is_empty() {
            lines.push(truncate_to_width(
                &(self.theme.hint)("  No matching settings"),
                width,
                TruncateOptions::default(),
            ));
            self.hint(&mut lines, width);
            return lines;
        }
        let start = self
            .state
            .selected
            .get()
            .saturating_sub(self.max_visible / 2)
            .min(items.len().saturating_sub(self.max_visible));
        let end = start.saturating_add(self.max_visible).min(items.len());
        let label_width = self
            .items
            .iter()
            .map(|item| visible_width(&item.label))
            .max()
            .unwrap_or(0)
            .min(30);
        for (index, item) in items.iter().enumerate().take(end).skip(start) {
            lines.push(self.row(item, index == self.state.selected.get(), width, label_width));
        }
        if start > 0 || end < items.len() {
            lines.push((self.theme.hint)(&clip(
                &format!("  ({}/{})", self.state.selected.get() + 1, items.len()),
                width.saturating_sub(2),
            )));
        }
        self.description(&items, &mut lines, width);
        self.hint(&mut lines, width);
        lines
    }
    /// Appends the current selected description from the captured view.
    fn description(&self, items: &[Rc<SharedSetting>], lines: &mut Vec<String>, width: usize) {
        if let Some(description) = items
            .get(self.state.selected.get())
            .and_then(|item| item.description.as_deref())
            .filter(|text| !text.is_empty())
        {
            lines.push(String::new());
            lines.extend(
                wrap_text_with_ansi(description, width.saturating_sub(4))
                    .into_iter()
                    .map(|line| (self.theme.description)(&format!("  {line}"))),
            );
        }
    }
    /// Styles label and then reads the current value before composing the row.
    fn row(
        &self,
        item: &SharedSetting,
        selected: bool,
        width: usize,
        label_width: usize,
    ) -> String {
        let prefix = if selected { &self.theme.cursor } else { "  " };
        let label = format!(
            "{}{}",
            item.label,
            " ".repeat(label_width.saturating_sub(visible_width(&item.label)))
        );
        let label = (self.theme.label)(&label, selected);
        let budget = width.saturating_sub(visible_width(prefix) + label_width + 4);
        let value = clip(&item.value.borrow(), budget);
        let value = (self.theme.value)(&value, selected);
        truncate_to_width(
            &format!("{prefix}{label}  {value}"),
            width,
            TruncateOptions::default(),
        )
    }
    /// Activates the selected setting.
    fn activate(&self) {
        let items = self.display_items();
        let Some(item) = items.get(self.state.selected.get()) else {
            return;
        };
        if let Some(factory) = &item.submenu {
            self.open_submenu(Rc::clone(item), factory);
        } else if let Some(values) = item.values.as_ref().filter(|values| !values.is_empty()) {
            let next = values
                .iter()
                .position(|value| *value == *item.value.borrow())
                .map_or(0, |index| (index + 1) % values.len());
            let value = values[next].clone();
            item.value.replace(value.clone());
            (self.on_change)(&item.id, &value);
        }
    }
    /// Opens a child whose completion retains the original setting.
    fn open_submenu(&self, item: Rc<SharedSetting>, factory: &Submenu) {
        self.state.opener.set(Some(self.state.selected.get()));
        let owner = Rc::downgrade(&self.state);
        let change = Rc::clone(&self.on_change);
        let current = item.value.borrow().clone();
        let completed = Rc::new(Cell::new(false));
        let completion = Rc::clone(&completed);
        let done: Done = Rc::new(move |value| {
            completion.set(true);
            if let Some(value) = value {
                item.value.replace(value.clone());
                change(&item.id, &value);
            }
            if let Some(state) = owner.upgrade() {
                state.close();
            }
        });
        let child = factory(current, done);
        if !completed.get() {
            let old = self.state.child.replace(Some(child));
            drop(old);
        }
    }
}
impl Component for SettingsList {
    fn render(&self, width: usize) -> Vec<String> {
        let child = self.state.child.borrow().clone();
        child.map_or_else(|| self.main_list(width), |child| child.render(width))
    }
    fn invalidate(&self) {
        let child = self.state.child.borrow().clone();
        if let Some(child) = child {
            child.invalidate();
        }
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
}
impl InputHandler for SettingsList {
    fn handle_input(&self, data: &str) {
        let child = self.state.child.borrow().clone();
        if let Some(child) = child {
            if let Some(handler) = child.input_handler() {
                handler.handle_input(data);
            }
            return;
        }
        let bindings = get_keybindings();
        let count = self.display_items().len();
        let selected = self.state.selected.get();
        if bindings.matches(data, "tui.select.up") {
            if count == 0 {
                return;
            }
            self.state.selected.set(if selected == 0 {
                count - 1
            } else {
                selected - 1
            });
        } else if bindings.matches(data, "tui.select.down") {
            if count == 0 {
                return;
            }
            self.state.selected.set(if selected == count - 1 {
                0
            } else {
                selected + 1
            });
        } else if bindings.matches(data, "tui.select.confirm") || data == " " {
            self.activate();
        } else if bindings.matches(data, "tui.select.cancel") {
            (self.on_cancel)();
        } else if let Some(search) = &self.search {
            let sanitized = data.replace(' ', "");
            if sanitized.is_empty() {
                return;
            }
            search.handle_input(&sanitized);
            self.apply_filter(&search.get_value());
        }
    }
}
/// Clips value and metadata without an ellipsis.
fn clip(text: &str, width: usize) -> String {
    truncate_to_width(
        text,
        width,
        TruncateOptions {
            ellipsis: "",
            pad: false,
        },
    )
}
