//! Retained input, flat-entry navigation and replaceable notifications.
use super::{
    ConfigSelectorOptions, ResourceItem, ResourceToggle,
    groups::{ResourceGroup, ResourceSubgroup},
};
use maestro_app::SettingsManager;
use maestro_tui::{
    Component, Input, get_keybindings, matches_key,
    tui::{FocusFlag, Focusable, InputHandler},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// A painted heading or live resource occurrence.
#[derive(Clone)]
pub(super) enum FlatEntry {
    /// Captured source label.
    Group(Rc<str>),
    /// Captured kind label.
    Subgroup(Rc<str>),
    /// Shared live item.
    Item(Rc<RefCell<ResourceItem>>),
}
/// Resource list whose headings are excluded from selection.
pub struct ResourceList {
    /// Immutable ancestry retaining live items.
    groups: Vec<ResourceGroup>,
    /// Current flattened filtered view.
    pub(super) filtered: RefCell<Vec<FlatEntry>>,
    /// Selected flat-entry index.
    pub(super) selected: Cell<usize>,
    /// Existing search owner.
    pub(super) search: Rc<Input>,
    /// Scope persistence owner.
    pub(super) settings: Rc<RefCell<SettingsManager>>,
    /// Supplied ambient effects.
    pub(super) options: Rc<ConfigSelectorOptions>,
    /// Forwarding search focus.
    focus: FocusFlag,
    /// Cancellation slot.
    cancel: RefCell<Option<Rc<dyn Fn()>>>,
    /// Interrupt slot.
    exit: RefCell<Option<Rc<dyn Fn()>>>,
    /// Successful-toggle slot.
    toggle: RefCell<Option<ResourceToggle>>,
}
impl ResourceList {
    /// Builds the first flat view and selects its first resource.
    pub(super) fn new(
        groups: Vec<ResourceGroup>,
        settings: Rc<RefCell<SettingsManager>>,
        options: Rc<ConfigSelectorOptions>,
    ) -> Self {
        let search = Rc::new(Input::new());
        let focus = FocusFlag::forwarding(search.clone());
        let list = Self {
            groups,
            filtered: RefCell::default(),
            selected: Cell::new(0),
            search,
            settings,
            options,
            focus,
            cancel: RefCell::default(),
            exit: RefCell::default(),
            toggle: RefCell::default(),
        };
        list.filter_items("");
        list
    }
    /// Returns the current cancellation handle.
    #[must_use]
    pub fn on_cancel(&self) -> Option<Rc<dyn Fn()>> {
        self.cancel.borrow().clone()
    }
    /// Replaces cancellation, releasing the old capture outside the slot borrow.
    pub fn set_on_cancel(&self, callback: Option<Rc<dyn Fn()>>) {
        let old = self.cancel.replace(callback);
        drop(old);
    }
    /// Returns the current interrupt handle.
    #[must_use]
    pub fn on_exit(&self) -> Option<Rc<dyn Fn()>> {
        self.exit.borrow().clone()
    }
    /// Replaces interrupt notification without retaining a slot borrow.
    pub fn set_on_exit(&self, callback: Option<Rc<dyn Fn()>>) {
        let old = self.exit.replace(callback);
        drop(old);
    }
    /// Returns the current successful-toggle handle.
    #[must_use]
    pub fn on_toggle(&self) -> Option<ResourceToggle> {
        self.toggle.borrow().clone()
    }
    /// Replaces toggle notification without retaining a slot borrow.
    pub fn set_on_toggle(&self, callback: Option<ResourceToggle>) {
        let old = self.toggle.replace(callback);
        drop(old);
    }
    /// Updates the supplied item first, then the first current path/kind match.
    pub fn update_item(&self, item: Rc<RefCell<ResourceItem>>, enabled: bool) {
        let (path, kind) = {
            let mut item = item.borrow_mut();
            item.enabled = enabled;
            (item.path.clone(), item.resource_type.clone())
        };
        drop(item);
        for candidate in self
            .groups
            .iter()
            .flat_map(|group| &group.subgroups)
            .flat_map(|group| &group.items)
        {
            let matches = {
                let candidate = candidate.borrow();
                candidate.path == path && candidate.resource_type == kind
            };
            if matches {
                candidate.borrow_mut().enabled = enabled;
                return;
            }
        }
    }
    /// Filters live item fields, retaining their captured ancestors and handles.
    fn filter_items(&self, query: &str) {
        let lower = query.to_lowercase();
        let all = query
            .trim_matches(maestro_app::is_whitespace_scalar)
            .is_empty();
        let query = (!all).then_some(lower.as_str());
        let mut entries = Vec::new();
        for group in &self.groups {
            let mut children = Vec::new();
            for subgroup in &group.subgroups {
                children.extend(subgroup.entries(query));
            }
            if !children.is_empty() {
                entries.push(FlatEntry::Group(group.label.clone()));
                entries.extend(children);
            }
        }
        self.selected.set(
            entries
                .iter()
                .position(|entry| matches!(entry, FlatEntry::Item(_)))
                .unwrap_or(0),
        );
        self.filtered.replace(entries);
    }
    /// Searches an index range in the requested direction, clamping when empty.
    fn select_in(&self, mut indices: impl Iterator<Item = usize>) {
        let entries = self.filtered.borrow();
        if let Some(index) =
            indices.find(|&index| matches!(entries.get(index), Some(FlatEntry::Item(_))))
        {
            self.selected.set(index);
        }
    }
    /// Persists the selected toggle before updating and notifying without borrows.
    fn confirm(&self) -> std::io::Result<()> {
        let item = match self.filtered.borrow().get(self.selected.get()) {
            Some(FlatEntry::Item(item)) => Some(item.clone()),
            _ => None,
        };
        if let Some(item) = item {
            let enabled = !item.borrow().enabled;
            self.toggle_resource(&item, enabled)?;
            self.update_item(item.clone(), enabled);
            if let Some(callback) = self.on_toggle() {
                callback(item, enabled);
            }
        }
        Ok(())
    }
    /// Dispatches live bindings in strict navigation/cancel/toggle/search priority.
    ///
    /// # Errors
    /// Returns the original native path failure before changing rows or settings.
    pub fn handle_input(&self, data: &str) -> std::io::Result<()> {
        let bindings = get_keybindings();
        let index = self.selected.get();
        let count = self.filtered.borrow().len();
        if bindings.matches(data, "tui.select.up") {
            self.select_in((0..index).rev());
        } else if bindings.matches(data, "tui.select.down") {
            self.select_in(index + 1..count);
        } else if bindings.matches(data, "tui.select.pageUp") {
            self.select_in(index.saturating_sub(15)..count);
        } else if bindings.matches(data, "tui.select.pageDown") {
            self.select_in((0..=index.saturating_add(15).min(count.saturating_sub(1))).rev());
        } else if bindings.matches(data, "tui.select.cancel") {
            if let Some(callback) = self.on_cancel() {
                callback();
            }
        } else if matches_key(data, "ctrl+c") {
            if let Some(callback) = self.on_exit() {
                callback();
            }
        } else if data == " " || bindings.matches(data, "tui.select.confirm") {
            self.confirm()?;
        } else {
            self.search.handle_input(data);
            self.filter_items(&self.search.get_value());
        }
        Ok(())
    }
}
impl Focusable for ResourceList {
    fn focus_flag(&self) -> &FocusFlag {
        &self.focus
    }
}
impl InputHandler for ResourceList {
    /// Legacy input discards only the error; failed toggles leave state unchanged.
    fn handle_input(&self, data: &str) {
        let _ = ResourceList::handle_input(self, data);
    }
    fn try_handle_input(&self, data: &str) -> Result<(), Box<dyn std::error::Error>> {
        Ok(ResourceList::handle_input(self, data)?)
    }
}
impl Component for ResourceList {
    /// Legacy rendering emits no rows when theme rendering fails.
    fn render(&self, width: usize) -> Vec<String> {
        self.try_render(width).unwrap_or_default()
    }
    fn try_render(&self, width: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        self.render_rows(width)
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
}

impl ResourceSubgroup {
    /// Keeps a heading only when at least one live item matches the query.
    fn entries(&self, query: Option<&str>) -> Vec<FlatEntry> {
        let mut entries = vec![FlatEntry::Subgroup(self.label.clone())];
        entries.extend(
            self.items
                .iter()
                .filter(|item| {
                    let item = item.borrow();
                    query.is_none_or(|query| {
                        [&item.display_name, &item.resource_type, &item.path]
                            .iter()
                            .any(|text| text.to_lowercase().contains(query))
                    })
                })
                .cloned()
                .map(FlatEntry::Item),
        );
        if entries.len() == 1 {
            entries.clear();
        }
        entries
    }
}
