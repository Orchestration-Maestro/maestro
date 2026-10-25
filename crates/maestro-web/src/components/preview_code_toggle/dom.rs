//! Native preview/source control group and its page element.
use super::{PreviewCodeToggle, ViewMode};
use crate::{
    browser::{attachment_content::AttachmentHostError, event_subscription::EventSubscription},
    dom::{self, DomContent, DomRoot, DomView, Getter, Link, Setter},
    icons,
    tools::types::ToolUiError,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::{CustomEvent, CustomEventInit, HtmlElement, Node};

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
/// The selection a mounted group reports to, replaced when another controller updates it.
type Selection = Rc<RefCell<Rc<Cell<ViewMode>>>>;
/// Getter and setter callables of one element property, kept as long as its element's state.
type Accessors = (Getter, Setter);
/// One mounted control group and the state its callbacks read.
struct Mounted {
    /// Group node holding the two buttons.
    group: HtmlElement,
    /// Selection the buttons report to.
    selection: Selection,
    /// Clicks released with the group.
    _clicks: Vec<EventSubscription<Closure<dyn Fn()>>>,
}
impl Mounted {
    /// Build the group for `selection`, reporting changed user selections from `host`.
    fn build(
        view: &DomView,
        selection: &Rc<Cell<ViewMode>>,
        host: Option<&HtmlElement>,
    ) -> Result<Self, ToolUiError> {
        let group = element(view, "div")?;
        group.set_class_name(GROUP);
        let host = host.cloned().unwrap_or_else(|| group.clone());
        let shared = Rc::new(RefCell::new(Rc::clone(selection)));
        let mut clicks = Vec::with_capacity(OPTIONS.len());
        for (mode, icon, key) in OPTIONS {
            let button = element(view, "button")?;
            button.set_class_name(&classes(mode, selection.get()));
            button.set_title(&view.i18n.text(key).map_err(ToolUiError::I18n)?);
            button
                .append_child(&glyph(icon)?)
                .map_err(|error| ToolUiError::native(&error))?;
            let (selection, watched, target) = (Rc::clone(&shared), group.clone(), host.clone());
            let click = Closure::<dyn Fn()>::new(move || {
                choose(&selection, &watched, &target, mode);
            });
            clicks.push(
                EventSubscription::new(button.clone().into(), "click", click)
                    .map_err(|error| ToolUiError::native(&error))?,
            );
            group
                .append_child(&button)
                .map_err(|error| ToolUiError::native(&error))?;
        }
        Ok(Self {
            group,
            selection: shared,
            _clicks: clicks,
        })
    }
    /// Show the selection the buttons currently report to.
    fn paint(&self) {
        paint(&self.group, self.selection.borrow().get());
    }
    /// Take over `selection` and this view: another controller owns the group afterwards.
    fn show(&self, selection: &Rc<Cell<ViewMode>>, view: &DomView) -> Result<(), ToolUiError> {
        localize(&self.group, view)?;
        *self.selection.borrow_mut() = Rc::clone(selection);
        self.paint();
        Ok(())
    }
}
impl DomContent for PreviewCodeToggle {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let mounted = Rc::new(Mounted::build(view, &self.selection, None)?);
        Ok(DomRoot {
            node: mounted.group.clone().into(),
            owner: Some(Box::new(mounted)),
        })
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let mounted = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Rc<Mounted>>())
            .ok_or_else(missing_group)?;
        mounted.show(&self.selection, view)
    }
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
        .map_err(|error| ToolUiError::native(&JsValue::from_str(&error.to_string())))?
        .ok_or_else(|| ToolUiError::native(&JsValue::from_str("Missing icon")))?;
    icons::icon_dom(&node, icons::IconSize::Sm, None)
        .map(Into::into)
        .map_err(|error| ToolUiError::native(&error))
}
/// A new element of `name` in the page.
fn element(view: &DomView, name: &str) -> Result<HtmlElement, ToolUiError> {
    view.document
        .create_element(name)
        .map_err(|error| ToolUiError::native(&error))?
        .dyn_into()
        .map_err(|error| ToolUiError::native(&error))
}
/// The group's buttons in native order with their mode, glyph and message key.
fn buttons(
    group: &HtmlElement,
) -> impl Iterator<Item = ((ViewMode, &'static str, &'static str), HtmlElement)> + '_ {
    let mut next = group.first_element_child();
    OPTIONS.into_iter().filter_map(move |option| {
        let button = next.clone()?.unchecked_into::<HtmlElement>();
        next = button.next_element_sibling();
        Some((option, button))
    })
}
/// Show `selected` on the buttons of `group`.
fn paint(group: &HtmlElement, selected: ViewMode) {
    for ((mode, ..), button) in buttons(group) {
        button.set_class_name(&classes(mode, selected));
    }
}
/// Refresh the localized button titles of `group` from `view`.
fn localize(group: &HtmlElement, view: &DomView) -> Result<(), ToolUiError> {
    for ((_, _, key), button) in buttons(group) {
        button.set_title(&view.i18n.text(key).map_err(ToolUiError::I18n)?);
    }
    Ok(())
}
/// Select `mode` in `selection`, reporting whether the selection changed.
fn select(selection: &Rc<Cell<ViewMode>>, mode: ViewMode) -> bool {
    if selection.get() == mode {
        return false;
    }
    selection.set(mode);
    true
}
/// Report one changed user selection, keeping any selection made while it dispatched.
fn choose(selection: &Selection, group: &HtmlElement, host: &HtmlElement, mode: ViewMode) {
    if !select(&selection.borrow(), mode) {
        return;
    }
    paint(group, mode);
    let _ = dispatch(host, mode);
    // A listener may select again while the event bubbles; its choice stays shown.
    paint(group, selection.borrow().get());
}
/// Dispatch one changed selection from the element a page listens on.
fn dispatch(host: &HtmlElement, mode: ViewMode) -> Result<bool, JsValue> {
    let init = CustomEventInit::new();
    init.set_detail(&JsValue::from_str(name(mode)));
    init.set_bubbles(true);
    init.set_composed(false);
    init.set_cancelable(false);
    let event = CustomEvent::new_with_event_init_dict(CHANGED, &init)?;
    host.dispatch_event(&event)
}
/// Mode spelling carried by the element property and the changed-selection event.
fn name(mode: ViewMode) -> &'static str {
    match mode {
        ViewMode::Preview => "preview",
        ViewMode::Code => "code",
    }
}
/// Report an update of a root this control does not own.
fn missing_group() -> ToolUiError {
    ToolUiError::Host(AttachmentHostError::Presentation(
        "Missing preview/source group".into(),
    ))
}
/// One page-defined preview/source control.
struct ToggleElement {
    /// Element owning this state.
    root: Link,
    /// Page operands.
    view: Rc<DomView>,
    /// Selected mode, kept across reconnection.
    selection: Rc<Cell<ViewMode>>,
    /// Group mounted while connected, painted by the `mode` setter.
    mounted: Rc<RefCell<Option<Rc<Mounted>>>>,
    /// Accessors of the unreflected `mode` property.
    _mode: Accessors,
}
impl custom_element::CustomElement for ToggleElement {
    fn connected_callback(&mut self) {
        let Some(root) = dom::connected(&self.root) else {
            return;
        };
        // Lifecycle callbacks have no error channel; a failed build leaves the element empty.
        let Ok(mounted) = Mounted::build(&self.view, &self.selection, Some(&root)) else {
            return;
        };
        let _ = root.append_child(&mounted.group);
        *self.mounted.borrow_mut() = Some(Rc::new(mounted));
    }
    fn disconnected_callback(&mut self) {
        if let Some(mounted) = self.mounted.borrow_mut().take() {
            mounted.group.remove();
        }
    }
}
/// Install the unreflected `mode` property reading and writing `selection`.
fn property(
    root: &HtmlElement,
    selection: &Rc<Cell<ViewMode>>,
    mounted: &Rc<RefCell<Option<Rc<Mounted>>>>,
) -> Accessors {
    let read = Rc::clone(selection);
    let get = Getter::new(move || name(read.get()).into());
    let (write, group) = (Rc::clone(selection), Rc::clone(mounted));
    let set = Setter::new(move |value: JsValue| {
        // Only the declared modes are operands; other assignments leave the selection alone.
        let mode = match value.as_string().as_deref() {
            Some("preview") => ViewMode::Preview,
            Some("code") => ViewMode::Code,
            _ => return,
        };
        if select(&write, mode)
            && let Some(mounted) = group.borrow().as_ref()
        {
            mounted.paint();
        }
    });
    // A valid property name on a live element only fails when it already exists unconfigurably.
    dom::install(root, "mode", &get, &set);
    (get, set)
}
/// Define the preview/source control once for the page.
/// # Errors
/// Returns the browser's registration failure.
pub(crate) fn define(view: &Rc<DomView>) -> Result<(), JsValue> {
    let view = Rc::clone(view);
    dom::define_element(TAG, Vec::new(), &[], move |root, _| {
        let selection = Rc::new(Cell::new(ViewMode::Preview));
        let mounted = Rc::default();
        let mode = property(&root, &selection, &mounted);
        ToggleElement {
            root: js_sys::WeakRef::new(&root),
            view: Rc::clone(&view),
            selection,
            mounted,
            _mode: mode,
        }
    })
}
