//! Literal message-link preparation with weak asynchronous feedback delivery.
use crate::browser::{
    attachment_content::{self, AttachmentHost, AttachmentUiError},
    copy_feedback::CopyFeedback,
};
use std::rc::Rc;
mod draw;
pub use draw::{CopyLinkView, render_copy_link_button};

/// One retained message control with an owned restoring timer.
pub struct CopyLinkState {
    /// Replaceable effects owner.
    host: Rc<dyn AttachmentHost>,
    /// Stable toolkit identity, unrelated to literal message identity.
    id: egui::Id,
    /// Pending clipboard work cannot retain this UI owner.
    feedback: CopyFeedback,
    /// Primary gesture admitted at its first endpoint.
    pressed: Option<()>,
    /// Mounted property histories, absent before the first resolved style.
    transitions: Option<draw::CopyLinkTransitions>,
}
impl CopyLinkState {
    /// Retain the supplied browser effects and a stable control identity.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            host,
            id: attachment_content::identity(),
            feedback: CopyFeedback::default(),
            pressed: None,
            transitions: None,
        }
    }
    /// Begin the native effect before scheduling its owned completion.
    fn copy(&self, text: &str, context: &egui::Context) {
        self.feedback.copy(
            &self.host,
            (
                text,
                crate::browser::attachment_content::ClipboardMode::Fallback,
            ),
            1500,
            context,
        );
    }
}
/// Escapes query syntax while preserving healthy opaque-key punctuation.
const KEY_ESCAPES: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
    .add(b'%')
    .add(b'&')
    .add(b'=')
    .add(b'+');
/// Prepare an activation-time URL without converting identities into markup.
fn build_share_url(entry: &str, view: &CopyLinkView<'_>) -> Result<String, AttachmentUiError> {
    let mut url = url::Url::parse(view.location_href)
        .map_err(|error| AttachmentUiError::Presentation(error.to_string()))?;
    let selected = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("leafId", view.current_leaf_id)
        .append_pair("targetId", entry)
        .finish();
    if let Some(base) = view.share_base_url {
        let (base, fragment) = base
            .split_once('#')
            .map_or((base, None), |(base, tail)| (base, Some(tail)));
        return Ok(format!(
            "{base}&{selected}{}",
            fragment.map_or_else(String::new, |tail| format!("#{tail}"))
        ));
    }
    let mut seen = std::collections::HashSet::new();
    let key = url.query_pairs().find_map(|(key, value)| {
        (seen.insert(key.to_string()) && value.is_empty() && key != "leafId" && key != "targetId")
            .then_some(key.into_owned())
    });
    let query = match key.filter(|key| !key.is_empty()) {
        Some(key) => {
            let escaped = percent_encoding::utf8_percent_encode(&key, KEY_ESCAPES);
            format!("{escaped}&{selected}")
        }
        None => selected,
    };
    url.set_query(Some(&query));
    Ok(url.into())
}
