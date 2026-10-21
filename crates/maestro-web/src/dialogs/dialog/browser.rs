//! Native literal content and root-owned dialog semantics.
use crate::browser::attachment_content::{AttachmentHostError, ReadingRoot, browser::host_error};
use web_sys::{Document, Element};
/// Set literal classes without parsing caller text as markup.
fn root(document: &Document, kind: &str, classes: &str) -> Result<Element, AttachmentHostError> {
    let element = document.create_element("div").map_err(|e| host_error(&e))?;
    element
        .set_attribute("class", &format!("{kind} {classes}"))
        .map_err(|e| host_error(&e))?;
    crate::browser::install_stylesheet(
        document,
        "maestro-dialog-style",
        include_str!("../dialog.css"),
    )?;
    Ok(element)
}
/// Mount a native heading even when its title is empty.
pub(super) fn header(
    document: &Document,
    props: &DialogHeaderProps,
) -> Result<ReadingRoot, AttachmentHostError> {
    let element = root(document, "maestro-dialog-header", &props.class_name)?;
    let title = document.create_element("h2").map_err(|e| host_error(&e))?;
    title.set_text_content(Some(&props.title));
    element.append_child(&title).map_err(|e| host_error(&e))?;
    if let Some(description) = props.description.as_ref().filter(|s| !s.is_empty()) {
        title
            .set_attribute("class", "maestro-dialog-described")
            .map_err(|e| host_error(&e))?;
        let text = document.create_element("p").map_err(|e| host_error(&e))?;
        text.set_text_content(Some(description));
        element.append_child(&text).map_err(|e| host_error(&e))?;
    }
    Ok(ReadingRoot {
        frame_style: "",
        decoration: None,
        element,
        slots: vec![],
        commit: None,
        owner: None,
    })
}
/// Compose whole mounted roots, retaining every child's lifetime in the one owner.
pub(super) fn content(
    document: &Document,
    props: &DialogContentProps,
) -> Result<ReadingRoot, AttachmentHostError> {
    let element = root(document, "maestro-dialog-content", &props.class_name)?;
    let mut slots = Vec::new();
    let mut owners = Vec::new();
    for child in &props.children {
        let child = (child.mount)(document)?;
        element
            .append_child(&child.element)
            .map_err(|e| host_error(&e))?;
        slots.extend(child.slots);
        owners.push(child.owner);
    }
    Ok(ReadingRoot {
        frame_style: "",
        decoration: None,
        element,
        slots,
        commit: None,
        owner: Some(Box::new(owners)),
    })
}

use crate::{
    browser::{attachment_content::ReadingContent, event_subscription::EventSubscription},
    dialogs::dialog_base::State,
};
use std::{
    cell::Cell,
    rc::{Rc, Weak},
};
use wasm_bindgen::{JsCast as _, closure::Closure};
/// Weak aliases follow the existing candidate/publication transaction.
#[derive(Default)]
pub(crate) struct Native {
    /// Original factory and its single semantic wrapper.
    factory: Option<(Rc<ReadingContent>, Rc<ReadingContent>)>,
    /// Alias to the root currently being measured.
    candidate: Weak<Semantic>,
    /// Alias published only after successful placement.
    committed: Weak<Semantic>,
    /// Selected localization for newly mounted candidates.
    close_name: String,
    /// Literal caller classes for initial and retained native roots.
    pub(in crate::dialogs) class_name: Rc<str>,
}
/// Non-painting native state retained by the mounted root only.
struct Semantic {
    /// Native panel content, not a second shell.
    root: Element,
    /// Native keyboard and accessibility target.
    button: Element,
    /// Native focus reflected by the canvas painter.
    focused: Cell<bool>,
}
/// Native event callback retained by the shared subscription owner.
type SemanticSubscription = EventSubscription<Closure<dyn FnMut(web_sys::Event)>>;
/// Root-specific subscriptions; a retained factory cannot retain this value.
struct Mounted {
    /// Semantic element lifetime.
    _semantic: Rc<Semantic>,
    /// Existing event-registration owners.
    _listeners: Vec<SemanticSubscription>,
}
/// Wrap a genuinely new reading factory once, preserving unchanged root identity.
pub(super) fn prepare(state: &Rc<State>, reading: &Rc<ReadingContent>) -> Rc<ReadingContent> {
    let cached = state
        .native
        .borrow()
        .factory
        .as_ref()
        .filter(|(old, _)| Rc::ptr_eq(old, reading))
        .map(|(_, content)| Rc::clone(content));
    if let Some(content) = cached {
        return content;
    }
    let weak = Rc::downgrade(state);
    let source = Rc::clone(reading);
    let content = Rc::new(ReadingContent {
        mount: Rc::new(move |document| mount(document, &source, &weak)),
    });
    let old = state
        .native
        .borrow_mut()
        .factory
        .replace((Rc::clone(reading), Rc::clone(&content)));
    drop(old);
    content
}
/// Mount child content before registering one semantic Close owner.
fn mount(
    document: &Document,
    source: &ReadingContent,
    state: &Weak<State>,
) -> Result<ReadingRoot, AttachmentHostError> {
    let child = (source.mount)(document)?;
    let element = root(document, "maestro-dialog-native", "")?;
    element
        .append_child(&child.element)
        .map_err(|e| host_error(&e))?;
    let button = document
        .create_element("button")
        .map_err(|e| host_error(&e))?;
    button
        .set_attribute("type", "button")
        .map_err(|e| host_error(&e))?;
    button
        .set_attribute("class", "maestro-dialog-semantic-close")
        .map_err(|e| host_error(&e))?;
    element.append_child(&button).map_err(|e| host_error(&e))?;
    let semantic = Rc::new(Semantic {
        root: element.clone(),
        button,
        focused: Cell::new(false),
    });
    if let Some(owner) = state.upgrade() {
        let classes = Rc::clone(&owner.native.borrow().class_name);
        let name = owner.native.borrow().close_name.clone();
        update(&semantic, &name, &classes)?;
    }
    let mut listeners = vec![escape_listener(document, state)?];
    for name in ["click", "focus", "blur", "keydown"] {
        let owner = state.clone();
        let target = Rc::downgrade(&semantic);
        let handler = Closure::wrap(
            Box::new(move |event| dispatch(&owner, &target, name, event))
                as Box<dyn FnMut(web_sys::Event)>,
        );
        listeners.push(
            EventSubscription::new(semantic.button.clone().into(), name, handler)
                .map_err(|e| host_error(&e))?,
        );
    }
    if let Some(state) = state.upgrade() {
        state.native.borrow_mut().candidate = Rc::downgrade(&semantic);
    }
    Ok(ReadingRoot {
        frame_style: "",
        decoration: None,
        element,
        slots: child.slots,
        commit: None,
        owner: Some(Box::new((
            child.owner,
            Mounted {
                _semantic: semantic,
                _listeners: listeners,
            },
        ))),
    })
}
/// Retain bubbling window Escape with the mounted reading root.
fn escape_listener(
    document: &Document,
    owner: &Weak<State>,
) -> Result<SemanticSubscription, AttachmentHostError> {
    let owner = owner.clone();
    let handler = Closure::wrap(Box::new(move |event: web_sys::Event| {
        if let Some(owner) = owner.upgrade()
            && owner.props.borrow().is_open
            && let Ok(event) = event.dyn_into()
        {
            owner
                .context
                .host()
                .native_key(&event, owner.context.drawing());
        }
    }) as Box<dyn FnMut(web_sys::Event)>);
    let window = document
        .default_view()
        .ok_or_else(|| AttachmentHostError::Host("dialog document has no window".into()))?;
    EventSubscription::new(window.into(), "keydown", handler).map_err(|e| host_error(&e))
}
/// Forward native activation and focus through weak existing owners.
fn dispatch(owner: &Weak<State>, target: &Weak<Semantic>, name: &str, event: web_sys::Event) {
    let (Some(owner), Some(target)) = (owner.upgrade(), target.upgrade()) else {
        return;
    };
    if !owner.props.borrow().is_open {
        return;
    }
    match name {
        "click" => {
            if let Err(error) = owner.close(true) {
                wasm_bindgen::throw_str(&error.to_string());
            }
        }
        "keydown" => {
            if let Ok(event) = event.dyn_into() {
                owner
                    .context
                    .host()
                    .native_key(&event, owner.context.drawing());
            }
        }
        "focus" | "blur" => {
            target.focused.set(name == "focus");
            owner.context.drawing().request_repaint();
        }
        _ => {}
    }
}
/// Select the actual staged root, falling back to the retained committed root.
fn target(state: &State) -> Option<Rc<Semantic>> {
    let native = state.native.borrow();
    native
        .candidate
        .upgrade()
        .or_else(|| native.committed.upgrade())
}
/// Update only literal caller classes and the live localized accessible name.
pub(super) fn configure(state: &State, name: &str) -> Result<(), AttachmentHostError> {
    let classes = {
        let mut native = state.native.borrow_mut();
        native.close_name = name.into();
        Rc::clone(&native.class_name)
    };
    if let Some(target) = target(state) {
        update(&target, name, &classes)?;
    }
    Ok(())
}
/// Publish only the alias; the existing reading root remains the sole strong owner.
pub(super) fn commit(state: &State) {
    let mut native = state.native.borrow_mut();
    if native.candidate.strong_count() > 0 {
        native.committed = std::mem::take(&mut native.candidate);
    }
}
/// Native focus is read live, without retaining the root from the dialog handle.
pub(super) fn focused(state: &State) -> bool {
    target(state).is_some_and(|target| target.focused.get())
}

/// Keep initial and live native attributes on one forwarding path.
fn update(target: &Semantic, name: &str, classes: &str) -> Result<(), AttachmentHostError> {
    target
        .root
        .set_attribute("class", &format!("maestro-dialog-native {classes}"))
        .map_err(|e| host_error(&e))?;
    target
        .button
        .set_attribute("aria-label", name)
        .map_err(|e| host_error(&e))
}

/// Literal heading and optional description operands.
pub struct DialogHeaderProps {
    /// Heading text, including an empty heading.
    pub title: String,
    /// Only absent or empty descriptions omit the paragraph.
    pub description: Option<String>,
    /// Literal wrapper classes.
    pub class_name: String,
}
/// Ordered native content operands.
pub struct DialogContentProps {
    /// Owned arbitrary native children.
    pub children: Vec<Rc<ReadingContent>>,
    /// Literal wrapper classes.
    pub class_name: String,
}
/// Retain a literal native heading factory.
#[must_use]
pub fn dialog_header(props: DialogHeaderProps) -> Rc<ReadingContent> {
    Rc::new(ReadingContent {
        mount: Rc::new(move |document| header(document, &props)),
    })
}
/// Retain ordered mixed native children with their mounted guards.
#[must_use]
pub fn dialog_content(props: DialogContentProps) -> Rc<ReadingContent> {
    Rc::new(ReadingContent {
        mount: Rc::new(move |document| content(document, &props)),
    })
}
