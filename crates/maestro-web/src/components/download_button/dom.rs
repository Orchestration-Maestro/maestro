//! Native download control writing the caller's original bytes.
use crate::{
    browser::{attachment_content::AttachmentHostError, event_subscription::EventSubscription},
    components::button::{ButtonProps, ButtonSize, ButtonType, ButtonVariant, button},
    dom::{DomRoot, DomView},
    icons::{self, IconSize},
    tools::types::ToolUiError,
    utils::{base64, download::download_bytes},
};
use js_sys::Uint8Array;
use std::any::Any;
use wasm_bindgen::closure::Closure;
use web_sys::{Document, Node};

/// MIME type used when the caller supplies none.
const DEFAULT_MIME_TYPE: &str = "text/plain";
/// Visible label and tooltip key of the control.
const LABEL: &str = "Download";
/// Glyph shown beside the label.
const GLYPH: &str = "Download";

/// Original payload of a download control.
pub enum DownloadContent {
    /// Text, decoded as Base64 only when the caller forces it or it qualifies.
    Text(String),
    /// Bytes read from the caller's live native view at activation.
    Bytes(Uint8Array),
}
/// Operands of the native download control.
pub struct DownloadButtonProps {
    /// Payload to save.
    pub content: DownloadContent,
    /// Literal suggested download name.
    pub filename: String,
    /// MIME type of the saved Blob.
    pub mime_type: String,
    /// Custom tooltip; absence uses the localized label and emptiness stays empty.
    pub title: Option<String>,
    /// Whether the localized label also shows beside the glyph.
    pub show_text: bool,
    /// Button extent.
    pub size: ButtonSize,
    /// Button emphasis.
    pub variant: ButtonVariant,
    /// Glyph extent.
    pub icon_size: IconSize,
    /// Whether text is decoded as Base64 regardless of its shape.
    pub is_base64: bool,
}
impl DownloadButtonProps {
    /// Text content with a plain-text MIME type and no custom tooltip or label.
    #[must_use]
    pub fn new(content: DownloadContent, filename: String) -> Self {
        Self {
            content,
            filename,
            mime_type: DEFAULT_MIME_TYPE.into(),
            title: None,
            show_text: false,
            size: ButtonSize::Sm,
            variant: ButtonVariant::Ghost,
            icon_size: IconSize::Sm,
            is_base64: false,
        }
    }
}
/// Operands retained for each activation of one mounted control.
struct Activation {
    /// Document owning the scoped download resources.
    document: Document,
    /// Payload read when the control activates.
    content: DownloadContent,
    /// Literal suggested download name.
    filename: String,
    /// MIME type of the saved Blob.
    mime_type: String,
    /// Whether text is always decoded as Base64.
    is_base64: bool,
}
impl Activation {
    /// Save the original bytes once through the shared byte sink.
    fn write(&self) {
        let bytes = match &self.content {
            DownloadContent::Text(text) => text_bytes(text, self.is_base64),
            DownloadContent::Bytes(array) => array.to_vec(),
        };
        // Activation has no error channel; a failed download leaves the page unchanged.
        let _ = download_bytes(&self.document, &bytes, &self.filename, &self.mime_type);
    }
}
/// Create the native download control with its retained activation.
/// # Errors
/// Returns localization, glyph or native creation failures.
pub fn download_button(view: &DomView, props: DownloadButtonProps) -> Result<DomRoot, ToolUiError> {
    let mut children = vec![glyph(props.icon_size)?];
    if props.show_text {
        let label = view.i18n.text(LABEL).map_err(ToolUiError::I18n)?;
        let caption = view
            .document
            .create_element("span")
            .map_err(|error| ToolUiError::native(&error))?;
        caption.set_text_content(Some(&label));
        children.push(caption.into());
    }
    // The localized label is read only where the caller uses it: a custom tooltip replaces it.
    let title = match props.title {
        Some(title) => title,
        None => view.i18n.text(LABEL).map_err(ToolUiError::I18n)?,
    };
    let control = button(
        &view.document,
        &ButtonProps {
            variant: props.variant,
            size: props.size,
            r#type: ButtonType::Button,
            title: Some(&title),
            ..ButtonProps::default()
        },
        &children,
    )
    .map_err(|error| ToolUiError::native(&error))?;
    let activation = Activation {
        document: view.document.clone(),
        content: props.content,
        filename: props.filename,
        mime_type: props.mime_type,
        is_base64: props.is_base64,
    };
    let click = Closure::<dyn Fn()>::new(move || activation.write());
    let subscription = EventSubscription::new(control.clone().into(), "click", click)
        .map_err(|error| ToolUiError::native(&error))?;
    Ok(DomRoot {
        node: control.into(),
        owner: Some(Box::new(subscription) as Box<dyn Any>),
    })
}
/// The control's glyph at the requested extent.
fn glyph(size: IconSize) -> Result<Node, ToolUiError> {
    let node = icons::icon_node(GLYPH)
        .map_err(|error| ToolUiError::Host(AttachmentHostError::Presentation(error.to_string())))?
        .ok_or_else(|| {
            ToolUiError::Host(AttachmentHostError::Presentation(format!(
                "Missing icon {GLYPH}"
            )))
        })?;
    icons::icon_dom(&node, size, None)
        .map(Into::into)
        .map_err(|error| ToolUiError::native(&error))
}
/// Bytes saved for text content, decoded only when it is forced or qualifies.
fn text_bytes(text: &str, forced: bool) -> Vec<u8> {
    if (forced || (text.len() >= 4 && text.len().is_multiple_of(4) && base64_shaped(text)))
        && let Ok(bytes) = base64::decode(text)
    {
        return bytes;
    }
    text.as_bytes().to_vec()
}
/// Whether `text` is a complete Base64 frame: the alphabet, then at most two `=`.
fn base64_shaped(text: &str) -> bool {
    let body = text.trim_end_matches('=');
    text.len() - body.len() <= 2
        && body
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/'))
}
