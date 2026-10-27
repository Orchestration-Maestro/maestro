//! The model picker dialog: search, capability filters and keyboard or pointer selection.
use super::{
    rows::Rows,
    search::{ModelEntry, ModelFilter, filter_models, model_entries},
};
use crate::{
    browser::{
        attachment_content::browser::{append, element, host_error},
        discovery::discover,
        event_subscription::EventSubscription,
    },
    components::{
        button::{ButtonProps, ButtonSize, ButtonVariant, button},
        input::{Input, InputProps},
    },
    dialogs::{
        dialog::{DialogHeaderProps, dialog_header},
        dialog_base::DialogBase,
    },
    dom::{DomView, icon, report},
    tools::types::ToolUiError,
};
use maestro_app::{CustomProviderType, Model, Presence, get_app_storage};
use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::{Rc, Weak},
};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::{Event, HtmlButtonElement, KeyboardEvent, Node};

/// Panel width of the picker.
const WIDTH: &str = "min(400px, 90vw)";
/// Native listener retained with the picker.
type Listener = EventSubscription<Closure<dyn Fn(Event)>>;
/// An open model picker. Clones share one picker.
#[derive(Clone)]
pub struct ModelSelector {
    /// Shared picker state.
    state: Rc<Selector>,
}
/// Picker operands, interaction state and retained nodes.
struct Selector {
    /// Page operands.
    view: Rc<DomView>,
    /// The shown dialog.
    base: DialogBase,
    /// Model marked as current and listed first without search text.
    current_model: Option<Model>,
    /// Receives the chosen model before the picker closes.
    on_select: Rc<dyn Fn(Model)>,
    /// Providers whose models may be listed.
    allowed_providers: Option<HashSet<String>>,
    /// Search text as typed.
    search_query: RefCell<String>,
    /// Reasoning filter.
    filter_thinking: Cell<bool>,
    /// Image-input filter.
    filter_vision: Cell<bool>,
    /// Highlighted row.
    selected_index: Cell<usize>,
    /// Whether the keyboard, rather than the pointer, moves the highlight.
    keyboard: Cell<bool>,
    /// Last pointer position, so only real movement returns to pointer navigation.
    last_mouse_position: Cell<(f64, f64)>,
    /// Models of the configured custom providers.
    custom_provider_models: RefCell<Vec<Model>>,
    /// Rows currently shown.
    filtered: RefCell<Vec<ModelEntry>>,
    /// Search field.
    search: Input,
    /// Thinking and vision filter controls.
    filters: [HtmlButtonElement; 2],
    /// Scrollable rows.
    rows: RefCell<Rows>,
    /// Native listeners.
    listeners: RefCell<Vec<Listener>>,
}
impl ModelSelector {
    /// Open the picker and start loading custom provider models.
    ///
    /// `on_select` receives the model chosen by click or Enter; the picker then closes.
    /// `allowed_providers` limits the listed models to those providers.
    /// # Errors
    /// Returns a localization, icon or native failure; the picker then stays closed.
    pub fn open(
        view: &Rc<DomView>,
        current_model: Option<Model>,
        on_select: Rc<dyn Fn(Model)>,
        allowed_providers: Option<Vec<String>>,
    ) -> Result<Self, ToolUiError> {
        let document = &view.document;
        let mut filters = Vec::new();
        for _ in 0..2 {
            let control =
                button(document, &ButtonProps::default(), &[]).map_err(|e| host_error(&e))?;
            filters.push(control);
        }
        let filters: [HtmlButtonElement; 2] = filters
            .try_into()
            .map_err(|_| ToolUiError::Presentation("Missing filter control".into()))?;
        let list = element(document, "div", "maestro-model-list")?;
        let state = Rc::new(Selector {
            view: Rc::clone(view),
            base: DialogBase::new(
                Rc::clone(view),
                WIDTH,
                crate::dialogs::dialog_base::MODAL_HEIGHT,
            ),
            current_model,
            on_select,
            allowed_providers: allowed_providers.map(|providers| providers.into_iter().collect()),
            search_query: RefCell::default(),
            filter_thinking: Cell::new(false),
            filter_vision: Cell::new(false),
            selected_index: Cell::new(0),
            keyboard: Cell::new(false),
            last_mouse_position: Cell::new((0.0, 0.0)),
            custom_provider_models: RefCell::default(),
            filtered: RefCell::default(),
            search: Input::new(view, InputProps::default())?,
            filters,
            rows: RefCell::new(Rows::new(list)),
            listeners: RefCell::default(),
        });
        let content = state.content()?;
        state.render()?;
        let component: Rc<dyn Any> = Rc::clone(&state) as Rc<dyn Any>;
        state.base.open(content, Some(component))?;
        state.bind()?;
        let _ = state.search.input().focus();
        state.load_custom_providers();
        Ok(Self { state })
    }
    /// Close the picker without choosing a model.
    pub fn close(&self) {
        self.state.base.close();
    }
    /// Whether the picker is shown.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.state.base.is_open()
    }
}
impl Selector {
    /// Header with search and filters, then the rows.
    fn content(self: &Rc<Self>) -> Result<Vec<Node>, ToolUiError> {
        let document = &self.view.document;
        let i18n = &self.view.i18n;
        let head = element(document, "div", "maestro-model-head")?;
        let header = dialog_header(
            document,
            &DialogHeaderProps {
                title: i18n.text("Select Model")?,
                ..DialogHeaderProps::default()
            },
        )?;
        append(&head, &header)?;
        let weak = Rc::downgrade(self);
        self.search.update(
            &self.view,
            InputProps {
                placeholder: i18n.text("Search models...")?,
                on_input: Some(Rc::new(move |_| {
                    if let Some(selector) = weak.upgrade() {
                        let typed = selector.search.input().value();
                        selector.search_query.replace(typed);
                        selector.restart();
                    }
                })),
                ..InputProps::default()
            },
        )?;
        append(&head, self.search.root())?;
        let toggles = element(document, "div", "maestro-model-filters")?;
        for (control, (glyph, label)) in self
            .filters
            .iter()
            .zip([("Brain", "Thinking"), ("Image", "Vision")])
        {
            let text = element(document, "span", "maestro-model-filter-label")?;
            append(&text, &icon(document, glyph, "")?)?;
            let caption = document.create_text_node(&format!(" {}", i18n.text(label)?));
            text.append_child(&caption).map_err(|e| host_error(&e))?;
            append(control, &text)?;
            append(&toggles, control)?;
        }
        append(&head, &toggles)?;
        Ok(vec![head.into(), self.rows.borrow().list().clone().into()])
    }
    /// Register the filter, list, keyboard and pointer listeners, holding the picker weakly.
    fn bind(self: &Rc<Self>) -> Result<(), ToolUiError> {
        let host: web_sys::EventTarget = self
            .base
            .panel()
            .and_then(|panel| panel.parent_element())
            .ok_or_else(|| ToolUiError::Presentation("Model picker is not mounted".into()))?
            .into();
        let list: web_sys::EventTarget = self.rows.borrow().list().clone().into();
        let mut bound = Vec::new();
        for (index, control) in self.filters.iter().enumerate() {
            bound.push((
                control.clone().into(),
                "click",
                self.handler(move |selector, _| selector.toggle(index)),
            ));
        }
        bound.push((
            list.clone(),
            "click",
            self.handler(|selector, event| selector.clicked(&event)),
        ));
        bound.push((
            list,
            "mouseover",
            self.handler(|selector, event| selector.hovered(&event)),
        ));
        bound.push((
            host.clone(),
            "mousemove",
            self.handler(|selector, event| selector.moved(&event)),
        ));
        bound.push((
            host,
            "keydown",
            self.handler(|selector, event| selector.key(&event)),
        ));
        let mut listeners = self.listeners.borrow_mut();
        for (target, name, callback) in bound {
            listeners
                .push(EventSubscription::new(target, name, callback).map_err(|e| host_error(&e))?);
        }
        Ok(())
    }
    /// A native callback reaching this picker while it exists.
    fn handler(
        self: &Rc<Self>,
        action: impl Fn(&Rc<Self>, Event) + 'static,
    ) -> Closure<dyn Fn(Event)> {
        let weak: Weak<Self> = Rc::downgrade(self);
        Closure::new(move |event: Event| {
            if let Some(selector) = weak.upgrade() {
                action(&selector, event);
            }
        })
    }
    /// Switch the thinking (0) or vision (1) filter.
    fn toggle(&self, index: usize) {
        let filter = if index == 0 {
            &self.filter_thinking
        } else {
            &self.filter_vision
        };
        filter.set(!filter.get());
        self.restart();
    }
    /// Return to the first row at the top of the list after the search or a filter changed.
    fn restart(&self) {
        self.selected_index.set(0);
        self.rows.borrow().list().set_scroll_top(0);
        report(self.render());
    }
    /// The rows for the current search, filters and custom provider models.
    fn filtered_models(&self) -> Vec<ModelEntry> {
        let query = self.search_query.borrow();
        let filter = ModelFilter {
            search_query: &query,
            filter_thinking: self.filter_thinking.get(),
            filter_vision: self.filter_vision.get(),
            current_model: self.current_model.as_ref(),
            allowed_providers: self.allowed_providers.as_ref(),
        };
        let entries = model_entries(&self.custom_provider_models.borrow());
        filter_models(entries, &filter, &|a, b| {
            js_sys::JsString::from(a)
                .locale_compare(b, &js_sys::Array::new(), &js_sys::Object::new())
                .cmp(&0)
        })
    }
    /// Show the filter states, the navigation mode and the rows.
    fn render(&self) -> Result<(), ToolUiError> {
        for (control, active) in self
            .filters
            .iter()
            .zip([self.filter_thinking.get(), self.filter_vision.get()])
        {
            let variant = if active {
                ButtonVariant::Default
            } else {
                ButtonVariant::Secondary
            };
            let props = ButtonProps {
                variant,
                size: ButtonSize::Sm,
                class_name: "maestro-pill",
                ..ButtonProps::default()
            };
            control.set_class_name(&props.classes());
            control
                .set_attribute("aria-pressed", if active { "true" } else { "false" })
                .map_err(|e| host_error(&e))?;
        }
        let entries = self.filtered_models();
        let mut rows = self.rows.borrow_mut();
        let mode = if self.keyboard.get() {
            "maestro-model-list"
        } else {
            "maestro-model-list maestro-model-pointer"
        };
        if rows.list().class_name() != mode {
            rows.list().set_class_name(mode);
        }
        rows.show(
            &self.view,
            &entries,
            self.current_model.as_ref(),
            self.selected_index.get(),
        )?;
        drop(rows);
        self.filtered.replace(entries);
        Ok(())
    }
    /// Choose `model`, then close.
    fn handle_select(&self, model: Model) {
        let on_select = Rc::clone(&self.on_select);
        on_select(model);
        self.base.close();
    }
    /// Choose the clicked row.
    fn clicked(&self, event: &Event) {
        let index = event_row(event).and_then(|row| self.rows.borrow().position(&row));
        let model = index.and_then(|index| {
            self.filtered
                .borrow()
                .get(index)
                .map(|entry| entry.model.clone())
        });
        if let Some(model) = model {
            self.handle_select(model);
        }
    }
    /// Highlight the row under the pointer while the pointer navigates.
    fn hovered(&self, event: &Event) {
        if self.keyboard.get() {
            return;
        }
        let index = event_row(event).and_then(|row| self.rows.borrow().position(&row));
        if let Some(index) = index.filter(|index| *index != self.selected_index.get()) {
            self.selected_index.set(index);
            report(self.render());
        }
    }
    /// Return to pointer navigation on real pointer movement, highlighting the row under it.
    fn moved(&self, event: &Event) {
        let position = (number(event, "clientX"), number(event, "clientY"));
        if position == self.last_mouse_position.get() {
            return;
        }
        self.last_mouse_position.set(position);
        if self.keyboard.replace(false) {
            if let Some(index) = event_row(event).and_then(|row| self.rows.borrow().position(&row))
            {
                self.selected_index.set(index);
            }
            report(self.render());
        }
    }
    /// Move the highlight with the arrows and choose with Enter, outside text composition.
    fn key(&self, event: &Event) {
        let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
            return;
        };
        if key.is_composing() || key.key() == "Process" {
            return;
        }
        let count = self.filtered.borrow().len();
        let current = self.selected_index.get();
        let next = match key.key().as_str() {
            "ArrowDown" => current.saturating_add(1).min(count.saturating_sub(1)),
            "ArrowUp" => current.saturating_sub(1),
            "Enter" => {
                key.prevent_default();
                let model = self
                    .filtered
                    .borrow()
                    .get(current)
                    .map(|entry| entry.model.clone());
                if let Some(model) = model {
                    self.handle_select(model);
                }
                return;
            }
            _ => return,
        };
        key.prevent_default();
        self.keyboard.set(true);
        self.selected_index.set(next);
        report(self.render());
        self.scroll_to_selected();
    }
    /// Bring the highlighted row into view.
    fn scroll_to_selected(&self) {
        let rows = self.rows.borrow();
        let Some(row) = rows.row(self.selected_index.get()) else {
            return;
        };
        let options = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&options, &"block".into(), &"nearest".into());
        let _ = js_sys::Reflect::set(&options, &"behavior".into(), &"smooth".into());
        let scroll = js_sys::Reflect::get(row, &"scrollIntoView".into())
            .and_then(wasm_bindgen::JsCast::dyn_into::<js_sys::Function>)
            .and_then(|scroll| scroll.call1(row, &options));
        if let Err(error) = scroll {
            web_sys::console::error_1(&error);
        }
    }
    /// Load the models of every configured custom provider, then show them.
    fn load_custom_providers(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        wasm_bindgen_futures::spawn_local(async move {
            let models = custom_models().await;
            if let Some(selector) = weak.upgrade() {
                selector.custom_provider_models.replace(models);
                report(selector.render());
            }
        });
    }
}
/// The row element containing an event's target.
fn event_row(event: &Event) -> Option<Node> {
    let target: web_sys::Element = event.target()?.dyn_into().ok()?;
    target
        .closest("[data-model-item]")
        .ok()
        .flatten()
        .map(Into::into)
}
/// A numeric event property.
fn number(event: &Event, name: &str) -> f64 {
    js_sys::Reflect::get(event, &JsValue::from_str(name))
        .ok()
        .and_then(|value| value.as_f64())
        .unwrap_or_default()
}
/// Discovered models of local servers, under their provider names, and stored manual models.
///
/// A server that cannot be reached contributes nothing; failures are logged.
async fn custom_models() -> Vec<Model> {
    let providers = match get_app_storage() {
        Ok(storage) => storage.custom_providers.get_all().await,
        Err(error) => Err(error),
    };
    let providers = match providers {
        Ok(providers) => providers,
        Err(error) => {
            web_sys::console::error_2(
                &"Failed to load custom providers:".into(),
                &error.to_string().into(),
            );
            return Vec::new();
        }
    };
    let mut models = Vec::new();
    for provider in providers {
        match provider.r#type {
            CustomProviderType::AutoDiscovery(kind) => {
                let key = match &provider.api_key {
                    Presence::Present(key) => Some(key.as_str()),
                    Presence::Missing | Presence::Null => None,
                };
                match discover(kind, &provider.base_url, key).await {
                    Ok(found) => models.extend(found.into_iter().map(|mut model| {
                        model.provider.clone_from(&provider.name);
                        model
                    })),
                    Err(error) => web_sys::console::debug_2(
                        &format!("Failed to load models from {}:", provider.name).into(),
                        &error.to_string().into(),
                    ),
                }
            }
            CustomProviderType::Manual(_) => {
                if let Presence::Present(manual) = provider.models {
                    models.extend(manual);
                }
            }
        }
    }
    models
}
