//! Searchable available-model selection with supplied scoped ordering.
#[cfg(test)]
mod tests;
use super::dynamic_border::DynamicBorder;
use maestro_app::{
    CatalogError, Model, ModelRegistry, ScopedModel, SettingsManager, key_hint,
    model_id_comparator, models_are_equal,
};
use maestro_theme::{LiveTheme, ThemeColor, paint};
use maestro_tui::{
    Component, Container, Input, Spacer, TUI, Text, fuzzy_filter, get_keybindings,
    tui::{FocusFlag, Focusable, InputHandler},
};
use std::{
    cell::{Cell, RefCell},
    error::Error,
    rc::Rc,
    sync::{Arc, PoisonError, RwLock},
};

/// Initial model view and published presentation.
pub struct ModelSelectorOptions {
    /// Current model identified by provider and ID.
    pub current_model: Option<Arc<RwLock<Model>>>,
    /// Supplied scoped order, including missing models and duplicates.
    pub scoped_models: Vec<ScopedModel>,
    /// Search applied after loading.
    pub initial_search_input: Option<String>,
    /// Published colors sampled when children are updated.
    pub theme: LiveTheme,
}
/// Synchronous selection and cancellation notifications.
pub struct ModelSelectorCallbacks {
    /// Receives the selected shared descriptor after defaults are updated.
    pub on_select: Rc<dyn Fn(Arc<RwLock<Model>>)>,
    /// Receives cancellation without changing defaults.
    pub on_cancel: Rc<dyn Fn()>,
}
/// Captured search labels with live descriptor metadata.
struct ModelItem {
    /// Load-time provider.
    provider: String,
    /// Load-time ID.
    id: String,
    /// Original descriptor handle.
    model: Arc<RwLock<Model>>,
}
impl ModelItem {
    /// Captures labels without copying the descriptor.
    fn new(model: Arc<RwLock<Model>>) -> Rc<Self> {
        let (provider, id) = {
            let m = model.read().unwrap_or_else(PoisonError::into_inner);
            (m.provider.clone(), m.id.clone())
        };
        Rc::new(Self {
            provider,
            id,
            model,
        })
    }
    /// Recognizes shared handles before reading independent descriptors' live identities.
    fn is_current(&self, current: Option<&Arc<RwLock<Model>>>) -> bool {
        if current.is_some_and(|current| Arc::ptr_eq(current, &self.model)) {
            return true;
        }
        let current = current.map(|m| m.read().unwrap_or_else(PoisonError::into_inner));
        let model = self.model.read().unwrap_or_else(PoisonError::into_inner);
        models_are_equal(current.as_deref(), Some(&model))
    }
}
/// State retained by Input submission, without owning Input or outer chrome.
struct SelectionState {
    /// Current descriptor for identity comparisons.
    current: Option<Arc<RwLock<Model>>>,
    /// Stable sorted all view.
    all: Vec<Rc<ModelItem>>,
    /// Supplied scoped view.
    scoped: Vec<Rc<ModelItem>>,
    /// Whether scoped is active.
    scoped_active: Cell<bool>,
    /// Filtered item handles in display order.
    filtered: RefCell<Vec<Rc<ModelItem>>>,
    /// Selected filtered position.
    selected: Cell<usize>,
    /// Caller settings owner.
    settings: Rc<RefCell<SettingsManager>>,
    /// Caller notifications.
    callbacks: ModelSelectorCallbacks,
    /// Published presentation.
    theme: LiveTheme,
    /// Diagnostic from registry loading.
    error: Option<String>,
    /// Composed cached list children.
    list: Rc<Container>,
}
impl SelectionState {
    /// Loads available and supplied scoped records, retaining shared descriptors.
    fn load_models(
        &mut self,
        registry: &ModelRegistry,
        scoped_models: Vec<ScopedModel>,
        compare: &dyn Fn(&str, &str) -> std::cmp::Ordering,
    ) -> Result<(), CatalogError> {
        self.all = registry
            .get_available()?
            .into_iter()
            .map(ModelItem::new)
            .collect();
        let current = self.current.as_ref();
        self.all.sort_by(|a, b| {
            b.is_current(current)
                .cmp(&a.is_current(current))
                .then_with(|| compare(&a.provider, &b.provider))
        });
        self.scoped = scoped_models
            .into_iter()
            .map(|scoped| {
                let refreshed = {
                    let m = scoped.model.read().unwrap_or_else(PoisonError::into_inner);
                    registry.find(&m.provider, &m.id)
                };
                ModelItem::new(refreshed.unwrap_or(scoped.model))
            })
            .collect();
        Ok(())
    }
    /// Samples an existing foreground role.
    fn paint(&self, key: &ThemeColor, text: &str) -> String {
        paint(self.theme.get().ok().as_deref(), key, text)
    }
    /// Currently active ordered records.
    fn active(&self) -> &[Rc<ModelItem>] {
        if self.scoped_active.get() {
            &self.scoped
        } else {
            &self.all
        }
    }
    /// Positions at the first current item, otherwise zero.
    fn position_current(&self) {
        self.selected.set(
            self.active()
                .iter()
                .position(|item| item.is_current(self.current.as_ref()))
                .unwrap_or(0),
        );
    }
    /// Filters captured labels and clamps the retained selection.
    fn filter(&self, query: &str) {
        let filtered = fuzzy_filter(self.active(), query, |item| {
            format!(
                "{} {} {}/{} {} {}",
                item.id, item.provider, item.provider, item.id, item.provider, item.id
            )
        })
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
        self.selected
            .set(self.selected.get().min(filtered.len().saturating_sub(1)));
        let old = self.filtered.replace(filtered);
        drop(old);
        self.update_list();
    }
    /// Saves live descriptor fields before calling out without guards.
    fn confirm(&self) {
        let model = self
            .filtered
            .borrow()
            .get(self.selected.get())
            .map(|item| item.model.clone());
        if let Some(model) = model {
            let (provider, id) = {
                let m = model.read().unwrap_or_else(PoisonError::into_inner);
                (m.provider.clone(), m.id.clone())
            };
            self.settings
                .borrow_mut()
                .set_default_model_and_provider(provider, id);
            (self.callbacks.on_select)(model);
        }
    }
    /// Composes scope text at its owning update point.
    fn scope_text(&self) -> String {
        let (all, scoped) = if self.scoped_active.get() {
            (ThemeColor::Muted, ThemeColor::Accent)
        } else {
            (ThemeColor::Accent, ThemeColor::Muted)
        };
        self.paint(&ThemeColor::Muted, "Scope: ")
            + &self.paint(&all, "all")
            + &self.paint(&ThemeColor::Muted, " | ")
            + &self.paint(&scoped, "scoped")
    }
    /// Formats current configured scope keys.
    fn hint(&self) -> Result<String, maestro_theme::ThemeError> {
        Ok(key_hint("tui.input.tab", "scope", &self.theme)?
            + &self.paint(&ThemeColor::Muted, " (all/scoped)"))
    }
    /// Appends cached Text through the common owner.
    fn text(&self, text: String) {
        self.list.add_child(Rc::new(Text::new(text, 0, 0, None)));
    }
    /// Formats one row from captured labels and live current identity.
    fn row(&self, index: usize, item: &ModelItem) -> String {
        let label = if index == self.selected.get() {
            self.paint(&ThemeColor::Accent, "→ ") + &self.paint(&ThemeColor::Accent, &item.id)
        } else {
            format!("  {}", item.id)
        };
        let badge = self.paint(&ThemeColor::Muted, &format!("[{}]", item.provider));
        let check = if item.is_current(self.current.as_ref()) {
            self.paint(&ThemeColor::Success, " ✓")
        } else {
            String::new()
        };
        format!("{label} {badge}{check}")
    }
    /// Replaces cached rows and the diagnostic or selected-name footer.
    fn update_list(&self) {
        self.list.clear();
        let filtered = self.filtered.borrow();
        let count = filtered.len();
        let window = super::list_window(self.selected.get(), count, 10);
        let (start, end) = (window.start, window.end);
        for (index, item) in filtered.iter().enumerate().take(end).skip(start) {
            self.text(self.row(index, item));
        }
        if start > 0 || end < count {
            self.text(self.paint(
                &ThemeColor::Muted,
                &format!("  ({}/{count})", self.selected.get() + 1),
            ));
        }
        if let Some(error) = self.error.as_deref().filter(|error| !error.is_empty()) {
            for line in error.split('\n') {
                self.text(self.paint(&ThemeColor::Error, line));
            }
        } else if let Some(item) = filtered.get(self.selected.get()) {
            let name = {
                let model = item.model.read().unwrap_or_else(PoisonError::into_inner);
                format!("  Model Name: {}", model.name)
            };
            self.list.add_child(Rc::new(Spacer::new(1)));
            self.text(self.paint(&ThemeColor::Muted, &name));
        } else {
            self.text(self.paint(&ThemeColor::Muted, "  No matching models"));
        }
    }
    /// Wraps one navigation action, leaving empty views untouched.
    fn navigate(&self, up: bool) {
        super::navigate_list(&self.selected, &self.filtered, up, || self.update_list());
    }
}
/// Model search panel with retained Input and editable Container children.
pub struct ModelSelectorComponent {
    /// Outer child-array owner.
    container: Container,
    /// Input retained separately from selection state.
    search: Rc<Input>,
    /// Shared selection and cached list owner.
    state: Rc<SelectionState>,
    /// Independent flag forwarding synchronous changes to Input.
    focus: FocusFlag,
    /// Cached scope text when scopes exist.
    scope_text: Option<Rc<Text>>,
    /// Cached hint updated only on a successful scope toggle.
    scope_hint: Option<Rc<Text>>,
}
impl ModelSelectorComponent {
    /// Refreshes the registry and builds the initial model view.
    ///
    /// # Errors
    /// Propagates refresh, locale and initial key-hint failures; availability errors become notices.
    pub fn new(
        tui: &TUI,
        settings_manager: Rc<RefCell<SettingsManager>>,
        model_registry: &ModelRegistry,
        callbacks: ModelSelectorCallbacks,
        options: ModelSelectorOptions,
    ) -> Result<Self, Box<dyn Error>> {
        model_registry.refresh()?;
        let compare = model_id_comparator(&|key| std::env::var(key).ok())?;
        let mut state = SelectionState {
            current: options.current_model,
            all: Vec::new(),
            scoped: Vec::new(),
            scoped_active: Cell::new(!options.scoped_models.is_empty()),
            filtered: RefCell::default(),
            selected: Cell::new(0),
            settings: settings_manager,
            callbacks,
            theme: options.theme,
            error: model_registry.get_error(),
            list: Rc::new(Container::new()),
        };
        if let Err(error) = state.load_models(model_registry, options.scoped_models, &compare) {
            state.error = Some(error.to_string());
            state.all.clear();
            state.scoped.clear();
        }
        state.position_current();
        let state = Rc::new(state);
        let search = Rc::new(Input::new());
        if let Some(initial) = options.initial_search_input {
            search.set_value(initial);
        }
        let submit = state.clone();
        search.set_on_submit(Some(Rc::new(move |_| submit.confirm())));
        state.filter(&search.get_value());
        let component = Self::compose(state, search)?;
        tui.request_render(false);
        Ok(component)
    }
    /// Composes cached help and live border children around the retained owners.
    fn compose(state: Rc<SelectionState>, search: Rc<Input>) -> Result<Self, Box<dyn Error>> {
        let container = Container::new();
        container.add_child(Rc::new(DynamicBorder::new(state.theme.clone(), None)));
        container.add_child(Rc::new(Spacer::new(1)));
        let (scope_text, scope_hint) = if state.scoped_active.get() {
            let text = Rc::new(Text::new(state.scope_text(), 0, 0, None));
            let hint = Rc::new(Text::new(state.hint()?, 0, 0, None));
            container.add_child(text.clone());
            container.add_child(hint.clone());
            (Some(text), Some(hint))
        } else {
            container.add_child(Rc::new(Text::new(
                state.paint(
                    &ThemeColor::Warning,
                    "Only showing models from configured providers. Use /login to add providers.",
                ),
                0,
                0,
                None,
            )));
            (None, None)
        };
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(search.clone());
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(state.list.clone());
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(Rc::new(DynamicBorder::new(state.theme.clone(), None)));
        let focus = FocusFlag::forwarding(search.clone());
        Ok(Self {
            container,
            search,
            state,
            focus,
            scope_text,
            scope_hint,
        })
    }
    /// Returns the editable Input whose submission retains selection capability.
    #[must_use]
    pub fn get_search_input(&self) -> &Rc<Input> {
        &self.search
    }
    /// Switches active scope, preserving query and refreshing only owned help.
    fn toggle_scope(&self) {
        if self.state.scoped.is_empty() {
            return;
        }
        self.state
            .scoped_active
            .set(!self.state.scoped_active.get());
        self.state.position_current();
        self.state.filter(&self.search.get_value());
        if let Some(text) = &self.scope_text {
            text.set_text(self.state.scope_text());
        }
        if let (Some(text), Ok(hint)) = (&self.scope_hint, self.state.hint()) {
            text.set_text(hint);
        }
    }
}
component_children!(ModelSelectorComponent, Container, container);
impl InputHandler for ModelSelectorComponent {
    fn handle_input(&self, data: &str) {
        let bindings = get_keybindings();
        if bindings.matches(data, "tui.input.tab") {
            self.toggle_scope();
        } else if bindings.matches(data, "tui.select.up") {
            self.state.navigate(true);
        } else if bindings.matches(data, "tui.select.down") {
            self.state.navigate(false);
        } else if bindings.matches(data, "tui.select.confirm") {
            self.state.confirm();
        } else if bindings.matches(data, "tui.select.cancel") {
            (self.state.callbacks.on_cancel)();
        } else {
            self.search.handle_input(data);
            self.state.filter(&self.search.get_value());
        }
    }
}
impl Focusable for ModelSelectorComponent {
    fn focus_flag(&self) -> &FocusFlag {
        &self.focus
    }
}
impl Component for ModelSelectorComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.container.render(width)
    }
    fn invalidate(&self) {
        self.container.invalidate();
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
}
