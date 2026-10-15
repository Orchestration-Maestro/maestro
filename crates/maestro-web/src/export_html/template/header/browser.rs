//! Native literal session identity composition.
use crate::browser::attachment_content::{AttachmentUiError, ReadingRoot, browser::host_error};
use web_sys::Document;
/// Mount owned identity text without interpreting markup.
pub(super) fn mount(document: &Document, title: &str) -> Result<ReadingRoot, AttachmentUiError> {
    let root = document
        .create_element("section")
        .map_err(|e| host_error(&e))?;
    root.set_attribute("class", "maestro-session-header")
        .map_err(|e| host_error(&e))?;
    let style = document
        .create_element("style")
        .map_err(|e| host_error(&e))?;
    style.set_text_content(Some(include_str!("../header.css")));
    root.append_child(&style).map_err(|e| host_error(&e))?;
    let heading = document.create_element("h1").map_err(|e| host_error(&e))?;
    heading.set_text_content(Some(title));
    root.append_child(&heading).map_err(|e| host_error(&e))?;
    Ok(ReadingRoot { element: root })
}
