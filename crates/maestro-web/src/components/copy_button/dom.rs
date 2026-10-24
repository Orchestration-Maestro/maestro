//! Native copy control element with timed success feedback.
use crate::{
    browser::{
        attachment_content::ClipboardMode, event_subscription::EventSubscription, timer::Timer,
    },
    components::controls::{ButtonProps, ButtonSize, ButtonVariant, button},
    page::{DomView, Link, Property, Reflected, connected, live},
    utils::localization::I18n,
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use wasm_bindgen::{JsValue, closure::Closure};
use web_sys::HtmlElement;

/// Element tag of the copy control.
const TAG: &str = "copy-button";
/// Success feedback duration in milliseconds.
const FEEDBACK_MS: i32 = 2000;
/// The active restoring timer while success feedback shows.
type Feedback = Rc<RefCell<Option<Timer>>>;
/// Localization a copy control resolves its labels through, replaceable by its creator.
pub(crate) type Localization = Rc<RefCell<Rc<I18n>>>;
thread_local! {
    /// Hands `create`'s localization to the constructor, which has no argument channel.
    static CREATING: RefCell<Option<Localization>> = const { RefCell::new(None) };
}
/// Properties reflected onto the attributes; `title` is the element's own.
const PROPERTIES: &[Property] = &[
    Property::Text("text", "text"),
    Property::Flag("showText", "show-text"),
];
/// One mounted copy control; `text`, `title` and `show-text` are its attributes.
struct CopyElement {
    /// Element owning this state.
    root: Link,
    /// Page operands.
    view: Rc<DomView>,
    /// Labels' localization.
    i18n: Localization,
    /// Click subscription while connected.
    click: Option<EventSubscription<Closure<dyn Fn()>>>,
    /// Restoring timer while the success state shows.
    feedback: Feedback,
    /// Accessors of the `text` and `showText` properties.
    _properties: Reflected,
}
impl custom_element::CustomElement for CopyElement {
    fn attribute_changed_callback(&mut self, _: &str, _: JsValue, _: JsValue) {
        if let Some(root) = connected(&self.root) {
            let copied = self.feedback.borrow().is_some();
            // Lifecycle callbacks have no error channel; a failed render keeps the previous control.
            let _ = render(&root, &self.view, &self.i18n.borrow(), copied);
        }
    }
    fn connected_callback(&mut self) {
        let Some(root) = live(&self.root) else {
            return;
        };
        // Lifecycle callbacks have no error channel; a failed render leaves the element empty.
        let _ = render(&root, &self.view, &self.i18n.borrow(), false);
        let (weak, view, i18n, feedback) = (
            self.root.clone(),
            Rc::clone(&self.view),
            Rc::clone(&self.i18n),
            Rc::downgrade(&self.feedback),
        );
        let callback = Closure::new(move || copy(&weak, (&view, &i18n), &feedback));
        self.click = EventSubscription::new(root.into(), "click", callback).ok();
    }
    fn disconnected_callback(&mut self) {
        self.click = None;
        let restoring = self.feedback.borrow_mut().take();
        drop(restoring);
    }
}
/// Copy the `text` attribute, then show success until the restoring timer fires.
fn copy(
    root: &Link,
    (view, i18n): (&Rc<DomView>, &Localization),
    feedback: &Weak<RefCell<Option<Timer>>>,
) {
    let Some(element) = live(root) else {
        return;
    };
    let text = element.get_attribute("text").unwrap_or_default();
    let work =
        crate::utils::clipboard::copy_to_clipboard(&view.document, &text, ClipboardMode::Direct);
    let (root, view, i18n, feedback) = (
        root.clone(),
        Rc::clone(view),
        Rc::clone(i18n),
        feedback.clone(),
    );
    wasm_bindgen_futures::spawn_local(async move {
        let (true, Some(owner), Some(window)) = (work.await, feedback.upgrade(), web_sys::window())
        else {
            return;
        };
        let (weak, restoring_view, restoring_i18n) =
            (root.clone(), Rc::clone(&view), Rc::clone(&i18n));
        let restore = Rc::new(move || {
            let Some(owner) = feedback.upgrade() else {
                return;
            };
            let finished = owner.borrow_mut().take();
            drop(finished);
            if let Some(element) = live(&weak) {
                let _ = render(&element, &restoring_view, &restoring_i18n.borrow(), false);
            }
        });
        let timer = crate::browser::timer::native(window, FEEDBACK_MS, false, restore).ok();
        let replaced = owner.replace(timer);
        drop(replaced);
        if let Some(element) = live(&root) {
            let _ = render(&element, &view, &i18n.borrow(), true);
        }
    });
}
/// Render the button with its icon and, when requested, the copied label.
fn render(root: &HtmlElement, view: &DomView, i18n: &I18n, copied: bool) -> Result<(), JsValue> {
    let document = &view.document;
    let node = crate::icons::icon_node(if copied { "Check" } else { "Copy" })
        .map_err(|error| JsValue::from_str(&error.to_string()))?
        .ok_or_else(|| JsValue::from_str("Missing icon"))?;
    let mut children: Vec<web_sys::Node> =
        vec![crate::icons::icon_dom(&node, crate::icons::IconSize::Sm, None)?.into()];
    if copied && root.has_attribute("show-text") {
        let label = document.create_element("span")?;
        let text = i18n
            .text("Copied!")
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        label.set_text_content(Some(&text));
        children.push(label.into());
    }
    let title = match root.get_attribute("title") {
        Some(title) => title,
        None => i18n
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
    crate::page::define_element(TAG, observed, PROPERTIES, move |root, properties| {
        CopyElement {
            root: js_sys::WeakRef::new(&root),
            view: Rc::clone(&view),
            i18n: CREATING
                .take()
                .unwrap_or_else(|| Rc::new(RefCell::new(Rc::clone(&view.i18n)))),
            click: None,
            feedback: Rc::default(),
            _properties: properties,
        }
    })
}
/// Create a copy control resolving its labels through `i18n`.
/// # Errors
/// Returns the browser's creation failure.
pub(crate) fn create(
    document: &web_sys::Document,
    i18n: &Localization,
) -> Result<web_sys::Element, JsValue> {
    CREATING.set(Some(Rc::clone(i18n)));
    let created = document.create_element(TAG);
    CREATING.take();
    created
}
