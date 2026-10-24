//! Page-content presentation of the default input and output listings.
use super::{DefaultRenderer, Listing, format_params, prepare, status};
use crate::{
    browser::attachment_content::browser::{element, host_error},
    page::{DomContent, DomRoot, DomView},
    tools::{
        renderer_registry::{ToolHeaderState, get_text_output},
        types::{ToolRenderResult, ToolRenderer, ToolResultMessageView, ToolUiError},
    },
};
use base64::Engine as _;
use serde_json::Value;
use wasm_bindgen::JsCast as _;
use web_sys::Element;

/// Retain formatted inputs and raw output as page content.
pub(crate) fn render_prepared(
    state: ToolHeaderState,
    params: Option<String>,
    output: Option<String>,
) -> ToolRenderResult<Box<dyn DomContent>> {
    ToolRenderResult {
        content: Box::new(DefaultDomContent {
            state,
            params,
            output,
        }),
        is_custom: false,
    }
}
impl<D> ToolRenderer<Value, D, Box<dyn DomContent>> for DefaultRenderer {
    fn render(
        &self,
        params: Option<&Value>,
        result: Option<ToolResultMessageView<'_, D>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult<Box<dyn DomContent>>, ToolUiError> {
        let state = status(result.as_ref(), is_streaming);
        let params = params.map(format_params).transpose()?.flatten();
        let output = result.as_ref().map(|result| get_text_output(Some(result)));
        Ok(render_prepared(state, params, output))
    }
}
/// Call-owned default content.
struct DefaultDomContent {
    /// Status selected before formatting input.
    state: ToolHeaderState,
    /// Optional already-formatted input.
    params: Option<String>,
    /// Raw joined output; absence means no result yet.
    output: Option<String>,
}
impl DomContent for DefaultDomContent {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let root = DomRoot::new(element(&view.document, "div", "maestro-tool-default")?);
        self.update(&root, view)?;
        Ok(root)
    }
    /// Replace the header and change the existing listings in place, keeping selection and copy feedback.
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let root: &Element = root.node.unchecked_ref();
        let no_output = view.i18n.text("(no output)")?;
        let prepared = prepare(
            self.params.as_deref(),
            self.output.as_deref(),
            self.state,
            &no_output,
        )?;
        let header = crate::tools::renderer_registry::heading::dom::render_header(
            &view.document,
            self.state,
            "Code",
            prepared.caption,
        )?;
        match root.first_element_child() {
            Some(old) => root.replace_child(&header, &old),
            None => root.append_child(&header),
        }
        .map_err(|error| host_error(&error))?;
        // Input directly follows the header; output closes the content.
        for (name, label, listing, after_header) in [
            ("input", "Input", &prepared.input, true),
            ("output", "Output", &prepared.output, false),
        ] {
            match (find(root, name)?, listing) {
                (Some(section), Some(listing)) => show_listing(&section, listing)?,
                (Some(section), None) => section.remove(),
                (None, Some(listing)) => {
                    let section = section(view, name, &view.i18n.text(label)?, listing)?;
                    let anchor = after_header.then(|| header.next_sibling()).flatten();
                    root.insert_before(&section, anchor.as_ref())
                        .map_err(|error| host_error(&error))?;
                }
                (None, None) => {}
            }
        }
        Ok(())
    }
}
/// The root's section named `name`, if shown.
fn find(root: &Element, name: &str) -> Result<Option<Element>, ToolUiError> {
    root.query_selector(&format!(":scope > [data-section={name}]"))
        .map_err(|error| ToolUiError::Host(host_error(&error)))
}
/// Point a section's listing at `listing`, encoded so its text always shows literally.
fn show_listing(section: &Element, listing: &Listing) -> Result<(), ToolUiError> {
    let code = section
        .query_selector(crate::components::source::dom::TAG)
        .map_err(|error| host_error(&error))?
        .ok_or_else(|| ToolUiError::Presentation("Missing listing".into()))?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&listing.code);
    for (name, value) in [("code", encoded.as_str()), ("language", listing.language)] {
        code.set_attribute(name, value)
            .map_err(|error| host_error(&error))?;
    }
    Ok(())
}
/// One labelled listing section.
fn section(
    view: &DomView,
    name: &str,
    label: &str,
    listing: &Listing,
) -> Result<Element, ToolUiError> {
    let document = &view.document;
    let section = document
        .create_element("div")
        .map_err(|error| host_error(&error))?;
    section
        .set_attribute("data-section", name)
        .map_err(|error| host_error(&error))?;
    let caption = element(document, "div", "maestro-tool-section-label")?;
    caption.set_text_content(Some(label));
    let code = document
        .create_element(crate::components::source::dom::TAG)
        .map_err(|error| host_error(&error))?;
    section
        .append_with_node_2(&caption, &code)
        .map_err(|error| host_error(&error))?;
    show_listing(&section, listing)?;
    Ok(section)
}
