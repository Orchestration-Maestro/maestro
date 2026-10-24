//! Native preview/source control group and its page element.
use super::{PreviewCodeToggle, ViewMode};
use crate::{
    browser::{attachment_content::browser::host_error, event_subscription::EventSubscription},
    dom::{DomContent, DomRoot, DomView},
    icons,
    tools::types::ToolUiError,
};
use js_sys::WeakRef;
use std::{any::Any, cell::Cell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::{CustomEvent, CustomEventInit, Element, HtmlElement, Node};

/// Element tag of the standalone preview/source control.
pub(crate) const TAG: &str = "preview-code-toggle";
/// Group class holding position, clipping, border and background.
const GROUP: &str = "maestro-toggle";
/// Button class owning its selected, hover and focus presentation.
const BUTTON: &str = "maestro-toggle-button";
/// Class marked on the selected button.
const SELECTED: &str = "maestro-toggle-selected";
/// Event dispatched by a changed user selection.
const CHANGED: &str = "mode-change";
/// The two controls in native order with their glyph and message key.
const OPTIONS: [(ViewMode, &str, &str); 2] = [
    (ViewMode::Preview, "Eye", "Preview"),
    (ViewMode::Code, "Code", "Code"),
];
/// A weak element reference held by a callback or an element's own state.
type Link = WeakRef<HtmlElement>;
/// The clicks of one built group, released with the root that owns them.
type Clicks = Box<dyn Any>;
impl DomContent for PreviewCodeToggle {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let (root, clicks) = build(view, &self.selection, None)?;
        Ok(DomRoot {
            node: root.into(),
            owner: Some(clicks),
        })
    }
    fn update(&self, root: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        paint(&self.selection, &link(&root.node));
        Ok(())
    }
}
/// Build the control group for `selection`, reporting changes from `host` or from itself.
fn build(
    view: &DomView,
    selection: &Rc<Cell<ViewMode>>,
    host: Option<&HtmlElement>,
) -> Result<(Element, Clicks), ToolUiError> {
    let root: HtmlElement = view
        .document
        .create_element("div")
        .map_err(|error| failed(&error))?
        .dyn_into()
        .map_err(|error| failed(&error))?;
    root.set_class_name(GROUP);
    let group = WeakRef::new(&root);
    let host = WeakRef::new(host.unwrap_or(&root));
    let mut clicks = Vec::with_capacity(OPTIONS.len());
    for (mode, icon, key) in OPTIONS {
        let button: HtmlElement = view
            .document
            .create_element("button")
            .map_err(|error| failed(&error))?
            .dyn_into()
            .map_err(|error| failed(&error))?;
        button.set_class_name(&classes(mode, selection.get()));
        button.set_title(&view.i18n.text(key).map_err(ToolUiError::I18n)?);
        button
            .append_child(&glyph(icon)?)
            .map_err(|error| failed(&error))?;
        let (selection, group, host) = (Rc::clone(selection), group.clone(), host.clone());
        let click = Closure::<dyn Fn()>::new(move || choose(&selection, &group, &host, mode));
        clicks.push(
            EventSubscription::new(button.clone().into(), "click", click)
                .map_err(|error| failed(&error))?,
        );
        root.append_child(&button).map_err(|error| failed(&error))?;
    }
    Ok((root.into(), Box::new(clicks)))
}
/// Classes of one button in `mode` while `selected` is current.
fn classes(mode: ViewMode, selected: ViewMode) -> String {
    if mode == selected {
        format!("{BUTTON} {SELECTED}")
    } else {
        BUTTON.to_owned()
    }
}
/// One glyph at the control's fixed extent.
fn glyph(icon: &str) -> Result<Node, ToolUiError> {
    let node = icons::icon_node(icon)
        .map_err(|error| failed(&JsValue::from_str(&error.to_string())))?
        .ok_or_else(|| failed(&JsValue::from_str("Missing icon")))?;
    icons::icon_dom(&node, icons::IconSize::Sm, None)
        .map(Into::into)
        .map_err(|error| failed(&error))
}
/// The element behind `link` while it exists and is connected.
fn live(link: &Link) -> Option<HtmlElement> {
    link.deref().filter(|element| element.is_connected())
}
/// A weak reference to `node`, which a callback must not keep alive.
fn link(node: &Node) -> Link {
    WeakRef::new(node.unchecked_ref::<HtmlElement>())
}
/// Select `mode` and show it, reporting whether the selection changed.
fn select(selection: &Rc<Cell<ViewMode>>, group: &Link, mode: ViewMode) -> bool {
    if selection.get() == mode {
        return false;
    }
    selection.set(mode);
    paint(selection, group);
    true
}
/// Report one changed user selection, keeping any selection made while it dispatched.
fn choose(selection: &Rc<Cell<ViewMode>>, group: &Link, host: &Link, mode: ViewMode) {
    if select(selection, group, mode) {
        // A listener may select again while the event bubbles; its choice stays shown.
        let _ = dispatch(host, mode);
        paint(selection, group);
    }
}
/// Show `selection` on the group's own buttons.
fn paint(selection: &Rc<Cell<ViewMode>>, group: &Link) {
    let Some(root) = live(group) else {
        return;
    };
    let mut next = root.first_element_child();
    for (mode, ..) in OPTIONS {
        let Some(button) = next else {
            return;
        };
        button.set_class_name(&classes(mode, selection.get()));
        next = button.next_element_sibling();
    }
}
/// Dispatch one changed selection from the group's actual root.
fn dispatch(host: &Link, mode: ViewMode) -> Result<bool, JsValue> {
    let Some(target) = live(host) else {
        return Ok(true);
    };
    let init = CustomEventInit::new();
    init.set_detail(&JsValue::from_str(name(mode)));
    init.set_bubbles(true);
    init.set_composed(false);
    init.set_cancelable(false);
    let event = CustomEvent::new_with_event_init_dict(CHANGED, &init)?;
    target.dispatch_event(&event)
}
/// Mode spelling carried by the element property and the changed-selection event.
fn name(mode: ViewMode) -> &'static str {
    match mode {
        ViewMode::Preview => "preview",
        ViewMode::Code => "code",
    }
}
/// Report a native failure as the control's own failure.
fn failed(error: &JsValue) -> ToolUiError {
    ToolUiError::Host(host_error(error))
}
/// One page-defined preview/source control.
struct ToggleElement {
    /// Element owning this state.
    root: Link,
    /// Page operands.
    view: Rc<DomView>,
    /// Selected mode, kept across reconnection.
    selection: Rc<Cell<ViewMode>>,
    /// Mounted group and the clicks that live with it while connected.
    mounted: Option<(Element, Clicks)>,
    /// Accessors of the unreflected `mode` property.
    _mode: Accessors,
}
impl custom_element::CustomElement for ToggleElement {
    fn connected_callback(&mut self) {
        let Some(root) = live(&self.root) else {
            return;
        };
        // Lifecycle callbacks have no error channel; a failed build leaves the element empty.
        if let Ok((group, clicks)) = build(&self.view, &self.selection, Some(&root)) {
            let _ = root.append_child(&group);
            self.mounted = Some((group, clicks));
        }
    }
    fn disconnected_callback(&mut self) {
        if let Some((group, _clicks)) = self.mounted.take() {
            group.remove();
        }
    }
}
/// Getter and setter callables of one element property, kept as long as its element's state.
type Accessors = (Closure<dyn Fn() -> JsValue>, Closure<dyn Fn(JsValue)>);
/// Install the unreflected `mode` property reading and writing `selection`.
fn property(root: &HtmlElement, selection: &Rc<Cell<ViewMode>>) -> Accessors {
    let read = Rc::clone(selection);
    let get = Closure::<dyn Fn() -> JsValue>::new(move || name(read.get()).into());
    let (write, group) = (Rc::clone(selection), WeakRef::new(root));
    let set = Closure::<dyn Fn(JsValue)>::new(move |value: JsValue| {
        // Only the declared modes are operands; other assignments leave the selection alone.
        let mode = match value.as_string().as_deref() {
            Some("preview") => ViewMode::Preview,
            Some("code") => ViewMode::Code,
            _ => return,
        };
        select(&write, &group, mode);
    });
    let descriptor = js_sys::Object::new();
    for (key, value) in [("get", get.as_ref()), ("set", set.as_ref())] {
        let _ = js_sys::Reflect::set(&descriptor, &key.into(), value);
    }
    let _ = js_sys::Reflect::set(&descriptor, &"configurable".into(), &JsValue::TRUE);
    // A valid property name on a live element only fails when it already exists unconfigurably.
    let _ = js_sys::Reflect::define_property(root, &"mode".into(), &descriptor);
    (get, set)
}
/// Define the preview/source control once for the page.
/// # Errors
/// Returns the browser's registration failure.
pub(crate) fn define(view: &Rc<DomView>) -> Result<(), JsValue> {
    let view = Rc::clone(view);
    crate::dom::define_element(TAG, Vec::new(), &[], move |root, _| {
        let selection = Rc::new(Cell::new(ViewMode::Preview));
        let mode = property(&root, &selection);
        ToggleElement {
            root: WeakRef::new(&root),
            view: Rc::clone(&view),
            selection,
            mounted: None,
            _mode: mode,
        }
    })
}
