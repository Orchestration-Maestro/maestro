//! Browser-owned attachment content effects.
pub mod attachment_content;

/// Cancel-on-drop browser timers.
pub mod timer;

/// Native sandbox values.
#[cfg(target_arch = "wasm32")]
pub mod sandbox_value;

/// Retained browser event subscriptions.
#[cfg(target_arch = "wasm32")]
pub(crate) mod event_subscription;

/// Append the selected pack's escaped custom-property declarations to native inline CSS.
#[cfg(target_arch = "wasm32")]
pub(crate) fn append_brand_properties(
    style: &mut String,
    presentation: &maestro_theme::BrandPresentation<'_>,
) {
    use std::fmt::Write as _;
    for (key, value) in presentation.css_properties() {
        let _ = write!(style, "{key}:{value};");
    }
}

/// Complete-message socket effects.
pub mod websocket;

/// Install one native stylesheet per supplied document without retaining feature owners.
#[cfg(target_arch = "wasm32")]
pub(crate) fn install_stylesheet(
    document: &web_sys::Document,
    id: &str,
    source: &str,
) -> Result<(), attachment_content::AttachmentUiError> {
    use attachment_content::{AttachmentUiError, browser::host_error};
    if document.get_element_by_id(id).is_some() {
        return Ok(());
    }
    let style = document
        .create_element("style")
        .map_err(|error| host_error(&error))?;
    style.set_id(id);
    style.set_text_content(Some(source));
    document
        .document_element()
        .ok_or_else(|| AttachmentUiError::Host("Document has no root".into()))?
        .append_child(&style)
        .map_err(|error| host_error(&error))?;
    Ok(())
}
