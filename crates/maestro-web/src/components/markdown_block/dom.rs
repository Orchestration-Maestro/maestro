//! Markdown mounted as native page content and as the `markdown-block` element.
use super::{MarkdownBlock, render};
use crate::{
    browser::attachment_content::{
        AttachmentHostError,
        browser::{append, element, host_error},
    },
    components::code_block::{Prepared as Code, dom::Listing},
    dom::{DomContent, DomRoot, DomView, MountedContent, Property, Reflected},
    tools::types::ToolUiError,
};
use js_sys::WeakRef;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue};
use web_sys::Element;

/// Element tag of the Markdown block.
pub(crate) const TAG: &str = "markdown-block";
/// Properties reflected onto the observed attributes.
const PROPERTIES: &[Property] = &[
    Property::Text("content", "content"),
    Property::Flag("isThinking", "is-thinking"),
    Property::Unset("escapeHtml", "unescaped-html"),
];

/// The published document's source and its listings, retained by the mounted root.
struct Shown {
    /// Source and escaping the document was prepared from.
    key: (String, bool),
    /// Listing sources in document order.
    sources: Rc<[render::Code]>,
    /// Mounted listings in document order.
    listings: Vec<Listing>,
}
/// Root state: the last successfully published document, if any.
type Published = RefCell<Option<Shown>>;

impl DomContent for MarkdownBlock {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        crate::browser::math_assets::install(&view.document)?;
        let published: Published = RefCell::new(None);
        let root = DomRoot {
            node: element(&view.document, "div", "maestro-markdown")?.into(),
            owner: Some(Box::new(published)),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    /// Republish changed source; otherwise refresh listings in place, keeping their nodes and selection.
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let published = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Published>())
            .ok_or_else(|| ToolUiError::Presentation("Missing Markdown state".into()))?;
        let node: &Element = root.node.unchecked_ref();
        let title = view.i18n.text("Copy code")?;
        let unchanged = published
            .borrow()
            .as_ref()
            .filter(|shown| shown.key.0 == self.content && shown.key.1 == self.escape_html)
            .map(|shown| Rc::clone(&shown.sources));
        if let Some(sources) = unchanged {
            let code = highlight(view, &sources)?;
            let shown = published.borrow();
            let listings = shown.as_ref().map_or(&[][..], |shown| &shown.listings);
            for (listing, code) in listings.iter().zip(&code) {
                listing.update(code, &title)?;
            }
        } else {
            let shown = self.publish(node, view, &title)?;
            *published.borrow_mut() = Some(shown);
        }
        node.set_class_name(if self.is_thinking {
            "maestro-markdown maestro-thinking"
        } else {
            "maestro-markdown"
        });
        Ok(())
    }
}
impl MarkdownBlock {
    /// Prepare the whole document detached, then replace the root's children with it.
    fn publish(&self, root: &Element, view: &DomView, title: &str) -> Result<Shown, ToolUiError> {
        let mut prepared = render::prepare(&self.content, self.escape_html);
        let sources: Rc<[render::Code]> = std::mem::take(&mut prepared.code).into();
        let code = highlight(view, &sources)?;
        let document = element(&view.document, "div", "")?;
        document.set_inner_html(&prepared.html);
        let slots = document
            .query_selector_all(&format!("[{}]", prepared.slot_attribute))
            .map_err(|error| host_error(&error))?;
        if slots.length() as usize != code.len() {
            return Err(AttachmentHostError::Host("Missing code slot".into()).into());
        }
        // One enumeration of the generated slots, walked in document order.
        let listings = js_sys::Array::from(&slots)
            .iter()
            .zip(&code)
            .map(|(slot, code)| {
                let slot: Element = slot.unchecked_into();
                slot.set_attribute("class", "maestro-code-wrap")
                    .map_err(|error| host_error(&error))?;
                let (node, listing) = Listing::build(&view.document, code, title)?;
                append(&slot, &node)?;
                Ok(listing)
            })
            .collect::<Result<Vec<_>, AttachmentHostError>>()?;
        root.replace_children_with_node(&js_sys::Array::from(&document.child_nodes()));
        Ok(Shown {
            key: (self.content.clone(), self.escape_html),
            sources,
            listings,
        })
    }
    /// Define the `markdown-block` element once for the page.
    /// # Errors
    /// Returns the browser's registration failure.
    pub(crate) fn define(view: &Rc<DomView>) -> Result<(), ToolUiError> {
        let view = Rc::clone(view);
        let observed = ["content", "is-thinking", "unescaped-html"].map(String::from);
        crate::dom::define_element(TAG, observed.into(), PROPERTIES, move |root, properties| {
            MarkdownElement {
                root: WeakRef::new(&root),
                view: Rc::clone(&view),
                shown: MountedContent::default(),
                _properties: properties,
            }
        })
        .map_err(|error| host_error(&error).into())
    }
}
/// Highlight every listing with the supplied engine before anything is published.
fn highlight(view: &DomView, sources: &[render::Code]) -> Result<Vec<Code>, ToolUiError> {
    sources
        .iter()
        .map(|code| Code::new(&*view.highlighter, code.code.clone(), code.language.clone()))
        .collect::<Result<_, _>>()
        .map_err(ToolUiError::reading)
}

/// One `markdown-block` element showing its attributes' document.
struct MarkdownElement {
    /// Element owning this state.
    root: WeakRef,
    /// Page operands.
    view: Rc<DomView>,
    /// The mounted document.
    shown: MountedContent,
    /// Accessors of the reflected properties.
    _properties: Reflected,
}
impl custom_element::CustomElement for MarkdownElement {
    fn connected_callback(&mut self) {
        // Lifecycle callbacks have no error channel; a failed render keeps the previous document.
        let _ = self.render();
    }
    fn attribute_changed_callback(&mut self, _: &str, _: JsValue, _: JsValue) {
        if crate::dom::connected(&self.root).is_some() {
            let _ = self.render();
        }
    }
}
impl MarkdownElement {
    /// Show the attributes' document inside the element.
    fn render(&mut self) -> Result<(), ToolUiError> {
        let Some(root) = crate::dom::connected(&self.root) else {
            return Ok(());
        };
        let mut block = MarkdownBlock::default();
        block.content = root.get_attribute("content").unwrap_or_default();
        block.is_thinking = root.has_attribute("is-thinking");
        block.escape_html = !root.has_attribute("unescaped-html");
        self.shown.show(&block, &root, &self.view)
    }
}
