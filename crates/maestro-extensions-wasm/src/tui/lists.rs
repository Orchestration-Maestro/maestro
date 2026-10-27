//! Host selection lists, settings lists and bordered loaders.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use super::widgets::{Hosted, Supplied, Widget, host};
use super::{Component, Style, TUI};
use crate::{AbortSignal, CallbackEmitter, ExtensionResult, Theme};
use std::rc::Rc;

/// One choice of a selection list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectItem {
    /// The value the list filters by and reports.
    pub value: String,
    /// The shown label.
    pub label: String,
    /// The shown description, when there is one.
    pub description: Option<String>,
}

/// What an authored primary-column truncation receives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectListTruncatePrimaryContext {
    /// The primary text.
    pub text: String,
    /// The width the text must fit.
    pub max_width: i64,
    /// The primary column's width.
    pub column_width: i64,
    /// The row's choice.
    pub item: SelectItem,
    /// Whether the row is selected.
    pub is_selected: bool,
}

/// Truncates a row's primary text.
pub type TruncatePrimary = Rc<
    dyn for<'a> Fn(
        &SelectListTruncatePrimaryContext,
        CallbackEmitter<'a>,
    ) -> ExtensionResult<String>,
>;
/// Receives a choice of a selection list.
pub type SelectHandler =
    Rc<dyn for<'a> Fn(&SelectItem, CallbackEmitter<'a>) -> ExtensionResult<()>>;
/// Receives a notice without operands: a cancellation or an abort.
pub type WidgetNotice = Rc<dyn for<'a> Fn(CallbackEmitter<'a>) -> ExtensionResult<()>>;
/// Receives a setting's id and new value.
pub type SettingChange = Rc<dyn for<'a> Fn(&str, &str, CallbackEmitter<'a>) -> ExtensionResult<()>>;
/// Builds a settings submenu from the current value and the operation that closes it.
pub type Submenu =
    Rc<dyn for<'a> Fn(&str, SubmenuDone, CallbackEmitter<'a>) -> ExtensionResult<Component>>;

/// Selection list styles.
#[derive(Clone)]
pub struct SelectListTheme {
    /// The selected row's prefix.
    pub selected_prefix: Style,
    /// The selected row's text.
    pub selected_text: Style,
    /// Descriptions.
    pub description: Style,
    /// The scroll position.
    pub scroll_info: Style,
    /// The no-match notice.
    pub no_match: Style,
}

/// Primary column layout; an absent member keeps the host's default.
#[derive(Clone, Default)]
pub struct SelectListLayoutOptions {
    /// The narrowest primary column.
    pub min_primary_column_width: Option<i64>,
    /// The widest primary column.
    pub max_primary_column_width: Option<i64>,
    /// Replaces the host's primary-text truncation.
    pub truncate_primary: Option<TruncatePrimary>,
}

/// An authored settings row style's body.
type RowStyleFn = dyn Fn(&str, bool) -> ExtensionResult<String>;

/// Styles a settings row's text, knowing whether the row is selected.
///
/// Clones share one function. A style the host supplied goes back to the host unchanged.
#[derive(Clone)]
pub struct RowStyle(pub(crate) Supplied<RowStyleFn>);
impl RowStyle {
    /// Wraps an authored row style.
    #[must_use]
    pub fn new(apply: impl Fn(&str, bool) -> ExtensionResult<String> + 'static) -> Self {
        Self(Supplied::authored(Rc::new(apply)))
    }
    /// Styles `text` of a row that is `selected` or not.
    ///
    /// # Errors
    /// Returns the function's failure.
    pub fn apply(&self, text: &str, selected: bool) -> ExtensionResult<String> {
        (self.0.function)(text, selected)
    }
}

/// Settings list styles.
#[derive(Clone)]
pub struct SettingsListTheme {
    /// Labels.
    pub label: RowStyle,
    /// Values.
    pub value: RowStyle,
    /// Descriptions.
    pub description: Style,
    /// The selected row's prefix.
    pub cursor: String,
    /// The hint row.
    pub hint: Style,
}

/// Settings list options.
#[derive(Clone, Copy, Default)]
pub struct SettingsListOptions {
    /// Shows a search row that filters by label.
    pub enable_search: bool,
}

/// One setting of a settings list.
#[derive(Clone)]
pub struct SettingItem {
    /// The setting's identity.
    pub id: String,
    /// The shown label.
    pub label: String,
    /// The description shown while the row is selected.
    pub description: Option<String>,
    /// The shown current value.
    pub current_value: String,
    /// Values that activation cycles through.
    pub values: Option<Vec<String>>,
    /// A submenu that activation opens.
    pub submenu: Option<Submenu>,
}

/// Bordered loader options.
#[derive(Clone, Copy, Default)]
pub struct BorderedLoaderOptions {
    /// Whether input can cancel the loader; absent means it can.
    pub cancellable: Option<bool>,
}

/// Requests to the host's submenu closer.
pub(crate) trait SubmenuDonePort {
    /// Closes the submenu, reporting `value` as the new value when there is one.
    fn done(&self, value: Option<&str>) -> ExtensionResult<()>;
}

/// Closes an open settings submenu; clones share it.
#[derive(Clone)]
pub struct SubmenuDone(Rc<dyn SubmenuDonePort>);
impl SubmenuDone {
    /// Retains the host's closer.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn new(port: Rc<dyn SubmenuDonePort>) -> Self {
        Self(port)
    }
    /// Closes the submenu; `Some` reports the chosen value as the setting's new value.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn done(&self, value: Option<&str>) -> ExtensionResult<()> {
        self.0.done(value)
    }
}

facade! {
    /// A list of choices with filtering, wrapping selection and confirm or cancel handlers.
    SelectList
}
impl SelectList {
    /// Builds the list.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn new(
        items: &[SelectItem],
        max_visible: u32,
        theme: &SelectListTheme,
        layout: &SelectListLayoutOptions,
    ) -> ExtensionResult<Self> {
        host()?
            .select_list(items, max_visible, theme, layout)
            .map(Self)
    }
    /// Keeps the choices whose value starts with `filter`, ignoring case, and selects the first.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn set_filter(&self, filter: &str) -> ExtensionResult<()> {
        self.0.port.set_filter(filter)
    }
    /// Selects the row at `index`, clamped to the rows.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn set_selected_index(&self, index: i64) -> ExtensionResult<()> {
        self.0.port.set_selected_index(index)
    }
    /// The selected choice, absent when no choice matches.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn get_selected_item(&self) -> ExtensionResult<Option<SelectItem>> {
        self.0.port.get_selected_item()
    }
    /// Replaces or removes the confirmation handler.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn set_on_select(&self, handler: Option<SelectHandler>) -> ExtensionResult<()> {
        self.0.port.set_on_select(handler)
    }
    /// Replaces or removes the cancellation handler.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn set_on_cancel(&self, handler: Option<WidgetNotice>) -> ExtensionResult<()> {
        self.0.port.set_on_cancel(handler)
    }
    /// Replaces or removes the handler of selection movement.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn set_on_selection_change(&self, handler: Option<SelectHandler>) -> ExtensionResult<()> {
        self.0.port.set_on_selection_change(handler)
    }
    /// Delivers input; the list's handlers may run first.
    ///
    /// # Errors
    /// Returns the host's or a handler's failure.
    pub fn handle_input(&self, data: &str) -> ExtensionResult<()> {
        self.0.port.handle_input(data)
    }
}

facade! {
    /// Settings rows with cycling values, submenus and optional search.
    SettingsList
}
impl SettingsList {
    /// Builds the list, reporting each change and the cancellation.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn new(
        items: Vec<SettingItem>,
        max_visible: u32,
        theme: &SettingsListTheme,
        (on_change, on_cancel): (SettingChange, WidgetNotice),
        options: SettingsListOptions,
    ) -> ExtensionResult<Self> {
        host()?
            .settings_list((items, max_visible), theme, (on_change, on_cancel), options)
            .map(Self)
    }
    /// Replaces the current value of the setting `id` without reporting a change.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn update_value(&self, id: &str, value: &str) -> ExtensionResult<()> {
        self.0.port.update_value(id, value)
    }
    /// Delivers input; the list's callbacks may run first.
    ///
    /// # Errors
    /// Returns the host's or a callback's failure.
    pub fn handle_input(&self, data: &str) -> ExtensionResult<()> {
        self.0.port.handle_input(data)
    }
}

facade! {
    /// A loader between borders, cancellable by input unless built otherwise.
    BorderedLoader
}
children!(BorderedLoader);
impl BorderedLoader {
    /// Builds the loader on a lent terminal, colored by `theme`.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn new(
        tui: &TUI,
        theme: &Theme,
        message: &str,
        options: Option<BorderedLoaderOptions>,
    ) -> ExtensionResult<Self> {
        let cancellable = options.and_then(|options| options.cancellable);
        host()?
            .bordered_loader(tui, theme, message, cancellable)
            .map(Self)
    }
    /// The signal input cancels; a loader that cannot be cancelled keeps its own.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn signal(&self) -> ExtensionResult<AbortSignal> {
        self.0.port.signal()
    }
    /// Replaces or removes the handler a cancellation runs.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn set_on_abort(&self, handler: Option<WidgetNotice>) -> ExtensionResult<()> {
        self.0.port.set_on_abort(handler)
    }
    /// Delivers input; a cancellation runs its handler first.
    ///
    /// # Errors
    /// Returns the host's or the handler's failure.
    pub fn handle_input(&self, data: &str) -> ExtensionResult<()> {
        self.0.port.handle_input(data)
    }
    /// Stops the animation without cancelling the signal.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn dispose(&self) -> ExtensionResult<()> {
        self.0.port.dispose()
    }
}

/// The live theme's selection list styles.
///
/// # Errors
/// Returns the host's failure.
pub fn get_select_list_theme() -> ExtensionResult<SelectListTheme> {
    host()?.select_list_theme()
}

/// The live theme's settings list styles.
///
/// # Errors
/// Returns the host's failure.
pub fn get_settings_list_theme() -> ExtensionResult<SettingsListTheme> {
    host()?.settings_list_theme()
}
