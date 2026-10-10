//! Command selection with prefix filtering and aligned descriptions.

use crate::text::utils::is_whitespace_scalar;
use crate::{Component, TruncateOptions, truncate_to_width, visible_width};
use crate::{get_keybindings, tui::InputHandler};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Command notification callable.
type ItemCallback = Rc<dyn Fn(Rc<SelectItem>)>;

/// A command identity and its display text.
pub struct SelectItem {
    /// Command identity.
    pub value: String,
    /// Primary display text; an empty label displays the value.
    pub label: String,
    /// Optional description.
    pub description: Option<String>,
}
/// Styling callbacks for command rows.
#[derive(Clone)]
pub struct SelectListTheme {
    /// Declared styling callback that this component does not invoke.
    pub selected_prefix: Rc<dyn Fn(&str) -> String>,
    /// Styles a selected row.
    pub selected_text: Rc<dyn Fn(&str) -> String>,
    /// Styles an unselected description with its spacing.
    pub description: Rc<dyn Fn(&str) -> String>,
    /// Styles scroll metadata.
    pub scroll_info: Rc<dyn Fn(&str) -> String>,
    /// Styles the empty-match message.
    pub no_match: Rc<dyn Fn(&str) -> String>,
}
/// Signed layout budgets and original command identity for custom truncation.
pub struct SelectListTruncatePrimaryContext<'a> {
    /// Display text.
    pub text: &'a str,
    /// Available primary cells.
    pub max_width: isize,
    /// Primary column cells.
    pub column_width: isize,
    /// Original command handle.
    pub item: Rc<SelectItem>,
    /// Whether the row is selected.
    pub is_selected: bool,
}
/// Custom primary truncation callable.
type TruncatePrimary = Rc<dyn for<'a> Fn(SelectListTruncatePrimaryContext<'a>) -> String>;
/// Primary column bounds and optional truncation.
#[derive(Default)]
pub struct SelectListLayoutOptions {
    /// Lower column bound.
    pub min_primary_column_width: Option<isize>,
    /// Upper column bound.
    pub max_primary_column_width: Option<isize>,
    /// Custom primary truncation, followed by cell clipping.
    pub truncate_primary: Option<TruncatePrimary>,
}
/// A terminal command-selection component.
pub struct SelectList {
    /// Owned original commands.
    items: Vec<Rc<SelectItem>>,
    /// Current matching handles.
    filtered: RefCell<Vec<Rc<SelectItem>>>,
    /// Current row.
    selected: Cell<usize>,
    /// Confirmation callable.
    select: RefCell<Option<ItemCallback>>,
    /// Movement callable.
    change: RefCell<Option<ItemCallback>>,
    /// Cancellation callable.
    cancel: RefCell<Option<Rc<dyn Fn()>>>,
    /// Row styling.
    theme: SelectListTheme,
    /// Primary layout.
    layout: SelectListLayoutOptions,
    /// Maximum visible rows.
    max_visible: usize,
}
impl SelectList {
    /// Owns command records and layout.
    #[must_use]
    pub fn new(
        items: Vec<SelectItem>,
        max_visible: usize,
        theme: SelectListTheme,
        layout: SelectListLayoutOptions,
    ) -> Self {
        let items: Vec<_> = items.into_iter().map(Rc::new).collect();
        Self {
            filtered: RefCell::new(items.clone()),
            items,
            selected: Cell::new(0),
            select: RefCell::default(),
            change: RefCell::default(),
            cancel: RefCell::default(),
            theme,
            layout,
            max_visible,
        }
    }
    /// Measures the primary column across current matches.
    fn primary_width(&self) -> usize {
        let min = self
            .layout
            .min_primary_column_width
            .or(self.layout.max_primary_column_width)
            .unwrap_or(32);
        let max = self
            .layout
            .max_primary_column_width
            .or(self.layout.min_primary_column_width)
            .unwrap_or(32);
        let low = usize::try_from(min.min(max).max(1)).unwrap_or(1);
        let high = usize::try_from(min.max(max).max(1)).unwrap_or(1);
        self.filtered
            .borrow()
            .iter()
            .map(|item| visible_width(display(item)) + 2)
            .max()
            .unwrap_or(0)
            .clamp(low, high)
    }
    /// Applies custom truncation then clips its returned text.
    fn primary(
        &self,
        item: &Rc<SelectItem>,
        selected: bool,
        max_width: isize,
        column_width: isize,
    ) -> String {
        let text = display(item);
        let out = self.layout.truncate_primary.as_ref().map_or_else(
            || clip(text, max_width),
            |callback| {
                callback(SelectListTruncatePrimaryContext {
                    text,
                    max_width,
                    column_width,
                    item: Rc::clone(item),
                    is_selected: selected,
                })
            },
        );
        clip(&out, max_width)
    }
    /// Composes one row before final clipping.
    fn row(&self, item: &Rc<SelectItem>, selected: bool, width: usize, column: usize) -> String {
        let prefix = if selected { "→ " } else { "  " };
        let description = item
            .description
            .as_deref()
            .map(normalize)
            .unwrap_or_default();
        if !description.is_empty() && width > 40 {
            let effective = column.min(width - 6).max(1);
            let primary = self.primary(
                item,
                selected,
                signed(effective.saturating_sub(2).max(1)),
                signed(effective),
            );
            let spacing = " ".repeat(effective.saturating_sub(visible_width(&primary)).max(1));
            let remaining = signed(width) - 4 - signed(visible_width(&primary) + spacing.len());
            if remaining > 10 {
                let desc = clip(&description, remaining);
                return self.styled_description(
                    selected,
                    &format!("{prefix}{primary}"),
                    &format!("{spacing}{desc}"),
                );
            }
        }
        let budget = signed(width) - 4;
        let primary = self.primary(item, selected, budget, budget);
        let row = format!("{prefix}{primary}");
        if selected {
            (self.theme.selected_text)(&row)
        } else {
            row
        }
    }
    /// Styles the full selected row or only its unselected description.
    fn styled_description(&self, selected: bool, primary: &str, description: &str) -> String {
        if selected {
            (self.theme.selected_text)(&format!("{primary}{description}"))
        } else {
            format!("{primary}{}", (self.theme.description)(description))
        }
    }
}
impl Component for SelectList {
    fn render(&self, width: usize) -> Vec<String> {
        let count = self.filtered.borrow().len();
        if count == 0 {
            return vec![bound_styled(
                &(self.theme.no_match)("  No matching commands"),
                width,
            )];
        }
        let column = self.primary_width();
        let start = self
            .selected
            .get()
            .saturating_sub(self.max_visible / 2)
            .min(count.saturating_sub(self.max_visible));
        let end = start.saturating_add(self.max_visible).min(count);
        let mut lines = Vec::new();
        for index in start..end {
            let item = self.filtered.borrow().get(index).cloned();
            if let Some(item) = item {
                lines.push(bound_styled(
                    &self.row(&item, index == self.selected.get(), width, column),
                    width,
                ));
            }
        }
        let count = self.filtered.borrow().len();
        if start > 0 || end < count {
            lines.push((self.theme.scroll_info)(&clip(
                &format!("  ({}/{count})", self.selected.get() + 1),
                signed(width) - 2,
            )));
        }
        lines
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
}
impl SelectList {
    /// Matches value prefixes using Unicode lowercase, resetting selection without notification.
    pub fn set_filter(&self, filter: &str) {
        let filter = filter.to_lowercase();
        let filtered = self
            .items
            .iter()
            .filter(|item| item.value.to_lowercase().starts_with(&filter))
            .cloned()
            .collect();
        let old = self.filtered.replace(filtered);
        drop(old);
        self.selected.set(0);
    }
    /// Clamps selection without notification.
    pub fn set_selected_index(&self, index: isize) {
        self.selected.set(
            usize::try_from(index)
                .unwrap_or(0)
                .min(self.filtered.borrow().len().saturating_sub(1)),
        );
    }
    /// Returns the retained original command handle, when selected.
    #[must_use]
    pub fn get_selected_item(&self) -> Option<Rc<SelectItem>> {
        self.filtered.borrow().get(self.selected.get()).cloned()
    }
    /// Returns the retained confirmation callable.
    #[must_use]
    pub fn on_select(&self) -> Option<ItemCallback> {
        self.select.borrow().clone()
    }
    /// Replaces or removes confirmation notification.
    pub fn set_on_select(&self, callback: Option<ItemCallback>) {
        let old = self.select.replace(callback);
        drop(old);
    }
    /// Returns the retained cancellation callable.
    #[must_use]
    pub fn on_cancel(&self) -> Option<Rc<dyn Fn()>> {
        self.cancel.borrow().clone()
    }
    /// Replaces or removes cancellation notification.
    pub fn set_on_cancel(&self, callback: Option<Rc<dyn Fn()>>) {
        let old = self.cancel.replace(callback);
        drop(old);
    }
    /// Returns the retained movement callable.
    #[must_use]
    pub fn on_selection_change(&self) -> Option<ItemCallback> {
        self.change.borrow().clone()
    }
    /// Replaces or removes movement notification.
    pub fn set_on_selection_change(&self, callback: Option<ItemCallback>) {
        let old = self.change.replace(callback);
        drop(old);
    }
}
impl InputHandler for SelectList {
    fn handle_input(&self, data: &str) {
        let bindings = get_keybindings();
        let count = self.filtered.borrow().len();
        let callback = if bindings.matches(data, "tui.select.up") {
            self.selected.set(if self.selected.get() == 0 {
                count.saturating_sub(1)
            } else {
                self.selected.get() - 1
            });
            self.on_selection_change()
        } else if bindings.matches(data, "tui.select.down") {
            self.selected
                .set(if self.selected.get() == count.saturating_sub(1) {
                    0
                } else {
                    self.selected.get() + 1
                });
            self.on_selection_change()
        } else if bindings.matches(data, "tui.select.confirm") {
            self.on_select()
        } else {
            if bindings.matches(data, "tui.select.cancel")
                && let Some(callback) = self.on_cancel()
            {
                callback();
            }
            return;
        };
        if let (Some(item), Some(callback)) = (self.get_selected_item(), callback) {
            callback(item);
        }
    }
}
/// Display fallback without changing command identity.
fn display(item: &SelectItem) -> &str {
    if item.label.is_empty() {
        &item.value
    } else {
        &item.label
    }
}
/// Saturating terminal geometry conversion.
fn signed(width: usize) -> isize {
    isize::try_from(width).unwrap_or(isize::MAX)
}
/// Bounds styled output, retaining fitting bytes at nonzero widths.
fn bound_styled(text: &str, width: usize) -> String {
    if width > 0 && visible_width(text) <= width {
        text.into()
    } else {
        clip(text, signed(width))
    }
}
/// Clips without an ellipsis using [`truncate_to_width`].
fn clip(text: &str, width: isize) -> String {
    truncate_to_width(
        text,
        usize::try_from(width).unwrap_or(0),
        TruncateOptions {
            ellipsis: "",
            ..TruncateOptions::default()
        },
    )
}
/// Collapses newline runs before applying scalar whitespace trimming.
fn normalize(text: &str) -> String {
    let mut out = String::new();
    let mut newline = false;
    for scalar in text.chars() {
        if matches!(scalar, '\r' | '\n') {
            if !newline {
                out.push(' ');
            }
            newline = true;
        } else {
            out.push(scalar);
            newline = false;
        }
    }
    out.trim_matches(is_whitespace_scalar).to_owned()
}
