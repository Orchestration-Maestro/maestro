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
    let row = row(view)?;
    let mut control = download_button(
        view,
        DownloadButtonProps {
            mime_type: mime.to_owned(),
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
