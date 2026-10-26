//! Native original-byte downloads with scoped URL and anchor ownership.
use crate::browser::attachment_content::{AttachmentHostError, browser::host_error};
use wasm_bindgen::JsCast as _;
use web_sys::{Document, HtmlAnchorElement, Url};
/// Resources released on either successful click or a later platform failure.
struct Download {
    /// Temporary object URL.
    url: ObjectUrl,
    /// Temporary DOM anchor, if created.
    anchor: Option<HtmlAnchorElement>,
}
impl Drop for Download {
    fn drop(&mut self) {
        if let Some(anchor) = &self.anchor {
            anchor.remove();
        }
    }
}
/// Use the owning window's native decoder and Blob implementation.
pub(crate) fn download(
    document: &Document,
    attachment: &crate::SharedAttachment,
) -> Result<(), AttachmentHostError> {
    let (content, name, mime) = {
        let a = attachment
            .try_read()
            .map_err(|_| AttachmentHostError::BorrowConflict)?;
        (a.content.clone(), a.file_name.clone(), a.mime_type.clone())
    };
    let bytes = document
        .default_view()
        .ok_or_else(|| AttachmentHostError::Host("Document has no window".into()))?
        .atob(&content)
        .map_err(|error| host_error(&error))?
        .chars()
        .map(|c| u8::try_from(u32::from(c)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AttachmentHostError::Host(e.to_string()))?;
    download_bytes(document, &bytes, &name, &mime)
}
/// Use the single scoped Blob/anchor owner for caller-supplied bytes.
pub(crate) fn download_bytes(
    document: &Document,
    bytes: &[u8],
    name: &str,
    mime: &str,
) -> Result<(), AttachmentHostError> {
    let mut owner = Download {
        url: ObjectUrl::new(bytes, mime)?,
        anchor: None,
    };
    let anchor = document
        .create_element("a")
        .map_err(|error| host_error(&error))?
        .dyn_into::<HtmlAnchorElement>()
        .map_err(|e| host_error(&e.into()))?;
    anchor.set_href(owner.url.as_str());
    anchor.set_download(name);
    owner.anchor = Some(anchor.clone());
    document
        .body()
        .ok_or_else(|| AttachmentHostError::Host("Document has no body".into()))?
        .append_child(&anchor)
        .map_err(|error| host_error(&error))?;
    anchor.click();
    Ok(())
}

/// One Blob URL revoked when its final resource owner is dropped.
pub(crate) struct ObjectUrl(String);
impl ObjectUrl {
    /// Allocate a URL containing the supplied original bytes.
    pub(crate) fn new(bytes: &[u8], mime: &str) -> Result<Self, AttachmentHostError> {
        let options = web_sys::BlobPropertyBag::new();
        options.set_type(mime);
        let data = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&data, &options)
            .map_err(|error| host_error(&error))?;
        Url::create_object_url_with_blob(&blob)
            .map(Self)
            .map_err(|error| host_error(&error))
    }
    /// Borrow the native resource address.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}
impl Drop for ObjectUrl {
    fn drop(&mut self) {
        let _ = Url::revoke_object_url(&self.0);
    }
}
