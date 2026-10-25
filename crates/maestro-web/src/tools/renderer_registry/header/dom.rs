//! Page-content status header.
use crate::{
    browser::attachment_content::browser::{element, host_error},
    tools::{renderer_registry::ToolHeaderState, types::ToolUiError},
};
use web_sys::{Document, Element};

/// Build a status header: the tool icon and caption, with a trailing loader while pending.
/// # Errors
/// Returns an icon or native creation failure.
pub(crate) fn render_header(
    document: &Document,
    state: ToolHeaderState,
    tool_icon: &str,
    text: &str,
) -> Result<Element, ToolUiError> {
    let status = match state {
        ToolHeaderState::Inprogress => "maestro-tool-icon",
        ToolHeaderState::Complete => "maestro-tool-icon maestro-tool-complete",
        ToolHeaderState::Error => "maestro-tool-icon maestro-tool-error",
    };
    let header = element(document, "div", "maestro-tool-header")?;
    let lead = element(document, "div", "maestro-tool-header-lead")?;
    let caption = document
        .create_element("span")
        .map_err(|error| host_error(&error))?;
    caption.set_text_content(Some(text));
    let tool = icon(document, tool_icon, status)?;
    lead.append_with_node_2(&tool, &caption)
        .map_err(|error| host_error(&error))?;
    header
        .append_child(&lead)
        .map_err(|error| host_error(&error))?;
    if state == ToolHeaderState::Inprogress {
        let spinner = icon(document, "Loader", "maestro-tool-icon maestro-tool-spinner")?;
        header
            .append_child(&spinner)
            .map_err(|error| host_error(&error))?;
    }
    Ok(header)
}
/// One small icon in a classed span.
fn icon(document: &Document, name: &str, class: &str) -> Result<Element, ToolUiError> {
    let node = crate::icons::icon_node(name)
        .map_err(|error| ToolUiError::Presentation(error.to_string()))?
        .ok_or_else(|| ToolUiError::Presentation(format!("Missing icon {name}")))?;
    let span = element(document, "span", class)?;
    let svg = crate::icons::icon_dom(&node, crate::icons::IconSize::Sm, None)
        .map_err(|error| host_error(&error))?;
    span.append_child(&svg)
        .map_err(|error| host_error(&error))?;
    Ok(span)
}
