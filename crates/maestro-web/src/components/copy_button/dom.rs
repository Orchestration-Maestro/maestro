//! Native copy control element with timed success feedback.
use crate::{
    browser::{
        attachment_content::ClipboardMode, event_subscription::EventSubscription, timer::Timer,
    },
    components::button::{ButtonProps, ButtonSize, ButtonVariant, button},
    dom::{DomView, Link, Property, Reflected, connected, live},
    utils::i18n::I18n,
};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::HtmlElement;

/// Element tag of the copy control.
const TAG: &str = "copy-button";
/// Success feedback duration of a control built outside [`create`], in milliseconds.
pub(crate) const FEEDBACK_MS: i32 = 2000;
/// Attribute selecting the compact control treatment.
const COMPACT: &str = "compact";
/// Class carrying the compact geometry and the muted idle and accent highlight roles.
const COMPACT_CLASS: &str = "maestro-button-compact";
/// The active restoring timer while success feedback shows.
type Feedback = Rc<RefCell<Option<Timer>>>;
/// Localization a copy control resolves its labels through, replaceable by its creator.
pub(crate) type Localization = Rc<RefCell<Rc<I18n>>>;
/// Text a creator produces when its control is activated, read instead of the `text`
/// attribute; `None` copies nothing.
pub(crate) type TextSource = Rc<dyn Fn() -> Option<String>>;
/// Operands [`create`] hands to the constructor through the shared construction channel.
struct Creating {
    /// Labels' localization.
    i18n: Localization,
    /// Success feedback duration in milliseconds.
    feedback: i32,
    /// Notification after each feedback change, which a creator's own view follows.
    changed: Option<Rc<dyn Fn()>>,
    /// Text produced at activation instead of the `text` attribute.
    text: Option<TextSource>,
}
/// One activation's retained operands, reached by the control's click subscription.
struct Activation {
    /// Element owning this state.
    root: Link,
    /// Page operands.
    view: Rc<DomView>,
    /// Labels' localization.
    i18n: Localization,
    /// Restoring timer while the success state shows.
    feedback: Weak<RefCell<Option<Timer>>>,
    /// Success feedback duration in milliseconds.
    feedback_ms: i32,
    /// Notification after each feedback change.
    changed: Option<Rc<dyn Fn()>>,
    /// Text produced at activation instead of the `text` attribute.
    text: Option<TextSource>,
    /// Connection generations, bumped on every connection and disconnection.
    epoch: Rc<Cell<u64>>,
    /// The generation that admitted this activation, which a later one retires.
    admitted: u64,
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
    /// Success feedback duration in milliseconds.
    feedback_ms: i32,
    /// Notification after each feedback change.
    changed: Option<Rc<dyn Fn()>>,
    /// Text produced at activation instead of the `text` attribute.
    text: Option<TextSource>,
    /// Connection generations, bumped on every connection and disconnection.
    epoch: Rc<Cell<u64>>,
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
        let activation = Rc::new(Activation {
            root: self.root.clone(),
            view: Rc::clone(&self.view),
            i18n: Rc::clone(&self.i18n),
            feedback: Rc::downgrade(&self.feedback),
            feedback_ms: self.feedback_ms,
            changed: self.changed.clone(),
            text: self.text.clone(),
            epoch: Rc::clone(&self.epoch),
            admitted: self.epoch.get().wrapping_add(1),
        });
        self.epoch.set(activation.admitted);
        let callback = Closure::new(move || copy(&activation));
        self.click = EventSubscription::new(root.into(), "click", callback).ok();
    }
    fn disconnected_callback(&mut self) {
        self.click = None;
        self.epoch.set(self.epoch.get().wrapping_add(1));
        let restoring = self.feedback.borrow_mut().take();
        drop(restoring);
    }
}
/// Copy the creator's text, or else the `text` attribute, then show success until the
/// restoring timer fires.
fn copy(activation: &Rc<Activation>) {
    let Some(element) = live(&activation.root) else {
        return;
    };
    let text = match &activation.text {
        Some(text) => text(),
        None => Some(element.get_attribute("text").unwrap_or_default()),
    };
    let Some(text) = text else {
        return;
    };
    let work = crate::utils::clipboard::copy_to_clipboard(
        &activation.view.document,
        &text,
        ClipboardMode::Direct,
    );
    let activation = Rc::clone(activation);
    wasm_bindgen_futures::spawn_local(async move {
        let settled = work.await;
        if activation.epoch.get() != activation.admitted {
            return;
        }
        let (true, Some(owner), Some(window)) =
            (settled, activation.feedback.upgrade(), web_sys::window())
        else {
            return;
        };
        let restoring = Rc::clone(&activation);
        let restore = Rc::new(move || restore(&restoring));
        let timer =
            crate::browser::timer::native(window, activation.feedback_ms, false, restore).ok();
        let replaced = owner.replace(timer);
        drop(replaced);
        if let Some(element) = live(&activation.root) {
            let _ = render(&element, &activation.view, &activation.i18n.borrow(), true);
            changed(&activation);
        }
    });
}
/// Return the control to its idle state and tell the creator the feedback ended.
fn restore(activation: &Activation) {
    let Some(owner) = activation.feedback.upgrade() else {
        return;
    };
    let finished = owner.borrow_mut().take();
    drop(finished);
    if let Some(element) = live(&activation.root) {
        let _ = render(&element, &activation.view, &activation.i18n.borrow(), false);
        changed(activation);
    }
}
/// Notify the creator after a feedback change.
fn changed(activation: &Activation) {
    if let Some(changed) = &activation.changed {
        changed();
    }
}
/// Render the button with its icon and, when requested, the copied label, keeping the native
/// button so that keyboard focus survives success and restoration.
fn render(root: &HtmlElement, view: &DomView, i18n: &I18n, copied: bool) -> Result<(), JsValue> {
    let document = &view.document;
    let props = ButtonProps {
        variant: ButtonVariant::Ghost,
        size: ButtonSize::Sm,
        class_name: if root.has_attribute(COMPACT) {
            COMPACT_CLASS
        } else {
            ""
        },
        ..ButtonProps::default()
    };
    let control = if let Some(control) = root.first_element_child() {
        control
    } else {
        let created = button(document, &props, &[])?;
        root.append_child(&created)?;
        created.unchecked_into()
    };
    let classes = props.classes();
    if control.class_name() != classes {
        control.set_class_name(&classes);
    }
    let title = match root.get_attribute("title") {
        Some(title) => title,
        None => i18n
            .text("Copy")
            .map_err(|error| JsValue::from_str(&error.to_string()))?,
    };
    if control.get_attribute("title").as_deref() != Some(title.as_str()) {
        control.set_attribute("title", &title)?;
    }
    let glyph = crate::icons::icon_node(if copied { "Check" } else { "Copy" })
        .map_err(|error| JsValue::from_str(&error.to_string()))?
        .ok_or_else(|| JsValue::from_str("Missing icon"))?;
    let children = document.create_document_fragment();
    let icon: web_sys::Node =
        crate::icons::icon_dom(&glyph, crate::icons::IconSize::Sm, None)?.into();
    children.append_child(&icon)?;
    if copied && root.has_attribute("show-text") {
        let label = document.create_element("span")?;
        let text = i18n
            .text("Copied!")
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        label.set_text_content(Some(&text));
        children.append_child(&label)?;
    }
    control.replace_children_with_node_1(&children);
    Ok(())
}
/// Define the copy control once for the page.
/// # Errors
/// Returns the browser's registration failure.
pub(crate) fn define(view: &Rc<DomView>) -> Result<(), JsValue> {
    let view = Rc::clone(view);
    let observed = vec!["title".into(), "show-text".into(), COMPACT.into()];
    crate::dom::define_element(TAG, observed, PROPERTIES, move |root, properties| {
        let creating = crate::dom::take_creating::<Creating>();
        CopyElement {
            root: js_sys::WeakRef::new(&root),
            view: Rc::clone(&view),
            i18n: creating.as_ref().map_or_else(
                || Rc::new(RefCell::new(Rc::clone(&view.i18n))),
                |creating| Rc::clone(&creating.i18n),
            ),
            click: None,
            feedback: Rc::default(),
            feedback_ms: creating.as_ref().map_or(FEEDBACK_MS, |c| c.feedback),
            text: creating.as_ref().and_then(|creating| creating.text.clone()),
            changed: creating.and_then(|creating| creating.changed),
            epoch: Rc::default(),
            _properties: properties,
        }
    })
}
/// Create a copy control resolving its labels through `i18n`, restoring after `feedback`
/// milliseconds, notifying `changed` after each feedback change and copying `text` when one
/// is supplied.
/// # Errors
/// Returns the browser's creation failure.
pub(crate) fn create(
    document: &web_sys::Document,
    i18n: &Localization,
    feedback: i32,
    changed: Option<Rc<dyn Fn()>>,
    text: Option<TextSource>,
) -> Result<web_sys::Element, JsValue> {
    crate::dom::create_with(
        document,
        TAG,
        Creating {
            i18n: Rc::clone(i18n),
            feedback,
            changed,
            text,
        },
    )
}
