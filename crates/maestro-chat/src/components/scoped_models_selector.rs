//! Ordered session-only model scope editing with explicit persistence.
use super::{dynamic_border::DynamicBorder, list_window};
use maestro_app::{Model, key_text};
use maestro_theme::{LiveTheme, Theme, ThemeColor, paint};
use maestro_tui::{
    Component, Container, FocusFlag, Focusable, Input, Spacer, Text, fuzzy_filter, get_keybindings,
    matches_key,
    tui::{InputHandler, LocalFuture, TuiRuntime},
};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, PoisonError, RwLock},
};

/// Supplied descriptors and initial cycling order.
pub struct ModelsConfig {
    /// Shared descriptors in available occurrence order.
    pub all_models: Vec<Arc<RwLock<Model>>>,
    /// Explicit ordered identities; absent means every model is enabled.
    pub enabled_model_ids: Option<Vec<String>>,
}
/// Immediate notifications with optional host-owned asynchronous tails.
pub struct ModelsCallbacks {
    /// Reports session-only edits with an independent selection snapshot.
    pub on_change: Rc<dyn Fn(Option<Vec<String>>) -> Option<LocalFuture>>,
    /// Requests saving the current ordered selection.
    pub on_persist: Rc<dyn Fn(Option<Vec<String>>) -> Option<LocalFuture>>,
    /// Reports cancellation without persistence.
    pub on_cancel: Rc<dyn Fn()>,
}
/// A projected occurrence retaining its constructor identity.
#[derive(Clone)]
struct ModelItem {
    /// Stable callback identity.
    full_id: String,
    /// Last supplied descriptor for this identity.
    model: Arc<RwLock<Model>>,
    /// Membership sampled during refresh.
    enabled: bool,
}
/// Tests membership in the all-enabled or explicit representation.
fn is_enabled(enabled: Option<&[String]>, id: &str) -> bool {
    enabled.is_none_or(|ids| ids.iter().any(|value| value == id))
}
/// Composes the enabled prefix followed by unselected available occurrences.
fn get_sorted_ids(enabled: Option<&[String]>, all: &[String]) -> Vec<String> {
    enabled.map_or_else(
        || all.to_vec(),
        |ids| {
            let membership: HashSet<_> = ids.iter().collect();
            ids.iter()
                .chain(all.iter().filter(|id| !membership.contains(id)))
                .cloned()
                .collect()
        },
    )
}
/// Toggles only the first explicit occurrence or starts an explicit scope.
fn toggle(enabled: Option<&[String]>, id: &str) -> Vec<String> {
    let Some(enabled) = enabled else {
        return vec![id.to_owned()];
    };
    let mut ids = enabled.to_vec();
    if let Some(index) = ids.iter().position(|value| value == id) {
        ids.remove(index);
    } else {
        ids.push(id.to_owned());
    }
    ids
}
/// Appends missing targets and collapses only complete identity coverage.
fn enable_all(
    enabled: Option<&[String]>,
    all: &[String],
    targets: Option<&[String]>,
) -> Option<Vec<String>> {
    let mut result = enabled?.to_vec();
    for id in targets.unwrap_or(all) {
        if !result.contains(id) {
            result.push(id.clone());
        }
    }
    (!all.iter().all(|id| result.contains(id))).then_some(result)
}
/// Removes every target occurrence, enumerating survivors from all-enabled.
fn clear_all(
    enabled: Option<&[String]>,
    all: &[String],
    targets: Option<&[String]>,
) -> Vec<String> {
    let source = enabled.unwrap_or(all);
    let targets: HashSet<_> = targets.unwrap_or(source).iter().collect();
    source
        .iter()
        .filter(|id| !targets.contains(id))
        .cloned()
        .collect()
}
/// Swaps the first explicit occurrence with an admitted signed neighbour.
fn r#move(enabled: Option<&[String]>, id: &str, delta: isize) -> Option<Vec<String>> {
    let mut result = enabled?.to_vec();
    if let Some(index) = result.iter().position(|value| value == id)
        && let Some(next) = index
            .checked_add_signed(delta)
            .filter(|next| *next < result.len())
    {
        result.swap(index, next);
    }
    Some(result)
}
/// Retained panel, editable children and independent parent focus.
pub struct ScopedModelsSelectorComponent {
    /// Presentation child owner.
    container: Container,
    /// Descriptors keyed at construction, with last replacement winning.
    models: HashMap<String, Arc<RwLock<Model>>>,
    /// Available identity occurrences.
    all_ids: Vec<String>,
    /// Current explicit order or all-enabled sentinel.
    enabled: RefCell<Option<Vec<String>>>,
    /// Filtered occurrence projections.
    filtered: RefCell<Vec<ModelItem>>,
    /// Current filtered position.
    selected: Cell<usize>,
    /// Separately retained editable Input.
    search: Rc<Input>,
    /// Cached list presentation.
    list: Rc<Container>,
    /// Cached live-key help and count.
    footer: Rc<Text>,
    /// Caller notifications.
    callbacks: ModelsCallbacks,
    /// Published palette sampled at rebuilds.
    theme: LiveTheme,
    /// Host owning returned futures.
    runtime: Rc<dyn TuiRuntime>,
    /// Unsaved edit flag.
    dirty: Cell<bool>,
    /// Independent parent-to-child forwarding.
    focus: FocusFlag,
}
impl ScopedModelsSelectorComponent {
    /// Builds the initial panel without registry, settings or external rendering effects.
    #[must_use]
    pub fn new(
        config: ModelsConfig,
        callbacks: ModelsCallbacks,
        theme: LiveTheme,
        runtime: Rc<dyn TuiRuntime>,
    ) -> Self {
        let mut models = HashMap::new();
        let mut all_ids = Vec::new();
        for model in config.all_models {
            let id = {
                let model = model.read().unwrap_or_else(PoisonError::into_inner);
                format!("{}/{}", model.provider, model.id)
            };
            all_ids.push(id.clone());
            models.insert(id, model);
        }
        let search = Rc::new(Input::new());
        let focus = FocusFlag::forwarding(search.clone());
        let component = Self {
            container: Container::new(),
            models,
            all_ids,
            enabled: RefCell::new(config.enabled_model_ids),
            filtered: RefCell::default(),
            selected: Cell::new(0),
            search,
            list: Rc::new(Container::new()),
            footer: Rc::new(Text::new(String::new(), 0, 0, None)),
            callbacks,
            theme,
            runtime,
            dirty: Cell::new(false),
            focus,
        };
        component.compose();
        component.refresh();
        component
    }
    /// Samples a foreground role without introducing palette values.
    fn paint(&self, color: &ThemeColor, text: &str) -> String {
        paint(self.theme.get().ok().as_deref(), color, text)
    }
    /// Composes captured header and retained children between live borders.
    fn compose(&self) {
        self.container
            .add_child(Rc::new(DynamicBorder::new(self.theme.clone(), None)));
        self.container.add_child(Rc::new(Spacer::new(1)));
        self.container.add_child(Rc::new(Text::new(
            self.paint(
                &ThemeColor::Accent,
                &Theme::bold("Model Configuration", true),
            ),
            0,
            0,
            None,
        )));
        self.container.add_child(Rc::new(Text::new(
            self.paint(
                &ThemeColor::Muted,
                &format!(
                    "Session-only. {} to save to settings.",
                    key_text("app.models.save")
                ),
            ),
            0,
            0,
            None,
        )));
        self.container.add_child(Rc::new(Spacer::new(1)));
        self.container.add_child(self.search.clone());
        self.container.add_child(Rc::new(Spacer::new(1)));
        self.container.add_child(self.list.clone());
        self.container.add_child(Rc::new(Spacer::new(1)));
        self.container.add_child(self.footer.clone());
        self.container
            .add_child(Rc::new(DynamicBorder::new(self.theme.clone(), None)));
    }
    /// Builds occurrences without copying descriptors or losing stale selection entries.
    fn build_items(&self) -> Vec<ModelItem> {
        let enabled = self.enabled.borrow();
        get_sorted_ids(enabled.as_deref(), &self.all_ids)
            .into_iter()
            .filter_map(|full_id| {
                self.models.get(&full_id).map(|model| ModelItem {
                    enabled: is_enabled(enabled.as_deref(), &full_id),
                    full_id,
                    model: model.clone(),
                })
            })
            .collect()
    }
    /// Refilters live descriptor labels and clamps the selected occurrence.
    fn refresh(&self) {
        let query = self.search.get_value();
        let items = self.build_items();
        let filtered = fuzzy_filter(&items, &query, |item| {
            let model = item.model.read().unwrap_or_else(PoisonError::into_inner);
            format!("{} {}", model.id, model.provider)
        })
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
        self.selected
            .set(self.selected.get().min(filtered.len().saturating_sub(1)));
        let old = self.filtered.replace(filtered);
        drop(old);
        self.update_list();
        self.update_footer();
    }
    /// Composes live key hints and explicit occurrence counts.
    fn update_footer(&self) {
        let count = self.enabled.borrow().as_ref().map_or_else(
            || "all enabled".to_owned(),
            |ids| format!("{}/{} enabled", ids.len(), self.all_ids.len()),
        );
        let parts = [
            format!("{} toggle", key_text("tui.select.confirm")),
            format!("{} all", key_text("app.models.enableAll")),
            format!("{} clear", key_text("app.models.clearAll")),
            format!("{} provider", key_text("app.models.toggleProvider")),
            format!(
                "{}/{} reorder",
                key_text("app.models.reorderUp"),
                key_text("app.models.reorderDown")
            ),
            format!("{} save", key_text("app.models.save")),
            count,
        ];
        let text = format!(
            "  {}{}",
            parts.join(" · "),
            if self.dirty.get() { " " } else { "" }
        );
        let mut text = self.paint(&ThemeColor::Dim, &text);
        if self.dirty.get() {
            text.push_str(&self.paint(&ThemeColor::Warning, "(unsaved)"));
        }
        self.footer.set_text(text);
    }
    /// Appends a cached row through the existing text owner.
    fn text(&self, text: String) {
        self.list.add_child(Rc::new(Text::new(text, 0, 0, None)));
    }
    /// Projects one row's live fields before applying presentation.
    fn row(&self, index: usize, item: &ModelItem) -> String {
        let (id, provider) = {
            let model = item.model.read().unwrap_or_else(PoisonError::into_inner);
            (model.id.clone(), model.provider.clone())
        };
        let label = if index == self.selected.get() {
            self.paint(&ThemeColor::Accent, "→ ") + &self.paint(&ThemeColor::Accent, &id)
        } else {
            format!("  {id}")
        };
        let badge = self.paint(&ThemeColor::Muted, &format!(" [{provider}]"));
        let status = if self.enabled.borrow().is_none() {
            String::new()
        } else if item.enabled {
            self.paint(&ThemeColor::Success, " ✓")
        } else {
            self.paint(&ThemeColor::Dim, " ✗")
        };
        label + &badge + &status
    }
    /// Rebuilds only list rows and the selected name.
    fn update_list(&self) {
        self.list.clear();
        let items = self.filtered.borrow();
        let count = items.len();
        if count == 0 {
            self.text(self.paint(&ThemeColor::Muted, "  No matching models"));
            return;
        }
        let window = list_window(self.selected.get(), count, 8);
        for index in window.clone() {
            self.text(self.row(index, &items[index]));
        }
        if window.start > 0 || window.end < count {
            self.text(self.paint(
                &ThemeColor::Muted,
                &format!("  ({}/{count})", self.selected.get() + 1),
            ));
        }
        let name = {
            let model = items[self.selected.get()]
                .model
                .read()
                .unwrap_or_else(PoisonError::into_inner);
            format!("  Model Name: {}", model.name)
        };
        self.list.add_child(Rc::new(Spacer::new(1)));
        self.text(self.paint(&ThemeColor::Muted, &name));
    }
    /// Changes one selection and rebuilds before notifying without retained borrows.
    fn edit(&self, value: Option<Vec<String>>) {
        let old = self.enabled.replace(value);
        drop(old);
        self.dirty.set(true);
        self.refresh();
        let snapshot = self.enabled.borrow().clone();
        if let Some(future) = (self.callbacks.on_change)(snapshot) {
            self.runtime.spawn_local(future);
        }
    }
    /// Wraps navigation while keeping empty results inert.
    fn navigate(&self, up: bool) {
        let count = self.filtered.borrow().len();
        if super::navigate_list(&self.selected, count, up) {
            self.update_list();
        }
    }
    /// Toggles the selected occurrence without calling out under state borrows.
    fn confirm(&self) {
        let value = self
            .filtered
            .borrow()
            .get(self.selected.get())
            .map(|item| toggle(self.enabled.borrow().as_deref(), &item.full_id));
        if let Some(value) = value {
            self.edit(Some(value));
        }
    }
    /// Reorders an enabled occurrence, clamping the filtered cursor at both ends.
    fn reorder(&self, delta: isize) {
        let value = {
            let enabled = self.enabled.borrow();
            let items = self.filtered.borrow();
            let Some((ids, item)) = enabled.as_ref().zip(items.get(self.selected.get())) else {
                return;
            };
            let Some(index) = ids.iter().position(|id| id == &item.full_id) else {
                return;
            };
            if index
                .checked_add_signed(delta)
                .is_none_or(|next| next >= ids.len())
            {
                return;
            }
            r#move(enabled.as_deref(), &item.full_id, delta)
        };
        self.selected
            .set(self.selected.get().saturating_add_signed(delta));
        self.edit(value);
    }
    /// Applies enable or clear to the raw-query-selected target set.
    fn bulk(&self, enable: bool) {
        let targets = (!self.search.get_value().is_empty()).then(|| {
            self.filtered
                .borrow()
                .iter()
                .map(|item| item.full_id.clone())
                .collect::<Vec<_>>()
        });
        let value = {
            let enabled = self.enabled.borrow();
            if enable {
                enable_all(enabled.as_deref(), &self.all_ids, targets.as_deref())
            } else {
                Some(clear_all(
                    enabled.as_deref(),
                    &self.all_ids,
                    targets.as_deref(),
                ))
            }
        };
        self.edit(value);
    }
    /// Toggles the entire live provider inventory, independently of search.
    fn toggle_provider(&self) {
        let provider = self.filtered.borrow().get(self.selected.get()).map(|item| {
            item.model
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .provider
                .clone()
        });
        let Some(provider) = provider else {
            return;
        };
        let targets = self
            .all_ids
            .iter()
            .filter(|id| {
                self.models[*id]
                    .read()
                    .unwrap_or_else(PoisonError::into_inner)
                    .provider
                    == provider
            })
            .cloned()
            .collect::<Vec<_>>();
        let value = {
            let enabled = self.enabled.borrow();
            if targets.iter().all(|id| is_enabled(enabled.as_deref(), id)) {
                Some(clear_all(enabled.as_deref(), &self.all_ids, Some(&targets)))
            } else {
                enable_all(enabled.as_deref(), &self.all_ids, Some(&targets))
            }
        };
        self.edit(value);
    }
    /// Returns the separately retained editable search child, without a submit callback.
    #[must_use]
    pub fn get_search_input(&self) -> &Rc<Input> {
        &self.search
    }
}
component_children!(ScopedModelsSelectorComponent, Container, container);
impl InputHandler for ScopedModelsSelectorComponent {
    fn handle_input(&self, data: &str) {
        let bindings = get_keybindings();
        if bindings.matches(data, "tui.select.up") {
            self.navigate(true);
        } else if bindings.matches(data, "tui.select.down") {
            self.navigate(false);
        } else if bindings.matches(data, "app.models.reorderUp") {
            self.reorder(-1);
        } else if bindings.matches(data, "app.models.reorderDown") {
            self.reorder(1);
        } else if bindings.matches(data, "tui.select.confirm") {
            self.confirm();
        } else if bindings.matches(data, "app.models.enableAll") {
            self.bulk(true);
        } else if bindings.matches(data, "app.models.clearAll") {
            self.bulk(false);
        } else if bindings.matches(data, "app.models.toggleProvider") {
            self.toggle_provider();
        } else if bindings.matches(data, "app.models.save") {
            let snapshot = self.enabled.borrow().clone();
            if let Some(future) = (self.callbacks.on_persist)(snapshot) {
                self.runtime.spawn_local(future);
            }
            self.dirty.set(false);
            self.update_footer();
        } else if matches_key(data, "ctrl+c") {
            if self.search.get_value().is_empty() {
                (self.callbacks.on_cancel)();
            } else {
                self.search.set_value(String::new());
                self.refresh();
            }
        } else if matches_key(data, "escape") {
            (self.callbacks.on_cancel)();
        } else {
            self.search.handle_input(data);
            self.refresh();
        }
    }
}
impl Focusable for ScopedModelsSelectorComponent {
    fn focus_flag(&self) -> &FocusFlag {
        &self.focus
    }
}
impl Component for ScopedModelsSelectorComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.container.render(width)
    }
    fn try_render(&self, width: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        self.container.try_render(width)
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

#[cfg(test)]
mod tests;
