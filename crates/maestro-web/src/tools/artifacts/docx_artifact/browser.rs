//! Native Word publication and document resource ownership.
use super::DocxArtifact;
use crate::{
    browser::attachment_content::browser::{set_text_if_changed, sync_text},
    dialogs::attachment_overlay::error,
    docx_preview::{Options, PageSizeOptions, RenderedDocument, render_async},
    dom::{DomRoot, DomView},
    tools::types::ToolUiError,
};
use std::rc::Rc;
use wasm_bindgen::JsCast as _;
use web_sys::Element;

/// Last processed operands and resources of one native body.
#[derive(Default)]
pub(super) struct Mounted {
    /// Last processed source, including empty assignments.
    source: String,
    /// Last processed assignment, also retained when rendering fails.
    assignment: Option<Rc<()>>,
    /// Whether the last rendering failed.
    failed: bool,
    /// Current error heading, localized without rebuilding its diagnostic.
    heading: Option<Element>,
    /// Live URLs and pending shape sizing of the successful document.
    rendered: Option<RenderedDocument>,
}
/// Reconcile a body, retaining successful equal-source nodes and retrying requested failures.
pub(super) fn update(
    artifact: &DocxArtifact,
    root: &DomRoot,
    view: &DomView,
    state: &mut Mounted,
) -> Result<(), ToolUiError> {
    let requested = state
        .assignment
        .as_ref()
        .is_none_or(|old| !Rc::ptr_eq(old, &artifact.assignment));
    if state.source == artifact.content && !(state.failed && requested) {
        if let Some(heading) = &state.heading {
            set_text_if_changed(heading, &view.i18n.text("Error loading document")?);
        }
        return Ok(());
    }
    state.source.clone_from(&artifact.content);
    state.assignment = Some(Rc::clone(&artifact.assignment));
    let node: &Element = root.node.unchecked_ref();
    if artifact.content.is_empty() {
        if state.failed {
            node.set_inner_html("");
            state.failed = false;
            state.heading = None;
        }
        return Ok(());
    }
    node.set_inner_html("");
    state.rendered = None;
    state.heading = None;
    match render(artifact, node, view) {
        Ok(rendered) => {
            state.failed = false;
            state.rendered = Some(rendered);
        }
        Err(error) => {
            state.failed = true;
            node.set_inner_html("");
            state.heading = Some(show_error(node, &error, view)?);
        }
    }
    Ok(())
}
/// Render with the artifact's page-width policy and the shared renderer's remaining defaults.
fn render(
    artifact: &DocxArtifact,
    node: &Element,
    view: &DomView,
) -> Result<RenderedDocument, ToolUiError> {
    let bytes = artifact.bytes()?;
    let wrapper = view
        .document
        .create_element("div")
        .map_err(|error| ToolUiError::native(&error))?;
    wrapper.set_class_name("maestro-docx-container");
    node.append_child(&wrapper)
        .map_err(|error| ToolUiError::native(&error))?;
    let options = Options {
        page_size: PageSizeOptions {
            ignore_width: true,
            ignore_height: false,
        },
        ..Options::default()
    };
    let (_, rendered) = render_async(&bytes, &wrapper, None, options).map_err(ToolUiError::Host)?;
    Ok(rendered)
}

/// Publish the shared literal error panel and return its live localized heading.
fn show_error(node: &Element, error: &ToolUiError, view: &DomView) -> Result<Element, ToolUiError> {
    let (panel, nodes) = error::nodes(&view.document, true).map_err(ToolUiError::Host)?;
    let values = [
        view.i18n.text("Error loading document")?,
        error::detail(&error.to_string(), &*view.i18n, "Failed to load document")?,
    ];
    sync_text(Some(&nodes), &values);
    node.append_child(&panel)
        .map_err(|error| ToolUiError::native(&error))?;
    let [heading, _] = nodes;
    Ok(heading)
}
