//! Reasoning disclosure mounted as native page content and as the `thinking-block` element.
use super::ThinkingBlock;
use crate::{
    browser::{
        attachment_content::browser::{element, host_error},
        event_subscription::EventSubscription,
    },
    components::{
        button::{ButtonProps, ButtonSize, ButtonVariant, button},
        markdown_block::MarkdownBlock,
    },
    dom::{DomContent, DomRoot, DomView, Link, Property, Reflected},
    tools::types::ToolUiError,
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use wasm_bindgen::{JsValue, closure::Closure};
use web_sys::Element;

/// Element tag of the reasoning disclosure.
pub(crate) const TAG: &str = "thinking-block";
/// Literal header text.
const HEADER: &str = "Thinking...";
/// Properties reflected onto the observed attributes.
const PROPERTIES: &[Property] = &[
    Property::Text("content", "content"),
    Property::Flag("isStreaming", "is-streaming"),
];

/// Connected disclosure state, retained by its root and reached by the header's callback.
struct Disclosure {
    /// Page operands of the latest update, for a body the header expands.
    view: Rc<DomView>,
    /// Reasoning source forwarded to the body.
    content: String,
    /// Whether the body is shown.
    expanded: bool,
    /// Disclosure node.
    root: Element,
    /// Native toggle.
    header: Element,
    /// Literal header text, shimmering while streaming.
    label: Element,
    /// Body while expanded.
    body: Option<DomRoot>,
    /// Toggle subscription while the root is attached.
    click: Option<EventSubscription<Closure<dyn Fn()>>>,
}
/// Root state shared with the toggle's weak capture.
type Shared = Rc<RefCell<Disclosure>>;

impl DomContent for ThinkingBlock {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let document = &view.document;
        let chevron = element(document, "span", "maestro-thinking-chevron")?;
        let glyph = crate::icons::icon_node("ChevronRight")
            .map_err(|error| ToolUiError::Presentation(error.to_string()))?
            .ok_or_else(|| ToolUiError::Presentation("Missing glyph: ChevronRight".into()))?;
        let svg = crate::icons::icon_dom(&glyph, crate::icons::IconSize::Sm, None)
            .map_err(|error| host_error(&error))?;
        chevron
            .append_child(&svg)
            .map_err(|error| host_error(&error))?;
        let label = element(document, "span", "")?;
        label.set_text_content(Some(HEADER));
        let props = ButtonProps {
            variant: ButtonVariant::Ghost,
            size: ButtonSize::Sm,
            class_name: "maestro-thinking-header maestro-text-action",
            ..ButtonProps::default()
        };
        let header = button(document, &props, &[chevron.into(), label.clone().into()])
            .map_err(|error| host_error(&error))?;
        let root = element(document, "div", "maestro-thinking-block")?;
        root.append_child(&header)
            .map_err(|error| host_error(&error))?;
        let shared: Shared = Rc::new(RefCell::new(Disclosure {
            view: Rc::new(view.clone()),
            content: String::new(),
            expanded: false,
            root: root.clone(),
            header: header.into(),
            label,
            body: None,
            click: None,
        }));
        let mounted = DomRoot {
            node: root.into(),
            owner: Some(Box::new(shared)),
        };
        self.update(&mounted, view)?;
        Ok(mounted)
    }
    /// Forward content and streaming state, keeping this root's expansion.
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let shared = disclosure(root)?;
        {
            let mut state = shared.borrow_mut();
            state.view = Rc::new(view.clone());
            self.content.clone_into(&mut state.content);
            state.label.set_class_name(if self.is_streaming {
                "maestro-shimmer"
            } else {
                ""
            });
            if state.click.is_none() {
                let weak = Rc::downgrade(shared);
                let toggle = Closure::<dyn Fn()>::new(move || toggle(&weak));
                state.click = Some(
                    EventSubscription::new(state.header.clone().into(), "click", toggle)
                        .map_err(|error| host_error(&error))?,
                );
            }
        }
        show(shared)
    }
}
impl ThinkingBlock {
    /// Define the `thinking-block` element once for the page.
    /// # Errors
    /// Returns the browser's registration failure.
    pub(crate) fn define(view: &Rc<DomView>) -> Result<(), ToolUiError> {
        let view = Rc::clone(view);
        let observed = ["content", "is-streaming"].map(String::from);
        crate::dom::define_element(TAG, observed.into(), PROPERTIES, move |root, properties| {
            ThinkingElement {
                root: Link::new(&root),
                view: Rc::clone(&view),
                shown: None,
                _properties: properties,
            }
        })
        .map_err(|error| host_error(&error).into())
    }
}
/// The disclosure state a mounted root owns.
fn disclosure(root: &DomRoot) -> Result<&Shared, ToolUiError> {
    root.owner
        .as_ref()
        .and_then(|owner| owner.downcast_ref::<Shared>())
        .ok_or_else(|| ToolUiError::Presentation("Missing disclosure state".into()))
}
/// Flip the expansion of a still-retained disclosure.
fn toggle(weak: &Weak<RefCell<Disclosure>>) {
    let Some(shared) = weak.upgrade() else {
        return;
    };
    let expanded = !shared.borrow().expanded;
    shared.borrow_mut().expanded = expanded;
    // Event callbacks have no error channel; a failed body keeps the previous one.
    let _ = show(&shared);
}
/// Present the expansion, mounting, updating or removing the body outside the state borrow.
fn show(shared: &Shared) -> Result<(), ToolUiError> {
    let (expanded, view, content, root, body) = {
        let mut state = shared.borrow_mut();
        let expanded = state.expanded;
        state.root.set_class_name(if expanded {
            "maestro-thinking-block maestro-thinking-expanded"
        } else {
            "maestro-thinking-block"
        });
        state
            .header
            .set_attribute("aria-expanded", if expanded { "true" } else { "false" })
            .map_err(|error| host_error(&error))?;
        let view = Rc::clone(&state.view);
        (
            expanded,
            view,
            if expanded {
                state.content.clone()
            } else {
                String::new()
            },
            state.root.clone(),
            state.body.take(),
        )
    };
    if !expanded {
        if let Some(body) = body {
            root.remove_child(&body.node)
                .map_err(|error| host_error(&error))?;
        }
        return Ok(());
    }
    let mut markdown = MarkdownBlock::default();
    markdown.content = content;
    markdown.is_thinking = true;
    let result = match &body {
        Some(body) => markdown.update(body, &view).map(|()| None),
        None => markdown.mount(&view).map(Some),
    };
    let body = match result {
        Ok(Some(mounted)) => {
            root.append_child(&mounted.node)
                .map_err(|error| host_error(&error))?;
            Some(mounted)
        }
        Ok(None) => body,
        Err(error) => {
            shared.borrow_mut().body = body;
            return Err(error);
        }
    };
    shared.borrow_mut().body = body;
    Ok(())
}

/// One `thinking-block` element showing its attributes' disclosure.
struct ThinkingElement {
    /// Element owning this state.
    root: Link,
    /// Page operands.
    view: Rc<DomView>,
    /// The mounted disclosure, kept with its expansion while the element is detached.
    shown: Option<DomRoot>,
    /// Accessors of the reflected properties.
    _properties: Reflected,
}
impl custom_element::CustomElement for ThinkingElement {
    fn connected_callback(&mut self) {
        // Lifecycle callbacks have no error channel; a failed render keeps the previous disclosure.
        let _ = self.render();
    }
    fn disconnected_callback(&mut self) {
        if let Some(shared) = self.shown.as_ref().and_then(|root| disclosure(root).ok()) {
            let released = shared.borrow_mut().click.take();
            drop(released);
        }
    }
    fn attribute_changed_callback(&mut self, _: &str, _: JsValue, _: JsValue) {
        if crate::dom::connected(&self.root).is_some() {
            let _ = self.render();
        }
    }
}
impl ThinkingElement {
    /// Show the attributes' disclosure inside the element, reattaching its toggle.
    fn render(&mut self) -> Result<(), ToolUiError> {
        let Some(element) = crate::dom::connected(&self.root) else {
            return Ok(());
        };
        let block = ThinkingBlock {
            content: element.get_attribute("content").unwrap_or_default(),
            is_streaming: element.has_attribute("is-streaming"),
        };
        if let Some(root) = &self.shown {
            return block.update(root, &self.view);
        }
        let root = block.mount(&self.view)?;
        element.replace_children_with_node_1(&root.node);
        self.shown = Some(root);
        Ok(())
    }
}
