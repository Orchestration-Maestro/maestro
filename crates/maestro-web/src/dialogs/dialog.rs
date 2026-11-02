//! Modal backdrop, panel and Close control mounted as native page content.
use crate::{
    browser::{
        attachment_content::browser::{append, element, host_error},
        event_subscription::EventSubscription,
    },
    components::input::{classes, unique_id},
    dom::{DomRoot, DomView, icon},
    tools::types::ToolUiError,
};
use std::rc::Rc;
use wasm_bindgen::{JsCast as _, closure::Closure};
use web_sys::{Document, Element, Event, KeyboardEvent, Node};

/// Dialog operands; `on_close` is notified by the backdrop, the Close control and Escape.
#[derive(Clone)]
pub struct DialogProps {
    /// Whether the dialog is shown; a closed dialog mounts nothing.
    pub is_open: bool,
    /// Notified once for each close request.
    pub on_close: Option<Rc<dyn Fn()>>,
    /// Panel children in order.
    pub children: Vec<Node>,
    /// Literal CSS width of the panel.
    pub width: String,
    /// Literal CSS height of the panel.
    pub height: String,
    /// Panel classes appended after the panel class.
    pub class_name: String,
    /// Backdrop classes appended after the backdrop class.
    pub backdrop_class_name: String,
}
impl DialogProps {
    /// An open dialog showing `children` at the standard width and automatic height.
    #[must_use]
    pub fn new(children: Vec<Node>) -> Self {
        Self {
            is_open: true,
            on_close: None,
            children,
            width: "min(600px, 90vw)".into(),
            height: "auto".into(),
            class_name: String::new(),
            backdrop_class_name: String::new(),
        }
    }
}
/// Native listener retained with a mounted dialog.
type Listener = EventSubscription<Closure<dyn Fn(Event)>>;
/// Listeners that live exactly as long as the mounted dialog.
struct Mounted {
    /// Backdrop, panel, Close and document Escape registrations.
    _listeners: Vec<Listener>,
}
/// Mount an open dialog as a backdrop holding its panel, or nothing when closed.
///
/// The returned root owns the document Escape listener, so dropping it releases that route.
/// # Errors
/// Returns a localization, icon or native failure.
pub fn dialog(view: &DomView, props: &DialogProps) -> Result<Option<DomRoot>, ToolUiError> {
    if !props.is_open {
        return Ok(None);
    }
    let document = &view.document;
    let backdrop = element(
        document,
        "div",
        &classes("maestro-dialog-backdrop", &props.backdrop_class_name),
    )?;
    let panel = element(
        document,
        "div",
        &classes("maestro-dialog", &props.class_name),
    )?;
    for (name, value) in [
        ("role", "dialog"),
        ("aria-modal", "true"),
        (
            "style",
            &format!("width: {}; height: {};", props.width, props.height),
        ),
    ] {
        panel
            .set_attribute(name, value)
            .map_err(|e| host_error(&e))?;
    }
    for child in &props.children {
        panel.append_child(child).map_err(|e| host_error(&e))?;
    }
    name_panel(&panel)?;
    let close = element(document, "button", "maestro-dialog-close")?;
    close
        .set_attribute("type", "button")
        .map_err(|e| host_error(&e))?;
    close
        .set_attribute("aria-label", &view.i18n.text("Close")?)
        .map_err(|e| host_error(&e))?;
    let glyph = icon(document, "X", "maestro-dialog-close-glyph")?;
    append(&close, &glyph)?;
    append(&panel, &close)?;
    append(&backdrop, &panel)?;
    let listeners = listeners(document, &backdrop, &panel, &close, props.on_close.as_ref())?;
    Ok(Some(DomRoot {
        node: backdrop.into(),
        owner: Some(Box::new(Mounted {
            _listeners: listeners,
        })),
    }))
}
/// Name the panel by its first heading, so assistive technology announces the title.
fn name_panel(panel: &Element) -> Result<(), ToolUiError> {
    if let Some(heading) = panel.query_selector("h2").map_err(|e| host_error(&e))? {
        if heading.id().is_empty() {
            heading.set_id(&unique_id("maestro-dialog-title"));
        }
        panel
            .set_attribute("aria-labelledby", &heading.id())
            .map_err(|e| host_error(&e))?;
    }
    Ok(())
}
/// Register the close routes; each delivers `on_close` once per request.
fn listeners(
    document: &Document,
    backdrop: &Element,
    panel: &Element,
    close: &Element,
    on_close: Option<&Rc<dyn Fn()>>,
) -> Result<Vec<Listener>, ToolUiError> {
    let notify = {
        let on_close = on_close.cloned();
        move || {
            if let Some(callback) = &on_close {
                callback();
            }
        }
    };
    let outside = {
        let notify = notify.clone();
        let backdrop = backdrop.clone();
        Closure::<dyn Fn(Event)>::new(move |event: Event| {
            if event
                .target()
                .is_some_and(|target| backdrop.is_same_node(target.dyn_ref()))
            {
                notify();
            }
        })
    };
    let contained = Closure::<dyn Fn(Event)>::new(|event: Event| event.stop_propagation());
    let button = {
        let notify = notify.clone();
        Closure::<dyn Fn(Event)>::new(move |_: Event| notify())
    };
    let escape = Closure::<dyn Fn(Event)>::new(move |event: Event| {
        if event
            .dyn_ref::<KeyboardEvent>()
            .is_some_and(|key| key.key() == "Escape")
        {
            notify();
        }
    });
    [
        (backdrop.clone().into(), "click", outside),
        (panel.clone().into(), "click", contained),
        (close.clone().into(), "click", button),
        (document.clone().into(), "keydown", escape),
    ]
    .into_iter()
    .map(|(target, name, callback)| {
        EventSubscription::new(target, name, callback).map_err(|e| host_error(&e).into())
    })
    .collect()
}
/// Literal heading and optional description operands.
#[derive(Clone, Default)]
pub struct DialogHeaderProps {
    /// Heading text, including an empty heading.
    pub title: String,
    /// Description below the heading; absent or empty omits it.
    pub description: Option<String>,
    /// Wrapper classes appended after the header class.
    pub class_name: String,
}
/// A heading with its optional description; text is never parsed as markup.
/// # Errors
/// Returns a native failure.
pub fn dialog_header(
    document: &Document,
    props: &DialogHeaderProps,
) -> Result<Element, ToolUiError> {
    let root = element(
        document,
        "div",
        &classes("maestro-dialog-header", &props.class_name),
    )?;
    let title = element(document, "h2", "")?;
    title.set_text_content(Some(&props.title));
    append(&root, &title)?;
    if let Some(description) = props.description.as_deref().filter(|text| !text.is_empty()) {
        title.set_class_name("maestro-dialog-described");
        let text = element(document, "p", "")?;
        text.set_text_content(Some(description));
        append(&root, &text)?;
    }
    Ok(root)
}
/// The padded content column holding `children` in order.
/// # Errors
/// Returns a native failure.
pub fn dialog_content(
    document: &Document,
    children: &[Node],
    class_name: &str,
) -> Result<Element, ToolUiError> {
    wrap(document, "maestro-dialog-content", children, class_name)
}
/// The action row holding `children` in order.
/// # Errors
/// Returns a native failure.
pub fn dialog_footer(
    document: &Document,
    children: &[Node],
    class_name: &str,
) -> Result<Element, ToolUiError> {
    wrap(document, "maestro-dialog-footer", children, class_name)
}
/// A classed wrapper holding `children` in order.
fn wrap(
    document: &Document,
    base: &str,
    children: &[Node],
    class_name: &str,
) -> Result<Element, ToolUiError> {
    let root = element(document, "div", &classes(base, class_name))?;
    for child in children {
        root.append_child(child).map_err(|e| host_error(&e))?;
    }
    Ok(root)
}
