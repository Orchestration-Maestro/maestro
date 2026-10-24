//! Native copy control element with timed success feedback.
use crate::{
    browser::{
        attachment_content::ClipboardMode, event_subscription::EventSubscription, timer::Timer,
    },
    components::button::{ButtonProps, ButtonSize, ButtonVariant, button},
    dom::{DomView, Property, Reflected},
};
use js_sys::WeakRef;
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::HtmlElement;

/// Element tag of the copy control.
pub(crate) const TAG: &str = "copy-button";
/// Success feedback duration in milliseconds.
const FEEDBACK_MS: i32 = 2000;
/// The active restoring timer while success feedback shows.
type Feedback = Rc<RefCell<Option<Timer>>>;
/// Properties reflected onto the attributes; `title` is the element's own.
const PROPERTIES: &[Property] = &[
    Property::Text("text", "text"),
    Property::Flag("showText", "show-text"),
];
/// One mounted copy control; `text`, `title` and `show-text` are its attributes.
struct CopyElement {
    /// Element owning this state.
    root: WeakRef,
    /// Page operands.
    view: Rc<DomView>,
    /// Click subscription while connected.
    click: Option<EventSubscription<Closure<dyn Fn()>>>,
    /// Restoring timer while the success state shows.
    feedback: Feedback,
    /// Accessors of the `text` and `showText` properties.
    _properties: Reflected,
}
impl custom_element::CustomElement for CopyElement {
    fn attribute_changed_callback(&mut self, _: &str, _: JsValue, _: JsValue) {
        if let Some(root) = crate::dom::connected(&self.root) {
            let copied = self.feedback.borrow().is_some();
            // Lifecycle callbacks have no error channel; a failed render keeps the previous control.
            let _ = render(&root, &self.view, copied);
        }
    }
    fn connected_callback(&mut self) {
        let Some(root) = live(&self.root) else {
            return;
        };
        // Lifecycle callbacks have no error channel; a failed render leaves the element empty.
        let _ = render(&root, &self.view, false);
        let (weak, view, feedback) = (
            self.root.clone(),
            Rc::clone(&self.view),
            Rc::downgrade(&self.feedback),
        );
        let callback = Closure::new(move || copy(&weak, &view, &feedback));
        self.click = EventSubscription::new(root.into(), "click", callback).ok();
    }
    fn disconnected_callback(&mut self) {
        self.click = None;
        let restoring = self.feedback.borrow_mut().take();
        drop(restoring);
    }
}
/// The element, while it exists.
fn live(root: &WeakRef) -> Option<HtmlElement> {
    root.deref().and_then(|root| root.dyn_into().ok())
}
/// Copy the `text` attribute, then show success until the restoring timer fires.
fn copy(root: &WeakRef, view: &Rc<DomView>, feedback: &Weak<RefCell<Option<Timer>>>) {
    let Some(element) = live(root) else {
        return;
    };
    let text = element.get_attribute("text").unwrap_or_default();
    let work =
        crate::utils::clipboard::copy_to_clipboard(&view.document, &text, ClipboardMode::Direct);
    let (root, view, feedback) = (root.clone(), Rc::clone(view), feedback.clone());
    wasm_bindgen_futures::spawn_local(async move {
        let (true, Some(owner), Some(window)) = (work.await, feedback.upgrade(), web_sys::window())
        else {
            return;
        };
        let (weak, restoring_view) = (root.clone(), Rc::clone(&view));
        let restore = Rc::new(move || {
            let Some(owner) = feedback.upgrade() else {
                return;
            };
            let finished = owner.borrow_mut().take();
            drop(finished);
            if let Some(element) = live(&weak) {
                let _ = render(&element, &restoring_view, false);
            }
        });
        let timer = crate::browser::timer::native(window, FEEDBACK_MS, false, restore).ok();
        let replaced = owner.replace(timer);
        drop(replaced);
        if let Some(element) = live(&root) {
            let _ = render(&element, &view, true);
        }
    });
}
/// Render the button with its icon and, when requested, the copied label.
fn render(root: &HtmlElement, view: &DomView, copied: bool) -> Result<(), JsValue> {
    let document = &view.document;
    let node = crate::icons::icon_node(if copied { "Check" } else { "Copy" })
        .map_err(|error| JsValue::from_str(&error.to_string()))?
        .ok_or_else(|| JsValue::from_str("Missing icon"))?;
    let mut children: Vec<web_sys::Node> =
        vec![crate::icons::icon_dom(&node, crate::icons::IconSize::Sm, None)?.into()];
    if copied && root.has_attribute("show-text") {
        let label = document.create_element("span")?;
        let text = view
            .i18n
            .text("Copied!")
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        label.set_text_content(Some(&text));
        children.push(label.into());
    }
    let title = match root.get_attribute("title") {
        Some(title) => title,
        None => view
            .i18n
            .text("Copy")
            .map_err(|error| JsValue::from_str(&error.to_string()))?,
    };
    let props = ButtonProps {
        variant: ButtonVariant::Ghost,
        size: ButtonSize::Sm,
        title: Some(&title),
        ..ButtonProps::default()
    };
    let control = button(document, &props, &children)?;
    root.replace_children_with_node_1(&control);
    Ok(())
}
/// Define the copy control once for the page.
/// # Errors
/// Returns the browser's registration failure.
pub(crate) fn define(view: &Rc<DomView>) -> Result<(), JsValue> {
    let view = Rc::clone(view);
    let observed = vec!["title".into(), "show-text".into()];
    crate::dom::define_element(TAG, observed, PROPERTIES, move |root, properties| {
        CopyElement {
            root: WeakRef::new(&root),
            view: Rc::clone(&view),
            click: None,
            feedback: Rc::default(),
            _properties: properties,
        }
    })
}
