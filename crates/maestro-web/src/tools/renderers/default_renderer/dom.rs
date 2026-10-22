//! Page-content presentation of the default input and output listings.
use super::{Listing, prepare};
use crate::{
    browser::attachment_content::browser::{element, host_error},
    dom::{DomContent, DomView},
    tools::{renderer_registry::ToolHeaderState, types::ToolUiError},
};
use wasm_bindgen::JsCast as _;
use web_sys::{Element, Node};

/// Call-owned default content.
pub(super) struct DefaultDomContent {
    /// Status selected before formatting input.
    pub(super) state: ToolHeaderState,
    /// Optional already-formatted input.
    pub(super) params: Option<String>,
    /// Raw joined output; absence means no result yet.
    pub(super) output: Option<String>,
}
impl DomContent for DefaultDomContent {
    fn mount(&self, view: &DomView) -> Result<Node, ToolUiError> {
        let root = element(&view.document, "div", "maestro-tool-default")?;
        self.update(&root, view)?;
        Ok(root.into())
    }
    fn update(&self, root: &Node, view: &DomView) -> Result<(), ToolUiError> {
        let no_output = view.i18n.text("(no output)")?;
        let prepared = prepare(
            self.params.as_deref(),
            self.output.as_deref(),
            self.state,
            &no_output,
        )?;
        let header = crate::tools::renderer_registry::header::dom::render_header(
            &view.document,
            self.state,
            "Code",
            prepared.caption,
        )?;
        let children = js_sys::Array::of1(&header);
        for (label, listing) in [("Input", &prepared.input), ("Output", &prepared.output)] {
            if let Some(listing) = listing {
                let listed = section(view, &view.i18n.text(label)?, listing)?;
                children.push(&listed);
            }
        }
        root.unchecked_ref::<Element>()
            .replace_children_with_node(&children);
        Ok(())
    }
}
/// One labelled listing.
fn section(view: &DomView, label: &str, listing: &Listing) -> Result<Element, ToolUiError> {
    let document = &view.document;
    let section = document
        .create_element("div")
        .map_err(|error| host_error(&error))?;
    let caption = element(document, "div", "maestro-tool-section-label")?;
    caption.set_text_content(Some(label));
    let code = document
        .create_element(crate::components::code_block::dom::TAG)
        .map_err(|error| host_error(&error))?;
    for (name, value) in [
        ("code", listing.code.as_str()),
        ("language", listing.language),
    ] {
        code.set_attribute(name, value)
            .map_err(|error| host_error(&error))?;
    }
    section
        .append_with_node_2(&caption, &code)
        .map_err(|error| host_error(&error))?;
    Ok(section)
}
