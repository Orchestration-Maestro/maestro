//! Native control rows shared by every artifact header.
use crate::{
    components::download_button::dom::{DownloadButtonProps, DownloadContent, download_button},
    dom::{DomRoot, DomView},
    tools::types::ToolUiError,
};
use js_sys::Uint8Array;

/// Control row of one independent artifact header.
const ROW: &str = "maestro-artifact-header";

/// Create an empty header row.
/// # Errors
/// Returns the native creation failure.
pub(crate) fn row(view: &DomView) -> Result<web_sys::Element, ToolUiError> {
    let row = view
        .document
        .create_element("div")
        .map_err(|error| ToolUiError::native(&error))?;
    row.set_class_name(ROW);
    Ok(row)
}
/// Build a header row holding one download control for the caller's complete original bytes.
/// # Errors
/// Returns native creation or localization failures.
pub(crate) fn download(
    view: &DomView,
    filename: &str,
    mime: &str,
    bytes: &[u8],
) -> Result<DomRoot, ToolUiError> {
    titled_download(view, filename, mime, bytes, "Download")
}
/// Build a download row with the format's localized tooltip.
fn titled_download(
    view: &DomView,
    filename: &str,
    mime: &str,
    bytes: &[u8],
    title: &str,
) -> Result<DomRoot, ToolUiError> {
    let row = row(view)?;
    let mut control = download_button(
        view,
        DownloadButtonProps {
            mime_type: mime.to_owned(),
            title: Some(view.i18n.text(title)?),
            ..DownloadButtonProps::new(
                DownloadContent::Bytes(Uint8Array::from(bytes)),
                filename.to_owned(),
            )
        },
    )?;
    row.append_child(&control.node)
        .map_err(|error| ToolUiError::native(&error))?;
    Ok(DomRoot {
        node: row.into(),
        owner: control.owner.take(),
    })
}

/// Build literal-source copy and download controls in the shared header row.
/// # Errors
/// Returns localization or native control failures.
pub(crate) fn source(
    view: &DomView,
    filename: &str,
    mime: &str,
    content: &str,
    titles: (&str, &str),
) -> Result<DomRoot, ToolUiError> {
    let root = titled_download(view, filename, mime, content.as_bytes(), titles.1)?;
    let localization = std::rc::Rc::new(std::cell::RefCell::new(std::rc::Rc::clone(&view.i18n)));
    let copy = crate::components::copy_button::dom::create(
        &view.document,
        &localization,
        crate::components::copy_button::dom::FEEDBACK_MS,
        None,
        None,
    )
    .map_err(|error| ToolUiError::native(&error))?;
    for (name, value) in [("text", content), ("title", &view.i18n.text(titles.0)?)] {
        copy.set_attribute(name, value)
            .map_err(|error| ToolUiError::native(&error))?;
    }
    root.node
        .insert_before(&copy, root.node.first_child().as_ref())
        .map_err(|error| ToolUiError::native(&error))?;
    Ok(root)
}
