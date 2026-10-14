//! Native original-byte downloads with scoped URL and anchor ownership.
use crate::{
    Attachment,
    browser::attachment_content::{AttachmentUiError, browser::host_error},
};
use std::cell::RefCell;
use wasm_bindgen::JsCast as _;
use web_sys::{Document, HtmlAnchorElement, Url};
/// Resources released on either successful click or a later platform failure.
struct Download {
    /// Temporary object URL.
    url: String,
    /// Temporary DOM anchor, if created.
    anchor: Option<HtmlAnchorElement>,
}
impl Drop for Download {
    fn drop(&mut self) {
        if let Some(anchor) = &self.anchor {
            anchor.remove();
        }
        let _ = Url::revoke_object_url(&self.url);
    }
}
/// Use the owning window's native decoder and Blob implementation.
pub(crate) fn download(
    document: &Document,
    attachment: &RefCell<Attachment>,
) -> Result<(), AttachmentUiError> {
    let (content, name, mime) = {
        let a = attachment
            .try_borrow()
            .map_err(|_| AttachmentUiError::BorrowConflict)?;
        (a.content.clone(), a.file_name.clone(), a.mime_type.clone())
    };
    let bytes = document
        .default_view()
        .ok_or_else(|| AttachmentUiError::Host("Document has no window".into()))?
        .atob(&content)
        .map_err(|error| host_error(&error))?
        .chars()
        .map(|c| u8::try_from(u32::from(c)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AttachmentUiError::Host(e.to_string()))?;
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(&mime);
    let data = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes.as_slice()));
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&data, &options)
        .map_err(|error| host_error(&error))?;
    let mut owner = Download {
        url: Url::create_object_url_with_blob(&blob).map_err(|error| host_error(&error))?,
        anchor: None,
    };
    let anchor = document
        .create_element("a")
        .map_err(|error| host_error(&error))?
        .dyn_into::<HtmlAnchorElement>()
        .map_err(|e| host_error(&e.into()))?;
    anchor.set_href(&owner.url);
    anchor.set_download(&name);
    owner.anchor = Some(anchor.clone());
    document
        .body()
        .ok_or_else(|| AttachmentUiError::Host("Document has no body".into()))?
        .append_child(&anchor)
        .map_err(|error| host_error(&error))?;
    anchor.click();
    Ok(())
}
