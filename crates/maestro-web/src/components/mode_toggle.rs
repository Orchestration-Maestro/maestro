//! Shared native mode buttons and the authored-label page element.
use crate::{
    browser::event_subscription::EventSubscription,
    dom::{self, DomView, Getter, Link, Setter},
    icons,
    tools::types::ToolUiError,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::{CustomEvent, CustomEventInit, HtmlElement};

/// One authored label, optionally represented by an icon instead of visible text.
pub(crate) struct Mode {
    /// Literal label, also used as the button title.
    pub label: String,
    /// Existing icon name for icon-only controls.
    pub icon: Option<&'static str>,
}
/// Current operands of the shared button owner.
pub(crate) struct Options {
    /// Labels in authored order.
    pub modes: Vec<Mode>,
    /// Read the controller currently receiving button selections.
    pub read: Rc<dyn Fn() -> f64>,
    /// Select before dispatching the changed-selection event.
    pub write: Rc<dyn Fn(f64)>,
    /// Caller-specific detail of the shared bubbling event.
    pub detail: fn(u32, &str) -> JsValue,
}
/// Mounted buttons, with subscriptions released alongside their root.
pub(crate) struct ModeGroup {
    /// Shared group root.
    pub node: HtmlElement,
    /// Operands rebound by an update, read without retaining a borrow across effects.
    options: Rc<RefCell<Rc<Options>>>,
    /// Click registrations owned by the current buttons.
    clicks: Vec<EventSubscription<Closure<dyn Fn()>>>,
    /// Event target, which must not keep its containing custom element alive.
    target: Link,
}
impl ModeGroup {
    /// Mount an empty group, then prepare its buttons.
    pub fn new(
        view: &DomView,
        options: Options,
        target: Option<&HtmlElement>,
    ) -> Result<Self, ToolUiError> {
        let node = element(view, "div")?;
        node.set_class_name("maestro-toggle");
        let target = js_sys::WeakRef::new(target.unwrap_or(&node));
        let mut group = Self {
            node,
            target,
            options: Rc::new(RefCell::new(Rc::new(options))),
            clicks: Vec::new(),
        };
        group.replace_buttons(view)?;
        Ok(group)
    }
    /// Rebind operands while retaining unchanged button nodes and their focus.
    pub fn show(&mut self, view: &DomView, options: Options) -> Result<(), ToolUiError> {
        let changed = {
            let previous = self.options.borrow();
            previous.modes.len() != options.modes.len()
                || previous
                    .modes
                    .iter()
                    .zip(&options.modes)
                    .any(|(left, right)| left.icon != right.icon)
        };
        let old = self.options.replace(Rc::new(options));
        drop(old);
        if changed {
            self.replace_buttons(view)?;
        } else {
            let current = self.options.borrow().clone();
            labels(&self.node, &current)?;
        }
        self.paint();
        Ok(())
    }
    /// Refresh the selected class after a silent or reentrant controller update.
    pub fn paint(&self) {
        let options = self.options.borrow().clone();
        paint(&self.node, &options);
    }
    /// Prepare replacement buttons before removing the previous children.
    fn replace_buttons(&mut self, view: &DomView) -> Result<(), ToolUiError> {
        let options = self.options.borrow().clone();
        let mut prepared = Vec::new();
        if options.modes.len() >= 2 {
            for (index, mode) in (0_u32..).zip(&options.modes) {
                let button = button(view, mode)?;
                let subscription = self.listen(&button, index)?;
                prepared.push((button, subscription));
            }
        }
        self.node.set_hidden(options.modes.len() < 2);
        self.node.set_text_content(None);
        self.clicks.clear();
        for (button, subscription) in prepared {
            self.node
                .append_child(&button)
                .map_err(|error| ToolUiError::native(&error))?;
            self.clicks.push(subscription);
        }
        self.paint();
        Ok(())
    }
    /// Subscribe one prepared button without retaining its containing custom element.
    fn listen(
        &self,
        button: &HtmlElement,
        index: u32,
    ) -> Result<EventSubscription<Closure<dyn Fn()>>, ToolUiError> {
        let (operands, group, target) = (
            Rc::clone(&self.options),
            js_sys::WeakRef::new(&self.node),
            self.target.clone(),
        );
        let click = Closure::<dyn Fn()>::new(move || {
            if let (Some(group), Some(target)) = (group.deref(), target.deref()) {
                choose(&operands, Some(&group), &target, index);
            }
        });
        EventSubscription::new(button.clone().into(), "click", click)
            .map_err(|error| ToolUiError::native(&error))
    }
}
/// Present the current controller selection without holding its operand borrow.
fn paint(group: &HtmlElement, options: &Options) {
    let selected = (options.read)();
    for (index, button) in (0_u32..).zip(buttons(group)) {
        button.set_class_name(
            if f64::from(index)
                .partial_cmp(&selected)
                .is_some_and(std::cmp::Ordering::is_eq)
            {
                "maestro-toggle-button maestro-toggle-selected"
            } else {
                "maestro-toggle-button"
            },
        );
    }
}
/// Walk native buttons in their displayed order.
fn buttons(group: &HtmlElement) -> impl Iterator<Item = web_sys::Element> {
    std::iter::successors(
        group.first_element_child(),
        web_sys::Element::next_element_sibling,
    )
}
/// Refresh the labels while preserving native button identities.
fn labels(group: &HtmlElement, options: &Options) -> Result<(), ToolUiError> {
    for (mode, button) in options.modes.iter().zip(buttons(group)) {
        button
            .set_attribute("title", &mode.label)
            .map_err(|error| ToolUiError::native(&error))?;
        if mode.icon.is_none() {
            button.set_text_content(Some(&mode.label));
        }
    }
    Ok(())
}
/// Select once and repaint after synchronous listeners have had their own turn.
fn choose(
    options: &RefCell<Rc<Options>>,
    group: Option<&HtmlElement>,
    target: &HtmlElement,
    index: u32,
) {
    let current = options.borrow().clone();
    let Some(mode) = current.modes.get(index as usize) else {
        return;
    };
    if (current.read)()
        .partial_cmp(&f64::from(index))
        .is_some_and(std::cmp::Ordering::is_eq)
    {
        return;
    }
    (current.write)(f64::from(index));
    if let Some(group) = group {
        paint(group, &current);
    }
    let init = CustomEventInit::new();
    init.set_detail(&(current.detail)(index, &mode.label));
    init.set_bubbles(true);
    if let Ok(event) = CustomEvent::new_with_event_init_dict("mode-change", &init) {
        let _ = target.dispatch_event(&event);
    }
    let current = options.borrow().clone();
    if let Some(group) = group {
        paint(group, &current);
    }
}
/// Build a button with literal text or an existing glyph.
fn button(view: &DomView, mode: &Mode) -> Result<HtmlElement, ToolUiError> {
    let button = element(view, "button")?;
    button.set_title(&mode.label);
    if let Some(icon) = mode.icon {
        let glyph = icons::icon_node(icon)
            .map_err(|error| ToolUiError::native(&JsValue::from_str(&error.to_string())))?
            .ok_or_else(|| ToolUiError::native(&JsValue::from_str("Missing icon")))?;
        let node = icons::icon_dom(&glyph, icons::IconSize::Sm, None)
            .map_err(|error| ToolUiError::native(&error))?;
        button
            .append_child(&node)
            .map_err(|error| ToolUiError::native(&error))?;
    } else {
        button.set_text_content(Some(&mode.label));
        button
            .set_attribute("data-mode-label", "")
            .map_err(|error| ToolUiError::native(&error))?;
    }
    Ok(button)
}
/// Create a native element in the current document.
fn element(view: &DomView, name: &str) -> Result<HtmlElement, ToolUiError> {
    view.document
        .create_element(name)
        .map_err(|error| ToolUiError::native(&error))?
        .dyn_into()
        .map_err(|error| ToolUiError::native(&error))
}
/// Authored operands kept across disconnect and reconnect.
struct State {
    /// Current caller-supplied labels.
    modes: Rc<RefCell<Vec<String>>>,
    /// Selected index, including a caller's silent out-of-range assignment.
    selected: Rc<Cell<f64>>,
    /// Connected group, released synchronously on disconnect.
    group: RefCell<Option<ModeGroup>>,
    /// Page operands for replacement buttons.
    view: Rc<DomView>,
    /// Non-owning route back to the element.
    root: Link,
}
impl State {
    /// Current labels and selection projected into the shared group owner.
    fn options(&self) -> Options {
        let (read, write) = (Rc::clone(&self.selected), Rc::clone(&self.selected));
        Options {
            modes: self
                .modes
                .borrow()
                .iter()
                .map(|label| Mode {
                    label: label.clone(),
                    icon: None,
                })
                .collect(),
            read: Rc::new(move || read.get()),
            write: Rc::new(move |index| write.set(index)),
            detail,
        }
    }
    /// Rebind current labels, preserving the group when its labels have not changed.
    fn refresh(&self) {
        let Some(root) = dom::connected(&self.root) else {
            return;
        };
        let options = self.options();
        let mut group = self.group.borrow_mut();
        if let Some(group) = group.as_mut() {
            let _ = group.show(&self.view, options);
        } else if let Ok(mounted) = ModeGroup::new(&self.view, options, Some(&root)) {
            let _ = root.append_child(&mounted.node);
            *group = Some(mounted);
        }
    }
    /// Apply the element's explicit changed-selection operation.
    fn select(&self, index: u32) {
        let mounted = self
            .group
            .borrow()
            .as_ref()
            .map(|group| (Rc::clone(&group.options), group.node.clone()));
        let Some(target) = dom::live(&self.root) else {
            return;
        };
        if let Some((options, group)) = mounted {
            choose(&options, Some(&group), &target, index);
        } else {
            choose(&RefCell::new(Rc::new(self.options())), None, &target, index);
        }
    }
}
/// Structured changed-selection detail of the authored-label element.
fn detail(index: u32, label: &str) -> JsValue {
    let value = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&value, &"index".into(), &index.into());
    let _ = js_sys::Reflect::set(&value, &"mode".into(), &label.into());
    value.into()
}
/// Page element owning property callables and connected resources.
struct ToggleElement {
    /// Shared state used by the accessors.
    state: Rc<State>,
    /// Property callables retained by this element.
    _properties: Vec<(Getter, Setter)>,
    /// Explicit changed-selection operation retained by this element.
    _select: Closure<dyn Fn(u32)>,
}
impl custom_element::CustomElement for ToggleElement {
    fn connected_callback(&mut self) {
        self.state.refresh();
    }
    fn disconnected_callback(&mut self) {
        let previous = self.state.group.borrow_mut().take();
        if let Some(group) = previous {
            group.node.remove();
        }
    }
}
/// Install authored mode labels and the silent selected-index property.
fn properties(root: &HtmlElement, state: &Rc<State>) -> Vec<(Getter, Setter)> {
    let (read, write) = (Rc::clone(&state.modes), Rc::clone(state));
    let modes = (
        Getter::new(move || {
            read.borrow()
                .iter()
                .map(JsValue::from)
                .collect::<js_sys::Array>()
                .into()
        }),
        Setter::new(move |value: JsValue| {
            if !js_sys::Array::is_array(&value) {
                return;
            }
            let Some(labels) = js_sys::Array::from(&value)
                .iter()
                .map(|value| value.as_string())
                .collect::<Option<Vec<_>>>()
            else {
                return;
            };
            *write.modes.borrow_mut() = labels;
            write.refresh();
        }),
    );
    dom::install(root, "modes", &modes.0, &modes.1);
    let (read, write) = (Rc::clone(&state.selected), Rc::clone(state));
    let selection = (
        Getter::new(move || read.get().into()),
        Setter::new(move |value: JsValue| {
            if let Some(index) = value.as_f64() {
                write.selected.set(index);
                write.refresh();
            }
        }),
    );
    dom::install(root, "selectedIndex", &selection.0, &selection.1);
    vec![modes, selection]
}
/// Register the authored-label light-DOM control.
pub(crate) fn define(view: &Rc<DomView>) -> Result<(), JsValue> {
    let view = Rc::clone(view);
    dom::define_element("mode-toggle", Vec::new(), &[], move |root, _| {
        let state = Rc::new(State {
            modes: Rc::new(RefCell::new(vec!["Mode 1".into(), "Mode 2".into()])),
            selected: Rc::new(Cell::new(0.)),
            group: RefCell::default(),
            view: Rc::clone(&view),
            root: js_sys::WeakRef::new(&root),
        });
        let properties = properties(&root, &state);
        let selected = Rc::clone(&state);
        let select = Closure::<dyn Fn(u32)>::new(move |index| selected.select(index));
        let _ = js_sys::Reflect::set(&root, &"setMode".into(), select.as_ref());
        ToggleElement {
            state,
            _properties: properties,
            _select: select,
        }
    })
}
